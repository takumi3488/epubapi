use epubapi::minio::minio::get_client;

use std::{
    env::var,
    error::Error,
    fs::{create_dir_all, remove_dir_all, File},
    io::{Read, Write},
    process::Command,
    result::Result,
};

use aws_sdk_s3::primitives::ByteStream;

use img2epub::img2epub;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 環境変数の読み込み
    let images_bucket: &str = &var("IMAGES_BUCKET").expect("IMAGES_BUCKET is not set");
    let epub_bucket: &str = &var("EPUB_BUCKET").expect("EPUB_BUCKET is not set");

    // クライアントの初期化
    let minio_client = get_client().await;

    // images_bucketに未処理のオブジェクトがあれば処理する
    let objects = minio_client
        .list_objects_v2()
        .bucket(images_bucket)
        .send()
        .await?
        .contents
        .unwrap();

    for object in objects {
        let uuid = Uuid::new_v4();

        // オブジェクトのダウンロード
        let key = object.key.unwrap();
        let body = minio_client
            .get_object()
            .bucket(images_bucket)
            .key(&key)
            .send()
            .await?
            .body;

        // .tar.gzを.epubに変換する
        let key_base = key.split("/").last().unwrap();
        let out_base = key_base.replace(".tar.gz", ".epub");
        let out = key.as_str().replace(".tar.gz", ".epub");

        // ByteStreamに変換する
        let body: ByteStream = convert_to_epub(uuid, body, key_base, &out_base).await?;

        // オブジェクトのアップロード
        minio_client
            .put_object()
            .bucket(epub_bucket)
            .key(&out)
            .body(body)
            .send()
            .await?;

        // オブジェクトの削除
        minio_client
            .delete_object()
            .bucket(images_bucket)
            .key(&key)
            .send()
            .await?;

        // ログを出力する
        println!("{}/{} -> {}/{}", images_bucket, key, epub_bucket, out);
    }

    Ok(())
}

/// .tar.gzをepubに変換する
/// 1. 作業ディレクトリを作成する
/// 2. .tar.gzを保存する
/// 3. .tar.gzを解凍する
/// 4. 解凍したファイルをepubに変換する
/// 5. 作業ディレクトリを削除する
/// 6. ByteStreamに変換する
/// 7. ByteStreamを返す
async fn convert_to_epub(
    uuid: Uuid,           // ランダムなUUID
    mut body: ByteStream, // .tar.gzのByteStream
    name: &str,           // .tar.gzのファイル名
    out: &str,            // epubのファイル名
) -> Result<ByteStream, Box<dyn Error>> {
    // 作業ディレクトリを作成する
    let work_dir = format!("/tmp/{}", uuid);
    create_dir_all(&work_dir)?;

    // .tar.gzを保存する
    let tar_path = format!("{}/{}", work_dir, name);
    let mut file = File::create(&tar_path)?;
    while let Some(chunk) = body.next().await {
        file.write_all(&chunk?)?;
    }

    // .tar.gzを解凍する
    Command::new("tar")
        .arg("-xvf")
        .arg(&tar_path)
        .current_dir(&work_dir)
        .spawn()?
        .wait()?;

    // 解凍したファイルをepubに変換する
    let out = format!("{}/{}", work_dir, out);
    img2epub(&work_dir, &out, None, None, None, None, None)?;

    // ByteStreamに変換する
    let mut file = File::open(&out)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let bs = ByteStream::from(buf);

    // 作業ディレクトリを削除する
    remove_dir_all(&work_dir)?;

    Ok(bs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    /// convert_to_epubのテスト
    async fn test_convert_to_epub() {
        let uuid = Uuid::new_v4();
        let body = ByteStream::from_path(Path::new("./test_assets/test.tar.gz"))
            .await
            .unwrap();
        let mut body: ByteStream = convert_to_epub(uuid, body, "test.tar.gz", "test.epub")
            .await
            .unwrap();

        // バイナリを見てZIPファイルかどうかを確認する
        let mut buf = Vec::new();
        while let Some(chunk) = body.next().await {
            buf.extend(chunk.unwrap());
        }
        assert_eq!(buf[0..2], [0x50, 0x4b]);
    }
}

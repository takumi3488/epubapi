use epubapi::minio::get_client;

use std::{
    env::var,
    error::Error,
    fs::{create_dir_all, remove_dir_all, File},
    io::{Read, Write},
    process::Command,
};

use aws_sdk_s3::primitives::ByteStream;

use img2epub::img2epub;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
struct Metadata {
    tags: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("epub2img start");
    // 環境変数の読み込み
    let endpoint = var("S3_ENDPOINT").expect("S3_ENDPOINT is not set");
    let images_bucket: &str = &var("IMAGES_BUCKET").expect("IMAGES_BUCKET is not set");
    let epub_bucket: &str = &var("EPUB_BUCKET").expect("EPUB_BUCKET is not set");
    let unconvertable_images_bucket: &str =
        &var("UNCONVERTABLE_IMAGES_BUCKET").expect("UNCONVERTABLE_IMAGES_BUCKET is not set");

    // クライアントの初期化
    let minio_client = get_client(&endpoint).await;

    // images_bucketに未処理のオブジェクトがあれば処理する
    let objects = match minio_client
        .list_objects_v2()
        .bucket(images_bucket)
        .send()
        .await
        .expect("Failed to list objects")
        .contents
    {
        Some(objects) => objects,
        None => {
            println!("No objects to process");
            return Ok(());
        }
    };

    for object in objects {
        let uuid = Uuid::new_v4();

        // オブジェクトのダウンロード
        let key = object.key.unwrap();
        if !key.ends_with(".tar.gz") {
            println!("Skip: {}", key);
            continue;
        }
        let body = minio_client
            .get_object()
            .bucket(images_bucket)
            .key(&key)
            .send()
            .await
            .expect("Failed to download object")
            .body;

        // .tar.gzを.epubに変換する
        let key_base = key.split("/").last().unwrap();
        let out_base = key_base.replace(".tar.gz", ".epub");
        let out = key.as_str().replace(".tar.gz", ".epub");

        // ByteStreamに変換する
        let (body, tags) = match convert_to_epub_with_tags(uuid, body, key_base, &out_base).await {
            Ok((body, tags)) => (body, tags),
            Err(e) => {
                if e.to_string().contains("No image files found") {
                    println!("No image files found: {}", key);
                    minio_client
                        .copy_object()
                        .copy_source(format!("{}/{}", images_bucket, key))
                        .bucket(unconvertable_images_bucket)
                        .key(&key)
                        .send()
                        .await?;
                    minio_client
                        .delete_object()
                        .bucket(images_bucket)
                        .key(&key)
                        .send()
                        .await?;
                }
                continue;
            }
        };

        // オブジェクトのアップロード
        minio_client
            .put_object()
            .bucket(epub_bucket)
            .key(&out)
            .body(body)
            .send()
            .await?;

        // tagsファイルをアップロードする
        minio_client
            .put_object()
            .bucket(epub_bucket)
            .key(format!("{}.tags", out.replace(".epub", "")))
            .body(tags)
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
/// 4. 解凍したファイルからtagsを取得する
/// 5. 解凍したファイルをepubに変換する
/// 6. 作業ディレクトリを削除する
/// 7. ByteStreamに変換する
/// 8. ByteStreamとtagsを返す
async fn convert_to_epub_with_tags(
    uuid: Uuid,           // ランダムなUUID
    mut body: ByteStream, // .tar.gzのByteStream
    name: &str,           // .tar.gzのファイル名
    out: &str,            // epubのファイル名
) -> Result<(ByteStream, ByteStream), Box<dyn Error>> {
    println!("Start converting to epub: {} → {}", name, out);

    // 作業ディレクトリを作成する
    let work_dir = format!("/tmp/{}", uuid);
    create_dir_all(&work_dir).expect("Failed to create work directory");

    // .tar.gzを保存する
    let tar_path = format!("{}/{}", work_dir, name);
    let mut file = File::create(&tar_path).expect("Failed to create file");
    while let Some(chunk) = body.next().await {
        file.write_all(&chunk?)?;
    }

    // .tar.gzを解凍する
    Command::new("tar")
        .arg("-xvf")
        .arg(name)
        .current_dir(&work_dir)
        .spawn()
        .expect("Failed to execute command")
        .wait()
        .expect("Failed to wait for command");

    // 解凍したファイルからtagsを取得する
    let tags = match File::open(format!("{}/metadata.json", work_dir)) {
        Ok(mut file) => {
            let mut buf = String::new();
            match file.read_to_string(&mut buf) {
                Ok(_) => {
                    let metadata: Metadata =
                        serde_json::from_str(&buf).expect("Failed to parse JSON");
                    metadata.tags
                }
                Err(_) => Vec::new(),
            }
        }
        Err(_) => Vec::new(),
    }
    .join("\n")
    .into_bytes();
    let tags = ByteStream::from(tags);

    // 解凍したファイルをepubに変換する
    let out = format!("{}/{}", work_dir, out);
    img2epub(&work_dir, &out, None, None, None, None, None)?;

    // ByteStreamに変換する
    let mut file = File::open(&out).expect("Failed to open file");
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).expect("Failed to read file");
    let bs = ByteStream::from(buf);

    // 作業ディレクトリを削除する
    remove_dir_all(&work_dir).expect("Failed to remove work directory");

    Ok((bs, tags))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    /// convert_to_epubのテスト
    async fn test_convert_to_epub_with_tags() {
        let uuid = Uuid::new_v4();
        let body = ByteStream::from_path(Path::new("./test_assets/images/test1.tar.gz"))
            .await
            .unwrap();
        let (mut body, mut tags) =
            convert_to_epub_with_tags(uuid, body, "test1.tar.gz", "test.epub")
                .await
                .unwrap();

        // バイナリを見てZIPファイルかどうかを確認する
        let mut buf = Vec::new();
        while let Some(chunk) = body.next().await {
            buf.extend(chunk.unwrap());
        }
        assert_eq!(buf[0..2], [0x50, 0x4b]);

        // tagsを見て内容が正しいかを確認する
        let mut buf = Vec::new();
        while let Some(chunk) = tags.next().await {
            buf.extend(chunk.unwrap());
        }
        assert_eq!(buf, b"tag1\ntag2\ntag3");
    }
}

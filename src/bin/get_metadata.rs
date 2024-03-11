
use epub::doc::EpubDoc;
use epubapi::{db::db::connect_db, minio::minio::get_client};
use sqlx::query;
use uuid::Uuid;
use std::{env::var, fs::File, io::Write};

#[tokio::main]
async fn main() {
    // 環境変数の読み込み
    let epub_bucket: &str = &var("EPUB_BUCKET").expect("EPUB_BUCKET is not set");

    // クライアントの初期化
    let db_client = connect_db().await;
    let minio_client = get_client().await;

    // ユーザーIDを取得する
    let user_ids: Vec<String> = query!("SELECT id FROM users")
        .fetch_all(&db_client)
        .await
        .unwrap()
        .iter()
        .map(|row| row.id.as_str().to_string())
        .collect();

    // ブックKEYを取得する
    let book_keys: Vec<String> = query!("SELECT key FROM books")
        .fetch_all(&db_client)
        .await
        .unwrap()
        .iter()
        .map(|row| row.key.as_str().to_string())
        .collect();

    println!("user_ids: {:?}", user_ids);
    println!("book_keys: {:?}", book_keys);

    // epub_bucketに未処理のオブジェクトがあれば処理する
    let mut response = minio_client
        .list_objects_v2()
        .bucket(epub_bucket)
        .max_keys(10)
        .into_paginator()
        .send();

    while let Some(result) = response.next().await {
        let objects = result.unwrap().contents.unwrap();

        // 存在するユーザーIDで存在しないブックKEYのオブジェクトを処理する
        for object in objects
            .iter()
            .filter(|&object| !book_keys.contains(&object.key().unwrap().to_string()))
            .filter(|&object| {
                user_ids.contains(&object.key().unwrap().split("/").nth(0).unwrap().to_string())
            })
        {
            let key = object.key().unwrap();
            println!("{}のメタデータを取得中...", key);
            let mut output = minio_client
                .get_object()
                .bucket(epub_bucket)
                .key(key)
                .send()
                .await
                .unwrap();

            // /tmpに保存する
            let tmp_path: String = format!("/tmp/{}", Uuid::new_v4());
            let mut file: File = File::create(&tmp_path).unwrap();
            while let Some(bytes) = output.body.try_next().await.unwrap() {
                file.write_all(&bytes).unwrap();
            }

            // メタデータを取得する
            let mut metadata = EpubDoc::new(&tmp_path).unwrap();

            // メタデータをDBに保存する
            query!(
                r#"INSERT INTO books (
                    key,
                    owner_id,
                    name,
                    creator,
                    publisher,
                    date,
                    cover_image
                ) VALUES (
                    $1,
                    $2,
                    $3,
                    $4,
                    $5,
                    $6,
                    $7
                )"#,
                key,
                key.split('/').nth(0).unwrap(),
                metadata.mdata("title").unwrap(),
                metadata.mdata("creator").unwrap(),
                metadata.mdata("publisher").unwrap(),
                metadata.mdata("date").unwrap(),
                metadata.get_cover().unwrap().0
            )
            .execute(&db_client)
            .await
            .unwrap();

            println!("{}のメタデータを保存しました", key);

            // /tmpのファイルを削除する
            std::fs::remove_file(&tmp_path).unwrap();
        }
    }
}

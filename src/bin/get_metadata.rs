use aws_sdk_s3::primitives::ByteStream;
use chrono::Local;
use epub::doc::EpubDoc;
use epubapi::{db::connect_db, minio::get_client, service::book::model::Direction};
use sqlx::{query, query_as};
use std::{
    env::var,
    fs::File,
    io::{Cursor, Write},
};
use tokio::fs::create_dir;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct Tag {
    name: String,
}

#[tokio::main]
async fn main() {
    println!("get_metadata start");
    // 環境変数の読み込み
    let endpoint = var("S3_ENDPOINT").expect("S3_ENDPOINT is not set");
    let epub_bucket: &str = &var("EPUB_BUCKET").expect("EPUB_BUCKET is not set");

    // クライアントの初期化
    let db_client = connect_db().await;
    let minio_client = get_client(&endpoint).await;

    // ユーザーIDを取得する
    let user_ids: Vec<String> = query!("SELECT id FROM users")
        .fetch_all(&db_client)
        .await
        .unwrap()
        .iter()
        .map(|row| row.id.as_str().to_string())
        .collect();

    // bookのkeyを取得する
    let book_keys: Vec<String> = query!("SELECT key FROM books")
        .fetch_all(&db_client)
        .await
        .unwrap()
        .iter()
        .map(|row| row.key.as_str().to_string())
        .collect();

    println!("user_ids: {:?}", user_ids);
    println!("book_keys: {:?}", book_keys);

    // 投稿済みのtagを取得する
    let mut exist_tags = query_as!(Tag, "SELECT name FROM tags")
        .fetch_all(&db_client)
        .await
        .unwrap()
        .iter()
        .map(|tag| tag.name.clone())
        .collect::<Vec<String>>();

    // epub_bucketに未処理のオブジェクトがあれば処理する
    let mut response = minio_client
        .list_objects_v2()
        .bucket(epub_bucket)
        .max_keys(10)
        .into_paginator()
        .send();

    while let Some(result) = response.next().await {
        let objects = match result.unwrap().contents {
            Some(objects) => objects,
            None => continue,
        };

        // 存在するユーザーIDで存在しないブックKEYのオブジェクトを処理する
        for object in objects
            .iter()
            .filter(|&object| !book_keys.contains(&object.key().unwrap().to_string()))
            .filter(|&object| {
                user_ids.contains(&object.key().unwrap().split("/").next().unwrap().to_string())
            })
        {
            let key = object.key().unwrap();
            println!("{}のメタデータを取得中...", key);
            let uuid = Uuid::new_v4().to_string();
            let mut output = minio_client
                .get_object()
                .bucket(epub_bucket)
                .key(key)
                .send()
                .await
                .unwrap();

            // /tmpに保存する
            let _ = create_dir("/tmp").await;
            let tmp_path: String = format!("/tmp/{}", Uuid::new_v4());
            let mut file: File = File::create(&tmp_path).unwrap();
            while let Some(bytes) = output.body.try_next().await.unwrap() {
                file.write_all(&bytes).unwrap();
            }

            // メタデータを取得する
            let mut metadata = EpubDoc::new(&tmp_path).unwrap();
            let direction = if metadata
                .mdata("primary-writing-mode")
                .is_some_and(|d| d == "vertical-rl")
            {
                Direction::Rtl
            } else {
                Direction::Ltr
            };

            // タグを取得する
            let res = minio_client
                .get_object()
                .bucket(epub_bucket)
                .key(format!("{}.tags", key.replace(".epub", "")))
                .send()
                .await
                .unwrap();
            let tags_bytes = res.body.bytes().expect("Failed to get tags");
            let tags = String::from_utf8(tags_bytes.to_vec())
                .unwrap()
                .split('\n')
                .filter_map(|tag| {
                    if tag.is_empty() {
                        None
                    } else {
                        Some(tag.to_string())
                    }
                })
                .collect::<Vec<String>>();

            // カバー画像をMinioに保存する
            let cover_image_bytes = metadata.get_cover().unwrap().0;
            let img = image::load_from_memory(&cover_image_bytes).unwrap();
            let resized_img = img.resize(
                img.width().min(500),
                img.height().min(500),
                image::imageops::FilterType::Lanczos3,
            );
            let mut resized_image_bytes = Vec::new();
            resized_img
                .write_to(
                    &mut Cursor::new(&mut resized_image_bytes),
                    image::ImageFormat::Avif,
                )
                .unwrap();
            let cover_image_byte_stream = ByteStream::from(resized_image_bytes);
            let cover_image_key = format!("{}.avif", uuid);
            minio_client
                .put_object()
                .bucket(epub_bucket)
                .key(&cover_image_key)
                .body(cover_image_byte_stream)
                .content_type("image/avif")
                .send()
                .await
                .unwrap();

            // メタデータをDBに保存する
            query!(
                r#"INSERT INTO books (
                    id,
                    key,
                    owner_id,
                    name,
                    creator,
                    publisher,
                    date,
                    cover_image,
                    direction
                ) VALUES (
                    $1,
                    $2,
                    $3,
                    $4,
                    $5,
                    $6,
                    $7,
                    $8,
                    $9
                )"#,
                uuid,
                key,
                key.split('/').nth(0).unwrap(),
                metadata.mdata("title").unwrap(),
                metadata.mdata("creator").unwrap_or_default(),
                metadata.mdata("publisher").unwrap_or_default(),
                metadata.mdata("date").unwrap_or(Local::now().to_rfc3339()),
                cover_image_key,
                direction as _,
            )
            .execute(&db_client)
            .await
            .unwrap();

            println!("{}のメタデータを保存しました", key);

            // tagをDBに保存する
            for tag in tags {
                if exist_tags.iter().any(|t| t == &tag) {
                    query!(r#"INSERT INTO tags (name) VALUES ($1)"#, tag)
                        .execute(&db_client)
                        .await
                        .unwrap();
                    exist_tags.push(tag.clone());
                }
                query!(
                    r#"INSERT INTO book_tags (book_id, tag_name) VALUES ($1, $2)"#,
                    uuid,
                    tag
                )
                .execute(&db_client)
                .await
                .unwrap();
            }

            // /tmpのファイルを削除する
            std::fs::remove_file(&tmp_path).unwrap();
        }
    }
}

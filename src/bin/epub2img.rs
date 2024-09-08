use aws_sdk_s3::primitives::ByteStream;
use epubapi::{
    db::connect_db,
    minio::get_client,
    service::book::model::{BookKey, BookLayout, BookUpdateImagesResponse},
};
use std::{
    env::var,
    error::Error,
    fs::{read_to_string, File},
    io::Write,
    path::Path,
    process::Command,
};
use tokio::fs::create_dir_all;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 環境変数の読み込み
    let endpoint = var("S3_ENDPOINT").expect("S3_ENDPOINT is not set");
    let epub_bucket: &str = &var("EPUB_BUCKET").expect("EPUB_BUCKET is not set");
    let out_images_bucket: &str = &var("OUT_IMAGES_BUCKET").expect("OUT_IMAGES_BUCKET is not set");
    let _ = &var("DATABASE_URL").expect("DATABASE_URL is not set");

    // DBに接続する
    let db = connect_db().await;

    // 未処理のbookのkeyを取得する
    let keys = sqlx::query_as!(BookKey, r#"SELECT key FROM books WHERE layout isnull"#)
        .fetch_all(&db)
        .await?;

    // Minioクライアントの初期化
    let minio_client = get_client(&endpoint).await;

    for key in keys {
        // epubファイルをダウンロードする
        let mut epub_stream = minio_client
            .get_object()
            .bucket(epub_bucket)
            .key(&key.key)
            .send()
            .await?;
        let file_path = format!("/tmp/{}", key.key);
        println!("file_path: {}", file_path);
        create_dir_all(Path::new(&file_path).parent().unwrap())
            .await
            .expect("Failed to create dir");
        let mut epub_file = File::create(&file_path).expect("Failed to create epub file to write");
        while let Some(bytes) = epub_stream.body.try_next().await? {
            epub_file.write_all(&bytes)?;
        }

        // epubを展開
        let work_dir = format!("/tmp/{}", key.key.replace(".epub", ""));
        create_dir_all(&work_dir)
            .await
            .expect("Failed to create dir to extract epub");
        Command::new("unzip")
            .arg(&file_path)
            .arg("-d")
            .arg(&work_dir)
            .output()
            .expect("Failed to unzip epub");

        // container.xml から rootfile の full-path を取得
        let content_path = roxmltree::Document::parse(
            &read_to_string(format!("{}/META-INF/container.xml", work_dir))
                .expect("container.xmlが見つかりませんでした"),
        )?
        .descendants()
        .find(|n| n.tag_name().name() == "rootfile")
        .unwrap()
        .attribute("full-path")
        .unwrap()
        .to_string();
        let content_path = Path::new(&work_dir).join(&content_path);

        // rendition:layout が pre-paginated であるか確認
        let layout = roxmltree::Document::parse(&read_to_string(&content_path)?)?
            .descendants()
            .find(|n| {
                n.tag_name().name() == "meta" && n.attribute("property") == Some("rendition:layout")
            })
            .map(|n| n.text().unwrap())
            .unwrap_or("reflowable")
            .to_string();
        if &layout == "reflowable" {
            // DBのみ更新して終了
            sqlx::query!(
                r#"UPDATE books SET layout = $1 WHERE key = $2"#,
                BookLayout::Reflowable as BookLayout,
                key.key
            )
            .execute(&db)
            .await?;
            println!("skip reflowable book: {}", key.key);
            continue;
        } else if &layout != "pre-paginated" {
            panic!("rendition:layout が不正です");
        }

        // 画像ファイルのパスを取得
        let content_xml = read_to_string(&content_path).unwrap();
        let doc = roxmltree::Document::parse(&content_xml).unwrap();
        let xhtml_paths = doc
            .descendants()
            .filter(|n| n.tag_name().name() == "itemref")
            .map(|n| n.attribute("idref").unwrap().to_string())
            .map(|idref| {
                let doc = roxmltree::Document::parse(&content_xml).unwrap();
                let node = doc
                    .descendants()
                    .find(|n| n.tag_name().name() == "item" && n.attribute("id") == Some(&idref))
                    .unwrap();
                content_path
                    .parent()
                    .unwrap()
                    .join(node.attribute("href").unwrap())
            });
        let image_paths = xhtml_paths
            .flat_map(|xhtml_path| {
                let xhtml = &read_to_string(&xhtml_path).unwrap();
                let doc = roxmltree::Document::parse_with_options(
                    xhtml,
                    roxmltree::ParsingOptions {
                        allow_dtd: true,
                        nodes_limit: u32::MAX,
                    },
                )
                .unwrap();
                doc.descendants()
                    .filter(|n| n.tag_name().name() == "img")
                    .map(|n| n.attribute("src").unwrap().to_string())
                    .map(|src| xhtml_path.parent().unwrap().join(src))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        // 画像ファイルをMinIOにアップロード
        let mut keys = Vec::new();
        for image_path in image_paths {
            let key = format!(
                "{}.{}",
                uuid::Uuid::new_v4(),
                image_path.extension().unwrap().to_str().unwrap()
            );
            let body = ByteStream::from_path(&image_path).await.unwrap();
            minio_client
                .put_object()
                .bucket(out_images_bucket)
                .key(&key)
                .body(body)
                .send()
                .await
                .unwrap();
            keys.push(key);
        }

        // DBを更新
        let response = sqlx::query_as!(
            BookUpdateImagesResponse,
            r#"UPDATE books SET layout = $1, images = $2 WHERE key = $3 RETURNING key, layout as "layout: BookLayout", images"#,
            BookLayout::PrePaginated as BookLayout,
            &keys,
            key.key
        ).fetch_one(&db).await?;
        println!("{:?}", response);
    }

    Ok(())
}

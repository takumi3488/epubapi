use std::{env, time::Duration};

use aws_sdk_s3::presigning::PresigningConfig;
use axum::extract::Multipart;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use sqlx::{types::chrono::NaiveDateTime, PgPool};
use utoipa::{IntoParams, ToSchema};

use crate::{minio, service::user::model::is_admin};

#[derive(Serialize, Deserialize, Debug, sqlx::Type, ToSchema, PartialEq, Clone, Copy)]
#[sqlx(type_name = "visibility", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Serialize, Deserialize, Debug, sqlx::Type, ToSchema, PartialEq, Clone, Copy)]
#[sqlx(type_name = "direction", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Ltr,
    Rtl,
}

#[derive(sqlx::FromRow, Serialize, Deserialize)]
pub struct Book {
    pub id: String,
    pub key: String,
    pub owner_id: String,
    pub name: String,
    pub creator: String,
    pub publisher: String,
    pub date: String,
    pub cover_image: String,
    pub visibility: Visibility,
    pub direction: Direction,
    pub created_at: NaiveDateTime,
    pub layout: Option<BookLayout>,
    pub images: Vec<String>,
}

#[derive(Serialize, Debug, sqlx::Type, ToSchema, Deserialize, Clone, Copy)]
#[sqlx(type_name = "layout", rename_all = "lowercase")]
pub enum BookLayout {
    Reflowable,
    #[sqlx(rename = "pre-paginated")]
    PrePaginated,
}

#[derive(ToSchema, Serialize, Deserialize)]
pub struct GetBooksResponse {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub creator: String,
    pub publisher: String,
    pub date: String,
    pub cover_image: String,
    #[schema(inline)]
    pub visibility: Visibility,
    #[schema(value_type = String, format = Date)]
    pub created_at: NaiveDateTime,
    pub tags: Vec<String>,
}

#[derive(ToSchema, Serialize, Deserialize)]
pub struct GetBookDetailsResponse {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub creator: String,
    pub publisher: String,
    pub date: String,
    pub cover_image: String,
    #[schema(inline)]
    pub visibility: Visibility,
    #[schema(inline)]
    pub direction: Direction,
    #[schema(value_type = String, format = Date)]
    pub created_at: NaiveDateTime,
    pub tags: Vec<String>,
    pub epub_url: String,
    #[schema(inline)]
    pub layout: Option<BookLayout>,
    pub images: Vec<String>,
}

#[derive(Serialize, Deserialize, ToSchema, IntoParams, Clone)]
#[into_params(style = Form, parameter_in = Query)]
pub struct BookQuery {
    pub page: Option<u32>,
    pub keyword: Option<String>,
    pub tag: Option<String>,
}

pub struct Epub {
    pub file: Multipart,
}

#[derive(ToSchema, Serialize, Deserialize)]
pub struct AddTagRequest {
    pub tag_name: String,
}

#[derive(ToSchema, Serialize, Deserialize)]
pub struct DeleteTagRequest {
    pub tag_name: String,
}

#[derive(ToSchema, Serialize, Deserialize)]
pub struct UpdateBookRequest {
    #[schema(inline)]
    pub visibility: Visibility,
}

#[derive(ToSchema, Serialize, Deserialize)]
pub struct DeleteBookRequest {
    pub key: String,
}

#[derive(Serialize, Deserialize)]
pub struct BookIdAndKey {
    pub id: String,
    pub key: String,
}

/// 本を検索して取得
pub async fn get_books(
    user_id: &str,
    query: BookQuery,
    db: &PgPool,
) -> Result<Vec<GetBooksResponse>, sqlx::Error> {
    // データベースから本の情報を取得
    let books = match sqlx::query_as!(
        Book,
        r#"
            SELECT
                b.id as id,
                b.key as key,
                b.owner_id as owner_id,
                b.name as name,
                b.creator as creator,
                b.publisher as publisher,
                b.date as date,
                b.cover_image as cover_image,
                b.created_at as created_at,
                b.visibility as "visibility: _",
                b.direction as "direction: _",
                b.layout as "layout: _",
                b.images as images
            FROM books b
            LEFT JOIN book_tags bt
                ON b.id = bt.book_id
            LEFT JOIN tags t
                ON bt.tag_name = t.name
            WHERE
                (
                    b.owner_id = $1
                    OR b.visibility = 'public'
                ) AND (
                    b.name ILIKE $2
                    OR b.creator ILIKE $2
                ) AND (
                    $3 = ''
                    OR EXISTS (
                        SELECT 1
                        FROM book_tags bt
                        WHERE bt.book_id = b.id
                        AND bt.tag_name = $3
                    )
                )
            GROUP BY b.id
            ORDER BY created_at DESC
            LIMIT 24 OFFSET $4
        "#,
        user_id,
        query
            .keyword
            .map(|k| format!("%{}%", k))
            .unwrap_or("%%".to_string()),
        query.tag.unwrap_or("".to_string()),
        ((query.page.unwrap_or(1) - 1) * 24) as i32,
    )
    .fetch_all(db)
    .await
    {
        Ok(books) => books,
        Err(e) => {
            panic!("{}", e.to_string());
        }
    };

    let response = books
        .iter()
        .map(|book| GetBooksResponse {
            id: book.id.clone(),
            owner_id: book.owner_id.clone(),
            name: book.name.clone(),
            creator: book.creator.clone(),
            publisher: book.publisher.clone(),
            date: book.date.clone(),
            cover_image: book.cover_image.clone(),
            visibility: book.visibility,
            created_at: book.created_at,
            tags: vec![],
        })
        .collect::<Vec<_>>();

    Ok(response)
}

/// 本の詳細を取得
pub async fn get_book_details(
    book_id: &str,
    user_id: &str,
    db: &PgPool,
) -> Result<GetBookDetailsResponse, sqlx::Error> {
    // データベースから本の情報を取得
    let book = sqlx::query_as!(
        Book,
        r#"
            SELECT
                b.id as id,
                b.key as key,
                b.owner_id as owner_id,
                b.name as name,
                b.creator as creator,
                b.publisher as publisher,
                b.date as date,
                b.cover_image as cover_image,
                b.created_at as created_at,
                b.visibility as "visibility: _",
                b.direction as "direction: _",
                b.layout as "layout: _",
                b.images as images
            FROM books b
            WHERE b.id = $1
        "#,
        book_id
    )
    .fetch_one(db)
    .await?;

    // 権限があるか確認
    if !is_available(&book, user_id, db).await {
        return Err(sqlx::Error::RowNotFound);
    }

    // MinIOから署名付きURLを取得してBookResponseを作成
    let endpoint = env::var("PUBLIC_S3_ENDPOINT").expect("PUBLIC_S3_ENDPOINT is not set");
    let minio_client = minio::get_client(&endpoint).await;
    let epub_bucket = env::var("EPUB_BUCKET").unwrap();
    let presign_epub_task = minio_client
        .get_object()
        .bucket(&epub_bucket)
        .key(book.key.clone())
        .presigned(PresigningConfig::expires_in(Duration::from_secs(60 * 60 * 24 * 7)).unwrap());
    let presign_images_task = book.images.iter().map(|image| {
        minio_client
            .get_object()
            .bucket(&epub_bucket)
            .key(image.clone())
            .presigned(PresigningConfig::expires_in(Duration::from_secs(60 * 60 * 24 * 7)).unwrap())
    });
    let presigned_epub_url = presign_epub_task.await.unwrap().uri().to_string();
    let presigned_image_urls = join_all(presign_images_task)
        .await
        .iter()
        .map(|r| r.as_ref().unwrap().uri().to_string())
        .collect::<Vec<String>>();

    Ok(GetBookDetailsResponse {
        id: book.id,
        owner_id: book.owner_id,
        name: book.name,
        creator: book.creator,
        publisher: book.publisher,
        date: book.date,
        cover_image: book.cover_image,
        visibility: book.visibility,
        direction: book.direction,
        created_at: book.created_at,
        tags: vec![],
        epub_url: presigned_epub_url,
        layout: book.layout,
        images: presigned_image_urls,
    })
}

/// タグを追加する
pub async fn add_tag(
    book_id: &str,
    tag_name: &str,
    user_id: &str,
    db: &PgPool,
) -> Result<(), sqlx::Error> {
    let book = sqlx::query!(
        r#"
            SELECT owner_id
            FROM books
            WHERE id = $1
        "#,
        book_id
    )
    .fetch_one(db)
    .await?;

    if book.owner_id != user_id && !is_admin(db, user_id).await {
        return Err(sqlx::Error::RowNotFound);
    }

    sqlx::query!(
        r#"
            INSERT INTO book_tags (book_id, tag_name)
            VALUES ($1, $2)
        "#,
        book_id,
        tag_name
    )
    .execute(db)
    .await?;
    Ok(())
}

/// タグを削除する
pub async fn delete_tag_from_book(
    book_id: &str,
    tag_name: &str,
    user_id: &str,
    db: &PgPool,
) -> Result<(), sqlx::Error> {
    let book = sqlx::query!(
        r#"
            SELECT owner_id
            FROM books
            WHERE id = $1
        "#,
        book_id
    )
    .fetch_one(db)
    .await?;

    if book.owner_id != user_id && !is_admin(db, user_id).await {
        return Err(sqlx::Error::RowNotFound);
    }

    sqlx::query!(
        r#"
            DELETE FROM book_tags
            WHERE book_id = $1
            AND tag_name = $2
        "#,
        book_id,
        tag_name
    )
    .execute(db)
    .await?;
    Ok(())
}

/// 本を更新する
pub async fn update_book(
    book_id: &str,
    user_id: &str,
    req: UpdateBookRequest,
    db: &PgPool,
) -> Result<(), sqlx::Error> {
    let book = sqlx::query!(
        r#"
            SELECT owner_id
            FROM books
            WHERE id = $1
        "#,
        book_id
    )
    .fetch_one(db)
    .await?;

    if book.owner_id != user_id && !is_admin(db, user_id).await {
        return Err(sqlx::Error::RowNotFound);
    }

    sqlx::query!(
        r#"
            UPDATE books
            SET visibility = $1
            WHERE id = $2
        "#,
        req.visibility as Visibility,
        book_id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// 本を削除する
pub async fn delete_book(book_id: &str, user_id: &str, db: &PgPool) -> Result<(), sqlx::Error> {
    let book = sqlx::query!(
        r#"
            SELECT owner_id, key
            FROM books
            WHERE id = $1
        "#,
        book_id
    )
    .fetch_one(db)
    .await?;

    if book.owner_id != user_id && !is_admin(db, user_id).await {
        return Err(sqlx::Error::RowNotFound);
    }

    let endpoint = env::var("S3_ENDPOINT").expect("S3_ENDPOINT is not set");
    let minio_client = minio::get_client(&endpoint).await;
    if let Err(e) = minio_client
        .delete_object()
        .bucket(env::var("EPUB_BUCKET").unwrap())
        .key(book.key)
        .send()
        .await
    {
        log::error!("Failed to delete object: {}", e);
    }

    sqlx::query!(
        r#"
            DELETE FROM books
            WHERE id = $1
        "#,
        book_id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Layoutの登録がない本を取得する
///
/// エンドユーザーには公開しないため、認証は不要
pub async fn get_books_without_layout(db: &PgPool) -> Result<Vec<BookIdAndKey>, sqlx::Error> {
    let keys = sqlx::query_as!(
        BookIdAndKey,
        r#"SELECT id, key FROM books WHERE layout isnull"#
    )
    .fetch_all(db)
    .await?;
    Ok(keys)
}

/// 本の画像を更新する
///
/// エンドユーザーには公開しないため、認証は不要
pub async fn update_book_images(
    book_id: &str,
    layout: BookLayout,
    images: Vec<String>,
    db: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
            UPDATE books
            SET layout = $1, images = $2
            WHERE id = $3
        "#,
        layout as BookLayout,
        &images,
        book_id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn is_available(book: &Book, user_id: &str, db: &PgPool) -> bool {
    if is_admin(db, user_id).await {
        return true;
    }

    book.owner_id == user_id || book.visibility == Visibility::Public
}

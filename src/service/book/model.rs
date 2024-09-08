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
}

#[derive(Serialize, Debug, sqlx::Type, ToSchema, Deserialize)]
#[sqlx(type_name = "layout", rename_all = "lowercase")]
pub enum BookLayout {
    Reflowable,
    #[sqlx(rename = "pre-paginated")]
    PrePaginated,
}

#[derive(ToSchema, Serialize, Deserialize)]
pub struct BookResponse {
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
}

#[derive(ToSchema, Serialize, Deserialize, Debug)]
pub struct BookUpdateImagesResponse {
    pub key: String,
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
pub struct BookKey {
    pub key: String,
}

pub async fn get_books(
    user_id: &str,
    query: BookQuery,
    db: &PgPool,
) -> Result<Vec<BookResponse>, sqlx::Error> {
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
                b.direction as "direction: _"
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

    // MinIOから署名付きURLを取得してBookResponseを作成
    let endpoint = env::var("PUBLIC_S3_ENDPOINT").expect("PUBLIC_S3_ENDPOINT is not set");
    let minio_client = minio::get_client(&endpoint).await;
    let presigning_tasks = books
        .iter()
        .map(|book| {
            let minio_client = minio_client.clone();
            async move {
                let epub_bucket = env::var("EPUB_BUCKET").unwrap();
                let presigned = minio_client
                    .get_object()
                    .bucket(&epub_bucket)
                    .key(book.key.clone())
                    .presigned(
                        PresigningConfig::expires_in(Duration::from_secs(60 * 60 * 24 * 7))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                BookResponse {
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
                    epub_url: presigned.uri().to_string(),
                    direction: book.direction,
                }
            }
        })
        .collect::<Vec<_>>();
    let presigned_urls = join_all(presigning_tasks).await;

    Ok(presigned_urls)
}

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

pub async fn is_available(book: &Book, user_id: &str, db: &PgPool) -> bool {
    if is_admin(db, user_id).await {
        return true;
    }

    book.owner_id == user_id || book.visibility == Visibility::Public
}

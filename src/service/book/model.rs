use std::env;

use axum::extract::Multipart;
use base64::{prelude::BASE64_STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sqlx::{types::chrono::NaiveDateTime, PgPool};
use utoipa::{IntoParams, ToSchema};

use crate::service::user::model::is_admin;

#[derive(Serialize, Deserialize, Debug, sqlx::Type, ToSchema, PartialEq)]
#[sqlx(type_name = "visibility", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

#[derive(sqlx::FromRow, Serialize)]
pub struct Book {
    pub key: String,
    pub owner_id: String,
    pub name: String,
    pub creator: String,
    pub publisher: String,
    pub date: String,
    pub cover_image: Vec<u8>,
    pub visibility: Visibility,
    pub created_at: NaiveDateTime,
}

#[derive(ToSchema, Serialize, Deserialize)]
pub struct BookResponse {
    pub key: String,
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

pub async fn get_book(book_id: &str, db: &PgPool) -> Result<Book, sqlx::Error> {
    sqlx::query_as!(
        Book,
        r#"
            SELECT
                key,
                owner_id,
                name,
                creator,
                publisher,
                date,
                cover_image,
                visibility as "visibility: _",
                created_at
            FROM books
            WHERE id = $1
        "#,
        book_id
    )
    .fetch_one(db)
    .await
}

pub async fn get_books(
    user_id: &str,
    query: BookQuery,
    db: &PgPool,
) -> Result<Vec<BookResponse>, sqlx::Error> {
    sqlx::query_as!(
        Book,
        r#"
            SELECT
                key,
                owner_id,
                name,
                creator,
                publisher,
                date,
                cover_image,
                visibility as "visibility: _",
                created_at
            FROM books
            WHERE
                (
                    owner_id = $1
                    OR visibility = 'public'
                ) AND (
                    name ILIKE $2
                    OR creator ILIKE $2
                ) AND (
                    $3 = ''
                    OR EXISTS (
                        SELECT 1
                        FROM book_tags
                        WHERE book_tags.book_id = books.id
                        AND book_tags.tag_name = $3
                    )
                )
            ORDER BY created_at DESC
            LIMIT 12 OFFSET $4
        "#,
        user_id,
        query
            .keyword
            .map(|k| format!("%{}%", k))
            .unwrap_or("%%".to_string()),
        query.tag.unwrap_or("".to_string()),
        ((query.page.unwrap_or(1) - 1) * 12) as i32,
    )
    .fetch_all(db)
    .await
    .map(|books| {
        books
            .into_iter()
            .map(|book| BookResponse {
                key: book.key,
                owner_id: book.owner_id,
                name: book.name,
                creator: book.creator,
                publisher: book.publisher,
                date: book.date,
                cover_image: BASE64_STANDARD.encode(&book.cover_image),
                visibility: book.visibility,
                created_at: book.created_at,
            })
            .collect()
    })
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

    let minio_client = crate::minio::minio::get_client().await;
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

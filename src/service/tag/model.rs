use crate::service::user::model::is_admin;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct NewTagRequest {
    pub name: String,
}

#[derive(Serialize, Deserialize, ToSchema, sqlx::FromRow, Debug)]
pub struct Tag {
    pub name: String,
    pub book_count: i64,
}

/// タグを作成する
pub async fn create_tag(name: &str, db: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
            INSERT INTO tags (name)
            VALUES ($1)
        "#,
        name
    )
    .execute(db)
    .await?;
    Ok(())
}

/// タグを更新する
pub async fn update_tag(old: &str, new: &str, db: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
            UPDATE tags
            SET name = $1
            WHERE name = $2
        "#,
        new,
        old
    )
    .execute(db)
    .await?;
    Ok(())
}

/// タグを削除する
pub async fn delete_tag(name: &str, db: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
            DELETE FROM tags
            WHERE name = $1
        "#,
        name
    )
    .execute(db)
    .await?;
    Ok(())
}

/// タグとそのbook件数の一覧を取得する
pub async fn get_tags(db: &PgPool, user_id: &str) -> Result<Vec<Tag>, sqlx::Error> {
    if is_admin(db, user_id).await {
        sqlx::query!(
            r#"
                SELECT name,
                (
                    SELECT COUNT(tag_name = tags.name)
                    FROM book_tags
                ) AS "book_count"
                FROM tags
            "#
        )
        .fetch_all(db)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|row| Tag {
                    name: row.name,
                    book_count: row.book_count.unwrap_or(0),
                })
                .collect()
        })
    } else {
        sqlx::query!(
            r#"
                SELECT t.name AS name,
                COUNT(DISTINCT bt.book_key) AS book_count
                FROM tags t
                LEFT JOIN book_tags bt ON bt.tag_name = t.name
                LEFT JOIN books b ON b.key = bt.book_key
                WHERE b.owner_id = $1 OR b.visibility = 'public'
                GROUP BY t.name;
            "#,
            user_id
        )
        .fetch_all(db)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|row| Tag {
                    name: row.name,
                    book_count: row.book_count.unwrap_or(0),
                })
                .collect()
        })
    }
}

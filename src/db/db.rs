use std::env;

use sqlx::{postgres::PgPoolOptions, query, Pool, Postgres};

use crate::service::user::model::UserRole;

/// 管理者ユーザーをデータベースに追加する
pub async fn insert_admin_user(db: &Pool<Postgres>) {
    let admin_id = env::var("ADMIN_ID").expect("ADMIN_ID is not set");
    let admin_password = env::var("ADMIN_PASSWORD").expect("ADMIN_PASSWORD is not set");
    if let Ok(admin_api_key) = sqlx::query!(r#"SELECT api_key FROM users WHERE id = $1"#, admin_id)
        .fetch_one(db)
        .await
    {
        println!("Admin user already exists: {}", admin_api_key.api_key);
        return;
    }
    match query!(
        r#"INSERT INTO users (id, password, role) VALUES ($1, $2, $3) returning api_key"#,
        admin_id,
        admin_password,
        UserRole::Admin as UserRole
    )
    .fetch_one(db)
    .await {
        Ok(row) => println!("Admin api key: {}", row.api_key),
        Err(e) => println!("Failed to create admin user: {}", e),
    }
}

/// データベースに接続する
pub async fn connect_db() -> Pool<Postgres> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
    let db: Pool<Postgres> = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");
    db
}

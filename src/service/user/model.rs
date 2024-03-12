use std::{env, result::Result};

use chrono::Duration;
use cookie::Cookie;
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Validation};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::service::invitation::model::InvitationState;

#[derive(Serialize, Debug, sqlx::Type)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    User,
}

#[derive(sqlx::FromRow, Debug, Serialize, ToSchema)]
pub struct User {
    pub id: String,
    pub password: String,
    pub role: UserRole,
    pub api_key: String,
    pub invitations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    id: String,
    exp: i64,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub enum UserError {
    #[serde(rename = "unauthorized")]
    Unauthorized(String),
    #[serde(rename = "invalid id or password")]
    InvalidIdOrPassword(String),
    #[serde(rename = "invalid invitation code")]
    InvalidInvitationCode(String),
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ShowUserRequest {
    pub id: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct NewUserRequest {
    pub id: String,
    pub password: String,
    pub invitation_code: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub id: String,
    pub password: String,
}

/// idとpasswordを受け取って、認証に成功したらJWTを返す
pub async fn varify_password(db: PgPool, id: &str, password: &str) -> Result<String, String> {
    // ログイン処理
    let user = match sqlx::query!(
        r#"SELECT id, password, role as "role: UserRole" FROM users WHERE id = $1"#,
        id,
    )
    .fetch_one(&db)
    .await
    {
        Ok(user) => user,
        Err(_) => {
            return Err(String::from("invalid id"));
        }
    };
    if user.password != password {
        return Err(String::from("invalid password"));
    }

    Ok(id_to_jwt(id))
}

/// IDを受け取って、JWTを返す
pub fn id_to_jwt(id: &str) -> String {
    // JWTの生成
    let key = EncodingKey::from_secret(
        env::var("JWT_SECRET")
            .expect("JWT_SECRET is not set")
            .as_bytes(),
    );
    let mut header = jsonwebtoken::Header::default();
    header.typ = Some(String::from("JWT"));
    header.alg = jsonwebtoken::Algorithm::HS256;
    let claim = Claims {
        id: id.to_string(),
        exp: (chrono::Utc::now() + Duration::days(30)).timestamp(),
    };
    encode(&header, &claim, &key).unwrap()
}

/// APIキーを受け取って、ユーザーIDを返す
pub async fn get_user_id_by_api_key(api_key: &str, db: &PgPool) -> Result<String, sqlx::Error> {
    sqlx::query!(
        r#"
            SELECT id
            FROM users
            WHERE api_key = $1
        "#,
        api_key
    )
    .fetch_one(db)
    .await
    .map(|row| row.id)
}

/// JWTを受け取って、認証に成功したらユーザーIDを返す
pub fn varify_token(token: &str) -> Option<String> {
    let validation = Validation::default();
    let key = DecodingKey::from_secret(
        env::var("JWT_SECRET")
            .expect("JWT_SECRET is not set")
            .as_bytes(),
    );
    match jsonwebtoken::decode::<Claims>(token, &key, &validation) {
        Ok(claim) => Some(claim.claims.id),
        Err(_) => None,
    }
}

/// HeaderMapからJWTを取り出して、認証に成功したらユーザーIDを返す
pub fn user_id_from_header(headers: &axum::http::HeaderMap) -> Option<String> {
    match headers.get("Cookie") {
        Some(cookie) => {
            let cookie = cookie.to_str().unwrap();
            Cookie::split_parse(cookie)
                .find(|c| c.clone().unwrap().name() == "token")
                .map(|c| c.unwrap().value().to_string())
                .and_then(|token| varify_token(&token))
        }
        None => None,
    }
}

/// user_idを受け取ってCookieを返す
/// テスト用
pub fn token_cookie_from_user_id(user_id: &str) -> String {
    let token = id_to_jwt(user_id);
    Cookie::build(("token", &token))
        .secure(true)
        .http_only(true)
        .to_string()
}

/// user_idを受け取って、そのユーザーが管理者かどうかを返す
pub async fn is_admin(db: &PgPool, user_id: &str) -> bool {
    match sqlx::query!(
        r#"
                SELECT role as "role: UserRole"
                FROM users
                WHERE id = $1
            "#,
        user_id
    )
    .fetch_one(db)
    .await
    {
        Ok(user) => match user.role {
            UserRole::Admin => true,
            _ => false,
        },
        Err(_) => false,
    }
}

/// 招待コードを確認して、ユーザーを作成する
pub async fn create_user(
    id: &str,
    password: &str,
    invitation_code: &str,
    db: &PgPool,
) -> Result<(), String> {
    // 招待コードの確認
    let invitation = match sqlx::query!(
        r#"
            SELECT
                state as "state: InvitationState",
                used_at
            FROM invitations
            WHERE
                code = $1
                AND
                state != 'used'
        "#,
        invitation_code
    )
    .fetch_one(db)
    .await
    {
        Ok(invitation) => invitation,
        Err(_) => {
            return Err("招待コードが見つかりませんでした".to_string());
        }
    };
    if invitation.state == InvitationState::Used {
        return Err("使用済みの招待コードです".to_string());
    }

    // ID(英数字と-と_で2文字以上), パスワード(英数字と記号で8文字以上)の確認
    let id_regex = Regex::new(r#"^[a-zA-Z0-9_-]{2,40}$"#).unwrap();
    if !id_regex.is_match(&id) {
        return Err("IDは2文字以上で英数字,-,_のみを使用してください".to_string());
    }
    if password.chars().count() < 8 {
        return Err("パスワードは8文字以上で入力してください".to_string());
    }

    // トランザクションの開始
    let mut transaction = match db.begin().await {
        Ok(transaction) => transaction,
        Err(e) => return Err(e.to_string()),
    };

    // ユーザーの作成
    if let Err(e) = sqlx::query!(
        r#"
            INSERT INTO users (id, password, role, api_key)
            VALUES ($1, $2, $3, $4)
        "#,
        id,
        password,
        UserRole::User as UserRole,
        Uuid::new_v4().to_string(),
    )
    .execute(&mut **(&mut transaction))
    .await
    {
        return Err(e.to_string());
    }

    // 招待コードの使用済み化
    if let Err(e) = sqlx::query!(
        r#"
            UPDATE invitations
            SET state = $1
            WHERE code = $2
        "#,
        InvitationState::Used as InvitationState,
        invitation_code
    )
    .execute(&mut **(&mut transaction))
    .await
    {
        return Err(e.to_string());
    }

    // トランザクションのコミット
    if let Err(e) = transaction.commit().await {
        return Err(e.to_string());
    }

    Ok(())
}

/// ユーザー情報の取得
pub async fn show_user(id: &str, db: &PgPool) -> Result<User, sqlx::Error> {
    let user = sqlx::query!(
        r#"
            SELECT id, password, role as "role: UserRole", api_key
            FROM users
            WHERE id = $1
        "#,
        id
    )
    .fetch_one(db)
    .await?;
    let invitations = sqlx::query!(
        r#"
            SELECT code
            FROM invitations
            WHERE issuer_id = $1
        "#,
        id
    )
    .fetch_all(db)
    .await?;
    Ok(User {
        id: user.id,
        password: user.password,
        role: user.role,
        api_key: user.api_key,
        invitations: invitations.iter().map(|i| i.code.clone()).collect(),
    })
}

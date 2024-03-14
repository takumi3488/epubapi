use axum::{
    body::Body,
    extract::State,
    http::{header::SET_COOKIE, HeaderMap, StatusCode},
    response::{AppendHeaders, IntoResponse},
    Json,
};
use sqlx::PgPool;

use super::model::{self, user_id_from_header, UserError};

#[utoipa::path(
    post,
    path = "/users",
    request_body = inline(model::NewUserRequest),
    responses(
        (status = 204, description = "OK"),
        (status = 400, description = "Bad Request", body = inline(UserError), example = json!(model::UserError::InvalidIdOrPassword(String::from("IDかパスワードが不正です")))),
    )
)]
pub async fn new_user(
    State(db): State<PgPool>,
    Json(body): Json<model::NewUserRequest>,
) -> impl IntoResponse {
    // ユーザー登録処理
    if let Err(e) = model::create_user(&body.id, &body.password, &body.invitation_code, &db).await {
        return (
            StatusCode::BAD_REQUEST,
            Json(model::UserError::InvalidInvitationCode(e.to_string())),
        )
            .into_response();
    }

    // JWTの生成
    let jwt = model::id_to_jwt(&body.id);

    (
        StatusCode::NO_CONTENT,
        AppendHeaders([(
            SET_COOKIE,
            format!(
                "token={};Max-Age={};Path=/;Secure;HttpOnly;SameSite=Lax;",
                jwt,
                3600 * 24 * 30
            ),
        )]),
        Body::empty(),
    )
        .into_response()
}

// ログイン
#[utoipa::path(
    post,
    path = "/login",
    responses(
        (status = 204),
        (status = 400, description = "Bad Request", body = inline(UserError), example = json!(model::UserError::InvalidIdOrPassword(String::from("invalid id or password")))),
    )
)]
pub async fn login(
    State(db): State<PgPool>,
    Json(body): Json<model::LoginRequest>,
) -> impl IntoResponse {
    let jwt = match model::varify_password(db, &body.id, &body.password).await {
        Ok(jwt) => jwt,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(model::UserError::InvalidIdOrPassword(err)),
            )
                .into_response();
        }
    };

    (
        StatusCode::NO_CONTENT,
        AppendHeaders([(
            SET_COOKIE,
            format!(
                "token={};Max-Age={};Path=/;Secure;HttpOnly;SameSite=Lax;",
                jwt,
                3600 * 24 * 30
            ),
        )]),
        Body::empty(),
    )
        .into_response()
}

/// ユーザー情報を取得
#[utoipa::path(
    get,
    path = "/users",
    responses(
        (status = 200, description = "OK", body = inline(model::User)),
        (status = 400, description = "Bad Request", body = inline(UserError), example = json!(model::UserError::Unauthorized(String::from("認証に失敗しました")))),
    )
)]
pub async fn show_user(headers: HeaderMap, State(db): State<PgPool>) -> impl IntoResponse {
    let user_id = match user_id_from_header(&headers) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(UserError::Unauthorized(String::from("missing user id"))),
            )
                .into_response()
        }
    };

    let user = match model::show_user(&user_id, &db).await {
        Ok(user) => user,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(UserError::Unauthorized(String::from("認証に失敗しました"))),
            )
                .into_response()
        }
    };

    (StatusCode::OK, Json(user)).into_response()
}

#[cfg(test)]
mod tests {

    use axum::{
        body::Body,
        http::{header, Method, Request},
    };
    use serde_json::{json, to_string};
    use sqlx::{self, PgPool};
    use tower::ServiceExt;

    use crate::{routes::routes::init_app, service::user::model::token_cookie_from_user_id};

    #[sqlx::test(fixtures("users", "invitations"))]
    async fn test_new_user(pool: PgPool) {
        let router = init_app(&pool);

        // ユーザー登録 無効なidもしくはpasswordの場合
        let req = Request::builder()
            .uri("/users")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "id": "test",
                        "password": "test",
                        "invitation_code": "unused_test_code"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 400);

        // IDが使用済みの場合
        let req = Request::builder()
            .uri("/users")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "id": "used_id",
                        "password": "Test1234",
                        "invitation_code": "unused_test_code"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 400);

        // ユーザー登録 成功した場合
        let req = Request::builder()
            .uri("/users")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "id": "unused_id",
                        "password": "Test1234",
                        "invitation_code": "unused_test_code"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);
        let headers = res.headers();
        let cookie = headers.get(header::SET_COOKIE).unwrap();
        assert!(cookie.to_str().unwrap().contains("token="));

        // ユーザー登録 招待コードが無効な場合
        let req = Request::builder()
            .uri("/users")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "id": "test2",
                        "password": "Test1234",
                        "invitation_code": "used_test_code"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 400);
    }

    #[sqlx::test(fixtures("users", "invitations"))]
    async fn test_login(pool: PgPool) {
        let router = init_app(&pool);

        // ログイン 無効なidもしくはpasswordの場合
        let req = Request::builder()
            .uri("/login")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "id": "test",
                        "password": "test"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 400);

        // ログイン 成功した場合
        let req = Request::builder()
            .uri("/login")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "id": "used_id",
                        "password": "Test1234"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);
        let headers = res.headers();
        let cookie = headers.get(header::SET_COOKIE).unwrap();
        assert!(cookie.to_str().unwrap().contains("token="));
    }

    #[sqlx::test(fixtures("users", "invitations"))]
    async fn test_show_user(pool: PgPool) {
        let router = init_app(&pool);

        // ユーザー情報取得 認証に失敗した場合
        let user_token = token_cookie_from_user_id("invalid_user_id");
        let req = Request::builder()
            .uri("/users")
            .method(Method::GET)
            .header(header::COOKIE, &user_token)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 400);

        // ユーザー情報取得 成功した場合
        let user_token = token_cookie_from_user_id("used_id");
        let req = Request::builder()
            .uri("/users")
            .method(Method::GET)
            .header(header::COOKIE, &user_token)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 200);
    }
}

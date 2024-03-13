use super::model;
use crate::service::user::model::{is_admin, user_id_from_header};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use sqlx::PgPool;

#[utoipa::path(
    post,
    path = "/tags",
    request_body = inline(model::NewTagRequest),
    responses(
        (status = 204, description = "OK"),
        (status = 400, description = "Bad Request"),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn new_tag(
    headers: HeaderMap,
    State(db): State<PgPool>,
    Json(body): Json<model::NewTagRequest>,
) -> impl IntoResponse {
    if user_id_from_header(&headers).is_none() {
        return (StatusCode::UNAUTHORIZED).into_response();
    }
    match model::create_tag(&body.name, &db).await {
        Ok(_) => (StatusCode::NO_CONTENT).into_response(),
        Err(_) => (StatusCode::BAD_REQUEST).into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/tags/{old}",
    request_body = inline(model::NewTagRequest),
    responses(
        (status = 204, description = "OK"),
        (status = 400, description = "Bad Request"),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn update_tag(
    headers: HeaderMap,
    Path(old): Path<String>,
    State(db): State<PgPool>,
    Json(body): Json<model::NewTagRequest>,
) -> impl IntoResponse {
    match user_id_from_header(&headers) {
        Some(id) => {
            if !is_admin(&db, &id).await {
                return (StatusCode::UNAUTHORIZED).into_response();
            }
        }
        None => return (StatusCode::UNAUTHORIZED).into_response(),
    }
    match model::update_tag(&old, &body.name, &db).await {
        Ok(_) => (StatusCode::NO_CONTENT).into_response(),
        Err(_) => (StatusCode::BAD_REQUEST).into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/tags/{name}",
    responses(
        (status = 204, description = "OK"),
        (status = 400, description = "Bad Request"),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn delete_tag(
    headers: HeaderMap,
    State(db): State<PgPool>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match user_id_from_header(&headers) {
        Some(id) => {
            if !is_admin(&db, &id).await {
                return (StatusCode::UNAUTHORIZED).into_response();
            }
        }
        None => return (StatusCode::UNAUTHORIZED).into_response(),
    }
    match model::delete_tag(&name, &db).await {
        Ok(_) => (StatusCode::NO_CONTENT).into_response(),
        Err(_) => (StatusCode::BAD_REQUEST).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/tags",
    responses(
        (status = 200, body = inline(Vec<model::Tag>), description = "OK"),
        (status = 400, description = "Bad Request"),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn get_tags(headers: HeaderMap, State(db): State<PgPool>) -> impl IntoResponse {
    let user_id = match user_id_from_header(&headers) {
        Some(id) => id,
        None => return (StatusCode::UNAUTHORIZED).into_response(),
    };

    match model::get_tags(&db, &user_id).await {
        Ok(tags) => (StatusCode::OK, Json(tags)).into_response(),
        Err(_) => (StatusCode::BAD_REQUEST).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use std::str::from_utf8;

    use crate::{routes::routes::init_app, service::user::model::token_cookie_from_user_id};

    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Method, Request},
    };
    use serde_json::{json, to_string};
    use sqlx;
    use tower::ServiceExt;

    #[sqlx::test(fixtures("users", "tags"))]
    async fn test_new_tag(pool: PgPool) {
        let router = init_app(&pool);
        let user_cookie = token_cookie_from_user_id("user_id");

        // POST /tags (unauthorized)
        let req: Request<Body> = Request::builder()
            .uri("/tags")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "name": "test"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 401);

        // POST /tags (authorized)
        let req = Request::builder()
            .uri("/tags")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .header(header::COOKIE, user_cookie)
            .body(Body::from(
                to_string(&json!(
                    {
                        "name": "test"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);
    }

    #[sqlx::test(fixtures("users", "tags"))]
    async fn test_update_tag(pool: PgPool) {
        let router = init_app(&pool);

        // POST /tags/test_tag (without token)
        let req: Request<Body> = Request::builder()
            .uri("/tags/test_tag")
            .method(Method::PUT)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "name": "edited_test_tag"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 401);

        // POST /tags/test_tag (with token but not admin)
        let user_cookie = token_cookie_from_user_id("user_id");
        let req = Request::builder()
            .uri("/tags/test_tag")
            .method(Method::PUT)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .header(header::COOKIE, user_cookie)
            .body(Body::from(
                to_string(&json!(
                    {
                        "name": "edited_test_tag"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 401);

        // POST /tags/test (authorized)
        let cookie = token_cookie_from_user_id("admin_id");
        let req = Request::builder()
            .uri("/tags/test_tag")
            .method(Method::PUT)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .header(header::COOKIE, cookie)
            .body(Body::from(
                to_string(&json!(
                    {
                        "name": "edited_test_tag"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);
    }

    #[sqlx::test(fixtures("users", "tags"))]
    async fn test_delete_tag(pool: PgPool) {
        let router = init_app(&pool);

        // DELETE /tags/test_tag (without token)
        let req: Request<Body> = Request::builder()
            .uri("/tags/test_tag")
            .method(Method::DELETE)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 401);

        // DELETE /tags/test_tag (with token but not admin)
        let cookie = token_cookie_from_user_id("user_id");
        let req = Request::builder()
            .uri("/tags/test_tag")
            .method(Method::DELETE)
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 401);

        // DELETE /tags/test_tag (authorized)
        let cookie = token_cookie_from_user_id("admin_id");
        let req = Request::builder()
            .uri("/tags/test_tag")
            .method(Method::DELETE)
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);
    }

    #[sqlx::test(fixtures("users", "tags", "book_with_tags"))]
    async fn test_get_tags(pool: PgPool) {
        let router = init_app(&pool);

        // GET /tags (without token)
        let req: Request<Body> = Request::builder()
            .uri("/tags")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 401);

        // GET /tags (with random token)
        let some_user_token = token_cookie_from_user_id("some_user");
        let req = Request::builder()
            .uri("/tags")
            .method(Method::GET)
            .header(header::COOKIE, some_user_token)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 200);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = from_utf8(&*bytes).unwrap();
        assert_eq!(text, r#"[{"name":"test_tag","book_count":2}]"#);

        // GET /tags (with user token)
        let user_token = token_cookie_from_user_id("user_id");
        let req = Request::builder()
            .uri("/tags")
            .method(Method::GET)
            .header(header::COOKIE, user_token)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 200);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = from_utf8(&*bytes).unwrap();
        assert_eq!(text, r#"[{"name":"test_tag","book_count":3}]"#);

        // GET /tags (with admin token)
        let admin_token = token_cookie_from_user_id("admin_id");
        let req = Request::builder()
            .uri("/tags")
            .method(Method::GET)
            .header(header::COOKIE, admin_token)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 200);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = from_utf8(&*bytes).unwrap();
        assert_eq!(text, r#"[{"name":"test_tag","book_count":4}]"#);
    }
}

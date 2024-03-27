use std::env;

use super::model;
use aws_sdk_s3::{
    operation::create_multipart_upload::CreateMultipartUploadOutput,
    primitives::ByteStream,
    types::{CompletedMultipartUpload, CompletedPart},
};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    minio::minio,
    service::user::model::{get_user_id_by_api_key, user_id_from_header, UserError},
};

/// 閲覧可能なbook一覧を取得する
/// page: ページ番号
/// keyword: タイトル・著者名での検索キーワード
#[utoipa::path(
    get,
    path = "/books",
    params(model::BookQuery),
    responses(
        (status = 200, description = "OK", body = inline(Vec<model::BookResponse>)),
        (status = 401, description = "Unauthorized", body = inline(UserError), example = json!(UserError::Unauthorized(String::from("missing user id")))),
    )
)]
pub async fn get_books(
    headers: HeaderMap,
    query: Query<model::BookQuery>,
    State(db): State<PgPool>,
) -> impl IntoResponse {
    let user_id = match user_id_from_header(&headers, &db).await {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(UserError::Unauthorized(String::from("missing user id"))),
            )
                .into_response()
        }
    };

    let books = match model::get_books(&user_id, query.0, &db).await {
        Ok(books) => books,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    };

    (StatusCode::OK, Json(books)).into_response()
}

/// bookを新規作成する
///
/// cookieではなく、ヘッダーにX-Api-Keyを設定する必要がある
pub async fn new_book(
    headers: HeaderMap,
    State(db): State<PgPool>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // APIキーの確認
    let api_key = headers.get("X-Api-Key");
    if api_key.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(UserError::Unauthorized(String::from("missing api key"))),
        )
            .into_response();
    }
    let api_key = api_key.unwrap().to_str().unwrap();
    let user_id = match get_user_id_by_api_key(api_key, &db).await {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(UserError::Unauthorized(String::from("incorrect api key"))),
            )
                .into_response()
        }
    };

    let endpoint = env::var("S3_ENDPOINT").expect("S3_ENDPOINT is not set");
    let client = minio::get_client(&endpoint).await;
    let epub_bucket = env::var("EPUB_BUCKET").expect("EPUB_BUCKET is not set");
    let id = Uuid::new_v4();
    let key = format!("{}/{}.epub", user_id, id.to_string());
    let multipart_upload_res: CreateMultipartUploadOutput = client
        .create_multipart_upload()
        .bucket(&epub_bucket)
        .key(&key)
        .send()
        .await
        .unwrap();
    let upload_id = multipart_upload_res.upload_id.unwrap();

    let mut upload_parts = vec![];
    let mut part_number = 1;
    while let Some(field) = multipart.next_field().await.unwrap() {
        let data = field.bytes().await.unwrap();
        let stream = ByteStream::from(data);
        let upload_part_res = client
            .upload_part()
            .key(&key)
            .bucket(&epub_bucket)
            .upload_id(&upload_id)
            .body(stream)
            .part_number(part_number)
            .send()
            .await
            .expect("failed to upload part");
        upload_parts.push(
            CompletedPart::builder()
                .e_tag(upload_part_res.e_tag.unwrap_or_default())
                .part_number(part_number)
                .build(),
        );
        part_number += 1;
    }
    let completed_multipart_upload = CompletedMultipartUpload::builder()
        .set_parts(Some(upload_parts))
        .build();
    let _completed_multipart_upload_res = client
        .complete_multipart_upload()
        .bucket(&epub_bucket)
        .key(&key)
        .upload_id(&upload_id)
        .multipart_upload(completed_multipart_upload)
        .send()
        .await
        .expect("failed to complete multipart upload");

    (StatusCode::NO_CONTENT).into_response()
}

/// bookにtagを追加する
#[utoipa::path(
    post,
    path = "/books/{book_id}/tags",
    request_body = inline(model::AddTagRequest),
    responses(
        (status = 204, description = "OK"),
        (status = 401, description = "Unauthorized", body = inline(UserError), example = json!(UserError::Unauthorized(String::from("missing user id")))),
    )
)]
pub async fn add_tag_to_book(
    Path(book_id): Path<String>,
    headers: HeaderMap,
    State(db): State<PgPool>,
    Json(req): Json<model::AddTagRequest>,
) -> impl IntoResponse {
    let user_id = match user_id_from_header(&headers, &db).await {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(UserError::Unauthorized(String::from("missing user id"))),
            )
                .into_response()
        }
    };

    match model::add_tag(&book_id, &req.tag_name, &user_id, &db).await {
        Ok(_) => (StatusCode::NO_CONTENT).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    }
}

/// bookからtagを削除する
#[utoipa::path(
    delete,
    path = "/books/{book_id}/tags/{tag_name}",
    responses(
        (status = 204, description = "OK"),
        (status = 401, description = "Unauthorized", body = inline(UserError), example = json!(UserError::Unauthorized(String::from("missing user id")))),
    )
)]
pub async fn delete_tag_from_book(
    Path((book_id, tag_name)): Path<(String, String)>,
    headers: HeaderMap,
    State(db): State<PgPool>,
) -> impl IntoResponse {
    let user_id = match user_id_from_header(&headers, &db).await {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(UserError::Unauthorized(String::from("missing user id"))),
            )
                .into_response()
        }
    };

    match model::delete_tag_from_book(&book_id, &tag_name, &user_id, &db).await {
        Ok(_) => (StatusCode::NO_CONTENT).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    }
}

/// bookを更新する
#[utoipa::path(
    patch,
    path = "/books/{book_id}",
    request_body = inline(model::UpdateBookRequest),
    responses(
        (status = 204, description = "OK"),
        (status = 401, description = "Unauthorized", body = inline(UserError), example = json!(UserError::Unauthorized(String::from("missing user id")))),
    )
)]
pub async fn update_book(
    Path(book_id): Path<String>,
    headers: HeaderMap,
    State(db): State<PgPool>,
    Json(req): Json<model::UpdateBookRequest>,
) -> impl IntoResponse {
    let user_id = match user_id_from_header(&headers, &db).await {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(UserError::Unauthorized(String::from("missing user id"))),
            )
                .into_response()
        }
    };

    match model::update_book(&book_id, &user_id, req, &db).await {
        Ok(_) => (StatusCode::NO_CONTENT).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    }
}

/// bookを削除する
#[utoipa::path(
    delete,
    path = "/books/{book_id}",
    responses(
        (status = 204, description = "OK"),
        (status = 401, description = "Unauthorized", body = inline(UserError), example = json!(UserError::Unauthorized(String::from("missing user id")))),
    )
)]
pub async fn delete_book(
    Path(book_id): Path<String>,
    headers: HeaderMap,
    State(db): State<PgPool>,
) -> impl IntoResponse {
    let user_id = match user_id_from_header(&headers, &db).await {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(UserError::Unauthorized(String::from("missing user id"))),
            )
                .into_response()
        }
    };

    match model::delete_book(&book_id, &user_id, &db).await {
        Ok(_) => (StatusCode::NO_CONTENT).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    }
}

/// カバー画像を取得する
pub async fn get_cover_image(
    Path(book_id): Path<String>,
    headers: HeaderMap,
    State(db): State<PgPool>,
) -> impl IntoResponse {
    if None == user_id_from_header(&headers, &db).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(UserError::Unauthorized(String::from("missing user id"))),
        )
            .into_response();
    }

    let book_id = book_id.clone().replace(".jpg", "");
    let endpoint = env::var("S3_ENDPOINT").expect("S3_ENDPOINT is not set");
    let minio_client = minio::get_client(&endpoint).await;
    let epub_bucket = env::var("EPUB_BUCKET").expect("EPUB_BUCKET is not set");
    let object = minio_client
        .get_object()
        .bucket(&epub_bucket)
        .key(format!("{}.jpg", book_id))
        .send()
        .await
        .expect("failed to get object");
    let cover_image = object
        .body
        .collect()
        .await
        .expect("failed to collect body")
        .into_bytes();

    (StatusCode::OK, cover_image).into_response()
}

#[cfg(test)]
mod tests {
    use std::str::from_utf8;

    use axum::{
        body::{to_bytes, Body},
        http::{header, Request},
    };
    use axum_test::{
        multipart::{MultipartForm, Part},
        TestServer,
    };
    use sqlx::PgPool;
    use tower::ServiceExt;

    use crate::{routes::routes::init_app, service::user::model::token_cookie_from_user_id};

    /// Book一覧取得のテスト
    #[sqlx::test(fixtures("users", "tags", "book_with_tags"))]
    async fn test_get_books(pool: PgPool) {
        let router = init_app(&pool);
        let user_cookie = token_cookie_from_user_id("user_id");

        // GET /books (no query)
        let req = Request::builder()
            .uri("/books")
            .header(header::COOKIE, &user_cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 200);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = from_utf8(&*&bytes).unwrap();
        assert!(text.contains(r#""id":"user_public_book_id""#));
        assert!(text.contains(r#""id":"user_private_book_id""#));
        assert!(text.contains(r#""id":"admin_public_book_id""#));
        assert!(!text.contains(r#""id":"admin_private_book_id""#));

        // GET /books (with query)
        let req = Request::builder()
            .uri(r#"/books?page=1&keyword=user&tag=test_tag"#)
            .header(header::COOKIE, &user_cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 200);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = from_utf8(&*&bytes).unwrap();
        assert!(text.contains(r#""id":"user_public_book_id""#));
        assert!(text.contains(r#""id":"user_private_book_id""#));
        assert!(!text.contains(r#""id":"admin_public_book_id""#));
        assert!(!text.contains(r#""id":"admin_private_book_id""#));

        let req = Request::builder()
            .uri(r#"/books?page=2&keyword=user&tag=test_tag"#)
            .header(header::COOKIE, &user_cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 200);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = from_utf8(&*&bytes).unwrap();
        assert_eq!(text, r#"[]"#);

        // GET /books (with api key)
        let req = Request::builder()
            .uri("/books")
            .header("X-Api-Key", "user_api_key")
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 200);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = from_utf8(&*&bytes).unwrap();
        assert!(text.contains(r#""id":"user_public_book_id""#));
        assert!(text.contains(r#""id":"user_private_book_id""#));
        assert!(text.contains(r#""id":"admin_public_book_id""#));
        assert!(!text.contains(r#""id":"admin_private_book_id""#));
    }

    /// Book新規作成のテスト
    #[sqlx::test(fixtures("users", "tags", "book_with_tags"))]
    async fn test_new_book(pool: PgPool) {
        let router = init_app(&pool);
        let server = TestServer::new(router).unwrap();

        // POST /books
        let part = Part::bytes(b"test epub file".as_slice()).file_name("test.epub");
        let multipart_form = MultipartForm::new().add_part("file", part);
        let res = server
            .post("/books")
            .multipart(multipart_form)
            .add_header(
                "X-Api-Key".parse().unwrap(),
                "user_api_key".parse().unwrap(),
            )
            .await;
        assert_eq!(res.status_code(), 204);
    }

    /// Bookにtagを追加するテスト
    #[sqlx::test(fixtures("users", "tags", "book_with_tags"))]
    async fn test_add_tag_to_book(pool: PgPool) {
        let router = init_app(&pool);
        let user_cookie = token_cookie_from_user_id("user_id");

        // POST /books/{book_id}/tags
        let req = Request::builder()
            .uri("/books/user_public_book_id/tags")
            .method("POST")
            .header(header::COOKIE, &user_cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"tag_name":"additional_tag"}"#))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);

        // POST /books/{book_id}/tags with invalid tag name
        let req = Request::builder()
            .uri("/books/user_public_book_id/tags")
            .method("POST")
            .header(header::COOKIE, &user_cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"tag_name":"invalid_tag"}"#))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 500);

        // POST /books/{book_id}/tags to other user's book
        let req = Request::builder()
            .uri("/books/admin_public_book_id/tags")
            .method("POST")
            .header(header::COOKIE, &user_cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"tag_name":"additional_tag"}"#))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 500);

        // POST /books/{book_id}/tags with admin user
        let admin_cookie = token_cookie_from_user_id("admin_id");
        let req = Request::builder()
            .uri("/books/user_private_book_id/tags")
            .method("POST")
            .header(header::COOKIE, &admin_cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"tag_name":"additional_tag"}"#))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);
    }

    /// Bookからtagを削除するテスト
    #[sqlx::test(fixtures("users", "tags", "book_with_tags"))]
    async fn test_delete_tag_from_book(pool: PgPool) {
        let router = init_app(&pool);
        let user_cookie = token_cookie_from_user_id("user_id");

        // DELETE /books/{book_id}/tags
        let req = Request::builder()
            .uri("/books/user_public_book_id/tags/test_tag")
            .method("DELETE")
            .header(header::COOKIE, &user_cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);

        // DELETE /books/{book_id}/tags with invalid tag name
        let req = Request::builder()
            .uri("/books/user_public_book_id/tags/invalid_tag")
            .method("DELETE")
            .header(header::COOKIE, &user_cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);

        // DELETE /books/{book_id}/tags to other user's book
        let req = Request::builder()
            .uri("/books/admin_public_book_id/tags/test_tag")
            .method("DELETE")
            .header(header::COOKIE, &user_cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 500);

        // DELETE /books/{book_id}/tags with admin user
        let admin_cookie = token_cookie_from_user_id("admin_id");
        let req = Request::builder()
            .uri("/books/user_private_book_id/tags/test_tag")
            .method("DELETE")
            .header(header::COOKIE, &admin_cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);
    }

    /// Bookの更新のテスト
    #[sqlx::test(fixtures("users", "tags", "book_with_tags"))]
    async fn test_update_book(pool: PgPool) {
        let router = init_app(&pool);
        let user_cookie = token_cookie_from_user_id("user_id");

        // PATCH /books/{book_id}
        let req = Request::builder()
            .uri("/books/user_public_book_id")
            .method("PATCH")
            .header(header::COOKIE, &user_cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"visibility":"private"}"#))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);

        // PATCH /books/{book_id} to other user's book
        let req = Request::builder()
            .uri("/books/admin_public_book_id")
            .method("PATCH")
            .header(header::COOKIE, &user_cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"visibility":"private"}"#))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 500);

        // PATCH /books/{book_id} with admin user
        let admin_cookie = token_cookie_from_user_id("admin_id");
        let req = Request::builder()
            .uri("/books/user_private_book_id")
            .method("PATCH")
            .header(header::COOKIE, &admin_cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"visibility":"public"}"#))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);
    }

    /// Bookの削除のテスト
    #[sqlx::test(fixtures("users", "tags", "book_with_tags"))]
    async fn test_delete_book(pool: PgPool) {
        let router = init_app(&pool);
        let user_cookie = token_cookie_from_user_id("user_id");

        // DELETE /books/{book_id}
        let req = Request::builder()
            .uri("/books/user_public_book_id")
            .method("DELETE")
            .header(header::COOKIE, &user_cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);

        // DELETE /books/{book_id} to other user's book
        let req = Request::builder()
            .uri("/books/admin_public_book_id")
            .method("DELETE")
            .header(header::COOKIE, &user_cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 500);

        // DELETE /books/{book_id} with admin user
        let admin_cookie = token_cookie_from_user_id("admin_id");
        let req = Request::builder()
            .uri("/books/user_private_book_id")
            .method("DELETE")
            .header(header::COOKIE, &admin_cookie)
            .body(Body::empty())
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 204);
    }
}

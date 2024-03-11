use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, patch, post, put},
    Router,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::service::{
    book::route::{
        add_tag_to_book, delete_book, delete_tag_from_book, get_books, new_book, update_book,
    },
    invitation::route::check_invitation,
    tag::route::{delete_tag, get_tags, new_tag, update_tag},
    user::route::{login, new_user},
};

#[derive(OpenApi)]
#[openapi(
    paths(
        self::health,
        crate::service::book::route::new_book,
        crate::service::invitation::route::check_invitation,
        crate::service::user::route::new_user,
        crate::service::tag::route::get_tags,
        crate::service::tag::route::new_tag,
        crate::service::tag::route::update_tag,
        crate::service::tag::route::delete_tag,
        crate::service::book::route::get_books,
        crate::service::book::route::new_book,
        crate::service::book::route::update_book,
        crate::service::book::route::delete_book,
        crate::service::book::route::add_tag_to_book,
        crate::service::book::route::delete_tag_from_book
    ),
    components(
        schemas(
            crate::service::invitation::model::CheckInvitationRequest,
            crate::service::invitation::model::CheckInvitationResponse,
            crate::service::user::model::UserError,
            crate::service::user::model::NewUserRequest,
            crate::service::user::model::LoginRequest,
            crate::service::tag::model::Tag,
            crate::service::tag::model::NewTagRequest,
            crate::service::book::model::Book,
            crate::service::book::model::BookQuery,
            crate::service::book::model::Epub,
            crate::service::book::model::Visibility,
            crate::service::book::model::UpdateBookRequest,
            crate::service::book::model::AddTagRequest,
            crate::service::book::model::DeleteTagRequest,
            crate::service::book::model::DeleteBookRequest,
            
        )
    ),
    tags(
        (name = "epubapi", description = "EPUB management API")
    )
)]
pub struct ApiDoc;

/// アプリケーションを初期化する
pub fn init_app(db: &sqlx::PgPool) -> Router {
    Router::new()
        .route("/", get(health))
        .route("/users", post(new_user))
        .route("/login", post(login))
        .route("/epubs", post(new_book))
        .route("/check_invitation", post(check_invitation))
        .route("/tags", get(get_tags).post(new_tag))
        .route("/tags/:name", put(update_tag).delete(delete_tag))
        .route(
            "/books",
            get(get_books)
                .post(new_book)
                .layer(DefaultBodyLimit::max(1024 * 1024 * 1024 * 20)),
        )
        .route("/books/:book_key", patch(update_book).delete(delete_book))
        .route("/books/:book_key/tags", post(add_tag_to_book))
        .route(
            "/books/:book_key/tags/:tag_name",
            delete(delete_tag_from_book),
        )
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(db.clone())
}

/// ヘルスチェック
#[utoipa::path(
    get,
    path = "/",
    responses(
        (status = StatusCode::OK, description = "OK")
    )
)]
pub async fn health() -> &'static str {
    "OK"
}

#[cfg(test)]
mod tests {
    use crate::routes::routes::init_app;

    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use sqlx::{self, PgPool};
    use tower::ServiceExt;

    #[sqlx::test]
    async fn test_health(pool: PgPool) {
        let router = init_app(&pool);

        // GET /health
        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn test_swagger_ui(pool: PgPool) {
        let router = init_app(&pool);

        // GET /swagger-ui/
        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/swagger-ui/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

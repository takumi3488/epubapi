use super::model;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use sqlx::PgPool;

#[utoipa::path(
    post,
    path = "/check_invitation",
    request_body = inline(model::CheckInvitationRequest),
    responses(
        (status = 200, description = "OK", body = inline(model::CheckInvitationResponse)),
        (status = 404, description = "Not Found"),
    )
)]
pub async fn check_invitation(
    State(db): State<PgPool>,
    Json(body): Json<model::CheckInvitationRequest>,
) -> impl IntoResponse {
    match model::check_invitation_state(&db, &body.invitation_code).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(_) => (StatusCode::NOT_FOUND).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use std::str::from_utf8;

    use crate::routes::routes::init_app;

    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Method, Request},
    };
    use serde_json::{json, to_string};
    use tower::ServiceExt;

    #[sqlx::test(fixtures("invitations"))]
    async fn test_check_invitation(pool: PgPool) {
        let router = init_app(&pool);

        // POST /check_invitation (not found)
        let req = Request::builder()
            .uri("/check_invitation")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "invitation_code": "not_found"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 404);

        // POST /check_invitation (unused)
        let req = Request::builder()
            .uri("/check_invitation")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "invitation_code": "unused_test_code"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 200);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = from_utf8(&*bytes).unwrap();
        assert_eq!(
            text,
            to_string(&json!(
                {
                    "state": "unused"
                }
            ))
            .unwrap()
        );

        // POST /check_invitation (using)
        let req = Request::builder()
            .uri("/check_invitation")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "invitation_code": "unused_test_code"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 200);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = from_utf8(&*bytes).unwrap();
        assert_eq!(
            text,
            to_string(&json!(
                {
                    "state": "using"
                }
            ))
            .unwrap()
        );

        // POST /check_invitation (used)
        let req = Request::builder()
            .uri("/check_invitation")
            .method(Method::POST)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                to_string(&json!(
                    {
                        "invitation_code": "used_test_code"
                    }
                ))
                .unwrap(),
            ))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), 200);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = from_utf8(&*bytes).unwrap();
        assert_eq!(
            text,
            to_string(&json!(
                {
                    "state": "used"
                }
            ))
            .unwrap()
        );
    }
}

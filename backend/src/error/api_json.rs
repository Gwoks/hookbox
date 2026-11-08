//! `ApiJson<T>`: `axum::Json<T>` whose rejection is the flat envelope.

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use axum::Json;
use serde::de::DeserializeOwned;

use super::{ApiError, ErrorCode};

/// `axum::Json<T>` whose rejection (bad content-type, unparseable or
/// mistyped body, oversize body) is the flat `{error, detail}` envelope
/// instead of axum's text/plain body. The status is axum's own: 422 →
/// `validation_error`, 413 → `payload_too_large`, 400 / 415 → `bad_request`;
/// `detail` is axum's message.
pub struct ApiJson<T>(pub T);

#[axum::async_trait]
impl<T, S> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(ApiJson(value)),
            Err(rejection) => Err(json_rejection(&rejection)),
        }
    }
}

fn json_rejection(rejection: &JsonRejection) -> ApiError {
    let status = rejection.status();
    let code = match status {
        StatusCode::UNPROCESSABLE_ENTITY => ErrorCode::ValidationError,
        StatusCode::PAYLOAD_TOO_LARGE => ErrorCode::PayloadTooLarge,
        _ => ErrorCode::BadRequest,
    };
    ApiError::new(status, code, rejection.body_text())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    async fn api_json(content_type: &str, body: &'static str) -> Result<i64, ApiError> {
        #[derive(serde::Deserialize)]
        struct Probe {
            n: i64,
        }
        let req = Request::builder()
            .method("POST")
            .uri("/")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();
        ApiJson::<Probe>::from_request(req, &())
            .await
            .map(|ApiJson(p)| p.n)
    }

    #[tokio::test]
    async fn api_json_maps_every_rejection_to_the_envelope() {
        assert_eq!(api_json("application/json", r#"{"n":1}"#).await.unwrap(), 1);
        let typed = api_json("application/json", r#"{"n":"x"}"#)
            .await
            .unwrap_err();
        assert_eq!(typed.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(typed.code, "validation_error");
        assert!(typed.detail.contains("invalid type"), "{}", typed.detail);
        let syntax = api_json("application/json", "{not json").await.unwrap_err();
        assert_eq!(syntax.status, StatusCode::BAD_REQUEST);
        assert_eq!(syntax.code, "bad_request");
        let media = api_json("text/plain", r#"{"n":1}"#).await.unwrap_err();
        assert_eq!(media.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(media.code, "bad_request");
    }
}

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

impl AppError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }

    pub fn bad_request(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Message(m) => {
                let lower = m.to_lowercase();
                if lower.contains("not found") {
                    (StatusCode::NOT_FOUND, m.clone())
                } else if lower.contains("forbidden") || lower.contains("not allowed") {
                    (StatusCode::FORBIDDEN, m.clone())
                } else if lower.contains("gone") || lower.contains("already used") {
                    (StatusCode::GONE, m.clone())
                } else if lower.contains("unauthorized") || lower.contains("invalid credentials") {
                    (StatusCode::UNAUTHORIZED, m.clone())
                } else {
                    (StatusCode::BAD_REQUEST, m.clone())
                }
            }
            AppError::Sqlx(e) => {
                tracing::error!("database error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::Anyhow(e) => {
                tracing::error!("error: {e:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

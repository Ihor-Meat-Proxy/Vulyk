use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    /// Resource existed but was deleted/retired (e.g. case key tombstone).
    #[error("gone: {0}")]
    Gone(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("too many requests: {0}")]
    TooManyRequests(String),
    /// Git host (GitHub/GitLab) REST rate limit; `reset_at` is UTC when known.
    #[error("{message}")]
    GitHostRateLimit {
        message: String,
        reset_at: Option<DateTime<Utc>>,
    },
    #[error("internal: {0}")]
    Internal(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reset_at: Option<DateTime<Utc>>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, reset_at) = match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found", None),
            AppError::Gone(_) => (StatusCode::GONE, "case_deleted", None),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request", None),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "conflict", None),
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized", None),
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden", None),
            AppError::TooManyRequests(_) => {
                (StatusCode::TOO_MANY_REQUESTS, "too_many_requests", None)
            }
            AppError::GitHostRateLimit { reset_at, .. } => (
                StatusCode::TOO_MANY_REQUESTS,
                "git_host_rate_limit",
                *reset_at,
            ),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal", None),
            AppError::Db(sqlx::Error::RowNotFound) => (StatusCode::NOT_FOUND, "not_found", None),
            AppError::Db(sqlx::Error::Database(err)) if err.constraint().is_some() => {
                (StatusCode::CONFLICT, "conflict", None)
            }
            AppError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal", None),
        };

        let body = Json(ErrorBody {
            error: ErrorDetail {
                code,
                message: self.to_string(),
                reset_at,
            },
        });

        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

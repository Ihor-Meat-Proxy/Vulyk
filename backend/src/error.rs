use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    #[allow(dead_code)]
    Conflict(String),
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
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            AppError::Db(sqlx::Error::RowNotFound) => (StatusCode::NOT_FOUND, "not_found"),
            AppError::Db(sqlx::Error::Database(err)) if err.constraint().is_some() => {
                (StatusCode::CONFLICT, "conflict")
            }
            AppError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };

        let body = Json(ErrorBody {
            error: ErrorDetail {
                code,
                message: self.to_string(),
            },
        });

        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

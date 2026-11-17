use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

/// RFC 9457 Problem Details (`application/problem+json`) plus a transitional
/// nested `error` object so existing clients that read `{error:{code,message}}`
/// keep working.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProblemDetails {
    /// URI reference that identifies the problem type (RFC 9457 `type`).
    #[schema(example = "urn:vulyk:api:problem:not_found")]
    pub r#type: String,
    /// Short, human-readable summary of the problem type.
    #[schema(example = "Not Found")]
    pub title: String,
    /// HTTP status code generated for this occurrence.
    #[schema(example = 404)]
    pub status: u16,
    /// Human-readable explanation specific to this occurrence.
    #[schema(example = "not found: project")]
    pub detail: String,
    /// Stable machine-readable code (extension; also mirrored under `error.code`).
    #[schema(example = "not_found")]
    pub code: String,
    /// Present for `git_host_rate_limit` when the upstream reset time is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "date-time")]
    pub reset_at: Option<DateTime<Utc>>,
    /// Transitional nested shape for older clients (`error.code` / `error.message`).
    pub error: LegacyErrorDetail,
}

/// Nested error object retained for backward compatibility with clients that
/// parse `{ "error": { "code", "message" } }`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LegacyErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "date-time")]
    pub reset_at: Option<DateTime<Utc>>,
}

pub const PROBLEM_JSON: &str = "application/problem+json";

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

impl AppError {
    /// Stable machine code, HTTP status, and RFC 9457 title for this error.
    pub fn problem_meta(&self) -> (StatusCode, &'static str, &'static str) {
        match self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found", "Not Found"),
            AppError::Gone(_) => (StatusCode::GONE, "case_deleted", "Gone"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request", "Bad Request"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "conflict", "Conflict"),
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized", "Unauthorized"),
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden", "Forbidden"),
            AppError::TooManyRequests(_) => {
                (StatusCode::TOO_MANY_REQUESTS, "too_many_requests", "Too Many Requests")
            }
            AppError::GitHostRateLimit { .. } => (
                StatusCode::TOO_MANY_REQUESTS,
                "git_host_rate_limit",
                "Too Many Requests",
            ),
            AppError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal", "Internal Server Error")
            }
            AppError::Db(sqlx::Error::RowNotFound) => {
                (StatusCode::NOT_FOUND, "not_found", "Not Found")
            }
            AppError::Db(sqlx::Error::Database(err)) if err.constraint().is_some() => {
                (StatusCode::CONFLICT, "conflict", "Conflict")
            }
            AppError::Db(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal", "Internal Server Error")
            }
        }
    }

    pub fn reset_at(&self) -> Option<DateTime<Utc>> {
        match self {
            AppError::GitHostRateLimit { reset_at, .. } => *reset_at,
            _ => None,
        }
    }

    pub fn into_problem(self) -> (StatusCode, ProblemDetails) {
        let (status, code, title) = self.problem_meta();
        let reset_at = self.reset_at();
        let detail = self.to_string();
        let problem = ProblemDetails {
            r#type: format!("urn:vulyk:api:problem:{code}"),
            title: title.to_string(),
            status: status.as_u16(),
            detail: detail.clone(),
            code: code.to_string(),
            reset_at,
            error: LegacyErrorDetail {
                code: code.to_string(),
                message: detail,
                reset_at,
            },
        };
        (status, problem)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, problem) = self.into_problem();
        problem_response(status, problem)
    }
}

/// Build an `application/problem+json` response from a [`ProblemDetails`] body.
pub fn problem_response(status: StatusCode, problem: ProblemDetails) -> Response {
    let mut res = (status, Json(problem)).into_response();
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(PROBLEM_JSON),
    );
    res
}

/// Build problem+json with an explicit machine code (middleware-only codes such as
/// `csrf_rejected` / `password_change_required`).
pub fn problem_with_code(
    status: StatusCode,
    code: &'static str,
    title: &'static str,
    detail: impl Into<String>,
) -> Response {
    let detail = detail.into();
    let reset_at = None;
    problem_response(
        status,
        ProblemDetails {
            r#type: format!("urn:vulyk:api:problem:{code}"),
            title: title.to_string(),
            status: status.as_u16(),
            detail: detail.clone(),
            code: code.to_string(),
            reset_at,
            error: LegacyErrorDetail {
                code: code.to_string(),
                message: detail,
                reset_at,
            },
        },
    )
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use serde_json::Value;

    async fn body_json(res: Response) -> Value {
        let bytes = to_bytes(res.into_body(), usize::MAX).await.expect("body");
        serde_json::from_slice(&bytes).expect("json")
    }

    #[tokio::test]
    async fn not_found_is_problem_json_with_legacy_error() {
        let res = AppError::NotFound("project".into()).into_response();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            PROBLEM_JSON
        );
        let body = body_json(res).await;
        assert_eq!(body["type"], "urn:vulyk:api:problem:not_found");
        assert_eq!(body["title"], "Not Found");
        assert_eq!(body["status"], 404);
        assert_eq!(body["code"], "not_found");
        assert_eq!(body["detail"], "not found: project");
        assert_eq!(body["error"]["code"], "not_found");
        assert_eq!(body["error"]["message"], "not found: project");
    }

    #[tokio::test]
    async fn bad_request_unauthorized_forbidden_shapes() {
        for (err, status, code, title) in [
            (
                AppError::BadRequest("name required".into()),
                StatusCode::BAD_REQUEST,
                "bad_request",
                "Bad Request",
            ),
            (
                AppError::Unauthorized("authentication required".into()),
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Unauthorized",
            ),
            (
                AppError::Forbidden("not a member of this project".into()),
                StatusCode::FORBIDDEN,
                "forbidden",
                "Forbidden",
            ),
        ] {
            let res = err.into_response();
            assert_eq!(res.status(), status);
            assert_eq!(
                res.headers().get(header::CONTENT_TYPE).unwrap(),
                PROBLEM_JSON
            );
            let body = body_json(res).await;
            assert_eq!(body["status"], status.as_u16());
            assert_eq!(body["code"], code);
            assert_eq!(body["title"], title);
            assert_eq!(body["type"], format!("urn:vulyk:api:problem:{code}"));
            assert_eq!(body["error"]["code"], code);
            assert!(body["detail"].as_str().is_some_and(|d| !d.is_empty()));
            assert_eq!(body["error"]["message"], body["detail"]);
        }
    }

    #[tokio::test]
    async fn git_host_rate_limit_includes_reset_at() {
        let reset = Utc::now();
        let res = AppError::GitHostRateLimit {
            message: "rate limited".into(),
            reset_at: Some(reset),
        }
        .into_response();
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
        let body = body_json(res).await;
        assert_eq!(body["code"], "git_host_rate_limit");
        assert!(body["reset_at"].is_string());
        assert!(body["error"]["reset_at"].is_string());
    }
}

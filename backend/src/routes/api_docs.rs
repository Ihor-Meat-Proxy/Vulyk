//! Self-hosted OpenAPI docs (Scalar) and machine-readable spec.
//!
//! `/api/docs` must not load scripts from a CDN — the API CSP is
//! `default-src 'none'` for JSON responses, and docs use a path-scoped CSP
//! with `script-src 'self'` plus the vendored Scalar standalone bundle.

use axum::{
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use utoipa::OpenApi;

use crate::openapi::ApiDoc;

const DOCS_HTML_TEMPLATE: &str = include_str!("../../assets/scalar/index.html");
const SCALAR_JS: &[u8] = include_bytes!("../../assets/scalar/standalone.js");

pub fn router() -> Router<crate::routes::AppState> {
    Router::new()
        .route("/api/docs", get(docs_html))
        .route("/api/docs/assets/scalar.js", get(scalar_js))
        .route("/api/openapi.json", get(openapi_json))
}

#[utoipa::path(
    get,
    path = "/api/docs",
    tag = "system",
    responses(
        (status = 200, description = "Self-hosted Scalar OpenAPI UI (HTML)"),
        (status = 500, description = "Failed to render OpenAPI document")
    )
)]
pub async fn docs_html() -> Response {
    let spec = match serde_json::to_string(&ApiDoc::openapi()) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "failed to serialize OpenAPI for docs");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to render OpenAPI docs",
            )
                .into_response();
        }
    };
    let html = DOCS_HTML_TEMPLATE
        .replace("$title", "Vulyk API")
        .replace("$spec", &spec);
    Html(html).into_response()
}

#[utoipa::path(
    get,
    path = "/api/docs/assets/scalar.js",
    tag = "system",
    responses((status = 200, description = "Vendored Scalar API Reference standalone bundle"))
)]
pub async fn scalar_js() -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/javascript; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    (headers, SCALAR_JS).into_response()
}

#[utoipa::path(
    get,
    path = "/api/openapi.json",
    tag = "system",
    responses((status = 200, description = "OpenAPI 3 document (JSON)"))
)]
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_template_uses_self_hosted_script() {
        assert!(
            DOCS_HTML_TEMPLATE.contains("/api/docs/assets/scalar.js"),
            "docs HTML must load Scalar from same origin"
        );
        assert!(
            !DOCS_HTML_TEMPLATE.contains("cdn.jsdelivr.net"),
            "docs HTML must not reference a CDN"
        );
        assert!(
            !DOCS_HTML_TEMPLATE.contains("https://"),
            "docs HTML must not load remote assets"
        );
        assert!(
            DOCS_HTML_TEMPLATE.contains("withDefaultFonts"),
            "docs must disable Scalar CDN fonts"
        );
    }

    #[test]
    fn scalar_bundle_is_non_empty() {
        assert!(SCALAR_JS.len() > 100_000, "vendored Scalar bundle missing?");
    }
}

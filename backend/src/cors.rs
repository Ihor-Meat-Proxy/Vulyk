//! CORS allowlist from env (security audit M3).

use axum::http::{header, HeaderName, HeaderValue, Method};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

/// Headers browsers may send on credentialed API calls (cannot use `*` with credentials).
fn allow_headers_list() -> [HeaderName; 6] {
    [
        header::AUTHORIZATION,
        header::CONTENT_TYPE,
        header::ACCEPT,
        header::ACCEPT_LANGUAGE,
        HeaderName::from_static("x-requested-with"),
        HeaderName::from_static("x-vulyk-csrf"),
    ]
}

/// Parse a public origin (`scheme://host[:port]`) from a base URL. Returns None if invalid.
pub fn origin_from_base_url(base: &str) -> Option<HeaderValue> {
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let (scheme, rest) = trimmed.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let hostport = rest.split('/').next().unwrap_or("");
    if hostport.is_empty() || hostport.contains(' ') {
        return None;
    }
    HeaderValue::from_str(&format!("{scheme}://{hostport}")).ok()
}

/// Collect allowlisted origins from `VULYK_CORS_ORIGINS` (comma-separated) and `VULYK_BASE_URL`.
pub fn resolve_cors_origins(
    cors_origins: Option<&str>,
    base_url: Option<&str>,
    production_like: bool,
) -> Vec<HeaderValue> {
    let mut out = Vec::new();
    if let Some(raw) = cors_origins {
        for part in raw.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Ok(v) = HeaderValue::from_str(part) {
                out.push(v);
            } else if let Some(v) = origin_from_base_url(part) {
                out.push(v);
            }
        }
    }
    if let Some(base) = base_url {
        if let Some(v) = origin_from_base_url(base) {
            if !out.contains(&v) {
                out.push(v);
            }
        }
    }
    // Production-like without an allowlist would be `Any` (bad). Prefer empty → credentials
    // layer with no origins (browser CORS fails closed) rather than reflecting Any.
    let _ = production_like;
    out
}

fn production_like_from_env() -> bool {
    let env = std::env::var("VULYK_ENV").ok();
    let base = std::env::var("VULYK_BASE_URL").ok();
    let env_prod = matches!(
        env.as_deref()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref(),
        Some("production" | "prod")
    );
    let https_base = base
        .as_deref()
        .map(|u| u.trim().to_ascii_lowercase().starts_with("https://"))
        .unwrap_or(false);
    env_prod || https_base
}

/// Build CORS layer: allowlist (+ credentials) when origins are configured or
/// production-like; keep `Any` for local DX when neither applies.
pub fn cors_layer_from_env() -> CorsLayer {
    let production_like = production_like_from_env();
    let origins = resolve_cors_origins(
        std::env::var("VULYK_CORS_ORIGINS").ok().as_deref(),
        std::env::var("VULYK_BASE_URL").ok().as_deref(),
        production_like,
    );
    let methods = [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::OPTIONS,
    ];

    if origins.is_empty() && !production_like {
        tracing::info!("CORS: allow_origin(Any) (local/dev; set VULYK_CORS_ORIGINS to lock down)");
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(methods)
            .allow_headers(Any);
    }

    if origins.is_empty() {
        tracing::warn!(
            "CORS: production-like deploy with empty allowlist — browser cross-origin calls will fail; set VULYK_CORS_ORIGINS and/or VULYK_BASE_URL"
        );
    } else {
        tracing::info!(
            count = origins.len(),
            "CORS: allowlist enabled with credentials"
        );
    }

    // Credentials forbid Allow-Headers: *; list the headers the SPA / clients send.
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods(methods)
        .allow_headers(allow_headers_list())
        .allow_credentials(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_from_base_strips_path() {
        let v = origin_from_base_url("https://app.example.com/api/v1").unwrap();
        assert_eq!(v, "https://app.example.com");
        let v = origin_from_base_url("http://localhost:5173/").unwrap();
        assert_eq!(v, "http://localhost:5173");
    }

    #[test]
    fn resolve_merges_cors_origins_and_base() {
        let list = resolve_cors_origins(
            Some("https://ui.example, http://localhost:5173"),
            Some("https://api.example/app"),
            true,
        );
        let as_str: Vec<_> = list
            .iter()
            .map(|h| h.to_str().unwrap().to_string())
            .collect();
        assert!(as_str.contains(&"https://ui.example".into()));
        assert!(as_str.contains(&"http://localhost:5173".into()));
        assert!(as_str.contains(&"https://api.example".into()));
    }

    #[test]
    fn resolve_empty_without_env() {
        assert!(resolve_cors_origins(None, None, false).is_empty());
    }

    #[test]
    fn allowlist_layer_builds_without_panic() {
        // tower-http panics if credentials + Allow-Headers:* are combined.
        let origins = resolve_cors_origins(None, Some("http://127.0.0.1:5173"), false);
        assert_eq!(origins.len(), 1);
        let _layer = CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([Method::GET])
            .allow_headers(allow_headers_list())
            .allow_credentials(true);
    }
}

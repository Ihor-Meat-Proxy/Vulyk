//! Baseline HTTP security headers for the API (security audit M4).

use axum::{
    extract::Request,
    http::{header, HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};

/// Whether responses should advertise HSTS (HTTPS / production-like deploy).
pub fn hsts_enabled(
    env: Option<&str>,
    base_url: Option<&str>,
    cookie_secure: Option<&str>,
) -> bool {
    if matches!(
        cookie_secure
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref(),
        Some("1" | "true" | "yes" | "on")
    ) {
        return true;
    }
    let env_prod = matches!(
        env.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
        Some("production" | "prod")
    );
    let https_base = base_url
        .map(|u| u.trim().to_ascii_lowercase().starts_with("https://"))
        .unwrap_or(false);
    env_prod || https_base
}

fn hsts_enabled_from_env() -> bool {
    hsts_enabled(
        std::env::var("VULYK_ENV").ok().as_deref(),
        std::env::var("VULYK_BASE_URL").ok().as_deref(),
        std::env::var("VULYK_COOKIE_SECURE").ok().as_deref(),
    )
}

/// Insert baseline security headers. API responses are JSON-only so CSP is strict.
pub fn insert_security_headers(headers: &mut HeaderMap, enable_hsts: bool) {
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        header::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    // API is not an HTML document surface — lock down any accidental HTML error pages.
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'; base-uri 'none'"),
    );
    if enable_hsts {
        headers.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
}

pub async fn security_headers_middleware(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    insert_security_headers(res.headers_mut(), hsts_enabled_from_env());
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_baseline_headers_without_hsts_by_default() {
        let mut headers = HeaderMap::new();
        insert_security_headers(&mut headers, false);
        assert_eq!(
            headers.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(),
            "nosniff"
        );
        assert_eq!(headers.get(header::X_FRAME_OPTIONS).unwrap(), "DENY");
        assert_eq!(
            headers
                .get(header::HeaderName::from_static("referrer-policy"))
                .unwrap(),
            "strict-origin-when-cross-origin"
        );
        assert!(headers.get(header::STRICT_TRANSPORT_SECURITY).is_none());
        assert!(headers
            .get(header::CONTENT_SECURITY_POLICY)
            .unwrap()
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'"));
    }

    #[test]
    fn hsts_when_cookie_secure_or_https_base() {
        assert!(!hsts_enabled(None, None, None));
        assert!(hsts_enabled(None, None, Some("true")));
        assert!(hsts_enabled(None, Some("https://app.example"), None));
        assert!(hsts_enabled(Some("production"), None, None));
        assert!(!hsts_enabled(
            Some("development"),
            Some("http://localhost"),
            None
        ));
    }
}

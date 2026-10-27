//! Egress URL policy: block SSRF to private/link-local/metadata addresses.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

use crate::error::{AppError, AppResult};

fn allow_private_egress() -> bool {
    std::env::var("VULYK_EGRESS_ALLOW_PRIVATE")
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

/// Validate that `raw` is an `https://` URL whose host resolves only to public IPs.
/// Rejects `file://`, `git://`, `http://`, literal private IPs, and DNS that resolves
/// into RFC1918 / link-local / loopback / cloud metadata ranges.
///
/// Set `VULYK_EGRESS_ALLOW_PRIVATE=1` only for local/dev/tests (e.g. mock HTTP servers).
pub fn validate_public_https_url(raw: &str) -> AppResult<()> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(AppError::BadRequest("URL is required".into()));
    }
    let lower = raw.to_ascii_lowercase();
    if allow_private_egress() {
        if !(lower.starts_with("https://") || lower.starts_with("http://")) {
            return Err(AppError::BadRequest(
                "only http(s):// URLs are allowed for remote egress".into(),
            ));
        }
        return Ok(());
    }
    if !(lower.starts_with("https://")) {
        return Err(AppError::BadRequest(
            "only https:// URLs are allowed for remote egress".into(),
        ));
    }

    let without_scheme = &raw[8..];
    let authority = without_scheme.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err(AppError::BadRequest("URL host is required".into()));
    }

    // Strip userinfo if present (https://user:pass@host).
    let hostport = authority.rsplit('@').next().unwrap_or(authority);
    let host = if hostport.starts_with('[') {
        let end = hostport
            .find(']')
            .ok_or_else(|| AppError::BadRequest("invalid IPv6 URL host".into()))?;
        &hostport[1..end]
    } else {
        hostport.split(':').next().unwrap_or(hostport)
    };
    if host.is_empty() || host.eq_ignore_ascii_case("localhost") {
        return Err(AppError::BadRequest(
            "URL host is not allowed (localhost)".into(),
        ));
    }

    // Literal IP — check without DNS.
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !is_public_ip(ip) {
            return Err(AppError::BadRequest(
                "URL host resolves to a private or reserved address".into(),
            ));
        }
        return Ok(());
    }

    // DNS resolve (with dummy port); every A/AAAA must be public.
    let addrs = (host, 443)
        .to_socket_addrs()
        .map_err(|e| AppError::BadRequest(format!("URL host could not be resolved: {e}")))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        if !is_public_ip(addr.ip()) {
            return Err(AppError::BadRequest(
                "URL host resolves to a private or reserved address".into(),
            ));
        }
    }
    if !any {
        return Err(AppError::BadRequest(
            "URL host could not be resolved".into(),
        ));
    }
    Ok(())
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    if ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || ip.is_multicast()
    {
        return false;
    }
    // CGNAT / shared (RFC 6598) 100.64.0.0/10
    let o = ip.octets();
    if o[0] == 100 && (o[1] & 0xc0) == 64 {
        return false;
    }
    // IETF protocol assignments 192.0.0.0/24 (includes some specials)
    if o[0] == 192 && o[1] == 0 && o[2] == 0 {
        return false;
    }
    // TEST-NET and documentation
    if o[0] == 192 && o[1] == 0 && o[2] == 2 {
        return false;
    }
    if o[0] == 198 && o[1] == 51 && o[2] == 100 {
        return false;
    }
    if o[0] == 203 && o[1] == 0 && o[2] == 113 {
        return false;
    }
    // Cloud metadata commonly 169.254.169.254 (already link-local) — covered.
    // Benchmarking 198.18.0.0/15
    if o[0] == 198 && (o[1] & 0xfe) == 18 {
        return false;
    }
    true
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return false;
    }
    // Unique local fc00::/7
    let s = ip.segments();
    if (s[0] & 0xfe00) == 0xfc00 {
        return false;
    }
    // Link-local fe80::/10
    if (s[0] & 0xffc0) == 0xfe80 {
        return false;
    }
    // IPv4-mapped — recurse on embedded v4.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_public_v4(v4);
    }
    true
}

/// Redact embedded credentials in URLs that may appear in error messages.
pub fn redact_url_credentials(text: &str) -> String {
    // https://user:pass@host → https://***@host
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(scheme_at) = rest.find("://") {
        let scheme_end = scheme_at + 3;
        out.push_str(&rest[..scheme_end]);
        rest = &rest[scheme_end..];
        if let Some(at) = rest.find('@') {
            let userinfo = &rest[..at];
            if !userinfo.is_empty()
                && !userinfo.contains('/')
                && !userinfo.contains(' ')
                && !userinfo.contains('\n')
            {
                out.push_str("***@");
                rest = &rest[at + 1..];
                continue;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_https_and_localhost() {
        assert!(validate_public_https_url("file:///etc/passwd").is_err());
        assert!(validate_public_https_url("http://example.com/repo.git").is_err());
        assert!(validate_public_https_url("https://localhost/repo.git").is_err());
        assert!(validate_public_https_url("https://127.0.0.1/repo.git").is_err());
        assert!(validate_public_https_url("https://10.0.0.5/repo.git").is_err());
        assert!(validate_public_https_url("https://169.254.169.254/latest").is_err());
        assert!(validate_public_https_url("https://192.168.1.1/x").is_err());
    }

    #[test]
    fn accepts_public_literal() {
        // 8.8.8.8 is public; no DNS needed.
        assert!(validate_public_https_url("https://8.8.8.8/repo.git").is_ok());
    }

    #[test]
    fn redacts_userinfo() {
        let s = redact_url_credentials(
            "git clone failed: fatal: https://oauth2:glpat-SECRET@gitlab.com/a/b.git",
        );
        assert!(!s.contains("glpat-SECRET"));
        assert!(s.contains("https://***@gitlab.com/a/b.git"));
    }
}

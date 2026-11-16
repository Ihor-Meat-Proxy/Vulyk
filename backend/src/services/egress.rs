//! Egress URL policy: block SSRF to private/link-local/metadata addresses.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::str::FromStr;
use std::sync::{Arc, RwLock};

use crate::error::{AppError, AppResult};

fn allow_private_egress() -> bool {
    std::env::var("VULYK_EGRESS_ALLOW_PRIVATE")
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

/// Optional DNS override for tests.
///
/// Return `Some(Ok(ips))` / `Some(Err(_))` to short-circuit system DNS, or
/// `None` to fall through to the OS resolver. Production leaves this unset.
pub type DnsResolverHook =
    Arc<dyn Fn(&str) -> Option<Result<Vec<IpAddr>, String>> + Send + Sync + 'static>;

static DNS_RESOLVER_HOOK: RwLock<Option<DnsResolverHook>> = RwLock::new(None);

/// Install (or clear with `None`) a process-wide DNS hook used by egress checks.
///
/// Intended for unit/integration tests so fixtures can keep reserved names like
/// `gitlab.example.com` without live DNS. Never set in production.
pub fn set_dns_resolver_for_tests(hook: Option<DnsResolverHook>) {
    if let Ok(mut guard) = DNS_RESOLVER_HOOK.write() {
        *guard = hook;
    }
}

/// Stub RFC 2606 / special-use documentation hostnames as a public IP so CI
/// URL validation does not require network. Other hosts fall through to OS DNS.
pub fn install_documentation_dns_stub_for_tests() {
    set_dns_resolver_for_tests(Some(Arc::new(|host: &str| {
        if is_documentation_hostname(host) {
            Some(Ok(vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))]))
        } else {
            None
        }
    })));
}

fn is_documentation_hostname(host: &str) -> bool {
    let h = host.to_ascii_lowercase();
    h == "example"
        || h == "example.com"
        || h == "example.net"
        || h == "example.org"
        || h.ends_with(".example")
        || h.ends_with(".example.com")
        || h.ends_with(".example.net")
        || h.ends_with(".example.org")
        || h.ends_with(".test")
        || h.ends_with(".invalid")
}

fn resolve_host_ips(host: &str) -> AppResult<Vec<IpAddr>> {
    if let Ok(guard) = DNS_RESOLVER_HOOK.read() {
        if let Some(hook) = guard.as_ref() {
            if let Some(outcome) = hook(host) {
                return outcome.map_err(|e| {
                    AppError::BadRequest(format!("URL host could not be resolved: {e}"))
                });
            }
        }
    }
    let addrs = (host, 443)
        .to_socket_addrs()
        .map_err(|e| AppError::BadRequest(format!("URL host could not be resolved: {e}")))?;
    let ips: Vec<IpAddr> = addrs.map(|a| a.ip()).collect();
    if ips.is_empty() {
        return Err(AppError::BadRequest(
            "URL host could not be resolved".into(),
        ));
    }
    Ok(ips)
}

/// Admin / env policy for CI destinations (Jenkins, GitHub-compatible Forgejo, GitLab).
///
/// When [`Self::allow_private`] is false (default), behaviour matches
/// [`validate_public_https_url`]. When true:
/// - `http://` and `https://` are accepted
/// - private / loopback / link-local hosts are accepted
/// - if [`Self::allowlist`] is **empty**, any such host is allowed
/// - if [`Self::allowlist`] is **non-empty**, only listed hosts / CIDRs match
///
/// Public `https://` destinations remain allowed regardless of the opt-in.
/// `file://` and other schemes are never allowed for CI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CiUrlPolicy {
    pub allow_private: bool,
    pub allowlist: Vec<String>,
}

impl CiUrlPolicy {
    pub fn strict() -> Self {
        Self::default()
    }

    pub fn from_parts(allow_private: bool, allowlist: impl IntoIterator<Item = String>) -> Self {
        let mut entries: Vec<String> = allowlist
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        entries.sort();
        entries.dedup();
        Self {
            allow_private,
            allowlist: entries,
        }
    }
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
        // Dev/test escape hatch: allow http(s) and local file:// fixtures.
        if !(lower.starts_with("https://")
            || lower.starts_with("http://")
            || lower.starts_with("file://"))
        {
            return Err(AppError::BadRequest(
                "only http(s):// or file:// URLs are allowed when private egress is enabled".into(),
            ));
        }
        return Ok(());
    }
    if !(lower.starts_with("https://")) {
        return Err(AppError::BadRequest(
            "only https:// URLs are allowed for remote egress".into(),
        ));
    }

    let host = extract_url_host(raw)?;
    validate_host_public_only(&host)
}

/// Validate an automation-repo Git remote under [`CiUrlPolicy`].
///
/// Same rules as [`validate_ci_url`], except the lab hatch
/// `VULYK_EGRESS_ALLOW_PRIVATE=1` also permits `file://` fixture remotes
/// (used by integration tests / local clones). Production never sets that
/// env, so `file://` stays blocked there.
pub fn validate_git_remote_url(raw: &str, policy: &CiUrlPolicy) -> AppResult<()> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(AppError::BadRequest("URL is required".into()));
    }
    let lower = raw.to_ascii_lowercase();
    if allow_private_egress() && lower.starts_with("file://") {
        return Ok(());
    }
    validate_ci_url(raw, policy)
}

/// Validate a CI base / remote URL under [`CiUrlPolicy`].
///
/// Lab hatch `VULYK_EGRESS_ALLOW_PRIVATE=1` still relaxes checks (http + private),
/// but never permits `file://` for CI destinations.
pub fn validate_ci_url(raw: &str, policy: &CiUrlPolicy) -> AppResult<()> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(AppError::BadRequest("URL is required".into()));
    }
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("file://") || lower.starts_with("git://") {
        return Err(AppError::BadRequest(
            "only http(s):// URLs are allowed for CI destinations".into(),
        ));
    }

    let lab = allow_private_egress();
    let allow_insecure = lab || policy.allow_private;

    if allow_insecure {
        if !(lower.starts_with("https://") || lower.starts_with("http://")) {
            return Err(AppError::BadRequest(
                "only http(s):// URLs are allowed for CI destinations".into(),
            ));
        }
    } else if !lower.starts_with("https://") {
        return Err(AppError::BadRequest(
            "only https:// URLs are allowed for CI destinations (enable Admin → Allow private CI URLs for http/LAN hosts)".into(),
        ));
    }

    let host = extract_url_host(raw)?;
    let is_https = lower.starts_with("https://");
    let public = host_is_public_ok(&host).unwrap_or(false);

    // Public HTTPS is always allowed (allowlist only scopes the private/http opt-in).
    if is_https && public {
        return Ok(());
    }

    // Everything else (http, private, localhost) requires the opt-in.
    if !allow_insecure {
        if !is_https {
            return Err(AppError::BadRequest(
                "only https:// URLs are allowed for CI destinations (enable Admin → Allow private CI URLs for http/LAN hosts)".into(),
            ));
        }
        return Err(AppError::BadRequest(
            "URL host is not allowed (private or reserved); enable Admin → Allow private CI URLs"
                .into(),
        ));
    }

    // Opt-in + empty allowlist = any private/localhost (and http, including public http).
    if policy.allowlist.is_empty() {
        return Ok(());
    }

    // Non-empty allowlist: only listed hosts/CIDRs for the relaxed destinations.
    ensure_allowlist_match(&host, &policy.allowlist)
}

fn extract_url_host(raw: &str) -> AppResult<String> {
    let lower = raw.to_ascii_lowercase();
    let scheme_len = if lower.starts_with("https://") {
        8
    } else if lower.starts_with("http://") {
        7
    } else {
        return Err(AppError::BadRequest(
            "only http(s):// URLs are allowed".into(),
        ));
    };
    let without_scheme = &raw[scheme_len..];
    let authority = without_scheme.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err(AppError::BadRequest("URL host is required".into()));
    }
    let hostport = authority.rsplit('@').next().unwrap_or(authority);
    let host = if hostport.starts_with('[') {
        let end = hostport
            .find(']')
            .ok_or_else(|| AppError::BadRequest("invalid IPv6 URL host".into()))?;
        hostport[1..end].to_string()
    } else {
        hostport.split(':').next().unwrap_or(hostport).to_string()
    };
    if host.is_empty() {
        return Err(AppError::BadRequest("URL host is required".into()));
    }
    Ok(host)
}

fn validate_host_public_only(host: &str) -> AppResult<()> {
    if host.eq_ignore_ascii_case("localhost") {
        return Err(AppError::BadRequest(
            "URL host is not allowed (localhost)".into(),
        ));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !is_public_ip(ip) {
            return Err(AppError::BadRequest(
                "URL host resolves to a private or reserved address".into(),
            ));
        }
        return Ok(());
    }
    let ips = resolve_host_ips(host)?;
    for ip in ips {
        if !is_public_ip(ip) {
            return Err(AppError::BadRequest(
                "URL host resolves to a private or reserved address".into(),
            ));
        }
    }
    Ok(())
}

/// Returns `Ok(true)` when the host is clearly public, `Ok(false)` when clearly
/// private/loopback/link-local, or `Err` on DNS failure for non-literal hosts
/// that must resolve under the strict policy. For CI private mode we treat
/// localhost and literal private IPs as non-public without DNS.
fn host_is_public_ok(host: &str) -> AppResult<bool> {
    if host.eq_ignore_ascii_case("localhost") {
        return Ok(false);
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(is_public_ip(ip));
    }
    // Hostname: try DNS (or the test hook). If resolution fails, treat as
    // not-public so private mode + allowlist can still match by hostname
    // string without DNS.
    match resolve_host_ips(host) {
        Ok(ips) => {
            if ips.is_empty() {
                return Ok(false);
            }
            Ok(ips.iter().copied().all(is_public_ip))
        }
        Err(_) => Ok(false),
    }
}

fn ensure_allowlist_match(host: &str, allowlist: &[String]) -> AppResult<()> {
    if allowlist_matches(host, allowlist) {
        return Ok(());
    }
    Err(AppError::BadRequest(
        "URL host is not on the CI URL allowlist".into(),
    ))
}

fn allowlist_matches(host: &str, allowlist: &[String]) -> bool {
    let host_l = host.to_ascii_lowercase();
    for entry in allowlist {
        let e = entry.trim();
        if e.is_empty() {
            continue;
        }
        // Hostname (exact, case-insensitive)
        if e.eq_ignore_ascii_case(host) || e.eq_ignore_ascii_case(&host_l) {
            return true;
        }
        // Literal IP equality
        if let (Ok(host_ip), Ok(entry_ip)) = (host.parse::<IpAddr>(), e.parse::<IpAddr>()) {
            if host_ip == entry_ip {
                return true;
            }
        }
        // CIDR
        if let Some((net, prefix)) = parse_cidr(e) {
            if let Ok(host_ip) = host.parse::<IpAddr>() {
                if ip_in_cidr(host_ip, net, prefix) {
                    return true;
                }
            } else if let Ok(ips) = resolve_host_ips(host) {
                for ip in ips {
                    if ip_in_cidr(ip, net, prefix) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn parse_cidr(s: &str) -> Option<(IpAddr, u8)> {
    let (addr_s, pref_s) = s.split_once('/')?;
    let prefix: u8 = pref_s.parse().ok()?;
    let addr = IpAddr::from_str(addr_s.trim()).ok()?;
    match addr {
        IpAddr::V4(_) if prefix > 32 => return None,
        IpAddr::V6(_) if prefix > 128 => return None,
        _ => {}
    }
    Some((addr, prefix))
}

fn ip_in_cidr(ip: IpAddr, network: IpAddr, prefix: u8) -> bool {
    match (ip, network) {
        (IpAddr::V4(ip), IpAddr::V4(net)) => {
            let mask = if prefix == 0 {
                0u32
            } else {
                u32::MAX << (32 - prefix)
            };
            (u32::from(ip) & mask) == (u32::from(net) & mask)
        }
        (IpAddr::V6(ip), IpAddr::V6(net)) => {
            let ip_bytes = u128::from(ip);
            let net_bytes = u128::from(net);
            let mask = if prefix == 0 {
                0u128
            } else {
                u128::MAX << (128 - prefix)
            };
            (ip_bytes & mask) == (net_bytes & mask)
        }
        _ => false,
    }
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

    fn with_cleared_lab_hatch<T>(f: impl FnOnce() -> T) -> T {
        // Ensure unit tests are not affected by ambient lab hatch.
        let prev = std::env::var("VULYK_EGRESS_ALLOW_PRIVATE").ok();
        std::env::remove_var("VULYK_EGRESS_ALLOW_PRIVATE");
        let out = f();
        match prev {
            Some(v) => std::env::set_var("VULYK_EGRESS_ALLOW_PRIVATE", v),
            None => std::env::remove_var("VULYK_EGRESS_ALLOW_PRIVATE"),
        }
        out
    }

    #[test]
    fn rejects_non_https_and_localhost() {
        with_cleared_lab_hatch(|| {
            assert!(validate_public_https_url("file:///etc/passwd").is_err());
            assert!(validate_public_https_url("http://example.com/repo.git").is_err());
            assert!(validate_public_https_url("https://localhost/repo.git").is_err());
            assert!(validate_public_https_url("https://127.0.0.1/repo.git").is_err());
            assert!(validate_public_https_url("https://10.0.0.5/repo.git").is_err());
            assert!(validate_public_https_url("https://169.254.169.254/latest").is_err());
            assert!(validate_public_https_url("https://192.168.1.1/x").is_err());
        });
    }

    #[test]
    fn accepts_public_literal() {
        with_cleared_lab_hatch(|| {
            // 8.8.8.8 is public; no DNS needed.
            assert!(validate_public_https_url("https://8.8.8.8/repo.git").is_ok());
        });
    }

    #[test]
    fn redacts_userinfo() {
        let s = redact_url_credentials(
            "git clone failed: fatal: https://oauth2:glpat-SECRET@gitlab.com/a/b.git",
        );
        assert!(!s.contains("glpat-SECRET"));
        assert!(s.contains("https://***@gitlab.com/a/b.git"));
    }

    #[test]
    fn ci_https_public_ok_always() {
        with_cleared_lab_hatch(|| {
            let strict = CiUrlPolicy::strict();
            assert!(validate_ci_url("https://8.8.8.8/job/x", &strict).is_ok());
            let open = CiUrlPolicy::from_parts(true, Vec::<String>::new());
            assert!(validate_ci_url("https://8.8.8.8/job/x", &open).is_ok());
        });
    }

    #[test]
    fn ci_http_and_private_denied_by_default() {
        with_cleared_lab_hatch(|| {
            let strict = CiUrlPolicy::strict();
            assert!(validate_ci_url("http://example.com/job", &strict).is_err());
            assert!(validate_ci_url("http://localhost:8090/", &strict).is_err());
            assert!(validate_ci_url("https://127.0.0.1:8090/", &strict).is_err());
            assert!(validate_ci_url("https://10.0.0.5/", &strict).is_err());
            assert!(validate_ci_url("https://192.168.1.10/job", &strict).is_err());
            assert!(validate_ci_url("file:///etc/passwd", &strict).is_err());
        });
    }

    #[test]
    fn ci_opt_in_allows_http_localhost_and_private() {
        with_cleared_lab_hatch(|| {
            let open = CiUrlPolicy::from_parts(true, Vec::<String>::new());
            assert!(validate_ci_url("http://localhost:8090/", &open).is_ok());
            assert!(validate_ci_url("http://127.0.0.1:8090/job", &open).is_ok());
            assert!(validate_ci_url("https://10.0.0.5/job", &open).is_ok());
            assert!(validate_ci_url("http://192.168.1.10:8080/", &open).is_ok());
            assert!(validate_ci_url("https://8.8.8.8/job", &open).is_ok());
            // Still no file://
            assert!(validate_ci_url("file:///tmp/x", &open).is_err());
        });
    }

    #[test]
    fn ci_allowlist_restricts_when_non_empty() {
        with_cleared_lab_hatch(|| {
            let policy = CiUrlPolicy::from_parts(
                true,
                vec![
                    "localhost".into(),
                    "10.0.0.0/8".into(),
                    "jenkins.lan".into(),
                ],
            );
            assert!(validate_ci_url("http://localhost:8090/", &policy).is_ok());
            assert!(validate_ci_url("http://10.1.2.3:8080/", &policy).is_ok());
            assert!(validate_ci_url("https://jenkins.lan/job", &policy).is_ok());
            // Not on allowlist
            assert!(validate_ci_url("http://192.168.1.10/", &policy).is_err());
            assert!(validate_ci_url("http://127.0.0.1:8090/", &policy).is_err());
            // Public https remains allowed even with a non-empty allowlist
            assert!(validate_ci_url("https://8.8.8.8/job", &policy).is_ok());
        });
    }

    #[test]
    fn ci_allowlist_cidr_and_literal_ip() {
        with_cleared_lab_hatch(|| {
            let policy =
                CiUrlPolicy::from_parts(true, vec!["192.168.0.0/16".into(), "172.16.5.5".into()]);
            assert!(validate_ci_url("http://192.168.10.20/", &policy).is_ok());
            assert!(validate_ci_url("https://172.16.5.5/", &policy).is_ok());
            assert!(validate_ci_url("http://172.16.5.6/", &policy).is_err());
            assert!(validate_ci_url("http://10.0.0.1/", &policy).is_err());
        });
    }

    #[test]
    fn ci_ssrf_weird_schemes_still_blocked() {
        with_cleared_lab_hatch(|| {
            let open = CiUrlPolicy::from_parts(true, Vec::<String>::new());
            assert!(validate_ci_url("git://localhost/repo", &open).is_err());
            assert!(validate_ci_url("ftp://10.0.0.1/", &open).is_err());
            assert!(validate_ci_url("", &open).is_err());
        });
    }

    #[test]
    fn git_remote_file_only_under_lab_hatch() {
        with_cleared_lab_hatch(|| {
            let open = CiUrlPolicy::from_parts(true, Vec::<String>::new());
            assert!(validate_git_remote_url("file:///tmp/fixture.git", &open).is_err());
            assert!(validate_git_remote_url("https://8.8.8.8/repo.git", &open).is_ok());
        });
        std::env::set_var("VULYK_EGRESS_ALLOW_PRIVATE", "1");
        let open = CiUrlPolicy::from_parts(true, Vec::<String>::new());
        assert!(validate_git_remote_url("file:///tmp/fixture.git", &open).is_ok());
        // CI destinations still reject file:// even with the lab hatch.
        assert!(validate_ci_url("file:///tmp/fixture.git", &open).is_err());
        std::env::remove_var("VULYK_EGRESS_ALLOW_PRIVATE");
    }

    #[test]
    fn documentation_dns_stub_allows_example_hosts_offline() {
        with_cleared_lab_hatch(|| {
            set_dns_resolver_for_tests(None);
            // Without a stub, NXDOMAIN documentation hosts are not public.
            let strict = CiUrlPolicy::strict();
            assert!(validate_ci_url("https://gitlab.example.com/", &strict).is_err());

            install_documentation_dns_stub_for_tests();
            assert!(validate_ci_url("https://gitlab.example.com/", &strict).is_ok());
            assert!(validate_ci_url("https://ci.example/job", &strict).is_ok());
            assert!(validate_public_https_url("https://gitlab.example.com/a.git").is_ok());
            // Stub does not weaken literal private / localhost checks.
            assert!(validate_ci_url("https://127.0.0.1/", &strict).is_err());
            assert!(validate_ci_url("http://gitlab.example.com/", &strict).is_err());
            set_dns_resolver_for_tests(None);
        });
    }
}

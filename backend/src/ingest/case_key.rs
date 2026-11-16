//! Extract Vulyk / external case keys from titles, annotations, and common test names.
//!
//! Accepted patterns (case-insensitive; result is always `PREFIX-N` uppercased):
//! - Hyphenated: `VQA-110`, `@VQA-110`, `@vulyk:VQA-110`, `key=VQA-110`, `[VQA-110]`
//! - Underscore: `VQA_110`, `test_VQA_110_login`
//! - Compact with project prefix: when the project key is `VQA`, also `vqa110` / `VQA110`
//!   (digits immediately after the project key — avoids matching arbitrary `foo123`).

use regex::Regex;
use std::sync::OnceLock;

/// Match keys like `KC-12`, `KOC-001`, `DEMO-42` (2–8 alnum prefix + digits).
pub fn extract_case_key(text: &str) -> Option<String> {
    extract_case_key_with_project(text, None)
}

/// Like [`extract_case_key`], optionally accepting compact `{projectKey}{digits}` forms.
pub fn extract_case_key_with_project(text: &str, project_key: Option<&str>) -> Option<String> {
    if let Some(k) = extract_hyphenated_key(text) {
        return Some(k);
    }
    if let Some(k) = extract_underscore_key(text) {
        return Some(k);
    }
    if let Some(pk) = project_key.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(k) = extract_compact_project_key(text, pk) {
            return Some(k);
        }
    }
    None
}

/// Prefer explicit `case_key` / `@key=` annotations, then free-form title / body match.
pub fn resolve_case_key(
    explicit: Option<&str>,
    title: Option<&str>,
    external_id: Option<&str>,
) -> Option<String> {
    resolve_case_key_with_project(explicit, title, external_id, None, None)
}

/// Resolve a case key, optionally using the project key for compact forms and an
/// extra body/text candidate (e.g. JUnit `system-out` / skip message).
pub fn resolve_case_key_with_project(
    explicit: Option<&str>,
    title: Option<&str>,
    external_id: Option<&str>,
    body: Option<&str>,
    project_key: Option<&str>,
) -> Option<String> {
    if let Some(raw) = explicit.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(k) = normalize_explicit_key(raw, project_key) {
            return Some(k);
        }
    }
    for candidate in [title, external_id, body].into_iter().flatten() {
        if let Some(k) = extract_annotation_key(candidate) {
            return Some(k);
        }
        if let Some(k) = extract_case_key_with_project(candidate, project_key) {
            return Some(k);
        }
    }
    None
}

fn normalize_explicit_key(raw: &str, project_key: Option<&str>) -> Option<String> {
    let first = raw.split(',').next().unwrap_or(raw).trim();
    // Strip TestRail-style C prefix if present; keep Vulyk PREFIX-N as-is.
    let cleaned = first
        .strip_prefix('C')
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(first);
    if let Some(k) = extract_case_key_with_project(cleaned, project_key) {
        return Some(k);
    }
    // Bare digits + known project key → PREFIX-N
    if let Some(pk) = project_key.map(str::trim).filter(|s| !s.is_empty()) {
        if cleaned.chars().all(|c| c.is_ascii_digit()) {
            return Some(format!("{}-{}", pk.to_ascii_uppercase(), cleaned));
        }
    }
    // Already hyphenated PREFIX-N (strict)
    if extract_hyphenated_key(cleaned).is_some() {
        return extract_hyphenated_key(cleaned);
    }
    // Fallback: uppercase as-is when it looks like PREFIX-N after underscore rewrite
    if let Some(k) = extract_underscore_key(cleaned) {
        return Some(k);
    }
    let upper = cleaned.to_ascii_uppercase();
    if upper.contains('-') {
        Some(upper)
    } else {
        None
    }
}

fn extract_hyphenated_key(text: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)\b([A-Z][A-Z0-9]{1,7}-\d{1,8})\b").expect("hyphen case key regex")
    });
    re.find(text).map(|m| m.as_str().to_ascii_uppercase())
}

fn extract_underscore_key(text: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    // test_VQA_110_login, VQA_110, suite_VQA_12
    // Note: do not require \b after digits — `_` is a word char, so
    // `110_login` would not match `\d+\b`.
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:^|[^A-Za-z0-9])(?:test[_-]?)?([A-Z][A-Z0-9]{1,7})[_-](\d{1,8})(?:[^0-9]|$)",
        )
        .expect("underscore case key regex")
    });
    re.captures(text).map(|c| {
        format!(
            "{}-{}",
            c.get(1).unwrap().as_str().to_ascii_uppercase(),
            c.get(2).unwrap().as_str()
        )
    })
}

fn extract_compact_project_key(text: &str, project_key: &str) -> Option<String> {
    let pk = project_key.trim();
    if pk.is_empty() || !pk.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let pat = format!(r"(?i)\b{}(\d{{1,8}})\b", regex::escape(pk));
    let re = Regex::new(&pat).ok()?;
    re.captures(text)
        .map(|c| format!("{}-{}", pk.to_ascii_uppercase(), c.get(1).unwrap().as_str()))
}

fn extract_annotation_key(text: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)(?:@vulyk:|@key=|key=|case_key=|\[)([A-Z][A-Z0-9]{1,7}-\d{1,8})(?:\])?")
            .expect("annotation regex")
    });
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_from_title() {
        assert_eq!(
            extract_case_key("Enable app KC-012 via UI"),
            Some("KC-012".into())
        );
        assert_eq!(
            resolve_case_key(None, Some("@vulyk:DEMO-3 login"), None),
            Some("DEMO-3".into())
        );
    }

    #[test]
    fn extracts_underscore_and_test_prefix() {
        assert_eq!(
            extract_case_key("test_VQA_110_login"),
            Some("VQA-110".into())
        );
        assert_eq!(extract_case_key("VQA_110"), Some("VQA-110".into()));
        assert_eq!(
            extract_case_key_with_project("vqa110 should map", Some("VQA")),
            Some("VQA-110".into())
        );
        // Without project key, compact form is rejected (too ambiguous).
        assert_eq!(extract_case_key("vqa110"), None);
    }

    #[test]
    fn resolves_from_body_when_title_lacks_key() {
        assert_eq!(
            resolve_case_key_with_project(
                None,
                Some("anonymous skip"),
                Some("pkg::anon"),
                Some("see VQA_42 in skip reason"),
                Some("VQA"),
            ),
            Some("VQA-42".into())
        );
    }
}

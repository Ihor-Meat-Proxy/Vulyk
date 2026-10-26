//! Extract Vulyk / external case keys from titles and annotations.

use regex::Regex;
use std::sync::OnceLock;

/// Match keys like `KC-12`, `KOC-001`, `DEMO-42` (2–8 alnum prefix + digits).
pub fn extract_case_key(text: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)\b([A-Z][A-Z0-9]{1,7}-\d{1,8})\b").expect("case key regex")
    });
    re.find(text).map(|m| m.as_str().to_ascii_uppercase())
}

/// Prefer explicit `case_key` / `@key=` annotations, then free-form title match.
pub fn resolve_case_key(
    explicit: Option<&str>,
    title: Option<&str>,
    external_id: Option<&str>,
) -> Option<String> {
    if let Some(k) = explicit.map(str::trim).filter(|s| !s.is_empty()) {
        return Some(k.to_ascii_uppercase());
    }
    for candidate in [title, external_id].into_iter().flatten() {
        // Annotation forms: @vulyk:KC-1, key=KC-1, [KC-1]
        if let Some(k) = extract_annotation_key(candidate) {
            return Some(k);
        }
        if let Some(k) = extract_case_key(candidate) {
            return Some(k);
        }
    }
    None
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
}

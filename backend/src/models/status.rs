//! Canonical execution result statuses (PascalCase).

use std::fmt;

/// Untested — initial / not yet executed.
pub const UNTESTED: &str = "Untested";
/// Passed
pub const PASSED: &str = "Passed";
/// Failed
pub const FAILED: &str = "Failed";
/// Blocked
pub const BLOCKED: &str = "Blocked";
/// Skipped
pub const SKIPPED: &str = "Skipped";
/// Retest — mark for re-execution
pub const RETEST: &str = "Retest";

/// Statuses allowed on run items (current state of a case in a run).
pub const RUN_ITEM_STATUSES: &[&str] = &[UNTESTED, PASSED, FAILED, BLOCKED, SKIPPED, RETEST];

/// Statuses allowed on result / result-step records.
pub const RESULT_STATUSES: &[&str] = &[UNTESTED, PASSED, FAILED, BLOCKED, SKIPPED, RETEST];

/// Statuses allowed on individual steps (no Retest).
pub const STEP_STATUSES: &[&str] = &[UNTESTED, PASSED, FAILED, BLOCKED, SKIPPED];

/// Normalize free-form / legacy / JUnit status strings to canonical PascalCase.
///
/// JUnit mapping: `skipped` → Skipped, `error` → Failed.
/// Accepts lowercase, PascalCase, and common synonyms.
pub fn normalize_result_status(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "pass" | "passed" | "success" | "successful" | "ok" => PASSED.into(),
        "fail" | "failed" | "failure" | "error" | "errored" => FAILED.into(),
        "skip" | "skipped" | "ignored" => SKIPPED.into(),
        "blocked" | "block" => BLOCKED.into(),
        "untested" | "none" | "" => UNTESTED.into(),
        "retest" => RETEST.into(),
        // Already-canonical or unknown — title-case known set, else pass through trimmed.
        other => canonical_or_passthrough(other, raw.trim()),
    }
}

fn canonical_or_passthrough(lower: &str, original: &str) -> String {
    for s in RESULT_STATUSES {
        if s.eq_ignore_ascii_case(lower) || s.eq_ignore_ascii_case(original) {
            return (*s).into();
        }
    }
    original.to_string()
}

pub fn is_valid_run_item_status(s: &str) -> bool {
    RUN_ITEM_STATUSES.iter().any(|a| a.eq_ignore_ascii_case(s)) || RUN_ITEM_STATUSES.contains(&s)
}

pub fn is_valid_result_status(s: &str) -> bool {
    let n = normalize_result_status(s);
    RESULT_STATUSES.contains(&n.as_str())
}

pub fn is_valid_step_status(s: &str) -> bool {
    let n = normalize_result_status(s);
    STEP_STATUSES.contains(&n.as_str())
}

/// Case-insensitive match against a canonical status.
pub fn status_eq(a: &str, canonical: &str) -> bool {
    a.eq_ignore_ascii_case(canonical)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultStatus {
    Untested,
    Passed,
    Failed,
    Blocked,
    Skipped,
    Retest,
}

impl ResultStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Untested => UNTESTED,
            Self::Passed => PASSED,
            Self::Failed => FAILED,
            Self::Blocked => BLOCKED,
            Self::Skipped => SKIPPED,
            Self::Retest => RETEST,
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match normalize_result_status(raw).as_str() {
            UNTESTED => Some(Self::Untested),
            PASSED => Some(Self::Passed),
            FAILED => Some(Self::Failed),
            BLOCKED => Some(Self::Blocked),
            SKIPPED => Some(Self::Skipped),
            RETEST => Some(Self::Retest),
            _ => None,
        }
    }
}

impl fmt::Display for ResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junit_mapping() {
        assert_eq!(normalize_result_status("skipped"), SKIPPED);
        assert_eq!(normalize_result_status("error"), FAILED);
        assert_eq!(normalize_result_status("passed"), PASSED);
        assert_eq!(normalize_result_status("Passed"), PASSED);
        assert_eq!(normalize_result_status("FAILED"), FAILED);
        assert_eq!(normalize_result_status("retest"), RETEST);
    }
}

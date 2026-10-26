//! Canonical execution result statuses (PascalCase).

use std::fmt;

/// Untested — initial / not yet executed (legacy alias of Ready).
pub const UNTESTED: &str = "Untested";
/// Ready — assigned but not started (SquashTM-style; preferred for iteration items).
pub const READY: &str = "Ready";
/// Running — currently executing.
pub const RUNNING: &str = "Running";
/// Passed
pub const PASSED: &str = "Passed";
/// Failed
pub const FAILED: &str = "Failed";
/// Blocked
pub const BLOCKED: &str = "Blocked";
/// Skipped
pub const SKIPPED: &str = "Skipped";
/// Untestable — cannot be executed in the current environment.
pub const UNTESTABLE: &str = "Untestable";
/// Retest — mark for re-execution
pub const RETEST: &str = "Retest";

/// Statuses allowed on run items (current state of a case in a run).
pub const RUN_ITEM_STATUSES: &[&str] = &[
    UNTESTED, READY, RUNNING, PASSED, FAILED, BLOCKED, SKIPPED, RETEST, UNTESTABLE,
];

/// Statuses allowed on result / result-step records.
pub const RESULT_STATUSES: &[&str] = &[
    UNTESTED, READY, RUNNING, PASSED, FAILED, BLOCKED, SKIPPED, RETEST, UNTESTABLE,
];

/// Statuses allowed on individual steps (no Retest).
pub const STEP_STATUSES: &[&str] = &[
    UNTESTED, READY, RUNNING, PASSED, FAILED, BLOCKED, SKIPPED, UNTESTABLE,
];

/// Dashboard donut statuses (SquashTM-style).
pub const ITERATION_DASHBOARD_STATUSES: &[&str] =
    &[READY, RUNNING, PASSED, FAILED, BLOCKED, SKIPPED, UNTESTABLE];

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
        "ready" => READY.into(),
        "running" | "in_progress" | "in-progress" => RUNNING.into(),
        "untestable" | "not_runnable" | "notrun" => UNTESTABLE.into(),
        "retest" => RETEST.into(),
        // Already-canonical or unknown — title-case known set, else pass through trimmed.
        other => canonical_or_passthrough(other, raw.trim()),
    }
}

/// Map any result status to a dashboard bucket (Ready absorbs Untested/Retest).
pub fn to_dashboard_status(raw: &str) -> &'static str {
    match normalize_result_status(raw).as_str() {
        PASSED => PASSED,
        FAILED => FAILED,
        BLOCKED => BLOCKED,
        SKIPPED => SKIPPED,
        UNTESTABLE => UNTESTABLE,
        RUNNING => RUNNING,
        READY | UNTESTED | RETEST => READY,
        _ => READY,
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
    Ready,
    Running,
    Passed,
    Failed,
    Blocked,
    Skipped,
    Untestable,
    Retest,
}

impl ResultStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Untested => UNTESTED,
            Self::Ready => READY,
            Self::Running => RUNNING,
            Self::Passed => PASSED,
            Self::Failed => FAILED,
            Self::Blocked => BLOCKED,
            Self::Skipped => SKIPPED,
            Self::Untestable => UNTESTABLE,
            Self::Retest => RETEST,
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match normalize_result_status(raw).as_str() {
            UNTESTED => Some(Self::Untested),
            READY => Some(Self::Ready),
            RUNNING => Some(Self::Running),
            PASSED => Some(Self::Passed),
            FAILED => Some(Self::Failed),
            BLOCKED => Some(Self::Blocked),
            SKIPPED => Some(Self::Skipped),
            UNTESTABLE => Some(Self::Untestable),
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
        assert_eq!(normalize_result_status("ready"), READY);
        assert_eq!(normalize_result_status("running"), RUNNING);
        assert_eq!(normalize_result_status("untestable"), UNTESTABLE);
        assert_eq!(to_dashboard_status("Untested"), READY);
        assert_eq!(to_dashboard_status("Retest"), READY);
    }
}

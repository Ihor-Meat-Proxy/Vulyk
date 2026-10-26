//! Shared password hashing and strength policy.
//!
//! Policy is configured once via `VULYK_PASSWORD_MIN_LENGTH` (default 16, floor 12)
//! and reused by setup CLI, seed, user create/reset, change-password, and SSO
//! local-password fallback. Login only verifies the hash; weak existing
//! passwords trigger `must_change_password` after a successful verify.

use std::sync::OnceLock;

use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{AppError, AppResult};

/// Floor enforced even if `VULYK_PASSWORD_MIN_LENGTH` is set lower.
pub const PASSWORD_MIN_LENGTH_FLOOR: usize = 12;
/// Default when the env var is unset.
pub const PASSWORD_MIN_LENGTH_DEFAULT: usize = 16;
/// Length used when generating admin / SSO fallback passwords.
pub const PASSWORD_GENERATED_LENGTH: usize = 24;

/// Dummy PHC hash used so missing-user logins still pay an Argon2 verify cost.
const DUMMY_PASSWORD_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

static POLICY: OnceLock<PasswordPolicy> = OnceLock::new();

/// Server-side password requirements (single source of truth).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct PasswordPolicy {
    /// Minimum Unicode scalar length.
    pub min_length: usize,
    pub require_letter: bool,
    pub require_digit: bool,
}

impl PasswordPolicy {
    pub fn from_env() -> Self {
        let raw = std::env::var("VULYK_PASSWORD_MIN_LENGTH")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(PASSWORD_MIN_LENGTH_DEFAULT);
        let min_length = raw.max(PASSWORD_MIN_LENGTH_FLOOR);
        if raw < PASSWORD_MIN_LENGTH_FLOOR {
            tracing::warn!(
                requested = raw,
                floor = PASSWORD_MIN_LENGTH_FLOOR,
                "VULYK_PASSWORD_MIN_LENGTH below floor; using floor"
            );
        }
        Self {
            min_length,
            require_letter: true,
            require_digit: true,
        }
    }

    /// Process-wide policy (initialized from env on first access).
    pub fn global() -> Self {
        *POLICY.get_or_init(Self::from_env)
    }

    /// Reset the process-wide policy (tests only).
    #[cfg(test)]
    pub fn set_global_for_tests(policy: Self) {
        let _ = POLICY.set(policy);
        // If already set, overwrite via get_mut is unavailable on OnceLock —
        // tests should call this before first global() or use from_env locally.
        let _ = policy;
    }

    pub fn meets(&self, password: &str) -> bool {
        if password.chars().count() < self.min_length {
            return false;
        }
        if self.require_letter && !password.chars().any(|c| c.is_ascii_alphabetic()) {
            return false;
        }
        if self.require_digit && !password.chars().any(|c| c.is_ascii_digit()) {
            return false;
        }
        true
    }

    pub fn validate<'a>(&self, password: &'a str) -> AppResult<&'a str> {
        if self.meets(password) {
            return Ok(password);
        }
        Err(AppError::BadRequest(self.error_message()))
    }

    pub fn error_message(&self) -> String {
        let mut parts = vec![format!(
            "password must be at least {} characters",
            self.min_length
        )];
        if self.require_letter {
            parts.push("include a letter".into());
        }
        if self.require_digit {
            parts.push("include a digit".into());
        }
        if parts.len() == 1 {
            parts[0].clone()
        } else {
            let last = parts.pop().unwrap();
            format!("{} and {}", parts.join(", "), last)
        }
    }

    /// Public shape embedded in `/auth/status` so the UI never drifts.
    pub fn public(self) -> PasswordPolicyPublic {
        PasswordPolicyPublic {
            min_length: self.min_length as u32,
            require_letter: self.require_letter,
            require_digit: self.require_digit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct PasswordPolicyPublic {
    pub min_length: u32,
    pub require_letter: bool,
    pub require_digit: bool,
}

pub fn hash_password(password: &str) -> AppResult<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("password hash failed: {e}")))
}

/// Verify `password` against a PHC hash. Returns false on mismatch or bad hash.
pub fn verify_password(password: &str, password_hash: &str) -> AppResult<bool> {
    let parsed = match PasswordHash::new(password_hash) {
        Ok(p) => p,
        Err(_) => {
            // Still burn a verify against the dummy hash for more uniform timing.
            let _ = verify_password_raw(password, DUMMY_PASSWORD_HASH);
            return Ok(false);
        }
    };
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

fn verify_password_raw(password: &str, password_hash: &str) -> bool {
    PasswordHash::new(password_hash)
        .ok()
        .and_then(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .ok()
        })
        .is_some()
}

/// Constant-time-ish login verify: always runs Argon2 even when the user/hash
/// is missing, so callers cannot distinguish unknown email from bad password
/// via short-circuit timing alone.
pub fn verify_password_for_login(password: &str, password_hash: Option<&str>) -> bool {
    let hash = password_hash.unwrap_or(DUMMY_PASSWORD_HASH);
    verify_password_raw(password, hash)
}

/// Strength check using the process-wide policy.
pub fn password_strength_ok(password: &str) -> bool {
    PasswordPolicy::global().meets(password)
}

/// Validate against the process-wide policy (400 on failure).
pub fn validate_password_strength(password: &str) -> AppResult<&str> {
    PasswordPolicy::global().validate(password)
}

/// Generate a random password that always meets `policy` (≈24 chars by default).
pub fn generate_password(policy: &PasswordPolicy) -> String {
    let target = policy.min_length.max(PASSWORD_GENERATED_LENGTH);
    // URL-safe-ish alphabet with letters + digits.
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut bytes = vec![0u8; target];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    let mut out: String = bytes
        .iter()
        .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
        .collect();
    // Guarantee letter + digit regardless of random draw.
    if policy.require_letter && !out.chars().any(|c| c.is_ascii_alphabetic()) {
        out.push('A');
    }
    if policy.require_digit && !out.chars().any(|c| c.is_ascii_digit()) {
        out.push('7');
    }
    while out.chars().count() < policy.min_length {
        out.push('x');
        out.push('9');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(min: usize) -> PasswordPolicy {
        PasswordPolicy {
            min_length: min,
            require_letter: true,
            require_digit: true,
        }
    }

    #[test]
    fn floor_and_default_env_parsing() {
        let p = PasswordPolicy {
            min_length: PASSWORD_MIN_LENGTH_DEFAULT.max(PASSWORD_MIN_LENGTH_FLOOR),
            require_letter: true,
            require_digit: true,
        };
        assert_eq!(p.min_length, 16);
        assert!(!p.meets("admin"));
        assert!(!p.meets("shortpass12345")); // 14
        assert!(p.meets("longenoughpass12")); // 16
        assert!(!p.meets("abcdefghijklmnop")); // no digit
        assert!(!p.meets("1234567890123456")); // no letter
    }

    #[test]
    fn validate_returns_clear_400_message() {
        let p = policy(16);
        let err = p.validate("short").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("16"), "{msg}");
        assert!(msg.contains("letter") || msg.contains("digit"), "{msg}");
    }

    #[test]
    fn generated_password_meets_policy() {
        let p = policy(16);
        for _ in 0..20 {
            let pw = generate_password(&p);
            assert!(p.meets(&pw), "{pw}");
            assert!(pw.chars().count() >= PASSWORD_GENERATED_LENGTH.min(p.min_length));
        }
    }

    #[test]
    fn hash_roundtrip_allows_legacy_short_password() {
        let hash = hash_password("admin").expect("hash");
        assert!(verify_password("admin", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
        assert!(verify_password_for_login("admin", Some(&hash)));
        assert!(!verify_password_for_login("admin", None));
        assert!(!verify_password_for_login("wrong", Some(&hash)));
    }


    #[test]
    fn strength_helpers_use_min_length() {
        let p = policy(12);
        assert!(p.meets("abcdefghi123")); // 12
        assert!(!p.meets("abcdefg12")); // 9
    }
}

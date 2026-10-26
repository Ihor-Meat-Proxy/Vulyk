//! In-memory login rate limiting with exponential backoff.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};

const MAX_TRACKED_KEYS: usize = 10_000;
const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
const BASE_LOCK_SECS: u64 = 2;
const MAX_LOCK_SECS: u64 = 15 * 60;
/// Failures before the first lockout is applied.
const LOCK_AFTER_FAILURES: u32 = 5;

#[derive(Debug, Default)]
struct AttemptState {
    failures: u32,
    window_started: Option<Instant>,
    locked_until: Option<Instant>,
}

/// Per-process login throttle keyed by normalized email (and optional IP).
#[derive(Clone, Default)]
pub struct LoginThrottle {
    inner: Arc<Mutex<HashMap<String, AttemptState>>>,
}

impl LoginThrottle {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(email: &str, client_ip: Option<&str>) -> String {
        match client_ip {
            Some(ip) if !ip.is_empty() => format!("{email}|{ip}"),
            _ => email.to_string(),
        }
    }

    /// Reject with 429 when the key is currently locked out.
    pub fn check(&self, email: &str, client_ip: Option<&str>) -> AppResult<()> {
        let key = Self::key(email, client_ip);
        let mut map = self
            .inner
            .lock()
            .map_err(|_| AppError::Internal("login throttle lock poisoned".into()))?;
        self.prune_locked(&mut map);
        if let Some(state) = map.get(&key) {
            if let Some(until) = state.locked_until {
                if Instant::now() < until {
                    let secs = until
                        .saturating_duration_since(Instant::now())
                        .as_secs()
                        .max(1);
                    return Err(AppError::TooManyRequests(format!(
                        "too many login attempts; try again in {secs}s"
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn record_failure(&self, email: &str, client_ip: Option<&str>) {
        let key = Self::key(email, client_ip);
        let Ok(mut map) = self.inner.lock() else {
            return;
        };
        self.prune_locked(&mut map);
        if map.len() >= MAX_TRACKED_KEYS && !map.contains_key(&key) {
            map.clear();
        }
        let state = map.entry(key).or_default();
        let now = Instant::now();
        match state.window_started {
            Some(start) if now.duration_since(start) <= FAILURE_WINDOW => {
                state.failures = state.failures.saturating_add(1);
            }
            _ => {
                state.window_started = Some(now);
                state.failures = 1;
            }
        }
        if state.failures >= LOCK_AFTER_FAILURES {
            let excess = state.failures - LOCK_AFTER_FAILURES;
            let lock_secs = BASE_LOCK_SECS
                .saturating_mul(2u64.saturating_pow(excess.min(16)))
                .min(MAX_LOCK_SECS);
            state.locked_until = Some(now + Duration::from_secs(lock_secs));
        }
    }

    pub fn record_success(&self, email: &str, client_ip: Option<&str>) {
        let key = Self::key(email, client_ip);
        if let Ok(mut map) = self.inner.lock() {
            map.remove(&key);
        }
    }

    fn prune_locked(&self, map: &mut HashMap<String, AttemptState>) {
        let now = Instant::now();
        map.retain(|_, state| {
            let locked = state
                .locked_until
                .map(|u| u > now)
                .unwrap_or(false);
            let in_window = state
                .window_started
                .map(|s| now.duration_since(s) <= FAILURE_WINDOW)
                .unwrap_or(false);
            locked || in_window
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_after_repeated_failures() {
        let t = LoginThrottle::new();
        let email = "lock@example.com";
        for _ in 0..LOCK_AFTER_FAILURES {
            assert!(t.check(email, None).is_ok());
            t.record_failure(email, None);
        }
        let err = t.check(email, None).unwrap_err();
        assert!(matches!(err, AppError::TooManyRequests(_)));
        t.record_success(email, None);
        assert!(t.check(email, None).is_ok());
    }
}

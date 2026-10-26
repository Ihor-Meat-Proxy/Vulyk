use rand::RngCore;

use crate::crypto::hash_token;

/// Opaque session/API secret hash (SHA-256 hex). Secrets are never stored plaintext.
pub fn hash_secret(secret: &str) -> String {
    hash_token(secret)
}

pub fn generate_session_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("vulyk_s_{}", hex::encode(bytes))
}

/// Returns `(full_token, prefix)` — full token shown once; only hash+prefix are stored.
pub fn generate_api_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let full = format!("vulyk_at_{}", hex::encode(bytes));
    let prefix = token_prefix(&full);
    (full, prefix)
}

/// Project-scoped CI reporting token (`vulyk_ci_…`).
/// Returns `(full_token, prefix)` — full token shown once; only hash+prefix are stored.
pub fn generate_ci_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let full = format!("vulyk_ci_{}", hex::encode(bytes));
    let prefix = token_prefix(&full);
    (full, prefix)
}

pub fn token_prefix(token: &str) -> String {
    token.chars().take(16).collect()
}

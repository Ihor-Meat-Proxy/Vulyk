//! Envelope encryption for integration / LLM secrets at rest.
//!
//! Key material comes from `VULYK_SECRETS_KEY` (32-byte value, base64 or hex).
//! When unset, a deterministic development key is used so local compose works;
//! production deployments must set a real key.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

const DEV_KEY_MATERIAL: &[u8] = b"vulyk-dev-secrets-key-not-for-prod";

#[derive(Clone)]
pub struct SecretsKey {
    key: [u8; 32],
}

impl SecretsKey {
    pub fn from_env() -> Self {
        match std::env::var("VULYK_SECRETS_KEY") {
            Ok(raw) if !raw.is_empty() => Self::parse(&raw),
            _ => {
                tracing::warn!(
                    "VULYK_SECRETS_KEY unset; using development key (do not use in production)"
                );
                Self {
                    key: hash32(DEV_KEY_MATERIAL),
                }
            }
        }
    }

    pub fn parse(raw: &str) -> Self {
        if let Ok(bytes) = B64.decode(raw) {
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                return Self { key };
            }
        }
        if let Ok(bytes) = hex::decode(raw) {
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                return Self { key };
            }
        }
        // Passphrase → SHA-256
        Self {
            key: hash32(raw.as_bytes()),
        }
    }

    pub fn encrypt(&self, plaintext: &str) -> AppResult<(Vec<u8>, Vec<u8>)> {
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| AppError::BadRequest(format!("crypto init: {e}")))?;
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| AppError::BadRequest(format!("encrypt failed: {e}")))?;
        Ok((ciphertext, nonce_bytes.to_vec()))
    }

    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> AppResult<String> {
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| AppError::BadRequest(format!("crypto init: {e}")))?;
        if nonce.len() != 12 {
            return Err(AppError::BadRequest("invalid nonce".into()));
        }
        let nonce = Nonce::from_slice(nonce);
        let plain = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AppError::BadRequest("decrypt failed".into()))?;
        String::from_utf8(plain).map_err(|_| AppError::BadRequest("invalid utf8 secret".into()))
    }
}

fn hash32(input: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let key = SecretsKey::parse("test-passphrase");
        let (ct, nonce) = key.encrypt("sk-secret").unwrap();
        assert_eq!(key.decrypt(&ct, &nonce).unwrap(), "sk-secret");
    }

    #[test]
    fn token_hash_stable() {
        assert_eq!(hash_token("abc"), hash_token("abc"));
        assert_ne!(hash_token("abc"), hash_token("abd"));
    }
}

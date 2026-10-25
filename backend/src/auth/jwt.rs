use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub is_system_admin: bool,
    pub exp: i64,
    pub iat: i64,
    /// Distinguishes session JWTs from opaque API tokens (always "user").
    pub typ: String,
}

pub fn issue_jwt(
    user_id: Uuid,
    email: &str,
    is_system_admin: bool,
    secret: &str,
    ttl_hours: i64,
) -> AppResult<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        is_system_admin,
        iat: now.timestamp(),
        exp: (now + Duration::hours(ttl_hours)).timestamp(),
        typ: "user".into(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("jwt encode failed: {e}")))
}

pub fn decode_jwt(token: &str, secret: &str) -> AppResult<Claims> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| AppError::Unauthorized("invalid or expired token".into()))?;
    Ok(data.claims)
}

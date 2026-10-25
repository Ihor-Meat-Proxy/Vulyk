//! Idempotent first-start seeding of admin / demo users and setup token.

use rand::RngCore;
use sqlx::PgPool;

use crate::auth::{hash_password, hash_secret};
use crate::error::AppResult;
use crate::settings::{self, InstanceSettings, RegistrationMode};

/// Run after migrations. Safe to call on every boot.
pub async fn run(pool: &PgPool) -> AppResult<()> {
    ensure_setup_token(pool).await?;
    seed_from_env(pool).await?;
    Ok(())
}

async fn ensure_setup_token(pool: &PgPool) -> AppResult<()> {
    let completed = settings::get_bool(pool, "setup_completed", false).await?;
    if completed {
        return Ok(());
    }

    // Headless env admin means setup will be marked complete — no token needed.
    if std::env::var("VULYK_ADMIN_PASSWORD")
        .ok()
        .filter(|p| !p.is_empty())
        .is_some()
    {
        return Ok(());
    }

    let existing: Option<(String,)> =
        sqlx::query_as("SELECT token_hash FROM setup_tokens WHERE consumed_at IS NULL LIMIT 1")
            .fetch_optional(pool)
            .await?;
    if existing.is_some() {
        tracing::warn!(
            "Instance setup incomplete. Use the one-time setup token printed on first boot \
             (or run `vulyk-api setup`) to finish configuration."
        );
        return Ok(());
    }

    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let raw = format!("vulyk_setup_{}", hex::encode(bytes));
    let token_hash = hash_secret(&raw);
    sqlx::query("INSERT INTO setup_tokens (token_hash) VALUES ($1)")
        .bind(&token_hash)
        .execute(pool)
        .await?;

    tracing::warn!("=================================================================");
    tracing::warn!(" Vulyk first-run setup token (one-time, protect this value):");
    tracing::warn!("   {raw}");
    tracing::warn!(" Complete setup via UI wizard, `vulyk-api setup`, or install.sh.");
    tracing::warn!("=================================================================");
    Ok(())
}

async fn seed_from_env(pool: &PgPool) -> AppResult<()> {
    let admin_email = std::env::var("VULYK_ADMIN_EMAIL")
        .unwrap_or_else(|_| "admin@vulyk.local".into())
        .trim()
        .to_lowercase();
    let admin_password = std::env::var("VULYK_ADMIN_PASSWORD").ok();
    let seed_demo = matches!(
        std::env::var("VULYK_SEED_DEMO")
            .unwrap_or_else(|_| "0".into())
            .to_lowercase()
            .as_str(),
        "1" | "true" | "yes"
    );

    // When admin password is provided via env, seed/skip wizard (headless/k8s/compose).
    if let Some(password) = admin_password.filter(|p| !p.is_empty()) {
        // Force change for the well-known default; custom env passwords skip the gate.
        let must_change = password == "admin"
            || matches!(
                std::env::var("VULYK_ADMIN_MUST_CHANGE")
                    .unwrap_or_default()
                    .to_lowercase()
                    .as_str(),
                "1" | "true" | "yes"
            );
        let created = upsert_user(
            pool,
            &admin_email,
            std::env::var("VULYK_ADMIN_DISPLAY_NAME")
                .unwrap_or_else(|_| "Administrator".into())
                .as_str(),
            &password,
            true,
            must_change,
        )
        .await?;

        let mut instance = settings::load_settings(pool).await?;
        if !instance.setup_completed {
            instance.setup_completed = true;
            if let Ok(name) = std::env::var("VULYK_INSTANCE_NAME") {
                if !name.is_empty() {
                    instance.instance_name = name;
                }
            }
            if let Ok(url) = std::env::var("VULYK_BASE_URL") {
                instance.base_url = url;
            }
            if let Ok(locale) = std::env::var("VULYK_DEFAULT_LOCALE") {
                if !locale.is_empty() {
                    instance.default_locale = locale;
                }
            }
            if let Ok(tz) = std::env::var("VULYK_DEFAULT_TIMEZONE") {
                if !tz.is_empty() {
                    instance.default_timezone = tz;
                }
            }
            if let Ok(mode) = std::env::var("VULYK_REGISTRATION_MODE") {
                if let Ok(parsed) = mode.parse::<RegistrationMode>() {
                    instance.registration_mode = parsed;
                }
            }
            settings::save_settings(pool, &instance).await?;
            // Consume any outstanding setup tokens.
            sqlx::query(
                "UPDATE setup_tokens SET consumed_at = NOW() WHERE consumed_at IS NULL",
            )
            .execute(pool)
            .await?;
        }

        if created {
            tracing::warn!(
                "Seeded system admin {admin_email}. Change the default password immediately \
                 (must_change_password is set)."
            );
        }
    }

    if seed_demo {
        let demo_email = std::env::var("VULYK_DEMO_EMAIL")
            .unwrap_or_else(|_| "user@vulyk.local".into())
            .trim()
            .to_lowercase();
        let demo_password =
            std::env::var("VULYK_DEMO_PASSWORD").unwrap_or_else(|_| "user".into());
        let created = upsert_user(
            pool,
            &demo_email,
            "Demo User",
            &demo_password,
            false,
            false,
        )
        .await?;
        if created {
            tracing::warn!(
                "Seeded demo user {demo_email} (intended role: Test Automation Engineer on \
                 project membership). Change the default password."
            );
        }
    }

    Ok(())
}

/// Returns true when a new user row was inserted.
async fn upsert_user(
    pool: &PgPool,
    email: &str,
    display_name: &str,
    password: &str,
    is_system_admin: bool,
    must_change_password: bool,
) -> AppResult<bool> {
    let existing: Option<(uuid::Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = $1")
            .bind(email)
            .fetch_optional(pool)
            .await?;
    if existing.is_some() {
        return Ok(false);
    }

    let password_hash = hash_password(password)?;
    sqlx::query(
        r#"INSERT INTO users (
               email, display_name, password_hash, is_system_admin, must_change_password
           ) VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(email)
    .bind(display_name)
    .bind(&password_hash)
    .bind(is_system_admin)
    .bind(must_change_password)
    .execute(pool)
    .await?;
    Ok(true)
}

/// Apply interactive / CLI setup completion (creates admin + marks setup done).
pub async fn complete_setup(
    pool: &PgPool,
    admin_email: &str,
    admin_password: &str,
    display_name: &str,
    settings: &InstanceSettings,
    setup_token: Option<&str>,
) -> AppResult<()> {
    let completed = settings::get_bool(pool, "setup_completed", false).await?;
    if completed {
        return Err(crate::error::AppError::Conflict(
            "setup already completed".into(),
        ));
    }

    if let Some(token) = setup_token {
        let hash = hash_secret(token);
        let row: Option<(uuid::Uuid,)> = sqlx::query_as(
            r#"SELECT id FROM setup_tokens
               WHERE token_hash = $1 AND consumed_at IS NULL"#,
        )
        .bind(&hash)
        .fetch_optional(pool)
        .await?;
        if row.is_none() {
            return Err(crate::error::AppError::Forbidden(
                "invalid or consumed setup token".into(),
            ));
        }
        sqlx::query("UPDATE setup_tokens SET consumed_at = NOW() WHERE token_hash = $1")
            .bind(&hash)
            .execute(pool)
            .await?;
    } else {
        // CLI path from trusted install may omit token when run locally.
        sqlx::query("UPDATE setup_tokens SET consumed_at = NOW() WHERE consumed_at IS NULL")
            .execute(pool)
            .await?;
    }

    let email = admin_email.trim().to_lowercase();
    upsert_user(pool, &email, display_name, admin_password, true, false).await?;

    let mut s = settings.clone();
    s.setup_completed = true;
    settings::save_settings(pool, &s).await?;
    Ok(())
}

/// Generate a random password of `len` bytes hex-encoded.
#[allow(dead_code)]
pub fn random_password(len: usize) -> String {
    let mut buf = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

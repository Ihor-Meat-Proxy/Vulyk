//! Instance-level system settings and registration mode.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::auth::ProjectRole;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationMode {
    Open,
    Invite,
    Disabled,
}

impl RegistrationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Invite => "invite",
            Self::Disabled => "disabled",
        }
    }
}

impl std::str::FromStr for RegistrationMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(Self::Open),
            "invite" => Ok(Self::Invite),
            "disabled" => Ok(Self::Disabled),
            other => Err(format!("unknown registration mode: {other}")),
        }
    }
}

impl std::fmt::Display for RegistrationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InstanceSettings {
    pub setup_completed: bool,
    pub instance_name: String,
    pub base_url: String,
    pub default_locale: String,
    pub default_timezone: String,
    /// Default appearance theme for users who have not chosen one.
    pub default_theme: String,
    pub registration_mode: RegistrationMode,
    pub default_role: ProjectRole,
    pub allowed_email_domains: Vec<String>,
}

impl Default for InstanceSettings {
    fn default() -> Self {
        Self {
            setup_completed: false,
            instance_name: "Vulyk".into(),
            base_url: String::new(),
            default_locale: "en".into(),
            default_timezone: "UTC".into(),
            default_theme: "system".into(),
            registration_mode: RegistrationMode::Disabled,
            default_role: ProjectRole::Viewer,
            allowed_email_domains: vec![],
        }
    }
}

pub async fn get_bool(pool: &PgPool, key: &str, default: bool) -> AppResult<bool> {
    let value: Option<Value> =
        sqlx::query_scalar("SELECT value FROM system_settings WHERE key = $1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(value.and_then(|v| v.as_bool()).unwrap_or(default))
}

pub async fn get_string(pool: &PgPool, key: &str, default: &str) -> AppResult<String> {
    let value: Option<Value> =
        sqlx::query_scalar("SELECT value FROM system_settings WHERE key = $1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(value
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| default.to_string()))
}

pub async fn set_value(pool: &PgPool, key: &str, value: Value) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO system_settings (key, value, updated_at)
           VALUES ($1, $2, NOW())
           ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()"#,
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn load_settings(pool: &PgPool) -> AppResult<InstanceSettings> {
    let mode = get_string(pool, "registration_mode", "disabled").await?;
    let role = get_string(pool, "default_role", "viewer").await?;
    let domains: Option<Value> =
        sqlx::query_scalar("SELECT value FROM system_settings WHERE key = 'allowed_email_domains'")
            .fetch_optional(pool)
            .await?;
    let allowed_email_domains = if let Some(Value::Array(arr)) = domains {
        arr.into_iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    } else {
        Vec::new()
    };

    let mut settings = InstanceSettings {
        setup_completed: get_bool(pool, "setup_completed", false).await?,
        instance_name: get_string(pool, "instance_name", "Vulyk").await?,
        base_url: get_string(pool, "base_url", "").await?,
        default_locale: get_string(pool, "default_locale", "en").await?,
        default_timezone: get_string(pool, "default_timezone", "UTC").await?,
        default_theme: get_string(pool, "default_theme", "system").await?,
        registration_mode: mode.parse().unwrap_or(RegistrationMode::Disabled),
        default_role: role.parse().unwrap_or(ProjectRole::Viewer),
        allowed_email_domains,
    };

    // Env overrides for registration mode (deploy-time).
    if let Ok(env_mode) = std::env::var("VULYK_REGISTRATION_MODE") {
        if let Ok(parsed) = env_mode.parse::<RegistrationMode>() {
            settings.registration_mode = parsed;
        }
    }

    Ok(settings)
}

pub async fn save_settings(pool: &PgPool, settings: &InstanceSettings) -> AppResult<()> {
    set_value(
        pool,
        "setup_completed",
        Value::Bool(settings.setup_completed),
    )
    .await?;
    set_value(
        pool,
        "instance_name",
        Value::String(settings.instance_name.clone()),
    )
    .await?;
    set_value(pool, "base_url", Value::String(settings.base_url.clone())).await?;
    set_value(
        pool,
        "default_locale",
        Value::String(settings.default_locale.clone()),
    )
    .await?;
    set_value(
        pool,
        "default_timezone",
        Value::String(settings.default_timezone.clone()),
    )
    .await?;
    set_value(
        pool,
        "default_theme",
        Value::String(settings.default_theme.clone()),
    )
    .await?;
    set_value(
        pool,
        "registration_mode",
        Value::String(settings.registration_mode.as_str().into()),
    )
    .await?;
    set_value(
        pool,
        "default_role",
        Value::String(settings.default_role.as_str().into()),
    )
    .await?;
    set_value(
        pool,
        "allowed_email_domains",
        Value::Array(
            settings
                .allowed_email_domains
                .iter()
                .map(|d| Value::String(d.clone()))
                .collect(),
        ),
    )
    .await?;
    Ok(())
}

pub fn email_domain_allowed(email: &str, allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return true;
    }
    let Some((_, domain)) = email.split_once('@') else {
        return false;
    };
    let domain = domain.to_lowercase();
    allowed
        .iter()
        .any(|d| d.trim().eq_ignore_ascii_case(&domain))
}

pub async fn require_registration_open(pool: &PgPool) -> AppResult<InstanceSettings> {
    let settings = load_settings(pool).await?;
    match settings.registration_mode {
        RegistrationMode::Open => Ok(settings),
        RegistrationMode::Invite => Err(AppError::Forbidden(
            "registration requires an invite".into(),
        )),
        RegistrationMode::Disabled => Err(AppError::Forbidden("registration is disabled".into())),
    }
}

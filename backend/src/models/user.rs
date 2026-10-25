use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::ProjectRole;
use crate::models::preferences::{UpdatePreferences, UserPreferences};
use crate::settings::RegistrationMode;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    #[serde(skip_serializing)]
    #[schema(value_type = Option<String>)]
    pub password_hash: Option<String>,
    pub is_system_admin: bool,
    #[serde(default)]
    pub must_change_password: bool,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub preferences: Value,
    pub disabled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn preferences_typed(&self) -> UserPreferences {
        UserPreferences::from_value(&self.preferences)
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UserPublic {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub is_system_admin: bool,
    pub must_change_password: bool,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub preferences: UserPreferences,
    pub disabled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        let preferences = UserPreferences::from_value(&u.preferences);
        Self {
            id: u.id,
            email: u.email,
            display_name: u.display_name,
            is_system_admin: u.is_system_admin,
            must_change_password: u.must_change_password,
            locale: u.locale,
            timezone: u.timezone,
            preferences,
            disabled_at: u.disabled_at,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BootstrapRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
    /// Required when registration_mode = invite.
    pub invite_token: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: Option<String>,
    pub new_password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMeRequest {
    pub display_name: Option<String>,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub preferences: Option<UpdatePreferences>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub token: String,
    pub token_type: String,
    pub expires_in_hours: i64,
    pub user: UserPublic,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthStatus {
    pub auth_required: bool,
    pub bootstrap_available: bool,
    pub setup_required: bool,
    pub registration_mode: RegistrationMode,
    pub default_locale: String,
    pub default_timezone: String,
    pub default_theme: String,
    pub instance_name: String,
    pub authenticated: bool,
    pub user: Option<UserPublic>,
    pub sso_providers: Vec<SsoProviderPublic>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SsoProviderPublic {
    pub id: Uuid,
    pub provider_key: String,
    pub display_name: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
    #[serde(default)]
    pub is_system_admin: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub password: Option<String>,
    pub is_system_admin: Option<bool>,
    pub disabled: Option<bool>,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub preferences: Option<UpdatePreferences>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CompleteSetupRequest {
    pub setup_token: String,
    pub email: String,
    pub password: String,
    pub display_name: String,
    pub instance_name: Option<String>,
    pub base_url: Option<String>,
    pub default_locale: Option<String>,
    pub default_timezone: Option<String>,
    pub default_theme: Option<String>,
    pub registration_mode: Option<RegistrationMode>,
    /// Seed the Kyiv concert sample project (default true).
    #[serde(default = "default_true")]
    pub seed_sample: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SetupStatus {
    pub setup_completed: bool,
    pub setup_required: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MyPermissions {
    pub is_system_admin: bool,
    pub project_id: Option<Uuid>,
    pub role: Option<ProjectRole>,
    pub permissions: Vec<&'static str>,
}

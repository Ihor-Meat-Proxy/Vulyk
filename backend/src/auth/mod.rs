//! Authentication helpers: passwords, JWT/session tokens, API tokens, RBAC.
//!
//! Compatible with W9/W10 API-token middleware: Bearer may be a JWT, session
//! secret (`vulyk_s_…`), or personal API token (`vulyk_at_…` / legacy `vulk_…`).

mod extractor;
mod jwt;
mod login_throttle;
mod password;
mod rbac;
mod tokens;

pub use extractor::{
    bearer_middleware, password_change_allowed_path, project_role_for, require_auth_enabled,
    require_permission, require_project_role, session_cookie_name, AuthConfig, AuthContext,
    AuthUser, AuthVia, OptionalAuthUser, RequireAuth, RequireSystemAdmin,
};
pub use jwt::{issue_jwt, Claims};
pub use login_throttle::LoginThrottle;
pub use password::{
    generate_password, hash_password, password_strength_ok, validate_password_strength,
    verify_password, verify_password_for_login, PasswordPolicy, PasswordPolicyPublic,
    PASSWORD_GENERATED_LENGTH, PASSWORD_MIN_LENGTH_DEFAULT, PASSWORD_MIN_LENGTH_FLOOR,
};
pub use rbac::{role_matrix, Permission, ProjectRole, RoleMatrixRow, RoleRequirement};
pub use tokens::{
    generate_api_token, generate_ci_token, generate_session_token, hash_secret, token_prefix,
};

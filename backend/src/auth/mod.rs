//! Authentication helpers: passwords, JWT/session tokens, API tokens, RBAC.
//!
//! Compatible with W9/W10 API-token middleware: Bearer may be a JWT, session
//! secret (`vulyk_s_…`), or personal API token (`vulyk_at_…` / legacy `vulk_…`).

mod extractor;
mod jwt;
mod password;
mod rbac;
mod tokens;

pub use extractor::{
    bearer_middleware, project_role_for, require_auth_enabled, require_permission,
    require_project_role, session_cookie_name, AuthConfig, AuthContext, AuthUser, OptionalAuthUser,
    RequireAuth, RequireSystemAdmin,
};
pub use jwt::{issue_jwt, Claims};
pub use password::{
    hash_password, password_strength_ok, validate_password_strength, verify_password,
};
pub use rbac::{role_matrix, Permission, ProjectRole, RoleMatrixRow, RoleRequirement};
pub use tokens::{generate_api_token, generate_session_token, hash_secret, token_prefix};

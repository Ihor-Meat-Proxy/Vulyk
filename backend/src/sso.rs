//! OAuth2 / OIDC SSO providers: load, encrypt secrets, authorize, callback, JIT.

use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD};
use base64::Engine;
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{hash_password, ProjectRole};
use crate::crypto::SecretsKey;
use crate::error::{AppError, AppResult};
use crate::models::{SsoProviderPublic, User};
use crate::settings::{self, RegistrationMode};

const USER_COLUMNS: &str = r#"id, email, display_name, password_hash, is_system_admin,
                  must_change_password, locale, timezone,
                  disabled_at, created_at, updated_at"#;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct SsoProvider {
    pub id: Uuid,
    pub provider_key: String,
    pub display_name: String,
    pub enabled: bool,
    pub issuer_url: Option<String>,
    pub authorization_endpoint: Option<String>,
    pub token_endpoint: Option<String>,
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: Option<String>,
    pub client_id: String,
    #[serde(skip_serializing)]
    #[schema(value_type = Option<String>)]
    pub client_secret_ciphertext: Option<Vec<u8>>,
    #[serde(skip_serializing)]
    #[schema(value_type = Option<String>)]
    pub client_secret_nonce: Option<Vec<u8>>,
    pub scopes: String,
    pub extra_config: Value,
    pub default_role: ProjectRole,
    pub group_role_mapping: Value,
    pub allowed_email_domains: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SsoProvider {
    pub fn to_public(&self) -> SsoProviderPublic {
        SsoProviderPublic {
            id: self.id,
            provider_key: self.provider_key.clone(),
            display_name: self.display_name.clone(),
            enabled: self.enabled,
        }
    }

    pub fn has_client_secret(&self) -> bool {
        self.client_secret_ciphertext
            .as_ref()
            .is_some_and(|c| !c.is_empty())
    }

    pub fn decrypt_secret(&self, secrets: &SecretsKey) -> AppResult<Option<String>> {
        match (
            self.client_secret_ciphertext.as_ref(),
            self.client_secret_nonce.as_ref(),
        ) {
            (Some(ct), Some(nonce)) if !ct.is_empty() => Ok(Some(secrets.decrypt(ct, nonce)?)),
            _ => Ok(None),
        }
    }

    fn extra_str(&self, key: &str) -> Option<String> {
        self.extra_config
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SsoProviderAdmin {
    pub id: Uuid,
    pub provider_key: String,
    pub display_name: String,
    pub enabled: bool,
    pub issuer_url: Option<String>,
    pub authorization_endpoint: Option<String>,
    pub token_endpoint: Option<String>,
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: Option<String>,
    pub client_id: String,
    pub has_client_secret: bool,
    pub scopes: String,
    pub extra_config: Value,
    pub default_role: ProjectRole,
    pub group_role_mapping: Value,
    pub allowed_email_domains: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&SsoProvider> for SsoProviderAdmin {
    fn from(p: &SsoProvider) -> Self {
        Self {
            id: p.id,
            provider_key: p.provider_key.clone(),
            display_name: p.display_name.clone(),
            enabled: p.enabled,
            issuer_url: p.issuer_url.clone(),
            authorization_endpoint: p.authorization_endpoint.clone(),
            token_endpoint: p.token_endpoint.clone(),
            userinfo_endpoint: p.userinfo_endpoint.clone(),
            jwks_uri: p.jwks_uri.clone(),
            client_id: p.client_id.clone(),
            has_client_secret: p.has_client_secret(),
            scopes: p.scopes.clone(),
            extra_config: p.extra_config.clone(),
            default_role: p.default_role,
            group_role_mapping: p.group_role_mapping.clone(),
            allowed_email_domains: p.allowed_email_domains.clone(),
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSsoProviderRequest {
    pub display_name: Option<String>,
    pub enabled: Option<bool>,
    pub issuer_url: Option<String>,
    pub authorization_endpoint: Option<String>,
    pub token_endpoint: Option<String>,
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: Option<String>,
    pub client_id: Option<String>,
    /// When set, encrypt and store; omit to leave unchanged; empty string clears.
    pub client_secret: Option<String>,
    pub scopes: Option<String>,
    pub extra_config: Option<Value>,
    pub default_role: Option<ProjectRole>,
    pub group_role_mapping: Option<Value>,
    pub allowed_email_domains: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct ResolvedEndpoints {
    pub authorization: String,
    pub token: String,
    pub userinfo: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SsoIdentity {
    pub subject: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub display_name: Option<String>,
    pub groups: Vec<String>,
    pub raw: Value,
}

/// Apply `VULYK_SSO_<KEY>_*` env overrides into the DB (idempotent).
pub async fn sync_from_env(pool: &PgPool, secrets: &SecretsKey) -> AppResult<()> {
    let keys = [
        "google",
        "microsoft",
        "github",
        "gitlab",
        "nextcloud",
        "oidc",
    ];
    for key in keys {
        let prefix = format!("VULYK_SSO_{}", key.to_uppercase());
        let client_id = std::env::var(format!("{prefix}_CLIENT_ID")).ok();
        let client_secret = std::env::var(format!("{prefix}_CLIENT_SECRET")).ok();
        let enabled = std::env::var(format!("{prefix}_ENABLED"))
            .ok()
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"));
        let issuer = std::env::var(format!("{prefix}_ISSUER")).ok();
        let base_url = std::env::var(format!("{prefix}_BASE_URL")).ok();
        let tenant = std::env::var(format!("{prefix}_TENANT")).ok();

        if client_id.is_none()
            && client_secret.is_none()
            && enabled.is_none()
            && issuer.is_none()
            && base_url.is_none()
            && tenant.is_none()
        {
            continue;
        }

        let mut provider = load_provider_by_key(pool, key)
            .await?
            .ok_or_else(|| AppError::Internal(format!("missing seeded SSO provider: {key}")))?;

        if let Some(id) = client_id.filter(|s| !s.is_empty()) {
            provider.client_id = id;
        }
        if let Some(sec) = client_secret.filter(|s| !s.is_empty()) {
            let (ct, nonce) = secrets.encrypt(&sec)?;
            provider.client_secret_ciphertext = Some(ct);
            provider.client_secret_nonce = Some(nonce);
        }
        if let Some(e) = enabled {
            provider.enabled = e;
        }
        if let Some(iss) = issuer.filter(|s| !s.is_empty()) {
            provider.issuer_url = Some(iss);
        }

        let mut extra = provider.extra_config.clone();
        if !extra.is_object() {
            extra = json!({});
        }
        if let Some(obj) = extra.as_object_mut() {
            if let Some(bu) = base_url.filter(|s| !s.is_empty()) {
                obj.insert("base_url".into(), Value::String(bu));
            }
            if let Some(t) = tenant.filter(|s| !s.is_empty()) {
                obj.insert("tenant".into(), Value::String(t));
            }
        }
        provider.extra_config = extra;

        sqlx::query(
            r#"UPDATE sso_providers SET
                 client_id = $2,
                 client_secret_ciphertext = $3,
                 client_secret_nonce = $4,
                 enabled = $5,
                 issuer_url = $6,
                 extra_config = $7,
                 updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(provider.id)
        .bind(&provider.client_id)
        .bind(&provider.client_secret_ciphertext)
        .bind(&provider.client_secret_nonce)
        .bind(provider.enabled)
        .bind(&provider.issuer_url)
        .bind(&provider.extra_config)
        .execute(pool)
        .await?;

        tracing::info!(
            provider = key,
            enabled = provider.enabled,
            "SSO env sync applied"
        );
    }
    Ok(())
}

pub async fn list_providers(pool: &PgPool) -> AppResult<Vec<SsoProvider>> {
    let rows = sqlx::query_as::<_, SsoProvider>(
        r#"SELECT id, provider_key, display_name, enabled, issuer_url,
                  authorization_endpoint, token_endpoint, userinfo_endpoint, jwks_uri,
                  client_id, client_secret_ciphertext, client_secret_nonce, scopes,
                  extra_config, default_role, group_role_mapping, allowed_email_domains,
                  created_at, updated_at
           FROM sso_providers
           ORDER BY display_name"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_enabled_public(pool: &PgPool) -> AppResult<Vec<SsoProviderPublic>> {
    let rows = list_providers(pool).await?;
    Ok(rows
        .into_iter()
        .filter(|p| p.enabled && !p.client_id.is_empty())
        .map(|p| p.to_public())
        .collect())
}

pub async fn load_provider_by_key(pool: &PgPool, key: &str) -> AppResult<Option<SsoProvider>> {
    let row = sqlx::query_as::<_, SsoProvider>(
        r#"SELECT id, provider_key, display_name, enabled, issuer_url,
                  authorization_endpoint, token_endpoint, userinfo_endpoint, jwks_uri,
                  client_id, client_secret_ciphertext, client_secret_nonce, scopes,
                  extra_config, default_role, group_role_mapping, allowed_email_domains,
                  created_at, updated_at
           FROM sso_providers WHERE provider_key = $1"#,
    )
    .bind(key)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn load_provider_by_id(pool: &PgPool, id: Uuid) -> AppResult<SsoProvider> {
    sqlx::query_as::<_, SsoProvider>(
        r#"SELECT id, provider_key, display_name, enabled, issuer_url,
                  authorization_endpoint, token_endpoint, userinfo_endpoint, jwks_uri,
                  client_id, client_secret_ciphertext, client_secret_nonce, scopes,
                  extra_config, default_role, group_role_mapping, allowed_email_domains,
                  created_at, updated_at
           FROM sso_providers WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("sso provider".into()))
}

pub async fn update_provider(
    pool: &PgPool,
    secrets: &SecretsKey,
    id: Uuid,
    body: UpdateSsoProviderRequest,
) -> AppResult<SsoProvider> {
    let mut p = load_provider_by_id(pool, id).await?;

    if let Some(v) = body.display_name.filter(|s| !s.trim().is_empty()) {
        p.display_name = v;
    }
    if let Some(v) = body.enabled {
        p.enabled = v;
    }
    if let Some(v) = body.issuer_url {
        p.issuer_url = empty_to_none(v);
    }
    if let Some(v) = body.authorization_endpoint {
        p.authorization_endpoint = empty_to_none(v);
    }
    if let Some(v) = body.token_endpoint {
        p.token_endpoint = empty_to_none(v);
    }
    if let Some(v) = body.userinfo_endpoint {
        p.userinfo_endpoint = empty_to_none(v);
    }
    if let Some(v) = body.jwks_uri {
        p.jwks_uri = empty_to_none(v);
    }
    if let Some(v) = body.client_id {
        p.client_id = v;
    }
    if let Some(secret) = body.client_secret {
        if secret.is_empty() {
            p.client_secret_ciphertext = None;
            p.client_secret_nonce = None;
        } else {
            let (ct, nonce) = secrets.encrypt(&secret)?;
            p.client_secret_ciphertext = Some(ct);
            p.client_secret_nonce = Some(nonce);
        }
    }
    if let Some(v) = body.scopes.filter(|s| !s.is_empty()) {
        p.scopes = v;
    }
    if let Some(v) = body.extra_config {
        p.extra_config = v;
    }
    if let Some(v) = body.default_role {
        p.default_role = v;
    }
    if let Some(v) = body.group_role_mapping {
        p.group_role_mapping = v;
    }
    if let Some(v) = body.allowed_email_domains {
        p.allowed_email_domains = v;
    }

    sqlx::query_as::<_, SsoProvider>(
        r#"UPDATE sso_providers SET
             display_name = $2,
             enabled = $3,
             issuer_url = $4,
             authorization_endpoint = $5,
             token_endpoint = $6,
             userinfo_endpoint = $7,
             jwks_uri = $8,
             client_id = $9,
             client_secret_ciphertext = $10,
             client_secret_nonce = $11,
             scopes = $12,
             extra_config = $13,
             default_role = $14,
             group_role_mapping = $15,
             allowed_email_domains = $16,
             updated_at = NOW()
           WHERE id = $1
           RETURNING id, provider_key, display_name, enabled, issuer_url,
                     authorization_endpoint, token_endpoint, userinfo_endpoint, jwks_uri,
                     client_id, client_secret_ciphertext, client_secret_nonce, scopes,
                     extra_config, default_role, group_role_mapping, allowed_email_domains,
                     created_at, updated_at"#,
    )
    .bind(p.id)
    .bind(&p.display_name)
    .bind(p.enabled)
    .bind(&p.issuer_url)
    .bind(&p.authorization_endpoint)
    .bind(&p.token_endpoint)
    .bind(&p.userinfo_endpoint)
    .bind(&p.jwks_uri)
    .bind(&p.client_id)
    .bind(&p.client_secret_ciphertext)
    .bind(&p.client_secret_nonce)
    .bind(&p.scopes)
    .bind(&p.extra_config)
    .bind(p.default_role)
    .bind(&p.group_role_mapping)
    .bind(&p.allowed_email_domains)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

fn empty_to_none(s: String) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

pub async fn resolve_endpoints(provider: &SsoProvider) -> AppResult<ResolvedEndpoints> {
    if let (Some(auth), Some(token)) = (
        provider.authorization_endpoint.as_ref(),
        provider.token_endpoint.as_ref(),
    ) {
        if !auth.is_empty() && !token.is_empty() {
            return Ok(ResolvedEndpoints {
                authorization: auth.clone(),
                token: token.clone(),
                userinfo: provider.userinfo_endpoint.clone(),
            });
        }
    }

    match provider.provider_key.as_str() {
        "google" => {
            let issuer = provider
                .issuer_url
                .clone()
                .unwrap_or_else(|| "https://accounts.google.com".into());
            discover_oidc(&issuer).await
        }
        "microsoft" => {
            let tenant = provider
                .extra_str("tenant")
                .unwrap_or_else(|| "common".into());
            let issuer = provider
                .issuer_url
                .clone()
                .unwrap_or_else(|| format!("https://login.microsoftonline.com/{tenant}/v2.0"));
            discover_oidc(&issuer).await
        }
        "github" => Ok(ResolvedEndpoints {
            authorization: "https://github.com/login/oauth/authorize".into(),
            token: "https://github.com/login/oauth/access_token".into(),
            userinfo: Some("https://api.github.com/user".into()),
        }),
        "gitlab" => {
            let base = provider
                .extra_str("base_url")
                .unwrap_or_else(|| "https://gitlab.com".into())
                .trim_end_matches('/')
                .to_string();
            if let Some(issuer) = provider.issuer_url.as_ref().filter(|s| !s.is_empty()) {
                return discover_oidc(issuer).await;
            }
            Ok(ResolvedEndpoints {
                authorization: format!("{base}/oauth/authorize"),
                token: format!("{base}/oauth/token"),
                userinfo: Some(format!("{base}/oauth/userinfo")),
            })
        }
        "nextcloud" => {
            let base = provider
                .extra_str("base_url")
                .ok_or_else(|| AppError::BadRequest("nextcloud base_url required".into()))?
                .trim_end_matches('/')
                .to_string();
            if let Some(issuer) = provider.issuer_url.as_ref().filter(|s| !s.is_empty()) {
                return discover_oidc(issuer).await;
            }
            Ok(ResolvedEndpoints {
                authorization: format!("{base}/index.php/apps/oauth2/authorize"),
                token: format!("{base}/index.php/apps/oauth2/api/v1/token"),
                userinfo: Some(format!("{base}/ocs/v2.php/cloud/user")),
            })
        }
        _ => {
            let issuer = provider
                .issuer_url
                .as_ref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::BadRequest("issuer_url required for OIDC".into()))?;
            discover_oidc(issuer).await
        }
    }
}

async fn discover_oidc(issuer: &str) -> AppResult<ResolvedEndpoints> {
    let issuer = issuer.trim_end_matches('/');
    let url = format!("{issuer}/.well-known/openid-configuration");
    let client = reqwest::Client::new();
    let doc: Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("OIDC discovery failed: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::BadRequest(format!("OIDC discovery HTTP error: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("OIDC discovery JSON: {e}")))?;

    let authorization = doc
        .get("authorization_endpoint")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("missing authorization_endpoint".into()))?
        .to_string();
    let token = doc
        .get("token_endpoint")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("missing token_endpoint".into()))?
        .to_string();
    let userinfo = doc
        .get("userinfo_endpoint")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(ResolvedEndpoints {
        authorization,
        token,
        userinfo,
    })
}

fn random_urlsafe(len: usize) -> String {
    let mut buf = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

pub struct AuthStart {
    pub authorize_url: String,
    pub state: String,
}

/// Begin OAuth authorization; stores state + PKCE verifier.
pub async fn start_authorization(
    pool: &PgPool,
    provider: &SsoProvider,
    redirect_uri: &str,
    redirect_to: Option<&str>,
) -> AppResult<AuthStart> {
    if !provider.enabled {
        return Err(AppError::BadRequest("SSO provider is disabled".into()));
    }
    if provider.client_id.is_empty() {
        return Err(AppError::BadRequest(
            "SSO provider client_id not configured".into(),
        ));
    }

    let endpoints = resolve_endpoints(provider).await?;
    let state = random_urlsafe(32);
    let code_verifier = random_urlsafe(48);
    let challenge = pkce_challenge(&code_verifier);
    let nonce = random_urlsafe(24);
    let expires_at = Utc::now() + chrono::Duration::minutes(15);

    sqlx::query(
        r#"INSERT INTO oauth_states (state, provider_id, nonce, redirect_to, code_verifier, expires_at)
           VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(&state)
    .bind(provider.id)
    .bind(&nonce)
    .bind(redirect_to)
    .bind(&code_verifier)
    .bind(expires_at)
    .execute(pool)
    .await?;

    let mut url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        endpoints.authorization,
        urlencoding::encode(&provider.client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(&provider.scopes),
        urlencoding::encode(&state),
        urlencoding::encode(&challenge),
    );
    if provider.provider_key != "github" {
        url.push_str(&format!("&nonce={}", urlencoding::encode(&nonce)));
    }

    Ok(AuthStart {
        authorize_url: url,
        state,
    })
}

#[derive(Debug)]
struct OAuthStateRow {
    provider_id: Uuid,
    redirect_to: Option<String>,
    code_verifier: Option<String>,
}

/// Exchange code, resolve identity, link or JIT-provision user.
pub async fn handle_callback(
    pool: &PgPool,
    secrets: &SecretsKey,
    code: &str,
    state: &str,
    redirect_uri: &str,
) -> AppResult<(User, Option<String>)> {
    let row = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>)>(
        r#"SELECT provider_id, redirect_to, code_verifier
           FROM oauth_states
           WHERE state = $1 AND expires_at > NOW()"#,
    )
    .bind(state)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("invalid or expired OAuth state".into()))?;

    let oauth = OAuthStateRow {
        provider_id: row.0,
        redirect_to: row.1,
        code_verifier: row.2,
    };

    sqlx::query("DELETE FROM oauth_states WHERE state = $1")
        .bind(state)
        .execute(pool)
        .await?;

    let provider = load_provider_by_id(pool, oauth.provider_id).await?;
    if !provider.enabled {
        return Err(AppError::Forbidden("SSO provider disabled".into()));
    }

    let endpoints = resolve_endpoints(&provider).await?;
    let client_secret = provider.decrypt_secret(secrets)?;
    let token_json = exchange_code(
        &provider,
        &endpoints,
        code,
        redirect_uri,
        client_secret.as_deref(),
        oauth.code_verifier.as_deref(),
    )
    .await?;

    let access_token = token_json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Unauthorized("token response missing access_token".into()))?;

    let identity = fetch_identity(&provider, &endpoints, access_token, &token_json).await?;
    let user = link_or_provision(pool, &provider, &identity).await?;
    Ok((user, oauth.redirect_to))
}

async fn exchange_code(
    provider: &SsoProvider,
    endpoints: &ResolvedEndpoints,
    code: &str,
    redirect_uri: &str,
    client_secret: Option<&str>,
    code_verifier: Option<&str>,
) -> AppResult<Value> {
    let client = reqwest::Client::new();
    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code.to_string()),
        ("redirect_uri", redirect_uri.to_string()),
        ("client_id", provider.client_id.clone()),
    ];
    if let Some(secret) = client_secret {
        form.push(("client_secret", secret.to_string()));
    }
    if let Some(verifier) = code_verifier {
        form.push(("code_verifier", verifier.to_string()));
    }

    let mut req = client.post(&endpoints.token).form(&form);
    if provider.provider_key == "github" {
        req = req.header("Accept", "application/json");
    }

    let resp = req
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("token exchange failed: {e}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Unauthorized(format!(
            "token exchange HTTP {status}: {body}"
        )));
    }
    resp.json()
        .await
        .map_err(|e| AppError::BadRequest(format!("token JSON: {e}")))
}

async fn fetch_identity(
    provider: &SsoProvider,
    endpoints: &ResolvedEndpoints,
    access_token: &str,
    token_json: &Value,
) -> AppResult<SsoIdentity> {
    // Prefer id_token claims when present (OIDC).
    if let Some(id_token) = token_json.get("id_token").and_then(|v| v.as_str()) {
        if let Ok(claims) = decode_jwt_payload(id_token) {
            let subject = claims
                .get("sub")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !subject.is_empty() {
                let email = claims
                    .get("email")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let email_verified = claims
                    .get("email_verified")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(email.is_some());
                let display_name = claims
                    .get("name")
                    .or_else(|| claims.get("preferred_username"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let groups = extract_groups(&claims);
                if email.is_some() || endpoints.userinfo.is_none() {
                    return Ok(SsoIdentity {
                        subject,
                        email,
                        email_verified,
                        display_name,
                        groups,
                        raw: claims,
                    });
                }
            }
        }
    }

    match provider.provider_key.as_str() {
        "github" => fetch_github_identity(access_token).await,
        _ => {
            let userinfo = endpoints
                .userinfo
                .as_ref()
                .ok_or_else(|| AppError::BadRequest("no userinfo endpoint".into()))?;
            let client = reqwest::Client::new();
            let mut req = client
                .get(userinfo)
                .bearer_auth(access_token)
                .header("Accept", "application/json");
            if provider.provider_key == "nextcloud" {
                req = req.header("OCS-APIRequest", "true");
            }
            let claims: Value = req
                .send()
                .await
                .map_err(|e| AppError::BadRequest(format!("userinfo failed: {e}")))?
                .error_for_status()
                .map_err(|e| AppError::Unauthorized(format!("userinfo HTTP: {e}")))?
                .json()
                .await
                .map_err(|e| AppError::BadRequest(format!("userinfo JSON: {e}")))?;

            parse_userinfo_claims(&provider.provider_key, claims)
        }
    }
}

fn parse_userinfo_claims(provider_key: &str, claims: Value) -> AppResult<SsoIdentity> {
    if provider_key == "nextcloud" {
        let data = claims
            .pointer("/ocs/data")
            .cloned()
            .unwrap_or(claims.clone());
        let subject = data
            .get("id")
            .or_else(|| data.get("uid"))
            .and_then(|v| {
                v.as_str()
                    .map(str::to_string)
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            })
            .ok_or_else(|| AppError::Unauthorized("nextcloud user missing id".into()))?;
        let email = data
            .get("email")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let display_name = data
            .get("display-name")
            .or_else(|| data.get("displayname"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        return Ok(SsoIdentity {
            subject,
            email,
            email_verified: true,
            display_name,
            groups: extract_groups(&data),
            raw: claims,
        });
    }

    let subject = claims
        .get("sub")
        .or_else(|| claims.get("id"))
        .and_then(|v| {
            v.as_str()
                .map(str::to_string)
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        })
        .ok_or_else(|| AppError::Unauthorized("userinfo missing sub".into()))?;
    let email = claims
        .get("email")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let email_verified = claims
        .get("email_verified")
        .and_then(|v| v.as_bool())
        .unwrap_or(email.is_some());
    let display_name = claims
        .get("name")
        .or_else(|| claims.get("preferred_username"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(SsoIdentity {
        subject,
        email,
        email_verified,
        display_name,
        groups: extract_groups(&claims),
        raw: claims,
    })
}

async fn fetch_github_identity(access_token: &str) -> AppResult<SsoIdentity> {
    let client = reqwest::Client::new();
    let user: Value = client
        .get("https://api.github.com/user")
        .bearer_auth(access_token)
        .header("User-Agent", "vulyk-api")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("github user: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::Unauthorized(format!("github user HTTP: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("github user JSON: {e}")))?;

    let subject = user
        .get("id")
        .and_then(|v| {
            v.as_i64()
                .map(|n| n.to_string())
                .or_else(|| v.as_str().map(str::to_string))
        })
        .ok_or_else(|| AppError::Unauthorized("github user missing id".into()))?;

    let mut email = user
        .get("email")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let mut email_verified = email.is_some();

    if email.is_none() {
        let emails: Value = client
            .get("https://api.github.com/user/emails")
            .bearer_auth(access_token)
            .header("User-Agent", "vulyk-api")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("github emails: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Unauthorized(format!("github emails HTTP: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::BadRequest(format!("github emails JSON: {e}")))?;

        if let Some(arr) = emails.as_array() {
            let primary = arr.iter().find(|e| {
                e.get("primary").and_then(|v| v.as_bool()).unwrap_or(false)
                    && e.get("verified").and_then(|v| v.as_bool()).unwrap_or(false)
            });
            let verified = arr
                .iter()
                .find(|e| e.get("verified").and_then(|v| v.as_bool()).unwrap_or(false));
            let chosen = primary.or(verified).or(arr.first());
            if let Some(e) = chosen {
                email = e.get("email").and_then(|v| v.as_str()).map(str::to_string);
                email_verified = e.get("verified").and_then(|v| v.as_bool()).unwrap_or(false);
            }
        }
    }

    let display_name = user
        .get("name")
        .or_else(|| user.get("login"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(SsoIdentity {
        subject,
        email,
        email_verified,
        display_name,
        groups: vec![],
        raw: user,
    })
}

fn extract_groups(claims: &Value) -> Vec<String> {
    let mut groups = Vec::new();
    for key in ["groups", "roles", "group"] {
        if let Some(arr) = claims.get(key).and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(s) = item.as_str() {
                    groups.push(s.to_string());
                }
            }
        } else if let Some(s) = claims.get(key).and_then(|v| v.as_str()) {
            groups.push(s.to_string());
        }
    }
    groups
}

fn decode_jwt_payload(token: &str) -> AppResult<Value> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return Err(AppError::BadRequest("invalid id_token".into()));
    }
    let payload = parts[1];
    let padded = match payload.len() % 4 {
        2 => format!("{payload}=="),
        3 => format!("{payload}="),
        _ => payload.to_string(),
    };
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| B64.decode(&padded))
        .map_err(|_| AppError::BadRequest("id_token payload decode".into()))?;
    serde_json::from_slice(&bytes).map_err(|e| AppError::BadRequest(format!("id_token JSON: {e}")))
}

fn role_from_groups(provider: &SsoProvider, groups: &[String]) -> ProjectRole {
    if let Some(map) = provider.group_role_mapping.as_object() {
        for g in groups {
            if let Some(role_val) = map.get(g).and_then(|v| v.as_str()) {
                if let Ok(role) = role_val.parse::<ProjectRole>() {
                    return role;
                }
            }
        }
    }
    provider.default_role
}

async fn link_or_provision(
    pool: &PgPool,
    provider: &SsoProvider,
    identity: &SsoIdentity,
) -> AppResult<User> {
    // Existing linked identity.
    if let Some(user_id) = sqlx::query_scalar::<_, Uuid>(
        r#"SELECT user_id FROM user_identities
           WHERE provider_id = $1 AND subject = $2"#,
    )
    .bind(provider.id)
    .bind(&identity.subject)
    .fetch_optional(pool)
    .await?
    {
        return load_user(pool, user_id).await;
    }

    let email = identity
        .email
        .as_ref()
        .map(|e| e.trim().to_lowercase())
        .filter(|e| e.contains('@'));

    // Link by verified email.
    if let Some(ref email) = email {
        if identity.email_verified {
            if let Some(existing) = find_user_by_email(pool, email).await? {
                attach_identity(pool, existing.id, provider.id, identity).await?;
                return Ok(existing);
            }
        }
    }

    // Domain allow-list on provider.
    if let Some(ref email) = email {
        if !provider.allowed_email_domains.is_empty()
            && !settings::email_domain_allowed(email, &provider.allowed_email_domains)
        {
            return Err(AppError::Forbidden(
                "email domain not allowed for this SSO provider".into(),
            ));
        }
    }

    let instance = settings::load_settings(pool).await?;
    match instance.registration_mode {
        RegistrationMode::Open | RegistrationMode::Invite => {
            // Invite mode still allows SSO JIT when email domains / provider allow.
        }
        RegistrationMode::Disabled => {
            // Only existing users, unless instance/provider domains allow JIT.
            let domains_ok = if let Some(ref email) = email {
                settings::email_domain_allowed(email, &instance.allowed_email_domains)
                    || (!provider.allowed_email_domains.is_empty()
                        && settings::email_domain_allowed(email, &provider.allowed_email_domains))
            } else {
                false
            };
            if !domains_ok {
                return Err(AppError::Forbidden(
                    "registration disabled; sign in with an existing account or allowed domain"
                        .into(),
                ));
            }
        }
    }

    let email = email.ok_or_else(|| {
        AppError::Forbidden("SSO account has no email; cannot provision user".into())
    })?;
    if !identity.email_verified {
        return Err(AppError::Forbidden("SSO email is not verified".into()));
    }

    let display_name = identity
        .display_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| email.clone());

    // Random unusable password hash (SSO-only account).
    let random_pw = {
        let mut buf = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut buf);
        hex::encode(buf)
    };
    let password_hash = hash_password(&random_pw)?;
    // default_role / group_role_mapping applied when assigning project memberships later.
    let _assigned_role = role_from_groups(provider, &identity.groups);
    tracing::debug!(role = %_assigned_role, "SSO JIT default role resolved");

    let user = sqlx::query_as::<_, User>(&format!(
        r#"INSERT INTO users (
               email, display_name, password_hash, is_system_admin, must_change_password,
               locale, timezone
           ) VALUES ($1, $2, $3, FALSE, FALSE, $4, $5)
           RETURNING {USER_COLUMNS}"#
    ))
    .bind(&email)
    .bind(&display_name)
    .bind(&password_hash)
    .bind(&instance.default_locale)
    .bind(&instance.default_timezone)
    .fetch_one(pool)
    .await?;

    attach_identity(pool, user.id, provider.id, identity).await?;
    Ok(user)
}

async fn attach_identity(
    pool: &PgPool,
    user_id: Uuid,
    provider_id: Uuid,
    identity: &SsoIdentity,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_identities (user_id, provider_id, subject, email, raw_claims)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (provider_id, subject) DO UPDATE
             SET user_id = EXCLUDED.user_id,
                 email = EXCLUDED.email,
                 raw_claims = EXCLUDED.raw_claims,
                 updated_at = NOW()"#,
    )
    .bind(user_id)
    .bind(provider_id)
    .bind(&identity.subject)
    .bind(&identity.email)
    .bind(&identity.raw)
    .execute(pool)
    .await?;
    Ok(())
}

async fn find_user_by_email(pool: &PgPool, email: &str) -> AppResult<Option<User>> {
    let user = sqlx::query_as::<_, User>(&format!(
        "SELECT {USER_COLUMNS} FROM users WHERE email = $1"
    ))
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

async fn load_user(pool: &PgPool, id: Uuid) -> AppResult<User> {
    sqlx::query_as::<_, User>(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user not found".into()))
}

/// Build the OAuth redirect_uri for this API instance.
pub fn callback_redirect_uri(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    format!("{base}/api/v1/auth/sso/callback")
}

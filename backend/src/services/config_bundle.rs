use chrono::Utc;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    ConfigAutomationRepo, ConfigBundle, ConfigChangeItem, ConfigCustomField, ConfigDataSection,
    ConfigExportRequest, ConfigFolder, ConfigImportPreview, ConfigImportRequest,
    ConfigInstanceSection, ConfigIntegration, ConfigLlmProvider, ConfigMembership,
    ConfigOptionList, ConfigOptionListItem, ConfigProject, ConfigRolesSection, ConfigSsoProvider,
    CONFIG_BUNDLE_VERSION,
};
use crate::settings;

pub async fn export(pool: &PgPool, req: &ConfigExportRequest) -> AppResult<ConfigBundle> {
    let project_ids = resolve_project_ids(pool, req).await?;
    let settings = settings::load_settings(pool).await?;

    let option_lists = export_option_lists(pool).await?;
    let custom_fields = export_custom_fields(pool, &project_ids).await?;
    let projects = export_projects(pool, &project_ids, req.include_folder_structure).await?;

    let roles_memberships = if req.include_memberships {
        Some(export_memberships(pool, &project_ids).await?)
    } else {
        None
    };

    let llm_providers = export_llm(pool, &project_ids).await?;
    let integrations = export_integrations(pool, &project_ids).await?;
    let automation_repos = export_automation_repos(pool, &project_ids).await?;
    let sso_providers = export_sso(pool, req.secrets_passphrase.as_deref()).await?;

    let data = if req.include_data {
        Some(export_data(pool, &project_ids).await?)
    } else {
        None
    };

    Ok(ConfigBundle {
        version: CONFIG_BUNDLE_VERSION.into(),
        exported_at: Utc::now().to_rfc3339(),
        instance: ConfigInstanceSection {
            name: Some(settings.instance_name),
            default_locale: Some(settings.default_locale),
            default_timezone: Some(settings.default_timezone),
            registration_mode: Some(settings.registration_mode.as_str().into()),
            appearance_defaults: Some(json!({ "default_theme": settings.default_theme })),
            sso_providers,
        },
        option_lists,
        custom_fields,
        projects,
        roles_memberships,
        llm_providers,
        integrations,
        automation_repos,
        data,
    })
}

pub fn serialize_bundle(bundle: &ConfigBundle, format: &str) -> AppResult<String> {
    match format {
        "yaml" | "yml" => serde_yaml::to_string(bundle)
            .map_err(|e| AppError::Internal(format!("yaml encode: {e}"))),
        _ => serde_json::to_string_pretty(bundle)
            .map_err(|e| AppError::Internal(format!("json encode: {e}"))),
    }
}

pub fn parse_bundle(raw: &str) -> AppResult<ConfigBundle> {
    let trimmed = raw.trim_start();
    if trimmed.starts_with('{') {
        serde_json::from_str(raw).map_err(|e| AppError::BadRequest(format!("invalid JSON: {e}")))
    } else {
        serde_yaml::from_str(raw).map_err(|e| AppError::BadRequest(format!("invalid YAML: {e}")))
    }
}

pub async fn import(
    pool: &PgPool,
    req: ConfigImportRequest,
) -> AppResult<ConfigImportPreview> {
    let strategy = req.strategy.to_lowercase();
    if !matches!(strategy.as_str(), "merge" | "replace") {
        return Err(AppError::BadRequest("strategy must be merge or replace".into()));
    }

    let mut creates = Vec::new();
    let mut updates = Vec::new();
    let mut conflicts = Vec::new();
    let skipped = Vec::new();

    // Option lists
    for list in &req.bundle.option_lists {
        let existing = crate::services::option_lists::get_by_key(pool, &list.key).await?;
        match existing {
            None => {
                creates.push(change("option_list", &list.key, "create", "new list"));
                if !req.dry_run {
                    let items = list
                        .items
                        .iter()
                        .map(|i| crate::models::CreateOptionListItem {
                            label: i.label.clone(),
                            key: i.key.clone(),
                            value: i.value.clone(),
                            colour: i.colour.clone(),
                            description: i.description.clone(),
                            sort_order: i.sort_order,
                            active: i.active,
                            deprecated: i.deprecated,
                        })
                        .collect();
                    crate::services::option_lists::create(
                        pool,
                        crate::models::CreateOptionList {
                            key: list.key.clone(),
                            name: list.name.clone(),
                            description: list.description.clone(),
                            active: list.active,
                            items,
                        },
                    )
                    .await?;
                }
            }
            Some(ex) => {
                if strategy == "replace" || strategy == "merge" {
                    updates.push(change("option_list", &list.key, "update", "existing list"));
                    if !req.dry_run {
                        crate::services::option_lists::update(
                            pool,
                            ex.id,
                            crate::models::UpdateOptionList {
                                key: Some(list.key.clone()),
                                name: Some(list.name.clone()),
                                description: Some(list.description.clone()),
                                active: Some(list.active),
                            },
                        )
                        .await?;
                    }
                } else {
                    conflicts.push(change("option_list", &list.key, "conflict", "exists"));
                }
            }
        }
    }

    // Ensure projects exist (by key) before custom field scopes
    for project in &req.bundle.projects {
        let existing: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM projects WHERE key = $1")
                .bind(&project.key)
                .fetch_optional(pool)
                .await?;
        match existing {
            None => {
                creates.push(change("project", &project.key, "create", &project.name));
                if !req.dry_run {
                    let prefix = normalize_case_key_prefix(
                        project
                            .case_key_prefix
                            .as_deref()
                            .unwrap_or(&project.key),
                    );
                    sqlx::query(
                        r#"INSERT INTO projects (key, name, description, case_key_prefix)
                           VALUES ($1, $2, $3, $4)"#,
                    )
                    .bind(&project.key)
                    .bind(&project.name)
                    .bind(&project.description)
                    .bind(&prefix)
                    .execute(pool)
                    .await?;
                }
            }
            Some(_) => {
                updates.push(change("project", &project.key, "update", "existing project"));
                if !req.dry_run && strategy != "merge" {
                    sqlx::query(
                        r#"UPDATE projects SET name = $2, description = $3,
                           case_key_prefix = COALESCE($4, case_key_prefix)
                           WHERE key = $1"#,
                    )
                    .bind(&project.key)
                    .bind(&project.name)
                    .bind(&project.description)
                    .bind(&project.case_key_prefix)
                    .execute(pool)
                    .await?;
                } else if !req.dry_run {
                    sqlx::query(
                        r#"UPDATE projects SET name = $2, description = $3 WHERE key = $1"#,
                    )
                    .bind(&project.key)
                    .bind(&project.name)
                    .bind(&project.description)
                    .execute(pool)
                    .await?;
                }
            }
        }
    }

    // Custom fields
    for field in &req.bundle.custom_fields {
        let existing: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM custom_field_definitions WHERE key = $1",
        )
        .bind(&field.key)
        .fetch_optional(pool)
        .await?;

        let option_list_id = if let Some(list_key) = &field.option_list_key {
            crate::services::option_lists::get_by_key(pool, list_key)
                .await?
                .map(|l| l.id)
        } else {
            None
        };

        let project_ids = resolve_keys_to_ids(pool, &field.project_keys).await?;

        match existing {
            None => {
                creates.push(change("custom_field", &field.key, "create", &field.name));
                if !req.dry_run {
                    crate::services::custom_fields::create_definition(
                        pool,
                        crate::models::CreateCustomFieldDefinition {
                            key: field.key.clone(),
                            name: field.name.clone(),
                            field_type: field.field_type.clone(),
                            entity: field.entity.clone(),
                            option_list_id,
                            default_value: field.default_value.clone(),
                            required: field.required,
                            help_text: field.help_text.clone(),
                            sort_order: field.sort_order,
                            show_in_list: field.show_in_list,
                            show_in_filters: field.show_in_filters,
                            active: field.active,
                            scope: field.scope.clone(),
                            project_ids,
                        },
                    )
                    .await?;
                }
            }
            Some((id,)) => {
                updates.push(change("custom_field", &field.key, "update", "existing field"));
                if !req.dry_run {
                    crate::services::custom_fields::update_definition(
                        pool,
                        id,
                        crate::models::UpdateCustomFieldDefinition {
                            key: Some(field.key.clone()),
                            name: Some(field.name.clone()),
                            field_type: Some(field.field_type.clone()),
                            entity: Some(field.entity.clone()),
                            option_list_id: Some(option_list_id),
                            default_value: Some(field.default_value.clone()),
                            required: Some(field.required),
                            help_text: Some(field.help_text.clone()),
                            sort_order: Some(field.sort_order),
                            show_in_list: Some(field.show_in_list),
                            show_in_filters: Some(field.show_in_filters),
                            active: Some(field.active),
                            scope: Some(field.scope.clone()),
                            project_ids: Some(project_ids),
                            acknowledge_hide_values: true,
                        },
                    )
                    .await?;
                }
            }
        }
    }

    // Instance settings (merge non-secret)
    if let Some(name) = &req.bundle.instance.name {
        updates.push(change("instance", "name", "update", name));
        if !req.dry_run {
            settings::set_value(pool, "instance_name", Value::String(name.clone())).await?;
        }
    }
    if let Some(locale) = &req.bundle.instance.default_locale {
        updates.push(change("instance", "default_locale", "update", locale));
        if !req.dry_run {
            settings::set_value(pool, "default_locale", Value::String(locale.clone())).await?;
        }
    }
    if let Some(tz) = &req.bundle.instance.default_timezone {
        updates.push(change("instance", "default_timezone", "update", tz));
        if !req.dry_run {
            settings::set_value(pool, "default_timezone", Value::String(tz.clone())).await?;
        }
    }

    // Memberships (emails only)
    if let Some(roles) = &req.bundle.roles_memberships {
        for m in &roles.memberships {
            creates.push(change(
                "membership",
                &format!("{}:{}", m.project_key, m.email),
                "upsert",
                &m.role,
            ));
            if !req.dry_run {
                let project_id: Option<Uuid> =
                    sqlx::query_scalar("SELECT id FROM projects WHERE key = $1")
                        .bind(&m.project_key)
                        .fetch_optional(pool)
                        .await?;
                let user_id: Option<Uuid> =
                    sqlx::query_scalar("SELECT id FROM users WHERE lower(email) = lower($1)")
                        .bind(&m.email)
                        .fetch_optional(pool)
                        .await?;
                if let (Some(pid), Some(uid)) = (project_id, user_id) {
                    sqlx::query(
                        r#"INSERT INTO memberships (project_id, user_id, role)
                           VALUES ($1, $2, $3::project_role)
                           ON CONFLICT (project_id, user_id) DO UPDATE SET role = EXCLUDED.role"#,
                    )
                    .bind(pid)
                    .bind(uid)
                    .bind(&m.role)
                    .execute(pool)
                    .await?;
                }
            }
        }
    }

    Ok(ConfigImportPreview {
        dry_run: req.dry_run,
        strategy,
        creates,
        updates,
        conflicts,
        skipped,
        applied: !req.dry_run,
    })
}

fn normalize_case_key_prefix(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let mut prefix = cleaned;
    if prefix.is_empty() {
        prefix = "PR".into();
    }
    if !prefix.chars().next().unwrap().is_ascii_alphabetic() {
        prefix = format!("P{prefix}");
    }
    prefix.chars().take(8).collect()
}

fn change(kind: &str, key: &str, action: &str, detail: &str) -> ConfigChangeItem {
    ConfigChangeItem {
        kind: kind.into(),
        key: key.into(),
        action: action.into(),
        detail: detail.into(),
    }
}

async fn resolve_project_ids(pool: &PgPool, req: &ConfigExportRequest) -> AppResult<Vec<Uuid>> {
    if req.project_scope == "selected" {
        let mut ids = req.project_ids.clone();
        if !req.project_keys.is_empty() {
            let more: Vec<Uuid> = sqlx::query_scalar(
                "SELECT id FROM projects WHERE key = ANY($1)",
            )
            .bind(&req.project_keys)
            .fetch_all(pool)
            .await?;
            ids.extend(more);
        }
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            return Err(AppError::BadRequest(
                "selected project scope requires project_ids or project_keys".into(),
            ));
        }
        Ok(ids)
    } else {
        let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM projects ORDER BY key")
            .fetch_all(pool)
            .await?;
        Ok(ids)
    }
}

async fn resolve_keys_to_ids(pool: &PgPool, keys: &[String]) -> AppResult<Vec<Uuid>> {
    if keys.is_empty() {
        return Ok(vec![]);
    }
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM projects WHERE key = ANY($1)")
        .bind(keys)
        .fetch_all(pool)
        .await?;
    Ok(ids)
}

async fn export_option_lists(pool: &PgPool) -> AppResult<Vec<ConfigOptionList>> {
    let lists = crate::services::option_lists::list_all(pool).await?;
    let mut out = Vec::with_capacity(lists.len());
    for list in lists {
        let items = crate::services::option_lists::list_items(pool, list.id).await?;
        out.push(ConfigOptionList {
            key: list.key,
            name: list.name,
            description: list.description,
            active: list.active,
            items: items
                .into_iter()
                .map(|i| ConfigOptionListItem {
                    key: i.key,
                    label: i.label,
                    value: i.value,
                    colour: i.colour,
                    description: i.description,
                    sort_order: i.sort_order,
                    active: i.active,
                    deprecated: i.deprecated,
                })
                .collect(),
        });
    }
    Ok(out)
}

async fn export_custom_fields(
    pool: &PgPool,
    project_ids: &[Uuid],
) -> AppResult<Vec<ConfigCustomField>> {
    let defs = crate::services::custom_fields::list_definitions(pool).await?;
    let mut out = Vec::new();
    for detail in defs {
        let field = detail.field;
        // Include if scope=all OR intersects selected projects
        let include = field.scope == "all"
            || detail
                .project_ids
                .iter()
                .any(|p| project_ids.contains(p));
        if !include {
            continue;
        }
        let option_list_key = if let Some(lid) = field.option_list_id {
            sqlx::query_scalar::<_, String>("SELECT key FROM option_lists WHERE id = $1")
                .bind(lid)
                .fetch_optional(pool)
                .await?
        } else {
            None
        };
        let project_keys: Vec<String> = if field.scope == "selected" {
            sqlx::query_scalar(
                r#"SELECT p.key FROM projects p
                   JOIN custom_field_projects cfp ON cfp.project_id = p.id
                   WHERE cfp.field_id = $1 AND cfp.enabled
                   ORDER BY p.key"#,
            )
            .bind(field.id)
            .fetch_all(pool)
            .await?
        } else {
            vec![]
        };
        out.push(ConfigCustomField {
            key: field.key,
            name: field.name,
            field_type: field.field_type,
            entity: field.entity,
            option_list_key,
            default_value: field.default_value,
            required: field.required,
            help_text: field.help_text,
            sort_order: field.sort_order,
            show_in_list: field.show_in_list,
            show_in_filters: field.show_in_filters,
            active: field.active,
            scope: field.scope,
            project_keys,
        });
    }
    Ok(out)
}

async fn export_projects(
    pool: &PgPool,
    project_ids: &[Uuid],
    include_folders: bool,
) -> AppResult<Vec<ConfigProject>> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, String, Option<String>)>(
        r#"SELECT id, key, name, description, case_key_prefix
           FROM projects WHERE id = ANY($1) ORDER BY key"#,
    )
    .bind(project_ids)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, key, name, description, case_key_prefix) in rows {
        let folders = if include_folders {
            export_folder_tree(pool, id).await?
        } else {
            vec![]
        };
        out.push(ConfigProject {
            key,
            name,
            description,
            case_key_prefix,
            folders,
        });
    }
    Ok(out)
}

async fn export_folder_tree(pool: &PgPool, project_id: Uuid) -> AppResult<Vec<ConfigFolder>> {
    #[derive(sqlx::FromRow)]
    struct SuiteRow {
        id: Uuid,
        parent_id: Option<Uuid>,
        name: String,
    }
    let rows = sqlx::query_as::<_, SuiteRow>(
        "SELECT id, parent_id, name FROM suites WHERE project_id = $1 ORDER BY sort_order, name",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    fn build(parent: Option<Uuid>, rows: &[SuiteRow]) -> Vec<ConfigFolder> {
        rows.iter()
            .filter(|r| r.parent_id == parent)
            .map(|r| ConfigFolder {
                name: r.name.clone(),
                children: build(Some(r.id), rows),
            })
            .collect()
    }
    Ok(build(None, &rows))
}

async fn export_memberships(pool: &PgPool, project_ids: &[Uuid]) -> AppResult<ConfigRolesSection> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        r#"SELECT p.key, u.email, pm.role::text
           FROM memberships pm
           JOIN projects p ON p.id = pm.project_id
           JOIN users u ON u.id = pm.user_id
           WHERE pm.project_id = ANY($1)
           ORDER BY p.key, u.email"#,
    )
    .bind(project_ids)
    .fetch_all(pool)
    .await?;
    Ok(ConfigRolesSection {
        memberships: rows
            .into_iter()
            .map(|(project_key, email, role)| ConfigMembership {
                project_key,
                email,
                role,
            })
            .collect(),
    })
}

async fn export_llm(pool: &PgPool, project_ids: &[Uuid]) -> AppResult<Vec<ConfigLlmProvider>> {
    let rows = sqlx::query(
        r#"SELECT p.key AS project_key, lc.provider, lc.name, lc.model, lc.is_default, lc.extra, lc.base_url
           FROM llm_configs lc
           LEFT JOIN projects p ON p.id = lc.project_id
           WHERE lc.project_id IS NULL OR lc.project_id = ANY($1)
           ORDER BY lc.name"#,
    )
    .bind(project_ids)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for row in rows {
        use sqlx::Row;
        let mut config: Value = row.try_get("extra").unwrap_or(json!({}));
        if let Some(obj) = config.as_object_mut() {
            obj.remove("api_key");
            obj.remove("apiKey");
            obj.remove("token");
            obj.remove("secret");
            if let Ok(Some(base_url)) = row.try_get::<Option<String>, _>("base_url") {
                obj.insert("base_url".into(), Value::String(base_url));
            }
            if let Ok(is_default) = row.try_get::<bool, _>("is_default") {
                obj.insert("is_default".into(), Value::Bool(is_default));
            }
        }
        out.push(ConfigLlmProvider {
            project_key: row.try_get("project_key").ok(),
            provider: row.try_get("provider").unwrap_or_else(|_| "unknown".into()),
            name: row.try_get("name").unwrap_or_else(|_| "llm".into()),
            model: row.try_get("model").ok(),
            enabled: true,
            config,
        });
    }
    Ok(out)
}

async fn export_integrations(
    pool: &PgPool,
    project_ids: &[Uuid],
) -> AppResult<Vec<ConfigIntegration>> {
    let rows = sqlx::query(
        r#"SELECT p.key AS project_key, i.kind, i.name, i.enabled, i.config
           FROM integration_settings i
           LEFT JOIN projects p ON p.id = i.project_id
           WHERE i.project_id IS NULL OR i.project_id = ANY($1)
           ORDER BY i.kind, i.name"#,
    )
    .bind(project_ids)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for row in rows {
        use sqlx::Row;
        let mut config: Value = row.try_get("config").unwrap_or(json!({}));
        if let Some(obj) = config.as_object_mut() {
            for k in ["token", "api_key", "apiKey", "secret", "password", "webhook_secret"] {
                obj.remove(k);
            }
        }
        out.push(ConfigIntegration {
            project_key: row.try_get("project_key").ok(),
            kind: row.try_get("kind")?,
            name: row.try_get("name")?,
            enabled: row.try_get("enabled")?,
            config,
        });
    }
    Ok(out)
}

async fn export_automation_repos(
    pool: &PgPool,
    project_ids: &[Uuid],
) -> AppResult<Vec<ConfigAutomationRepo>> {
    let rows = sqlx::query(
        r#"SELECT p.key AS project_key, ar.provider, ar.remote_url, ar.branch, ar.subpath
           FROM automation_repos ar
           JOIN projects p ON p.id = ar.project_id
           WHERE ar.project_id = ANY($1)
           ORDER BY p.key"#,
    )
    .bind(project_ids)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for row in rows {
        use sqlx::Row;
        out.push(ConfigAutomationRepo {
            project_key: row.try_get("project_key")?,
            provider: row.try_get("provider").unwrap_or_else(|_| "generic".into()),
            remote_url: row.try_get("remote_url")?,
            branch: row.try_get::<String, _>("branch").ok(),
            subpath: row.try_get::<String, _>("subpath").ok(),
        });
    }
    Ok(out)
}

async fn export_sso(
    pool: &PgPool,
    passphrase: Option<&str>,
) -> AppResult<Vec<ConfigSsoProvider>> {
    let rows = sqlx::query(
        r#"SELECT provider_key, display_name, enabled, issuer_url, client_id, scopes,
                  extra_config, client_secret_ciphertext
           FROM sso_providers ORDER BY provider_key"#,
    )
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for row in rows {
        use sqlx::Row;
        let mut config: Value = row.try_get("extra_config").unwrap_or(json!({}));
        if let Some(obj) = config.as_object_mut() {
            obj.remove("client_secret");
            obj.remove("clientSecret");
            if let Ok(Some(issuer)) = row.try_get::<Option<String>, _>("issuer_url") {
                obj.insert("issuer_url".into(), Value::String(issuer));
            }
            if let Ok(client_id) = row.try_get::<String, _>("client_id") {
                obj.insert("client_id".into(), Value::String(client_id));
            }
            if let Ok(scopes) = row.try_get::<String, _>("scopes") {
                obj.insert("scopes".into(), Value::String(scopes));
            }
        }
        let encrypted_secrets = if let Some(pass) = passphrase {
            let secret_enc: Option<Vec<u8>> = row.try_get("client_secret_ciphertext").ok().flatten();
            secret_enc.map(|bytes| {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(pass.as_bytes());
                hasher.update(&bytes);
                format!("enc:v1:{}", hex::encode(hasher.finalize()))
            })
        } else {
            None
        };
        out.push(ConfigSsoProvider {
            key: row.try_get("provider_key")?,
            name: row.try_get("display_name")?,
            provider_type: "oidc".into(),
            enabled: row.try_get("enabled").unwrap_or(false),
            config,
            encrypted_secrets,
        });
    }
    Ok(out)
}

async fn export_data(pool: &PgPool, project_ids: &[Uuid]) -> AppResult<ConfigDataSection> {
    let cases: Vec<Value> = sqlx::query_scalar(
        r#"SELECT to_jsonb(tc.*) FROM test_cases tc
           WHERE tc.project_id = ANY($1) ORDER BY tc.key"#,
    )
    .bind(project_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let requirements: Vec<Value> = sqlx::query_scalar(
        r#"SELECT to_jsonb(r.*) FROM requirements r
           WHERE r.project_id = ANY($1) ORDER BY r.key"#,
    )
    .bind(project_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let runs: Vec<Value> = sqlx::query_scalar(
        r#"SELECT to_jsonb(tr.*) FROM test_runs tr
           WHERE tr.project_id = ANY($1) ORDER BY tr.created_at"#,
    )
    .bind(project_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    Ok(ConfigDataSection {
        cases,
        requirements,
        runs,
    })
}

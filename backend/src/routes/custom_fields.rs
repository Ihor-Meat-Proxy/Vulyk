use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission, RequireSystemAdmin};
use crate::error::{AppError, AppResult};
use crate::models::{
    is_allowed_field_type, needs_option_list, normalize_field_type, AddFieldToProject,
    CreateCustomField, CreateCustomFieldDefinition, CustomField, CustomFieldDefinition,
    CustomFieldDefinitionDetail, CustomFieldProjectOverride, ScopeChangeWarning,
    UpdateCustomField, UpdateCustomFieldDefinition, UpsertProjectFieldOverride,
};
use crate::routes::AppState;
use crate::services::{cases::ensure_project, custom_fields, option_lists};
use crate::services::option_lists::suggest_key;

pub fn router() -> Router<AppState> {
    Router::new()
        // Admin: global field definitions
        .route(
            "/api/v1/admin/custom-fields",
            get(admin_list_fields).post(admin_create_field),
        )
        .route(
            "/api/v1/admin/custom-fields/{field_id}",
            get(admin_get_field)
                .put(admin_update_field)
                .delete(admin_delete_field),
        )
        // Project-scoped resolved fields (editors read; create still admin-oriented via selected add)
        .route(
            "/api/v1/projects/{project_id}/custom-fields",
            get(list_fields).post(create_field_compat),
        )
        .route(
            "/api/v1/projects/{project_id}/custom-fields/available",
            get(list_available),
        )
        .route(
            "/api/v1/projects/{project_id}/custom-fields/{field_id}",
            get(get_field).put(update_field_compat).delete(delete_field_compat),
        )
        .route(
            "/api/v1/projects/{project_id}/custom-fields/{field_id}/add",
            axum::routing::post(add_field_to_project),
        )
        .route(
            "/api/v1/projects/{project_id}/custom-fields/{field_id}/override",
            axum::routing::put(upsert_override),
        )
}

#[derive(Debug, Deserialize)]
pub struct EntityQuery {
    pub entity: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/custom-fields",
    tag = "custom_fields",
    responses((status = 200, body = [CustomFieldDefinitionDetail])),
    security(("bearer_auth" = []))
)]
pub async fn admin_list_fields(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<Vec<CustomFieldDefinitionDetail>>> {
    Ok(Json(custom_fields::list_definitions(&state.pool).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/custom-fields",
    tag = "custom_fields",
    request_body = CreateCustomFieldDefinition,
    responses((status = 200, body = CustomFieldDefinitionDetail)),
    security(("bearer_auth" = []))
)]
pub async fn admin_create_field(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(body): Json<CreateCustomFieldDefinition>,
) -> AppResult<Json<CustomFieldDefinitionDetail>> {
    let (detail, _) = custom_fields::create_definition(&state.pool, body).await?;
    Ok(Json(detail))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/custom-fields/{field_id}",
    tag = "custom_fields",
    params(("field_id" = Uuid, Path)),
    responses((status = 200, body = CustomFieldDefinitionDetail)),
    security(("bearer_auth" = []))
)]
pub async fn admin_get_field(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(field_id): Path<Uuid>,
) -> AppResult<Json<CustomFieldDefinitionDetail>> {
    Ok(Json(
        custom_fields::get_definition(&state.pool, field_id).await?,
    ))
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct UpdateFieldResponse {
    pub field: CustomFieldDefinitionDetail,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<ScopeChangeWarning>,
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/custom-fields/{field_id}",
    tag = "custom_fields",
    params(("field_id" = Uuid, Path)),
    request_body = UpdateCustomFieldDefinition,
    responses((status = 200, body = UpdateFieldResponse)),
    security(("bearer_auth" = []))
)]
pub async fn admin_update_field(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(field_id): Path<Uuid>,
    Json(body): Json<UpdateCustomFieldDefinition>,
) -> AppResult<Json<UpdateFieldResponse>> {
    let (field, warning) = custom_fields::update_definition(&state.pool, field_id, body).await?;
    Ok(Json(UpdateFieldResponse { field, warning }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/custom-fields/{field_id}",
    tag = "custom_fields",
    params(("field_id" = Uuid, Path)),
    responses((status = 200, description = "deleted or deactivated")),
    security(("bearer_auth" = []))
)]
pub async fn admin_delete_field(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(field_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let action = custom_fields::delete_or_deactivate(&state.pool, field_id).await?;
    Ok(Json(json!({ "action": action })))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/custom-fields",
    tag = "custom_fields",
    params(
        ("project_id" = Uuid, Path),
        ("entity" = Option<String>, Query, description = "test_case | requirement")
    ),
    responses((status = 200, description = "Custom fields", body = [CustomField])),
    security(("bearer_auth" = []))
)]
pub async fn list_fields(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Query(query): Query<EntityQuery>,
) -> AppResult<Json<Vec<CustomField>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let entity = query.entity.as_deref();
    Ok(Json(
        custom_fields::list_for_project(&state.pool, project_id, entity).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/custom-fields/available",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path)),
    responses((status = 200, body = [CustomFieldDefinition])),
    security(("bearer_auth" = []))
)]
pub async fn list_available(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<CustomFieldDefinition>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    Ok(Json(
        custom_fields::available_to_add(&state.pool, project_id).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/custom-fields/{field_id}/add",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path), ("field_id" = Uuid, Path)),
    request_body = AddFieldToProject,
    responses((status = 200, body = CustomField)),
    security(("bearer_auth" = []))
)]
pub async fn add_field_to_project(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, field_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AddFieldToProject>,
) -> AppResult<Json<CustomField>> {
    // Project admins / system admins via manage memberships or create-edit as proxy;
    // prefer system admin or CreateEditCases for adding available fields.
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    Ok(Json(
        custom_fields::add_to_project(&state.pool, project_id, field_id, body).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/custom-fields/{field_id}/override",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path), ("field_id" = Uuid, Path)),
    request_body = UpsertProjectFieldOverride,
    responses((status = 200, body = CustomFieldProjectOverride)),
    security(("bearer_auth" = []))
)]
pub async fn upsert_override(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, field_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpsertProjectFieldOverride>,
) -> AppResult<Json<CustomFieldProjectOverride>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    Ok(Json(
        custom_fields::upsert_override(&state.pool, project_id, field_id, body).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/custom-fields",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path)),
    request_body = CreateCustomField,
    responses((status = 200, description = "Created (selected scope on this project)", body = CustomField)),
    security(("bearer_auth" = []))
)]
pub async fn create_field_compat(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateCustomField>,
) -> AppResult<Json<CustomField>> {
    // Only system admins may create definitions; project editors get 403 unless open auth.
    if state.auth.required {
        let is_admin = auth
            .as_ref()
            .map(|a| a.is_system_admin())
            .unwrap_or(false);
        if !is_admin {
            return Err(AppError::Forbidden(
                "only system administrators can create custom field definitions".into(),
            ));
        }
    }
    ensure_project(&state.pool, project_id).await?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    if !is_allowed_field_type(&body.field_type) {
        return Err(AppError::BadRequest("invalid field_type".into()));
    }
    let field_type = normalize_field_type(&body.field_type);
    let key = body
        .key
        .unwrap_or_else(|| suggest_key(name))
        .trim()
        .to_string();

    let mut option_list_id = body.option_list_id;
    if needs_option_list(&field_type) && option_list_id.is_none() {
        // Build a list from legacy options array if provided.
        if let Some(arr) = body.options.as_array() {
            let items: Vec<_> = arr
                .iter()
                .enumerate()
                .filter_map(|(idx, v)| {
                    let label = v
                        .as_str()
                        .map(|s| s.to_string())
                        .or_else(|| v.get("label").and_then(|x| x.as_str()).map(str::to_string))
                        .or_else(|| v.get("name").and_then(|x| x.as_str()).map(str::to_string))?;
                    Some(crate::models::CreateOptionListItem {
                        label,
                        key: v.get("key").and_then(|x| x.as_str()).map(str::to_string),
                        value: v.get("value").and_then(|x| x.as_str()).map(str::to_string),
                        colour: v.get("colour").and_then(|x| x.as_str()).map(str::to_string),
                        description: String::new(),
                        sort_order: idx as i32,
                        active: true,
                        deprecated: false,
                    })
                })
                .collect();
            let list = option_lists::create(
                &state.pool,
                crate::models::CreateOptionList {
                    key: format!("{key}_options"),
                    name: format!("{name} options"),
                    description: String::new(),
                    active: true,
                    items,
                },
            )
            .await?;
            option_list_id = Some(list.list.id);
        } else {
            return Err(AppError::BadRequest(
                "option_list_id or options required for dropdown/multi_select".into(),
            ));
        }
    }

    let (detail, _) = custom_fields::create_definition(
        &state.pool,
        CreateCustomFieldDefinition {
            key,
            name: name.into(),
            field_type,
            entity: body.entity.unwrap_or_else(|| "test_case".into()),
            option_list_id,
            default_value: body.default_value.unwrap_or(serde_json::Value::Null),
            required: body.required,
            help_text: body.help_text.unwrap_or_default(),
            sort_order: body.sort_order,
            show_in_list: body.show_in_list.unwrap_or(false),
            show_in_filters: body.show_in_filters.unwrap_or(false),
            active: true,
            scope: "selected".into(),
            project_ids: vec![project_id],
        },
    )
    .await?;

    custom_fields::list_for_project(&state.pool, project_id, None)
        .await?
        .into_iter()
        .find(|f| f.id == detail.field.id)
        .map(Json)
        .ok_or_else(|| AppError::Internal("created field not visible".into()))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/custom-fields/{field_id}",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path), ("field_id" = Uuid, Path)),
    responses((status = 200, description = "Field", body = CustomField)),
    security(("bearer_auth" = []))
)]
pub async fn get_field(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, field_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<CustomField>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    custom_fields::list_for_project(&state.pool, project_id, None)
        .await?
        .into_iter()
        .find(|f| f.id == field_id)
        .map(Json)
        .ok_or_else(|| AppError::NotFound("custom field".into()))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/custom-fields/{field_id}",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path), ("field_id" = Uuid, Path)),
    request_body = UpdateCustomField,
    responses((status = 200, description = "Updated", body = CustomField)),
    security(("bearer_auth" = []))
)]
pub async fn update_field_compat(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, field_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCustomField>,
) -> AppResult<Json<CustomField>> {
    if state.auth.required {
        let is_admin = auth
            .as_ref()
            .map(|a| a.is_system_admin())
            .unwrap_or(false);
        if !is_admin {
            // Allow per-project override of required/default for editors with create-edit.
            require_permission(
                &state.pool,
                &auth,
                state.auth.required,
                project_id,
                Permission::CreateEditCases,
            )
            .await?;
            custom_fields::upsert_override(
                &state.pool,
                project_id,
                field_id,
                UpsertProjectFieldOverride {
                    required_override: body.required.map(Some),
                    default_value_override: body.default_value.map(Some),
                    enabled: body.active,
                },
            )
            .await?;
            return custom_fields::list_for_project(&state.pool, project_id, None)
                .await?
                .into_iter()
                .find(|f| f.id == field_id)
                .map(Json)
                .ok_or_else(|| AppError::NotFound("custom field".into()));
        }
    }
    custom_fields::update_definition(
        &state.pool,
        field_id,
        UpdateCustomFieldDefinition {
            key: None,
            name: body.name,
            field_type: body.field_type,
            entity: None,
            option_list_id: body.option_list_id.map(Some),
            default_value: body.default_value,
            required: body.required,
            help_text: body.help_text,
            sort_order: body.sort_order,
            show_in_list: body.show_in_list,
            show_in_filters: body.show_in_filters,
            active: body.active,
            scope: None,
            project_ids: None,
            acknowledge_hide_values: false,
        },
    )
    .await?;
    custom_fields::list_for_project(&state.pool, project_id, None)
        .await?
        .into_iter()
        .find(|f| f.id == field_id)
        .map(Json)
        .ok_or_else(|| AppError::NotFound("custom field".into()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/custom-fields/{field_id}",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path), ("field_id" = Uuid, Path)),
    responses((status = 200, description = "Hidden from project or deactivated")),
    security(("bearer_auth" = []))
)]
pub async fn delete_field_compat(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, field_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    if state.auth.required {
        let is_admin = auth
            .as_ref()
            .map(|a| a.is_system_admin())
            .unwrap_or(false);
        if !is_admin {
            require_permission(
                &state.pool,
                &auth,
                state.auth.required,
                project_id,
                Permission::CreateEditCases,
            )
            .await?;
            // Hide from project; retain values.
            custom_fields::upsert_override(
                &state.pool,
                project_id,
                field_id,
                UpsertProjectFieldOverride {
                    required_override: None,
                    default_value_override: None,
                    enabled: Some(false),
                },
            )
            .await?;
            return Ok(Json(json!({
                "action": "hidden",
                "warning": "Field hidden from project; stored values retained."
            })));
        }
    }
    let action = custom_fields::delete_or_deactivate(&state.pool, field_id).await?;
    Ok(Json(json!({ "action": action })))
}

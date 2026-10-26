//! Content archive restore / import.

use std::collections::HashMap;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    BackupFieldMapping, ContentArchive, ContentFolderNode, ContentProjectBundle, RestoreConflict,
    RestoreCounts, RestorePreview, RestoreRequest,
};
use crate::services::case_keys;

pub async fn preview_or_apply(
    pool: &PgPool,
    archive: &ContentArchive,
    req: &RestoreRequest,
) -> AppResult<RestorePreview> {
    let mode = req.mode.to_lowercase();
    if !matches!(mode.as_str(), "new_project" | "merge" | "replace") {
        return Err(AppError::BadRequest(
            "mode must be new_project, merge, or replace".into(),
        ));
    }
    let conflict = req.conflict.to_lowercase();
    if !matches!(conflict.as_str(), "skip" | "overwrite") {
        return Err(AppError::BadRequest(
            "conflict must be skip or overwrite".into(),
        ));
    }

    let bundle = archive
        .projects
        .first()
        .ok_or_else(|| AppError::BadRequest("archive has no projects".into()))?;

    // Multi-project archives: restore one at a time for now; use first or match target
    let bundle = if let Some(tid) = req.target_project_id {
        let key: String = sqlx::query_scalar("SELECT key FROM projects WHERE id = $1")
            .bind(tid)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("target project".into()))?;
        archive
            .projects
            .iter()
            .find(|p| p.project.key == key)
            .unwrap_or(bundle)
    } else {
        bundle
    };

    let mut counts = RestoreCounts::default();
    let mut conflicts = Vec::new();
    let mut mappings = Vec::new();
    let mut warnings = Vec::new();

    let target_project_id = match mode.as_str() {
        "new_project" => {
            let key = req
                .new_project_key
                .clone()
                .unwrap_or_else(|| bundle.project.key.clone());
            let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM projects WHERE key = $1")
                .bind(&key)
                .fetch_optional(pool)
                .await?;
            if exists.is_some() && req.new_project_key.is_none() {
                warnings.push(format!(
                    "project key '{key}' exists; will allocate a unique key on apply"
                ));
            }
            None
        }
        _ => {
            let id = req.target_project_id.ok_or_else(|| {
                AppError::BadRequest("target_project_id required for merge/replace".into())
            })?;
            Some(id)
        }
    };

    // Custom field mapping preview
    if let Some(pid) = target_project_id {
        let existing: Vec<(String, String)> =
            sqlx::query_as("SELECT name, name FROM custom_fields WHERE project_id = $1")
                .bind(pid)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
        // Use name as key slug comparison
        for field in bundle
            .custom_fields
            .iter()
            .chain(archive.custom_fields.iter())
        {
            let by_key = existing.iter().any(|(n, _)| slug(n) == field.key);
            let by_name = existing.iter().any(|(n, _)| n == &field.name);
            let action = if by_key || by_name { "map" } else { "create" };
            if action == "create" {
                counts.custom_fields_create += 1;
            }
            mappings.push(BackupFieldMapping {
                archive_key: field.key.clone(),
                archive_name: field.name.clone(),
                action: action.into(),
                target_key: if by_key {
                    Some(field.key.clone())
                } else {
                    None
                },
            });
        }

        for case in &bundle.cases {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM test_cases WHERE project_id=$1 AND key=$2)",
            )
            .bind(pid)
            .bind(&case.key)
            .fetch_one(pool)
            .await?;
            if exists {
                conflicts.push(RestoreConflict {
                    entity: "case".into(),
                    key: case.key.clone(),
                    detail: format!("exists; will {conflict}"),
                });
                if conflict == "skip" {
                    counts.cases_skip += 1;
                } else {
                    counts.cases_update += 1;
                }
            } else {
                counts.cases_create += 1;
            }
        }

        for req_item in &bundle.requirements {
            let key = req_item
                .external_key
                .clone()
                .unwrap_or_else(|| req_item.title.clone());
            let exists: bool = if let Some(ref ek) = req_item.external_key {
                sqlx::query_scalar(
                    r#"SELECT EXISTS(
                         SELECT 1 FROM requirements
                         WHERE project_id=$1 AND external_key=$2
                       )"#,
                )
                .bind(pid)
                .bind(ek)
                .fetch_one(pool)
                .await?
            } else {
                false
            };
            if exists {
                conflicts.push(RestoreConflict {
                    entity: "requirement".into(),
                    key,
                    detail: format!("exists; will {conflict}"),
                });
                if conflict == "skip" {
                    counts.requirements_skip += 1;
                } else {
                    counts.requirements_update += 1;
                }
            } else {
                counts.requirements_create += 1;
            }
        }
    } else {
        counts.cases_create = bundle.cases.len() as i32;
        counts.requirements_create = bundle.requirements.len() as i32;
        counts.custom_fields_create = bundle.custom_fields.len() as i32;
        counts.tags_create = bundle.tags.len() as i32;
        counts.tombstones_create = bundle.tombstones.len() as i32;
        counts.folders_create = count_folders(&bundle.case_folders) as i32
            + count_folders(&bundle.requirement_folders) as i32;
        counts.attachments_create = bundle.attachments.len() as i32;
        for field in &bundle.custom_fields {
            mappings.push(BackupFieldMapping {
                archive_key: field.key.clone(),
                archive_name: field.name.clone(),
                action: "create".into(),
                target_key: None,
            });
        }
    }

    counts.tags_create = if target_project_id.is_some() {
        0 // refined on apply
    } else {
        counts.tags_create
    };

    let preview = RestorePreview {
        dry_run: req.dry_run,
        mode: mode.clone(),
        schema_version: archive.manifest.schema_version.clone(),
        project_key: bundle.project.key.clone(),
        counts: counts.clone(),
        conflicts,
        custom_field_mappings: mappings,
        warnings,
        applied: false,
    };

    if req.dry_run {
        return Ok(preview);
    }

    let applied_counts = apply(pool, archive, bundle, req, &mode, &conflict).await?;
    Ok(RestorePreview {
        dry_run: false,
        applied: true,
        counts: applied_counts,
        ..preview
    })
}

async fn apply(
    pool: &PgPool,
    archive: &ContentArchive,
    bundle: &ContentProjectBundle,
    req: &RestoreRequest,
    mode: &str,
    conflict: &str,
) -> AppResult<RestoreCounts> {
    let mut counts = RestoreCounts::default();

    let project_id = match mode {
        "new_project" => {
            let mut key = req
                .new_project_key
                .clone()
                .unwrap_or_else(|| bundle.project.key.clone());
            let mut n = 2;
            loop {
                let exists: bool =
                    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE key=$1)")
                        .bind(&key)
                        .fetch_one(pool)
                        .await?;
                if !exists {
                    break;
                }
                key = format!("{}{}", bundle.project.key, n);
                n += 1;
            }
            let prefix = case_keys::validate_prefix(&bundle.project.case_key_prefix)
                .or_else(|_| case_keys::validate_prefix(&key))
                .unwrap_or_else(|_| "PR".into());
            let prefix = unique_prefix(pool, &prefix).await?;
            let id: Uuid = sqlx::query_scalar(
                r#"INSERT INTO projects (key, name, description, case_key_prefix, next_case_number)
                   VALUES ($1, $2, $3, $4, $5) RETURNING id"#,
            )
            .bind(&key)
            .bind(&bundle.project.name)
            .bind(&bundle.project.description)
            .bind(&prefix)
            .bind(bundle.project.next_case_number.max(0))
            .fetch_one(pool)
            .await?;
            id
        }
        "replace" => {
            let id = req
                .target_project_id
                .ok_or_else(|| AppError::BadRequest("target_project_id required".into()))?;
            // Clear content (keep memberships / project row)
            sqlx::query("DELETE FROM test_cases WHERE project_id = $1")
                .bind(id)
                .execute(pool)
                .await?;
            sqlx::query("DELETE FROM requirements WHERE project_id = $1")
                .bind(id)
                .execute(pool)
                .await?;
            sqlx::query("DELETE FROM suites WHERE project_id = $1")
                .bind(id)
                .execute(pool)
                .await?;
            sqlx::query("DELETE FROM requirement_folders WHERE project_id = $1")
                .bind(id)
                .execute(pool)
                .await
                .ok();
            sqlx::query("DELETE FROM tags WHERE project_id = $1")
                .bind(id)
                .execute(pool)
                .await?;
            sqlx::query("DELETE FROM custom_fields WHERE project_id = $1")
                .bind(id)
                .execute(pool)
                .await?;
            sqlx::query("DELETE FROM attachments WHERE project_id = $1")
                .bind(id)
                .execute(pool)
                .await?;
            // Keep tombstones — never reuse retired numbers
            id
        }
        _ => req
            .target_project_id
            .ok_or_else(|| AppError::BadRequest("target_project_id required".into()))?,
    };

    // Folders
    let suite_map = ensure_folder_tree(pool, project_id, &bundle.case_folders, "suites").await?;
    counts.folders_create += suite_map.len() as i32;
    let req_folder_map = ensure_folder_tree(
        pool,
        project_id,
        &bundle.requirement_folders,
        "requirement_folders",
    )
    .await?;
    counts.folders_create += req_folder_map.len() as i32;

    // Tags
    let mut tag_ids: HashMap<String, Uuid> = HashMap::new();
    for tag in &bundle.tags {
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO tags (project_id, name, color)
               VALUES ($1, $2, $3)
               ON CONFLICT (project_id, name) DO UPDATE SET color = EXCLUDED.color
               RETURNING id"#,
        )
        .bind(project_id)
        .bind(&tag.name)
        .bind(&tag.color)
        .fetch_one(pool)
        .await?;
        tag_ids.insert(tag.name.clone(), id);
        counts.tags_create += 1;
    }

    // Custom fields
    let mut field_ids: HashMap<String, Uuid> = HashMap::new();
    let all_fields: Vec<_> = archive
        .custom_fields
        .iter()
        .chain(bundle.custom_fields.iter())
        .collect();
    let mut seen = std::collections::HashSet::new();
    for field in all_fields {
        if !seen.insert(field.key.clone()) {
            continue;
        }
        // Match by name first
        let existing: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM custom_fields WHERE project_id = $1 AND name = $2")
                .bind(project_id)
                .bind(&field.name)
                .fetch_optional(pool)
                .await?;
        let id = if let Some((id,)) = existing {
            id
        } else {
            let id: Uuid = sqlx::query_scalar(
                r#"INSERT INTO custom_fields (project_id, name, field_type, options, required, sort_order)
                   VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#,
            )
            .bind(project_id)
            .bind(&field.name)
            .bind(&field.field_type)
            .bind(&field.options)
            .bind(field.required)
            .bind(field.sort_order)
            .fetch_one(pool)
            .await?;
            counts.custom_fields_create += 1;
            id
        };
        field_ids.insert(field.key.clone(), id);
        field_ids.insert(slug(&field.name), id);
    }

    // Tombstones first
    for t in &bundle.tombstones {
        let res = sqlx::query(
            r#"INSERT INTO case_key_tombstones (project_id, key, case_number)
               VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"#,
        )
        .bind(project_id)
        .bind(&t.key)
        .bind(t.case_number)
        .execute(pool)
        .await?;
        if res.rows_affected() > 0 {
            counts.tombstones_create += 1;
        }
    }

    // Cases
    let mut case_id_by_key: HashMap<String, Uuid> = HashMap::new();
    for case in &bundle.cases {
        let existing: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM test_cases WHERE project_id = $1 AND key = $2")
                .bind(project_id)
                .bind(&case.key)
                .fetch_optional(pool)
                .await?;

        let suite_id = if case.folder_path.is_empty() {
            None
        } else {
            suite_map.get(&case.folder_path.join("/")).copied()
        };

        let case_id = match existing {
            Some((id,)) if conflict == "skip" && mode == "merge" => {
                counts.cases_skip += 1;
                case_id_by_key.insert(case.key.clone(), id);
                continue;
            }
            Some((id,)) => {
                sqlx::query(
                    r#"UPDATE test_cases SET title=$2, description=$3, precondition=$4,
                       priority=$5, status=$6, suite_id=$7, sort_order=$8, external_key=$9,
                       version=$10, updated_at=NOW()
                       WHERE id=$1"#,
                )
                .bind(id)
                .bind(&case.title)
                .bind(&case.description)
                .bind(&case.precondition)
                .bind(&case.priority)
                .bind(&case.status)
                .bind(suite_id)
                .bind(case.sort_order)
                .bind(&case.external_key)
                .bind(case.version.max(1))
                .execute(pool)
                .await?;
                sqlx::query("DELETE FROM test_steps WHERE case_id = $1")
                    .bind(id)
                    .execute(pool)
                    .await?;
                counts.cases_update += 1;
                id
            }
            None => {
                // Insert with preserved key (bypass allocate) — still respect tombstones
                let tombstoned: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM case_key_tombstones WHERE project_id=$1 AND key=$2)",
                )
                .bind(project_id)
                .bind(&case.key)
                .fetch_one(pool)
                .await?;
                if tombstoned {
                    // Key was retired — skip recreating under same key
                    continue;
                }
                let id: Uuid = sqlx::query_scalar(
                    r#"INSERT INTO test_cases
                       (project_id, suite_id, key, title, description, precondition,
                        priority, status, version, external_key, sort_order)
                       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
                       RETURNING id"#,
                )
                .bind(project_id)
                .bind(suite_id)
                .bind(&case.key)
                .bind(&case.title)
                .bind(&case.description)
                .bind(&case.precondition)
                .bind(&case.priority)
                .bind(&case.status)
                .bind(case.version.max(1))
                .bind(&case.external_key)
                .bind(case.sort_order)
                .fetch_one(pool)
                .await?;
                counts.cases_create += 1;
                id
            }
        };

        for step in &case.steps {
            sqlx::query(
                r#"INSERT INTO test_steps (case_id, position, action, expected_result, data)
                   VALUES ($1,$2,$3,$4,$5)"#,
            )
            .bind(case_id)
            .bind(step.position)
            .bind(&step.action)
            .bind(&step.expected_result)
            .bind(&step.data)
            .execute(pool)
            .await?;
        }

        for tag_name in &case.tags {
            if let Some(tid) = tag_ids.get(tag_name) {
                sqlx::query(
                    "INSERT INTO case_tags (case_id, tag_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
                )
                .bind(case_id)
                .bind(tid)
                .execute(pool)
                .await?;
            }
        }

        for fv in &case.custom_fields {
            if let Some(fid) = field_ids
                .get(&fv.field_key)
                .or_else(|| field_ids.get(&slug(&fv.field_name)))
            {
                sqlx::query(
                    r#"INSERT INTO custom_field_values (case_id, field_id, value)
                       VALUES ($1,$2,$3)
                       ON CONFLICT (case_id, field_id) DO UPDATE SET value = EXCLUDED.value"#,
                )
                .bind(case_id)
                .bind(fid)
                .bind(&fv.value)
                .execute(pool)
                .await?;
            }
        }

        for ver in &case.versions {
            sqlx::query(
                r#"INSERT INTO case_versions (case_id, version, snapshot, change_summary)
                   VALUES ($1,$2,$3,$4)
                   ON CONFLICT (case_id, version) DO NOTHING"#,
            )
            .bind(case_id)
            .bind(ver.version)
            .bind(&ver.snapshot)
            .bind(&ver.change_summary)
            .execute(pool)
            .await
            .ok();
        }

        case_id_by_key.insert(case.key.clone(), case_id);
    }

    // Bump next_case_number past max of imported keys and tombstones
    bump_next_case_number(pool, project_id).await?;

    // Requirements
    for req_item in &bundle.requirements {
        let folder_id = if req_item.folder_path.is_empty() {
            None
        } else {
            req_folder_map.get(&req_item.folder_path.join("/")).copied()
        };

        let existing: Option<(Uuid,)> = if let Some(ref ek) = req_item.external_key {
            sqlx::query_as("SELECT id FROM requirements WHERE project_id=$1 AND external_key=$2")
                .bind(project_id)
                .bind(ek)
                .fetch_optional(pool)
                .await?
        } else {
            None
        };

        let rid = match existing {
            Some((id,)) if conflict == "skip" && mode == "merge" => {
                counts.requirements_skip += 1;
                id
            }
            Some((id,)) => {
                sqlx::query(
                    r#"UPDATE requirements SET title=$2, description=$3, acceptance_criteria=$4,
                       source=$5, url=$6, folder_id=$7, updated_at=NOW()
                       WHERE id=$1"#,
                )
                .bind(id)
                .bind(&req_item.title)
                .bind(&req_item.description)
                .bind(&req_item.acceptance_criteria)
                .bind(&req_item.source)
                .bind(&req_item.url)
                .bind(folder_id)
                .execute(pool)
                .await?;
                counts.requirements_update += 1;
                id
            }
            None => {
                let id: Uuid = sqlx::query_scalar(
                    r#"INSERT INTO requirements
                       (project_id, folder_id, title, description, acceptance_criteria,
                        source, external_key, external_id, url, metadata)
                       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'{}'::jsonb)
                       RETURNING id"#,
                )
                .bind(project_id)
                .bind(folder_id)
                .bind(&req_item.title)
                .bind(&req_item.description)
                .bind(&req_item.acceptance_criteria)
                .bind(&req_item.source)
                .bind(&req_item.external_key)
                .bind(&req_item.external_id)
                .bind(&req_item.url)
                .fetch_one(pool)
                .await?;
                counts.requirements_create += 1;
                id
            }
        };

        if !(existing.is_some() && conflict == "skip" && mode == "merge") {
            for ck in &req_item.case_keys {
                if let Some(cid) = case_id_by_key.get(ck) {
                    sqlx::query(
                        r#"INSERT INTO case_requirements (case_id, requirement_id)
                           VALUES ($1,$2) ON CONFLICT DO NOTHING"#,
                    )
                    .bind(cid)
                    .bind(rid)
                    .execute(pool)
                    .await?;
                }
            }
        }
    }

    let _ = archive; // global lists applied when PR #24 tables exist — future hook
    let _ = json!({});
    Ok(counts)
}

async fn bump_next_case_number(pool: &PgPool, project_id: Uuid) -> AppResult<()> {
    let prefix: String = sqlx::query_scalar("SELECT case_key_prefix FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    let max_from_cases: Option<i32> = sqlx::query_scalar(
        r#"SELECT MAX(NULLIF(substring(key from '[0-9]+$'), '')::int)
           FROM test_cases
           WHERE project_id = $1 AND key ~ ('^' || $2 || '-[0-9]+$')"#,
    )
    .bind(project_id)
    .bind(&prefix)
    .fetch_one(pool)
    .await?;
    let max_from_tomb: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(case_number) FROM case_key_tombstones WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    let max_n = max_from_cases.unwrap_or(0).max(max_from_tomb.unwrap_or(0));
    sqlx::query(
        r#"UPDATE projects SET next_case_number = GREATEST(next_case_number, $2)
           WHERE id = $1"#,
    )
    .bind(project_id)
    .bind(max_n)
    .execute(pool)
    .await?;
    Ok(())
}

async fn unique_prefix(pool: &PgPool, base: &str) -> AppResult<String> {
    let mut prefix = base.to_string();
    let mut n = 2;
    loop {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE case_key_prefix=$1)")
                .bind(&prefix)
                .fetch_one(pool)
                .await?;
        if !exists {
            return Ok(prefix);
        }
        let stem = if base.len() > 6 { &base[..6] } else { base };
        prefix = format!("{stem}{n}");
        n += 1;
        if n > 99 {
            return Err(AppError::Conflict(
                "unable to allocate case_key_prefix".into(),
            ));
        }
    }
}

async fn ensure_folder_tree(
    pool: &PgPool,
    project_id: Uuid,
    nodes: &[ContentFolderNode],
    table: &str,
) -> AppResult<HashMap<String, Uuid>> {
    let mut map = HashMap::new();
    async fn walk(
        pool: &PgPool,
        project_id: Uuid,
        table: &str,
        nodes: &[ContentFolderNode],
        parent: Option<Uuid>,
        path_prefix: &str,
        map: &mut HashMap<String, Uuid>,
    ) -> AppResult<()> {
        for node in nodes {
            let path = if path_prefix.is_empty() {
                node.name.clone()
            } else {
                format!("{path_prefix}/{}", node.name)
            };
            let id = match table {
                "suites" => {
                    let existing: Option<(Uuid,)> = if let Some(pid) = parent {
                        sqlx::query_as(
                            "SELECT id FROM suites WHERE project_id=$1 AND parent_id=$2 AND name=$3",
                        )
                        .bind(project_id)
                        .bind(pid)
                        .bind(&node.name)
                        .fetch_optional(pool)
                        .await?
                    } else {
                        sqlx::query_as(
                            "SELECT id FROM suites WHERE project_id=$1 AND parent_id IS NULL AND name=$2",
                        )
                        .bind(project_id)
                        .bind(&node.name)
                        .fetch_optional(pool)
                        .await?
                    };
                    if let Some((id,)) = existing {
                        id
                    } else {
                        sqlx::query_scalar(
                            r#"INSERT INTO suites (project_id, parent_id, name, sort_order)
                               VALUES ($1,$2,$3,$4) RETURNING id"#,
                        )
                        .bind(project_id)
                        .bind(parent)
                        .bind(&node.name)
                        .bind(node.sort_order)
                        .fetch_one(pool)
                        .await?
                    }
                }
                _ => {
                    let existing: Option<(Uuid,)> = if let Some(pid) = parent {
                        sqlx::query_as(
                            "SELECT id FROM requirement_folders WHERE project_id=$1 AND parent_id=$2 AND name=$3",
                        )
                        .bind(project_id)
                        .bind(pid)
                        .bind(&node.name)
                        .fetch_optional(pool)
                        .await?
                    } else {
                        sqlx::query_as(
                            "SELECT id FROM requirement_folders WHERE project_id=$1 AND parent_id IS NULL AND name=$2",
                        )
                        .bind(project_id)
                        .bind(&node.name)
                        .fetch_optional(pool)
                        .await?
                    };
                    if let Some((id,)) = existing {
                        id
                    } else {
                        sqlx::query_scalar(
                            r#"INSERT INTO requirement_folders (project_id, parent_id, name, sort_order)
                               VALUES ($1,$2,$3,$4) RETURNING id"#,
                        )
                        .bind(project_id)
                        .bind(parent)
                        .bind(&node.name)
                        .bind(node.sort_order)
                        .fetch_one(pool)
                        .await?
                    }
                }
            };
            map.insert(path.clone(), id);
            Box::pin(walk(
                pool,
                project_id,
                table,
                &node.children,
                Some(id),
                &path,
                map,
            ))
            .await?;
        }
        Ok(())
    }
    walk(pool, project_id, table, nodes, None, "", &mut map).await?;
    Ok(map)
}

fn count_folders(nodes: &[ContentFolderNode]) -> usize {
    nodes.iter().map(|n| 1 + count_folders(&n.children)).sum()
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    s.trim_matches('_').to_string()
}

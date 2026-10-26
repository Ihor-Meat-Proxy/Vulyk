//! Build and parse content archives (JSON + ZIP + checksums).

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::{AppError, AppResult};
use crate::models::{
    ContentArchive, ContentArchiveManifest, ContentArchiveProject, ContentAttachmentMeta,
    ContentCase, ContentCaseVersion, ContentCustomFieldDef, ContentFieldValue, ContentFolderNode,
    ContentOptionList, ContentProjectBundle, ContentRequirement, ContentStep, ContentTag,
    ContentTombstone, FileChecksum, CONTENT_ARCHIVE_SCHEMA_VERSION,
};

pub struct BuiltArchive {
    pub archive: ContentArchive,
    /// Relative path → bytes (for unpacked mode + ZIP members).
    pub files: Vec<(String, Vec<u8>)>,
    pub zip_bytes: Vec<u8>,
    pub checksum_sha256: String,
    pub size_bytes: i64,
}

pub async fn export_projects(
    pool: &PgPool,
    project_ids: &[Uuid],
    include_attachments: bool,
) -> AppResult<BuiltArchive> {
    let mut bundles = Vec::new();
    let mut project_keys = Vec::new();
    let mut attachment_blobs: Vec<(String, Vec<u8>)> = Vec::new();

    for pid in project_ids {
        let bundle =
            export_one_project(pool, *pid, include_attachments, &mut attachment_blobs).await?;
        project_keys.push(bundle.project.key.clone());
        bundles.push(bundle);
    }

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut checksums = Vec::new();

    // Global option lists / CF defs (union) — align with ConfigBundle keys
    let mut global_lists = Vec::new();
    let mut global_fields = Vec::new();
    for b in &bundles {
        for l in &b.option_lists {
            if !global_lists
                .iter()
                .any(|x: &ContentOptionList| x.key == l.key)
            {
                global_lists.push(l.clone());
            }
        }
        for f in &b.custom_fields {
            if !global_fields
                .iter()
                .any(|x: &ContentCustomFieldDef| x.key == f.key)
            {
                global_fields.push(f.clone());
            }
        }
    }

    push_json(
        &mut files,
        &mut checksums,
        "config/option_lists.json",
        &global_lists,
    )?;
    push_json(
        &mut files,
        &mut checksums,
        "config/custom_fields.json",
        &global_fields,
    )?;

    for bundle in &bundles {
        let base = format!("projects/{}", bundle.project.key);
        push_json(
            &mut files,
            &mut checksums,
            &format!("{base}/project.json"),
            &bundle.project,
        )?;
        push_json(
            &mut files,
            &mut checksums,
            &format!("{base}/case_folders.json"),
            &bundle.case_folders,
        )?;
        push_json(
            &mut files,
            &mut checksums,
            &format!("{base}/requirement_folders.json"),
            &bundle.requirement_folders,
        )?;
        push_json(
            &mut files,
            &mut checksums,
            &format!("{base}/tags.json"),
            &bundle.tags,
        )?;
        push_json(
            &mut files,
            &mut checksums,
            &format!("{base}/custom_fields.json"),
            &bundle.custom_fields,
        )?;
        push_json(
            &mut files,
            &mut checksums,
            &format!("{base}/cases.json"),
            &bundle.cases,
        )?;
        push_json(
            &mut files,
            &mut checksums,
            &format!("{base}/requirements.json"),
            &bundle.requirements,
        )?;
        push_json(
            &mut files,
            &mut checksums,
            &format!("{base}/tombstones.json"),
            &bundle.tombstones,
        )?;
        push_json(
            &mut files,
            &mut checksums,
            &format!("{base}/attachments.json"),
            &bundle.attachments,
        )?;
    }

    for (path, bytes) in &attachment_blobs {
        let digest = hex_sha256(bytes);
        checksums.push(FileChecksum {
            path: path.clone(),
            sha256: digest,
            size_bytes: bytes.len() as u64,
        });
        files.push((path.clone(), bytes.clone()));
    }

    let manifest = ContentArchiveManifest {
        schema_version: CONTENT_ARCHIVE_SCHEMA_VERSION.into(),
        created_at: Utc::now().to_rfc3339(),
        producer: format!("vulyk-api/{}", env!("CARGO_PKG_VERSION")),
        scope: if project_ids.len() == 1 {
            "project".into()
        } else {
            "all".into()
        },
        project_keys: project_keys.clone(),
        include_attachments,
        pack_mode: "zip".into(),
        file_checksums: checksums.clone(),
    };
    push_json(&mut files, &mut checksums, "manifest.json", &manifest)?;

    // Rewrite manifest with full checksums (including itself excluded — store without self)
    let mut manifest = manifest;
    manifest.file_checksums = checksums
        .into_iter()
        .filter(|c| c.path != "manifest.json")
        .collect();
    // Update manifest file bytes
    if let Some((_, bytes)) = files.iter_mut().find(|(p, _)| p == "manifest.json") {
        *bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| AppError::Internal(format!("manifest encode: {e}")))?;
    }

    let archive = ContentArchive {
        manifest: manifest.clone(),
        option_lists: global_lists,
        custom_fields: global_fields,
        projects: bundles,
    };

    // Also include archive.json for single-file consumers
    let archive_json = serde_json::to_vec_pretty(&archive)
        .map_err(|e| AppError::Internal(format!("archive encode: {e}")))?;
    files.push(("archive.json".into(), archive_json));

    let zip_bytes = zip_files(&files)?;
    let checksum_sha256 = hex_sha256(&zip_bytes);
    let size_bytes = zip_bytes.len() as i64;

    Ok(BuiltArchive {
        archive,
        files,
        zip_bytes,
        checksum_sha256,
        size_bytes,
    })
}

fn push_json<T: serde::Serialize>(
    files: &mut Vec<(String, Vec<u8>)>,
    checksums: &mut Vec<FileChecksum>,
    path: &str,
    value: &T,
) -> AppResult<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| AppError::Internal(format!("json encode {path}: {e}")))?;
    checksums.push(FileChecksum {
        path: path.into(),
        sha256: hex_sha256(&bytes),
        size_bytes: bytes.len() as u64,
    });
    files.push((path.into(), bytes));
    Ok(())
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

pub fn zip_files(files: &[(String, Vec<u8>)]) -> AppResult<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (path, bytes) in files {
            zip.start_file(path, opts)
                .map_err(|e| AppError::Internal(format!("zip start: {e}")))?;
            zip.write_all(bytes)
                .map_err(|e| AppError::Internal(format!("zip write: {e}")))?;
        }
        zip.finish()
            .map_err(|e| AppError::Internal(format!("zip finish: {e}")))?;
    }
    Ok(cursor.into_inner())
}

pub fn parse_archive_bytes(bytes: &[u8]) -> AppResult<ContentArchive> {
    // ZIP?
    if bytes.len() >= 4 && &bytes[0..2] == b"PK" {
        let mut zip = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| AppError::BadRequest(format!("invalid zip: {e}")))?;
        // Prefer archive.json
        if let Ok(mut f) = zip.by_name("archive.json") {
            let mut buf = String::new();
            f.read_to_string(&mut buf)
                .map_err(|e| AppError::BadRequest(format!("read archive.json: {e}")))?;
            let archive: ContentArchive = serde_json::from_str(&buf)
                .map_err(|e| AppError::BadRequest(format!("archive.json: {e}")))?;
            return migrate_archive(archive);
        }
        return Err(AppError::BadRequest("zip missing archive.json".into()));
    }
    // Raw JSON
    let archive: ContentArchive = serde_json::from_slice(bytes)
        .map_err(|e| AppError::BadRequest(format!("invalid archive JSON: {e}")))?;
    migrate_archive(archive)
}

pub fn migrate_archive(archive: ContentArchive) -> AppResult<ContentArchive> {
    let v = archive.manifest.schema_version.as_str();
    match v {
        "content-1.0" => Ok(archive),
        other if other.starts_with("content-1.") => Ok(archive), // forward-compatible minors
        other => Err(AppError::BadRequest(format!(
            "unsupported archive schema_version '{other}'"
        ))),
    }
}

pub fn verify_checksums(
    archive: &ContentArchive,
    files: &HashMap<String, Vec<u8>>,
) -> AppResult<()> {
    for fc in &archive.manifest.file_checksums {
        let Some(bytes) = files.get(&fc.path) else {
            return Err(AppError::BadRequest(format!(
                "missing file in archive: {}",
                fc.path
            )));
        };
        let actual = hex_sha256(bytes);
        if actual != fc.sha256 {
            return Err(AppError::BadRequest(format!(
                "checksum mismatch for {}",
                fc.path
            )));
        }
    }
    Ok(())
}

async fn export_one_project(
    pool: &PgPool,
    project_id: Uuid,
    include_attachments: bool,
    attachment_blobs: &mut Vec<(String, Vec<u8>)>,
) -> AppResult<ContentProjectBundle> {
    let project: (String, String, String, String, i32) = sqlx::query_as(
        r#"SELECT key, name, description, case_key_prefix, next_case_number
           FROM projects WHERE id = $1"#,
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;

    let case_folders = load_suite_tree(pool, project_id).await?;
    let requirement_folders = load_req_folder_tree(pool, project_id).await?;

    let tags: Vec<(String, String)> =
        sqlx::query_as("SELECT name, color FROM tags WHERE project_id = $1 ORDER BY name")
            .bind(project_id)
            .fetch_all(pool)
            .await?;
    let tags: Vec<ContentTag> = tags
        .into_iter()
        .map(|(name, color)| ContentTag { name, color })
        .collect();

    // Legacy per-project custom fields (main). Option-list / global defs filled when tables exist.
    let cf_rows: Vec<(Uuid, String, String, Value, bool, i32)> = sqlx::query_as(
        r#"SELECT id, name, field_type, options, required, sort_order
           FROM custom_fields WHERE project_id = $1 ORDER BY sort_order, name"#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut custom_fields = Vec::new();
    let mut cf_id_to_key: HashMap<Uuid, (String, String)> = HashMap::new();
    for (id, name, field_type, options, required, sort_order) in cf_rows {
        let key = slug_key(&name);
        cf_id_to_key.insert(id, (key.clone(), name.clone()));
        custom_fields.push(ContentCustomFieldDef {
            key,
            name,
            field_type,
            entity: "case".into(),
            options,
            required,
            sort_order,
            option_list_key: None,
            help_text: String::new(),
            active: true,
            scope: "project".into(),
        });
    }

    // Optional PR #24 tables — ignore if missing
    let option_lists = load_option_lists_if_present(pool).await.unwrap_or_default();
    if let Ok(defs) = load_global_custom_fields_if_present(pool, project_id).await {
        for d in defs {
            if !custom_fields.iter().any(|c| c.key == d.key) {
                custom_fields.push(d);
            }
        }
    }

    let suite_paths = suite_id_paths(pool, project_id).await?;

    let cases_raw: Vec<(
        Uuid,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<Uuid>,
        i32,
        i32,
    )> = sqlx::query_as(
        r#"SELECT id, key, title, description, precondition, priority, status,
                  external_key, suite_id, sort_order, version
           FROM test_cases WHERE project_id = $1 ORDER BY key"#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let mut cases = Vec::new();
    for (
        case_id,
        key,
        title,
        description,
        precondition,
        priority,
        status,
        external_key,
        suite_id,
        sort_order,
        version,
    ) in cases_raw
    {
        let steps: Vec<(i32, String, String, String)> = sqlx::query_as(
            r#"SELECT position, action, expected_result, data
               FROM test_steps WHERE case_id = $1 ORDER BY position"#,
        )
        .bind(case_id)
        .fetch_all(pool)
        .await?;
        let steps: Vec<ContentStep> = steps
            .into_iter()
            .map(|(position, action, expected_result, data)| ContentStep {
                position,
                action,
                expected_result,
                data,
            })
            .collect();

        let tag_names: Vec<String> = sqlx::query_scalar(
            r#"SELECT t.name FROM tags t
               JOIN case_tags ct ON ct.tag_id = t.id
               WHERE ct.case_id = $1 ORDER BY t.name"#,
        )
        .bind(case_id)
        .fetch_all(pool)
        .await?;

        let cf_vals: Vec<(Uuid, Value)> =
            sqlx::query_as("SELECT field_id, value FROM custom_field_values WHERE case_id = $1")
                .bind(case_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
        let custom_field_values: Vec<ContentFieldValue> = cf_vals
            .into_iter()
            .filter_map(|(fid, value)| {
                let (field_key, field_name) = cf_id_to_key.get(&fid)?.clone();
                Some(ContentFieldValue {
                    field_key,
                    field_name,
                    value,
                })
            })
            .collect();

        let versions: Vec<(i32, Value, String)> = sqlx::query_as(
            r#"SELECT version, snapshot, change_summary
               FROM case_versions WHERE case_id = $1 ORDER BY version"#,
        )
        .bind(case_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        let versions: Vec<ContentCaseVersion> = versions
            .into_iter()
            .map(|(version, snapshot, change_summary)| ContentCaseVersion {
                version,
                snapshot,
                change_summary,
            })
            .collect();

        let mut attachment_ids = Vec::new();
        if include_attachments {
            let atts: Vec<(Uuid, String, String, i64, Vec<u8>)> = sqlx::query_as(
                r#"SELECT id, filename, content_type, size_bytes, content
                   FROM attachments
                   WHERE project_id = $1 AND entity_type = 'case' AND entity_id = $2"#,
            )
            .bind(project_id)
            .bind(case_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            for (aid, filename, content_type, size_bytes, content) in atts {
                let id_str = aid.to_string();
                let path = format!("projects/{}/attachments/{id_str}_{filename}", project.0);
                attachment_blobs.push((path, content));
                attachment_ids.push(id_str);
                let _ = (content_type, size_bytes); // recorded in attachments.json below
            }
        }

        let folder_path = suite_id
            .and_then(|sid| suite_paths.get(&sid).cloned())
            .unwrap_or_default();

        cases.push(ContentCase {
            key,
            title,
            description,
            precondition,
            priority,
            status,
            external_key,
            folder_path,
            sort_order,
            version,
            steps,
            tags: tag_names,
            custom_fields: custom_field_values,
            versions,
            attachment_ids,
        });
    }

    let req_folder_paths = req_folder_id_paths(pool, project_id).await?;
    let reqs_raw: Vec<(
        Uuid,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        String,
        Option<Uuid>,
    )> = sqlx::query_as(
        r#"SELECT id, title, description, acceptance_criteria, external_key, external_id,
                  source, url, folder_id
           FROM requirements WHERE project_id = $1 ORDER BY created_at"#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let mut requirements = Vec::new();
    for (
        rid,
        title,
        description,
        acceptance_criteria,
        external_key,
        external_id,
        source,
        url,
        folder_id,
    ) in reqs_raw
    {
        let case_keys: Vec<String> = sqlx::query_scalar(
            r#"SELECT tc.key FROM test_cases tc
               JOIN case_requirements cr ON cr.case_id = tc.id
               WHERE cr.requirement_id = $1 ORDER BY tc.key"#,
        )
        .bind(rid)
        .fetch_all(pool)
        .await?;
        let folder_path = folder_id
            .and_then(|fid| req_folder_paths.get(&fid).cloned())
            .unwrap_or_default();
        requirements.push(ContentRequirement {
            title,
            description,
            acceptance_criteria,
            external_key,
            external_id,
            source,
            url,
            folder_path,
            case_keys,
            custom_fields: vec![],
        });
    }

    let tombstones: Vec<(String, i32)> = sqlx::query_as(
        "SELECT key, case_number FROM case_key_tombstones WHERE project_id = $1 ORDER BY case_number",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let tombstones: Vec<ContentTombstone> = tombstones
        .into_iter()
        .map(|(key, case_number)| ContentTombstone { key, case_number })
        .collect();

    let att_meta: Vec<ContentAttachmentMeta> = if include_attachments {
        sqlx::query_as::<_, (Uuid, String, String, i64, String, Uuid)>(
            r#"SELECT id, filename, content_type, size_bytes, entity_type, entity_id
               FROM attachments WHERE project_id = $1 AND entity_type = 'case'"#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(
            |(id, filename, content_type, size_bytes, entity_type, entity_id)| {
                // Resolve case key lazily from already-exported cases
                let entity_key = cases
                    .iter()
                    .find(|c| c.attachment_ids.contains(&id.to_string()))
                    .map(|c| c.key.clone());
                let _ = entity_id;
                ContentAttachmentMeta {
                    id: id.to_string(),
                    filename,
                    content_type,
                    size_bytes,
                    entity_type,
                    entity_key,
                }
            },
        )
        .collect()
    } else {
        vec![]
    };

    Ok(ContentProjectBundle {
        project: ContentArchiveProject {
            key: project.0,
            name: project.1,
            description: project.2,
            case_key_prefix: project.3,
            next_case_number: project.4,
        },
        case_folders,
        requirement_folders,
        tags,
        custom_fields,
        option_lists,
        cases,
        requirements,
        tombstones,
        attachments: att_meta,
    })
}

fn slug_key(name: &str) -> String {
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
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        "field".into()
    } else {
        s
    }
}

async fn load_suite_tree(pool: &PgPool, project_id: Uuid) -> AppResult<Vec<ContentFolderNode>> {
    let rows: Vec<(Uuid, Option<Uuid>, String, i32)> = sqlx::query_as(
        r#"SELECT id, parent_id, name, sort_order FROM suites
           WHERE project_id = $1 ORDER BY sort_order, name"#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(build_folder_tree(&rows))
}

async fn load_req_folder_tree(
    pool: &PgPool,
    project_id: Uuid,
) -> AppResult<Vec<ContentFolderNode>> {
    let rows: Vec<(Uuid, Option<Uuid>, String, i32)> = sqlx::query_as(
        r#"SELECT id, parent_id, name, sort_order FROM requirement_folders
           WHERE project_id = $1 ORDER BY sort_order, name"#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    Ok(build_folder_tree(&rows))
}

fn build_folder_tree(rows: &[(Uuid, Option<Uuid>, String, i32)]) -> Vec<ContentFolderNode> {
    fn children_of(
        parent: Option<Uuid>,
        rows: &[(Uuid, Option<Uuid>, String, i32)],
    ) -> Vec<ContentFolderNode> {
        rows.iter()
            .filter(|(_, p, _, _)| *p == parent)
            .map(|(id, _, name, sort_order)| ContentFolderNode {
                name: name.clone(),
                sort_order: *sort_order,
                children: children_of(Some(*id), rows),
            })
            .collect()
    }
    children_of(None, rows)
}

async fn suite_id_paths(pool: &PgPool, project_id: Uuid) -> AppResult<HashMap<Uuid, Vec<String>>> {
    let rows: Vec<(Uuid, Option<Uuid>, String)> =
        sqlx::query_as("SELECT id, parent_id, name FROM suites WHERE project_id = $1")
            .bind(project_id)
            .fetch_all(pool)
            .await?;
    let by_id: HashMap<Uuid, (Option<Uuid>, String)> = rows
        .into_iter()
        .map(|(id, parent, name)| (id, (parent, name)))
        .collect();
    let mut out = HashMap::new();
    for id in by_id.keys().copied() {
        let mut path = Vec::new();
        let mut cur = Some(id);
        while let Some(cid) = cur {
            if let Some((parent, name)) = by_id.get(&cid) {
                path.push(name.clone());
                cur = *parent;
            } else {
                break;
            }
        }
        path.reverse();
        out.insert(id, path);
    }
    Ok(out)
}

async fn req_folder_id_paths(
    pool: &PgPool,
    project_id: Uuid,
) -> AppResult<HashMap<Uuid, Vec<String>>> {
    let rows: Vec<(Uuid, Option<Uuid>, String)> =
        sqlx::query_as("SELECT id, parent_id, name FROM requirement_folders WHERE project_id = $1")
            .bind(project_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
    let by_id: HashMap<Uuid, (Option<Uuid>, String)> = rows
        .into_iter()
        .map(|(id, parent, name)| (id, (parent, name)))
        .collect();
    let mut out = HashMap::new();
    for id in by_id.keys().copied() {
        let mut path = Vec::new();
        let mut cur = Some(id);
        while let Some(cid) = cur {
            if let Some((parent, name)) = by_id.get(&cid) {
                path.push(name.clone());
                cur = *parent;
            } else {
                break;
            }
        }
        path.reverse();
        out.insert(id, path);
    }
    Ok(out)
}

async fn load_option_lists_if_present(pool: &PgPool) -> AppResult<Vec<ContentOptionList>> {
    // Detect table
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
             SELECT 1 FROM information_schema.tables
             WHERE table_schema = 'public' AND table_name = 'option_lists'
           )"#,
    )
    .fetch_one(pool)
    .await?;
    if !exists {
        return Ok(vec![]);
    }
    let lists: Vec<(Uuid, String, String, String, bool)> =
        sqlx::query_as("SELECT id, key, name, description, active FROM option_lists ORDER BY key")
            .fetch_all(pool)
            .await?;
    let mut out = Vec::new();
    for (id, key, name, description, active) in lists {
        let items: Vec<(
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            i32,
            bool,
            bool,
        )> = sqlx::query_as(
            r#"SELECT key, label, value, colour, sort_order, active, deprecated
               FROM option_list_items WHERE list_id = $1 ORDER BY sort_order"#,
        )
        .bind(id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        out.push(ContentOptionList {
            key,
            name,
            description,
            active,
            items: items
                .into_iter()
                .map(
                    |(key, label, value, colour, sort_order, active, deprecated)| {
                        crate::models::ContentOptionListItem {
                            key,
                            label,
                            value,
                            colour,
                            sort_order,
                            active,
                            deprecated,
                        }
                    },
                )
                .collect(),
        });
    }
    Ok(out)
}

async fn load_global_custom_fields_if_present(
    pool: &PgPool,
    project_id: Uuid,
) -> AppResult<Vec<ContentCustomFieldDef>> {
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
             SELECT 1 FROM information_schema.tables
             WHERE table_schema = 'public' AND table_name = 'custom_field_definitions'
           )"#,
    )
    .fetch_one(pool)
    .await?;
    if !exists {
        return Ok(vec![]);
    }
    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        Value,
        bool,
        String,
        i32,
        bool,
        String,
    )> = sqlx::query_as(
        r#"SELECT d.key, d.name, d.field_type, d.entity,
                  ol.key, d.default_value, d.required, d.help_text, d.sort_order, d.active, d.scope
           FROM custom_field_definitions d
           LEFT JOIN option_lists ol ON ol.id = d.option_list_id
           WHERE d.active = TRUE
             AND (d.scope = 'all' OR EXISTS (
                   SELECT 1 FROM custom_field_projects cfp
                   WHERE cfp.definition_id = d.id AND cfp.project_id = $1
                 ))
           ORDER BY d.sort_order, d.key"#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    Ok(rows
        .into_iter()
        .map(
            |(
                key,
                name,
                field_type,
                entity,
                option_list_key,
                default_value,
                required,
                help_text,
                sort_order,
                active,
                scope,
            )| ContentCustomFieldDef {
                key,
                name,
                field_type,
                entity,
                options: default_value,
                required,
                sort_order,
                option_list_key,
                help_text,
                active,
                scope,
            },
        )
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ContentArchiveProject;

    #[test]
    fn zip_roundtrip_archive_json() {
        let archive = ContentArchive {
            manifest: ContentArchiveManifest {
                schema_version: CONTENT_ARCHIVE_SCHEMA_VERSION.into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                producer: "test".into(),
                scope: "project".into(),
                project_keys: vec!["DEMO".into()],
                include_attachments: false,
                pack_mode: "zip".into(),
                file_checksums: vec![],
            },
            option_lists: vec![],
            custom_fields: vec![],
            projects: vec![ContentProjectBundle {
                project: ContentArchiveProject {
                    key: "DEMO".into(),
                    name: "Demo".into(),
                    description: "".into(),
                    case_key_prefix: "DEMO".into(),
                    next_case_number: 1,
                },
                case_folders: vec![],
                requirement_folders: vec![],
                tags: vec![],
                custom_fields: vec![],
                option_lists: vec![],
                cases: vec![],
                requirements: vec![],
                tombstones: vec![],
                attachments: vec![],
            }],
        };
        let json = serde_json::to_vec_pretty(&archive).unwrap();
        let zip = zip_files(&[("archive.json".into(), json)]).unwrap();
        let parsed = parse_archive_bytes(&zip).unwrap();
        assert_eq!(parsed.projects[0].project.key, "DEMO");
    }
}

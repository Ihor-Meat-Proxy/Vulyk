//! Application-level content backup orchestration.

#![allow(clippy::type_complexity)]

pub mod archive;
pub mod destinations;
pub mod import;

use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto::SecretsKey;
use crate::error::{AppError, AppResult};
use crate::models::{
    BackupDestination, BackupPolicy, BackupRun, BackupTestResult, CreateBackupDestination,
    CreateBackupPolicy, ManualBackupRequest, RestorePreview, RestoreRequest,
    UpdateBackupDestination, UpdateBackupPolicy,
};

/// Postgres advisory lock key for the backup scheduler (stable hash).
pub const SCHEDULER_LOCK_KEY: i64 = 0x5655_4c59_4b42_4b31; // "VULYBK1"

const DEST_SELECT: &str = r#"id, name, kind, config, enabled, created_at, updated_at,
    (secrets_ciphertext IS NOT NULL AND length(secrets_ciphertext) > 0) AS has_secrets"#;

const POLICY_SELECT: &str = r#"id, project_id, destination_id, schedule, debounce_minutes,
    retain_count, retain_days, pack_mode, include_attachments, enabled,
    last_scheduled_at, created_at, updated_at"#;

const RUN_SELECT: &str = r#"id, policy_id, destination_id, project_id, status, trigger,
    remote_path, local_path, size_bytes, duration_ms, error, summary, checksum_sha256,
    started_at, finished_at, created_at"#;

// ── Destinations ────────────────────────────────────────────────────────────

pub async fn list_destinations(pool: &PgPool) -> AppResult<Vec<BackupDestination>> {
    let rows = sqlx::query_as::<_, BackupDestination>(&format!(
        "SELECT {DEST_SELECT} FROM backup_destinations ORDER BY name"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_destination(pool: &PgPool, id: Uuid) -> AppResult<BackupDestination> {
    sqlx::query_as::<_, BackupDestination>(&format!(
        "SELECT {DEST_SELECT} FROM backup_destinations WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("backup destination".into()))
}

pub async fn create_destination(
    pool: &PgPool,
    secrets: &SecretsKey,
    body: CreateBackupDestination,
) -> AppResult<BackupDestination> {
    validate_kind(&body.kind)?;
    let (ct, nonce) = encrypt_secrets(secrets, body.secrets.as_ref())?;
    let row = sqlx::query_as::<_, BackupDestination>(&format!(
        r#"INSERT INTO backup_destinations (name, kind, config, secrets_ciphertext, secrets_nonce, enabled)
           VALUES ($1,$2,$3,$4,$5,$6)
           RETURNING {DEST_SELECT}"#
    ))
    .bind(body.name.trim())
    .bind(body.kind.trim())
    .bind(if body.config.is_null() {
        json!({})
    } else {
        body.config
    })
    .bind(ct)
    .bind(nonce)
    .bind(body.enabled)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update_destination(
    pool: &PgPool,
    secrets: &SecretsKey,
    id: Uuid,
    body: UpdateBackupDestination,
) -> AppResult<BackupDestination> {
    let current = get_destination(pool, id).await?;
    let name = body.name.unwrap_or(current.name);
    let config = body.config.unwrap_or(current.config);
    let enabled = body.enabled.unwrap_or(current.enabled);

    if let Some(sec) = body.secrets {
        let (ct, nonce) = if sec.as_object().map(|o| o.is_empty()).unwrap_or(false) {
            (None, None)
        } else {
            encrypt_secrets(secrets, Some(&sec))?
        };
        sqlx::query(
            r#"UPDATE backup_destinations
               SET name=$2, config=$3, enabled=$4,
                   secrets_ciphertext=$5, secrets_nonce=$6, updated_at=NOW()
               WHERE id=$1"#,
        )
        .bind(id)
        .bind(&name)
        .bind(&config)
        .bind(enabled)
        .bind(ct)
        .bind(nonce)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            r#"UPDATE backup_destinations
               SET name=$2, config=$3, enabled=$4, updated_at=NOW()
               WHERE id=$1"#,
        )
        .bind(id)
        .bind(&name)
        .bind(&config)
        .bind(enabled)
        .execute(pool)
        .await?;
    }
    get_destination(pool, id).await
}

pub async fn delete_destination(pool: &PgPool, id: Uuid) -> AppResult<()> {
    let res = sqlx::query("DELETE FROM backup_destinations WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("backup destination".into()));
    }
    Ok(())
}

pub async fn test_destination(
    pool: &PgPool,
    secrets: &SecretsKey,
    id: Uuid,
) -> AppResult<BackupTestResult> {
    match destinations::build_destination(pool, secrets, id).await {
        Ok(dest) => match dest.test_connection().await {
            Ok(()) => Ok(BackupTestResult {
                ok: true,
                message: format!("{} connection ok", dest.kind()),
            }),
            Err(e) => Ok(BackupTestResult {
                ok: false,
                message: e.to_string(),
            }),
        },
        Err(e) => Ok(BackupTestResult {
            ok: false,
            message: e.to_string(),
        }),
    }
}

fn validate_kind(kind: &str) -> AppResult<()> {
    match kind {
        "local" | "github" | "gitlab" | "nextcloud" | "gdrive" => Ok(()),
        other => Err(AppError::BadRequest(format!("unsupported kind '{other}'"))),
    }
}

fn encrypt_secrets(
    secrets: &SecretsKey,
    value: Option<&Value>,
) -> AppResult<(Option<Vec<u8>>, Option<Vec<u8>>)> {
    match value {
        None => Ok((None, None)),
        Some(v) if v.is_null() => Ok((None, None)),
        Some(v) => {
            let plain = serde_json::to_string(v)
                .map_err(|e| AppError::BadRequest(format!("secrets json: {e}")))?;
            let (ct, nonce) = secrets.encrypt(&plain)?;
            Ok((Some(ct), Some(nonce)))
        }
    }
}

// ── Policies ────────────────────────────────────────────────────────────────

pub async fn list_policies(pool: &PgPool) -> AppResult<Vec<BackupPolicy>> {
    Ok(sqlx::query_as::<_, BackupPolicy>(&format!(
        "SELECT {POLICY_SELECT} FROM backup_policies ORDER BY created_at"
    ))
    .fetch_all(pool)
    .await?)
}

pub async fn create_policy(pool: &PgPool, body: CreateBackupPolicy) -> AppResult<BackupPolicy> {
    validate_schedule(&body.schedule)?;
    validate_pack(&body.pack_mode)?;
    let _ = get_destination(pool, body.destination_id).await?;
    Ok(sqlx::query_as::<_, BackupPolicy>(&format!(
        r#"INSERT INTO backup_policies
           (project_id, destination_id, schedule, debounce_minutes, retain_count, retain_days,
            pack_mode, include_attachments, enabled)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
           RETURNING {POLICY_SELECT}"#
    ))
    .bind(body.project_id)
    .bind(body.destination_id)
    .bind(body.schedule)
    .bind(body.debounce_minutes)
    .bind(body.retain_count)
    .bind(body.retain_days)
    .bind(body.pack_mode)
    .bind(body.include_attachments)
    .bind(body.enabled)
    .fetch_one(pool)
    .await?)
}

pub async fn update_policy(
    pool: &PgPool,
    id: Uuid,
    body: UpdateBackupPolicy,
) -> AppResult<BackupPolicy> {
    let current: BackupPolicy = sqlx::query_as(&format!(
        "SELECT {POLICY_SELECT} FROM backup_policies WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("backup policy".into()))?;

    if let Some(ref s) = body.schedule {
        validate_schedule(s)?;
    }
    if let Some(ref p) = body.pack_mode {
        validate_pack(p)?;
    }

    let project_id = if body.clear_project {
        None
    } else {
        body.project_id.or(current.project_id)
    };

    Ok(sqlx::query_as::<_, BackupPolicy>(&format!(
        r#"UPDATE backup_policies SET
             project_id = $2,
             destination_id = COALESCE($3, destination_id),
             schedule = COALESCE($4, schedule),
             debounce_minutes = COALESCE($5, debounce_minutes),
             retain_count = COALESCE($6, retain_count),
             retain_days = COALESCE($7, retain_days),
             pack_mode = COALESCE($8, pack_mode),
             include_attachments = COALESCE($9, include_attachments),
             enabled = COALESCE($10, enabled),
             updated_at = NOW()
           WHERE id = $1
           RETURNING {POLICY_SELECT}"#
    ))
    .bind(id)
    .bind(project_id)
    .bind(body.destination_id)
    .bind(body.schedule)
    .bind(body.debounce_minutes)
    .bind(body.retain_count)
    .bind(body.retain_days)
    .bind(body.pack_mode)
    .bind(body.include_attachments)
    .bind(body.enabled)
    .fetch_one(pool)
    .await?)
}

pub async fn delete_policy(pool: &PgPool, id: Uuid) -> AppResult<()> {
    let res = sqlx::query("DELETE FROM backup_policies WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("backup policy".into()));
    }
    Ok(())
}

fn validate_schedule(s: &str) -> AppResult<()> {
    match s {
        "hourly" | "daily" | "manual_only" => Ok(()),
        other => Err(AppError::BadRequest(format!("invalid schedule '{other}'"))),
    }
}

fn validate_pack(s: &str) -> AppResult<()> {
    match s {
        "zip" | "unpacked" => Ok(()),
        other => Err(AppError::BadRequest(format!("invalid pack_mode '{other}'"))),
    }
}

// ── Runs ────────────────────────────────────────────────────────────────────

pub async fn list_runs(
    pool: &PgPool,
    project_id: Option<Uuid>,
    limit: i64,
) -> AppResult<Vec<BackupRun>> {
    let limit = limit.clamp(1, 200);
    if let Some(pid) = project_id {
        Ok(sqlx::query_as::<_, BackupRun>(&format!(
            "SELECT {RUN_SELECT} FROM backup_runs WHERE project_id = $1 OR project_id IS NULL
             ORDER BY started_at DESC LIMIT $2"
        ))
        .bind(pid)
        .bind(limit)
        .fetch_all(pool)
        .await?)
    } else {
        Ok(sqlx::query_as::<_, BackupRun>(&format!(
            "SELECT {RUN_SELECT} FROM backup_runs ORDER BY started_at DESC LIMIT $1"
        ))
        .bind(limit)
        .fetch_all(pool)
        .await?)
    }
}

pub async fn get_run(pool: &PgPool, id: Uuid) -> AppResult<BackupRun> {
    sqlx::query_as::<_, BackupRun>(&format!(
        "SELECT {RUN_SELECT} FROM backup_runs WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("backup run".into()))
}

pub async fn run_manual(
    pool: &PgPool,
    secrets: &SecretsKey,
    req: ManualBackupRequest,
) -> AppResult<BackupRun> {
    validate_pack(&req.pack_mode)?;
    let _ = get_destination(pool, req.destination_id).await?;
    execute_backup(
        pool,
        secrets,
        None,
        req.destination_id,
        req.project_id,
        "manual",
        &req.pack_mode,
        req.include_attachments,
        None,
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_backup(
    pool: &PgPool,
    secrets: &SecretsKey,
    policy_id: Option<Uuid>,
    destination_id: Uuid,
    project_id: Option<Uuid>,
    trigger: &str,
    pack_mode: &str,
    include_attachments: bool,
    retain_count: Option<i32>,
    retain_days: Option<i32>,
) -> AppResult<BackupRun> {
    let run_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO backup_runs
           (policy_id, destination_id, project_id, status, trigger, started_at)
           VALUES ($1,$2,$3,'running',$4,NOW())
           RETURNING id"#,
    )
    .bind(policy_id)
    .bind(destination_id)
    .bind(project_id)
    .bind(trigger)
    .fetch_one(pool)
    .await?;

    let started = std::time::Instant::now();
    let result = do_backup(
        pool,
        secrets,
        destination_id,
        project_id,
        pack_mode,
        include_attachments,
        retain_count,
        retain_days,
    )
    .await;

    let duration_ms = started.elapsed().as_millis() as i32;
    match result {
        Ok((remote_path, size_bytes, checksum, summary)) => {
            sqlx::query(
                r#"UPDATE backup_runs SET
                     status='success', remote_path=$2, local_path=$2, size_bytes=$3,
                     duration_ms=$4, checksum_sha256=$5, summary=$6, finished_at=NOW()
                   WHERE id=$1"#,
            )
            .bind(run_id)
            .bind(&remote_path)
            .bind(size_bytes)
            .bind(duration_ms)
            .bind(&checksum)
            .bind(summary)
            .execute(pool)
            .await?;
            if let Some(pid) = project_id {
                advance_watermark(pool, pid).await?;
            } else {
                let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM projects")
                    .fetch_all(pool)
                    .await?;
                for pid in ids {
                    advance_watermark(pool, pid).await?;
                }
            }
        }
        Err(err) => {
            let msg = err.to_string();
            sqlx::query(
                r#"UPDATE backup_runs SET
                     status='failed', error=$2, duration_ms=$3, finished_at=NOW()
                   WHERE id=$1"#,
            )
            .bind(run_id)
            .bind(&msg)
            .bind(duration_ms)
            .execute(pool)
            .await?;
            notify_admins_failure(pool, &msg, run_id).await?;
        }
    }
    get_run(pool, run_id).await
}

#[allow(clippy::too_many_arguments)]
async fn do_backup(
    pool: &PgPool,
    secrets: &SecretsKey,
    destination_id: Uuid,
    project_id: Option<Uuid>,
    pack_mode: &str,
    include_attachments: bool,
    retain_count: Option<i32>,
    retain_days: Option<i32>,
) -> AppResult<(String, i64, String, Value)> {
    let project_ids: Vec<Uuid> = if let Some(pid) = project_id {
        vec![pid]
    } else {
        sqlx::query_scalar("SELECT id FROM projects ORDER BY key")
            .fetch_all(pool)
            .await?
    };
    if project_ids.is_empty() {
        return Err(AppError::BadRequest("no projects to back up".into()));
    }

    let built = archive::export_projects(pool, &project_ids, include_attachments).await?;
    let dest = destinations::build_destination(pool, secrets, destination_id).await?;
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let scope = if project_ids.len() == 1 {
        let key: String = sqlx::query_scalar("SELECT key FROM projects WHERE id = $1")
            .bind(project_ids[0])
            .fetch_one(pool)
            .await?;
        key
    } else {
        "all".into()
    };

    let remote_path = if pack_mode == "unpacked" {
        let root = format!("vulyk-content/{scope}/{stamp}");
        dest.put_tree(&root, &built.files).await?;
        root
    } else {
        let path = format!("vulyk-content/{scope}/{stamp}.zip");
        dest.put_file(&path, &built.zip_bytes).await?;
        path
    };

    prune_retention(
        pool,
        secrets,
        destination_id,
        &scope,
        retain_count,
        retain_days,
    )
    .await?;

    let summary = json!({
        "projects": built.archive.manifest.project_keys,
        "cases": built.archive.projects.iter().map(|p| p.cases.len()).sum::<usize>(),
        "requirements": built.archive.projects.iter().map(|p| p.requirements.len()).sum::<usize>(),
        "schema_version": built.archive.manifest.schema_version,
        "pack_mode": pack_mode,
    });

    Ok((
        remote_path,
        built.size_bytes,
        built.checksum_sha256,
        summary,
    ))
}

async fn prune_retention(
    pool: &PgPool,
    secrets: &SecretsKey,
    destination_id: Uuid,
    scope: &str,
    retain_count: Option<i32>,
    retain_days: Option<i32>,
) -> AppResult<()> {
    if retain_count.is_none() && retain_days.is_none() {
        return Ok(());
    }
    let dest = destinations::build_destination(pool, secrets, destination_id).await?;
    let prefix = format!("vulyk-content/{scope}/");
    let mut objects = dest.list(&prefix).await.unwrap_or_default();
    objects.sort_by(|a, b| b.path.cmp(&a.path));

    if let Some(days) = retain_days {
        let cutoff = Utc::now() - Duration::days(days as i64);
        for obj in &objects {
            // path contains timestamp like 20261026T120000Z
            if let Some(stamp) = obj.path.rsplit('/').next() {
                let stamp = stamp.trim_end_matches(".zip");
                if let Ok(ts) = chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%dT%H%M%SZ") {
                    let ts = ts.and_utc();
                    if ts < cutoff {
                        let _ = dest.delete(&obj.path).await;
                    }
                }
            }
        }
    }

    if let Some(count) = retain_count {
        if count >= 0 && objects.len() > count as usize {
            for obj in objects.iter().skip(count as usize) {
                let _ = dest.delete(&obj.path).await;
            }
        }
    }

    // Also prune DB run rows beyond retain_count for this destination+scope
    if let Some(count) = retain_count {
        sqlx::query(
            r#"DELETE FROM backup_runs WHERE id IN (
                 SELECT id FROM backup_runs
                 WHERE destination_id = $1 AND status = 'success'
                   AND remote_path LIKE $2
                 ORDER BY started_at DESC
                 OFFSET $3
               )"#,
        )
        .bind(destination_id)
        .bind(format!("{prefix}%"))
        .bind(count)
        .execute(pool)
        .await
        .ok();
    }
    Ok(())
}

async fn advance_watermark(pool: &PgPool, project_id: Uuid) -> AppResult<()> {
    let cases_at: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT MAX(updated_at) FROM test_cases WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await?;
    let reqs_at: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT MAX(updated_at) FROM requirements WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await?;
    sqlx::query(
        r#"INSERT INTO backup_change_watermarks
             (project_id, cases_updated_at, requirements_updated_at, last_success_at, content_seq, updated_at)
           VALUES ($1, $2, $3, NOW(), 1, NOW())
           ON CONFLICT (project_id) DO UPDATE SET
             cases_updated_at = EXCLUDED.cases_updated_at,
             requirements_updated_at = EXCLUDED.requirements_updated_at,
             last_success_at = NOW(),
             content_seq = backup_change_watermarks.content_seq + 1,
             updated_at = NOW()"#,
    )
    .bind(project_id)
    .bind(cases_at)
    .bind(reqs_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn project_is_dirty(pool: &PgPool, project_id: Uuid) -> AppResult<bool> {
    let wm: Option<(
        Option<chrono::DateTime<Utc>>,
        Option<chrono::DateTime<Utc>>,
        Option<chrono::DateTime<Utc>>,
    )> = sqlx::query_as(
        r#"SELECT cases_updated_at, requirements_updated_at, last_success_at
           FROM backup_change_watermarks WHERE project_id = $1"#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let cases_at: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT MAX(updated_at) FROM test_cases WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await?;
    let reqs_at: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT MAX(updated_at) FROM requirements WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await?;

    let Some((_wc, _wr, last_success)) = wm else {
        // Never backed up — dirty if any content exists
        return Ok(cases_at.is_some() || reqs_at.is_some());
    };

    let last = last_success.unwrap_or(chrono::DateTime::<Utc>::UNIX_EPOCH);
    let dirty_cases = cases_at.map(|t| t > last).unwrap_or(false);
    let dirty_reqs = reqs_at.map(|t| t > last).unwrap_or(false);
    Ok(dirty_cases || dirty_reqs)
}

pub async fn download_run_bytes(
    pool: &PgPool,
    secrets: &SecretsKey,
    run_id: Uuid,
) -> AppResult<(String, Vec<u8>)> {
    let run = get_run(pool, run_id).await?;
    if run.status != "success" {
        return Err(AppError::BadRequest(
            "run is not a successful backup".into(),
        ));
    }
    let dest_id = run
        .destination_id
        .ok_or_else(|| AppError::BadRequest("run has no destination".into()))?;
    let path = run
        .remote_path
        .ok_or_else(|| AppError::BadRequest("run has no remote_path".into()))?;
    let dest = destinations::build_destination(pool, secrets, dest_id).await?;
    if path.ends_with(".zip") {
        let bytes = dest.get_file(&path).await?;
        let name = path.rsplit('/').next().unwrap_or("backup.zip").to_string();
        Ok((name, bytes))
    } else {
        // Unpacked — rebuild zip from archive.json if present
        let archive_path = format!("{path}/archive.json");
        let bytes = dest.get_file(&archive_path).await?;
        let zip = archive::zip_files(&[("archive.json".into(), bytes)])?;
        Ok(("backup.zip".into(), zip))
    }
}

pub async fn restore(
    pool: &PgPool,
    secrets: &SecretsKey,
    req: RestoreRequest,
) -> AppResult<RestorePreview> {
    let archive = if let Some(ref a) = req.archive {
        a.clone()
    } else if let Some(run_id) = req.run_id {
        let (_name, bytes) = download_run_bytes(pool, secrets, run_id).await?;
        archive::parse_archive_bytes(&bytes)?
    } else {
        return Err(AppError::BadRequest("archive or run_id required".into()));
    };
    import::preview_or_apply(pool, &archive, &req).await
}

async fn notify_admins_failure(pool: &PgPool, message: &str, run_id: Uuid) -> AppResult<()> {
    let admins: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE is_system_admin = TRUE")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    for uid in admins {
        sqlx::query(
            r#"INSERT INTO notifications (user_id, kind, title, body, payload)
               VALUES ($1, 'backup_failed', 'Content backup failed', $2, $3)"#,
        )
        .bind(uid)
        .bind(message)
        .bind(json!({ "run_id": run_id }))
        .execute(pool)
        .await
        .ok();
    }
    Ok(())
}

/// Scheduler tick: advisory lock + due dirty policies.
pub async fn scheduler_tick(pool: &PgPool, secrets: &SecretsKey) -> AppResult<usize> {
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(SCHEDULER_LOCK_KEY)
        .fetch_one(pool)
        .await?;
    if !locked {
        return Ok(0);
    }

    let result = scheduler_tick_inner(pool, secrets).await;

    let _: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
        .bind(SCHEDULER_LOCK_KEY)
        .fetch_one(pool)
        .await
        .unwrap_or(true);

    result
}

async fn scheduler_tick_inner(pool: &PgPool, secrets: &SecretsKey) -> AppResult<usize> {
    let policies: Vec<BackupPolicy> = sqlx::query_as(&format!(
        "SELECT {POLICY_SELECT} FROM backup_policies WHERE enabled = TRUE AND schedule <> 'manual_only'"
    ))
    .fetch_all(pool)
    .await?;

    let mut ran = 0usize;
    let now = Utc::now();

    for policy in policies {
        let interval = match policy.schedule.as_str() {
            "hourly" => Duration::hours(1),
            "daily" => Duration::days(1),
            _ => continue,
        };
        if let Some(last) = policy.last_scheduled_at {
            if now - last < interval {
                continue;
            }
        }

        let project_ids: Vec<Uuid> = if let Some(pid) = policy.project_id {
            vec![pid]
        } else {
            sqlx::query_scalar("SELECT id FROM projects")
                .fetch_all(pool)
                .await?
        };

        let mut any_dirty = false;
        let mut debounce_ok = true;
        for pid in &project_ids {
            if project_is_dirty(pool, *pid).await? {
                any_dirty = true;
                // Debounce: newest update must be older than debounce_minutes
                let newest: Option<chrono::DateTime<Utc>> = sqlx::query_scalar(
                    r#"SELECT GREATEST(
                         (SELECT MAX(updated_at) FROM test_cases WHERE project_id = $1),
                         (SELECT MAX(updated_at) FROM requirements WHERE project_id = $1)
                       )"#,
                )
                .bind(pid)
                .fetch_one(pool)
                .await?;
                if let Some(t) = newest {
                    if now - t < Duration::minutes(policy.debounce_minutes as i64) {
                        debounce_ok = false;
                    }
                }
            }
        }

        sqlx::query("UPDATE backup_policies SET last_scheduled_at = NOW() WHERE id = $1")
            .bind(policy.id)
            .execute(pool)
            .await?;

        if !any_dirty {
            sqlx::query(
                r#"INSERT INTO backup_runs
                   (policy_id, destination_id, project_id, status, trigger, summary, finished_at)
                   VALUES ($1,$2,$3,'skipped','schedule', '{"reason":"no_changes"}'::jsonb, NOW())"#,
            )
            .bind(policy.id)
            .bind(policy.destination_id)
            .bind(policy.project_id)
            .execute(pool)
            .await?;
            continue;
        }
        if !debounce_ok {
            continue;
        }

        let _ = execute_backup(
            pool,
            secrets,
            Some(policy.id),
            policy.destination_id,
            policy.project_id,
            "schedule",
            &policy.pack_mode,
            policy.include_attachments,
            policy.retain_count,
            policy.retain_days,
        )
        .await?;
        ran += 1;
    }
    Ok(ran)
}

pub fn spawn_scheduler(pool: PgPool, secrets: SecretsKey) {
    tokio::spawn(async move {
        // Initial delay so migrations/seed settle
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        loop {
            match scheduler_tick(&pool, &secrets).await {
                Ok(n) if n > 0 => tracing::info!(ran = n, "content backup scheduler"),
                Ok(_) => {}
                Err(e) => tracing::warn!(error = %e, "content backup scheduler tick failed"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
        }
    });
}

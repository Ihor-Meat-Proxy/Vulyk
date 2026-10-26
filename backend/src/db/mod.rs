use sqlx::migrate::{Migrate, MigrateError, Migrator};
use sqlx::{postgres::PgPoolOptions, PgConnection, PgPool};

/// Advisory lock key for Vulyk schema migrations (stable across replicas).
/// SQLx's built-in migrator also locks; we take this lock around status/run
/// so one-shot upgrade Jobs and replicas coordinate explicitly.
const MIGRATE_LOCK_KEY: i64 = 0x5655_4c59_4b4d; // "VULYK_M" truncated

/// Application semver from Cargo.toml (`vulyk-api`).
pub fn crate_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn migrator() -> Migrator {
    sqlx::migrate!("./migrations")
}

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

/// Whether the API should apply pending migrations on startup.
///
/// Defaults to **true** (dev / local Compose). Production Compose and Helm set
/// `VULYK_AUTO_MIGRATE=false` so `install.sh upgrade` / the Helm migrate Job
/// run migrations as an explicit step before serving traffic.
pub fn auto_migrate_enabled() -> bool {
    match std::env::var("VULYK_AUTO_MIGRATE") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            !(t == "0" || t == "false" || t == "no" || t == "off")
        }
        Err(_) => true,
    }
}

#[derive(Debug, Clone)]
pub struct MigrationInfo {
    pub version: i64,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct MigrationStatus {
    pub applied: Vec<MigrationInfo>,
    pub pending: Vec<MigrationInfo>,
    /// Applied versions that are not present in this binary (downgrade / skew).
    pub unknown_applied: Vec<i64>,
}

impl MigrationStatus {
    pub fn is_up_to_date(&self) -> bool {
        self.pending.is_empty() && self.unknown_applied.is_empty()
    }

    pub fn schema_newer_than_binary(&self) -> bool {
        !self.unknown_applied.is_empty()
    }
}

async fn acquire_migrate_lock(conn: &mut PgConnection) -> Result<(), MigrateError> {
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(MIGRATE_LOCK_KEY)
        .execute(&mut *conn)
        .await
        .map_err(MigrateError::Execute)?;
    Ok(())
}

async fn release_migrate_lock(conn: &mut PgConnection) {
    if let Err(e) = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(MIGRATE_LOCK_KEY)
        .execute(&mut *conn)
        .await
    {
        tracing::warn!(error = %e, "failed to release migration advisory lock");
    }
}

async fn migration_status_on_conn(
    conn: &mut PgConnection,
    migrator: &Migrator,
) -> Result<MigrationStatus, MigrateError> {
    conn.ensure_migrations_table().await?;
    let applied_raw = conn.list_applied_migrations().await?;

    let known: std::collections::HashSet<i64> = migrator.iter().map(|m| m.version).collect();
    let applied_versions: std::collections::HashSet<i64> =
        applied_raw.iter().map(|m| m.version).collect();

    let mut unknown_applied: Vec<i64> = applied_raw
        .iter()
        .filter(|m| !known.contains(&m.version))
        .map(|m| m.version)
        .collect();
    unknown_applied.sort_unstable();

    let description_by_version: std::collections::HashMap<i64, String> = migrator
        .iter()
        .map(|m| (m.version, m.description.to_string()))
        .collect();

    let mut applied: Vec<MigrationInfo> = applied_raw
        .into_iter()
        .map(|m| MigrationInfo {
            version: m.version,
            description: description_by_version
                .get(&m.version)
                .cloned()
                .unwrap_or_else(|| "(unknown)".into()),
        })
        .collect();
    applied.sort_by_key(|m| m.version);

    let mut pending: Vec<MigrationInfo> = migrator
        .iter()
        .filter(|m| m.migration_type.is_up_migration())
        .filter(|m| !applied_versions.contains(&m.version))
        .map(|m| MigrationInfo {
            version: m.version,
            description: m.description.to_string(),
        })
        .collect();
    pending.sort_by_key(|m| m.version);

    Ok(MigrationStatus {
        applied,
        pending,
        unknown_applied,
    })
}

/// Read applied + pending migrations (holds advisory lock briefly).
pub async fn migration_status(pool: &PgPool) -> Result<MigrationStatus, MigrateError> {
    let migrator = migrator();
    let mut conn = pool.acquire().await.map_err(MigrateError::Execute)?;
    acquire_migrate_lock(&mut conn).await?;
    let result = migration_status_on_conn(&mut conn, &migrator).await;
    release_migrate_lock(&mut conn).await;
    result
}

/// Apply pending migrations under an advisory lock. Logs each applied version.
pub async fn migrate(pool: &PgPool) -> Result<(), MigrateError> {
    let status = migration_status(pool).await?;
    if status.schema_newer_than_binary() {
        return Err(MigrateError::VersionMissing(status.unknown_applied[0]));
    }

    if status.pending.is_empty() {
        tracing::info!(
            applied = status.applied.len(),
            "database schema is up to date"
        );
        return Ok(());
    }

    tracing::info!(
        pending = status.pending.len(),
        "applying database migrations"
    );
    for m in &status.pending {
        tracing::info!(version = m.version, description = %m.description, "pending migration");
    }

    // SQLx Migrator also takes a Postgres advisory lock (locking = true by default).
    let migrator = migrator();
    migrator.run(pool).await?;

    let after = migration_status(pool).await?;
    for m in &after.applied {
        tracing::info!(
            version = m.version,
            description = %m.description,
            "applied migration"
        );
    }
    tracing::info!(applied = after.applied.len(), "migrations complete");
    Ok(())
}

/// Refuse to serve if the database schema is newer than this binary.
pub async fn assert_schema_compatible(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let status = migration_status(pool).await?;
    if status.schema_newer_than_binary() {
        let versions: Vec<String> = status
            .unknown_applied
            .iter()
            .map(|v| v.to_string())
            .collect();
        return Err(format!(
            "database schema is newer than this binary (unknown applied migration version(s): {}). \
             Refusing to start — deploy a matching or newer Vulyk API, or restore from backup. \
             See docs/guides/upgrade.md",
            versions.join(", ")
        )
        .into());
    }
    Ok(())
}

/// When auto-migrate is off, refuse to start if migrations are still pending.
pub async fn assert_no_pending_migrations(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let status = migration_status(pool).await?;
    if !status.pending.is_empty() {
        let pending: Vec<String> = status
            .pending
            .iter()
            .map(|m| format!("{} ({})", m.version, m.description))
            .collect();
        return Err(format!(
            "pending database migrations (VULYK_AUTO_MIGRATE=false): {}. \
             Run `vulyk-api migrate run` (or ./scripts/install.sh upgrade) before starting the API.",
            pending.join(", ")
        )
        .into());
    }
    Ok(())
}

/// Startup path: optional auto-migrate, then compatibility / pending checks, log applied set.
pub async fn prepare_schema(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    assert_schema_compatible(pool).await?;

    if auto_migrate_enabled() {
        migrate(pool).await?;
    } else {
        assert_no_pending_migrations(pool).await?;
        tracing::info!("VULYK_AUTO_MIGRATE=false; skipping automatic migrations");
    }

    let status = migration_status(pool).await?;
    tracing::info!(
        version = crate_version(),
        applied = status.applied.len(),
        "schema ready"
    );
    for m in &status.applied {
        tracing::debug!(
            version = m.version,
            description = %m.description,
            "migration present"
        );
    }
    Ok(())
}

/// Human-readable status for the CLI (`vulyk-api migrate status`).
pub fn format_migration_status(status: &MigrationStatus) -> String {
    let mut out = String::new();
    out.push_str(&format!("vulyk-api {}\n", crate_version()));
    out.push_str(&format!("applied: {}\n", status.applied.len()));
    for m in &status.applied {
        out.push_str(&format!("  + {}  {}\n", m.version, m.description));
    }
    out.push_str(&format!("pending: {}\n", status.pending.len()));
    if status.pending.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for m in &status.pending {
            out.push_str(&format!("  - {}  {}\n", m.version, m.description));
        }
    }
    if !status.unknown_applied.is_empty() {
        out.push_str("WARNING: applied migrations missing from this binary (schema newer):\n");
        for v in &status.unknown_applied {
            out.push_str(&format!("  ! {v}\n"));
        }
    }
    out
}

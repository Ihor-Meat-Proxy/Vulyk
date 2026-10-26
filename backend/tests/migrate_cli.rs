//! Migration status / downgrade guards (needs DATABASE_URL).

#[tokio::test]
async fn migrate_status_lists_applied() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let database_url = std::env::var("DATABASE_URL").unwrap();
    let pool = vulyk_api::db::connect(&database_url)
        .await
        .expect("connect");
    vulyk_api::db::migrate(&pool).await.expect("migrate");
    let status = vulyk_api::db::migration_status(&pool)
        .await
        .expect("status");
    assert!(
        !status.applied.is_empty(),
        "expected applied migrations after migrate"
    );
    assert!(
        status.pending.is_empty(),
        "expected no pending migrations after migrate"
    );
    assert!(
        !status.schema_newer_than_binary(),
        "schema should match binary"
    );
    let rendered = vulyk_api::db::format_migration_status(&status);
    assert!(rendered.contains("vulyk-api"));
    assert!(rendered.contains("applied:"));
}

#[test]
fn crate_version_is_semverish() {
    let v = vulyk_api::db::crate_version();
    assert!(
        v.split('.').count() >= 2,
        "expected semver-like version, got {v}"
    );
}

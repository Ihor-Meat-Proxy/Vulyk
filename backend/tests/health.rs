//! Smoke test that compiles with the crate. Full HTTP tests run in CI with Postgres.

#[test]
fn migrations_folder_exists() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    assert!(path.is_dir(), "migrations directory missing");
    let entries: Vec<_> = std::fs::read_dir(&path)
        .expect("read migrations")
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        entries.len() >= 2,
        "expected at least two migration files, found {}",
        entries.len()
    );
}

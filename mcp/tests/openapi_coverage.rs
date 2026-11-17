//! Contract: every OpenAPI operation is either mapped to an MCP tool or
//! explicitly excluded in `mcp/openapi-coverage.json`.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: u32,
    excluded_tags: Vec<String>,
    tools: BTreeMap<String, Vec<String>>,
    excluded_operations: BTreeMap<String, String>,
}

fn repo_root() -> PathBuf {
    // tests/ → mcp/ → repo root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("mcp crate parent")
        .to_path_buf()
}

fn load_openapi_ops(root: &std::path::Path) -> (BTreeSet<String>, BTreeMap<String, Vec<String>>) {
    let path = root.join("docs/api/openapi.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("read {}: {e}", path.display());
    });
    let spec: serde_json::Value = serde_json::from_str(&raw).expect("parse openapi.json");
    let mut ops = BTreeSet::new();
    let mut tags_by_op: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let paths = spec
        .get("paths")
        .and_then(|v| v.as_object())
        .expect("openapi paths");
    for (path, methods) in paths {
        let methods = methods.as_object().expect("path item");
        for (method, op) in methods {
            let m = method.to_ascii_uppercase();
            if !matches!(
                m.as_str(),
                "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
            ) {
                continue;
            }
            let key = format!("{m} {path}");
            ops.insert(key.clone());
            let tags = op
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            tags_by_op.insert(key, tags);
        }
    }
    (ops, tags_by_op)
}

#[test]
fn mcp_tools_cover_or_exclude_every_openapi_operation() {
    let root = repo_root();
    let manifest_path = root.join("mcp/openapi-coverage.json");
    let raw = std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        panic!("read {}: {e}", manifest_path.display());
    });
    let manifest: Manifest = serde_json::from_str(&raw).expect("parse openapi-coverage.json");
    assert_eq!(manifest.schema_version, 1);
    for tag in ["admin", "backups", "webhooks"] {
        assert!(
            manifest.excluded_tags.iter().any(|t| t == tag),
            "excluded_tags must include {tag}"
        );
    }

    let (openapi_ops, tags_by_op) = load_openapi_ops(&root);
    let mut covered: BTreeSet<String> = BTreeSet::new();
    for (tool, ops) in &manifest.tools {
        assert!(!ops.is_empty(), "tool {tool} has no operations");
        for op in ops {
            assert!(
                openapi_ops.contains(op),
                "tool {tool} maps unknown operation {op}"
            );
            if let Some(tags) = tags_by_op.get(op) {
                for tag in &manifest.excluded_tags {
                    assert!(
                        !tags.iter().any(|t| t == tag),
                        "tool {tool} must not map {op} (tag {tag} is excluded from MCP surface)"
                    );
                }
            }
            covered.insert(op.clone());
        }
    }

    let mut missing = Vec::new();
    let mut stale_exclusions = Vec::new();
    for op in &openapi_ops {
        if covered.contains(op) {
            if manifest.excluded_operations.contains_key(op) {
                stale_exclusions.push(op.clone());
            }
            continue;
        }
        if !manifest.excluded_operations.contains_key(op) {
            missing.push(op.clone());
        }
    }
    let unknown: Vec<_> = manifest
        .excluded_operations
        .keys()
        .filter(|op| !openapi_ops.contains(*op))
        .cloned()
        .collect();

    assert!(
        missing.is_empty(),
        "OpenAPI operations missing from MCP tools and excluded_operations (add a tool mapping or an exclusion with justification):\n{}",
        missing.join("\n")
    );
    assert!(
        unknown.is_empty(),
        "excluded_operations entries not present in docs/api/openapi.json:\n{}",
        unknown.join("\n")
    );
    assert!(
        stale_exclusions.is_empty(),
        "operations are both tool-covered and excluded (remove from excluded_operations):\n{}",
        stale_exclusions.join("\n")
    );
}

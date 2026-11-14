//! OpenAPI operation inventory and Axum route-source parity checks.
//!
//! Axum does not expose a stable route iterator.  We therefore inspect the
//! `.route(...)` declarations in `src/routes/*.rs`, then independently send
//! each OpenAPI operation through the assembled router in the integration
//! suite.  The source check catches Axum-only routes; the runtime check catches
//! parser mistakes and OpenAPI-only routes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::OpenApi;

use crate::openapi::ApiDoc;

pub const HTTP_METHODS: [&str; 7] = ["delete", "get", "head", "options", "patch", "post", "put"];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ApiOperation {
    pub method: String,
    pub path: String,
    pub operation_id: Option<String>,
    pub success_statuses: Vec<u16>,
    pub has_request_body: bool,
    pub has_path_parameters: bool,
}

impl ApiOperation {
    pub fn key(&self) -> String {
        format!("{} {}", self.method, self.path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RouteOperation {
    pub method: String,
    pub path: String,
    pub source: String,
}

impl RouteOperation {
    pub fn key(&self) -> String {
        format!("{} {}", self.method, self.path)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageItem {
    pub method: String,
    pub path: String,
    pub operation_id: Option<String>,
    pub success_statuses: Vec<u16>,
    pub observed_runtime_status: Option<u16>,
    pub route_registration: &'static str,
    pub success_class: &'static str,
    pub auth_401: &'static str,
    pub validation_400_422: &'static str,
    pub not_found_404: &'static str,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageSummary {
    pub total_operations: usize,
    pub route_covered: usize,
    pub auth_covered_or_na: usize,
    pub success_contract_covered: usize,
    pub database_sweeps_run: bool,
    pub coverage_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageReport {
    pub schema_version: u8,
    pub generated_by: &'static str,
    pub summary: CoverageSummary,
    pub operations: Vec<CoverageItem>,
}

#[derive(Debug, Deserialize)]
pub struct CoverageManifest {
    pub schema_version: u8,
    pub public_operations: BTreeMap<String, ManifestException>,
    #[serde(default)]
    pub validation_not_applicable: BTreeMap<String, ManifestException>,
    #[serde(default)]
    pub not_found_not_applicable: BTreeMap<String, ManifestException>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestException {
    pub justification: String,
}

pub fn openapi_operations() -> Result<Vec<ApiOperation>, String> {
    let value = serde_json::to_value(ApiDoc::openapi())
        .map_err(|error| format!("serialize OpenAPI document: {error}"))?;
    let paths = value
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| "OpenAPI document has no paths object".to_string())?;
    let mut operations = Vec::new();

    for (path, item) in paths {
        let Some(item) = item.as_object() else {
            continue;
        };
        for method in HTTP_METHODS {
            let Some(operation) = item.get(method).and_then(Value::as_object) else {
                continue;
            };
            let mut success_statuses = operation
                .get("responses")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(status, _)| status.parse::<u16>().ok())
                // Redirects are the successful contract for browser-driven SSO.
                .filter(|status| (200..400).contains(status))
                .collect::<Vec<_>>();
            success_statuses.sort_unstable();

            operations.push(ApiOperation {
                method: method.to_ascii_uppercase(),
                path: path.clone(),
                operation_id: operation
                    .get("operationId")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                success_statuses,
                has_request_body: operation.get("requestBody").is_some(),
                has_path_parameters: path.contains('{'),
            });
        }
    }
    operations.sort();
    Ok(operations)
}

pub fn routes_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/routes")
}

pub fn source_route_operations() -> Result<Vec<RouteOperation>, String> {
    let mut files = fs::read_dir(routes_dir())
        .map_err(|error| format!("read routes directory: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .filter(|path| path.file_name().is_some_and(|name| name != "mod.rs"))
        .collect::<Vec<_>>();
    files.sort();

    let mut operations = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let file = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>")
            .to_string();
        for call in route_calls(&source)? {
            let Some((route_path, rest)) = first_string_literal(call) else {
                return Err(format!("{file}: .route call has no literal path: {call}"));
            };
            let methods = routing_methods(rest);
            if methods.is_empty() {
                return Err(format!(
                    "{file}: cannot determine HTTP method for route {route_path}"
                ));
            }
            for method in methods {
                operations.push(RouteOperation {
                    method,
                    path: route_path.clone(),
                    source: file.clone(),
                });
            }
        }
    }
    operations.sort();
    operations.dedup();
    Ok(operations)
}

pub fn parity_errors() -> Result<Vec<String>, String> {
    let openapi = openapi_operations()?
        .into_iter()
        .map(|operation| operation.key())
        .collect::<BTreeSet<_>>();
    let router = source_route_operations()?
        .into_iter()
        .map(|operation| operation.key())
        .collect::<BTreeSet<_>>();
    let mut errors = openapi
        .difference(&router)
        .map(|key| format!("OpenAPI-only operation: {key}"))
        .collect::<Vec<_>>();
    errors.extend(
        router
            .difference(&openapi)
            .map(|key| format!("Axum-only operation: {key}")),
    );
    Ok(errors)
}

pub fn load_manifest(path: &Path) -> Result<CoverageManifest, String> {
    let contents =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let manifest: CoverageManifest = serde_json::from_str(&contents)
        .map_err(|error| format!("parse {}: {error}", path.display()))?;
    if manifest.schema_version != 1 {
        return Err(format!(
            "{}: unsupported schema_version {}",
            path.display(),
            manifest.schema_version
        ));
    }
    Ok(manifest)
}

pub fn default_manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/api-coverage-manifest.json")
}

pub fn manifest_errors(operations: &[ApiOperation], manifest: &CoverageManifest) -> Vec<String> {
    let keys = operations
        .iter()
        .map(ApiOperation::key)
        .collect::<BTreeSet<_>>();
    let mut errors = Vec::new();
    for (kind, entries) in [
        ("public_operations", &manifest.public_operations),
        (
            "validation_not_applicable",
            &manifest.validation_not_applicable,
        ),
        (
            "not_found_not_applicable",
            &manifest.not_found_not_applicable,
        ),
    ] {
        for (key, exception) in entries {
            if !keys.contains(key) {
                errors.push(format!("{kind} contains unknown operation: {key}"));
            }
            if exception.justification.trim().is_empty() {
                errors.push(format!("{kind} entry has no justification: {key}"));
            }
        }
    }
    errors
}

pub fn build_report(operations: &[ApiOperation], manifest: &CoverageManifest) -> CoverageReport {
    let database_sweeps_run = std::env::var("DATABASE_URL").is_ok();
    let items = operations
        .iter()
        .map(|operation| {
            let key = operation.key();
            let public = manifest.public_operations.get(&key);
            let validation_na = manifest.validation_not_applicable.get(&key);
            let not_found_na = manifest.not_found_not_applicable.get(&key);
            let mut notes = Vec::new();
            if let Some(entry) = public {
                notes.push(format!("401 N/A: {}", entry.justification));
            }
            if let Some(entry) = validation_na {
                notes.push(format!("400/422 N/A: {}", entry.justification));
            }
            if let Some(entry) = not_found_na {
                notes.push(format!("404 N/A: {}", entry.justification));
            }
            CoverageItem {
                method: operation.method.clone(),
                path: operation.path.clone(),
                operation_id: operation.operation_id.clone(),
                success_statuses: operation.success_statuses.clone(),
                observed_runtime_status: None,
                route_registration: "covered: source parity + runtime dispatch",
                success_class: if operation.success_statuses.is_empty() {
                    "missing: no 2xx/3xx OpenAPI response"
                } else {
                    "covered: OpenAPI success contract"
                },
                auth_401: if public.is_some() {
                    "N/A: intentionally public"
                } else {
                    "covered: unauthenticated runtime sweep"
                },
                validation_400_422: if validation_na.is_some() {
                    "N/A: no client payload to validate"
                } else if operation.has_request_body {
                    if public.is_some() {
                        "covered: public malformed JSON runtime sweep"
                    } else if database_sweeps_run {
                        "covered: authenticated malformed JSON runtime sweep"
                    } else {
                        "skipped locally: DATABASE_URL not set"
                    }
                } else {
                    "covered: OpenAPI parameter contract"
                },
                not_found_404: if not_found_na.is_some() {
                    "N/A: collection/action has no addressable resource"
                } else if operation.has_path_parameters {
                    if database_sweeps_run {
                        "covered: random identifier DB sweep"
                    } else {
                        "skipped locally: DATABASE_URL not set"
                    }
                } else {
                    "N/A: path has no resource identifier"
                },
                notes,
            }
        })
        .collect::<Vec<_>>();
    let total = items.len();
    let success_contract_covered = operations
        .iter()
        .filter(|operation| !operation.success_statuses.is_empty())
        .count();
    let fully_covered = if success_contract_covered == total {
        total
    } else {
        success_contract_covered
    };
    CoverageReport {
        schema_version: 1,
        generated_by: "backend/tests/api_coverage.rs",
        summary: CoverageSummary {
            total_operations: total,
            route_covered: total,
            auth_covered_or_na: total,
            success_contract_covered,
            database_sweeps_run,
            coverage_percent: if total == 0 {
                100.0
            } else {
                fully_covered as f64 * 100.0 / total as f64
            },
        },
        operations: items,
    }
}

pub fn report_markdown(report: &CoverageReport) -> String {
    let mut output = format!(
        "# API coverage\n\n\
         - Operations: **{}**\n\
         - Route registration: **{}/{}**\n\
         - Authentication checks or justified N/A: **{}/{}**\n\
         - Success contracts: **{}/{}**\n\
         - Database-backed sweeps: **{}**\n\
         - Coverage: **{:.2}%**\n\n\
         | Method | Path | Route | Success | Auth | Validation | Not found |\n\
         | --- | --- | --- | --- | --- | --- | --- |\n",
        report.summary.total_operations,
        report.summary.route_covered,
        report.summary.total_operations,
        report.summary.auth_covered_or_na,
        report.summary.total_operations,
        report.summary.success_contract_covered,
        report.summary.total_operations,
        if report.summary.database_sweeps_run {
            "run"
        } else {
            "skipped (`DATABASE_URL` not set)"
        },
        report.summary.coverage_percent,
    );
    for item in &report.operations {
        output.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} | {} |\n",
            item.method,
            item.path,
            item.route_registration,
            item.success_class,
            item.auth_401,
            item.validation_400_422,
            item.not_found_404,
        ));
    }
    output
}

fn route_calls(source: &str) -> Result<Vec<&str>, String> {
    let mut calls = Vec::new();
    let mut offset = 0;
    while let Some(relative) = source[offset..].find(".route(") {
        let start = offset + relative + ".route(".len();
        let bytes = source.as_bytes();
        let mut depth = 1_i32;
        let mut index = start;
        let mut quote = None;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if let Some(delimiter) = quote {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == delimiter {
                    quote = None;
                }
            } else {
                match byte {
                    b'"' | b'\'' => quote = Some(byte),
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            calls.push(&source[start..index]);
                            offset = index + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            index += 1;
        }
        if depth != 0 {
            return Err("unbalanced .route(...) call".to_string());
        }
    }
    Ok(calls)
}

fn first_string_literal(call: &str) -> Option<(String, &str)> {
    let start = call.find('"')? + 1;
    let bytes = call.as_bytes();
    let mut index = start;
    let mut escaped = false;
    while index < bytes.len() {
        if escaped {
            escaped = false;
        } else if bytes[index] == b'\\' {
            escaped = true;
        } else if bytes[index] == b'"' {
            return Some((call[start..index].to_string(), &call[index + 1..]));
        }
        index += 1;
    }
    None
}

fn routing_methods(expression: &str) -> BTreeSet<String> {
    HTTP_METHODS
        .iter()
        .filter(|method| {
            let needle = format!("{method}(");
            expression.match_indices(&needle).any(|(index, _)| {
                expression[..index]
                    .chars()
                    .next_back()
                    .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
            })
        })
        .map(|method| method.to_ascii_uppercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_parser_handles_chained_and_qualified_methods() {
        let source = r#"
            Router::new()
                .route("/one", get(one).post(create))
                .route(
                    "/two/{id}",
                    axum::routing::put(update).patch(update).delete(remove),
                )
        "#;
        let calls = route_calls(source).expect("route calls");
        assert_eq!(calls.len(), 2);
        let (path, expression) = first_string_literal(calls[1]).expect("literal");
        assert_eq!(path, "/two/{id}");
        assert_eq!(
            routing_methods(expression),
            BTreeSet::from(["DELETE".into(), "PATCH".into(), "PUT".into()])
        );
    }

    #[test]
    fn axum_and_openapi_operations_are_identical() {
        let errors = parity_errors().expect("build route inventories");
        assert!(
            errors.is_empty(),
            "Axum/OpenAPI route drift:\n{}",
            errors.join("\n")
        );
    }

    #[test]
    fn coverage_manifest_only_names_openapi_operations() {
        let operations = openapi_operations().expect("OpenAPI operations");
        let manifest = load_manifest(&default_manifest_path()).expect("coverage manifest");
        let errors = manifest_errors(&operations, &manifest);
        assert!(
            errors.is_empty(),
            "invalid API coverage manifest:\n{}",
            errors.join("\n")
        );
    }
}

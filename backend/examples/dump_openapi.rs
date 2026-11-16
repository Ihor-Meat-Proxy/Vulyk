//! Dump `ApiDoc` as pretty JSON to stdout.
//! Used by `scripts/generate-openapi.sh` / `scripts/check-openapi-drift.sh`.
//!
//! Optional output path via env `OPENAPI_OUT` (avoids parsing process args).
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use utoipa::OpenApi;
use vulyk_api::openapi::ApiDoc;

fn main() -> ExitCode {
    let out_path = env::var_os("OPENAPI_OUT").map(PathBuf::from);

    let doc = ApiDoc::openapi();
    let json = match serde_json::to_string_pretty(&doc) {
        Ok(s) => format!("{s}\n"),
        Err(err) => {
            eprintln!("error: serialize OpenAPI: {err}");
            return ExitCode::FAILURE;
        }
    };

    if let Some(path) = out_path {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(err) = fs::create_dir_all(parent) {
                    eprintln!("error: create {}: {err}", parent.display());
                    return ExitCode::FAILURE;
                }
            }
        }
        if let Err(err) = fs::write(&path, &json) {
            eprintln!("error: write {}: {err}", path.display());
            return ExitCode::FAILURE;
        }
        eprintln!("wrote {}", path.display());
    } else if let Err(err) = io::stdout().write_all(json.as_bytes()) {
        eprintln!("error: write stdout: {err}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

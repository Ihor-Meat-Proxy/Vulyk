//! `vulyk-api config export|import` CLI.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{ConfigExportRequest, ConfigImportRequest};
use crate::services::config_bundle;

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Export instance/project configuration to a JSON or YAML file
    Export(ConfigExportArgs),
    /// Import configuration from a JSON or YAML file (supports --dry-run)
    Import(ConfigImportArgs),
}

#[derive(Debug, Args)]
pub struct ConfigExportArgs {
    /// Output file path (- for stdout)
    #[arg(short, long, default_value = "-")]
    pub output: PathBuf,
    /// all | selected
    #[arg(long, default_value = "all")]
    pub project_scope: String,
    /// Project ids (comma-separated) when scope=selected
    #[arg(long, value_delimiter = ',')]
    pub project_id: Vec<Uuid>,
    /// Project keys (comma-separated) when scope=selected
    #[arg(long, value_delimiter = ',')]
    pub project_key: Vec<String>,
    #[arg(long, default_value_t = false)]
    pub include_memberships: bool,
    #[arg(long, default_value_t = false)]
    pub include_data: bool,
    #[arg(long, default_value_t = true)]
    pub include_folder_structure: bool,
    #[arg(long)]
    pub secrets_passphrase: Option<String>,
    /// json | yaml
    #[arg(long, default_value = "json")]
    pub format: String,
}

#[derive(Debug, Args)]
pub struct ConfigImportArgs {
    /// Input file path (- for stdin)
    #[arg(short, long)]
    pub input: PathBuf,
    /// merge | replace
    #[arg(long, default_value = "merge")]
    pub strategy: String,
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
    #[arg(long)]
    pub secrets_passphrase: Option<String>,
    /// Apply without confirmation when not dry-run
    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

pub async fn run(pool: &PgPool, cmd: ConfigCommand) -> AppResult<()> {
    match cmd {
        ConfigCommand::Export(args) => run_export(pool, args).await,
        ConfigCommand::Import(args) => run_import(pool, args).await,
    }
}

async fn run_export(pool: &PgPool, args: ConfigExportArgs) -> AppResult<()> {
    let req = ConfigExportRequest {
        project_scope: args.project_scope,
        project_ids: args.project_id,
        project_keys: args.project_key,
        include_memberships: args.include_memberships,
        include_data: args.include_data,
        include_folder_structure: args.include_folder_structure,
        secrets_passphrase: args.secrets_passphrase,
        format: args.format.clone(),
    };
    let bundle = config_bundle::export(pool, &req).await?;
    let body = config_bundle::serialize_bundle(&bundle, &args.format)?;
    if args.output.as_os_str() == "-" {
        print!("{body}");
    } else {
        std::fs::write(&args.output, body)
            .map_err(|e| AppError::Internal(format!("write {}: {e}", args.output.display())))?;
        eprintln!("Wrote {}", args.output.display());
    }
    Ok(())
}

async fn run_import(pool: &PgPool, args: ConfigImportArgs) -> AppResult<()> {
    let raw = if args.input.as_os_str() == "-" {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
            .map_err(|e| AppError::BadRequest(format!("stdin: {e}")))?;
        buf
    } else {
        std::fs::read_to_string(&args.input)
            .map_err(|e| AppError::BadRequest(format!("read {}: {e}", args.input.display())))?
    };
    let bundle = config_bundle::parse_bundle(&raw)?;
    if !args.dry_run && !args.yes {
        return Err(AppError::BadRequest(
            "refusing to apply import without --yes (use --dry-run to preview)".into(),
        ));
    }
    let preview = config_bundle::import(
        pool,
        ConfigImportRequest {
            bundle,
            strategy: args.strategy,
            dry_run: args.dry_run,
            secrets_passphrase: args.secrets_passphrase,
        },
    )
    .await?;
    println!("{}", serde_json::to_string_pretty(&preview).unwrap());
    Ok(())
}

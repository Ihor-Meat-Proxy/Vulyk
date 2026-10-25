//! Vulyk CLI — report automated results to a Vulyk run (W4).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

#[derive(Parser, Debug)]
#[command(name = "vulyk", about = "Vulyk test management CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Print version information
    Version,
    /// Upload JUnit XML or Vulyk JSON results to a run
    Report {
        /// Path to JUnit XML or Vulyk JSON results
        #[arg(short, long)]
        file: PathBuf,
        /// Result format: auto | junit | vulyk_json
        #[arg(long, default_value = "auto")]
        format: String,
        /// Target run id (created when omitted with --project-id / --project-key)
        #[arg(long, env = "VULYK_RUN_ID")]
        run_id: Option<String>,
        /// Project id (used with --create-run)
        #[arg(long, env = "VULYK_PROJECT_ID")]
        project_id: Option<String>,
        /// Project key (resolved to id when --project-id is unset)
        #[arg(long, env = "VULYK_PROJECT_KEY")]
        project_key: Option<String>,
        /// Create a new run when run_id is unset
        #[arg(long, default_value_t = false)]
        create_run: bool,
        /// Name for a newly created run
        #[arg(long, env = "VULYK_RUN_NAME")]
        run_name: Option<String>,
        /// API base URL
        #[arg(long, env = "VULYK_API_URL", default_value = "http://127.0.0.1:8080")]
        api_url: String,
        /// Bearer token (optional in open Phase 1)
        #[arg(long, env = "VULYK_API_TOKEN")]
        token: Option<String>,
        /// Force async ingest job
        #[arg(long, default_value_t = false)]
        async_mode: bool,
        /// Poll job until complete when async
        #[arg(long, default_value_t = true)]
        wait: bool,
    },
}

struct ReportArgs {
    file: PathBuf,
    format: String,
    run_id: Option<String>,
    project_id: Option<String>,
    project_key: Option<String>,
    create_run: bool,
    run_name: Option<String>,
    api_url: String,
    token: Option<String>,
    async_mode: bool,
    wait: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Version => {
            println!("vulyk {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Commands::Report {
            file,
            format,
            run_id,
            project_id,
            project_key,
            create_run,
            run_name,
            api_url,
            token,
            async_mode,
            wait,
        } => report(ReportArgs {
            file,
            format,
            run_id,
            project_id,
            project_key,
            create_run,
            run_name,
            api_url,
            token,
            async_mode,
            wait,
        }),
    }
}

fn report(args: ReportArgs) -> Result<()> {
    let content =
        fs::read_to_string(&args.file).with_context(|| format!("read {}", args.file.display()))?;
    let format = resolve_format(&args.format, &args.file, &content)?;

    let client = HttpClient::new(&args.api_url, args.token.as_deref())?;

    let run_id = if let Some(id) = args.run_id {
        id
    } else {
        if !args.create_run {
            bail!("--run-id is required unless --create-run is set");
        }
        let project_id = resolve_project_id(&client, args.project_id, args.project_key)?;
        let name = args.run_name.unwrap_or_else(|| {
            format!(
                "CLI report {}",
                args.file
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("results")
            )
        });
        let run: Value = client.post_json(
            &format!("/api/v1/projects/{project_id}/runs"),
            &json!({
                "name": name,
                "status": "running",
                "source": "cli",
            }),
        )?;
        run.get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .context("create run response missing id")?
    };

    let body = json!({
        "format": format,
        "content": content,
        "async_mode": args.async_mode,
        "source": format,
    });

    let resp: Value = client.post_json(&format!("/api/v1/runs/{run_id}/ingest"), &body)?;
    println!("{}", serde_json::to_string_pretty(&resp)?);

    if args.wait {
        if let Some(job_id) = resp.get("job_id").and_then(|v| v.as_str()) {
            let final_job = wait_for_job(&client, job_id)?;
            println!("{}", serde_json::to_string_pretty(&final_job)?);
            let status = final_job
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if status == "failed" {
                bail!(
                    "ingest job failed: {}",
                    final_job
                        .get("last_error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                );
            }
        }
    }

    Ok(())
}

fn resolve_format(format: &str, file: &Path, content: &str) -> Result<String> {
    let f = format.trim().to_lowercase();
    if f != "auto" {
        return Ok(f);
    }
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let trimmed = content.trim_start();
    if ext == "xml"
        || trimmed.starts_with("<?xml")
        || trimmed.starts_with("<testsuites")
        || trimmed.starts_with("<testsuite")
    {
        return Ok("junit".into());
    }
    if ext == "json" || trimmed.starts_with('{') {
        return Ok("vulyk_json".into());
    }
    bail!("cannot detect format; pass --format junit|vulyk_json");
}

fn resolve_project_id(
    client: &HttpClient,
    project_id: Option<String>,
    project_key: Option<String>,
) -> Result<String> {
    if let Some(id) = project_id {
        return Ok(id);
    }
    let Some(key) = project_key else {
        bail!("--project-id or --project-key required with --create-run");
    };
    let projects: Value = client.get_json("/api/v1/projects")?;
    let arr = projects
        .as_array()
        .context("projects response is not an array")?;
    let key_up = key.to_uppercase();
    arr.iter()
        .find(|p| {
            p.get("key")
                .and_then(|v| v.as_str())
                .map(|k| k.eq_ignore_ascii_case(&key_up))
                .unwrap_or(false)
        })
        .and_then(|p| p.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .with_context(|| format!("project key '{key}' not found"))
}

fn wait_for_job(client: &HttpClient, job_id: &str) -> Result<Value> {
    for _ in 0..60 {
        let job: Value = client.get_json(&format!("/api/v1/jobs/{job_id}"))?;
        let status = job.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if matches!(status, "completed" | "failed" | "cancelled") {
            return Ok(job);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    bail!("timed out waiting for job {job_id}");
}

struct HttpClient {
    base: String,
    token: Option<String>,
    inner: reqwest::blocking::Client,
}

impl HttpClient {
    fn new(base: &str, token: Option<&str>) -> Result<Self> {
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            token: token.map(|s| s.to_string()),
            inner: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()?,
        })
    }

    fn get_json(&self, path: &str) -> Result<Value> {
        let mut req = self.inner.get(format!("{}{path}", self.base));
        if let Some(ref t) = self.token {
            req = req.bearer_auth(t);
        }
        let res = req.send().context("HTTP GET")?;
        let status = res.status();
        let text = res.text().unwrap_or_default();
        if !status.is_success() {
            bail!("GET {path} failed ({status}): {text}");
        }
        serde_json::from_str(&text).with_context(|| format!("decode GET {path}"))
    }

    fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
        let mut req = self.inner.post(format!("{}{path}", self.base)).json(body);
        if let Some(ref t) = self.token {
            req = req.bearer_auth(t);
        }
        let res = req.send().context("HTTP POST")?;
        let status = res.status();
        let text = res.text().unwrap_or_default();
        if !status.is_success() {
            bail!("POST {path} failed ({status}): {text}");
        }
        serde_json::from_str(&text).with_context(|| format!("decode POST {path}"))
    }
}

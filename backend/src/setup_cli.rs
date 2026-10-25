//! Interactive and non-interactive first-run setup (`vulyk-api setup`).

use std::io::{self, IsTerminal, Write};

use clap::Parser;
use sqlx::PgPool;

use crate::auth::validate_password_strength;
use crate::error::{AppError, AppResult};
use crate::seed;
use crate::settings::{self, InstanceSettings, RegistrationMode};

#[derive(Debug, Clone, Parser)]
#[command(name = "setup", about = "Complete first-run instance setup")]
pub struct SetupArgs {
    /// Admin email address
    #[arg(long, env = "VULYK_ADMIN_EMAIL")]
    pub admin_email: Option<String>,

    /// Admin password
    #[arg(long, env = "VULYK_ADMIN_PASSWORD")]
    pub admin_password: Option<String>,

    /// Admin display name
    #[arg(
        long,
        env = "VULYK_ADMIN_DISPLAY_NAME",
        default_value = "Administrator"
    )]
    pub admin_display_name: String,

    /// Instance display name
    #[arg(long, env = "VULYK_INSTANCE_NAME")]
    pub instance_name: Option<String>,

    /// Public base URL of the API / UI
    #[arg(long, env = "VULYK_BASE_URL")]
    pub base_url: Option<String>,

    /// Default locale (e.g. en)
    #[arg(long, env = "VULYK_DEFAULT_LOCALE")]
    pub locale: Option<String>,

    /// Default timezone (e.g. UTC)
    #[arg(long, env = "VULYK_DEFAULT_TIMEZONE")]
    pub timezone: Option<String>,

    /// Registration mode: open | invite | disabled
    #[arg(long, env = "VULYK_REGISTRATION_MODE")]
    pub registration_mode: Option<String>,

    /// One-time setup token (optional for local trusted CLI)
    #[arg(long, env = "VULYK_SETUP_TOKEN")]
    pub setup_token: Option<String>,

    /// Skip confirmation prompts
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Seed the Kyiv Open-Air Concert sample project (default on)
    #[arg(long, env = "VULYK_SEED_SAMPLE", default_value = "1")]
    pub seed_sample: String,
}

/// Entry point used by `vulyk-api setup`.
pub async fn run_setup(pool: &PgPool, args: SetupArgs) -> AppResult<()> {
    let completed = settings::get_bool(pool, "setup_completed", false).await?;
    if completed {
        return Err(AppError::Conflict("setup already completed".into()));
    }

    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal() && !args.yes;

    let admin_email = match args.admin_email.filter(|s| !s.trim().is_empty()) {
        Some(e) => e,
        None if interactive => prompt("Admin email")?,
        None => {
            return Err(AppError::BadRequest(
                "--admin-email (or VULYK_ADMIN_EMAIL) is required".into(),
            ))
        }
    };
    let admin_email = admin_email.trim().to_lowercase();
    if !admin_email.contains('@') {
        return Err(AppError::BadRequest("valid admin email is required".into()));
    }

    let admin_password = match args.admin_password.filter(|s| !s.is_empty()) {
        Some(p) => p,
        None if interactive => prompt_secret("Admin password")?,
        None => {
            return Err(AppError::BadRequest(
                "--admin-password (or VULYK_ADMIN_PASSWORD) is required".into(),
            ))
        }
    };
    validate_password_strength(&admin_password)?;

    let mut instance = settings::load_settings(pool).await?;

    if let Some(name) = args.instance_name.filter(|s| !s.is_empty()) {
        instance.instance_name = name;
    } else if interactive && instance.instance_name == "Vulyk" {
        let name = prompt_default("Instance name", &instance.instance_name)?;
        if !name.is_empty() {
            instance.instance_name = name;
        }
    }

    if let Some(url) = args.base_url.filter(|s| !s.is_empty()) {
        instance.base_url = url;
    } else if interactive {
        let url = prompt_default("Base URL", &instance.base_url)?;
        instance.base_url = url;
    }

    if let Some(locale) = args.locale.filter(|s| !s.is_empty()) {
        instance.default_locale = locale;
    } else if interactive {
        instance.default_locale = prompt_default("Default locale", &instance.default_locale)?;
    }

    if let Some(tz) = args.timezone.filter(|s| !s.is_empty()) {
        instance.default_timezone = tz;
    } else if interactive {
        instance.default_timezone = prompt_default("Default timezone", &instance.default_timezone)?;
    }

    if let Some(mode) = args.registration_mode.filter(|s| !s.is_empty()) {
        instance.registration_mode = mode
            .parse::<RegistrationMode>()
            .map_err(AppError::BadRequest)?;
    } else if interactive {
        let current = instance.registration_mode.as_str();
        let mode = prompt_default("Registration mode (open|invite|disabled)", current)?;
        instance.registration_mode = mode
            .parse::<RegistrationMode>()
            .map_err(AppError::BadRequest)?;
    }

    if interactive && !args.yes {
        println!();
        println!("About to complete setup:");
        println!("  admin:    {admin_email}");
        println!("  instance: {}", instance.instance_name);
        println!("  base_url: {}", instance.base_url);
        println!("  locale:   {}", instance.default_locale);
        println!("  timezone: {}", instance.default_timezone);
        println!("  reg mode: {}", instance.registration_mode);
        let confirm = prompt_default("Proceed? [y/N]", "n")?;
        if !matches!(confirm.to_lowercase().as_str(), "y" | "yes") {
            return Err(AppError::BadRequest("setup cancelled".into()));
        }
    }

    let display_name = args.admin_display_name.trim();
    let display_name = if display_name.is_empty() {
        "Administrator"
    } else {
        display_name
    };

    let seed_sample = !matches!(
        args.seed_sample.to_lowercase().as_str(),
        "0" | "false" | "no" | "off"
    );

    seed::complete_setup_with_options(
        pool,
        &admin_email,
        &admin_password,
        display_name,
        &instance,
        args.setup_token.as_deref(),
        seed_sample,
    )
    .await?;

    println!("Setup complete. You can sign in as {admin_email}.");
    Ok(())
}

fn prompt(label: &str) -> AppResult<String> {
    print!("{label}: ");
    io::stdout()
        .flush()
        .map_err(|e| AppError::Internal(format!("stdout: {e}")))?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|e| AppError::Internal(format!("stdin: {e}")))?;
    Ok(line.trim().to_string())
}

fn prompt_default(label: &str, default: &str) -> AppResult<String> {
    print!("{label} [{default}]: ");
    io::stdout()
        .flush()
        .map_err(|e| AppError::Internal(format!("stdout: {e}")))?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|e| AppError::Internal(format!("stdin: {e}")))?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn prompt_secret(label: &str) -> AppResult<String> {
    eprint!("{label}: ");
    let _ = io::stderr().flush();
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("stty")
            .arg("-echo")
            .stdin(std::process::Stdio::inherit())
            .status();
        let mut line = String::new();
        let result = io::stdin().read_line(&mut line);
        let _ = std::process::Command::new("stty")
            .arg("echo")
            .stdin(std::process::Stdio::inherit())
            .status();
        eprintln!();
        result.map_err(|e| AppError::Internal(format!("stdin: {e}")))?;
        Ok(line.trim().to_string())
    }
    #[cfg(not(unix))]
    {
        prompt(label)
    }
}

/// Convenience for tests / library callers building settings manually.
#[allow(dead_code)]
pub async fn complete_with_settings(
    pool: &PgPool,
    admin_email: &str,
    admin_password: &str,
    display_name: &str,
    settings: &InstanceSettings,
    setup_token: Option<&str>,
) -> AppResult<()> {
    validate_password_strength(admin_password)?;
    seed::complete_setup(
        pool,
        admin_email,
        admin_password,
        display_name,
        settings,
        setup_token,
    )
    .await
}

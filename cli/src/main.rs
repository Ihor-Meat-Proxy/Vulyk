//! Vulyk CLI skeleton (reporter lands in workstream W4).

use clap::{Parser, Subcommand};

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
    /// Report results to a run (stub)
    Report {
        /// Path to JUnit XML or Vulyk JSON results
        #[arg(short, long)]
        file: String,
        /// Target run id
        #[arg(long, env = "VULYK_RUN_ID")]
        run_id: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Version => {
            println!("vulyk-cli {}", env!("CARGO_PKG_VERSION"));
        }
        Commands::Report { file, run_id } => {
            println!(
                "Report stub: file={file} run_id={}",
                run_id.as_deref().unwrap_or("(unset)")
            );
            println!("Full reporter ships in workstream W4.");
        }
    }
}

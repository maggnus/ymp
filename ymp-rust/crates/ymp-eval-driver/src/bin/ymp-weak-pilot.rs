//! YMP-201 research preparation; never a second distributed application.
#[path = "weak_pilot/mod.rs"]
mod weak_pilot;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "YMP-201 frozen-manifest consumer and offline measurement proofs")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(name = "_supervise", hide = true)]
    Supervise {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<std::ffi::OsString>,
    },
    /// Exercise injected providers, real storage admission, and the current Engine.
    OfflineProof {
        #[arg(long)]
        output: PathBuf,
    },
    /// Consume an exact frozen native manifest with external owner authorization.
    Native {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        approval: Option<PathBuf>,
        #[arg(long)]
        calibration_report: Option<PathBuf>,
    },
    /// Same consumer, exclusively against the local protocol executable.
    Scripted {
        #[arg(long)]
        manifest: PathBuf,
    },
    #[command(hide = true)]
    Mcp {
        #[arg(long)]
        socket: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    match Args::parse().command {
        Command::Supervise { argv } => ymp_providers::supervisor::run(argv).await,
        Command::OfflineProof { output } => {
            let output = weak_pilot::fresh_output(&output)?;
            let report = weak_pilot::prove(&output).await?;
            weak_pilot::write_json(&output.join("offline-proof.json"), &report)?;
            println!("{}", output.join("offline-proof.json").display());
            Ok(())
        }
        Command::Native {
            manifest,
            approval,
            calibration_report,
        } => {
            let report = weak_pilot::consumer::run(
                &manifest,
                false,
                approval.as_deref(),
                calibration_report.as_deref(),
            )
            .await?;
            println!("{}", serde_json::to_string(&report)?);
            anyhow::ensure!(
                report["complete"] == true,
                "Run interrupted; all remaining cells are retained as not_started"
            );
            Ok(())
        }
        Command::Scripted { manifest } => {
            let report = weak_pilot::consumer::run(&manifest, true, None, None).await?;
            println!("{}", serde_json::to_string(&report)?);
            anyhow::ensure!(
                report["complete"] == true,
                "Scripted run interrupted; inspect the saved report"
            );
            Ok(())
        }
        Command::Mcp { socket } => ymp_runtime::mcp::stdio_bridge(&socket).await,
    }
}

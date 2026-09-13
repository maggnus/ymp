//! YMP-201 research preparation; never a second distributed application.
#[path = "weak_pilot/mod.rs"]
mod weak_pilot;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Offline YMP-201 measurement-boundary proofs; native work remains blocked")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Exercise injected providers, real storage admission, and the current Engine.
    OfflineProof {
        #[arg(long)]
        output: PathBuf,
    },
    /// Record the missing native guarantees, then fail before any provider spawn.
    Native {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    match Args::parse().command {
        Command::OfflineProof { output } => {
            let output = weak_pilot::fresh_output(&output)?;
            let report = weak_pilot::prove(&output).await?;
            weak_pilot::write_json(&output.join("offline-proof.json"), &report)?;
            println!("{}", output.join("offline-proof.json").display());
            Ok(())
        }
        Command::Native { manifest, output } => {
            // Reading a proposal is not authority to release an experimental prompt.
            let bytes = std::fs::read(&manifest)?;
            let _: serde_json::Value = serde_json::from_slice(&bytes)?;
            let output = weak_pilot::fresh_output(&output)?;
            weak_pilot::write_json(
                &output.join("native-preflight.json"),
                &json!({
                    "schema_version":1,
                    "status":"blocked_before_native",
                    "provider_spawned":false,
                    "native_inference":false,
                    "manifest_sha256":ymp_core::bytes_digest(&bytes),
                    "actual_model":null,"actual_usage":null,"actual_elapsed_seconds":null,
                    "limitations":weak_pilot::native_boundary(),
                    "owner_authorization":"separate parent approval required"
                }),
            )?;
            bail!("native_controls_unproven: native subagent, memory and retry suppression are not established; inspect native-preflight.json")
        }
    }
}

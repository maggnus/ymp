//! Trusted offline driver. Acceptance exports are derived from runtime records.
mod adaptive_protocol;
mod artifact_protocol;
mod authority_protocol;
mod budget_protocol;
mod concurrency_protocol;
mod effort_protocol;
mod evidence_protocol;
mod export;
mod knowledge_protocol;
mod location_protocol;
mod partial_protocol;
mod protocols;
mod restart_protocol;
mod roster_protocol;
mod script;
mod task_protocol;
mod team_protocol;
mod workflows;
use anyhow::{ensure, Result};
use clap::Parser;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use ymp_core::{bytes_digest, now};

#[derive(Parser)]
struct Args {
    /// Fresh evidence directory outside any Git checkout.
    #[arg(long)]
    output: PathBuf,
    /// Run one named workflow/protocol, otherwise run all declared cases.
    #[arg(long)]
    case: Option<String>,
    #[arg(long, hide = true)]
    restart_stage: Option<String>,
}
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_owned()
}
fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
fn read_json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}
fn file_digest(path: &Path) -> Result<String> {
    Ok(bytes_digest(&std::fs::read(path)?))
}
fn require_non_git(path: &Path) -> Result<()> {
    ensure!(
        !path.ancestors().any(|p| p.join(".git").exists()),
        "Evaluation working directories must be outside Git"
    );
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if let Some(stage) = &args.restart_stage {
        return restart_protocol::worker(&root(), &args.output, stage).await;
    }
    ensure!(
        !args.output.exists(),
        "Evidence directory must be fresh; existing evidence is never overwritten"
    );
    std::fs::create_dir_all(&args.output)?;
    let output = args.output.canonicalize()?;
    require_non_git(&output)?;
    let root = root();
    let workflows = read_json(&root.join("ymp-evals/scenarios/universal-workflows.json"))?;
    let protocols = read_json(&root.join("ymp-evals/scenarios/universal-protocol.json"))?;
    let mut report = export::manifest(&root)?;
    let mut cases = Vec::new();
    let names = workflows["workflows"]
        .as_object()
        .unwrap()
        .keys()
        .chain(protocols["cases"].as_object().unwrap().keys())
        .cloned()
        .collect::<Vec<_>>();
    if let Some(case) = &args.case {
        ensure!(names.contains(case), "Unknown case {case}");
    }
    report["case_ids"] = json!(names);
    report["started_at"] = json!(now());
    report["command"] = json!(std::env::args().collect::<Vec<_>>());
    write_json(&output.join("run.json"), &report)?;
    for case in names
        .iter()
        .filter(|case| args.case.as_ref().is_none_or(|selected| selected == *case))
    {
        let directory = output.join(case);
        std::fs::create_dir(&directory)?;
        let result = if let Some(spec) = workflows["workflows"].get(case) {
            workflows::run(
                &root,
                &directory,
                case,
                serde_json::from_value(spec.clone())?,
            )
            .await
        } else {
            protocols::run(&root, &directory, case, &protocols["cases"][case]).await
        };
        let receipt = match result {
            Ok(receipt) => receipt,
            Err(error) => json!({"case_id":case,"complete":false,"error":format!("{error:#}")}),
        };
        write_json(&directory.join("case.json"), &receipt)?;
        eprintln!(
            "{}: {}",
            case,
            if receipt["complete"] == true {
                "passed"
            } else {
                "incomplete"
            }
        );
        cases.push(receipt);
    }
    report["cases"] = json!(cases);
    report["finished_at"] = json!(now());
    report["complete"] =
        json!(cases.len() == names.len() && cases.iter().all(|case| case["complete"] == true));
    write_json(&output.join("run.json"), &report)?;
    ensure!(
        cases.iter().all(|case| case["complete"] == true),
        "One or more runtime scenarios remain incomplete; inspect {}",
        output.display()
    );
    Ok(())
}

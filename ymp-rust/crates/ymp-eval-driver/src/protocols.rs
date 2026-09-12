//! Production-boundary protocol adapters. Missing behavior remains a failed case.
use crate::write_json;
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

pub async fn run(root: &Path, directory: &Path, case: &str, spec: &Value) -> Result<Value> {
    match case {
        "restart-inspection" => crate::restart_protocol::run(root, directory).await,
        "assignment-authority" => crate::authority_protocol::run(root, directory).await,
        "knowledge-correction" => crate::knowledge_protocol::run(root, directory, spec).await,
        "adaptive-team" => crate::adaptive_protocol::run(root, directory, spec).await,
        "artifact-version" => crate::artifact_protocol::run(root, directory).await,
        "evidence-boundaries" => crate::evidence_protocol::run(root, directory).await,
        "partial-usage" => crate::partial_protocol::run(root, directory, spec).await,
        "effort-support" => crate::effort_protocol::run(root, directory, spec).await,
        "location-retrieval" => crate::location_protocol::run(root, directory).await,
        "concurrency-conflicts" => crate::concurrency_protocol::run(root, directory, spec).await,
        "fixed-roster" => crate::roster_protocol::run(root, directory).await,
        "fixed-size" => crate::team_protocol::run(root, directory, case, spec).await,
        "budget-reservations" => crate::budget_protocol::run(root, directory, spec).await,
        _ => {
            anyhow::bail!("Protocol adapter {case} is not implemented; scenario remains incomplete")
        }
    }
}
pub async fn validate(root: &Path, directory: &Path, case: &str) -> Result<Value> {
    let output = tokio::process::Command::new("python3")
        .arg(root.join("ymp-evals/validators/universal.py"))
        .args(["protocol", "--case", case, "--workdir"])
        .arg(directory.join("work"))
        .arg("--observed")
        .arg(directory.join("observed.json"))
        .output()
        .await?;
    let result = serde_json::from_slice(&output.stdout)?;
    write_json(&directory.join("validator.json"), &result)?;
    Ok(result)
}

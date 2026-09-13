mod fixture;
mod proofs;
mod raw;

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub use proofs::prove;

pub fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

pub fn fresh_output(path: &Path) -> Result<PathBuf> {
    ensure!(!path.exists(), "Output must be fresh");
    let parent = path.parent().context("Output needs a parent directory")?;
    let parent = parent.canonicalize()?;
    ensure!(
        !parent.ancestors().any(|p| p.join(".git").exists()),
        "Offline evidence must be outside Git"
    );
    ensure!(
        !parent
            .components()
            .any(|p| matches!(p.as_os_str().to_str(), Some(".ymp2" | "_ymp3" | "3223c6a9"))),
        "Live application data is forbidden"
    );
    let output = parent.join(path.file_name().context("Output needs a name")?);
    std::fs::create_dir(&output)?;
    Ok(output)
}

pub fn native_boundary() -> Value {
    json!({
        "raw_api":"ymp_providers::run_turn_with_backend + NativeExecutionBackend",
        "cooperation_api":"ymp_runtime::Engine::run_identified",
        "missing_controls":[
            "TurnRequest/NativeResourceControls expose no proved Codex native-subagent disable control",
            "Engine.use_memory=false disables ymp retrieval/learning, not native provider memory or inherited native configuration",
            "Limits.attempts=1 disables ymp task/review repeats, not Codex willRetry or internal model requests",
            "ProviderConfig.args can carry verified native settings, but accepted names and enforcement for these controls have not been established"
        ],
        "effort":"Use AgentExecutionPolicy.fixed model and low; Codex sends both thread config and turn effort. Reconfirm native catalog and actual sent/reported observations before a measurement.",
        "membership":"Pinned Engine IDs are assignments/contexts; no claim of exactly N model workers is available while native children remain uncontrolled.",
        "token_bound":"Observed input+output is an admission/stop threshold. Native in-flight token overshoot has no proved finite bound.",
        "call_bound":"Limits.turns and the outer counter count ymp invocations, not internal native API requests or retries.",
        "deadline":"Use one external absolute deadline and cancellation for the complete condition; no whole-condition deadline exists in Limits.",
        "mcp":"Engine.executable must point to current ymp, or a research command must delegate to the existing mcp::stdio_bridge; the default eval-driver has no mcp subcommand.",
        "independent_context":"Fresh native contexts and distinct solving directories avoid provided peer history. Native filesystem isolation from sibling directories is not established by cwd.",
        "raw_review_reserve":"Storage retains one protected review invocation even for raw calls. The outer cap is exact; the captured storage ceiling adds one unused reserve without mislabeling raw work as final_review."
    })
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn offline_measurement_boundary_has_real_negative_controls() {
        let temp = tempfile::tempdir().unwrap();
        let output = super::fresh_output(&temp.path().join("proof")).unwrap();
        let report = super::prove(&output).await.unwrap();
        assert_eq!(report["complete"], true);
        assert_eq!(report["native_inference"], false);
    }
}

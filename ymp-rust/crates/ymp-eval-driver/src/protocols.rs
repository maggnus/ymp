//! Production-boundary protocol adapters. Missing behavior remains a failed case.
use crate::{
    export,
    script::{NativeJournal, ScriptedBackend},
    workflows, write_json,
};
use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_runtime::Engine;
use ymp_storage::Store;

pub async fn run(root: &Path, directory: &Path, case: &str, spec: &Value) -> Result<Value> {
    match case {
        "effort-support" => crate::effort_protocol::run(root, directory, spec).await,
        "location-retrieval" => crate::location_protocol::run(root, directory).await,
        "concurrency-conflicts" => crate::concurrency_protocol::run(root, directory, spec).await,
        "fixed-size" => crate::team_protocol::run(root, directory, case, spec).await,
        "budget-reservations" => budget_gap(root, directory, spec).await,
        _ => {
            anyhow::bail!("Protocol adapter {case} is not implemented; scenario remains incomplete")
        }
    }
}
/// Exercise the first real admission instead of pretending variable script
/// allowances are representable by the current session-wide reservation field.
async fn budget_gap(root: &Path, directory: &Path, spec: &Value) -> Result<Value> {
    let started = Instant::now();
    let work = directory.join("work");
    std::fs::create_dir(&work)?;
    let store = Store::open(&directory.join("metadata"))?;
    let mut config = workflows::config();
    let resources = config.limits.resources.as_mut().unwrap();
    resources.observed_tokens = Some(spec["setup"]["total_units"].as_u64().unwrap());
    let execution = spec["script"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["phase"] == "execution" && action["action"] == "reserve")
        .unwrap();
    resources.invocation_tokens = Some(execution["units"].as_u64().unwrap());
    resources.required_review_invocations = 1;
    let journal = Arc::new(NativeJournal::default());
    let backend = Arc::new(ScriptedBackend {
        case: "qualitative".into(),
        task_title: "Admission probe".into(),
        task_prompt: "Plan a bounded synthetic fact".into(),
        output: "outputs/ideas.md".into(),
        writer: root.join("ymp-evals/driver/artifact_writer.py"),
        journal: journal.clone(),
    });
    let (events, _) = mpsc::unbounded_channel();
    let mut engine = Engine::new(store.clone(), config, events, CancellationToken::new())?
        .with_execution_backend(backend)?;
    engine.use_memory = false;
    let outcome = engine
        .run(&work, "Plan a bounded synthetic fact", None)
        .await?;
    let trace = store.trace(&outcome.session.id)?;
    write_json(&directory.join("runtime.json"), &trace)?;
    journal.save(directory)?;
    write_json(&directory.join("alias-map.json"), &export::aliases(&trace))?;
    let budget = trace.budget.as_ref().unwrap();
    let mut projected = Vec::new();
    for event in &trace.history {
        if event.kind == "budget_denied" {
            projected.push(json!({"type":"admission_denied","phase":event.data["purpose"],"reason":event.data["code"]}));
        }
    }
    let observed = json!({"schema_version":1,"case_id":"budget-reservations","events":projected,"final_state":{"spent_units":budget.observed_usage.known_total(),"reserved_units":budget.reserved_tokens,"remaining_units":budget.limits.resources.as_ref().unwrap().observed_tokens.map(|total|total.saturating_sub(budget.observed_usage.known_total().unwrap_or(0))),"usage_by_agent":trace.usage.agents.iter().map(|(agent,usage)|(agent.clone(),json!(usage.known_total()))).collect::<serde_json::Map<_,_>>(),"denied_invocations_started":trace.invocations.len()}});
    write_json(&directory.join("observed.json"), &observed)?;
    let metrics = export::metrics(&trace, started.elapsed().as_secs_f64(), journal.peak())?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    let result = tokio::process::Command::new("python3")
        .arg(root.join("ymp-evals/validators/universal.py"))
        .args(["protocol", "--case", "budget-reservations", "--workdir"])
        .arg(&work)
        .arg("--observed")
        .arg(directory.join("observed.json"))
        .output()
        .await?;
    let validation: Value = serde_json::from_slice(&result.stdout)?;
    write_json(&directory.join("validator.json"), &validation)?;
    let compatibility = json!({"gap":"per_assignment_token_reservations","requested_execution_units":execution["units"],"requested_review_reserve_units":spec["setup"]["review_reserve_units"],"actual_global_invocation_tokens":budget.limits.resources.as_ref().unwrap().invocation_tokens,"actual_protected_review_invocations":budget.protected_review_invocations,"actual_protected_review_tokens":budget.limits.resources.as_ref().unwrap().invocation_tokens.map(|each|each*budget.protected_review_invocations),"actual_invocations":trace.invocations.len(),"runtime_summary":outcome.summary,"required_correction":"Capture separate review token reserve and bounded per-assignment reservations; recheck them atomically with shared spend. The immutable fixture remains unchanged."});
    write_json(&directory.join("compatibility.json"), &compatibility)?;
    ensure!(!result.status.success(),"Compatibility probe unexpectedly satisfies the script; implement the full action adapter before claiming completion");
    Ok(
        json!({"case_id":"budget-reservations","complete":false,"validator":validation,"compatibility":compatibility,"metrics":metrics}),
    )
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

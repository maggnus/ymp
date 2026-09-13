//! One captured cooperation condition through the existing production Engine.
use super::{measurement, write_json};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path, sync::Arc};
use tokio::{sync::mpsc, time::Instant};
use tokio_util::sync::CancellationToken;
use ymp_core::{new_id, now, Config, UiEvent, UnknownUsagePolicy};
use ymp_providers::ExecutionBackend;
use ymp_runtime::Engine;
use ymp_storage::Store;

pub async fn run_with_work(
    directory: &Path,
    work: &Path,
    config: Config,
    prompt: &str,
    backend: Arc<dyn ExecutionBackend>,
    deadline: Instant,
) -> Result<Value> {
    config.validate()?;
    let members = config.members();
    let requested_ids = members
        .iter()
        .map(|agent| agent.id.clone())
        .collect::<BTreeSet<_>>();
    ensure!(
        (2..=3).contains(&members.len()),
        "Cooperation requires two or three captured participants"
    );
    ensure!(
        config.team_constraints.fixed_size == Some(members.len())
            && config
                .team_constraints
                .fixed_roster
                .as_ref()
                .is_some_and(|ids| ids.iter().cloned().collect::<BTreeSet<_>>() == requested_ids),
        "Cooperation requires the exact fixed roster and size"
    );
    let mut models = BTreeSet::new();
    for member in &members {
        let fixed = &config
            .execution
            .get(&member.id)
            .context("Missing fixed participant execution policy")?
            .fixed;
        ensure!(
            fixed.effort.as_deref() == Some("low"),
            "Cooperation requires fixed low for every participant"
        );
        models.insert(
            fixed
                .model
                .as_ref()
                .context("Cooperation requires a fixed native model")?
                .clone(),
        );
    }
    ensure!(
        models.len() == 1,
        "Cooperation participants must use the same captured model"
    );
    ensure!(
        config.limits.attempts == 1,
        "Cooperation must disable ymp task/review retries"
    );
    let resources = config
        .limits
        .resources
        .as_ref()
        .context("Missing captured cooperation resources")?;
    ensure!(
        resources.unknown_usage == UnknownUsagePolicy::Stop
            && resources.observed_tokens.is_some()
            && resources.invocation_tokens.is_some(),
        "Cooperation requires observed token admission with unknown_usage=stop"
    );
    ensure!(
        config
            .acceptance_contracts
            .as_ref()
            .is_none_or(Vec::is_empty),
        "This observer must not install external confirmation contracts in the solving run"
    );

    // The caller stages work outside the whole private controller subtree.
    let (directory, work) = super::raw::disjoint_work_roots(directory, work)?;
    let metadata = directory.join("metadata");
    ensure!(!metadata.exists(), "Cooperation metadata must be fresh");
    let store = Store::open(&metadata)?;
    let (events, mut received) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();
    let mut engine = Engine::new(store.clone(), config.clone(), events, cancel.clone())?
        .with_execution_backend(backend)?;
    engine.use_memory = false;
    engine.adaptive = false;
    // current_exe remains the MCP host. The caller provides the existing stdio
    // bridge subcommand; there is no alternate cooperation protocol here.
    let session_id = new_id();
    let started = Instant::now();
    let mut deadline_exceeded = started >= deadline;
    if deadline_exceeded {
        cancel.cancel();
    }
    let mut journal = Vec::new();
    let mut future = Box::pin(engine.run_identified(&work, prompt, &session_id));
    let outcome = loop {
        tokio::select! {
            result = &mut future => break result,
            _ = tokio::time::sleep_until(deadline), if !deadline_exceeded => {
                deadline_exceeded = true;
                cancel.cancel();
            },
            Some(event) = received.recv() => journal.push(ui_event(event)),
        }
    };
    drop(future);
    drop(engine);
    while let Ok(event) = received.try_recv() {
        journal.push(ui_event(event));
    }
    write_json(&directory.join("ui-events.json"), &journal)?;

    let trace_result = store.trace(&session_id);
    let trace_error = trace_result
        .as_ref()
        .err()
        .map(|error| format!("{error:#}"));
    let trace = trace_result.ok();
    if let Some(trace) = &trace {
        write_json(&directory.join("trace.json"), trace)?;
    } else {
        write_json(
            &directory.join("trace-error.json"),
            &json!({"session_id":session_id,"error":trace_error}),
        )?;
    }
    let runtime_status = trace
        .as_ref()
        .map(|t| t.session.status.clone())
        .or_else(|| {
            outcome
                .as_ref()
                .ok()
                .map(|result| result.session.status.clone())
        });
    let engine_summary = outcome.as_ref().ok().map(|result| result.summary.clone());
    let error = outcome
        .as_ref()
        .err()
        .map(|error| format!("{error:#}"))
        .or_else(|| {
            outcome
                .as_ref()
                .ok()
                .filter(|result| result.session.status != "completed")
                .map(|result| result.summary.clone())
        })
        .or_else(|| {
            deadline_exceeded.then(|| {
                "external_group_deadline: cancellation requested for the whole condition".into()
            })
        });
    let mut summary = measurement::inspect(trace.as_ref(), &config);
    summary["session_id"] = json!(session_id);
    summary["status"] = json!(runtime_status.as_deref().unwrap_or(if deadline_exceeded {
        "interrupted"
    } else {
        "failed"
    }));
    summary["error"] = json!(error);
    summary["engine_summary"] = json!(engine_summary);
    summary["requested_agent_ids"] = json!(requested_ids);
    summary["deadline_exceeded"] = json!(deadline_exceeded);
    summary["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    summary["trace_error"] = json!(trace_error);
    if deadline_exceeded {
        summary["protocol_deviations"]
            .as_array_mut()
            .unwrap()
            .push(json!({"code":"external_deadline_exceeded"}));
    }
    write_json(&directory.join("summary.json"), &summary)?;
    // Runtime failures are retained outcomes. Returning their summary must never
    // be interpreted as acceptance or permission to retry the condition.
    Ok(summary)
}

fn ui_event(event: UiEvent) -> Value {
    let mut value = match event {
        UiEvent::Usage { session_id, usage } => {
            json!({"kind":"usage","session_id":session_id,"usage":usage})
        }
        UiEvent::Message(message) => json!({"kind":"message","value":message}),
        UiEvent::Delta { agent, text } => json!({"kind":"delta","agent":agent,"text":text}),
        UiEvent::AgentStatus { agent, status } => {
            json!({"kind":"agent_status","agent":agent,"status":status})
        }
        UiEvent::Task(task) => json!({"kind":"task","value":task}),
        UiEvent::Status(status) => json!({"kind":"status","value":status}),
        UiEvent::Finished { session_id, status } => {
            json!({"kind":"finished","session_id":session_id,"status":status})
        }
    };
    value["observed_at"] = json!(now());
    value
}

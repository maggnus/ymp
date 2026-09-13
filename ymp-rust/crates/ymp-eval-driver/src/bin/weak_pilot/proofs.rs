use super::{
    fixture::{self, Behavior, ProtocolBackend},
    native_boundary,
    raw::Group,
    write_json,
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
    sync::Arc,
    time::Duration,
};
use tokio::{sync::mpsc, time::Instant};
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::NativeExecutionBackend;
use ymp_runtime::Engine;
use ymp_storage::Store;

fn isolation(requests: &[Value]) -> Result<()> {
    let mut directories = HashSet::new();
    let mut contexts = HashSet::new();
    for request in requests {
        ensure!(
            request["prompt"] == fixture::VISIBLE_PROMPT,
            "candidate_prompt_contamination"
        );
        ensure!(
            request["resume"].is_null() && request["mcp_present"] == false,
            "candidate_context_reuse"
        );
        let cwd = Path::new(
            request["cwd"]
                .as_str()
                .context("Missing candidate directory")?,
        );
        ensure!(
            directories.insert(cwd.to_owned())
                && contexts.insert(request["context_id"].clone().to_string()),
            "candidate_scope_reuse"
        );
        let names = std::fs::read_dir(cwd)?
            .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
            .collect::<Result<HashSet<_>>>()?;
        ensure!(
            names == HashSet::from(["requirements.txt".into(), "input.txt".into()]),
            "candidate_file_contamination"
        );
        ensure!(
            std::fs::read_to_string(cwd.join("input.txt"))? == fixture::VISIBLE_INPUT,
            "candidate_input_drift"
        );
        ensure!(
            std::fs::read_to_string(cwd.join("requirements.txt"))? == fixture::VISIBLE_PROMPT,
            "candidate_requirements_drift"
        );
    }
    Ok(())
}

pub async fn prove(output: &Path) -> Result<Value> {
    let mut cases = Vec::new();
    for members in [1, 2, 3] {
        let name = format!("independent-{members}");
        let group = Group::new(&output.join(&name), members, members as u64, 12_000, 10_000)?;
        let backend = ProtocolBackend::new(Behavior::Complete);
        for agent in 0..members {
            group.candidate(&backend, agent).await?;
        }
        let denied = group.candidate(&backend, 0).await.unwrap_err();
        ensure!(
            denied.to_string().contains("pilot_call_limit"),
            "Extra call did not reach its explicit limit"
        );
        let requests = backend.starts();
        ensure!(
            requests.len() == members,
            "Call limit admitted an extra backend invocation"
        );
        isolation(&requests)?;
        let mut contaminated = requests.clone();
        contaminated[0]["prompt"] =
            json!(format!("{}\nPRIVATE-PEER-ANSWER", fixture::VISIBLE_PROMPT));
        ensure!(
            isolation(&contaminated).is_err(),
            "Prompt isolation negative control did not fail"
        );
        let cwd = Path::new(requests[0]["cwd"].as_str().unwrap());
        std::fs::write(cwd.join("hidden-answer.txt"), "PRIVATE-REFERENCE")?;
        ensure!(
            isolation(&requests).is_err(),
            "File isolation negative control did not fail"
        );
        std::fs::remove_file(cwd.join("hidden-answer.txt"))?;
        isolation(&requests)?;
        let trace = group.store.trace(&group.session.id)?;
        ensure!(
            trace.usage.total.known_total() == Some(10 * members as u64),
            "Repeated snapshots doubled token spend"
        );
        ensure!(
            !trace.usage.total.is_partial(),
            "Complete protocol accounting became partial"
        );
        ensure!(
            trace
                .history
                .iter()
                .filter(|event| event.kind == "pilot_provider_event"
                    && event.data["event"]["kind"] == "usage")
                .count()
                == 2 * members,
            "Raw repeated events were lost"
        );
        ensure!(
            group
                .config
                .execution_settings(
                    &group.config.agents[0],
                    &ModelEffort {
                        model: None,
                        effort: Some("high".into())
                    }
                )
                .is_err(),
            "Fixed low accepted high"
        );
        cases.push(json!({"case":name,"passed":true,"extra_call_denial":denied.to_string(),
            "negative_controls":["extra_call","peer_prompt_contamination","hidden_file_contamination","high_instead_of_fixed_low"],
            "observation":group.save(&backend)?}));
    }

    let group = Group::new(&output.join("group-budget"), 3, 6, 100, 10_000)?;
    let mut backend = ProtocolBackend::new(Behavior::Complete);
    backend.tokens = 45;
    group.candidate(&backend, 0).await?;
    group.candidate(&backend, 1).await?;
    let denied = group.candidate(&backend, 2).await.unwrap_err();
    ensure!(
        denied
            .downcast_ref::<BudgetDenial>()
            .is_some_and(|d| d.code == "token_limit"),
        "Shared storage token limit was not reached: {denied}"
    );
    ensure!(
        backend.starts().len() == 2,
        "Denied group member reached backend"
    );
    ensure!(
        group
            .store
            .session_usage(&group.session.id)?
            .total
            .known_total()
            == Some(90),
        "Group spend was reset per actor"
    );
    cases.push(json!({"case":"group-budget","passed":true,"negative_control":denied.to_string(),"observation":group.save(&backend)?}));

    for (name, behavior) in [
        ("unknown-usage", Behavior::Unknown),
        ("partial-usage", Behavior::Partial),
    ] {
        let group = Group::new(&output.join(name), 2, 6, 12_000, 10_000)?;
        let backend = ProtocolBackend::new(behavior);
        group.candidate(&backend, 0).await?;
        let denied = group.candidate(&backend, 1).await.unwrap_err();
        ensure!(
            denied
                .downcast_ref::<BudgetDenial>()
                .is_some_and(|d| d.code == "unknown_usage"),
            "Incomplete usage did not stop group admission"
        );
        ensure!(
            backend.starts().len() == 1,
            "Unknown usage allowed another native start"
        );
        ensure!(
            group
                .store
                .session_usage(&group.session.id)?
                .total
                .is_partial(),
            "Missing usage became zero"
        );
        cases.push(json!({"case":name,"passed":true,"negative_control":denied.to_string(),"observation":group.save(&backend)?}));
    }

    let group = Group::new(&output.join("provider-error"), 1, 6, 12_000, 10_000)?;
    let backend = ProtocolBackend::new(Behavior::Failure);
    ensure!(
        group.candidate(&backend, 0).await.is_err(),
        "Synthetic provider error was swallowed"
    );
    let trace = group.store.trace(&group.session.id)?;
    ensure!(
        trace.invocations[0].state == InvocationState::Failed
            && trace.usage.total.known_total() == Some(10),
        "Provider error lost spend or terminal state"
    );
    ensure!(
        backend.starts().len() == 1,
        "Failure was retried automatically"
    );
    cases.push(json!({"case":"provider-error","passed":true,"observation":group.save(&backend)?}));

    let group = Group::new(&output.join("group-deadline"), 2, 6, 12_000, 500)?;
    let started = Instant::now();
    let mut first = ProtocolBackend::new(Behavior::Complete);
    first.delay_ms = 10;
    group.candidate(&first, 0).await?;
    let pending = ProtocolBackend::new(Behavior::Pending);
    let stopped = group.candidate(&pending, 1).await.unwrap_err();
    ensure!(
        stopped.to_string().contains("pilot_group_deadline"),
        "Pending call did not stop at the group deadline"
    );
    ensure!(
        started.elapsed() < Duration::from_secs(3),
        "External deadline did not stop promptly"
    );
    ensure!(
        group
            .candidate(&pending, 0)
            .await
            .unwrap_err()
            .to_string()
            .contains("pilot_group_deadline"),
        "New call reset the shared deadline"
    );
    ensure!(
        pending.starts().len() == 1
            && pending
                .journal()
                .iter()
                .filter(|v| v["type"] == "closed")
                .count()
                == 1,
        "Deadline leaked a backend future"
    );
    let trace = group.store.trace(&group.session.id)?;
    ensure!(
        trace.usage.total.known_total() == Some(20) && trace.usage.total.is_partial(),
        "Deadline lost observed partial spend"
    );
    write_json(
        &output.join("group-deadline/first-protocol.json"),
        &first.journal(),
    )?;
    cases.push(json!({"case":"group-deadline","passed":true,"negative_control":stopped.to_string(),"observation":group.save(&pending)?}));

    let group = Group::new(&output.join("observed-stop"), 1, 6, 12_000, 10_000)?;
    let mut pending = ProtocolBackend::new(Behavior::Pending);
    pending.tokens = 12_001;
    ensure!(
        group
            .candidate(&pending, 0)
            .await
            .unwrap_err()
            .to_string()
            .contains("pilot_observed_token_threshold"),
        "Observed token threshold did not cancel active work"
    );
    let budget = group.store.session_budget(&group.session.id)?.unwrap();
    ensure!(
        budget.observed_token_overshoot == Some(1) && !budget.strict_token_bound,
        "Observed overshoot was hidden or promoted to a hard cap"
    );
    ensure!(
        budget.require_strict_token_bound().is_err(),
        "Partial spend authorized a strict-budget claim"
    );
    cases.push(json!({"case":"observed-stop","passed":true,"observation":group.save(&pending)?}));

    for members in [2, 3] {
        cases.push(cooperation(output, members).await?);
    }
    cases.push(native_protocol(output).await?);
    Ok(
        json!({"schema_version":1,"complete":true,"execution_kind":"injected-protocol",
        "native_inference":false,"model_measurements":[],"cases":cases,
        "limitations":native_boundary(),"scope":"Measurement preparation only; synthetic usage is not experimental cost or model-quality evidence"}),
    )
}

async fn native_protocol(output: &Path) -> Result<Value> {
    let mut receipts = Vec::new();
    for behavior in ["low", "drift"] {
        let directory = output.join(format!("codex-protocol-{behavior}"));
        let mut group = Group::new(&directory, 1, 1, 12_000, 10_000)?;
        let wire = directory.join("wire.jsonl");
        let provider = &mut group.config.providers[0];
        provider.kind = ProviderKind::Codex;
        provider.command = "python3".into();
        provider.args = vec![
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/bin/weak_pilot/codex_protocol.py")
                .to_string_lossy()
                .into_owned(),
            wire.to_string_lossy().into_owned(),
            behavior.into(),
        ];
        let result = group.candidate(&NativeExecutionBackend, 0).await;
        let requests = std::fs::read_to_string(&wire)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let thread = requests
            .iter()
            .find(|r| r["method"] == "thread/start")
            .context("Missing actual thread/start")?;
        ensure!(
            thread["params"]["model"] == "protocol-weak"
                && thread["params"]["config"]["model_reasoning_effort"] == "low",
            "Low was not written to the native protocol"
        );
        let turns = requests
            .iter()
            .filter(|r| r["method"] == "turn/start")
            .collect::<Vec<_>>();
        let trace = group.store.trace(&group.session.id)?;
        let failure = result.as_ref().err().map(|error| format!("{error:#}"));
        if behavior == "low" {
            result?;
            ensure!(
                turns.len() == 1 && turns[0]["params"]["effort"] == "low",
                "Low did not reach native turn/start"
            );
            ensure!(
                trace.usage.total.known_total() == Some(10) && !trace.usage.total.is_partial(),
                "Native duplicate usage was miscounted"
            );
        } else {
            ensure!(
                turns.is_empty(),
                "Acknowledged effort drift released an experimental prompt"
            );
            let error = result.unwrap_err();
            ensure!(
                error.to_string().contains("different effort"),
                "Expected the original effort-mismatch cause after native cleanup, got: {error:#}"
            );
        }
        write_json(&directory.join("trace.json"), &trace)?;
        receipts
            .push(json!({"behavior":behavior,"turn_start_count":turns.len(),"failure":failure,"usage":trace.usage}));
    }
    Ok(
        json!({"case":"codex-native-protocol","passed":true,"native_inference":false,
        "process":"python3 stdio fixture through NativeExecutionBackend","controls":receipts}),
    )
}

async fn cooperation(output: &Path, members: usize) -> Result<Value> {
    let name = format!("cooperation-{members}");
    let directory = output.join(&name);
    std::fs::create_dir(&directory)?;
    let work = directory.join("work");
    std::fs::create_dir(&work)?;
    std::fs::write(work.join("input.txt"), fixture::VISIBLE_INPUT)?;
    let store = Store::open(&directory.join("metadata"))?;
    let config = fixture::config(members, 8, 12_000);
    config.validate()?;
    let roster = config.team.clone();
    let (events, mut received) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();
    let mut backend = ProtocolBackend::new(Behavior::Cooperation);
    backend.cooperation_tasks = members - 1;
    let backend = Arc::new(backend);
    let mut engine = Engine::new(store.clone(), config, events, cancel.clone())?
        .with_execution_backend(backend.clone())?;
    engine.use_memory = false;
    engine.adaptive = false;
    let id = new_id();
    let mut future = Box::pin(engine.run_identified(
        &work,
        if members == 3 {
            "Independently copy input.txt into protocol-marker-1.txt and protocol-marker-2.txt, then independently inspect both outputs."
        } else {
            "Copy input.txt into protocol-marker.txt and independently inspect it."
        },
        &id,
    ));
    let outcome = tokio::select! {
        outcome = &mut future => outcome?,
        _ = tokio::time::sleep(Duration::from_secs(10)) => { cancel.cancel(); future.as_mut().await? },
    };
    drop(future);
    let trace = store.trace(&id)?;
    write_json(&directory.join("trace.json"), &trace)?;
    write_json(&directory.join("protocol.json"), &backend.journal())?;
    let mut ui_events = Vec::new();
    while let Ok(event) = received.try_recv() {
        ui_events.push(match event {
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
        });
    }
    write_json(&directory.join("ui-events.json"), &ui_events)?;
    ensure!(
        outcome.session.status == "completed",
        "Eight-invocation Engine proof did not complete: {}",
        outcome.summary
    );
    let producers = trace
        .assignments
        .iter()
        .filter(|a| a.purpose == "execute")
        .map(|a| a.agent_id.as_str())
        .collect::<HashSet<_>>();
    ensure!(
        producers.len() == members - 1,
        "No reserved independent reviewer remained"
    );
    let final_reviews = trace
        .assignments
        .iter()
        .filter(|a| a.purpose == "final_review")
        .collect::<Vec<_>>();
    ensure!(
        final_reviews.len() == 1 && !producers.contains(final_reviews[0].agent_id.as_str()),
        "Producer self-accepted final work"
    );
    ensure!(
        trace
            .assignments
            .iter()
            .all(|a| roster.contains(&a.agent_id) && a.requested.effort.as_deref() == Some("low")),
        "Engine widened membership or effort"
    );
    ensure!(
        trace
            .invocations
            .iter()
            .all(|i| i.sent.effort.as_deref() == Some("low")
                && i.reported.effort.as_deref() == Some("low")),
        "Injected transport did not receive fixed low"
    );
    ensure!(
        trace.usage.total.calls <= 8
            && trace.usage.total.open_calls == 0
            && !trace.usage.total.is_partial(),
        "Engine exceeded its quota or lost usage"
    );
    ensure!(
        trace.usage.total.known_total() == Some(trace.usage.total.calls * 10),
        "Engine summed duplicate snapshots"
    );
    ensure!(
        trace
            .assignments
            .iter()
            .all(|a| !["learn", "review_memory"].contains(&a.purpose.as_str())),
        "Disabled ymp memory still invoked a model"
    );
    let mut contexts = BTreeMap::new();
    let mut participants = HashSet::new();
    for invocation in &trace.invocations {
        let assignment = trace
            .assignments
            .iter()
            .find(|a| a.id == invocation.assignment_id)
            .context("Missing originating assignment")?;
        let context = invocation
            .native_session_id
            .as_ref()
            .context("Missing captured protocol context")?;
        if let Some(previous) = contexts.insert(context.clone(), assignment.agent_id.clone()) {
            ensure!(
                previous == assignment.agent_id,
                "Different actors shared a native context"
            );
        }
        if let Some(resumed) = &invocation.resumed_from {
            ensure!(
                contexts.get(resumed) == Some(&assignment.agent_id),
                "Continuation crossed its originating actor"
            );
        }
        participants.insert(assignment.agent_id.clone());
    }
    ensure!(
        participants == roster.iter().cloned().collect(),
        "Captured roster includes actors with no invocation"
    );
    Ok(
        json!({"case":name,"passed":true,"captured_roster":roster,"producer_ids":producers,
        "final_reviewer":final_reviews[0].agent_id,"usage":trace.usage,
        "actual_participant_ids":participants,"actual_participant_count":participants.len(),
        "native_context_owners":contexts,"scheduler_parallel_ceiling":2,"outer_invocation_ceiling":8,
        "native_inference":false,"allocation":"current BoundedAllocationPolicy; reputation influence disabled",
        "acceptance":"Engine independent review retained; no external quality or production confirmation supplied"}),
    )
}

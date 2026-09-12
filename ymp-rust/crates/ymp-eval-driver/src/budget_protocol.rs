//! Variable allowances are real captured admission requests; charged units come
//! from native Usage events after controlled barriers, never from expected traces.
use crate::{
    concurrency_protocol::{execute_reserved_with_allowance, HeldBackend},
    export, protocols,
    script::NativeJournal,
    workflows, write_json,
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Instant};
use tokio::sync::{mpsc, Notify};
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::TurnRequest;
use ymp_runtime::{mcp::TeamServer, Engine, WorkspaceAdmission};
use ymp_storage::Store;
pub async fn run(root: &Path, directory: &Path, spec: &Value) -> Result<Value> {
    let started = Instant::now();
    let work = directory.join("work");
    std::fs::create_dir(&work)?;
    let store = Store::open(&directory.join("metadata"))?;
    let mut config = workflows::config();
    config.limits.parallel = 3;
    let resources = config.limits.resources.as_mut().unwrap();
    resources.observed_tokens = spec["setup"]["total_units"].as_u64();
    resources.invocation_tokens = spec["script"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["action"] == "reserve")
        .filter_map(|a| a["units"].as_u64())
        .max();
    resources.review_reserve_tokens = spec["setup"]["review_reserve_units"].as_u64();
    resources.required_review_invocations = 1;
    let session = Session {
        id: new_id(),
        project_id: store.project(&work)?.id,
        title: spec["purpose"].as_str().unwrap().into(),
        status: "running".into(),
        created_at: now(),
        team: config.agents.clone(),
        turns_used: 0,
    };
    store.create_session(
        &session,
        &SessionPolicy {
            session_id: session.id.clone(),
            goal: session.title.clone(),
            constraints: None,
            cwd: work.canonicalize()?,
            limits: config.limits.clone(),
            eligible_pool: config.agents.clone(),
            captured_team: session.team.clone(),
            team_constraints: Some(config.team_constraints.clone()),
            execution: config.execution.clone(),
            assignment_settings: vec![],
            parent_session_id: None,
            evaluation: None,
            captured_at: now(),
        },
    )?;
    let (events, _) = mpsc::unbounded_channel();
    let server = Arc::new(TeamServer::start(store.clone(), &session, events.clone()).await?);
    let journal = Arc::new(NativeJournal::default());
    let owner_engine = Engine::new(
        store.clone(),
        config.clone(),
        events.clone(),
        CancellationToken::new(),
    )?;
    let owner = owner_engine.acquire_workspace_owner(&session.id)?;
    let mut pending = BTreeMap::new();
    let mut attempts = BTreeMap::new();
    let mut denial_links = BTreeMap::new();
    let mut boundary = Vec::new();
    for (index, action) in spec["script"].as_array().unwrap().iter().enumerate() {
        match action["action"].as_str().unwrap() {
            "charge" | "reserve" => {
                let agent = action["agent"].as_str().unwrap();
                let phase = action["phase"].as_str().unwrap();
                let alias = action["assignment"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("charge-{index}"));
                let units = action["units"].as_u64().unwrap();
                let actual = if action["action"] == "charge" {
                    units
                } else {
                    spec["script"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|a| a["action"] == "finish" && a["assignment"] == alias)
                        .and_then(|a| a["actual_units"].as_u64())
                        .unwrap_or(0)
                };
                let gate = Arc::new(Notify::new());
                let (notice, mut beginnings) = mpsc::unbounded_channel();
                let backend = Arc::new(HeldBackend {
                    gates: [(agent.into(), gate.clone())].into_iter().collect(),
                    started: notice,
                    journal: journal.clone(),
                    reported_tokens: actual,
                    reports_effort: true,
                });
                let engine = Engine::new(
                    store.clone(),
                    config.clone(),
                    events.clone(),
                    CancellationToken::new(),
                )?
                .with_execution_backend(backend.clone())?;
                let purpose = match phase {
                    "planning" => "plan",
                    "communication" => "conversation",
                    "execution" => "execution",
                    other => other,
                };
                let request = TurnRequest {
                    profile: config.agent(agent)?.clone(),
                    provider: config.providers[0].clone(),
                    settings: ExecutionSettings {
                        model: Some("scripted-small".into()),
                        effort: Some("low".into()),
                        permission_mode: Some("read_only".into()),
                    },
                    cwd: work.clone(),
                    prompt: serde_json::to_string(&WorkspaceAccess::ReadAll)?,
                    purpose: purpose.into(),
                    read_only: true,
                    resume: None,
                    usage_baseline: None,
                    mcp: None,
                    resource_controls: NativeResourceControls {
                        max_turns: Some(2),
                        max_output_chars: Some(2000),
                    },
                    timeout_secs: 10,
                    bridge: Path::new("").into(),
                };
                let before = store.trace(&session.id)?;
                let previous_seq = before.history.last().map_or(0, |e| e.seq);
                let WorkspaceAdmission::Acquired(lease) =
                    engine.try_reserve_workspace(&owner, &session.id, &request, None)?
                else {
                    anyhow::bail!("Unexpected resource deferral in token protocol")
                };
                let id = lease.assignment_id().to_owned();
                attempts.insert(id.clone(),json!({"alias":alias,"phase":phase,"action":action["action"],"units":units,"denied":false}));
                let mut handle = tokio::spawn(execute_reserved_with_allowance(
                    store.clone(),
                    server.clone(),
                    backend,
                    lease,
                    request,
                    session.id.clone(),
                    Some(units),
                ));
                let admitted = tokio::select! {notice=beginnings.recv()=>{ensure!(notice.as_deref()==Some(agent),"Unexpected native start");true},result=&mut handle=>{let result=result?;let error=result.err().context("Native work ended before the reservation barrier")?;let after=store.trace(&session.id)?;for event in after.history.iter().filter(|e|e.seq>previous_seq&&e.kind=="budget_denied"){denial_links.insert(event.seq,json!({"alias":alias,"in_flight":before.budget.as_ref().map_or(0,|b|b.in_flight_invocations)}));}attempts.get_mut(&id).unwrap()["denied"]=json!(true);boundary.push(json!({"action":action,"assignment_id":id,"error":error.to_string(),"invocations_before":before.invocations.len(),"invocations_after":after.invocations.len()}));false}};
                if admitted {
                    boundary.push(json!({"action":action,"assignment_id":id,"admitted":true}));
                    if action["action"] == "charge" {
                        gate.notify_one();
                        handle.await??;
                    } else {
                        pending.insert(alias, (gate, handle));
                    }
                }
            }
            "finish" => {
                let alias = action["assignment"].as_str().unwrap();
                let (gate, handle) = pending
                    .remove(alias)
                    .context("Finish has no admitted allowance")?;
                gate.notify_one();
                handle.await??;
            }
            action => anyhow::bail!("Unsupported budget action {action}"),
        }
    }
    for (alias, (gate, handle)) in pending {
        gate.notify_one();
        handle.await??;
        boundary.push(json!({"unexpected_active_allowance":alias}));
    }
    let trace = store.trace(&session.id)?;
    let mut rows = Vec::new();
    let mut sources = Vec::new();
    for event in &trace.history {
        let row = if event.kind == "budget_denied" {
            let context = denial_links.get(&event.seq);
            let code = event.data["code"].as_str().unwrap_or("unknown");
            let reason = match code {
                "token_review_reserve"
                    if context.is_some_and(|c| c["in_flight"].as_u64().unwrap_or(0) > 0) =>
                {
                    "in_flight_and_review_reserve"
                }
                "token_review_reserve" => "review_reserve",
                "token_limit" => "remaining_budget",
                other => other,
            };
            Some(
                json!({"type":"admission_denied","assignment":context.map(|c|c["alias"].clone()).unwrap_or(Value::Null),"reason":reason}),
            )
        } else if event.kind == "provenance" {
            match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
                ProvenanceEvent::AssignmentStarted { assignment, .. } => {
                    let request = attempts.get(&assignment.id);
                    if request.is_some_and(|r| r["action"] == "charge") {
                        None
                    } else {
                        Some(
                            json!({"type":"allowance_reserved","assignment":request.map(|r|r["alias"].clone()).unwrap_or(json!(assignment.id)),"agent":assignment.agent_id,"phase":request.map(|r|r["phase"].clone()).unwrap_or(json!(assignment.purpose)),"units":assignment.token_reservation.or(trace.policy.as_ref().and_then(|p|p.limits.resources.as_ref()).and_then(|r|r.invocation_tokens))}),
                        )
                    }
                }
                ProvenanceEvent::InvocationFinished { invocation } => {
                    let assignment = trace
                        .assignments
                        .iter()
                        .find(|a| a.id == invocation.assignment_id)
                        .unwrap();
                    let request = attempts.get(&assignment.id);
                    let units = invocation
                        .usage
                        .as_ref()
                        .and_then(|u| u.counts.known_total());
                    let mut row = json!({"type":"usage_charged","agent":assignment.agent_id,"phase":request.map(|r|r["phase"].clone()).unwrap_or(json!(assignment.purpose)),"units":units});
                    if request.is_none_or(|r| r["action"] != "charge") {
                        row["assignment"] = request
                            .map(|r| r["alias"].clone())
                            .unwrap_or(json!(assignment.id));
                        row["released_units"] = json!(units
                            .zip(assignment.token_reservation)
                            .map(|(actual, reserved)| reserved.saturating_sub(actual)));
                    }
                    Some(row)
                }
                _ => None,
            }
        } else {
            None
        };
        if let Some(row) = row {
            sources.push(json!({"projected_index":rows.len(),"runtime_event_seq":event.seq}));
            rows.push(row);
        }
    }
    let budget = trace.budget.as_ref().unwrap();
    let denied_started = trace
        .assignments
        .iter()
        .filter(|a| attempts.get(&a.id).is_some_and(|r| r["denied"] == true))
        .count();
    let observed = json!({"schema_version":1,"case_id":"budget-reservations","events":rows,"final_state":{"spent_units":trace.usage.total.known_total(),"reserved_units":budget.reserved_tokens,"remaining_units":budget.limits.resources.as_ref().and_then(|r|r.observed_tokens).zip(trace.usage.total.known_total()).map(|(limit,spent)|limit.saturating_sub(spent)),"usage_by_agent":trace.usage.agents.iter().map(|(agent,usage)|(agent.clone(),json!(usage.known_total()))).collect::<serde_json::Map<_,_>>(),"denied_invocations_started":denied_started}});
    write_json(&directory.join("runtime.json"), &trace)?;
    journal.save(directory)?;
    write_json(&directory.join("observed.json"), &observed)?;
    write_json(&directory.join("boundary-actions.json"), &boundary)?;
    write_json(
        &directory.join("alias-map.json"),
        &json!({"attempts":attempts,"denial_sources":denial_links,"projection_sources":sources}),
    )?;
    let metrics = export::metrics(&trace, started.elapsed().as_secs_f64(), 1)?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, "budget-reservations").await?;
    Ok(
        json!({"case_id":"budget-reservations","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}

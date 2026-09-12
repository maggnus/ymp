use crate::{
    concurrency_protocol::execute_reserved, export, protocols, script::NativeJournal, workflows,
    write_json,
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Instant};
use tokio::sync::{mpsc, Notify};
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};
use ymp_runtime::{mcp::TeamServer, Engine, WorkspaceAdmission};
use ymp_storage::Store;

pub async fn run(root: &Path, directory: &Path, spec: &Value) -> Result<Value> {
    let started = Instant::now();
    let work = directory.join("work");
    std::fs::create_dir(&work)?;
    let store = Store::open(&directory.join("metadata"))?;
    let gates = [("a".to_owned(), Arc::new(Notify::new()))]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let (notice, mut beginnings) = mpsc::unbounded_channel();
    let journal = Arc::new(NativeJournal::default());
    let tokens = spec["script"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["action"] == "finish")
        .unwrap()["tokens"]
        .as_u64()
        .unwrap();
    let mut engines = BTreeMap::new();
    let mut backends = BTreeMap::new();
    let mut sessions = BTreeMap::new();
    let mut servers = BTreeMap::new();
    for (name, pins) in spec["setup"]["sessions"].as_object().unwrap() {
        let mut config = workflows::config();
        for agent in &mut config.agents {
            agent.model = Some("model-one".into());
        }
        for policy in config.execution.values_mut() {
            policy.fixed = ModelEffort {
                model: pins["fixed_model"].as_str().map(str::to_owned),
                effort: pins["fixed_effort"].as_str().map(str::to_owned),
            };
            policy.defaults.effort = Some("low".into());
        }
        config.capabilities.insert(
            "scripted".into(),
            ProviderCapabilities {
                source: CapabilitySource::Configured,
                models_complete: true,
                default_model: Some("model-one".into()),
                models: spec["setup"]["capabilities"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(model, efforts)| ModelCapabilities {
                        id: model.clone(),
                        controls: Some(vec![NativeControl {
                            display_name: None,
                            value_names: Default::default(),
                            id: "effort".into(),
                            values: NativeControlValues::Choices {
                                options: efforts
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .map(|e| e.as_str().unwrap().into())
                                    .collect(),
                            },
                            default: None,
                        }]),
                        ..Default::default()
                    })
                    .collect(),
            },
        );
        let session = Session {
            id: new_id(),
            project_id: store.project(&work)?.id,
            title: "Native effort admission protocol".into(),
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
        let backend = Arc::new(EffortBackend {
            store: store.clone(),
            session: session.id.clone(),
            journal: journal.clone(),
            gate: gates["a"].clone(),
            started: notice.clone(),
            tokens,
        });
        let engine = Engine::new(store.clone(), config, events, CancellationToken::new())?
            .with_execution_backend(backend.clone())?;
        backends.insert(name.clone(), backend);
        engines.insert(name.clone(), engine);
        servers.insert(name.clone(), server);
        sessions.insert(name.clone(), session);
    }
    let first = sessions.keys().next().unwrap();
    let owner = engines[first].acquire_workspace_owner(&sessions[first].id)?;
    let mut pending = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    let mut calls = Vec::new();
    let mut count = 0;
    for action in spec["script"].as_array().unwrap() {
        match action["action"].as_str().unwrap() {
            "request_invocation" => {
                let name = action["session"].as_str().unwrap();
                let engine = &engines[name];
                let session = &sessions[name];
                let agent = action["agent"].as_str().unwrap();
                let request = TurnRequest {
                    profile: engine.config.agent(agent)?.clone(),
                    provider: engine.config.providers[0].clone(),
                    settings: ExecutionSettings {
                        model: action["model"].as_str().map(str::to_owned),
                        effort: action["effort"].as_str().map(str::to_owned),
                        permission_mode: Some("read_only".into()),
                    },
                    cwd: work.clone(),
                    prompt: serde_json::to_string(&WorkspaceAccess::ReadAll)?,
                    purpose: "conversation".into(),
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
                let before = store.trace(&session.id)?.invocations.len();
                match engine.try_reserve_workspace(&owner, &session.id, &request, None) {
                    Err(error) => {
                        let message = error.to_string();
                        store.event(&session.id, "evaluation_boundary_observed", &json!({"operation":"request_invocation","outcome":"denied","error":message,"model":request.settings.model,"effort":request.settings.effort}))?;
                        calls.push(json!({"request":action,"error":message,"invocations_before":before,"invocations_after":store.trace(&session.id)?.invocations.len()}));
                    }
                    Ok(WorkspaceAdmission::Deferred(wait)) => {
                        calls.push(json!({"request":action,"deferred":wait}));
                    }
                    Ok(WorkspaceAdmission::Acquired(lease)) => {
                        count += 1;
                        let alias = format!("{agent}{count}");
                        let assignment_id = lease.assignment_id().to_owned();
                        aliases.insert(alias.clone(), assignment_id.clone());
                        let handle = tokio::spawn(execute_reserved(
                            store.clone(),
                            servers[name].clone(),
                            backends[name].clone(),
                            lease,
                            request,
                            session.id.clone(),
                        ));
                        ensure!(
                            tokio::time::timeout(
                                std::time::Duration::from_secs(5),
                                beginnings.recv()
                            )
                            .await?
                            .as_deref()
                                == Some(agent),
                            "Unexpected native start"
                        );
                        pending.insert(alias, (name.to_owned(), agent.to_owned(), handle));
                        calls.push(json!({"request":action,"assignment_id":assignment_id,"invocations_before":before}));
                    }
                }
            }
            "finish" => {
                let alias = action["assignment"].as_str().unwrap();
                let (name, agent, handle) = pending
                    .remove(alias)
                    .context("Missing requested invocation")?;
                gates[&agent].notify_one();
                handle.await??;
                calls
                    .push(json!({"operation":"release_barrier","assignment":alias,"session":name}));
            }
            action => anyhow::bail!("Unknown effort action {action}"),
        }
    }
    for (alias, (name, agent, handle)) in pending {
        gates[&agent].notify_one();
        handle.await??;
        store.event(
            &sessions[&name].id,
            "evaluation_boundary_observed",
            &json!({"operation":"unexpected_barrier_release","assignment":alias}),
        )?;
    }
    let traces = sessions
        .iter()
        .map(|(name, session)| Ok((name.clone(), store.trace(&session.id)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let projected = project(&traces, &aliases, &journal.snapshot())?;
    let calls_count = traces.values().map(|t| t.invocations.len()).sum::<usize>();
    let known_effort = traces
        .values()
        .flat_map(|t| &t.invocations)
        .any(|i| i.reported.effort.is_some());
    let observed = json!({"schema_version":1,"case_id":"effort-support","events":projected.events,"final_state":{"invocations":calls_count,"model_calls_for_discovery":traces.values().flat_map(|t|&t.assignments).filter(|a|a.purpose=="discovery").count(),"effort_verified_from_native_report":known_effort}});
    write_json(&directory.join("observed.json"), &observed)?;
    write_json(&directory.join("runtime.json"), &traces)?;
    journal.save(directory)?;
    write_json(&directory.join("boundary-actions.json"), &calls)?;
    write_json(
        &directory.join("alias-map.json"),
        &json!({"projection_sources":projected.sources,"ordering":"Durable runtime and native observation sequence", "assignments":aliases,"sessions":sessions.iter().map(|(name,s)|(name.clone(),s.id.clone())).collect::<BTreeMap<_,_>>()}),
    )?;
    let metrics = traces
        .iter()
        .map(|(name, trace)| {
            Ok((
                name.clone(),
                export::metrics(trace, started.elapsed().as_secs_f64(), 1)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, "effort-support").await?;
    Ok(
        json!({"case_id":"effort-support","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}

struct EffortBackend {
    store: Store,
    session: String,
    journal: Arc<NativeJournal>,
    gate: Arc<Notify>,
    started: mpsc::UnboundedSender<String>,
    tokens: u64,
}
impl ExecutionBackend for EffortBackend {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.evals.effort-observations".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, _: &TurnRequest) -> WorkspaceAccess {
        WorkspaceAccess::ReadAll
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            let id = new_id();
            export::record_native(
                &self.store,
                &self.session,
                &self.journal,
                json!({"type":"native_started","native_id":id,"agent_id":request.profile.id,"settings":request.settings}),
            )?;
            let mut reported = request.settings.clone();
            reported.effort = None;
            events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
                sent: Some(request.settings.clone()),
                reported: Some(reported),
                native_session_id: Some(id.clone()),
                native_turn_id: Some(id.clone()),
                native_version: Some("scripted-effort-v1".into()),
                ..Default::default()
            })))?;
            self.started.send(request.profile.id.clone())?;
            self.gate.notified().await;
            let usage = UsageSnapshot {
                counts: TokenCounts {
                    input: Some(self.tokens),
                    output: Some(0),
                    ..TokenCounts::zero()
                },
                finalized: true,
                partial: false,
                note: Some("Controlled synthetic effort workload".into()),
                native_total: None,
            };
            events.send(ProviderEvent::Usage(usage.clone()))?;
            export::record_native(
                &self.store,
                &self.session,
                &self.journal,
                json!({"type":"native_closed","native_id":id,"usage":usage}),
            )?;
            Ok(TurnResult {
                text: "Read-only effort fixture completed".into(),
                session_id: id,
                usage: None,
            })
        })
    }
}

pub(crate) fn project(
    traces: &BTreeMap<String, SessionTrace>,
    aliases: &BTreeMap<String, String>,
    native: &[Value],
) -> Result<export::Projection> {
    let history = export::ordered_history(traces.values())?;
    export::audit_native_sources(&history, native)?;
    let mut projected = export::Projection::default();
    let mut lifecycle = export::LifecycleAudit::default();
    for event in history {
        let (session, trace) = traces
            .iter()
            .find(|(_, trace)| trace.session.id == event.session_id)
            .context("Unbound source session")?;
        if let Some(anomaly) = lifecycle.anomaly(event, trace, native, "native_closed")? {
            projected.push(anomaly, export::event_source(event));
            continue;
        }
        let value = if event.kind == "evaluation_native_observed" {
            let id = event.data["native_id"]
                .as_str()
                .context("Missing native identity")?;
            let invocation = trace
                .invocations
                .iter()
                .find(|invocation| invocation.native_session_id.as_deref() == Some(id));
            if let Some(invocation) = invocation {
                let assignment = trace
                    .assignments
                    .iter()
                    .find(|a| a.id == invocation.assignment_id)
                    .context("Native invocation lacks assignment")?;
                let alias = aliases
                    .iter()
                    .find(|(_, id)| **id == assignment.id)
                    .map(|(alias, _)| alias.as_str())
                    .unwrap_or(&assignment.id);
                match event.data["type"].as_str() {
                    Some("native_started") => Some(
                        json!({"type":"invocation_started","assignment":alias,"agent":assignment.agent_id,"model":invocation.requested.model,"requested_effort":invocation.requested.effort,"sent_effort":invocation.sent.effort,"reported_effort":invocation.reported.effort,"session":session}),
                    ),
                    Some("native_closed") => {
                        let usage: UsageSnapshot =
                            serde_json::from_value(event.data["usage"].clone())?;
                        Some(
                            json!({"type":"invocation_closed","assignment":alias,"agent":assignment.agent_id,"tokens":usage.counts.known_total(),"coverage":export::coverage(Some(&usage))}),
                        )
                    }
                    _ => Some(json!({"type":"unexpected_native_event","native":event.data})),
                }
            } else {
                Some(json!({"type":"unattributed_native_event","native":event.data}))
            }
        } else if event.kind == "evaluation_boundary_observed" {
            if event.data["outcome"] == "denied" {
                let error = event.data["error"]
                    .as_str()
                    .context("Missing actual denial")?;
                let reason = if error.contains("violates fixed model") {
                    "fixed_model"
                } else if error.contains("violates fixed effort") {
                    "fixed_effort"
                } else if error.contains("unsupported_effort") {
                    "unsupported_effort"
                } else {
                    error
                };
                Some(
                    json!({"type":"admission_denied","reason":reason,"model":event.data["model"],"effort":event.data["effort"],"session":session}),
                )
            } else {
                Some(json!({"type":"unexpected_boundary_event","observation":event.data}))
            }
        } else if event.kind == "provenance" {
            match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
                ProvenanceEvent::DecisionRecorded { decision }
                    if decision.kind == "assignment_waiting" =>
                {
                    Some(
                        json!({"type":"admission_deferred","reason":decision.links.workspace_wait.context("Missing actual wait")?.code,"session":session}),
                    )
                }

                _ => None,
            }
        } else {
            None
        };
        if let Some(value) = value {
            let source = if event.kind == "evaluation_native_observed" {
                export::native_source(event, native)?
            } else {
                export::event_source(event)
            };
            projected.push(value, source);
        }
    }
    Ok(projected)
}

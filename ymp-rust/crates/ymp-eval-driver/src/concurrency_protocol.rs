//! Deterministic barriers around actual one-shot Engine reservations and native
//! admission. No simulated lock state or fabricated assignment completion.
use crate::{export, protocols, script::NativeJournal, workflows, write_json};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Instant};
use tokio::sync::{mpsc, Notify};
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};
use ymp_runtime::{mcp::TeamServer, Engine, WorkspaceAdmission, WorkspaceReservation};
use ymp_storage::Store;

pub(crate) struct HeldBackend {
    pub(crate) gates: BTreeMap<String, Arc<Notify>>,
    pub(crate) started: mpsc::UnboundedSender<String>,
    pub(crate) journal: Arc<NativeJournal>,
    pub(crate) reported_tokens: u64,
    pub(crate) reports_effort: bool,
}
impl ExecutionBackend for HeldBackend {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.evals.scoped-barriers".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        serde_json::from_str(&request.prompt).unwrap_or(WorkspaceAccess::WriteAll)
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            let id = new_id();
            let access = self.workspace_access(&request);
            self.journal.push(json!({"type":"native_started","native_id":id,"agent_id":request.profile.id,"purpose":request.purpose,"settings":request.settings,"access":access}));
            events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
                sent: Some(request.settings.clone()),
                reported: Some(ExecutionSettings {
                    effort: if self.reports_effort {
                        request.settings.effort.clone()
                    } else {
                        None
                    },
                    ..request.settings.clone()
                }),
                native_session_id: Some(id.clone()),
                native_turn_id: Some(id.clone()),
                native_version: Some("scripted-scoped-v1".into()),
                ..Default::default()
            })))?;
            if let WorkspaceAccess::Scoped { reads, writes } = &access {
                for path in reads {
                    let _ = std::fs::read(request.cwd.join(path))?;
                }
                for path in writes {
                    let path = request.cwd.join(path);
                    std::fs::create_dir_all(path.parent().unwrap())?;
                    std::fs::write(
                        path,
                        format!("Observed bounded writer {}\n", request.profile.id),
                    )?;
                }
            }
            self.started.send(request.profile.id.clone())?;
            self.gates
                .get(&request.profile.id)
                .context("Missing native barrier")?
                .notified()
                .await;
            let usage = UsageSnapshot {
                counts: TokenCounts {
                    input: Some(self.reported_tokens),
                    output: Some(0),
                    ..TokenCounts::zero()
                },
                finalized: true,
                partial: false,
                note: Some("Controlled synthetic barrier workload".into()),
                native_total: None,
            };
            self.journal
                .push(json!({"type":"native_usage","native_id":id,"usage":usage}));
            events.send(ProviderEvent::Usage(usage))?;
            self.journal
                .push(json!({"type":"native_closed","native_id":id,"outcome":"completed"}));
            Ok(TurnResult {
                text: "Bounded resource operation completed".into(),
                session_id: id,
                usage: None,
            })
        })
    }
}
pub(crate) async fn execute_reserved(
    store: Store,
    server: Arc<TeamServer>,
    backend: Arc<dyn ExecutionBackend>,
    lease: Box<WorkspaceReservation>,
    request: TurnRequest,
    session: String,
) -> Result<()> {
    execute_reserved_with_allowance(store, server, backend, lease, request, session, None).await
}

pub(crate) async fn execute_reserved_with_allowance(
    store: Store,
    server: Arc<TeamServer>,
    backend: Arc<dyn ExecutionBackend>,
    lease: Box<WorkspaceReservation>,
    request: TurnRequest,
    session: String,
    token_reservation: Option<u64>,
) -> Result<()> {
    execute_reserved_with_operations(
        store,
        server,
        backend,
        lease,
        request,
        session,
        token_reservation,
        vec![TeamOperation::TeamRead],
    )
    .await
}

// This fixture helper passes each existing production boundary object unchanged.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_reserved_with_operations(
    store: Store,
    server: Arc<TeamServer>,
    backend: Arc<dyn ExecutionBackend>,
    mut lease: Box<WorkspaceReservation>,
    mut request: TurnRequest,
    session: String,
    token_reservation: Option<u64>,
    operations: Vec<TeamOperation>,
) -> Result<()> {
    let mut assignment = AssignmentRecord {
        token_reservation,
        agent_identity: None,
        id: lease.assignment_id().into(),
        session_id: session.clone(),
        task: None,
        agent_id: request.profile.id.clone(),
        agent_config_version: content_digest(&serde_json::to_string(&request.settings)?),
        provider_id: request.provider.id.clone(),
        purpose: request.purpose.clone(),
        reason: "One-shot admitted scoped protocol operation".into(),
        cwd: request.cwd.clone(),
        requested: request.settings.clone(),
        timeout_secs: request.timeout_secs,
        grant_ids: vec![],
        context: vec![
            ContextReference {
                kind: ContextKind::Prompt,
                id: content_digest(&request.prompt),
                session_id: Some(session.clone()),
                digest: Some(content_digest(&request.prompt)),
                included_chars: Some(request.prompt.chars().count()),
            },
            ContextReference {
                kind: ContextKind::ProfileInstructions,
                id: request.profile.version(&request.provider),
                session_id: None,
                digest: Some(content_digest(&request.profile.instructions)),
                included_chars: Some(request.profile.instructions.chars().count()),
            },
        ],
        state: InvocationState::Running,
        started_at: now(),
        ended_at: None,
    };
    let mut invocation = InvocationRecord {
        id: new_id(),
        session_id: session.clone(),
        assignment_id: assignment.id.clone(),
        execution_backend: Some(backend.identity()),
        turn: 1,
        requested: request.settings.clone(),
        sent: Default::default(),
        reported: Default::default(),
        resumed_from: request.resume.clone(),
        native_session_id: None,
        native_turn_id: None,
        native_version: None,
        state: InvocationState::Running,
        started_at: now(),
        ended_at: None,
        usage: None,
        terminal_reason: None,
    };
    let token =
        lease.admit_reserved(server.clone(), &mut assignment, &mut invocation, operations)?;
    request.mcp = Some(ymp_providers::McpEndpoint {
        command: "unused-scripted-stdio".into(),
        args: vec![server.socket.display().to_string()],
        token,
    });
    let (events, mut observed) = mpsc::unbounded_channel();
    let db = store.clone();
    let session_id = session.clone();
    let invocation_id = invocation.id.clone();
    let capture = tokio::spawn(async move {
        while let Some(event) = observed.recv().await {
            let observation = match event {
                ProviderEvent::Execution(observation) => Some(*observation),
                ProviderEvent::Usage(usage) => Some(InvocationObservation {
                    usage: Some(usage),
                    ..Default::default()
                }),
                ProviderEvent::Session(id) => Some(InvocationObservation {
                    native_session_id: Some(id),
                    ..Default::default()
                }),
                _ => None,
            };
            if let Some(observation) = observation {
                db.observe_invocation(&session_id, &invocation_id, &observation)?;
            }
        }
        anyhow::Ok(())
    });
    let result = lease
        .run_turn(
            backend.as_ref(),
            request,
            CancellationToken::new(),
            events.clone(),
        )
        .await;
    drop(events);
    capture.await??;
    server.finish(
        &invocation.id,
        if result.is_ok() {
            InvocationState::Completed
        } else {
            InvocationState::Failed
        },
        Some(if result.is_ok() {
            "Observed scripted native completion"
        } else {
            "Observed scripted native failure"
        }),
    )?;
    drop(lease);
    result?;
    Ok(())
}
pub async fn run(root: &Path, directory: &Path, spec: &Value) -> Result<Value> {
    let started = Instant::now();
    let work = directory.join("work");
    std::fs::create_dir(&work)?;
    for input in spec["inputs"].as_array().into_iter().flatten() {
        let input = input.as_str().unwrap();
        let path = work.join(input);
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::copy(root.join("ymp-evals/fixtures/universal").join(input), path)?;
    }
    let store = Store::open(&directory.join("metadata"))?;
    let mut config = workflows::config();
    let mut c = config.agents[0].clone();
    c.id = "c".into();
    c.name = "Scripted c".into();
    config.agents.push(c);
    config.team.push("c".into());
    config.team_constraints.fixed_roster = Some(config.team.clone());
    config.limits.parallel = spec["setup"]["max_active"].as_u64().unwrap() as usize;
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
    let gates = ["a", "b", "c"]
        .into_iter()
        .map(|agent| (agent.into(), Arc::new(Notify::new())))
        .collect::<BTreeMap<_, _>>();
    let (notice, mut beginnings) = mpsc::unbounded_channel();
    let journal = Arc::new(NativeJournal::default());
    let backend = Arc::new(HeldBackend {
        gates: gates.clone(),
        started: notice,
        journal: journal.clone(),
        reported_tokens: 1,
        reports_effort: true,
    });
    let (events, _) = mpsc::unbounded_channel();
    let engine = Engine::new(
        store.clone(),
        config,
        events.clone(),
        CancellationToken::new(),
    )?
    .with_execution_backend(backend.clone())?;
    let owner = engine.acquire_workspace_owner(&session.id)?;
    let server = Arc::new(TeamServer::start(store.clone(), &session, events).await?);
    let mut work_handles = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    let mut attempts = Vec::new();
    for action in spec["script"].as_array().unwrap() {
        let alias = action["assignment"]
            .as_str()
            .context("Scoped action needs an assignment alias")?;
        match action["action"].as_str().unwrap() {
            "admit_and_hold" | "request_admission" => {
                let agent = action["agent"].as_str().unwrap();
                let paths = |name| {
                    action[name]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|value| value.as_str().unwrap().into())
                        .collect()
                };
                let access = WorkspaceAccess::Scoped {
                    reads: paths("reads"),
                    writes: paths("writes"),
                };
                let read_only =
                    matches!(&access,WorkspaceAccess::Scoped{writes,..} if writes.is_empty());
                let request = TurnRequest {
                    profile: engine.config.agent(agent)?.clone(),
                    provider: engine.config.providers[0].clone(),
                    settings: ExecutionSettings {
                        model: Some("scripted-small".into()),
                        effort: Some("low".into()),
                        permission_mode: Some(if read_only { "read_only" } else { "write" }.into()),
                    },
                    cwd: work.clone(),
                    prompt: serde_json::to_string(&access)?,
                    purpose: "conversation".into(),
                    read_only,
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
                match engine.try_reserve_workspace(&owner, &session.id, &request, None)? {
                    WorkspaceAdmission::Deferred(wait) => {
                        let trace = store.trace(&session.id)?;
                        let record = trace
                            .decisions
                            .iter()
                            .rev()
                            .find(|d| d.kind == "assignment_waiting")
                            .context("Missing durable deferral")?;
                        let reservation = record
                            .links
                            .workspace_access
                            .as_ref()
                            .unwrap()
                            .reservation_id
                            .clone();
                        aliases.insert(reservation.clone(), alias.to_owned());
                        attempts.push(json!({"alias":alias,"reservation_id":reservation,"deferred":wait,"assignment_created":false,"invocations_observed":trace.invocations.len()}));
                    }
                    WorkspaceAdmission::Acquired(lease) => {
                        let id = lease.assignment_id().to_owned();
                        aliases.insert(id.clone(), alias.to_owned());
                        attempts.push(json!({"alias":alias,"reservation_id":id,"admitted":true}));
                        let handle = tokio::spawn(execute_reserved(
                            store.clone(),
                            server.clone(),
                            backend.clone(),
                            lease,
                            request,
                            session.id.clone(),
                        ));
                        ensure!(
                            beginnings.recv().await.as_deref() == Some(agent),
                            "Unexpected native start order"
                        );
                        work_handles.insert(alias.to_owned(), (agent.to_owned(), handle));
                    }
                }
            }
            "release_barrier" | "finish" => {
                let (agent, handle) = work_handles
                    .remove(alias)
                    .context("No active native assignment for barrier")?;
                gates[&agent].notify_one();
                handle.await??;
            }
            action => anyhow::bail!("Unknown concurrency action {action}"),
        }
    }
    let trace = store.trace(&session.id)?;
    let alias = |id: &str| aliases.get(id).cloned().unwrap_or_else(|| id.to_owned());
    let mut events = Vec::new();
    let mut sources = Vec::new();
    let mut active = std::collections::HashSet::new();
    let mut peak = 0;
    for event in &trace.history {
        if event.kind != "provenance" {
            continue;
        }
        let value: ProvenanceEvent = serde_json::from_value(event.data.clone())?;
        let projected = match value {
            ProvenanceEvent::AssignmentStarted { assignment, .. } => {
                active.insert(assignment.id.clone());
                peak = peak.max(active.len());
                let access = trace
                    .decisions
                    .iter()
                    .find(|d| {
                        d.kind == "workspace_access_admitted"
                            && d.links.assignment_id.as_ref() == Some(&assignment.id)
                    })
                    .and_then(|d| d.links.workspace_access.as_ref())
                    .context("Started assignment has no admitted access")?;
                match &access.effective_access {
                    WorkspaceAccess::Scoped { reads, writes } => Some(
                        json!({"type":"assignment_started","assignment":alias(&assignment.id),"agent":assignment.agent_id,"writes":writes,"reads":reads}),
                    ),
                    _ => Some(
                        json!({"type":"assignment_started","assignment":alias(&assignment.id),"agent":assignment.agent_id,"unexpected_access":access.effective_access}),
                    ),
                }
            }
            ProvenanceEvent::InvocationFinished { invocation } => {
                active.remove(&invocation.assignment_id);
                Some(
                    json!({"type":"assignment_closed","assignment":alias(&invocation.assignment_id)}),
                )
            }
            ProvenanceEvent::DecisionRecorded { decision }
                if decision.kind == "assignment_waiting" =>
            {
                let access = decision
                    .links
                    .workspace_access
                    .as_ref()
                    .context("Missing deferred scope")?;
                let wait = decision
                    .links
                    .workspace_wait
                    .as_ref()
                    .context("Missing deferral reason")?;
                Some(
                    json!({"type":"admission_deferred","assignment":alias(&access.reservation_id),"reason":wait.code,"holder":wait.holder.as_deref().map(alias)}),
                )
            }
            _ => None,
        };
        if let Some(value) = projected {
            sources.push(json!({"projected_index":events.len(),"runtime_event_seq":event.seq}));
            events.push(value);
        }
    }
    let mut writers = std::collections::HashSet::new();
    let mut conflict = false;
    for event in &events {
        if event["type"] == "assignment_started"
            && !event["writes"].as_array().is_none_or(Vec::is_empty)
        {
            if !writers.is_empty() {
                conflict = true;
            }
            writers.insert(event["assignment"].clone().to_string());
        } else if event["type"] == "assignment_closed" {
            writers.remove(&event["assignment"].clone().to_string());
        }
    }
    let observed = json!({"schema_version":1,"case_id":"concurrency-conflicts","events":events,"final_state":{"peak_active":peak,"conflicting_writers_overlapped":conflict,"active_assignments":active.into_iter().map(|id|alias(&id)).collect::<Vec<_>>(),"working_directory":work.canonicalize()?}});
    write_json(&directory.join("runtime.json"), &trace)?;
    journal.save(directory)?;
    write_json(
        &directory.join("alias-map.json"),
        &json!({"aliases":aliases,"projection_sources":sources}),
    )?;
    write_json(&directory.join("boundary-actions.json"), &attempts)?;
    write_json(&directory.join("observed.json"), &observed)?;
    let metrics = export::metrics(&trace, started.elapsed().as_secs_f64(), peak)?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, "concurrency-conflicts").await?;
    Ok(
        json!({"case_id":"concurrency-conflicts","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}

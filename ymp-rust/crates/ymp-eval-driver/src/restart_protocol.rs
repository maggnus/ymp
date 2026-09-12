//! Two-process interruption and actual Engine recovery. Capabilities use private
//! inherited process pipes and are deliberately absent from retained journals.
use crate::{export, file_digest, protocols, script::NativeJournal, workflows, write_json};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Instant,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};
use ymp_runtime::{mcp::TeamServer, Engine};
use ymp_storage::Store;

struct Backend {
    root: PathBuf,
    directory: PathBuf,
    recovering: bool,
    journal: Arc<NativeJournal>,
}
impl ExecutionBackend for Backend {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.evals.restart-process".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        if self.recovering {
            WorkspaceAccess::Scoped {
                reads: vec!["outputs/workshop.md".into(), "outputs/totals.csv".into()],
                writes: vec![],
            }
        } else if request.read_only {
            WorkspaceAccess::ReadAll
        } else {
            WorkspaceAccess::WriteAll
        }
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            let id = new_id();
            self.journal.push(json!({"type":"native_started","native_id":id,"agent_id":request.profile.id,"purpose":request.purpose,"prompt":request.prompt,"settings":request.settings,"process_id":std::process::id()}));
            events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
                sent: Some(request.settings.clone()),
                reported: Some(request.settings.clone()),
                native_session_id: Some("context-2".into()),
                native_turn_id: Some(id.clone()),
                native_version: Some("scripted-restart-v1".into()),
                ..Default::default()
            })))?;
            let (text, tokens) = if self.recovering {
                ensure!(
                    request.read_only && request.purpose == "review",
                    "Recovery attempted production before inspection"
                );
                let inspected = ["outputs/workshop.md", "outputs/totals.csv"].iter().map(|p| Ok(json!({"path":p,"sha256":file_digest(&request.cwd.join(p))?,"bytes":std::fs::read(request.cwd.join(p))?}))).collect::<Result<Vec<_>>>()?;
                self.journal.push(
                    json!({"type":"actual_files_inspected","native_id":id,"files":inspected}),
                );
                let store = Store::open(&self.directory.join("metadata"))?;
                let session = store.sessions(None)?.remove(0);
                let preserved = store
                    .trace(&session.id)?
                    .decisions
                    .into_iter()
                    .filter(|d| d.kind == "task_accepted")
                    .filter_map(|d| d.links.result.map(|r| r.id))
                    .collect::<Vec<_>>();
                let unresolved = store
                    .tasks(&session.id)?
                    .into_iter()
                    .filter(|t| t.state != TaskState::Accepted)
                    .map(|t| t.id)
                    .collect::<Vec<_>>();
                store.event(&session.id,"eval_recovery_inspected",&json!({"files":inspected,"preserved_result_ids":preserved,"unresolved_task_ids":unresolved,"response":{"approved":false,"reason":"The document remains valid; totals contains a partial header and must remain unresolved"}}))?;
                (json!({"approved":false,"reason":"The document remains valid; totals contains a partial header and must remain unresolved"}).to_string(),3)
            } else {
                match request.purpose.as_str() {
            "plan" => (json!({"summary":"Produce the document before totals","tasks":[{"title":"Produce document","description":"Create outputs/workshop.md from inputs/document.json","competence":"implementation","difficulty":"simple","access":"write","dependencies":[],"checks":[]},{"title":"Produce totals","description":"Produce outputs/totals.csv","competence":"implementation","difficulty":"simple","access":"write","dependencies":[0],"checks":[]}]}).to_string(),5),
            "review_plan" => (json!({"approved":true,"reason":"The dependency preserves the document before totals"}).to_string(),5),
            "execute" if request.prompt.contains("Execute this assigned task in the current working directory:\nProduce totals\n") => {
                std::fs::create_dir_all(request.cwd.join("outputs"))?;
                std::fs::write(request.cwd.join("outputs/totals.csv"),b"account,total_cents,posted_entries\nNorth,")?;
                let usage=usage(7,true); events.send(ProviderEvent::Usage(usage.clone()))?;
                self.journal.push(json!({"type":"partial_usage_and_write","native_id":id,"usage":usage,"path":"outputs/totals.csv","sha256":file_digest(&request.cwd.join("outputs/totals.csv"))?})); self.journal.save(&self.directory)?;
                let endpoint=request.mcp.as_ref().context("Missing actual capability")?;
                // stdout is a private parent-owned pipe, never an evidence file.
                writeln!(std::io::stdout(),"{}",json!({"token":endpoint.token,"native_id":id}))?;std::io::stdout().flush()?;
                std::future::pending::<Result<(String,u64)>>().await?
            },
            "execute" => {
                let output=tokio::process::Command::new("python3").arg(self.root.join("ymp-evals/driver/artifact_writer.py")).arg("document").arg(&request.cwd).kill_on_drop(true).output().await?;
                ensure!(output.status.success(),"Document production failed");
                self.journal.push(json!({"type":"document_written","native_id":id,"sha256":file_digest(&request.cwd.join("outputs/workshop.md"))?}));
                ("Created outputs/workshop.md from the declared source".into(),20)
            },
            "review" => { let bytes=std::fs::read(request.cwd.join("outputs/workshop.md"))?; self.journal.push(json!({"type":"document_inspected","native_id":id,"bytes":bytes})); (json!({"approved":true,"reason":"Inspected the actual complete document and runtime check"}).to_string(),10) },
            other => anyhow::bail!("Unexpected restart setup purpose {other}"),
        }
            };
            let usage = usage(tokens, false);
            events.send(ProviderEvent::Usage(usage.clone()))?;
            self.journal
                .push(json!({"type":"native_closed","native_id":id,"usage":usage}));
            if self.recovering {
                let store = Store::open(&self.directory.join("metadata"))?;
                let session = store.sessions(None)?.remove(0);
                store.event(
                    &session.id,
                    "eval_recovery_native_closed",
                    &json!({"native_id":id,"usage":usage}),
                )?;
            }
            Ok(TurnResult {
                text,
                session_id: "context-2".into(),
                usage: None,
            })
        })
    }
}
fn usage(tokens: u64, partial: bool) -> UsageSnapshot {
    UsageSnapshot {
        counts: TokenCounts {
            input: Some(tokens),
            output: Some(0),
            ..TokenCounts::zero()
        },
        finalized: !partial,
        partial,
        note: Some("Controlled restart workload units".into()),
        native_total: None,
    }
}
fn config() -> Config {
    let mut c = workflows::config();
    c.limits.attempts = 1;
    c.limits.turn_timeout_secs = 60;
    c.limits.resources = Some(ResourceLimits {
        unknown_usage: UnknownUsagePolicy::BoundedNative,
        observed_tokens: Some(100),
        invocation_tokens: Some(20),
        review_reserve_tokens: Some(20),
        required_review_invocations: 1,
        ..Default::default()
    });
    c
}
fn contract(root: &Path) -> AcceptanceContract {
    AcceptanceContract {
        knowledge_correction: None,
        task_title: "Produce document".into(),
        criteria: vec![AcceptanceCriterion {
            id: "document-content".into(),
            description: "The actual document satisfies the declared source requirements".into(),
        }],
        artifacts: vec!["outputs/workshop.md".into()],
        inputs: vec!["inputs/document.json".into()],
        checks: vec![TrustedCheck {
            id: "document-check".into(),
            criterion_ids: vec!["document-content".into()],
            assertion: CheckAssertion::Command {
                program: "/usr/bin/env".into(),
                args: vec![
                    "python3".into(),
                    root.join("ymp-evals/validators/universal.py")
                        .display()
                        .to_string(),
                    "artifact".into(),
                    "--case".into(),
                    "document".into(),
                    "--workdir".into(),
                    "{workdir}".into(),
                ],
                verifier_files: vec![
                    root.join("ymp-evals/validators/universal.py"),
                    root.join("ymp-evals/scenarios/universal-workflows.json"),
                ],
            },
        }],
    }
}
/// Internal subprocess mode, never a reference-output producer.
pub async fn worker(root: &Path, directory: &Path, stage: &str) -> Result<()> {
    let store = Store::open(&directory.join("metadata"))?;
    let (events, _) = mpsc::unbounded_channel();
    let journal = Arc::new(NativeJournal::default());
    let mut engine = Engine::new(
        store.clone(),
        config(),
        events.clone(),
        CancellationToken::new(),
    )?
    .with_execution_backend(Arc::new(Backend {
        root: root.into(),
        directory: directory.into(),
        recovering: stage == "inspect",
        journal: journal.clone(),
    }))?;
    engine.use_memory = false;
    if stage == "seed" {
        engine.acceptance_contracts = vec![contract(root)];
        let result = engine
            .run(
                &directory.join("work"),
                "Create a checked workshop document, then produce totals",
                None,
            )
            .await;
        write_json(
            &directory.join("unexpected-seed-end.json"),
            &json!({"result":format!("{result:?}")}),
        )?;
        anyhow::bail!("Seed ended before interruption")
    }
    ensure!(stage == "inspect", "Unknown internal stage");
    let mut private = String::new();
    std::io::stdin().read_to_string(&mut private)?;
    let private: Value = serde_json::from_str(&private)?;
    let session = store.session(private["session"].as_str().context("Missing session")?)?;
    {
        let _owner = engine.acquire_workspace_owner(&session.id)?;
        let _session_lock = store.lock_session(&session)?;
        store.interrupt_open_invocations(&session.id)?;
        let trace = store.trace(&session.id)?;
        let mut active = 0;
        for assignment in &trace.assignments {
            for id in &assignment.grant_ids {
                active += usize::from(store.team_grant(&session.id, id)?.revoked_at.is_none());
            }
        }
        store.event(
            &session.id,
            "eval_runtime_restarted",
            &json!({"process_id":std::process::id(),"restored_active_grants":active}),
        )?;
        let server = TeamServer::start(store.clone(), &session, events).await?;
        let mut socket = tokio::net::UnixStream::connect(&server.socket).await?;
        socket.write_all(format!("{}\n",json!({"token":private["token"],"request_id":new_id(),"name":"team_post","arguments":{"text":"Expired restart capability control"}})).as_bytes()).await?;
        let mut reply = String::new();
        BufReader::new(socket).read_line(&mut reply).await?;
        let reply: Value = serde_json::from_str(&reply)?;
        store.event(
            &session.id,
            "eval_expired_tool_response",
            &json!({"operation":"team_post","response":reply}),
        )?;
    }
    let result = engine
        .run(
            &directory.join("work"),
            "Inspect the actual interrupted files and preserve accepted work",
            Some(&session.id),
        )
        .await;
    write_json(
        &directory.join("recovery-outcome.json"),
        &json!({"result":format!("{result:?}")}),
    )?;
    write_json(&directory.join("runtime.json"), &store.trace(&session.id)?)?;
    write_json(&directory.join("recovery-native.json"), &journal.snapshot())?;
    Ok(())
}
pub async fn run(root: &Path, directory: &Path) -> Result<Value> {
    let started = Instant::now();
    let work = directory.join("work");
    std::fs::create_dir_all(work.join("inputs"))?;
    std::fs::copy(
        root.join("ymp-evals/fixtures/universal/inputs/document.json"),
        work.join("inputs/document.json"),
    )?;
    let binary = std::env::current_exe()?;
    let mut seed = tokio::process::Command::new(&binary)
        .arg("--output")
        .arg(directory)
        .args(["--restart-stage", "seed"])
        .stdout(Stdio::piped())
        .stderr(std::fs::File::create(directory.join("seed-stderr.log"))?)
        .kill_on_drop(true)
        .spawn()?;
    let seed_pid = seed.id();
    let mut line = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(50),
        BufReader::new(seed.stdout.take().unwrap()).read_line(&mut line),
    )
    .await??;
    ensure!(
        !line.is_empty(),
        "Seed exited before interruption; inspect unexpected-seed-end.json"
    );
    let private: Value = serde_json::from_str(&line)?;
    let store = Store::open(&directory.join("metadata"))?;
    let session = store.sessions(None)?.remove(0);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let trace = store.trace(&session.id)?;
            if trace.invocations.iter().any(|i| {
                i.state == InvocationState::Running
                    && i.usage.as_ref().and_then(|u| u.counts.known_total()) == Some(7)
            }) {
                break anyhow::Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await??;
    write_json(
        &directory.join("before-crash-runtime.json"),
        &store.trace(&session.id)?,
    )?;
    seed.start_kill()?;
    let seed_status = seed.wait().await?;
    store.event(&session.id,"eval_process_terminated",&json!({"process_id":seed_pid,"exit":seed_status.to_string(),"open_invocation_ids":store.trace(&session.id)?.invocations.iter().filter(|i|i.state==InvocationState::Running).map(|i|i.id.clone()).collect::<Vec<_>>()}))?;
    let mut inspect = tokio::process::Command::new(&binary)
        .arg("--output")
        .arg(directory)
        .args(["--restart-stage", "inspect"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(std::fs::File::create(directory.join("inspect-stderr.log"))?)
        .kill_on_drop(true)
        .spawn()?;
    let inspect_pid = inspect.id();
    let mut input = inspect.stdin.take().unwrap();
    input
        .write_all(
            serde_json::to_string(&json!({"session":session.id,"token":private["token"]}))?
                .as_bytes(),
        )
        .await?;
    drop(input);
    let inspected = inspect.wait_with_output().await?;
    write_json(
        &directory.join("processes.json"),
        &json!({"seed_pid":seed_pid,"seed_exit":seed_status.to_string(),"inspection_pid":inspect_pid,"inspection_exit":inspected.status.to_string(),"binary_sha256":file_digest(&binary)?,"capability_transport":"Private inherited pipes; secret payload is not retained"}),
    )?;
    ensure!(inspected.status.success(), "Recovery process failed");
    let trace = store.trace(&session.id)?;
    write_json(&directory.join("runtime.json"), &trace)?;
    let metrics = export::metrics(&trace, started.elapsed().as_secs_f64(), 1)?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    // Normalization is added only from actual records after the boundary outcome
    // is known; a denied inspection cannot receive a positive completion receipt.
    let before: SessionTrace = serde_json::from_value(crate::read_json(
        &directory.join("before-crash-runtime.json"),
    )?)?;
    let inspections = trace
        .invocations
        .iter()
        .filter(|i| {
            !before.invocations.iter().any(|old| old.id == i.id)
                && trace
                    .assignments
                    .iter()
                    .any(|a| a.id == i.assignment_id && a.purpose == "review")
        })
        .count();
    write_json(
        &directory.join("boundary-result.json"),
        &json!({"inspection_invocations":inspections,"budget":store.session_budget(&session.id)?,"recovery":crate::read_json(&directory.join("recovery-outcome.json"))?}),
    )?;
    ensure!(inspections>0,"Actual recovery inspection was not admitted; preserve captured 100-token policy and inspect boundary-result.json");
    let (observed, aliases) = project(&trace, &before)?;
    write_json(&directory.join("observed.json"), &observed)?;
    write_json(&directory.join("alias-map.json"), &aliases)?;
    let validation = protocols::validate(root, directory, "restart-inspection").await?;
    Ok(
        json!({"case_id":"restart-inspection","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}

fn project(trace: &SessionTrace, before: &SessionTrace) -> Result<(Value, Value)> {
    use std::collections::BTreeMap;
    let mut assignment_aliases = BTreeMap::new();
    let mut result_aliases = BTreeMap::new();
    let mut task_aliases = BTreeMap::new();
    for task in &trace.tasks {
        if task.title == "Produce totals" {
            task_aliases.insert(task.id.clone(), "totals".to_owned());
        }
    }
    for d in &before.decisions {
        if d.kind == "task_accepted" {
            if let Some(r) = &d.links.result {
                if r.artifacts
                    .iter()
                    .any(|a| a.path == Path::new("outputs/workshop.md"))
                {
                    result_aliases.insert(r.id.clone(), "r1".to_owned());
                }
            }
        }
    }
    for i in &before.invocations {
        if i.state == InvocationState::Running {
            assignment_aliases.insert(i.assignment_id.clone(), "a2".to_owned());
        }
    }
    let mut inspections = 0;
    for a in &trace.assignments {
        if !before.assignments.iter().any(|old| old.id == a.id) {
            inspections += usize::from(a.purpose == "review");
            assignment_aliases.insert(
                a.id.clone(),
                if a.purpose == "review" {
                    format!("inspect{inspections}")
                } else {
                    a.id.clone()
                },
            );
        }
    }
    let alias = |id: &str| {
        assignment_aliases
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.into())
    };
    let result_alias = |id: &str| result_aliases.get(id).cloned().unwrap_or_else(|| id.into());
    let task_alias = |id: &str| task_aliases.get(id).cloned().unwrap_or_else(|| id.into());
    let new_assignments = trace
        .assignments
        .iter()
        .filter(|a| !before.assignments.iter().any(|old| old.id == a.id))
        .collect::<Vec<_>>();
    let replays = new_assignments
        .iter()
        .filter(|a| a.purpose == "execute")
        .count();
    let mut projection = export::Projection::default();
    for event in export::ordered_history([trace])? {
        let mut source = export::event_source(event);
        match event.kind.as_str() {
            "eval_process_terminated"=>for id in event.data["open_invocation_ids"].as_array().context("Missing terminated invocation IDs")? {
                let invocation=trace.invocations.iter().find(|i|Some(i.id.as_str())==id.as_str()).context("Terminated invocation record missing")?;
                let a=trace.assignments.iter().find(|a|a.id==invocation.assignment_id).context("Missing interrupted assignment")?;
                source["invocation_id"]=json!(invocation.id);
                projection.push(json!({"type":"invocation_closed","assignment":alias(&a.id),"agent":a.agent_id,"outcome":invocation.state,"tokens":invocation.usage.as_ref().and_then(|u|u.counts.known_total()),"coverage":coverage(invocation.usage.as_ref())}),source.clone());
            },
            "eval_runtime_restarted"=>projection.push(json!({"type":"runtime_restarted","restored_active_grants":event.data["restored_active_grants"]}),source),
            "eval_expired_tool_response"=>{let reply=&event.data["response"];let error=reply["error"].as_str().unwrap_or("");projection.push(json!({"type":if reply["ok"]==false{"tool_denied"}else{"tool_allowed"},"assignment":"a2","operation":"board_post","reason":if error.contains("expired"){"stale_grant"}else{error}}),source);},
            "eval_recovery_inspected"=>projection.push(json!({"type":"recovery_inspected","preserved":event.data["preserved_result_ids"].as_array().unwrap().iter().map(|id|result_alias(id.as_str().unwrap())).collect::<Vec<_>>(),"unresolved":event.data["unresolved_task_ids"].as_array().unwrap().iter().map(|id|task_alias(id.as_str().unwrap())).collect::<Vec<_>>(),"repeated_uncertain_effects":replays>0}),source),
            "eval_recovery_native_closed"=>{let i=trace.invocations.iter().find(|i|i.native_turn_id.as_deref()==event.data["native_id"].as_str()).context("Closed native observation has no runtime invocation")?;let a=trace.assignments.iter().find(|a|a.id==i.assignment_id).unwrap();let usage:UsageSnapshot=serde_json::from_value(event.data["usage"].clone())?;source["invocation_id"]=json!(i.id);projection.push(json!({"type":"invocation_closed","assignment":alias(&a.id),"agent":a.agent_id,"tokens":usage.counts.known_total(),"coverage":coverage(Some(&usage))}),source);},
            "provenance"=>match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
                ProvenanceEvent::DecisionRecorded{decision} if decision.kind=="task_accepted"=>{let r=decision.links.result.context("Acceptance without result")?;for artifact in &r.artifacts{projection.push(json!({"type":"result_preserved","result":result_alias(&r.id),"artifact":artifact.path,"sha256":artifact.sha256,"confirmation":serde_json::to_value(&decision.outcome)?["confirmation"]}),source.clone());}},
                ProvenanceEvent::AssignmentStarted{assignment,..} if new_assignments.iter().any(|a|a.id==assignment.id)=>{
                    let access=trace.decisions.iter().find(|d|d.kind=="workspace_access_admitted"&&d.links.assignment_id.as_ref()==Some(&assignment.id)).and_then(|d|d.links.workspace_access.as_ref()).context("Missing real inspection access")?;
                    let (reads,writes)=match &access.effective_access {WorkspaceAccess::Scoped{reads,writes}=>(reads.clone(),writes.clone()),other=>{projection.push(json!({"type":"unexpected_workspace_access","assignment":alias(&assignment.id),"access":other}),source.clone());(vec![],vec![])}};
                    projection.push(json!({"type":"assignment_started","assignment":alias(&assignment.id),"agent":assignment.agent_id,"purpose":if assignment.purpose=="review"{"inspect_actual_files"}else{&assignment.purpose},"reads":reads,"writes":writes}),source);
                },
                ProvenanceEvent::GrantRevoked{grant} if assignment_aliases.contains_key(&grant.assignment_id)=>projection.push(json!({"type":"grant_revoked","assignment":alias(&grant.assignment_id)}),source),
                _=>{},
            },
            _=>{},
        }
    }
    let confirmed = trace
        .decisions
        .iter()
        .filter(|d| {
            d.kind == "task_accepted"
                && matches!(
                    d.outcome,
                    Some(DecisionOutcome::Accepted {
                        confirmation: ConfirmationStatus::Confirmed
                    })
                )
        })
        .filter_map(|d| d.links.result.as_ref())
        .filter(|r| {
            trace
                .tasks
                .iter()
                .any(|t| t.id == r.id && t.state == TaskState::Accepted)
        })
        .map(|r| result_alias(&r.id))
        .collect::<Vec<_>>();
    let budget = trace.budget.as_ref().context("Missing captured budget")?;
    let spent = budget.observed_usage.known_total();
    let total = budget
        .limits
        .resources
        .as_ref()
        .and_then(|r| r.observed_tokens);
    let inspection_units = trace
        .invocations
        .iter()
        .filter(|i| {
            new_assignments
                .iter()
                .any(|a| a.id == i.assignment_id && a.purpose == "review")
        })
        .filter_map(|i| i.usage.as_ref().and_then(|u| u.counts.known_total()))
        .sum::<u64>();
    let restarted = trace
        .history
        .iter()
        .filter(|e| e.kind == "eval_runtime_restarted")
        .any(|e| {
            e.data["restored_active_grants"]
                .as_u64()
                .is_none_or(|n| n > 0)
        });
    Ok((
        json!({"schema_version":1,"case_id":"restart-inspection","events":projection.events,"final_state":{"confirmed_results":confirmed,"automatic_production_replays":replays,"reported_spent_units":spent,"remaining_reported_units":total.zip(spent).map(|(total,spent)|total.saturating_sub(spent)),"usage_coverage":if budget.observed_usage.is_partial(){"partial"}else{"complete"},"old_role_restored":restarted,"inspection_units":inspection_units}}),
        json!({"assignments":assignment_aliases,"results":result_aliases,"tasks":task_aliases,"projection_sources":projection.sources,"process_close_source":"Observed OS process exit precedes runtime revocation; final terminal state and measured usage are linked by the exact invocation ID"}),
    ))
}
fn coverage(usage: Option<&UsageSnapshot>) -> &'static str {
    match usage {
        None => "unknown",
        Some(u)
            if u.partial
                || !u.finalized
                || u.counts.input.is_none()
                || u.counts.output.is_none() =>
        {
            "partial"
        }
        Some(_) => "complete",
    }
}

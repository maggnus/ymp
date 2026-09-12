use anyhow::Result;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};
use ymp_runtime::{mcp::TeamServer, *};
use ymp_storage::Store;

struct Scoped(AtomicUsize);
impl ExecutionBackend for Scoped {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "test.scoped-reservation".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        serde_json::from_str(&request.prompt).unwrap_or(if request.read_only {
            WorkspaceAccess::ReadAll
        } else {
            WorkspaceAccess::WriteAll
        })
    }
    fn execute(
        &self,
        _: TurnRequest,
        _: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(TurnResult{text:json!({"action":"answer","answer":"The requested read-only inspection is complete"}).to_string(),session_id:new_id(),usage:None})
        })
    }
}
struct Fixture {
    _directory: tempfile::TempDir,
    store: Store,
    engine: Engine,
    path: PathBuf,
    session: Session,
    backend: Arc<Scoped>,
    events: mpsc::UnboundedReceiver<UiEvent>,
}
impl Fixture {
    fn new(parallel: usize) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("work");
        std::fs::create_dir(&path).unwrap();
        let store = Store::open(&directory.path().join("state")).unwrap();
        let mut config = Config {
            providers: vec![ProviderConfig {
                id: "scripted".into(),
                kind: ProviderKind::Mock,
                command: "internal".into(),
                args: vec![],
                env_refs: Default::default(),
                enabled: true,
            }],
            agents: ["a", "b", "c"]
                .iter()
                .map(|id| AgentProfile {
                    id: (*id).into(),
                    name: (*id).into(),
                    provider: "scripted".into(),
                    model: None,
                    instructions: "Bounded scripted work".into(),
                    enabled: true,
                })
                .collect(),
            team: vec!["a".into(), "b".into(), "c".into()],
            limits: Limits {
                parallel,
                turns: 30,
                turn_timeout_secs: 10,
                attempts: 2,
                resources: Some(ResourceLimits::default()),
            },
            ..Default::default()
        };
        for id in ["a", "b", "c"] {
            config.execution.insert(
                id.into(),
                AgentExecutionPolicy {
                    fixed: ModelEffort {
                        model: None,
                        effort: Some("low".into()),
                    },
                    ..Default::default()
                },
            );
        }
        let (tx, events) = mpsc::unbounded_channel();
        let backend = Arc::new(Scoped(AtomicUsize::new(0)));
        let engine = Engine::new(store.clone(), config, tx, CancellationToken::new())
            .unwrap()
            .with_execution_backend(backend.clone())
            .unwrap();
        let session = capture(&store, &engine.config, &path);
        Self {
            _directory: directory,
            store,
            engine,
            path,
            session,
            backend,
            events,
        }
    }
    fn request(&self, agent: &str, access: WorkspaceAccess) -> TurnRequest {
        TurnRequest {
            profile: self.engine.config.agent(agent).unwrap().clone(),
            provider: self.engine.config.providers[0].clone(),
            settings: ExecutionSettings {
                effort: Some("low".into()),
                permission_mode: Some(
                    if matches!(access,WorkspaceAccess::Scoped{ref writes,..} if writes.is_empty())
                    {
                        "read_only"
                    } else {
                        "write"
                    }
                    .into(),
                ),
                ..Default::default()
            },
            cwd: self.path.clone(),
            prompt: serde_json::to_string(&access).unwrap(),
            purpose: "conversation".into(),
            read_only: matches!(access,WorkspaceAccess::Scoped{ref writes,..} if writes.is_empty()),
            resume: None,
            usage_baseline: None,
            mcp: None,
            resource_controls: NativeResourceControls {
                max_turns: Some(2),
                max_output_chars: Some(2000),
            },
            timeout_secs: 10,
            bridge: PathBuf::new(),
        }
    }
}
fn capture(store: &Store, config: &Config, path: &std::path::Path) -> Session {
    let session = Session {
        id: new_id(),
        project_id: store.project(path).unwrap().id,
        title: "Bounded workspace admission".into(),
        status: "running".into(),
        created_at: now(),
        team: config.agents.clone(),
        turns_used: 0,
    };
    store
        .create_session(
            &session,
            &SessionPolicy {
                session_id: session.id.clone(),
                goal: session.title.clone(),
                constraints: None,
                cwd: path.into(),
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
        )
        .unwrap();
    session
}
fn writer() -> WorkspaceAccess {
    WorkspaceAccess::Scoped {
        reads: vec![],
        writes: vec!["outputs/shared.txt".into()],
    }
}
fn reader(path: &str) -> WorkspaceAccess {
    WorkspaceAccess::Scoped {
        reads: vec![path.into()],
        writes: vec![],
    }
}
fn acquired(value: WorkspaceAdmission) -> WorkspaceReservation {
    match value {
        WorkspaceAdmission::Acquired(lease) => *lease,
        WorkspaceAdmission::Deferred(wait) => panic!("Unexpected deferral: {wait:?}"),
    }
}
#[test]
fn one_shot_conflict_and_capacity_probes_leave_no_assignment_grant_spend_or_queue() {
    let f = Fixture::new(3);
    let a = acquired(
        f.engine
            .try_reserve_workspace(&f.session.id, &f.request("a", writer()), None)
            .unwrap(),
    );
    let b = acquired(
        f.engine
            .try_reserve_workspace(
                &f.session.id,
                &f.request("b", reader("inputs/ledger.csv")),
                None,
            )
            .unwrap(),
    );
    for access in [writer(), reader("outputs/shared.txt")] {
        match f
            .engine
            .try_reserve_workspace(&f.session.id, &f.request("c", access), None)
            .unwrap()
        {
            WorkspaceAdmission::Deferred(wait) => {
                assert_eq!(wait.code, "resource_conflict");
                assert_eq!(wait.holder.as_deref(), Some(a.assignment_id()));
            }
            _ => panic!("Conflicting access was admitted"),
        }
    }
    let trace = f.store.trace(&f.session.id).unwrap();
    assert!(trace.assignments.is_empty());
    assert!(trace.invocations.is_empty());
    assert_eq!(trace.usage.total.calls, 0);
    drop(a);
    assert!(f.store.trace(&f.session.id).unwrap().assignments.is_empty());
    let c = acquired(
        f.engine
            .try_reserve_workspace(&f.session.id, &f.request("c", writer()), None)
            .unwrap(),
    );
    drop(b);
    drop(c);
    let mut f = Fixture::new(1);
    f.engine.config.limits.parallel = 99;
    let _a = acquired(
        f.engine
            .try_reserve_workspace(&f.session.id, &f.request("a", writer()), None)
            .unwrap(),
    );
    assert!(
        matches!(f.engine.try_reserve_workspace(&f.session.id,&f.request("b",reader("inputs/data")),None).unwrap(),WorkspaceAdmission::Deferred(wait) if wait.code=="concurrency_limit")
    );
}
struct FalseScope;
impl WorkspaceAccessPolicy for FalseScope {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "test.false-scope".into(),
            version: "1".into(),
        }
    }
    fn resolve(&self, _: &WorkspaceAccessInput<'_>) -> Result<WorkspaceAccess> {
        Ok(WorkspaceAccess::Scoped {
            reads: vec![],
            writes: vec![],
        })
    }
}
#[test]
fn policy_cannot_assert_false_enforced_scope_or_forge_native_configuration() {
    let mut f = Fixture::new(3);
    let request = f.request("a", writer());
    f.engine = f
        .engine
        .with_workspace_access_policy(Arc::new(FalseScope))
        .unwrap();
    assert!(f
        .engine
        .try_reserve_workspace(&f.session.id, &request, None)
        .err()
        .unwrap()
        .to_string()
        .contains("cannot narrow"));
    f.engine = f
        .engine
        .with_workspace_access_policy(Arc::new(DirectWorkspaceAccessPolicy))
        .unwrap();
    let mut forged = request.clone();
    forged.provider.kind = ProviderKind::Acp;
    assert!(f
        .engine
        .try_reserve_workspace(&f.session.id, &forged, None)
        .err()
        .unwrap()
        .to_string()
        .contains("native provider"));
    let _valid = acquired(
        f.engine
            .try_reserve_workspace(&f.session.id, &request, None)
            .unwrap(),
    );
}
#[tokio::test]
async fn actual_engine_waits_on_the_same_public_reservation_then_runs_after_release() {
    let mut f = Fixture::new(3);
    let lease = acquired(
        f.engine
            .try_reserve_workspace(&f.session.id, &f.request("a", writer()), None)
            .unwrap(),
    );
    let session = capture(&f.store, &f.engine.config, &f.path);
    let engine = f.engine.clone();
    let path = f.path.clone();
    let work = tokio::spawn(async move {
        engine
            .follow_up(
                &path,
                "Inspect this directory without changing it",
                &session.id,
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5),async{loop{if matches!(f.events.recv().await,Some(UiEvent::AgentStatus{status,..}) if status.contains("resource_conflict")){break;}}}).await.unwrap();
    assert_eq!(f.backend.0.load(Ordering::SeqCst), 0);
    drop(lease);
    assert!(work.await.unwrap().is_ok());
    assert_eq!(f.backend.0.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn lease_admission_binds_identity_and_drop_revokes_before_releasing_access() {
    let f = Fixture::new(3);
    let request = f.request("a", writer());
    let mut lease = acquired(
        f.engine
            .try_reserve_workspace(&f.session.id, &request, None)
            .unwrap(),
    );
    let server = Arc::new(
        TeamServer::start(f.store.clone(), &f.session, f.engine.events.clone())
            .await
            .unwrap(),
    );
    let mut assignment = AssignmentRecord {
        token_reservation: None,
        id: lease.assignment_id().into(),
        session_id: f.session.id.clone(),
        task: None,
        agent_id: "a".into(),
        agent_config_version: "scripted".into(),
        provider_id: "scripted".into(),
        purpose: request.purpose.clone(),
        reason: "Explicit bounded probe".into(),
        cwd: f.path.clone(),
        requested: request.settings.clone(),
        timeout_secs: 10,
        grant_ids: vec![],
        context: vec![ContextReference {
            kind: ContextKind::Prompt,
            id: content_digest(&request.prompt),
            session_id: Some(f.session.id.clone()),
            digest: Some(content_digest(&request.prompt)),
            included_chars: Some(request.prompt.chars().count()),
        }],
        state: InvocationState::Running,
        started_at: now(),
        ended_at: None,
    };
    let mut invocation = InvocationRecord {
        id: new_id(),
        session_id: f.session.id.clone(),
        assignment_id: assignment.id.clone(),
        execution_backend: Some(f.backend.identity()),
        turn: 1,
        requested: request.settings.clone(),
        sent: Default::default(),
        reported: Default::default(),
        resumed_from: None,
        native_session_id: None,
        native_turn_id: None,
        native_version: None,
        state: InvocationState::Running,
        started_at: now(),
        ended_at: None,
        usage: None,
        terminal_reason: None,
    };
    assignment.agent_id = "b".into();
    assert!(lease
        .admit_reserved(
            server.clone(),
            &mut assignment,
            &mut invocation,
            TeamOperation::coordination()
        )
        .is_err());
    assert!(f.store.trace(&f.session.id).unwrap().invocations.is_empty());
    assignment.agent_id = "a".into();
    let token = lease
        .admit_reserved(
            server.clone(),
            &mut assignment,
            &mut invocation,
            TeamOperation::coordination(),
        )
        .unwrap();
    let (events, _) = mpsc::unbounded_channel();
    let mut changed = request.clone();
    changed.prompt = serde_json::to_string(&WorkspaceAccess::WriteAll).unwrap();
    assert!(lease
        .run_turn(
            f.backend.as_ref(),
            changed,
            CancellationToken::new(),
            events.clone()
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("changed after reservation"));
    assert_eq!(f.backend.0.load(Ordering::SeqCst), 0);
    lease
        .run_turn(
            f.backend.as_ref(),
            request.clone(),
            CancellationToken::new(),
            events.clone(),
        )
        .await
        .unwrap();
    assert!(lease
        .run_turn(
            f.backend.as_ref(),
            request,
            CancellationToken::new(),
            events
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("replay requires fresh admission"));
    assert_eq!(f.backend.0.load(Ordering::SeqCst), 1);
    drop(lease);
    assert_eq!(
        f.store
            .invocation(&f.session.id, &invocation.id)
            .unwrap()
            .state,
        InvocationState::Interrupted
    );
    let mut stream = UnixStream::connect(&server.socket).await.unwrap();
    stream.write_all(format!("{}\n",json!({"token":token,"request_id":new_id(),"name":"team_post","arguments":{"text":"Expired role"}})).as_bytes()).await.unwrap();
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&response).unwrap()["ok"],
        false
    );
    let _next = acquired(
        f.engine
            .try_reserve_workspace(&f.session.id, &f.request("b", writer()), None)
            .unwrap(),
    );
}

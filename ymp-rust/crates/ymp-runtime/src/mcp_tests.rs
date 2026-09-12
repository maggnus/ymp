use super::*;
use ymp_core::*;

struct Fixture {
    _dir: tempfile::TempDir,
    store: Store,
    session: Session,
    server: TeamServer,
    assignment: AssignmentRecord,
    invocation: InvocationRecord,
    token: String,
}
impl Fixture {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("state")).unwrap();
        let project = store.project(dir.path()).unwrap();
        let profile = Config::default().agents.remove(0);
        let session = Session {
            id: new_id(),
            project_id: project.id,
            title: "grant controls".into(),
            status: "running".into(),
            created_at: now(),
            team: vec![profile.clone()],
            turns_used: 0,
        };
        store.save_session(&session).unwrap();
        let (tx, _) = mpsc::unbounded_channel();
        let server = TeamServer::start(store.clone(), &session, tx)
            .await
            .unwrap();
        let mut assignment = AssignmentRecord {
            id: new_id(),
            session_id: session.id.clone(),
            task: None,
            agent_id: profile.id.clone(),
            agent_config_version: "fixture".into(),
            provider_id: profile.provider,
            purpose: "plan".into(),
            reason: "fixture".into(),
            cwd: dir.path().into(),
            requested: ExecutionSettings::default(),
            timeout_secs: 10,
            grant_ids: vec![],
            context: vec![],
            state: InvocationState::Running,
            started_at: now(),
            ended_at: None,
        };
        let invocation = InvocationRecord {
            id: new_id(),
            session_id: session.id.clone(),
            assignment_id: assignment.id.clone(),
            turn: 1,
            requested: assignment.requested.clone(),
            sent: ExecutionSettings::default(),
            reported: ExecutionSettings::default(),
            resumed_from: Some("saved-native-context".into()),
            native_session_id: None,
            native_turn_id: None,
            native_version: None,
            state: InvocationState::Running,
            started_at: now(),
            ended_at: None,
            usage: None,
            terminal_reason: None,
        };
        let token = server
            .admit(&mut assignment, &invocation, TeamOperation::coordination())
            .unwrap();
        Self {
            _dir: dir,
            store,
            session,
            server,
            assignment,
            invocation,
            token,
        }
    }
    fn request(&self, id: &str) -> Value {
        json!({"token":self.token,"request_id":id,"name":"team_post","arguments":{"text":"test"}})
    }
    async fn call(&self, request: Value) -> Value {
        call(&self.server.socket, request).await
    }
}
async fn call(socket: &Path, request: Value) -> Value {
    let mut stream = UnixStream::connect(socket).await.unwrap();
    stream
        .write_all(format!("{request}\n").as_bytes())
        .await
        .unwrap();
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).await.unwrap();
    serde_json::from_str(&line).unwrap()
}

#[tokio::test]
async fn terminal_assignment_credentials_reject_completion_failure_and_cancellation() {
    for state in [
        InvocationState::Completed,
        InvocationState::Failed,
        InvocationState::Cancelled,
        InvocationState::Interrupted,
    ] {
        let f = Fixture::new().await;
        f.store
            .finish_invocation(&f.session.id, &f.invocation.id, state, None)
            .unwrap();
        assert_eq!(
            f.call(f.request("stale")).await["ok"],
            false,
            "terminal {state:?} retained authority"
        );
        assert!(f.store.messages(&f.session.id, 0, 100).unwrap().is_empty());
    }
}
#[tokio::test]
async fn cross_assignment_and_session_arguments_are_rejected() {
    let f = Fixture::new().await;
    for field in [
        "session_id",
        "session",
        "assignment_id",
        "agent_id",
        "author",
        "grant_id",
        "invocation_id",
    ] {
        let mut request = f.request(field);
        request["arguments"][field] = json!("forged");
        assert_eq!(
            f.call(request).await["ok"],
            false,
            "caller overrode {field}"
        );
    }
    assert!(f.store.messages(&f.session.id, 0, 100).unwrap().is_empty());
}
#[tokio::test]
async fn concurrent_replayed_request_has_only_one_effect() {
    let f = Fixture::new().await;
    let request = f.request("same-request");
    let (a, b) = tokio::join!(f.call(request.clone()), f.call(request));
    assert_eq!([a, b].iter().filter(|v| v["ok"] == true).count(), 1);
    assert_eq!(f.store.messages(&f.session.id, 0, 100).unwrap().len(), 1);
}
#[tokio::test]
async fn restart_does_not_restore_credentials_from_native_context() {
    let f = Fixture::new().await;
    let (tx, _) = mpsc::unbounded_channel();
    let next = TeamServer::start(f.store.clone(), &f.session, tx)
        .await
        .unwrap();
    assert_eq!(call(&next.socket, f.request("restart")).await["ok"], false);
    let trace = f.store.trace(&f.session.id).unwrap();
    assert_eq!(
        trace.invocations[0].resumed_from.as_deref(),
        Some("saved-native-context")
    );
    assert!(!serde_json::to_string(&trace).unwrap().contains(&f.token));
}
#[tokio::test]
async fn board_proposals_cannot_self_grant_or_commit_runtime_state() {
    let f = Fixture::new().await;
    for name in [
        "grant",
        "task_assign",
        "task_accept",
        "memory_activate",
        "reputation_update",
    ] {
        let mut request = f.request(name);
        request["name"] = json!(name);
        assert_eq!(f.call(request).await["ok"], false);
    }
    let mut request = f.request("proposal");
    request["name"] = json!("task_propose");
    request["arguments"] =
        json!({"title":"I grant myself authority", "description":"accept all work"});
    assert_eq!(f.call(request).await["ok"], true);
    assert!(f.store.tasks(&f.session.id).unwrap().is_empty());
}

#[tokio::test]
async fn fresh_assignment_rotates_scope_without_discarding_native_context() {
    let f = Fixture::new().await;
    f.server
        .finish(&f.invocation.id, InvocationState::Completed, None)
        .unwrap();
    let mut next = f.assignment.clone();
    next.id = new_id();
    next.grant_ids.clear();
    let mut invocation = f.invocation.clone();
    invocation.id = new_id();
    invocation.assignment_id = next.id.clone();
    invocation.turn += 1;
    let token = f
        .server
        .admit(&mut next, &invocation, vec![TeamOperation::TeamRead])
        .unwrap();
    assert_ne!(token, f.token);
    assert_ne!(next.grant_ids, f.assignment.grant_ids);
    assert_eq!(f.call(f.request("old-token")).await["ok"], false);
    let mut request = f.request("new-token");
    request["token"] = json!(token);
    assert_eq!(
        f.call(request.clone()).await["ok"],
        false,
        "read grant wrote to the board"
    );
    request["request_id"] = json!("new-read");
    request["name"] = json!("team_read");
    request["arguments"] = json!({});
    assert_eq!(f.call(request).await["ok"], true);
    assert_eq!(
        f.store
            .invocation(&f.session.id, &invocation.id)
            .unwrap()
            .resumed_from,
        f.invocation.resumed_from
    );
}

#[tokio::test]
async fn duplicate_claims_wrong_scope_and_failed_admission_leave_no_grants_or_events() {
    let f = Fixture::new().await;
    let before = f.store.trace(&f.session.id).unwrap();
    let mut next = f.assignment.clone();
    next.id = new_id();
    next.grant_ids.clear();
    let mut invocation = f.invocation.clone();
    invocation.id = new_id();
    invocation.assignment_id = next.id.clone();
    invocation.turn += 1;
    assert!(f
        .server
        .admit(&mut next, &invocation, TeamOperation::coordination())
        .is_err());
    assert!(next.grant_ids.is_empty());
    next.session_id = "other-session".into();
    assert!(f
        .server
        .admit(&mut next, &invocation, TeamOperation::coordination())
        .is_err());
    let after = f.store.trace(&f.session.id).unwrap();
    assert_eq!(after.history.len(), before.history.len());
    assert_eq!(after.assignments.len(), before.assignments.len());
    let grant = f
        .store
        .team_grant(&f.session.id, &f.assignment.grant_ids[0])
        .unwrap();
    assert!(f.store.team_grant("other-session", &grant.id).is_err());
    assert_eq!(f.call(f.request("still-current")).await["ok"], true);
}

#[tokio::test]
async fn same_token_is_rejected_by_another_session_server() {
    let f = Fixture::new().await;
    let mut other = f.session.clone();
    other.id = new_id();
    f.store.save_session(&other).unwrap();
    let (tx, _) = mpsc::unbounded_channel();
    let server = TeamServer::start(f.store.clone(), &other, tx)
        .await
        .unwrap();
    assert_eq!(
        call(&server.socket, f.request("cross-session")).await["ok"],
        false
    );
    assert!(f.store.messages(&other.id, 0, 10).unwrap().is_empty());
}

#[tokio::test]
async fn task_reassignment_revokes_the_previous_attempt_in_the_same_commit() {
    let mut f = Fixture::new().await;
    f.server
        .finish(&f.invocation.id, InvocationState::Completed, None)
        .unwrap();
    let mut task = Task {
        id: new_id(),
        session_id: f.session.id.clone(),
        title: "work".into(),
        description: "work".into(),
        competence: "implementation".into(),
        difficulty: "simple".into(),
        dependencies: vec![],
        checks: vec![],
        state: TaskState::Running,
        assignee: Some(f.assignment.agent_id.clone()),
        reviewer: None,
        attempts: 1,
        result: None,
        workspace: None,
        base_commit: None,
        interrupted: false,
    };
    f.store.save_task(&task).unwrap();
    f.assignment.id = new_id();
    f.assignment.grant_ids.clear();
    f.assignment.task = Some(TaskAttemptRef::from(&task));
    f.assignment.purpose = "execute".into();
    f.invocation.id = new_id();
    f.invocation.assignment_id = f.assignment.id.clone();
    f.invocation.turn = 2;
    f.token = f
        .server
        .admit(
            &mut f.assignment,
            &f.invocation,
            TeamOperation::coordination(),
        )
        .unwrap();
    task.state = TaskState::Ready;
    task.assignee = None;
    f.store.save_task(&task).unwrap();
    assert_eq!(f.call(f.request("after-reassignment")).await["ok"], false);
    let grant = f
        .store
        .team_grant(&f.session.id, &f.assignment.grant_ids[0])
        .unwrap();
    assert!(grant.revoked_at.is_some());
    assert_eq!(
        f.store
            .invocation(&f.session.id, &f.invocation.id)
            .unwrap()
            .state,
        InvocationState::Interrupted
    );
    // A stale claim over the prior attempt cannot be admitted again.
    let mut stale = f.assignment.clone();
    stale.id = new_id();
    stale.grant_ids.clear();
    let mut invocation = f.invocation.clone();
    invocation.id = new_id();
    invocation.assignment_id = stale.id.clone();
    invocation.turn = 3;
    assert!(f
        .server
        .admit(&mut stale, &invocation, TeamOperation::coordination())
        .is_err());
}

#[tokio::test]
async fn concurrent_revocation_cannot_commit_an_operation_after_the_terminal_event() {
    let f = Fixture::new().await;
    let request = f.request("racing-cancel");
    let (_, _) = tokio::join!(f.call(request), async {
        f.server
            .finish(&f.invocation.id, InvocationState::Cancelled, None)
            .unwrap()
    });
    let trace = f.store.trace(&f.session.id).unwrap();
    let terminal = trace
        .history
        .iter()
        .position(|e| e.data["change"] == "invocation_finished")
        .unwrap();
    assert!(!trace.history[terminal + 1..]
        .iter()
        .any(|e| e.data["change"] == "team_operation_committed"));
    assert_eq!(f.call(f.request("after-cancel")).await["ok"], false);
}

#[tokio::test]
async fn grant_event_rolls_back_when_later_invocation_insert_fails() {
    let f = Fixture::new().await;
    f.server
        .finish(&f.invocation.id, InvocationState::Completed, None)
        .unwrap();
    let before = f.store.trace(&f.session.id).unwrap();
    let mut assignment = f.assignment.clone();
    assignment.id = new_id();
    assignment.grant_ids.clear();
    let mut invocation = f.invocation.clone();
    invocation.turn = 2;
    invocation.assignment_id = assignment.id.clone();
    // The next ordinal is valid; the reused invocation ID collides only after
    // grant issuance and assignment insertion inside the transaction.
    let grant =
        GrantRecord::for_assignment(&assignment, &invocation, TeamOperation::coordination());
    assignment.grant_ids.push(grant.id.clone());
    assert!(f
        .store
        .begin_invocation_with_grants(&assignment, &invocation, std::slice::from_ref(&grant))
        .is_err());
    assert!(f.store.team_grant(&f.session.id, &grant.id).is_err());
    let after = f.store.trace(&f.session.id).unwrap();
    assert_eq!(after.history.len(), before.history.len());
    assert_eq!(after.assignments.len(), before.assignments.len());
}

#[tokio::test]
async fn competing_runtime_admissions_commit_one_assignment() {
    let f = Fixture::new().await;
    f.server
        .finish(&f.invocation.id, InvocationState::Completed, None)
        .unwrap();
    let results = std::thread::scope(|scope| {
        let handles = (2..4)
            .map(|turn| {
                let mut assignment = f.assignment.clone();
                assignment.id = new_id();
                assignment.grant_ids.clear();
                let mut invocation = f.invocation.clone();
                invocation.id = new_id();
                invocation.assignment_id = assignment.id.clone();
                invocation.turn = turn;
                let server = &f.server;
                scope.spawn(move || {
                    server
                        .admit(&mut assignment, &invocation, TeamOperation::coordination())
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.into_iter().filter(|ok| *ok).count(), 1);
    let trace = f.store.trace(&f.session.id).unwrap();
    assert_eq!(
        trace
            .assignments
            .iter()
            .filter(|a| a.state == InvocationState::Running)
            .count(),
        1
    );
}

#[tokio::test]
async fn server_drop_ends_grants_and_restart_requires_fresh_admission() {
    let f = Fixture::new().await;
    let Fixture {
        _dir,
        store,
        session,
        server,
        assignment,
        invocation,
        token,
    } = f;
    let socket = server.socket.clone();
    drop(server);
    assert!(!socket.exists());
    assert_eq!(
        store.invocation(&session.id, &invocation.id).unwrap().state,
        InvocationState::Interrupted
    );
    assert!(store
        .team_grant(&session.id, &assignment.grant_ids[0])
        .unwrap()
        .revoked_at
        .is_some());
    let (tx, _) = mpsc::unbounded_channel();
    let next = TeamServer::start(store, &session, tx).await.unwrap();
    assert_eq!(
        call(
            &next.socket,
            json!({"token":token,"request_id":"restart","name":"team_read","arguments":{}})
        )
        .await["ok"],
        false
    );
}

#[tokio::test]
async fn memory_proposal_records_bound_origin_without_activation() {
    let f = Fixture::new().await;
    let mut request = f.request("memory-self-activation");
    request["name"] = json!("memory_propose");
    request["arguments"] = json!({"title":"lesson","content":"check the output","status":"active"});
    assert_eq!(f.call(request.clone()).await["ok"], false);
    request["request_id"] = json!("memory-proposal");
    request["arguments"]
        .as_object_mut()
        .unwrap()
        .remove("status");
    let response = f.call(request).await;
    assert_eq!(response["ok"], true);
    assert_eq!(response["value"]["status"], "proposed");
    let memory = f.store.proposed_memory(&f.session.id).unwrap();
    assert_eq!(memory.len(), 1);
    assert_eq!(memory[0].id, response["value"]["id"]);
    assert_eq!(memory[0].author, f.assignment.agent_id);
    assert_eq!(memory[0].source_session, f.session.id);
    assert!(f
        .store
        .memory(Some(&f.session.project_id), "lesson")
        .unwrap()
        .is_empty());
    let trace = f.store.trace(&f.session.id).unwrap();
    let event = trace
        .history
        .iter()
        .find(|e| e.data["change"] == "team_operation_committed")
        .unwrap();
    assert_eq!(event.data["memory_id"], memory[0].id);
    assert_eq!(event.data["grant_id"], f.assignment.grant_ids[0]);
    assert_eq!(event.data["assignment_id"], f.assignment.id);
    assert_eq!(event.data["invocation_id"], f.invocation.id);
}

include!("budget_authority_tests.rs");

//! Joint public consumers for accepted allocation and incremental knowledge.
use anyhow::{bail, ensure, Result};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{
    ExecutionBackend, ExecutionFuture, NativeExecutionBackend, ProviderEvent, TurnRequest,
};
use ymp_runtime::*;
use ymp_storage::Store;

struct CapturingBackend {
    requests: Mutex<Vec<TurnRequest>>,
    fail_planning: AtomicBool,
}
impl ExecutionBackend for CapturingBackend {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "test.joint-state".into(),
            version: "1".into(),
        }
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            ensure!(
                request.provider.kind == ProviderKind::Mock,
                "Only mock execution is allowed"
            );
            self.requests.lock().unwrap().push(request.clone());
            if request.purpose == "plan" && self.fail_planning.load(Ordering::SeqCst) {
                bail!("Scripted later-session planning failure");
            }
            if request.purpose == "execute"
                && request
                    .prompt
                    .rsplit("Your current assignment")
                    .next()
                    .unwrap()
                    .contains("Create another greeting")
            {
                bail!("Scripted later sibling failure");
            }
            NativeExecutionBackend.execute(request, events).await
        })
    }
}
struct ReplaceMembers(Vec<String>);
impl AllocationPolicy for ReplaceMembers {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "test.joint-replacement".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, input: &AllocationInput) -> Result<AllocationProposal> {
        let mut proposal = BoundedAllocationPolicy.propose(input)?;
        proposal.members = self.0.clone();
        proposal.method = "retain_checked_work_during_replacement".into();
        proposal.rationale =
            "Replace an idle producer while preserving independent review eligibility".into();
        Ok(proposal)
    }
}
fn idle() -> AllocationDemand {
    AllocationDemand {
        purpose: "execute".into(),
        task_id: None,
        competence: "implementation".into(),
        difficulty: "simple".into(),
        risk: TaskRisk::Standard,
        ready_work: 0,
    }
}
fn admission(store: &Store, session: &str, agent: &str) -> (AssignmentRecord, InvocationRecord) {
    let trace = store.trace(session).unwrap();
    let mut task = trace
        .tasks
        .iter()
        .find(|task| task.state == TaskState::Accepted)
        .unwrap()
        .clone();
    task.id = new_id();
    task.state = TaskState::Ready;
    task.assignee = None;
    task.reviewer = None;
    task.attempts = 0;
    task.result = None;
    task.assign(agent, &HashSet::new()).unwrap();
    store.save_task(&task).unwrap();
    let mut assignment = trace
        .assignments
        .iter()
        .find(|a| a.purpose == "execute")
        .unwrap()
        .clone();
    assignment.id = new_id();
    assignment.agent_id = agent.into();
    assignment.task = Some(TaskAttemptRef::from(&task));
    assignment.state = InvocationState::Running;
    assignment.ended_at = None;
    assignment.grant_ids.clear();
    let mut invocation = trace
        .invocations
        .iter()
        .find(|i| {
            i.assignment_id
                == trace
                    .assignments
                    .iter()
                    .find(|a| a.purpose == "execute")
                    .unwrap()
                    .id
        })
        .unwrap()
        .clone();
    invocation.id = new_id();
    invocation.assignment_id = assignment.id.clone();
    invocation.turn = store.session_usage(session).unwrap().total.calls + 1;
    invocation.state = InvocationState::Running;
    invocation.ended_at = None;
    invocation.usage = None;
    invocation.terminal_reason = None;
    (assignment, invocation)
}

#[tokio::test]
async fn membership_changes_preserve_supported_knowledge_and_original_locations() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("project");
    std::fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let store = Store::open(&temporary.path().join("state")).unwrap();
    let config = Config {
        providers: vec![ProviderConfig {
            id: "mock".into(),
            kind: ProviderKind::Mock,
            command: "internal".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        }],
        agents: ["one", "two", "three"]
            .into_iter()
            .map(|id| AgentProfile {
                id: id.into(),
                name: format!("Captured {id}"),
                provider: "mock".into(),
                model: None,
                instructions: "[mock:split-writers][mock:usage]".into(),
                enabled: true,
            })
            .collect(),
        team: vec!["one".into(), "two".into(), "three".into()],
        team_constraints: TeamConstraints {
            fixed_size: Some(2),
            ..Default::default()
        },
        limits: Limits {
            parallel: 2,
            turns: 80,
            turn_timeout_secs: 10,
            attempts: 2,
            resources: Some(ResourceLimits::default()),
        },
        ..Default::default()
    };
    let backend = Arc::new(CapturingBackend {
        requests: Mutex::new(vec![]),
        fail_planning: AtomicBool::new(false),
    });
    let (tx, _) = mpsc::unbounded_channel();
    let mut engine = Engine::new(store.clone(), config, tx, CancellationToken::new())
        .unwrap()
        .with_execution_backend(backend.clone())
        .unwrap();
    engine.acceptance_contracts.push(AcceptanceContract {
        task_title: "Create a greeting".into(),
        criteria: vec![AcceptanceCriterion {
            id: "exact-greeting".into(),
            description: "Greeting contains exactly the requested bytes".into(),
        }],
        artifacts: vec!["greeting.txt".into()],
        inputs: vec![],
        checks: vec![TrustedCheck {
            id: "greeting-bytes".into(),
            criterion_ids: vec!["exact-greeting".into()],
            assertion: CheckAssertion::ExactBytes {
                artifact: "greeting.txt".into(),
                expected: b"Hello from ymp\n".to_vec(),
            },
        }],
    });
    let source = engine
        .run(&directory, "Create a greeting", None)
        .await
        .unwrap();
    assert_eq!(source.session.status, "blocked");
    assert!(
        source.summary.contains("Scripted later sibling failure"),
        "{}",
        source.summary
    );
    let before = store.trace(&source.session.id).unwrap();
    assert!(before.assignments.iter().all(|a| a.purpose != "learn"));
    let accepted = before
        .decisions
        .iter()
        .find(|d| d.kind == "task_accepted")
        .unwrap();
    assert_eq!(
        accepted.outcome,
        Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Confirmed
        })
    );
    let producer = before
        .assignments
        .iter()
        .find(|a| {
            accepted
                .links
                .result
                .as_ref()
                .unwrap()
                .producer_assignment_ids
                .contains(&a.id)
        })
        .unwrap()
        .agent_id
        .clone();
    let reserved = before
        .team_state
        .as_ref()
        .unwrap()
        .reserved_final_reviewer
        .clone()
        .unwrap();
    assert_ne!(producer, reserved);
    let incoming = engine
        .config
        .agents
        .iter()
        .find(|a| {
            !before
                .team_state
                .as_ref()
                .unwrap()
                .current_members
                .contains(&a.id)
        })
        .unwrap()
        .id
        .clone();
    let members = vec![incoming.clone(), reserved.clone()];
    let replacement = engine
        .clone()
        .with_allocation_policy(Arc::new(ReplaceMembers(members.clone())))
        .unwrap();
    // The producer still owns its failed second task. The combined runtime
    // must retain that responsibility until real recovery inspection completes.
    let denied = replacement
        .reconsider_allocation(
            &source.session.id,
            AllocationBoundary::ResultAvailable,
            idle(),
        )
        .unwrap_err();
    assert!(denied.to_string().contains("active_responsibility"));
    assert_eq!(
        store.team_state(&source.session.id).unwrap(),
        before.team_state
    );
    assert_eq!(
        store.session_usage(&source.session.id).unwrap(),
        before.usage
    );
    let recovered = engine
        .run(
            &directory,
            "Inspect interrupted work",
            Some(&source.session.id),
        )
        .await
        .unwrap();
    assert_eq!(
        recovered.session.status, "completed",
        "{}",
        recovered.summary
    );
    let before = store.trace(&source.session.id).unwrap();
    assert_eq!(
        before
            .assignments
            .iter()
            .filter(|a| a.purpose == "execute")
            .count(),
        2,
        "Recovery must inspect uncertain work without replaying production"
    );
    assert!(before
        .tasks
        .iter()
        .any(|task| task.interrupted && task.state == TaskState::Accepted));
    replacement
        .reconsider_allocation(
            &source.session.id,
            AllocationBoundary::ResultAvailable,
            idle(),
        )
        .unwrap();
    let after = store.trace(&source.session.id).unwrap();
    assert_eq!(after.team_state.as_ref().unwrap().current_members, members);
    assert_eq!(after.session.team.len(), 3);
    assert_eq!(
        serde_json::to_value(&after.policy).unwrap(),
        serde_json::to_value(&before.policy).unwrap()
    );
    assert_eq!(after.usage.total.calls, before.usage.total.calls);
    let history = SessionAgentView::from_captured(&after.session, Some(&members), None).unwrap();
    assert_eq!(
        history.captured_name(&producer),
        Some(format!("Captured {producer}").as_str())
    );
    assert!(!history
        .current_members
        .as_ref()
        .unwrap()
        .contains(&producer));
    let outcome = store.outcomes(&source.session.id).unwrap();
    let confirmed = outcome
        .iter()
        .find(|item| item.acceptance_id == accepted.id)
        .unwrap();
    assert_eq!(confirmed.artifacts[0].path, directory.join("greeting.txt"));
    assert_eq!(confirmed.confirmation, ConfirmationStatus::Confirmed);
    let memory = store
        .memory(Some(&source.session.project_id), "greeting")
        .unwrap()
        .into_iter()
        .find(|m| m.kind == "outcome")
        .unwrap();
    assert_eq!(memory.author, producer);
    assert_eq!(
        memory
            .provenance
            .as_ref()
            .unwrap()
            .source
            .as_ref()
            .unwrap()
            .acceptance_id,
        accepted.id
    );
    assert_eq!(store.observations().unwrap().len(), 1);
    let observation = store.observations().unwrap().remove(0);
    assert_eq!(
        store
            .reputation(&observation.agent_version, "implementation", "simple")
            .unwrap()
            .successes,
        1
    );
    let candidates = store
        .search_memory(
            Some(&source.session.project_id),
            "lesson",
            &Default::default(),
            KnowledgeRetrievalMode::IncludeUnconfirmed,
        )
        .unwrap()
        .into_iter()
        .filter(|m| m.status == "proposed")
        .collect::<Vec<_>>();
    assert!(!candidates.is_empty());
    let supported = store
        .memory(Some(&source.session.project_id), "lesson")
        .unwrap();
    assert!(supported
        .iter()
        .all(|m| !candidates.iter().any(|candidate| candidate.id == m.id)));

    let (tx, _) = mpsc::unbounded_channel();
    let server = ymp_runtime::mcp::TeamServer::start(store.clone(), &after.session, tx)
        .await
        .unwrap();
    for reserved_ordinal in [false, true] {
        for denied in [&producer, &reserved] {
            let (mut assignment, mut invocation) = admission(&store, &source.session.id, denied);
            let snapshot = serde_json::to_value(store.trace(&source.session.id).unwrap()).unwrap();
            let result = if reserved_ordinal {
                server.admit_reserved(
                    &mut assignment,
                    &mut invocation,
                    TeamOperation::coordination(),
                )
            } else {
                server.admit(&mut assignment, &invocation, TeamOperation::coordination())
            };
            assert!(
                result.is_err(),
                "Retired or reserved producer acquired a grant"
            );
            assert!(assignment.grant_ids.is_empty());
            assert_eq!(
                serde_json::to_value(store.trace(&source.session.id).unwrap()).unwrap(),
                snapshot
            );
        }
        let (mut assignment, mut invocation) = admission(&store, &source.session.id, &incoming);
        let result = if reserved_ordinal {
            server.admit_reserved(
                &mut assignment,
                &mut invocation,
                TeamOperation::coordination(),
            )
        } else {
            server.admit(&mut assignment, &invocation, TeamOperation::coordination())
        };
        assert!(
            result.is_ok(),
            "Ordinary current executor must remain admissible"
        );
        server
            .finish(
                &invocation.id,
                InvocationState::Failed,
                Some("Synthetic control; no result produced"),
            )
            .unwrap();
    }
    drop(server);
    assert_eq!(
        store.session_usage(&source.session.id).unwrap().total.calls,
        before.usage.total.calls + 2
    );
    assert_eq!(
        store.observations().unwrap().len(),
        1,
        "Admission/failure must not create unsupported credit"
    );
    let reopened = Store::open(&store.home).unwrap();
    assert!(reopened
        .memory(Some(&source.session.project_id), "greeting")
        .unwrap()
        .iter()
        .any(|m| m.id == memory.id));

    backend.fail_planning.store(true, Ordering::SeqCst);
    let next = engine
        .run(&directory, "Find greeting content", None)
        .await
        .unwrap();
    assert_ne!(next.session.id, source.session.id);
    let next_trace = store.trace(&next.session.id).unwrap();
    assert!(next_trace
        .history
        .iter()
        .any(|event| event.kind == "memory_retrieval"
            && event.data["entries"]
                .as_array()
                .is_some_and(
                    |entries| entries.iter().any(|entry| entry["id"] == memory.id
                        && entry["source_session"] == source.session.id
                        && entry["version"]
                            == content_digest(&serde_json::to_string(&memory).unwrap()))
                )));
    assert!(
        backend
            .requests
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .prompt
            .contains("Hello from ymp"),
        "Actual later-session prompt must receive supported knowledge"
    );

    let relocated = temporary.path().join("relocated-empty");
    std::fs::create_dir(&relocated).unwrap();
    store
        .relocate_project(&source.session.project_id, &relocated)
        .unwrap();
    assert_eq!(store.outcomes(&source.session.id).unwrap(), outcome);
    let invocations_before = store.trace(&source.session.id).unwrap().invocations.len();
    let requests_before = backend.requests.lock().unwrap().len();
    let answer = engine
        .follow_up(&relocated, "Where did you save it?", &source.session.id)
        .await
        .unwrap();
    assert!(
        answer
            .summary
            .contains(&directory.join("greeting.txt").display().to_string()),
        "Location answer replaced the original path: {}",
        answer.summary
    );
    assert!(!answer
        .summary
        .contains(&relocated.join("greeting.txt").display().to_string()));
    assert_eq!(
        answer.workspace, directory,
        "The structured outcome must preserve the captured output directory"
    );
    assert_eq!(
        store.trace(&source.session.id).unwrap().invocations.len(),
        invocations_before
    );
    assert_eq!(backend.requests.lock().unwrap().len(), requests_before);
    assert_eq!(
        std::fs::read(directory.join("greeting.txt")).unwrap(),
        b"Hello from ymp\n"
    );
    assert!(!relocated.join("greeting.txt").exists());
    let final_observations = store.observations().unwrap();
    assert_eq!(final_observations.len(), 1);
    assert_eq!(final_observations[0].id, observation.id);

    // The later session failed before producing anything. Its location fallback
    // must still use captured metadata after registration moves elsewhere.
    assert!(store.outcomes(&next.session.id).unwrap().is_empty());
    let fallback = engine
        .follow_up(&relocated, "Where did you save it?", &next.session.id)
        .await
        .unwrap();
    assert_eq!(fallback.workspace, directory);
    assert!(fallback.summary.contains("Recorded working directory:"));
    assert!(fallback.summary.contains(&directory.display().to_string()));
    assert!(!fallback.summary.contains(&relocated.display().to_string()));

    std::fs::remove_file(
        store
            .session_dir(&next.session)
            .join("workspace/workspace.json"),
    )
    .unwrap();
    let policy_only = engine
        .follow_up(&relocated, "Where did you save it?", &next.session.id)
        .await
        .unwrap();
    assert_eq!(policy_only.workspace, directory);
    assert!(policy_only
        .summary
        .contains("Stored workspace is unavailable"));
    assert_eq!(
        store.trace(&next.session.id).unwrap().invocations.len(),
        next_trace.invocations.len()
    );

    // Legacy sessions without a policy, workspace or outcome cannot establish
    // an output location from the project's mutable current registration.
    let mut legacy = next.session.clone();
    legacy.id = new_id();
    legacy.turns_used = 0;
    store.save_session(&legacy).unwrap();
    let unknown = engine
        .follow_up(&relocated, "Where did you save it?", &legacy.id)
        .await
        .unwrap_err();
    assert!(unknown.to_string().contains("outcome_location_unknown"));
    assert!(store.trace(&legacy.id).unwrap().invocations.is_empty());
    assert_eq!(backend.requests.lock().unwrap().len(), requests_before);
}

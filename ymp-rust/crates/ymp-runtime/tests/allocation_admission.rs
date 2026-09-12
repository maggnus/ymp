use anyhow::{bail, Result};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};
use ymp_runtime::*;
use ymp_storage::Store;

struct Script {
    requests: Mutex<Vec<TurnRequest>>,
}
impl ExecutionBackend for Script {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "independent.script".into(),
            version: "1".into(),
        }
    }
    fn execute(
        &self,
        req: TurnRequest,
        _: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            self.requests.lock().unwrap().push(req.clone());
            let text=match req.purpose.as_str(){
 "plan"=>serde_json::json!({"summary":"Bounded fact","tasks":[{"title":"Fact","description":"Explain a fact","competence":"implementation","difficulty":"standard","dependencies":[],"checks":[]}]}).to_string(),
 "review_plan"|"review"|"final_review"=>serde_json::json!({"approved":true,"reason":"Meets the qualitative criterion"}).to_string(),
 "execute"=>"The verified test fact".into(),
 "synthesis"=>"The checked result is ready".into(),
 "conversation"=>if req.prompt.contains("new unrelated task"){serde_json::json!({"action":"task","task":"Create another fact"}).to_string()}else{serde_json::json!({"action":"steer","answer":"Clarification retained"}).to_string()},
 p=>bail!("Unexpected purpose {p}")
 };
            Ok(TurnResult {
                text,
                session_id: new_id(),
                usage: None,
            })
        })
    }
}
struct Fixture {
    _dir: tempfile::TempDir,
    path: PathBuf,
    store: Store,
    engine: Engine,
    script: Arc<Script>,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("work");
    std::fs::create_dir(&path).unwrap();
    let store = Store::open(&dir.path().join("state")).unwrap();
    let config = Config {
        providers: vec![ProviderConfig {
            id: "script".into(),
            kind: ProviderKind::Mock,
            command: "internal".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        }],
        agents: ["one", "two"]
            .iter()
            .map(|id| AgentProfile {
                id: id.to_string(),
                name: format!("Captured {id}"),
                provider: "script".into(),
                model: None,
                instructions: "Bounded scripted work".into(),
                enabled: true,
            })
            .collect(),
        team: vec!["one".into(), "two".into()],
        limits: Limits {
            parallel: 2,
            turns: 80,
            turn_timeout_secs: 10,
            attempts: 2,
            resources: Some(ResourceLimits::default()),
        },
        ..Default::default()
    };
    let (tx, _rx) = mpsc::unbounded_channel();
    let script = Arc::new(Script {
        requests: Mutex::new(vec![]),
    });
    let mut engine = Engine::new(store.clone(), config, tx, CancellationToken::new())
        .unwrap()
        .with_execution_backend(script.clone())
        .unwrap();
    engine.use_memory = false;
    Fixture {
        _dir: dir,
        path,
        store,
        engine,
        script,
    }
}
fn task_admission(f: &Fixture, session: &str, agent: &str) -> (AssignmentRecord, InvocationRecord) {
    let trace = f.store.trace(session).unwrap();
    let mut task = trace.tasks[0].clone();
    task.id = new_id();
    task.state = TaskState::Ready;
    task.assignee = None;
    task.reviewer = None;
    task.attempts = 0;
    task.result = None;
    task.assign(agent, &HashSet::new()).unwrap();
    f.store.save_task(&task).unwrap();
    let mut a = trace
        .assignments
        .iter()
        .find(|a| a.purpose == "execute")
        .unwrap()
        .clone();
    a.id = new_id();
    a.task = Some(TaskAttemptRef::from(&task));
    a.agent_id = agent.into();
    a.state = InvocationState::Running;
    a.ended_at = None;
    a.grant_ids.clear();
    let mut i = trace.invocations[0].clone();
    i.id = new_id();
    i.assignment_id = a.id.clone();
    i.requested = a.requested.clone();
    i.turn = trace.usage.total.calls + 1;
    i.state = InvocationState::Running;
    i.ended_at = None;
    i.usage = None;
    i.terminal_reason = None;
    (a, i)
}
#[tokio::test]
async fn public_consumer_preserves_conversation_and_separates_distinct_task() {
    let f = fixture();
    let first = f.engine.run(&f.path, "Create a fact", None).await.unwrap();
    assert_eq!(first.session.status, "completed", "{}", first.summary);
    let before = f.store.trace(&first.session.id).unwrap();
    assert!(before.assignments.iter().all(|a| a.purpose != "bid"));
    assert!(before
        .assignments
        .iter()
        .filter(|a| a.purpose == "execute")
        .all(|a| Some(&a.agent_id)
            != before
                .team_state
                .as_ref()
                .unwrap()
                .reserved_final_reviewer
                .as_ref()));
    let steering = f
        .engine
        .follow_up(&f.path, "Clarify the wording", &first.session.id)
        .await
        .unwrap();
    assert_eq!(first.session.id, steering.session.id);
    assert_eq!(steering.session.turns_used, first.session.turns_used + 1);
    let second = f
        .engine
        .follow_up(&f.path, "new unrelated task", &first.session.id)
        .await
        .unwrap();
    assert_ne!(second.session.id, first.session.id);
    assert_eq!(f.store.sessions(None).unwrap().len(), 2);
    assert_eq!(
        f.store
            .session_policy(&second.session.id)
            .unwrap()
            .unwrap()
            .parent_session_id,
        Some(first.session.id.clone())
    );
    assert_eq!(
        f.store
            .session_policy(&first.session.id)
            .unwrap()
            .unwrap()
            .goal,
        "Create a fact"
    );
}
#[tokio::test]
async fn public_admission_must_not_consume_reserved_final_reviewer() {
    let f = fixture();
    let outcome = f.engine.run(&f.path, "Create a fact", None).await.unwrap();
    let state = f.store.team_state(&outcome.session.id).unwrap().unwrap();
    let reserved = state.reserved_final_reviewer.unwrap();
    let (tx, _) = mpsc::unbounded_channel();
    let server = ymp_runtime::mcp::TeamServer::start(f.store.clone(), &outcome.session, tx)
        .await
        .unwrap();
    let (mut a, i) = task_admission(&f, &outcome.session.id, &reserved);
    let admitted = server
        .admit(&mut a, &i, TeamOperation::coordination())
        .is_ok();
    if admitted {
        server
            .finish(
                &i.id,
                InvocationState::Completed,
                Some("synthetic probe completed"),
            )
            .unwrap();
    }
    assert!(
        !admitted,
        "PUBLIC ADMISSION CONSUMED RESERVED REVIEWER: agent={reserved}; grants={}; members={:?}",
        a.grant_ids.len(),
        state.current_members
    );
}
#[tokio::test]
async fn public_admission_control_allows_ordinary_current_executor() {
    for reserved_ordinal in [false, true] {
        let f = fixture();
        let outcome = f.engine.run(&f.path, "Create a fact", None).await.unwrap();
        let state = f.store.team_state(&outcome.session.id).unwrap().unwrap();
        let executor = state
            .current_members
            .iter()
            .find(|id| Some(*id) != state.reserved_final_reviewer.as_ref())
            .unwrap();
        let (tx, _) = mpsc::unbounded_channel();
        let server = ymp_runtime::mcp::TeamServer::start(f.store.clone(), &outcome.session, tx)
            .await
            .unwrap();
        let (mut a, mut i) = task_admission(&f, &outcome.session.id, executor);
        let admitted = if reserved_ordinal {
            server
                .admit_reserved(&mut a, &mut i, TeamOperation::coordination())
                .is_ok()
        } else {
            server
                .admit(&mut a, &i, TeamOperation::coordination())
                .is_ok()
        };
        assert!(admitted);
        assert_eq!(a.grant_ids.len(), 1);
        server
            .finish(
                &i.id,
                InvocationState::Completed,
                Some("synthetic probe completed"),
            )
            .unwrap();
    }
}

#[tokio::test]
async fn invalid_pinned_reviewer_must_be_rejected_before_startup() {
    let mut f = fixture();
    f.engine.config.team_constraints.fixed_roster = Some(vec!["one".into(), "two".into()]);
    f.engine.config.capabilities.insert(
        "script".into(),
        ProviderCapabilities {
            models_complete: true,
            default_model: Some("model-a".into()),
            models: vec![ModelCapabilities {
                id: "model-a".into(),
                controls: Some(vec![NativeControl {
                    id: "effort".into(),
                    values: NativeControlValues::Choices {
                        options: vec!["brief".into()],
                    },
                    default: None,
                }]),
            }],
            ..Default::default()
        },
    );
    f.engine.config.execution.insert(
        "two".into(),
        AgentExecutionPolicy {
            fixed: ModelEffort {
                model: Some("model-a".into()),
                effort: Some("unsupported".into()),
            },
            ..Default::default()
        },
    );
    let result = f.engine.run(&f.path, "Create a fact", None).await;
    let requests = f.script.requests.lock().unwrap();
    let purposes = requests
        .iter()
        .map(|r| r.purpose.as_str())
        .collect::<Vec<_>>();
    let state = result
        .as_ref()
        .map(|o| format!("{}: {}", o.session.status, o.summary))
        .unwrap_or_else(|e| e.to_string());
    assert!(
        requests.is_empty(),
        "INFEASIBLE PINNED REVIEWER SPENT STARTUP: purposes={purposes:?}; outcome={state}"
    );
    assert!(
        state.contains("unsupported_effort")
            && state.contains("two")
            && state.contains("model-a")
            && state.contains("unsupported"),
        "{state}"
    );
    assert!(
        f.store.sessions(None).unwrap().is_empty(),
        "A contradictory startup must not create an admitted session"
    );
}

struct FixedMembers(Vec<String>);
impl AllocationPolicy for FixedMembers {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "independent.membership".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, input: &AllocationInput) -> Result<AllocationProposal> {
        let mut p = BoundedAllocationPolicy.propose(input)?;
        p.members = self.0.clone();
        p.rationale = "Independent idle membership probe".into();
        Ok(p)
    }
}
fn idle() -> AllocationDemand {
    AllocationDemand {
        purpose: "execute".into(),
        task_id: None,
        competence: "implementation".into(),
        difficulty: "standard".into(),
        risk: TaskRisk::Standard,
        ready_work: 0,
    }
}
#[tokio::test]
async fn replacement_denies_retired_id_and_contradictions_preserve_capture() {
    let mut f = fixture();
    let mut third = f.engine.config.agents[0].clone();
    third.id = "three".into();
    third.name = "Captured three".into();
    f.engine.config.agents.push(third);
    f.engine.config.team.push("three".into());
    f.engine.config.team_constraints.fixed_size = Some(2);
    let out = f.engine.run(&f.path, "Create a fact", None).await.unwrap();
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    let s = &out.session.id;
    let before = f.store.trace(s).unwrap();
    let mut ids = before.team_state.as_ref().unwrap().current_members.clone();
    let retired = ids.pop().unwrap();
    let incoming = ["one", "two", "three"]
        .iter()
        .find(|id| {
            !before
                .team_state
                .as_ref()
                .unwrap()
                .current_members
                .contains(&id.to_string())
        })
        .unwrap()
        .to_string();
    ids.push(incoming);
    let engine = f
        .engine
        .clone()
        .with_allocation_policy(Arc::new(FixedMembers(ids.clone())))
        .unwrap();
    engine
        .reconsider_allocation(s, AllocationBoundary::ResultAvailable, idle())
        .unwrap();
    let after = f.store.trace(s).unwrap();
    assert_eq!(after.team_state.as_ref().unwrap().current_members, ids);
    assert_eq!(after.session.team.len(), 3);
    assert_eq!(
        serde_json::to_value(&before.policy).unwrap(),
        serde_json::to_value(&after.policy).unwrap()
    );
    assert_eq!(before.usage.total.calls, after.usage.total.calls);
    let (tx, _) = mpsc::unbounded_channel();
    let server = ymp_runtime::mcp::TeamServer::start(f.store.clone(), &after.session, tx)
        .await
        .unwrap();
    let (mut a, mut i) = task_admission(&f, s, &retired);
    a.purpose = "conversation".into();
    a.task = None;
    let denial = server.admit_reserved(&mut a, &mut i, TeamOperation::coordination());
    assert!(denial.is_err());
    assert!(denial
        .err()
        .unwrap()
        .to_string()
        .contains("ineligible_member"));
    assert!(a.grant_ids.is_empty());
    let bad = engine
        .with_allocation_policy(Arc::new(FixedMembers(vec![
            "one".into(),
            "two".into(),
            "three".into(),
        ])))
        .unwrap();
    assert!(bad
        .reconsider_allocation(s, AllocationBoundary::WorkReady, idle())
        .unwrap_err()
        .to_string()
        .contains("fixed_size"));
    assert_eq!(f.store.team_state(s).unwrap().unwrap().current_members, ids);
    assert_eq!(
        f.store.session_usage(s).unwrap().total.calls,
        before.usage.total.calls
    );
}
#[tokio::test]
async fn public_reserved_admission_must_not_consume_reserved_final_reviewer() {
    let f = fixture();
    let outcome = f.engine.run(&f.path, "Create a fact", None).await.unwrap();
    let state = f.store.team_state(&outcome.session.id).unwrap().unwrap();
    let reserved = state.reserved_final_reviewer.unwrap();
    let (tx, _) = mpsc::unbounded_channel();
    let server = ymp_runtime::mcp::TeamServer::start(f.store.clone(), &outcome.session, tx)
        .await
        .unwrap();
    let (mut a, mut i) = task_admission(&f, &outcome.session.id, &reserved);
    let admitted = server
        .admit_reserved(&mut a, &mut i, TeamOperation::coordination())
        .is_ok();
    if admitted {
        server
            .finish(
                &i.id,
                InvocationState::Completed,
                Some("synthetic probe completed"),
            )
            .unwrap();
    }
    assert!(
        !admitted,
        "RESERVED-ORDINAL ADMISSION CONSUMED RESERVED REVIEWER: agent={reserved}; grants={}",
        a.grant_ids.len()
    );
}
#[tokio::test]
async fn invalid_pinned_reviewer_control_supported_effort_completes() {
    let mut f = fixture();
    f.engine.config.team_constraints.fixed_roster = Some(vec!["one".into(), "two".into()]);
    f.engine.config.capabilities.insert(
        "script".into(),
        ProviderCapabilities {
            models_complete: true,
            default_model: Some("model-a".into()),
            models: vec![ModelCapabilities {
                id: "model-a".into(),
                controls: Some(vec![NativeControl {
                    id: "effort".into(),
                    values: NativeControlValues::Choices {
                        options: vec!["brief".into()],
                    },
                    default: None,
                }]),
            }],
            ..Default::default()
        },
    );
    f.engine.config.execution.insert(
        "two".into(),
        AgentExecutionPolicy {
            fixed: ModelEffort {
                model: Some("model-a".into()),
                effort: Some("brief".into()),
            },
            ..Default::default()
        },
    );
    let out = f.engine.run(&f.path, "Create a fact", None).await.unwrap();
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    assert!(f
        .script
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.profile.id == "two")
        .all(|r| r.settings.effort.as_deref() == Some("brief")));
}

struct ReserveReviewer {
    reviewer: String,
    captured: Option<Arc<std::sync::Barrier>>,
    commit: Option<Arc<std::sync::Barrier>>,
}
impl AllocationPolicy for ReserveReviewer {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "independent.reservation".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, input: &AllocationInput) -> Result<AllocationProposal> {
        let mut proposal = BoundedAllocationPolicy.propose(input)?;
        proposal.reserved_final_reviewer = Some(self.reviewer.clone());
        if let Some(barrier) = &self.captured {
            barrier.wait();
        }
        if let Some(barrier) = &self.commit {
            barrier.wait();
        }
        Ok(proposal)
    }
}
fn three_member_fixture() -> Fixture {
    let mut f = fixture();
    let mut third = f.engine.config.agents[0].clone();
    third.id = "three".into();
    f.engine.config.agents.push(third);
    f.engine.config.team.push("three".into());
    f.engine.config.team_constraints.fixed_roster = Some(f.engine.config.team.clone());
    f
}

#[tokio::test]
async fn reservation_commit_rechecks_production_admitted_after_its_snapshot() {
    for reserved_ordinal in [false, true] {
        for close_before_commit in [false, true] {
            let f = three_member_fixture();
            let out = f.engine.run(&f.path, "Create a fact", None).await.unwrap();
            assert_eq!(out.session.status, "completed");
            let captured = Arc::new(std::sync::Barrier::new(2));
            let commit = Arc::new(std::sync::Barrier::new(2));
            let engine = f
                .engine
                .clone()
                .with_allocation_policy(Arc::new(ReserveReviewer {
                    reviewer: "two".into(),
                    captured: Some(captured.clone()),
                    commit: Some(commit.clone()),
                }))
                .unwrap();
            let session = out.session.id.clone();
            let proposal = std::thread::spawn(move || {
                engine.reconsider_allocation(&session, AllocationBoundary::WorkReady, idle())
            });
            captured.wait();
            let (tx, _) = mpsc::unbounded_channel();
            let server = ymp_runtime::mcp::TeamServer::start(f.store.clone(), &out.session, tx)
                .await
                .unwrap();
            let (mut assignment, mut invocation) = task_admission(&f, &out.session.id, "two");
            let admitted = if reserved_ordinal {
                server
                    .admit_reserved(
                        &mut assignment,
                        &mut invocation,
                        TeamOperation::coordination(),
                    )
                    .is_ok()
            } else {
                server
                    .admit(&mut assignment, &invocation, TeamOperation::coordination())
                    .is_ok()
            };
            assert!(
                admitted,
                "The unreserved current executor must retain valid admission"
            );
            if close_before_commit {
                server
                    .finish(
                        &invocation.id,
                        InvocationState::Completed,
                        Some("scripted production ended"),
                    )
                    .unwrap();
            }
            let before = f.store.trace(&out.session.id).unwrap();
            commit.wait();
            let result = proposal.join().unwrap();
            if !close_before_commit {
                server
                    .finish(
                        &invocation.id,
                        InvocationState::Completed,
                        Some("scripted production ended"),
                    )
                    .unwrap();
            }
            assert!(
                result.is_err(),
                "Reservation installed an agent that became a producer after the proposal snapshot"
            );
            assert!(result
                .err()
                .unwrap()
                .to_string()
                .contains("no_independent_eligible_reviewer"));
            let after = f.store.trace(&out.session.id).unwrap();
            assert_eq!(after.team_state, before.team_state);
            assert_eq!(after.invocations.len(), before.invocations.len());
            assert_eq!(after.usage.total.calls, before.usage.total.calls);
            assert_eq!(assignment.grant_ids.len(), 1);
        }
    }
}

#[tokio::test]
async fn admission_uses_reservation_committed_after_assignment_records_were_prepared() {
    for reserved_ordinal in [false, true] {
        let f = three_member_fixture();
        let out = f.engine.run(&f.path, "Create a fact", None).await.unwrap();
        let (mut assignment, mut invocation) = task_admission(&f, &out.session.id, "two");
        let engine = f
            .engine
            .clone()
            .with_allocation_policy(Arc::new(ReserveReviewer {
                reviewer: "two".into(),
                captured: None,
                commit: None,
            }))
            .unwrap();
        engine
            .reconsider_allocation(&out.session.id, AllocationBoundary::WorkReady, idle())
            .unwrap();
        let before = f.store.trace(&out.session.id).unwrap();
        let (tx, _) = mpsc::unbounded_channel();
        let server = ymp_runtime::mcp::TeamServer::start(f.store.clone(), &out.session, tx)
            .await
            .unwrap();
        let result = if reserved_ordinal {
            server
                .admit_reserved(
                    &mut assignment,
                    &mut invocation,
                    TeamOperation::coordination(),
                )
                .map(|_| ())
        } else {
            server
                .admit(&mut assignment, &invocation, TeamOperation::coordination())
                .map(|_| ())
        };
        if result.is_ok() {
            server
                .finish(
                    &invocation.id,
                    InvocationState::Completed,
                    Some("scripted probe ended"),
                )
                .unwrap();
        }
        assert!(
            result.is_err(),
            "Admission ignored the newly committed reviewer reservation"
        );
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("no_independent_eligible_reviewer"));
        let after = f.store.trace(&out.session.id).unwrap();
        assert_eq!(
            serde_json::to_value(after).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert!(assignment.grant_ids.is_empty());
    }
}

fn catalog(controls: Option<Vec<NativeControl>>, complete: bool) -> ProviderCapabilities {
    ProviderCapabilities {
        models_complete: complete,
        default_model: Some("model-a".into()),
        models: vec![ModelCapabilities {
            id: "model-a".into(),
            controls,
        }],
        ..Default::default()
    }
}
fn effort_control(value: &str) -> Vec<NativeControl> {
    vec![NativeControl {
        id: "effort".into(),
        values: NativeControlValues::Choices {
            options: vec![value.into()],
        },
        default: None,
    }]
}

#[tokio::test]
async fn unknown_native_metadata_does_not_turn_a_pin_into_a_contradiction() {
    for unknown in ["catalog", "controls", "unlisted-model"] {
        let mut f = fixture();
        f.engine.config.team_constraints.fixed_roster = Some(vec!["one".into(), "two".into()]);
        let model = if unknown == "unlisted-model" {
            "not-listed"
        } else {
            "model-a"
        };
        if unknown != "catalog" {
            f.engine.config.capabilities.insert(
                "script".into(),
                catalog(
                    if unknown == "controls" {
                        None
                    } else {
                        Some(effort_control("brief"))
                    },
                    unknown != "unlisted-model",
                ),
            );
        }
        f.engine.config.execution.insert(
            "two".into(),
            AgentExecutionPolicy {
                fixed: ModelEffort {
                    model: Some(model.into()),
                    effort: Some("unknown-native-effort".into()),
                },
                ..Default::default()
            },
        );
        let out = f.engine.run(&f.path, "Create a fact", None).await.unwrap();
        assert_eq!(
            out.session.status, "completed",
            "{unknown}: {}",
            out.summary
        );
        let requests = f.script.requests.lock().unwrap();
        let reviewer: Vec<_> = requests.iter().filter(|r| r.profile.id == "two").collect();
        assert!(!reviewer.is_empty());
        assert!(reviewer
            .iter()
            .all(|r| r.settings.model.as_deref() == Some(model)
                && r.settings.effort.as_deref() == Some("unknown-native-effort")));
        // Script execution cannot establish a native acknowledgement of these controls.
        assert!(f
            .store
            .trace(&out.session.id)
            .unwrap()
            .invocations
            .iter()
            .all(|i| i.reported.effort.is_none()));
    }
}

#[tokio::test]
async fn an_effort_pin_can_choose_another_supported_model_without_changing_the_pin() {
    let mut f = fixture();
    let mut offerings = catalog(Some(effort_control("brief")), true);
    offerings.models.push(ModelCapabilities {
        id: "model-b".into(),
        controls: Some(effort_control("deep")),
    });
    f.engine
        .config
        .capabilities
        .insert("script".into(), offerings);
    f.engine.config.execution.insert(
        "two".into(),
        AgentExecutionPolicy {
            fixed: ModelEffort {
                model: None,
                effort: Some("deep".into()),
            },
            ..Default::default()
        },
    );
    let out = f.engine.run(&f.path, "Create a fact", None).await.unwrap();
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    let requests = f.script.requests.lock().unwrap();
    let reviewer: Vec<_> = requests.iter().filter(|r| r.profile.id == "two").collect();
    assert!(!reviewer.is_empty());
    assert!(reviewer
        .iter()
        .all(|r| r.settings.model.as_deref() == Some("model-b")
            && r.settings.effort.as_deref() == Some("deep")));
}

#[tokio::test]
async fn known_model_and_effort_conflicts_do_not_create_startup_records() {
    for conflict in ["model", "all-model-efforts", "no-effort-control"] {
        let mut f = fixture();
        f.engine.config.team_constraints.fixed_roster = Some(vec!["one".into(), "two".into()]);
        f.engine.config.capabilities.insert(
            "script".into(),
            catalog(
                Some(if conflict == "no-effort-control" {
                    vec![]
                } else {
                    effort_control("brief")
                }),
                true,
            ),
        );
        let fixed = match conflict {
            "model" => ModelEffort {
                model: Some("absent-model".into()),
                effort: None,
            },
            "all-model-efforts" => ModelEffort {
                model: None,
                effort: Some("unsupported".into()),
            },
            _ => ModelEffort {
                model: Some("model-a".into()),
                effort: Some("unsupported".into()),
            },
        };
        f.engine.config.execution.insert(
            "two".into(),
            AgentExecutionPolicy {
                fixed,
                ..Default::default()
            },
        );
        let result = f.engine.run(&f.path, "Create a fact", None).await;
        assert!(result.is_err());
        let error = result.err().unwrap().to_string();
        assert!(
            error.contains("two")
                && error.contains(if conflict == "model" {
                    "unsupported_model"
                } else {
                    "unsupported_effort"
                }),
            "{error}"
        );
        assert!(f.script.requests.lock().unwrap().is_empty());
        assert!(f.store.sessions(None).unwrap().is_empty());
    }
}

#[tokio::test]
async fn required_review_configuration_is_checked_before_startup() {
    for purpose in ["review_plan", "final_review"] {
        let mut f = fixture();
        f.engine.config.team_constraints.fixed_roster = Some(vec!["one".into(), "two".into()]);
        f.engine.config.capabilities.insert(
            "script".into(),
            catalog(Some(effort_control("brief")), true),
        );
        f.engine
            .set_assignment_settings(vec![AssignmentSettingsRule {
                agent_id: "two".into(),
                purpose: Some(purpose.into()),
                task_id: None,
                settings: ModelEffort {
                    model: Some("model-a".into()),
                    effort: Some("unsupported".into()),
                },
            }])
            .unwrap();
        let result = f.engine.run(&f.path, "Create a fact", None).await;
        assert!(result.is_err());
        let error = format!("{:#}", result.err().unwrap());
        assert!(
            error.contains("two")
                && error.contains("model-a")
                && error.contains("unsupported_effort"),
            "{error}"
        );
        assert!(f.script.requests.lock().unwrap().is_empty());
        assert!(f.store.sessions(None).unwrap().is_empty());
    }
}

//! Public Engine consumer walks. All execution is scripted and offline.
use anyhow::{bail, Result};
use serde_json::json;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, Semaphore};
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{
    ExecutionBackend, ExecutionFuture, NativeExecutionBackend, ProviderEvent, TurnRequest,
    TurnResult,
};
use ymp_runtime::*;
use ymp_storage::Store;

#[derive(Clone, Copy)]
enum Mode {
    Reads,
    Scoped,
    Unbounded,
    Failure,
    TwoFailures,
}
struct Script {
    mode: Mode,
    started: mpsc::UnboundedSender<String>,
    release: HashMap<String, Arc<Semaphore>>,
    requests: Mutex<Vec<TurnRequest>>,
}
fn label(request: &TurnRequest) -> String {
    request
        .prompt
        .rsplit("Your current assignment (execute):\n")
        .next()
        .unwrap()
        .lines()
        .nth(1)
        .unwrap_or("")
        .to_owned()
}
impl Script {
    fn tasks(&self) -> Vec<serde_json::Value> {
        let names: &[&str] = match self.mode {
            Mode::Reads | Mode::Failure => &["A", "B"],
            _ => &["A", "B", "C"],
        };
        let failure = matches!(self.mode, Mode::Failure | Mode::TwoFailures);
        names.iter().map(|name| json!({"title":name,"description":format!("Produce contribution {name}"),"access":if matches!(self.mode,Mode::Reads) || (*name == "B" && !failure) {"read_only"} else {"write"},"competence":if failure {if *name=="A" {"analysis"} else {"synthesis"}} else if matches!(self.mode,Mode::Reads) || (*name == "B" && !failure) {"analysis"} else {"implementation"},"difficulty":"standard","dependencies":[],"checks":[]})).collect()
    }
}
impl ExecutionBackend for Script {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "fixture.access-enforced".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        if request.purpose != "execute" {
            return WorkspaceAccess::ReadAll;
        }
        match self.mode {
            Mode::Reads | Mode::Unbounded => NativeExecutionBackend.workspace_access(request),
            Mode::Scoped => match label(request).as_str() {
                "B" => WorkspaceAccess::Scoped {
                    reads: vec!["inputs/ledger.csv".into()],
                    writes: vec![],
                },
                _ => WorkspaceAccess::Scoped {
                    reads: vec![],
                    writes: vec!["outputs/shared.txt".into()],
                },
            },
            Mode::Failure | Mode::TwoFailures => WorkspaceAccess::Scoped {
                reads: vec![],
                writes: vec![format!("outputs/{}.txt", label(request)).into()],
            },
        }
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.clone());
            let text = match request.purpose.as_str() {
                "plan" => {
                    json!({"summary":"Independent bounded contributions","tasks":self.tasks()})
                        .to_string()
                }
                "review_plan" | "review" | "final_review" => {
                    json!({"approved":true,"reason":"Actual recorded contribution inspected"})
                        .to_string()
                }
                "execute" => {
                    let label = label(&request);
                    self.started.send(label.clone()).unwrap();
                    self.release[&label].acquire().await.unwrap().forget();
                    if matches!(self.mode, Mode::Reads) {
                        // Exercise actual built-in offline native execution after the
                        // barrier; the adapter must honor the read_only request.
                        return NativeExecutionBackend.execute(request, events).await;
                    }
                    let access = self.workspace_access(&request);
                    // This backend has no shell/tools/general file operation route.
                    // Its only I/O is the literal path selected by the same contract.
                    if let WorkspaceAccess::Scoped { reads, writes } = access {
                        for path in reads {
                            let _ = std::fs::read(request.cwd.join(path))?;
                        }
                        for path in writes {
                            std::fs::write(
                                request.cwd.join(path),
                                format!("Contribution {label}"),
                            )?;
                        }
                    }
                    if matches!(self.mode, Mode::Failure | Mode::TwoFailures) && label != "A" {
                        let _ = events.send(ProviderEvent::Usage(UsageSnapshot {
                            counts: TokenCounts {
                                input: Some(7),
                                ..Default::default()
                            },
                            finalized: false,
                            ..Default::default()
                        }));
                        bail!("Scripted sibling infrastructure failure");
                    }
                    format!("Contribution {label}")
                }
                "synthesis" => "Contributions independently accepted".into(),
                purpose => bail!("Unexpected purpose {purpose}"),
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
    _temp: tempfile::TempDir,
    path: PathBuf,
    store: Store,
    engine: Engine,
    script: Arc<Script>,
    started: mpsc::UnboundedReceiver<String>,
}
fn fixture(mode: Mode) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("work");
    std::fs::create_dir_all(path.join("inputs")).unwrap();
    std::fs::create_dir_all(path.join("outputs")).unwrap();
    std::fs::write(path.join("inputs/ledger.csv"), "account,total\nNorth,1\n").unwrap();
    let store = Store::open(&temp.path().join("state")).unwrap();
    let (started, rx) = mpsc::unbounded_channel();
    let script = Arc::new(Script {
        mode,
        started,
        release: ["A", "B", "C"]
            .into_iter()
            .map(|id| (id.into(), Arc::new(Semaphore::new(0))))
            .collect(),
        requests: Mutex::new(vec![]),
    });
    let config = Config {
        providers: vec![ProviderConfig {
            id: "offline".into(),
            kind: ProviderKind::Mock,
            command: "internal".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        }],
        agents: ["one", "two", "three", "reviewer"]
            .into_iter()
            .map(|id| AgentProfile {
                id: id.into(),
                name: id.into(),
                provider: "offline".into(),
                model: None,
                instructions: "Offline bounded fixture".into(),
                enabled: true,
            })
            .collect(),
        team: ["one", "two", "three", "reviewer"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        team_constraints: TeamConstraints {
            fixed_roster: Some(
                ["one", "two", "three", "reviewer"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            ),
            ..Default::default()
        },
        limits: Limits {
            parallel: 3,
            turns: 80,
            turn_timeout_secs: 10,
            attempts: 1,
            resources: Some(ResourceLimits::default()),
        },
        ..Default::default()
    };
    let (events, _) = mpsc::unbounded_channel();
    let mut engine = Engine::new(store.clone(), config, events, CancellationToken::new())
        .unwrap()
        .with_execution_backend(script.clone())
        .unwrap();
    engine.use_memory = false;
    if matches!(mode, Mode::Failure | Mode::TwoFailures) {
        engine.acceptance_contracts.push(AcceptanceContract {
            task_title: "A".into(),
            criteria: vec![AcceptanceCriterion {
                id: "exact".into(),
                description: "The analysis writes its requested file".into(),
            }],
            artifacts: vec!["outputs/A.txt".into()],
            inputs: vec![],
            checks: vec![TrustedCheck {
                id: "exact".into(),
                criterion_ids: vec!["exact".into()],
                assertion: CheckAssertion::ExactBytes {
                    artifact: "outputs/A.txt".into(),
                    expected: b"Contribution A".to_vec(),
                },
            }],
        });
    }
    Fixture {
        _temp: temp,
        path,
        store,
        engine,
        script,
        started: rx,
    }
}
async fn next(f: &mut Fixture) -> String {
    tokio::time::timeout(Duration::from_secs(3), f.started.recv())
        .await
        .expect("Expected a useful assignment to reach its barrier")
        .unwrap()
}
fn start(f: &Fixture) -> tokio::task::JoinHandle<RunOutcome> {
    let engine = f.engine.clone();
    let path = f.path.clone();
    tokio::spawn(async move {
        engine
            .run(&path, "Produce the independent contributions", None)
            .await
            .unwrap()
    })
}
async fn finish(run: tokio::task::JoinHandle<RunOutcome>) -> RunOutcome {
    tokio::time::timeout(Duration::from_secs(5), run)
        .await
        .expect("Engine did not finish after barriers released")
        .unwrap()
}
fn release(f: &Fixture, label: &str) {
    f.script.release[label].add_permits(1);
}
fn assert_closed(f: &Fixture, outcome: &RunOutcome) {
    let trace = f.store.trace(&outcome.session.id).unwrap();
    assert!(trace
        .invocations
        .iter()
        .all(|i| i.state != InvocationState::Running));
    assert_eq!(trace.budget.unwrap().in_flight_invocations, 0);
    let acquired = trace
        .decisions
        .iter()
        .filter(|d| d.kind == "workspace_access_acquired")
        .count();
    let released = trace
        .decisions
        .iter()
        .filter(|d| d.kind == "workspace_access_released")
        .count();
    assert_eq!(
        acquired, released,
        "Resource ownership leaked past terminal accounting"
    );
    for a in trace.assignments {
        for id in a.grant_ids {
            assert!(f
                .store
                .team_grant(&outcome.session.id, &id)
                .unwrap()
                .revoked_at
                .is_some());
        }
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_read_only_contributions_overlap_through_public_engine() {
    let mut f = fixture(Mode::Reads);
    // Ordinary default allocation must discover useful width, without roster or
    // complexity pins. A fourth eligible identity is available but need not work.
    f.engine.config.team_constraints = TeamConstraints::default();
    f.engine.config.team = vec!["one".into(), "two".into()];
    let run = start(&f);
    let a = next(&mut f).await;
    let b = next(&mut f).await;
    assert_ne!(a, b); // Both backend executions are held, so overlap is causal.
    release(&f, &a);
    release(&f, &b);
    let outcome = finish(run).await;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let trace = f.store.trace(&outcome.session.id).unwrap();
    let producers = trace
        .assignments
        .iter()
        .filter(|a| a.purpose == "execute")
        .collect::<Vec<_>>();
    assert_eq!(producers.len(), 2);
    assert!(producers
        .iter()
        .all(|a| a.requested.permission_mode.as_deref() == Some("read_only")));
    assert!(f
        .script
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.purpose == "execute")
        .all(|r| r.read_only));
    assert!(trace.tasks.iter().all(|t| t.state == TaskState::Accepted));
    assert!(trace
        .decisions
        .iter()
        .filter(|d| d.kind == "workspace_access_admitted" && d.links.task.is_some())
        .any(
            |d| d.links.workspace_access.as_ref().unwrap().effective_access
                == WorkspaceAccess::ReadAll
        ));
    assert!(
        !f.path.join("greeting.txt").exists(),
        "Read-only native execution wrote a file"
    );
    assert_eq!(
        std::fs::read_dir(f.path.join("outputs")).unwrap().count(),
        0
    );
    assert_closed(&f, &outcome);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scoped_backend_overlaps_independent_read_but_serializes_conflicting_writers() {
    let mut f = fixture(Mode::Scoped);
    let run = start(&f);
    let one = next(&mut f).await;
    let two = next(&mut f).await;
    assert!(
        one == "B" || two == "B",
        "Only the independent reader may overlap a writer"
    );
    let writer = if one == "B" { two } else { one };
    assert!(
        tokio::time::timeout(Duration::from_millis(100), f.started.recv())
            .await
            .is_err(),
        "Conflicting writer entered while its predecessor was held"
    );
    release(&f, &writer);
    let other = next(&mut f).await;
    assert_ne!(other, writer);
    assert_ne!(other, "B");
    release(&f, &other);
    release(&f, "B");
    let outcome = finish(run).await;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let trace = f.store.trace(&outcome.session.id).unwrap();
    assert!(trace.decisions.iter().any(|d| d
        .links
        .workspace_wait
        .as_ref()
        .is_some_and(|w| w.code == "resource_conflict" && w.holder.is_some())));
    assert!(f.path.join("outputs/shared.txt").exists());
    assert!(!f.path.join(".git").exists());
    assert_closed(&f, &outcome);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_unbounded_access_serializes_writers_and_readers() {
    let mut f = fixture(Mode::Unbounded);
    let run = start(&f);
    for _ in 0..3 {
        let current = next(&mut f).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(100), f.started.recv())
                .await
                .is_err(),
            "Native unrestricted writer overlapped another assignment"
        );
        release(&f, &current);
    }
    let outcome = finish(run).await;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    assert_closed(&f, &outcome);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn failed_sibling_preserves_completed_work_review_usage_and_restart_inspection() {
    let mut f = fixture(Mode::Failure);
    let run = start(&f);
    let a = next(&mut f).await;
    let b = next(&mut f).await;
    assert_ne!(a, b);
    release(&f, "B");
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let sessions = f.store.sessions(None).unwrap();
            if sessions.first().is_some_and(|s| {
                f.store
                    .trace(&s.id)
                    .unwrap()
                    .invocations
                    .iter()
                    .any(|i| i.state == InvocationState::Failed)
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    release(&f, "A");
    let outcome = finish(run).await;
    assert_eq!(outcome.session.status, "blocked");
    let trace = f.store.trace(&outcome.session.id).unwrap();
    assert_eq!(
        trace.tasks.iter().find(|t| t.title == "A").unwrap().state,
        TaskState::Accepted,
        "Successful sibling must be reviewed despite failure"
    );
    assert_eq!(trace.usage.total.counts.input, Some(7));
    assert!(trace.decisions.iter().any(|d| d.kind == "task_accepted"
        && d.outcome
            == Some(DecisionOutcome::Accepted {
                confirmation: ConfirmationStatus::Confirmed
            })));
    assert!(trace.tasks.iter().all(|t| t.access == TaskAccess::Write));
    assert!(trace.tasks.iter().any(|t| t.competence == "analysis"));
    assert!(trace.tasks.iter().any(|t| t.competence == "synthesis"));
    assert!(f
        .script
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.purpose == "execute")
        .all(|r| !r.read_only));
    assert_closed(&f, &outcome);
    let calls = trace
        .assignments
        .iter()
        .filter(|a| a.purpose == "execute")
        .count();
    // A hard process exit may retain an acquired record with no release event.
    let mut historical = trace
        .decisions
        .iter()
        .find_map(|d| d.links.workspace_access.clone())
        .unwrap();
    historical.reservation_id = "interrupted-resource-holder".into();
    f.store
        .record_decision(&DecisionRecord {
            id: new_id(),
            session_id: outcome.session.id.clone(),
            kind: "workspace_access_acquired".into(),
            actor: None,
            reason: "Fixture: resource record persisted before a hard process exit".into(),
            outcome: None,
            links: RecordLinks {
                workspace_access: Some(historical),
                ..Default::default()
            },
            created_at: now(),
        })
        .unwrap();
    let resumed = f
        .engine
        .run(
            &f.path,
            "Inspect interrupted work",
            Some(&outcome.session.id),
        )
        .await
        .unwrap();
    assert_eq!(resumed.session.status, "completed", "{}", resumed.summary);
    let after = f.store.trace(&outcome.session.id).unwrap();
    assert_eq!(
        after
            .assignments
            .iter()
            .filter(|a| a.purpose == "execute")
            .count(),
        calls,
        "Recovery repeated uncertain production"
    );
    assert_eq!(
        after
            .tasks
            .iter()
            .find(|t| t.title == "A")
            .unwrap()
            .attempts,
        1
    );
    assert!(after
        .decisions
        .iter()
        .any(|d| d.reason.contains("interrupted artifacts")));
    assert_closed(&f, &resumed);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_probe_two_failed_siblings_keep_eligible_review_reachable() {
    let mut f = fixture(Mode::TwoFailures);
    f.engine.config.team_constraints = TeamConstraints::default();
    f.engine.config.team = vec!["one".into(), "two".into()];
    f.engine.config.capabilities.insert(
        "offline".into(),
        ProviderCapabilities {
            models_complete: true,
            models: vec![ModelCapabilities {
                id: "available".into(),
                controls: None,
            }],
            default_model: Some("available".into()),
            ..Default::default()
        },
    );
    // B and C can produce, but only the independent fourth identity can review
    // A. Their unsupported review configurations must not be silently changed.
    f.engine
        .set_assignment_settings(
            ["two", "three"]
                .into_iter()
                .map(|id| AssignmentSettingsRule {
                    agent_id: id.into(),
                    purpose: Some("review".into()),
                    task_id: None,
                    settings: ModelEffort {
                        model: Some("unavailable".into()),
                        effort: None,
                    },
                })
                .collect(),
        )
        .unwrap();
    let run = start(&f);
    let mut started = Vec::new();
    for _ in 0..3 {
        started.push(next(&mut f).await);
    }
    started.sort();
    assert_eq!(started, ["A", "B", "C"]);
    release(&f, "B");
    release(&f, "C");
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let session = f.store.sessions(None).unwrap().remove(0);
            let trace = f.store.trace(&session.id).unwrap();
            if trace
                .invocations
                .iter()
                .filter(|i| i.state == InvocationState::Failed)
                .count()
                == 2
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    release(&f, "A");
    let outcome = finish(run).await;
    let trace = f.store.trace(&outcome.session.id).unwrap();
    let completed = trace.tasks.iter().find(|t| t.title == "A").unwrap();
    println!("Outcome: {}: {}", outcome.session.status, outcome.summary);
    assert_eq!(
        completed.state,
        TaskState::Accepted,
        "Completed sibling lost independent review although reviewer is eligible, max_members=4, and budget remains"
    );
    assert!(trace.decisions.iter().any(|d| d.kind == "task_accepted"
        && d.links
            .task
            .as_ref()
            .is_some_and(|task| task.task_id == completed.id)
        && d.outcome
            == Some(DecisionOutcome::Accepted {
                confirmation: ConfirmationStatus::Confirmed
            })));
    assert_eq!(
        std::fs::read(f.path.join("outputs/A.txt")).unwrap(),
        b"Contribution A"
    );
    assert_eq!(outcome.session.status, "blocked");
    assert!(outcome
        .summary
        .contains("Scripted sibling infrastructure failure"));
    let failed = trace
        .invocations
        .iter()
        .filter(|i| i.state == InvocationState::Failed)
        .collect::<Vec<_>>();
    assert_eq!(failed.len(), 2);
    // Invocation records retain the runtime classification, not raw SDK errors.
    assert!(failed
        .iter()
        .all(|i| i.terminal_reason.as_deref() == Some("failed")));
    assert!(trace
        .tasks
        .iter()
        .filter(|t| t.title != "A")
        .all(|t| t.state == TaskState::Running));
    assert_eq!(trace.usage.total.counts.input, Some(14));
    assert_eq!(trace.usage.total.partial_calls, 2);
    let budget = trace.budget.as_ref().unwrap();
    assert!(budget.admitted_invocations < u64::try_from(budget.limits.turns).unwrap());
    assert!(budget.last_denial.is_none());

    let review = f
        .store
        .allocation_decisions(&outcome.session.id)
        .unwrap()
        .into_iter()
        .find(|d| {
            d.accepted
                && d.input.demand.purpose == "review"
                && d.input.demand.task_id.as_ref() == Some(&completed.id)
        })
        .unwrap();
    assert_eq!(review.input.constraints, TeamConstraints::default());
    assert_eq!(review.input.occupied_agent_ids, ["three", "two"]);
    assert_eq!(review.proposal.members.len(), 3);
    assert!(review
        .input
        .occupied_agent_ids
        .iter()
        .all(|id| review.proposal.members.contains(id)));
    assert_eq!(
        review.proposal.executor.as_ref().unwrap().agent_id,
        "reviewer"
    );
    let reviewer = trace
        .assignments
        .iter()
        .find(|a| {
            a.purpose == "review"
                && a.task
                    .as_ref()
                    .is_some_and(|task| task.task_id == completed.id)
        })
        .unwrap();
    assert_eq!(reviewer.agent_id, "reviewer");
    assert_ne!(completed.assignee.as_ref(), Some(&reviewer.agent_id));
    assert!(reviewer.requested.model.is_none()); // Preserve the provider default.

    // An actual smaller ceiling or size pin still forbids this membership.
    // Increasing a heuristic target must not change captured constraints.
    for constraints in [
        TeamConstraints {
            max_members: 2,
            ..Default::default()
        },
        TeamConstraints {
            fixed_size: Some(2),
            ..Default::default()
        },
    ] {
        let mut constrained = review.input.clone();
        constrained.constraints = constraints;
        assert!(BoundedAllocationPolicy
            .propose(&constrained)
            .unwrap_err()
            .to_string()
            .contains("active_responsibility"));
    }
    let mut already_occupied = review.input.clone();
    already_occupied.occupied_agent_ids = vec!["reviewer".into(), "two".into()];
    assert_eq!(
        BoundedAllocationPolicy
            .propose(&already_occupied)
            .unwrap()
            .members,
        already_occupied.occupied_agent_ids,
        "An already retained reviewer must not inflate the membership target"
    );
    assert_closed(&f, &outcome);
}
struct DishonestPolicy;
impl WorkspaceAccessPolicy for DishonestPolicy {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "fixture.dishonest".into(),
            version: "1".into(),
        }
    }
    fn resolve(&self, _: &WorkspaceAccessInput<'_>) -> Result<WorkspaceAccess> {
        Ok(WorkspaceAccess::Scoped {
            reads: vec![],
            writes: vec!["safe.txt".into()],
        })
    }
}
#[tokio::test]
async fn policy_cannot_claim_unsupported_narrow_backend_guarantees() {
    let mut f = fixture(Mode::Unbounded);
    f.engine = f
        .engine
        .with_workspace_access_policy(Arc::new(DishonestPolicy))
        .unwrap();
    let outcome = f
        .engine
        .run(&f.path, "Attempt unsupported protection", None)
        .await
        .unwrap();
    assert_eq!(outcome.session.status, "blocked");
    assert!(f.script.requests.lock().unwrap().is_empty());
    assert_eq!(
        f.store
            .trace(&outcome.session.id)
            .unwrap()
            .usage
            .total
            .calls,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancellation_releases_waiters_without_spend_and_resume_inspects_only_admitted_work() {
    let mut f = fixture(Mode::Unbounded);
    let run = start(&f);
    let _held = next(&mut f).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(100), f.started.recv())
            .await
            .is_err()
    );
    f.engine.cancel.cancel();
    let outcome = finish(run).await;
    assert_eq!(outcome.session.status, "paused");
    let trace = f.store.trace(&outcome.session.id).unwrap();
    let executions = trace
        .assignments
        .iter()
        .filter(|a| a.purpose == "execute")
        .count();
    assert_eq!(
        executions, 1,
        "Waiting selections spent admission resources"
    );
    assert_closed(&f, &outcome);
    f.engine.cancel = CancellationToken::new();
    for label in ["A", "B", "C"] {
        f.script.release[label].add_permits(3);
    }
    let resumed = tokio::time::timeout(
        Duration::from_secs(5),
        f.engine
            .run(&f.path, "Inspect and continue", Some(&outcome.session.id)),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(resumed.session.status, "completed", "{}", resumed.summary);
    let after = f.store.trace(&outcome.session.id).unwrap();
    assert_eq!(
        after
            .assignments
            .iter()
            .filter(|a| a.purpose == "execute")
            .count(),
        3,
        "Uncertain admitted work was replayed, or waiting work was lost"
    );
    assert_closed(&f, &resumed);
}

#[test]
fn legacy_task_access_defaults_to_write_and_is_bound_into_candidate_definition() {
    let definition = json!({"title":"Analysis document","description":"Create a report","competence":"analysis","difficulty":"standard","dependencies":[],"checks":[]});
    let task: PlanTask = serde_json::from_value(definition.clone()).unwrap();
    assert_eq!(task.access, TaskAccess::Write);
    assert_eq!(
        serde_json::to_value(task).unwrap(),
        definition,
        "Legacy plan serialization must retain historical digests"
    );
    assert!(serde_json::from_value::<PlanTask>(
        json!({"access":"guaranteed_narrow","title":"X","description":"Y"})
    )
    .is_err());
}

#[test]
fn reviewed_task_definition_includes_explicit_authority() {
    let mut task:Task=serde_json::from_value(json!({"id":"t","session_id":"s","title":"Report","description":"Analyze inputs","competence":"analysis","difficulty":"standard","dependencies":[],"checks":[],"state":"ready","assignee":null,"reviewer":null,"attempts":0,"result":null,"workspace":null})).unwrap();
    let write = TaskDefinition::from(&task);
    task.access = TaskAccess::ReadOnly;
    let read = TaskDefinition::from(&task);
    assert_ne!(write, read);
    assert_ne!(
        content_digest(&serde_json::to_string(&write).unwrap()),
        content_digest(&serde_json::to_string(&read).unwrap())
    );
}

#[tokio::test]
async fn public_admission_cannot_enlarge_reviewed_read_only_authority() {
    let f = fixture(Mode::Reads);
    for label in ["A", "B"] {
        release(&f, label);
    }
    let outcome = f
        .engine
        .run(&f.path, "Read the contributions", None)
        .await
        .unwrap();
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let trace = f.store.trace(&outcome.session.id).unwrap();
    let mut task = trace.tasks[0].clone();
    task.state = TaskState::Ready;
    let mut assignment = trace
        .assignments
        .iter()
        .find(|a| a.purpose == "execute" && a.task.as_ref().is_some_and(|t| t.task_id == task.id))
        .unwrap()
        .clone();
    task.id = new_id();
    task.attempts = 0;
    task.result = None;
    task.reviewer = None;
    task.assign(&assignment.agent_id, &Default::default())
        .unwrap();
    f.store.save_task(&task).unwrap();
    let mut invocation = trace
        .invocations
        .iter()
        .find(|i| i.assignment_id == assignment.id)
        .unwrap()
        .clone();
    assignment.id = new_id();
    assignment.task = Some(TaskAttemptRef::from(&task));
    assignment.grant_ids.clear();
    assignment.state = InvocationState::Running;
    assignment.ended_at = None;
    assignment.requested.permission_mode = Some("write".into());
    invocation.id = new_id();
    invocation.assignment_id = assignment.id.clone();
    invocation.requested = assignment.requested.clone();
    invocation.state = InvocationState::Running;
    invocation.ended_at = None;
    invocation.usage = None;
    invocation.terminal_reason = None;
    let error = f
        .store
        .admit_invocation(&assignment, invocation)
        .unwrap_err();
    assert!(error.to_string().contains("task_access"), "{error:#}");
    assert_eq!(
        f.store
            .trace(&outcome.session.id)
            .unwrap()
            .usage
            .total
            .calls,
        trace.usage.total.calls
    );
}

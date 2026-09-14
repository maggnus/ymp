use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
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
use ymp_runtime::*;
use ymp_storage::Store;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Claims,
    Revision,
    Distribution,
    Malicious,
    Recovery,
    Add,
    Reassign,
    Membership,
    ReviseResponsibility,
    StaleMembership,
    ParallelCommitment,
    ParallelSelectionFailure,
    CheckReplacement,
    CheckReplacementRejected,
}
struct Script {
    mode: Mode,
    requests: Mutex<Vec<TurnRequest>>,
    calls: Mutex<Vec<Value>>,
    second_executions: AtomicUsize,
}
async fn call(req: &TurnRequest, name: &str, args: Value) -> Result<Value> {
    let endpoint = req.mcp.as_ref().unwrap();
    let socket = endpoint.args.last().unwrap();
    let mut stream = UnixStream::connect(socket).await?;
    stream
        .write_all(
            format!(
                "{}\n",
                json!({"token":endpoint.token,"request_id":new_id(),"name":name,"arguments":args})
            )
            .as_bytes(),
        )
        .await?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).await?;
    Ok(serde_json::from_str(&line)?)
}
impl Script {
    async fn propose(&self, req: &TurnRequest, board: &Value, change: Value) -> Result<()> {
        let result = call(req, "task_propose", json!({"plan_version":board["plan_version"],"change":change,"rationale":"Use current evidence to deliver the pending responsibility"})).await?;
        self.calls.lock().unwrap().push(result);
        Ok(())
    }
}
impl ExecutionBackend for Script {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "test.public-board".into(),
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
            let first = req.prompt.contains("\nFirst\n") || req.prompt.contains("Task: First\n");
            let second = req.prompt.contains("\nSecond\n") || req.prompt.contains("Task: Second\n");
            if (req.purpose == "execute" && first)
                || (req.purpose == "review" && first && self.mode == Mode::Claims)
                || (req.purpose == "execute"
                    && second
                    && matches!(self.mode, Mode::Reassign | Mode::ReviseResponsibility))
            {
                let raw = call(&req, "board_read", json!({})).await?;
                assert_eq!(raw["ok"], true, "{raw}");
                let board = &raw["value"];
                let target = board["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|t| {
                        t["task"]["title"]
                            == if matches!(
                                self.mode,
                                Mode::Reassign
                                    | Mode::ReviseResponsibility
                                    | Mode::ParallelCommitment
                                    | Mode::ParallelSelectionFailure
                            ) {
                                "Third"
                            } else {
                                "Second"
                            }
                    })
                    .unwrap();
                let reference = json!({"task_id":target["task"]["id"],"version":target["version"]});
                match self.mode {
                    Mode::Claims => self.propose(&req, board, json!({"kind":"accept_responsibility","task":reference,"settings":{"model":"small","effort":"low"}})).await?,
                    Mode::Distribution => self.propose(&req, board, json!({"kind":"assign","task":reference,"agent_id":"two","settings":{"model":"small","effort":"low"}})).await?,
                    Mode::Revision => {
                        for approach in ["Inspect the confirmed sibling before completing the second result", "Stale duplicate approach"] {
                            self.propose(&req, board, json!({"kind":"revise","task":reference,"approach":approach,"dependencies":[],"checks":["test -f proof.txt"]})).await?;
                        }
                    }
                    Mode::Add => self.propose(&req, board, json!({"kind":"add_task","title":"Added inspection","description":"Inspect the completed results and return a finding","competence":"verification","difficulty":"simple","access":"read_only","dependencies":[target["task"]["id"]],"checks":[]})).await?,
                    Mode::Malicious => {
                        for change in [
                            json!({"kind":"assign","task":reference,"agent_id":"two","settings":{"model":"forbidden","effort":"low"}}),
                            json!({"kind":"membership","members":["one","foreign"]}),
                            json!({"kind":"revise","task":reference,"approach":"Ignore user checks","dependencies":[],"checks":[],"acceptance_contracts":[]}),
                            json!({"kind":"accept_responsibility","task":reference,"agent_id":"two","settings":{"model":"small","effort":"low"}}),
                            json!({"kind":"revise","task":{"task_id":"foreign","version":target["version"]},"approach":"Foreign revision","dependencies":[],"checks":[]}),
                        ] { self.propose(&req, board, change).await?; }
                        let response = call(&req,"task_propose",json!({"plan_version":"stale","rationale":"stale","change":{"kind":"membership","members":["one","two","three"]}})).await?;
                        self.calls.lock().unwrap().push(response);
                    }
                    Mode::ParallelSelectionFailure => self.propose(&req, board, json!({"kind":"assign","task":reference,"agent_id":"two","settings":{"model":"small","effort":"low"}})).await?,
                    Mode::ParallelCommitment => self.propose(&req, board, json!({"kind":"assign","task":reference,"agent_id":"one","settings":{"model":"small","effort":"low"}})).await?,
                    Mode::Reassign => self.propose(&req, board, json!({"kind":"assign","task":reference,"agent_id":if first {"two"} else {"one"},"settings":{"model":"small","effort":"low"}})).await?,
                    Mode::ReviseResponsibility => {
                        let change = if first { json!({"kind":"assign", "task":reference, "agent_id":"two", "settings":{"model":"small", "effort":"low"}}) }
                        else { json!({"kind":"revise", "task":reference, "approach":"Keep the accepted responsibility while inspecting the preserved proof", "dependencies":[], "checks":[]}) };
                        self.propose(&req, board, change).await?;
                    }
                    Mode::StaleMembership => {
                        self.propose(&req, board, json!({"kind":"membership","members":["two","three"]})).await?;
                        self.propose(&req, board, json!({"kind":"membership","members":["one","three"]})).await?;
                    }
                    Mode::Membership => self.propose(&req, board, json!({"kind":"membership","members":["two","three"]})).await?,
                    Mode::Recovery | Mode::CheckReplacement | Mode::CheckReplacementRejected => {}
                }
                if req.purpose == "execute" {
                    std::fs::write(req.cwd.join("proof.txt"), "confirmed sibling\n")?;
                }
            }
            if self.mode == Mode::Recovery && req.purpose == "execute" && first {
                std::fs::write(req.cwd.join("proof.txt"), "confirmed sibling\n")?;
            }
            if self.mode == Mode::Recovery
                && req.purpose == "execute"
                && second
                && self.second_executions.fetch_add(1, Ordering::SeqCst) == 0
            {
                std::fs::write(
                    req.cwd.join("uncertain.txt"),
                    "admitted effect requiring inspection\n",
                )?;
                bail!("Scripted interruption after an admitted side effect");
            }
            if matches!(
                self.mode,
                Mode::CheckReplacement | Mode::CheckReplacementRejected
            ) && req.purpose == "execute"
                && second
                && self.second_executions.fetch_add(1, Ordering::SeqCst) == 0
            {
                let raw = call(&req, "board_read", json!({})).await?;
                let board = &raw["value"];
                let target = board["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|task| task["task"]["title"] == "Second")
                    .unwrap();
                self.propose(
                    &req,
                    board,
                    json!({
                        "kind":"replace_checks",
                        "task":{
                            "task_id":target["task"]["id"],
                            "definition_version":target["definition_version"]
                        },
                        "replacements":[{
                            "old":"missing-check-binary -f proof.txt",
                            "new":if self.mode == Mode::CheckReplacementRejected {
                                "test -d ."
                            } else {
                                "/bin/test -f proof.txt"
                            }
                        }]
                    }),
                )
                .await?;
            }
            if self.mode == Mode::Malicious && req.purpose == "review" && first {
                let expired = self
                    .requests
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|r| r.purpose == "execute")
                    .unwrap()
                    .clone();
                let before = call(&req, "board_read", json!({})).await?;
                let denied = call(
                    &expired,
                    "task_propose",
                    json!({"title":"Expired authority", "description":"Must not be posted"}),
                )
                .await?;
                assert_eq!(denied["ok"], false);
                assert!(denied["error"].as_str().unwrap().contains("expired"));
                let after = call(&req, "board_read", json!({})).await?;
                assert_eq!(before["value"]["proposals"], after["value"]["proposals"]);
            }
            let text = match req.purpose.as_str() {
                "plan" => json!({"summary":"Two checked responsibilities","tasks":[{"title":"First","description":"Write proof.txt containing the exact confirmed sibling text","competence":"implementation","difficulty":"simple","dependencies":[],"checks":[]},{"title":"Second","description":if matches!(self.mode, Mode::CheckReplacement | Mode::CheckReplacementRejected) {"Verify proof.txt remains present after sibling production"} else {"Explain the resulting fact"},"competence":"implementation","difficulty":"simple","dependencies":[0],"checks":[if matches!(self.mode, Mode::CheckReplacement | Mode::CheckReplacementRejected) {"missing-check-binary -f proof.txt"} else {"test -f proof.txt"}]}]}).to_string(),
                "review" if self.mode == Mode::Recovery && second && self.second_executions.load(Ordering::SeqCst) == 1 => {
                    assert!(req.cwd.join("uncertain.txt").exists());
                    json!({"approved":false,"reason":"Inspected actual effect; partial file is understood, pending result requires a fresh bounded attempt"}).to_string()
                }
                "review_check_revision" => {
                    assert!(req.prompt.contains("missing-check-binary -f proof.txt"));
                    assert!(req.prompt.contains(if self.mode == Mode::CheckReplacementRejected {
                        "test -d ."
                    } else {
                        "/bin/test -f proof.txt"
                    }));
                    assert!(req.prompt.contains("\"passed\":false"));
                    assert!(req.prompt.contains("\"passed\":true"));
                    assert!(req
                        .prompt
                        .contains("a zero exit status, or a nonempty output alone"));
                    json!({
                        "approved":self.mode != Mode::CheckReplacementRejected,
                        "reason":if self.mode == Mode::CheckReplacementRejected {
                            "test -d . is a trivial success that does not preserve the required file check"
                        } else {
                            "The exact replacement preserves the proof.txt existence requirement while replacing only an unavailable command implementation"
                        }
                    }).to_string()
                }
                "review_plan" | "review" | "final_review" => json!({"approved":true,"reason":"Independently inspected the specified result and observed checks"}).to_string(),
                "execute" => "Completed the assigned result and inspected existing artifacts".into(),
                "synthesis" => "All requested results are available".into(),
                purpose => bail!("Unexpected purpose {purpose}"),
            };
            let text = if req.purpose == "plan"
                && matches!(
                    self.mode,
                    Mode::Reassign
                        | Mode::ReviseResponsibility
                        | Mode::ParallelCommitment
                        | Mode::ParallelSelectionFailure
                ) {
                let mut plan: Value = serde_json::from_str(&text)?;
                plan["tasks"].as_array_mut().unwrap().push(json!({"title":"Third", "description":"Complete the reassigned final responsibility", "competence":"implementation", "difficulty":"simple", "dependencies":[if matches!(self.mode, Mode::ParallelCommitment | Mode::ParallelSelectionFailure) {0} else {1}], "checks":[]}));
                plan.to_string()
            } else {
                text
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
fn fixture(mode: Mode) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("work");
    std::fs::create_dir(&path).unwrap();
    let store = Store::open(&dir.path().join("state")).unwrap();
    let mut config = Config {
        providers: vec![ProviderConfig {
            id: "script".into(),
            kind: ProviderKind::Mock,
            command: "internal".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        }],
        agents: ["one", "two", "three"]
            .iter()
            .map(|id| AgentProfile {
                id: (*id).into(),
                name: (*id).into(),
                provider: "script".into(),
                model: Some("small".into()),
                instructions: "Use minimal scripted effort".into(),
                enabled: true,
            })
            .collect(),
        team: vec!["one".into(), "two".into(), "three".into()],
        limits: Limits {
            parallel: 1,
            turns: 80,
            turn_timeout_secs: 10,
            attempts: 3,
            resources: Some(ResourceLimits::default()),
        },
        team_constraints: TeamConstraints {
            fixed_roster: Some(vec!["one".into(), "two".into(), "three".into()]),
            ..Default::default()
        },
        ..Default::default()
    };
    for id in ["one", "two", "three"] {
        config.execution.insert(
            id.into(),
            AgentExecutionPolicy {
                fixed: ModelEffort {
                    model: Some("small".into()),
                    effort: Some("low".into()),
                },
                ..Default::default()
            },
        );
    }
    if matches!(mode, Mode::Membership | Mode::StaleMembership) {
        config.team_constraints.fixed_roster = None;
    }
    let (events, _) = mpsc::unbounded_channel();
    let script = Arc::new(Script {
        mode,
        requests: Mutex::new(vec![]),
        calls: Mutex::new(vec![]),
        second_executions: AtomicUsize::new(0),
    });
    let mut engine = Engine::new(store.clone(), config, events, CancellationToken::new())
        .unwrap()
        .with_execution_backend(script.clone())
        .unwrap();
    engine.use_memory = false;
    engine.acceptance_contracts.push(AcceptanceContract {
        knowledge_correction: None,
        task_title: "First".into(),
        criteria: vec![AcceptanceCriterion {
            id: "exact".into(),
            description: "Exact requested proof".into(),
        }],
        artifacts: vec!["proof.txt".into()],
        inputs: vec![],
        checks: vec![TrustedCheck {
            id: "proof".into(),
            criterion_ids: vec!["exact".into()],
            assertion: CheckAssertion::ExactBytes {
                artifact: "proof.txt".into(),
                expected: b"confirmed sibling\n".to_vec(),
            },
        }],
    });
    Fixture {
        _dir: dir,
        path,
        store,
        engine,
        script,
    }
}
async fn run(f: &Fixture) -> RunOutcome {
    f.engine
        .run(&f.path, "Create a checked proof and explain its fact", None)
        .await
        .unwrap()
}
fn decisions(f: &Fixture, session: &str) -> Vec<BoardDecision> {
    f.store
        .trace(session)
        .unwrap()
        .decisions
        .into_iter()
        .filter_map(|d| d.links.board.map(|b| *b))
        .collect()
}
#[tokio::test]
async fn actual_team_tools_commit_one_of_two_conflicting_claims_and_execute_the_winner() {
    let f = fixture(Mode::Claims);
    let out = run(&f).await;
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    let changes = decisions(&f, &out.session.id);
    assert_eq!(changes.len(), 2);
    assert!(changes[0].accepted);
    assert!(!changes[1].accepted && changes[1].reason.contains("stale_task"));
    let board = f.engine.board(&out.session.id).unwrap();
    assert_eq!(board.proposals[0].agent_id, "one");
    assert_eq!(board.proposals[1].agent_id, "two");
    assert_eq!(board.tasks[1].task.assignee.as_deref(), Some("one"));
    assert!(f
        .script
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|r| r.settings.effort.as_deref() == Some("low")));
    let accepted = f
        .store
        .trace(&out.session.id)
        .unwrap()
        .decisions
        .into_iter()
        .find(|d| d.kind == "task_accepted")
        .unwrap();
    assert_eq!(
        accepted.outcome,
        Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Confirmed
        })
    );
}
#[tokio::test]
async fn post_commit_revision_preserves_objectives_contract_and_rejects_stale_plan() {
    let f = fixture(Mode::Revision);
    let out = run(&f).await;
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    let changes = decisions(&f, &out.session.id);
    assert!(changes[0].accepted);
    assert!(!changes[1].accepted && changes[1].reason.contains("stale_plan"));
    let board = f.engine.board(&out.session.id).unwrap();
    assert!(board.tasks[1]
        .task
        .description
        .starts_with("Explain the resulting fact\n\nBoard approach:"));
    assert_eq!(
        board.tasks[1].task.dependencies,
        vec![board.tasks[0].task.id.clone()]
    );
    assert_eq!(board.tasks[1].task.checks, vec!["test -f proof.txt"]);
    assert_eq!(
        f.store
            .trace(&out.session.id)
            .unwrap()
            .decisions
            .iter()
            .filter(|d| d.kind == "acceptance_contract_captured")
            .count(),
        1
    );
}

#[tokio::test]
async fn active_executor_can_replace_a_broken_ordinary_check_before_attempts_exhaust() {
    let mut f = fixture(Mode::CheckReplacement);
    f.engine.config.limits.attempts = 1;
    let out = run(&f).await;
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    let trace = f.store.trace(&out.session.id).unwrap();
    let second = trace
        .tasks
        .iter()
        .find(|task| task.title == "Second")
        .unwrap();
    assert_eq!(second.checks, vec!["/bin/test -f proof.txt"]);
    assert_eq!(second.attempts, 1);
    assert_eq!(second.dependencies, vec![trace.tasks[0].id.clone()]);
    assert_eq!(second.access, TaskAccess::Write);
    assert_eq!(
        trace
            .assignments
            .iter()
            .filter(|assignment| assignment.purpose == "execute"
                && assignment
                    .task
                    .as_ref()
                    .is_some_and(|task| task.task_id == trace.tasks[0].id))
            .count(),
        1,
        "accepted sibling production was replayed"
    );
    let revision = decisions(&f, &out.session.id)
        .into_iter()
        .find(|decision| matches!(decision.proposal.change, BoardChange::ReplaceChecks { .. }))
        .unwrap();
    assert!(revision.accepted);
    assert_eq!(revision.review_ids.len(), 1);
    let revision_review = trace
        .decisions
        .iter()
        .find(|decision| decision.id == revision.review_ids[0])
        .unwrap();
    assert_ne!(
        revision_review.actor.as_ref(),
        Some(&revision.proposal.agent_id)
    );
    let evidence = revision_review.links.board_check_revision().unwrap();
    assert_eq!(
        evidence.retained_checks,
        vec!["missing-check-binary -f proof.txt"]
    );
    assert_eq!(evidence.proposed_checks, vec!["/bin/test -f proof.txt"]);
    assert_eq!(evidence.retained_runs.len(), 1);
    assert!(!evidence.retained_runs[0].passed);
    assert!(evidence.retained_runs[0]
        .output
        .contains("exit status: 127"));
    assert_eq!(evidence.proposed_runs.len(), 1);
    assert!(evidence.proposed_runs[0].passed);
    assert!(evidence.proposed_runs[0].output.contains("exit status: 0"));
    assert_eq!(
        trace
            .assignments
            .iter()
            .filter(|assignment| assignment.purpose == "review_check_revision")
            .count(),
        1,
        "the replacement must have one independently admitted review"
    );
    let origin = trace
        .assignments
        .iter()
        .find(|assignment| assignment.id == revision.proposal.assignment_id)
        .unwrap();
    assert_eq!(origin.purpose, "execute");
    assert_eq!(origin.agent_id, revision.proposal.agent_id);
    assert!(origin.grant_ids.contains(&revision.proposal.grant_id));
    assert_eq!(
        trace
            .assignments
            .iter()
            .filter(|assignment| assignment.purpose == "execute"
                && assignment
                    .task
                    .as_ref()
                    .is_some_and(|task| task.task_id == second.id))
            .count(),
        1,
        "check replacement repeated target production"
    );
    assert_eq!(
        trace
            .assignments
            .iter()
            .filter(|assignment| assignment.purpose == "review"
                && assignment
                    .task
                    .as_ref()
                    .is_some_and(|task| task.task_id == second.id))
            .count(),
        1,
        "the revised existing result needs one fresh ordinary review"
    );
    assert_eq!(
        trace.budget.as_ref().unwrap().admitted_invocations,
        trace.assignments.len() as u64,
        "check and candidate reviews were not charged to the session ledger"
    );
    let original_result = trace
        .decisions
        .iter()
        .find(|decision| {
            decision.kind == "result_submitted"
                && decision
                    .links
                    .task
                    .as_ref()
                    .is_some_and(|task| task.task_id == second.id)
        })
        .and_then(|decision| decision.links.result.as_ref())
        .unwrap();
    let revised_decision = trace
        .decisions
        .iter()
        .find(|decision| decision.kind == "result_check_revised")
        .unwrap();
    let revised_result = revised_decision.links.result.as_ref().unwrap();
    let result_revision = revised_decision.links.result_check_revision().unwrap();
    assert_ne!(original_result.id, revised_result.id);
    assert_eq!(original_result.version, revised_result.version);
    assert_eq!(result_revision.attempt_before, 1);
    assert_eq!(result_revision.attempt_after, 1);
    assert!(!f
        .store
        .result_is_current(&out.session.id, original_result)
        .unwrap());
    assert!(f
        .store
        .result_is_current(&out.session.id, revised_result)
        .unwrap());
    let checks = f.store.checks(&out.session.id).unwrap();
    assert_eq!(
        checks
            .iter()
            .filter(|run| run.command.as_deref() == Some("missing-check-binary -f proof.txt"))
            .count(),
        1
    );
    assert_eq!(
        checks
            .iter()
            .filter(|run| run.command.as_deref() == Some("/bin/test -f proof.txt"))
            .count(),
        3,
        "the reviewed replacement was not rerun for candidate and final acceptance"
    );
    let second_acceptance = trace
        .decisions
        .iter()
        .find(|decision| {
            decision.kind == "task_accepted"
                && decision
                    .links
                    .task
                    .as_ref()
                    .is_some_and(|task| task.task_id == second.id)
        })
        .unwrap();
    assert_eq!(
        second_acceptance.outcome,
        Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Unconfirmed
        })
    );
    assert_eq!(
        trace
            .decisions
            .iter()
            .filter(|decision| decision.kind == "acceptance_contract_captured")
            .count(),
        1
    );
}

#[tokio::test]
async fn trivial_check_replacement_is_refused_at_independent_review_boundary() {
    let mut f = fixture(Mode::CheckReplacementRejected);
    f.engine.config.limits.attempts = 1;
    let out = run(&f).await;
    assert_eq!(out.session.status, "blocked", "{}", out.summary);
    let trace = f.store.trace(&out.session.id).unwrap();
    assert_eq!(
        trace
            .assignments
            .iter()
            .filter(|assignment| assignment.purpose == "review_check_revision")
            .count(),
        1
    );
    let revision = decisions(&f, &out.session.id)
        .into_iter()
        .find(|decision| matches!(decision.proposal.change, BoardChange::ReplaceChecks { .. }))
        .unwrap();
    assert!(!revision.accepted);
    assert_eq!(revision.review_ids.len(), 1);
    let review = trace
        .decisions
        .iter()
        .find(|decision| decision.id == revision.review_ids[0])
        .unwrap();
    assert_eq!(review.outcome, Some(DecisionOutcome::Rejected));
    let evidence = review.links.board_check_revision().unwrap();
    assert_eq!(evidence.proposed_checks, vec!["test -d ."]);
    assert!(evidence.proposed_runs.iter().all(|run| run.passed));
    assert!(evidence.retained_runs.iter().all(|run| !run.passed));
    assert_eq!(
        trace
            .tasks
            .iter()
            .find(|task| task.title == "Second")
            .unwrap()
            .checks,
        vec!["missing-check-binary -f proof.txt"]
    );
}
#[tokio::test]
async fn a_participant_can_distribute_work_and_a_new_subtask_reaches_independent_acceptance() {
    for mode in [Mode::Distribution, Mode::Add] {
        let f = fixture(mode);
        let out = run(&f).await;
        assert_eq!(out.session.status, "completed", "{}", out.summary);
        let board = f.engine.board(&out.session.id).unwrap();
        assert!(decisions(&f, &out.session.id)[0].accepted);
        if mode == Mode::Distribution {
            assert_eq!(board.tasks[1].task.assignee.as_deref(), Some("two"));
        } else {
            assert_eq!(board.tasks.len(), 3);
            assert_eq!(board.tasks[2].task.state, TaskState::Accepted);
            assert!(f
                .store
                .trace(&out.session.id)
                .unwrap()
                .decisions
                .iter()
                .any(|d| d.kind == "board_committed"
                    && d.links.related_task_ids.contains(&board.tasks[2].task.id)));
        }
    }
}
#[tokio::test]
async fn malicious_proposals_cannot_override_identity_versions_pins_or_contracts() {
    let f = fixture(Mode::Malicious);
    let out = run(&f).await;
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    let calls = f.script.calls.lock().unwrap();
    assert_eq!(calls.len(), 6);
    assert!(calls.iter().take(2).all(|c| c["ok"] == true));
    assert!(calls.iter().skip(2).all(|c| c["ok"] == false));
    let changes = decisions(&f, &out.session.id);
    assert_eq!(changes.len(), 2);
    assert!(changes.iter().all(|d| !d.accepted));
    assert!(changes[0].reason.contains("invalid_execution_choice"));
    let board = f.engine.board(&out.session.id).unwrap();
    assert_eq!(
        board.team.unwrap().current_members,
        vec!["one", "two", "three"]
    );
    assert_eq!(
        board.tasks[1].task.description,
        "Explain the resulting fact"
    );
}
#[tokio::test]
async fn recovery_inspects_uncertain_effect_before_replay_and_preserves_confirmed_sibling() {
    let f = fixture(Mode::Recovery);
    let out = run(&f).await;
    assert_ne!(out.session.status, "completed");
    let prior = f.store.trace(&out.session.id).unwrap();
    assert_eq!(prior.tasks[0].state, TaskState::Accepted);
    let acceptance = prior
        .decisions
        .iter()
        .find(|d| d.kind == "task_accepted")
        .unwrap()
        .clone();
    let expired = f
        .script
        .requests
        .lock()
        .unwrap()
        .iter()
        .find(|r| r.purpose == "execute")
        .unwrap()
        .clone();
    // The old process capability cannot mutate a restarted server.
    assert!(call(
        &expired,
        "task_propose",
        json!({"title":"replay","description":"old token"})
    )
    .await
    .is_err());
    let resumed = f
        .engine
        .run(
            &f.path,
            "Create a checked proof and explain its fact",
            Some(&out.session.id),
        )
        .await
        .unwrap();
    assert_eq!(resumed.session.status, "completed", "{}", resumed.summary);
    let after = f.store.trace(&out.session.id).unwrap();
    let retained = after
        .decisions
        .iter()
        .find(|d| d.id == acceptance.id)
        .unwrap();
    assert_eq!(
        serde_json::to_value(retained).unwrap(),
        serde_json::to_value(acceptance).unwrap()
    );
    assert_eq!(after.tasks[0].attempts, 1);
    let requests = f.script.requests.lock().unwrap();
    let inspection = requests
        .iter()
        .position(|r| r.purpose == "review" && r.prompt.contains("Execution was interrupted"))
        .unwrap();
    let executions = requests
        .iter()
        .enumerate()
        .filter(|(_, r)| r.purpose == "execute" && r.prompt.contains("\nSecond\n"))
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    assert_eq!(executions.len(), 2);
    assert!(executions[0] < inspection && inspection < executions[1]);
}

#[tokio::test]
async fn reassignment_and_membership_changes_preserve_historical_contributions() {
    let f = fixture(Mode::Reassign);
    let out = run(&f).await;
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    let changes = decisions(&f, &out.session.id);
    assert_eq!(changes.len(), 2);
    assert!(changes.iter().all(|d| d.accepted));
    assert_eq!(changes[0].commitment.as_ref().unwrap().agent_id, "two");
    assert_eq!(changes[1].commitment.as_ref().unwrap().agent_id, "one");
    let board = f.engine.board(&out.session.id).unwrap();
    assert_eq!(board.tasks[2].task.assignee.as_deref(), Some("one"));
    assert_eq!(board.tasks[2].task.attempts, 1);
    let f = fixture(Mode::Membership);
    let out = run(&f).await;
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    assert!(decisions(&f, &out.session.id)[0].accepted);
    let captured = f.store.session(&out.session.id).unwrap();
    assert!(captured.team.iter().any(|a| a.id == "one"));
    assert!(f
        .store
        .trace(&out.session.id)
        .unwrap()
        .assignments
        .iter()
        .any(|a| a.agent_id == "one" && a.purpose == "execute"));
}
struct Reverse;
impl BoardProposalPolicy for Reverse {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "test.reverse-board".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, board: &BoardSnapshot) -> Result<Vec<String>> {
        Ok(board
            .proposals
            .iter()
            .rev()
            .filter(|p| p.status == BoardProposalStatus::Pending)
            .map(|p| p.id.clone())
            .collect())
    }
}
#[tokio::test]
async fn replacing_strategy_changes_choice_without_bypassing_runtime_validation() {
    let mut f = fixture(Mode::Claims);
    f.engine = f
        .engine
        .with_board_proposal_policy(Arc::new(Reverse))
        .unwrap();
    let out = run(&f).await;
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    let changes = decisions(&f, &out.session.id);
    assert!(changes[0].accepted && !changes[1].accepted);
    assert_eq!(
        f.engine.board(&out.session.id).unwrap().tasks[1]
            .task
            .assignee
            .as_deref(),
        Some("two")
    );
    assert!(changes
        .iter()
        .all(|d| d.implementation.id == "test.reverse-board"));
}

#[tokio::test]
async fn racing_public_claims_have_one_winner_and_do_not_issue_authority() {
    let f = fixture(Mode::Distribution);
    let out = run(&f).await;
    let mut task = f.store.tasks(&out.session.id).unwrap()[1].clone();
    task.id = new_id();
    task.title = "Competing pending responsibility".into();
    task.state = TaskState::Ready;
    task.assignee = None;
    task.reviewer = None;
    task.result = None;
    task.attempts = 0;
    task.dependencies.clear();
    f.store.save_task(&task).unwrap();
    let board = f.engine.board(&out.session.id).unwrap();
    let reference = BoardTaskRef {
        task_id: task.id.clone(),
        version: board
            .tasks
            .iter()
            .find(|t| t.task.id == task.id)
            .unwrap()
            .version
            .clone(),
    };
    let before = f.store.trace(&out.session.id).unwrap().invocations.len();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let outcomes = std::thread::scope(|scope| {
        let workers = ["one", "two"]
            .into_iter()
            .map(|agent| {
                let mut candidate = task.clone();
                candidate.assign(agent, &Default::default()).unwrap();
                let reference = reference.clone();
                let barrier = barrier.clone();
                let store = f.store.clone();
                scope.spawn(move || {
                    barrier.wait();
                    store
                        .claim_board_task(&reference, &candidate)
                        .map_err(|e| e.to_string())
                })
            })
            .collect::<Vec<_>>();
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(outcomes
        .iter()
        .filter_map(|r| r.as_ref().err())
        .all(|e| e.contains("stale_claim")));
    let trace = f.store.trace(&out.session.id).unwrap();
    assert_eq!(trace.invocations.len(), before);
    assert!(!trace
        .assignments
        .iter()
        .any(|a| a.task.as_ref().is_some_and(|t| t.task_id == task.id)));
    assert_eq!(
        trace
            .decisions
            .iter()
            .filter(|d| d.kind == "task_claimed"
                && d.links.task.as_ref().is_some_and(|t| t.task_id == task.id))
            .count(),
        1
    );
}

#[tokio::test]
async fn an_approach_revision_keeps_responsibility_and_stale_membership_cannot_overwrite() {
    let f = fixture(Mode::ReviseResponsibility);
    let out = run(&f).await;
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    let changes = decisions(&f, &out.session.id);
    assert_eq!(changes.len(), 2);
    assert!(changes
        .iter()
        .all(|d| d.accepted && d.commitment.as_ref().unwrap().agent_id == "two"));
    let board = f.engine.board(&out.session.id).unwrap();
    assert_eq!(board.tasks[2].task.assignee.as_deref(), Some("two"));
    assert!(board.tasks[2]
        .task
        .description
        .contains("Keep the accepted responsibility"));
    let f = fixture(Mode::StaleMembership);
    let out = run(&f).await;
    assert_eq!(out.session.status, "completed", "{}", out.summary);
    let changes = decisions(&f, &out.session.id);
    assert_eq!(changes.len(), 2);
    assert!(changes[0].accepted);
    assert!(!changes[1].accepted && changes[1].reason.contains("stale_membership"));
}

#[tokio::test]
async fn parallel_commitment_waits_for_busy_owner_and_keeps_prior_claims_running() {
    for parallel in [2, 1] {
        let mut f = fixture(Mode::ParallelCommitment);
        f.engine.config.limits.parallel = parallel;
        let out = run(&f).await;
        let trace = f.store.trace(&out.session.id).unwrap();
        assert_eq!(
            out.session.status, "completed",
            "parallel={parallel}: {}",
            out.summary
        );
        assert_eq!(trace.tasks.len(), 3);
        assert!(trace
            .tasks
            .iter()
            .all(|task| task.state == TaskState::Accepted));
        let third = trace
            .tasks
            .iter()
            .find(|task| task.title == "Third")
            .unwrap();
        assert_eq!(third.assignee.as_deref(), Some("one"));
        assert_eq!(third.attempts, 1);
        assert_eq!(
            trace
                .assignments
                .iter()
                .filter(|a| a.purpose == "execute")
                .count(),
            3
        );
        assert!(trace
            .invocations
            .iter()
            .all(|i| i.requested.effort.as_deref() == Some("low")));
        if parallel == 2 {
            assert!(trace
                .decisions
                .iter()
                .any(|d| d.kind == "assignment_waiting"
                    && d.links
                        .task
                        .as_ref()
                        .is_some_and(|task| task.task_id == third.id)
                    && d.links
                        .workspace_wait
                        .as_ref()
                        .is_some_and(|wait| wait.code == "commitment_busy")));
        }
    }
}

struct FailRepeatedExecutionChoice(Mutex<std::collections::HashMap<String, usize>>);
impl AllocationPolicy for FailRepeatedExecutionChoice {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "test.late-selection-error".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, input: &AllocationInput) -> Result<AllocationProposal> {
        if input.demand.purpose == "execute" {
            if let Some(task) = &input.demand.task_id {
                let mut counts = self.0.lock().unwrap();
                let count = counts.entry(task.clone()).or_default();
                *count += 1;
                if *count > 1 {
                    bail!("scripted_selection_error: configuration became unavailable after commitment");
                }
            }
        }
        BoundedAllocationPolicy.propose(input)
    }
}
#[tokio::test]
async fn later_selection_error_drains_and_reviews_already_claimed_work() {
    let mut f = fixture(Mode::ParallelSelectionFailure);
    f.engine.config.limits.parallel = 2;
    f.engine = f
        .engine
        .with_allocation_policy(Arc::new(FailRepeatedExecutionChoice(Mutex::new(
            Default::default(),
        ))))
        .unwrap();
    let out = run(&f).await;
    assert_eq!(out.session.status, "blocked");
    assert!(out.summary.contains("scripted_selection_error"));
    let trace = f.store.trace(&out.session.id).unwrap();
    let second = trace
        .tasks
        .iter()
        .find(|task| task.title == "Second")
        .unwrap();
    let third = trace
        .tasks
        .iter()
        .find(|task| task.title == "Third")
        .unwrap();
    assert_eq!(second.state, TaskState::Accepted);
    assert_eq!(third.state, TaskState::Ready);
    assert!(!trace
        .tasks
        .iter()
        .any(|task| task.state == TaskState::Running));
    assert_eq!(
        trace
            .assignments
            .iter()
            .filter(|a| a.purpose == "execute")
            .count(),
        2
    );
    assert_eq!(
        f.engine
            .board(&out.session.id)
            .unwrap()
            .tasks
            .iter()
            .find(|t| t.task.id == third.id)
            .unwrap()
            .commitment
            .as_ref()
            .unwrap()
            .agent_id,
        "two"
    );
}

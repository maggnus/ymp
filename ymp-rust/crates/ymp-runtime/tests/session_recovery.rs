//! Offline walks through the public engine; every home and workspace is temporary.
use anyhow::{bail, Result};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};
use ymp_runtime::*;
use ymp_storage::Store;

#[path = "session_recovery/execution_rework.rs"]
mod execution_rework;
#[path = "session_recovery/rework.rs"]
mod rework;

#[derive(Clone, Copy)]
enum Mode {
    Unknown,
    Transient,
    Auth,
    Negative,
    Malformed,
}
struct Gate {
    purpose: &'static str,
    started: tokio::sync::Semaphore,
    release: tokio::sync::Semaphore,
}
struct Script {
    failure_purpose: &'static str,
    gate: Option<Arc<Gate>>,
    mode: Mode,
    writes: bool,
    failures: AtomicUsize,
    calls: Mutex<Vec<TurnRequest>>,
}
impl ExecutionBackend for Script {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "fixture.recovery".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, _: &TurnRequest) -> WorkspaceAccess {
        if self.writes {
            WorkspaceAccess::WriteAll
        } else {
            WorkspaceAccess::ReadAll
        }
    }
    fn execute(
        &self,
        request: TurnRequest,
        _: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(request.clone());
            if let Some(gate) = &self.gate {
                if request.purpose == gate.purpose {
                    gate.started.add_permits(1);
                    gate.release.acquire().await.unwrap().forget();
                }
            }
            if request.purpose == self.failure_purpose
                && self
                    .failures
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                    .is_ok()
            {
                match self.mode {
                    Mode::Unknown => bail!("Unknown scripted failure"),
                    Mode::Transient => return Err(std::io::Error::new(std::io::ErrorKind::ConnectionReset, "Scripted transport reset").into()),
                    Mode::Auth => return Err(ymp_providers::NativeFailure { class:FailureClass::Authentication, code:Some("401".into()) }.into()),
                    Mode::Negative => return Ok(TurnResult { text:json!({"approved":false,"reason":"Keep this objection: missing analysis"}).to_string(), session_id:new_id(), usage:None }),
                    Mode::Malformed => return Ok(TurnResult { text:"incomplete {".into(), session_id:new_id(), usage:None }),
                }
            }
            let text = match request.purpose.as_str() {
                "plan" => json!({"summary":"Saved proposal", "tasks":[{"title":"Read", "description":"Return findings", "access":"read_only", "competence":"analysis", "difficulty":"simple", "dependencies":[], "checks":[]}]}).to_string(),
                "review_plan" | "review" | "final_review" => json!({"approved":true,"reason":"Independent evidence inspected"}).to_string(),
                "execute" => "Findings from this invocation".into(),
                "synthesis" => "Accepted findings".into(),
                p => bail!("Unexpected purpose {p}"),
            };
            let text = if request.purpose == "plan"
                && self.gate.as_ref().is_some_and(|g| g.purpose == "execute")
            {
                let mut plan: serde_json::Value = serde_json::from_str(&text)?;
                plan["tasks"].as_array_mut().unwrap().push(json!({"title":"Second read","description":"Use the first finding","access":"read_only","competence":"analysis","difficulty":"simple","dependencies":[0],"checks":[]}));
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
fn config() -> Config {
    Config {
        providers: vec![ProviderConfig {
            id: "offline".into(),
            kind: ProviderKind::Mock,
            command: "internal".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        }],
        agents: ["author", "reviewer", "reserve"]
            .into_iter()
            .map(|id| AgentProfile {
                id: id.into(),
                name: id.into(),
                provider: "offline".into(),
                model: None,
                instructions: "Scripted fixture".into(),
                enabled: true,
            })
            .collect(),
        team: vec!["author".into(), "reviewer".into()],
        limits: Limits {
            parallel: 1,
            turns: 80,
            turn_timeout_secs: 10,
            attempts: 3,
            resources: Some(ResourceLimits {
                startup_invocations: 20,
                ..Default::default()
            }),
        },
        ..Default::default()
    }
}
fn engine(store: Store, script: Arc<Script>) -> Engine {
    let (events, _) = mpsc::unbounded_channel();
    let mut engine = Engine::new(store, config(), events, CancellationToken::new())
        .unwrap()
        .with_execution_backend(script)
        .unwrap();
    engine.use_memory = false;
    engine
}
#[tokio::test]
async fn restart_retains_plan_without_duplicate_planning() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let home = temp.path().join("state");
    let script = Arc::new(Script {
        failure_purpose: "review_plan",
        mode: Mode::Unknown,
        gate: None,
        writes: false,
        failures: AtomicUsize::new(1),
        calls: Mutex::new(vec![]),
    });
    let first = engine(Store::open(&home)?, script.clone())
        .run(&path, "Inspect this directory", None)
        .await?;
    assert_ne!(first.session.status, "completed");
    let reopened = Store::open(&home)?;
    let stage = reopened
        .recovery_stages(&first.session.id)?
        .into_iter()
        .find(|s| s.purpose == "review_plan")
        .unwrap();
    reopened.control_recovery(&RecoveryControlCommand {
        session_id: first.session.id.clone(),
        stage_id: stage.id,
        expected_revision: stage.revision,
        command_id: new_id(),
        action: RecoveryControl::Retry,
    })?;
    let second = engine(Store::open(&home)?, script.clone())
        .run(&path, "Inspect this directory", Some(&first.session.id))
        .await?;
    assert_eq!(second.session.status, "completed", "{}", second.summary);
    assert_eq!(
        script
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.purpose == "plan")
            .count(),
        1,
        "saved plan must survive restart"
    );
    Ok(())
}

struct ManualWait;
impl RecoveryPolicy for ManualWait {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "fixture.manual-wait".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, _: &RecoveryInput) -> Result<RecoveryAction> {
        Ok(RecoveryAction::Wait {
            condition: "Owner must select continuation".into(),
        })
    }
}
async fn walk(
    mode: Mode,
    failures: usize,
    writes: bool,
    manual: bool,
    separate: bool,
    cap: bool,
) -> Result<(
    RunOutcome,
    SessionTrace,
    Vec<RecoveryStage>,
    Vec<TurnRequest>,
)> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let store = Store::open(&temp.path().join("state"))?;
    let script = Arc::new(Script {
        failure_purpose: "review_plan",
        mode,
        gate: None,
        writes,
        failures: AtomicUsize::new(failures),
        calls: Mutex::new(vec![]),
    });
    let mut engine = engine(store.clone(), script.clone());
    if manual {
        engine = engine.with_recovery_policy(Arc::new(ManualWait))?;
    }
    if separate {
        for index in 0..3 {
            let mut provider = engine.config.providers[0].clone();
            provider.id = format!("offline-{index}");
            engine.config.agents[index].provider = provider.id.clone();
            engine.config.providers.push(provider);
        }
    }
    if cap {
        let r = engine.config.limits.resources.as_mut().unwrap();
        r.observed_tokens = Some(100_000);
        r.invocation_tokens = Some(100);
    }
    let outcome = engine.run(&path, "Inspect this directory", None).await?;
    let trace = store.trace(&outcome.session.id)?;
    let stages = store.recovery_stages(&outcome.session.id)?;
    let calls = script.calls.lock().unwrap().clone();
    Ok((outcome, trace, stages, calls))
}
#[tokio::test]
async fn finite_transient_and_manual_wait_use_the_same_consumer() -> Result<()> {
    for manual in [false, true] {
        let (outcome, trace, stages, calls) =
            walk(Mode::Transient, 1, false, manual, false, false).await?;
        assert_eq!(
            outcome.session.status == "completed",
            !manual,
            "{}",
            outcome.summary
        );
        assert_eq!(calls.iter().filter(|c| c.purpose == "plan").count(), 1);
        assert_eq!(
            calls.iter().filter(|c| c.purpose == "review_plan").count(),
            if manual { 1 } else { 2 }
        );
        let decisions = trace
            .decisions
            .iter()
            .filter_map(|d| d.links.recovery.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1);
        assert!(decisions[0].accepted);
        assert_eq!(
            decisions[0].policy.implementation.id,
            if manual {
                "fixture.manual-wait"
            } else {
                "ymp.bounded-recovery"
            }
        );
        assert_eq!(
            stages
                .iter()
                .find(|s| s.purpose == "review_plan")
                .unwrap()
                .failures[0]
                .class,
            FailureClass::TransientTransport
        );
    }
    Ok(())
}
#[tokio::test]
async fn permanent_failure_replaces_independent_reviewer_through_allocation() -> Result<()> {
    let (outcome, trace, _, calls) = walk(Mode::Auth, 1, false, false, true, false).await?;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let reviews = calls
        .iter()
        .filter(|c| c.purpose == "review_plan")
        .collect::<Vec<_>>();
    assert_eq!(reviews.len(), 2);
    assert_ne!(reviews[0].profile.id, reviews[1].profile.id);
    assert_ne!(reviews[0].provider.id, reviews[1].provider.id);
    let author = &calls
        .iter()
        .find(|c| c.purpose == "plan")
        .unwrap()
        .profile
        .id;
    assert!(reviews.iter().all(|c| &c.profile.id != author));
    assert!(trace
        .decisions
        .iter()
        .filter_map(|d| d.links.allocation.as_ref())
        .any(|a| a.accepted
            && a.input.demand.purpose == "review_plan"
            && a.proposal
                .executor
                .as_ref()
                .is_some_and(|c| c.agent_id == reviews[1].profile.id)));
    Ok(())
}
#[tokio::test]
async fn correlated_provider_failure_has_finite_wait_without_acceptance() -> Result<()> {
    let (outcome, trace, stages, calls) =
        walk(Mode::Transient, 99, false, false, false, false).await?;
    assert_ne!(outcome.session.status, "completed");
    assert_eq!(calls.len(), 3);
    assert_eq!(stages[0].status, RecoveryStatus::Waiting);
    assert!(stages[0]
        .condition
        .as_ref()
        .unwrap()
        .contains("unaffected provider"));
    assert!(trace.tasks.is_empty());
    assert!(!trace.decisions.iter().any(|d| d.kind == "plan_committed"));
    Ok(())
}
#[tokio::test]
async fn negative_verdict_requires_revision_but_malformed_response_is_not_a_verdict() -> Result<()>
{
    for mode in [Mode::Negative, Mode::Malformed] {
        let (outcome, trace, _, calls) = walk(mode, 1, false, false, false, false).await?;
        assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
        let negative = matches!(mode, Mode::Negative);
        assert_eq!(
            calls.iter().filter(|c| c.purpose == "plan").count(),
            if negative { 2 } else { 1 }
        );
        let rejected = trace
            .decisions
            .iter()
            .filter(|d| d.kind == "plan_review" && d.outcome == Some(DecisionOutcome::Rejected))
            .collect::<Vec<_>>();
        assert_eq!(rejected.len(), usize::from(negative));
        if negative {
            assert!(rejected[0].reason.contains("Keep this objection"));
        }
        assert_eq!(
            trace
                .decisions
                .iter()
                .filter(|d| d.kind == "malformed_review")
                .count(),
            usize::from(!negative)
        );
    }
    Ok(())
}
#[tokio::test]
async fn uncertainty_and_unknown_accounting_block_replay() -> Result<()> {
    let (outcome, _, stages, calls) = walk(Mode::Transient, 1, true, false, false, false).await?;
    assert_ne!(outcome.session.status, "completed");
    assert_eq!(calls.len(), 2);
    assert_eq!(stages[0].status, RecoveryStatus::OwnerAction);
    assert!(stages[0].condition.as_ref().unwrap().contains("Uncertain"));
    let (outcome, trace, _, calls) = walk(Mode::Transient, 1, false, false, false, true).await?;
    assert_ne!(outcome.session.status, "completed");
    assert_eq!(
        calls.len(),
        1,
        "Unknown planner accounting already prevents review under the cap"
    );
    assert_eq!(
        trace.budget.unwrap().last_denial.unwrap().code,
        "unknown_usage"
    );
    Ok(())
}

#[tokio::test]
async fn owner_pause_idempotence_and_stale_stage_survive_reopen() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let home = temp.path().join("state");
    let store = Store::open(&home)?;
    let script = Arc::new(Script {
        failure_purpose: "review_plan",
        mode: Mode::Unknown,
        gate: None,
        writes: false,
        failures: AtomicUsize::new(1),
        calls: Mutex::new(vec![]),
    });
    let engine = engine(store.clone(), script.clone());
    let first = engine.run(&path, "Inspect", None).await?;
    let stage = store.recovery_stages(&first.session.id)?.remove(0);
    let command = RecoveryControlCommand {
        session_id: first.session.id.clone(),
        stage_id: stage.id.clone(),
        expected_revision: stage.revision,
        command_id: new_id(),
        action: RecoveryControl::Pause,
    };
    let receipt = engine.control_recovery(&command)?;
    assert_eq!(Store::open(&home)?.control_recovery(&command)?, receipt);
    let mut stale = command.clone();
    stale.command_id = new_id();
    assert!(engine
        .control_recovery(&stale)
        .unwrap_err()
        .to_string()
        .contains("stale_recovery"));
    let mut conflict = command.clone();
    conflict.action = RecoveryControl::Retry;
    assert!(engine.control_recovery(&conflict).is_err());
    let before = script.calls.lock().unwrap().len();
    let paused = engine
        .run(&path, "Inspect", Some(&first.session.id))
        .await?;
    assert_ne!(paused.session.status, "completed");
    assert_eq!(script.calls.lock().unwrap().len(), before);
    let mut changed = stage.clone();
    changed.revision += 1;
    assert!(store
        .transition_recovery(stage.revision, &changed, None)
        .is_err());
    let command = RecoveryControlCommand {
        expected_revision: receipt.resulting_revision,
        command_id: new_id(),
        action: RecoveryControl::Continue,
        ..command
    };
    engine.control_recovery(&command)?;
    let result = engine
        .run(&path, "Inspect", Some(&first.session.id))
        .await?;
    assert_eq!(result.session.status, "completed", "{}", result.summary);
    Ok(())
}

fn team_command(view: &TeamControlView, action: OwnerTeamAction) -> OwnerTeamCommand {
    OwnerTeamCommand {
        session_id: view.session_id.clone(),
        expected_revision: view.revision,
        command_id: new_id(),
        revise_pinned_roster: false,
        action,
    }
}
#[tokio::test]
async fn live_replace_drains_writer_and_running_engine_observes_new_native_member() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let store = Store::open(&temp.path().join("state"))?;
    let gate = Arc::new(Gate {
        purpose: "execute",
        started: tokio::sync::Semaphore::new(0),
        release: tokio::sync::Semaphore::new(0),
    });
    let script = Arc::new(Script {
        failure_purpose: "review_plan",
        gate: Some(gate.clone()),
        mode: Mode::Unknown,
        writes: true,
        failures: AtomicUsize::new(0),
        calls: Mutex::new(vec![]),
    });
    let mut running = engine(store.clone(), script.clone());
    running.config.limits.parallel = 2;
    running.config.team_constraints.fixed_roster = Some(vec!["author".into(), "reserve".into()]);
    let mut owner = running.clone();
    let session_id = new_id();
    let background = tokio::spawn({
        let path = path.clone();
        let session_id = session_id.clone();
        async move { running.run_identified(&path, "Inspect", &session_id).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), gate.started.acquire())
        .await??
        .forget();
    let before = store.trace(&session_id)?;
    let producer = before
        .assignments
        .iter()
        .find(|a| a.purpose == "execute" && a.state == InvocationState::Running)
        .unwrap();
    let mut newcomer = owner.config.agents[0].clone();
    newcomer.id = "new-native-member".into();
    newcomer.name = "new-native-member".into();
    owner.config.agents.push(newcomer.clone());
    let view = owner.team_control(&session_id)?;
    let mut command = team_command(
        &view,
        OwnerTeamAction::Replace {
            agent_id: producer.agent_id.clone(),
            replacement_id: newcomer.id.clone(),
        },
    );
    assert!(
        owner.owner_team_command(&command).is_err(),
        "Pinned roster needs explicit owner revision"
    );
    assert_eq!(owner.team_control(&session_id)?.revision, view.revision);
    command.revise_pinned_roster = true;
    let receipt = owner.owner_team_command(&command)?;
    assert_eq!(owner.owner_team_command(&command)?, receipt);
    let pending = owner.team_control(&session_id)?;
    assert_eq!(pending.pending_departures.len(), 1);
    assert!(pending
        .effective
        .current_members
        .contains(&producer.agent_id));
    assert!(!pending.desired_members.contains(&producer.agent_id));
    assert!(pending.desired_members.contains(&newcomer.id));
    assert!(pending
        .responsibilities
        .iter()
        .any(|r| r.agent_id == producer.agent_id && r.kind == "workspace_access"));
    assert_eq!(
        store
            .invocation(
                &session_id,
                &before
                    .invocations
                    .iter()
                    .find(|i| i.assignment_id == producer.id)
                    .unwrap()
                    .id
            )?
            .state,
        InvocationState::Running
    );
    assert_eq!(
        serde_json::to_value(store.session_policy(&session_id)?)?,
        serde_json::to_value(&before.policy)?
    );
    let mut stale = command.clone();
    stale.command_id = new_id();
    assert!(owner
        .owner_team_command(&stale)
        .unwrap_err()
        .to_string()
        .contains("stale_owner_command"));
    let mut forbidden = producer.clone();
    forbidden.id = new_id();
    forbidden.purpose = "conversation".into();
    forbidden.task = None;
    forbidden.grant_ids.clear();
    let mut invocation = before
        .invocations
        .iter()
        .find(|i| i.assignment_id == producer.id)
        .unwrap()
        .clone();
    invocation.id = new_id();
    invocation.assignment_id = forbidden.id.clone();
    invocation.usage = None;
    let denial = store.admit_invocation(&forbidden, invocation).unwrap_err();
    assert!(denial.to_string().contains("pending_departure"), "{denial}");
    let mut stale_allocation = before
        .decisions
        .iter()
        .rev()
        .find(|d| d.links.allocation.as_ref().is_some_and(|a| a.accepted))
        .unwrap()
        .clone();
    stale_allocation.id = new_id();
    assert!(store
        .commit_allocation(&stale_allocation)
        .unwrap_err()
        .to_string()
        .contains("stale_allocation"));
    let call_count = script.calls.lock().unwrap().len();
    gate.release.add_permits(2);
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), background).await???;
    assert_eq!(result.session.status, "completed", "{}", result.summary);
    let view = owner.team_control(&session_id)?;
    assert!(view.pending_departures.is_empty());
    assert!(!view.effective.current_members.contains(&producer.agent_id));
    assert!(view.effective.current_members.contains(&newcomer.id));
    assert!(script.calls.lock().unwrap()[call_count..]
        .iter()
        .all(|r| r.profile.id != producer.agent_id));
    let after = store.trace(&session_id)?;
    assert_eq!(after.tasks[0].assignee.as_ref(), Some(&producer.agent_id));
    assert!(after.tasks.iter().all(|t| t.state == TaskState::Accepted));
    assert!(after
        .tasks
        .iter()
        .any(|t| t.assignee.as_ref() == Some(&newcomer.id)));
    assert!(script
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.purpose == "execute" && r.profile.id == newcomer.id));
    assert_eq!(after.policy.as_ref().unwrap().captured_team.len(), 2);
    assert_eq!(
        after.budget.as_ref().unwrap().limits,
        before.budget.as_ref().unwrap().limits
    );
    Ok(())
}
#[tokio::test]
async fn owner_add_idle_remove_and_no_reviewer_choice_are_atomic() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let store = Store::open(&temp.path().join("state"))?;
    let script = Arc::new(Script {
        failure_purpose: "review_plan",
        gate: None,
        mode: Mode::Unknown,
        writes: false,
        failures: AtomicUsize::new(1),
        calls: Mutex::new(vec![]),
    });
    let engine = engine(store.clone(), script.clone());
    let first = engine.run(&path, "Inspect", None).await?;
    let original = store.trace(&first.session.id)?;
    let view = engine.team_control(&first.session.id)?;
    let added = engine
        .config
        .agents
        .iter()
        .find(|a| !view.effective.current_members.contains(&a.id))
        .unwrap();
    let add = team_command(
        &view,
        OwnerTeamAction::Add {
            agent_id: added.id.clone(),
        },
    );
    engine.owner_team_command(&add)?;
    let view = engine.team_control(&first.session.id)?;
    assert!(view.effective.current_members.contains(&added.id));
    let forged = team_command(
        &view,
        OwnerTeamAction::Add {
            agent_id: "invented-native-name".into(),
        },
    );
    assert!(engine.owner_team_command(&forged).is_err());
    assert_eq!(
        engine.team_control(&first.session.id)?.revision,
        view.revision
    );
    engine.owner_team_command(&team_command(
        &view,
        OwnerTeamAction::Remove {
            agent_id: added.id.clone(),
        },
    ))?;
    let mut view = engine.team_control(&first.session.id)?;
    assert!(view.pending_departures.is_empty());
    let author = original
        .decisions
        .iter()
        .find(|d| d.kind == "plan_proposed")
        .unwrap()
        .actor
        .as_ref()
        .unwrap();
    for id in view
        .effective
        .current_members
        .clone()
        .into_iter()
        .filter(|id| id != author)
    {
        engine.owner_team_command(&team_command(
            &view,
            OwnerTeamAction::Remove { agent_id: id },
        ))?;
        view = engine.team_control(&first.session.id)?;
    }
    assert_eq!(view.effective.current_members, vec![author.clone()]);
    let stage = store.recovery_stages(&first.session.id)?.remove(0);
    engine.control_recovery(&RecoveryControlCommand {
        session_id: first.session.id.clone(),
        stage_id: stage.id,
        expected_revision: stage.revision,
        command_id: new_id(),
        action: RecoveryControl::Continue,
    })?;
    let resumed = engine
        .run(&path, "Inspect", Some(&first.session.id))
        .await?;
    assert_ne!(resumed.session.status, "completed");
    assert!(store.tasks(&first.session.id)?.is_empty());
    let after = store.trace(&first.session.id)?;
    assert_eq!(after.usage.total, original.usage.total);
    assert_eq!(
        serde_json::to_value(after.policy)?,
        serde_json::to_value(original.policy)?
    );
    Ok(())
}

#[tokio::test]
async fn candidate_and_final_review_restart_preserve_production_and_result_version() -> Result<()> {
    for purpose in ["review", "final_review"] {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("work");
        std::fs::create_dir(&path)?;
        let home = temp.path().join("state");
        let store = Store::open(&home)?;
        let script = Arc::new(Script {
            failure_purpose: purpose,
            gate: None,
            mode: Mode::Unknown,
            writes: false,
            failures: AtomicUsize::new(1),
            calls: Mutex::new(vec![]),
        });
        let first = engine(store.clone(), script.clone())
            .run(&path, "Inspect", None)
            .await?;
        assert_ne!(first.session.status, "completed");
        let before = store.trace(&first.session.id)?;
        let stage = store
            .recovery_stages(&first.session.id)?
            .into_iter()
            .find(|s| s.purpose == purpose && s.status != RecoveryStatus::Complete)
            .unwrap();
        assert!(stage.result.is_some());
        store.control_recovery(&RecoveryControlCommand {
            session_id: first.session.id.clone(),
            stage_id: stage.id,
            expected_revision: stage.revision,
            command_id: new_id(),
            action: RecoveryControl::Continue,
        })?;
        let resumed = engine(Store::open(&home)?, script.clone())
            .run(&path, "Inspect", Some(&first.session.id))
            .await?;
        assert_eq!(
            resumed.session.status, "completed",
            "{purpose}: {}",
            resumed.summary
        );
        let calls = script.calls.lock().unwrap();
        assert_eq!(calls.iter().filter(|r| r.purpose == "plan").count(), 1);
        assert_eq!(calls.iter().filter(|r| r.purpose == "execute").count(), 1);
        let after = store.trace(&first.session.id)?;
        for decision in before
            .decisions
            .iter()
            .filter(|d| d.kind == "result_submitted" || d.kind == "result_aggregated")
        {
            assert!(after
                .decisions
                .iter()
                .any(|d| d.id == decision.id && d.links.result == decision.links.result));
        }
        assert_eq!(
            after
                .decisions
                .iter()
                .filter(|d| d.kind == "result_aggregated")
                .count(),
            1
        );
    }
    Ok(())
}

struct UnsafeRetry;
impl RecoveryPolicy for UnsafeRetry {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "fixture.unsafe-retry".into(),
            version: "1".into(),
        }
    }
    fn configuration(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
    fn propose(&self, _: &RecoveryInput) -> Result<RecoveryAction> {
        Ok(RecoveryAction::Retry { delay_ms: 0 })
    }
}
#[tokio::test]
async fn injected_policy_cannot_replay_an_uncertain_writer() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let store = Store::open(&temp.path().join("state"))?;
    let script = Arc::new(Script {
        failure_purpose: "review_plan",
        gate: None,
        mode: Mode::Transient,
        writes: true,
        failures: AtomicUsize::new(1),
        calls: Mutex::new(vec![]),
    });
    let engine =
        engine(store.clone(), script.clone()).with_recovery_policy(Arc::new(UnsafeRetry))?;
    let result = engine.run(&path, "Inspect", None).await?;
    assert_ne!(result.session.status, "completed");
    assert_eq!(script.calls.lock().unwrap().len(), 2);
    let decisions = store.decisions(&result.session.id)?;
    assert!(decisions
        .iter()
        .filter_map(|d| d.links.recovery.as_ref())
        .any(|r| !r.accepted));
    let stage = store
        .recovery_stages(&result.session.id)?
        .into_iter()
        .find(|s| s.purpose == "review_plan")
        .unwrap();
    assert!(!stage
        .manual_actions()
        .controls
        .contains(&RecoveryControlKind::Retry));
    assert!(engine
        .control_recovery(&RecoveryControlCommand {
            session_id: result.session.id,
            stage_id: stage.id,
            expected_revision: stage.revision,
            command_id: new_id(),
            action: RecoveryControl::Retry
        })
        .is_err());
    Ok(())
}
#[tokio::test]
async fn interrupted_review_has_no_invented_termination_evidence_after_restart() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let home = temp.path().join("state");
    let store = Store::open(&home)?;
    let gate = Arc::new(Gate {
        purpose: "review_plan",
        started: tokio::sync::Semaphore::new(0),
        release: tokio::sync::Semaphore::new(0),
    });
    let script = Arc::new(Script {
        failure_purpose: "review_plan",
        gate: Some(gate.clone()),
        mode: Mode::Unknown,
        writes: true,
        failures: AtomicUsize::new(0),
        calls: Mutex::new(vec![]),
    });
    let session = new_id();
    let running = engine(store.clone(), script.clone());
    let task = tokio::spawn({
        let path = path.clone();
        let session = session.clone();
        async move { running.run_identified(&path, "Inspect", &session).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), gate.started.acquire())
        .await??
        .forget();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let before = script.calls.lock().unwrap().len();
    let resumed = engine(Store::open(&home)?, script.clone())
        .run(&path, "Inspect", Some(&session))
        .await?;
    assert_ne!(resumed.session.status, "completed");
    assert_eq!(script.calls.lock().unwrap().len(), before);
    let stage = store
        .recovery_stages(&session)?
        .into_iter()
        .find(|s| s.purpose == "review_plan")
        .unwrap();
    assert_eq!(stage.status, RecoveryStatus::OwnerAction);
    assert_eq!(
        stage.failures[0].termination,
        TerminationEvidence::UnverifiedAfterRestart
    );
    assert_eq!(store.tasks(&session)?.len(), 0);
    assert!(store
        .trace(&session)?
        .invocations
        .iter()
        .all(|i| i.state != InvocationState::Running));
    Ok(())
}
#[tokio::test]
async fn initial_planner_failure_uses_bounded_recovery_and_keeps_its_attribution() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let store = Store::open(&temp.path().join("state"))?;
    let script = Arc::new(Script {
        failure_purpose: "plan",
        gate: None,
        mode: Mode::Transient,
        writes: false,
        failures: AtomicUsize::new(1),
        calls: Mutex::new(vec![]),
    });
    let result = engine(store.clone(), script.clone())
        .run(&path, "Inspect", None)
        .await?;
    assert_eq!(result.session.status, "completed", "{}", result.summary);
    let trace = store.trace(&result.session.id)?;
    assert_eq!(
        script
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.purpose == "plan")
            .count(),
        2
    );
    assert_eq!(
        trace
            .decisions
            .iter()
            .filter(|d| d.kind == "plan_proposed")
            .count(),
        1
    );
    assert!(store
        .recovery_stages(&result.session.id)?
        .iter()
        .any(|s| s.purpose == "plan"
            && s.recovery_attempts == 1
            && s.status == RecoveryStatus::Complete));
    Ok(())
}

#[tokio::test]
async fn owner_pause_drains_current_work_and_survives_resume_without_new_admission() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let store = Store::open(&temp.path().join("state"))?;
    let gate = Arc::new(Gate {
        purpose: "execute",
        started: tokio::sync::Semaphore::new(0),
        release: tokio::sync::Semaphore::new(0),
    });
    let script = Arc::new(Script {
        failure_purpose: "review_plan",
        gate: Some(gate.clone()),
        mode: Mode::Unknown,
        writes: true,
        failures: AtomicUsize::new(0),
        calls: Mutex::new(vec![]),
    });
    let owner = engine(store.clone(), script.clone());
    let session = new_id();
    let background = tokio::spawn({
        let running = owner.clone();
        let path = path.clone();
        let session = session.clone();
        async move { running.run_identified(&path, "Inspect", &session).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), gate.started.acquire())
        .await??
        .forget();
    let view = owner.team_control(&session)?;
    owner.owner_team_command(&team_command(&view, OwnerTeamAction::Pause))?;
    let calls = script.calls.lock().unwrap().len();
    gate.release.add_permits(1);
    let paused = tokio::time::timeout(std::time::Duration::from_secs(10), background).await???;
    assert_eq!(paused.session.status, "paused", "{}", paused.summary);
    assert_eq!(script.calls.lock().unwrap().len(), calls);
    assert!(store
        .tasks(&session)?
        .iter()
        .any(|t| t.state == TaskState::Review && t.result.is_some()));
    let again = engine(store.clone(), script.clone())
        .run(&path, "Inspect", Some(&session))
        .await?;
    assert_eq!(again.session.status, "paused");
    assert_eq!(script.calls.lock().unwrap().len(), calls);
    let view = owner.team_control(&session)?;
    owner.owner_team_command(&team_command(&view, OwnerTeamAction::Continue))?;
    gate.release.add_permits(2);
    let result = engine(store.clone(), script.clone())
        .run(&path, "Inspect", Some(&session))
        .await?;
    assert_eq!(result.session.status, "completed", "{}", result.summary);
    assert_eq!(
        script
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.purpose == "execute")
            .count(),
        2
    );
    assert!(store
        .trace(&session)?
        .invocations
        .iter()
        .all(|i| i.state == InvocationState::Completed));
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_replacement_racing_completion_is_atomic_or_explicitly_stale() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let store = Store::open(&temp.path().join("state"))?;
    let gate = Arc::new(Gate {
        purpose: "execute",
        started: tokio::sync::Semaphore::new(0),
        release: tokio::sync::Semaphore::new(0),
    });
    let script = Arc::new(Script {
        failure_purpose: "review_plan",
        gate: Some(gate.clone()),
        mode: Mode::Unknown,
        writes: true,
        failures: AtomicUsize::new(0),
        calls: Mutex::new(vec![]),
    });
    let mut owner = engine(store.clone(), script.clone());
    owner.config.team_constraints.fixed_roster = Some(vec!["author".into(), "reserve".into()]);
    let session = new_id();
    let background = tokio::spawn({
        let running = owner.clone();
        let path = path.clone();
        let session = session.clone();
        async move { running.run_identified(&path, "Inspect", &session).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), gate.started.acquire())
        .await??
        .forget();
    let before = store.trace(&session)?;
    let producer = before
        .assignments
        .iter()
        .find(|a| a.purpose == "execute")
        .unwrap()
        .agent_id
        .clone();
    let mut command = team_command(
        &owner.team_control(&session)?,
        OwnerTeamAction::Replace {
            agent_id: producer.clone(),
            replacement_id: "reviewer".into(),
        },
    );
    command.revise_pinned_roster = true;
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let change = tokio::task::spawn_blocking({
        let owner = owner.clone();
        let command = command.clone();
        let barrier = barrier.clone();
        move || {
            barrier.wait();
            owner.owner_team_command(&command)
        }
    });
    barrier.wait();
    gate.release.add_permits(2);
    let receipt = change.await?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), background).await???;
    assert!(
        matches!(result.session.status.as_str(), "completed" | "blocked"),
        "{}",
        result.summary
    );
    if let Err(error) = receipt {
        assert!(error.to_string().contains("stale_owner_command"), "{error}");
        command.expected_revision = owner.team_control(&session)?.revision;
        owner.owner_team_command(&command)?;
    }
    let after = store.trace(&session)?;
    let accepted_command = after
        .history
        .iter()
        .find(|e| e.kind == "owner_team_command")
        .unwrap();
    assert!(!after.history.iter().any(|e| e.seq > accepted_command.seq
        && e.data.get("change").and_then(serde_json::Value::as_str) == Some("assignment_started")
        && e.data
            .pointer("/assignment/agent_id")
            .and_then(serde_json::Value::as_str)
            == Some(producer.as_str())));
    assert_eq!(
        serde_json::to_value(after.policy)?,
        serde_json::to_value(before.policy)?
    );
    assert!(owner.team_control(&session)?.pending_departures.is_empty());
    Ok(())
}

#[tokio::test]
async fn sole_remaining_independent_reviewer_can_finish_saved_final_review() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let store = Store::open(&temp.path().join("state"))?;
    let script = Arc::new(Script {
        failure_purpose: "final_review",
        gate: None,
        mode: Mode::Unknown,
        writes: false,
        failures: AtomicUsize::new(1),
        calls: Mutex::new(vec![]),
    });
    let mut owner = engine(store.clone(), script.clone());
    owner.config.team_constraints.fixed_roster = Some(vec!["author".into(), "reserve".into()]);
    let first = owner.run(&path, "Inspect", None).await?;
    assert_ne!(first.session.status, "completed");
    let before = store.trace(&first.session.id)?;
    let producer = before
        .assignments
        .iter()
        .find(|a| a.purpose == "execute")
        .unwrap()
        .agent_id
        .clone();
    let mut command = team_command(
        &owner.team_control(&first.session.id)?,
        OwnerTeamAction::Remove {
            agent_id: producer.clone(),
        },
    );
    command.revise_pinned_roster = true;
    owner.owner_team_command(&command)?;
    let view = owner.team_control(&first.session.id)?;
    assert_eq!(view.effective.current_members.len(), 1);
    assert_ne!(view.effective.current_members[0], producer);
    let stage = store
        .recovery_stages(&first.session.id)?
        .into_iter()
        .find(|s| s.purpose == "final_review")
        .unwrap();
    owner.control_recovery(&RecoveryControlCommand {
        session_id: first.session.id.clone(),
        stage_id: stage.id,
        expected_revision: stage.revision,
        command_id: new_id(),
        action: RecoveryControl::Continue,
    })?;
    let resumed = owner.run(&path, "Inspect", Some(&first.session.id)).await?;
    assert_eq!(resumed.session.status, "completed", "{}", resumed.summary);
    assert_eq!(
        script
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.purpose == "execute")
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn legacy_failed_review_without_stage_binding_cannot_replay_a_writer() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let first_store = Store::open(&temp.path().join("new-format-state"))?;
    let script = Arc::new(Script {
        failure_purpose: "review_plan",
        gate: None,
        mode: Mode::Transient,
        writes: true,
        failures: AtomicUsize::new(1),
        calls: Mutex::new(vec![]),
    });
    let first = engine(first_store.clone(), script.clone())
        .run(&path, "Inspect", None)
        .await?;
    let source = first_store.trace(&first.session.id)?;
    // Reconstruct only records available before YMP-146 in a different temporary home.
    // No new recovery stages, bindings, owner state or failure-classification decisions.
    let legacy = Store::open(&temp.path().join("legacy-state"))?;
    let mut session = source.session.clone();
    session.project_id = legacy.project(&path)?.id;
    session.turns_used = 0;
    legacy.create_session(&session, source.policy.as_ref().unwrap())?;
    for original in &source.invocations {
        let mut assignment = source
            .assignments
            .iter()
            .find(|a| a.id == original.assignment_id)
            .unwrap()
            .clone();
        assignment.grant_ids.clear();
        assignment.state = InvocationState::Running;
        assignment.ended_at = None;
        let mut invocation = original.clone();
        invocation.state = InvocationState::Running;
        invocation.ended_at = None;
        invocation.terminal_reason = None;
        invocation.usage = None;
        legacy.begin_invocation(&assignment, &invocation)?;
        legacy.finish_invocation(
            &session.id,
            &invocation.id,
            original.state,
            Some(original.state.as_str()),
        )?;
    }
    for decision in source.decisions.iter().filter(|d| {
        matches!(
            d.kind.as_str(),
            "plan_proposed"
                | "workspace_access_acquired"
                | "workspace_access_admitted"
                | "workspace_access_released"
        )
    }) {
        legacy.record_decision(decision)?;
    }
    assert!(legacy.recovery_stages(&session.id)?.is_empty());
    let calls = script.calls.lock().unwrap().len();
    let result = engine(legacy.clone(), script.clone())
        .run(&path, "Inspect", Some(&session.id))
        .await?;
    assert_ne!(result.session.status, "completed");
    assert_eq!(
        script.calls.lock().unwrap().len(),
        calls,
        "Missing legacy stage is not evidence permitting write replay"
    );
    let stages = legacy.recovery_stages(&session.id)?;
    assert!(stages
        .iter()
        .any(|s| s.status == RecoveryStatus::OwnerAction
            && s.condition.as_ref().is_some_and(|r| r.contains("legacy"))));
    assert!(legacy.tasks(&session.id)?.is_empty());
    Ok(())
}

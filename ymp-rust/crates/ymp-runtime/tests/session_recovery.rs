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

#[derive(Clone, Copy)]
enum Mode {
    Unknown,
    Transient,
    Auth,
    Negative,
    Malformed,
}
struct Script {
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
            if request.purpose == "review_plan"
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
        mode: Mode::Unknown,
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
    fn configuration(&self) -> serde_json::Value {
        json!({"mode":"owner"})
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
        mode,
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
            stages[0].failures[0].class,
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
        mode: Mode::Unknown,
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

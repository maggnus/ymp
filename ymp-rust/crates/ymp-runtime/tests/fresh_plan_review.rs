//! Native adapter protocol fixtures and genuine historical records with no effect scope.
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_runtime::Engine;
use ymp_storage::Store;

struct Fixture {
    _temp: tempfile::TempDir,
    path: PathBuf,
    log: PathBuf,
    store: Store,
    engine: Engine,
    stage: RecoveryStage,
    original: SessionTrace,
}
impl Fixture {
    async fn new() -> Result<Self> {
        Self::configured(40, false).await
    }
    async fn configured(turns: usize, unknown: bool) -> Result<Self> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("work");
        std::fs::create_dir(&path)?;
        let log = temp.path().join("protocol.jsonl");
        let source = Store::open(&temp.path().join("source"))?;
        let script =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fresh_plan_review.py");
        let config: Config = serde_json::from_value(json!({
            "version":1,
            "providers":[
                {"id":"author-native","kind":"codex","command":"python3","args":[script,"codex","author",log,"native-v1"]},
                {"id":"failed-native","kind":"acp","command":"python3","args":[script,"acp","failed",log,"native-v1"]},
                {"id":"fresh-native","kind":"codex","command":"python3","args":[script,"codex","fresh",log,"native-v1"]}
            ],
            "agents":[{"id":"author","name":"Author","provider":"author-native"},{"id":"failed","name":"Failed","provider":"failed-native"},{"id":"fresh","name":"Fresh","provider":"fresh-native"}],
            "team":["author","failed"],"team_constraints":{"fixed_roster":["author","failed"]},"limits":{"parallel":1,"turns":turns,"turn_timeout_secs":5,"attempts":3,"resources":{"startup_invocations":10}}
        }))?;
        let make_engine = |store: Store| -> Result<Engine> {
            let (events, _) = mpsc::unbounded_channel();
            let mut engine = Engine::new(store, config.clone(), events, CancellationToken::new())?;
            engine.use_memory = false;
            Ok(engine)
        };
        let first = make_engine(source.clone())?
            .run(&path, "Inspect the directory", None)
            .await?;
        assert_ne!(first.session.status, "completed", "{}", first.summary);
        let original = source.trace(&first.session.id)?;
        assert_eq!(
            original.invocations.len(),
            2,
            "{}; calls={:?}; protocol={}",
            first.summary,
            original
                .assignments
                .iter()
                .map(|a| (&a.agent_id, &a.purpose))
                .collect::<Vec<_>>(),
            std::fs::read_to_string(&log)?
        );
        assert!(original.tasks.is_empty());
        let failed = original
            .assignments
            .iter()
            .find(|a| a.purpose == "review_plan")
            .unwrap();
        assert_eq!(failed.agent_id, "failed");
        let store = Store::open(&temp.path().join("legacy"))?;
        let mut session = original.session.clone();
        session.project_id = store.project(&path)?.id;
        session.turns_used = 0;
        store.create_session(&session, original.policy.as_ref().unwrap())?;
        store.put_value(
            &format!("team_state:v1:{}", session.id),
            &serde_json::to_value(&original.team_state)?,
        )?;
        store.put_value(
            &format!("prompt:{}", session.id),
            &json!("Inspect the directory"),
        )?;
        for invocation in &original.invocations {
            let mut assignment = original
                .assignments
                .iter()
                .find(|a| a.id == invocation.assignment_id)
                .unwrap()
                .clone();
            assignment.state = InvocationState::Running;
            assignment.ended_at = None;
            assignment.grant_ids.clear();
            let mut opened = invocation.clone();
            opened.state = InvocationState::Running;
            opened.ended_at = None;
            opened.usage = None;
            opened.terminal_reason = None;
            store.begin_invocation(&assignment, &opened)?;
            store.finish_invocation(
                &session.id,
                &opened.id,
                if unknown && assignment.purpose == "review_plan" {
                    InvocationState::Interrupted
                } else {
                    invocation.state
                },
                Some(invocation.state.as_str()),
            )?;
        }
        for record in original.decisions.iter().filter(|d| {
            matches!(
                d.kind.as_str(),
                "plan_proposed"
                    | "workspace_access_acquired"
                    | "workspace_access_admitted"
                    | "workspace_access_released"
            )
        }) {
            assert!(record
                .links
                .workspace_access
                .as_ref()
                .is_none_or(|a| a.local_effect_scope.is_none()));
            store.record_decision(record)?;
        }
        let plan = original
            .decisions
            .iter()
            .find_map(|d| {
                (d.kind == "plan_proposed")
                    .then_some(d.links.plan_proposal.as_ref())
                    .flatten()
            })
            .unwrap();
        store.invocation_message(
            &session.id,
            &plan.producer_invocation_id,
            "plan",
            &serde_json::to_string(&plan.plan)?,
        )?;
        let mut engine = make_engine(store.clone())?;
        ymp_providers::discovery::refresh_catalog(
            &mut engine.config,
            &temp.path().join("native-catalog"),
            &path,
            &PathBuf::new(),
            ymp_providers::discovery::ScanOptions {
                provider: Some("fresh-native".into()),
                timeout_secs: 5,
            },
            CancellationToken::new(),
        )
        .await?;
        let model = engine
            .config
            .native_provider_snapshot("fresh-native")
            .unwrap()
            .catalog
            .as_ref()
            .unwrap()
            .default_model
            .clone()
            .unwrap();
        engine
            .config
            .agents
            .iter_mut()
            .find(|a| a.id == "fresh")
            .unwrap()
            .model = Some(model);
        let before = std::fs::read(&log)?;
        assert_ne!(
            engine
                .run(&path, "", Some(&session.id))
                .await?
                .session
                .status,
            "completed"
        );
        assert_eq!(std::fs::read(&log)?, before);
        let stage = engine
            .recovery_stages(&session.id)?
            .into_iter()
            .find(|s| s.purpose == "review_plan")
            .unwrap();
        assert_eq!(
            stage.failures[0].effective_access,
            WorkspaceAccess::WriteAll
        );
        assert_eq!(
            stage.failures[0].termination,
            if unknown {
                TerminationEvidence::UnverifiedAfterRestart
            } else {
                TerminationEvidence::BackendEnded
            }
        );
        Ok(Self {
            _temp: temp,
            path,
            log,
            store,
            engine,
            stage,
            original,
        })
    }
    fn owner(&self, action: OwnerTeamAction) -> Result<()> {
        let view = self.engine.team_control(&self.stage.session_id)?;
        self.engine.owner_team_command(&OwnerTeamCommand {
            session_id: view.session_id,
            expected_revision: view.revision,
            command_id: new_id(),
            revise_pinned_roster: true,
            action,
        })?;
        Ok(())
    }
    fn command(&self) -> Result<FreshPlanReviewCommand> {
        let stage = self
            .engine
            .recovery_stages(&self.stage.session_id)?
            .into_iter()
            .find(|s| s.id == self.stage.id)
            .unwrap();
        Ok(FreshPlanReviewCommand {
            session_id: stage.session_id,
            stage_id: stage.id,
            expected_revision: stage.revision,
            command_id: new_id(),
            proposal: stage.plan.unwrap(),
            reviewer_id: "fresh".into(),
        })
    }
    async fn warm_fresh_context(&mut self) -> Result<()> {
        self.owner(OwnerTeamAction::Add {
            agent_id: "fresh".into(),
        })?;
        self.owner(OwnerTeamAction::Remove {
            agent_id: "failed".into(),
        })?;
        self.engine.config.team = vec!["fresh".into(), "author".into()];
        self.engine
            .follow_up(
                &self.path,
                "Give the current status only",
                &self.stage.session_id,
            )
            .await?;
        assert!(self
            .requests()?
            .iter()
            .any(|r| r["actor"] == "fresh" && r["purpose"] == "conversation"));
        Ok(())
    }
    fn control(&self, action: RecoveryControl) -> Result<RecoveryControlReceipt> {
        let command = self.command()?;
        self.engine.control_recovery(&RecoveryControlCommand {
            session_id: command.session_id,
            stage_id: command.stage_id,
            expected_revision: command.expected_revision,
            command_id: new_id(),
            action,
        })
    }
    fn requests(&self) -> Result<Vec<Value>> {
        std::fs::read_to_string(&self.log)?
            .lines()
            .map(|line| Ok(serde_json::from_str(line)?))
            .collect()
    }
}

#[tokio::test]
async fn native_legacy_failure_has_no_scope_or_automatic_replay() -> Result<()> {
    let f = Fixture::new().await?;
    assert!(f.stage.effect_resolution.is_none());
    assert!(f.store.tasks(&f.stage.session_id)?.is_empty());
    assert_eq!(f.original.invocations.len(), 2);
    assert!(f.requests()?.iter().all(|r| r["purpose"] != "execute"));
    assert_ne!(
        f.engine
            .run(&f.path, "", Some(&f.stage.session_id))
            .await?
            .session
            .status,
        "completed"
    );
    Ok(())
}

#[tokio::test]
async fn fresh_native_review_is_plan_only_and_continuation_preserves_unknown_dependencies(
) -> Result<()> {
    let mut f = Fixture::new().await?;
    f.warm_fresh_context().await?;
    let original = f.store.trace(&f.stage.session_id)?;
    let before = f.requests()?.len();
    let command = f.command()?;
    let receipt = f.engine.review_saved_plan_fresh(&command).await?;
    assert!(receipt.record.approved);
    assert_eq!(receipt.record.prior_failures, f.stage.failures);
    assert_eq!(
        receipt.record.command.proposal,
        f.stage.plan.clone().unwrap()
    );
    assert_eq!(
        receipt.next_action,
        FreshPlanReviewNextAction::AwaitOwnerContinuation
    );
    assert_eq!(receipt.record.response.agent_id, "fresh");
    let requests = f.requests()?;
    assert_eq!(
        requests[before..]
            .iter()
            .filter(|r| r["purpose"] == "review_plan")
            .count(),
        1
    );
    assert_eq!(
        requests[before..]
            .iter()
            .filter(|r| r["method"] == "thread/start" && r["sandbox"] == "read-only")
            .count(),
        1
    );
    assert!(!requests[before..]
        .iter()
        .any(|r| r["method"] == "thread/resume"
            || r["method"] == "session/load"
            || r["purpose"] == "plan"
            || r["purpose"] == "execute"));
    let observed = f
        .store
        .invocation(&f.stage.session_id, &receipt.record.response.invocation_id)?;
    assert!(observed.resumed_from.is_none());
    let after = f.store.trace(&f.stage.session_id)?;
    assert_eq!(after.invocations.len(), original.invocations.len() + 1);
    assert!(after.tasks.is_empty());
    assert_eq!(
        after.budget.as_ref().unwrap().limits,
        original.budget.as_ref().unwrap().limits
    );
    assert!(after
        .decisions
        .iter()
        .filter_map(|d| d.links.workspace_access.as_ref())
        .all(|a| a.local_effect_scope.is_none()));
    assert!(after
        .decisions
        .iter()
        .all(|d| d.links.recovery_inspection.is_none()));
    assert!(f
        .engine
        .recovery_stages(&f.stage.session_id)?
        .iter()
        .all(|s| s.effect_resolution.is_none()));
    assert_eq!(f.engine.review_saved_plan_fresh(&command).await?, receipt);
    assert_eq!(f.requests()?, requests);
    assert_ne!(
        f.engine
            .run(&f.path, "", Some(&f.stage.session_id))
            .await?
            .session
            .status,
        "completed"
    );
    assert!(f.store.tasks(&f.stage.session_id)?.is_empty());
    assert_eq!(f.requests()?, requests);
    let continue_command = RecoveryControlCommand {
        session_id: f.stage.session_id.clone(),
        stage_id: f.stage.id.clone(),
        expected_revision: receipt.resulting_revision,
        command_id: new_id(),
        action: RecoveryControl::Continue,
    };
    let mut retry = continue_command.clone();
    retry.command_id = new_id();
    retry.action = RecoveryControl::Retry;
    assert!(f
        .engine
        .control_recovery(&retry)
        .unwrap_err()
        .to_string()
        .contains("uncertain_effects"));
    f.engine.control_recovery(&continue_command)?;
    assert!(
        !f.engine
            .recovery_stages(&f.stage.session_id)?
            .into_iter()
            .find(|s| s.id == f.stage.id)
            .unwrap()
            .manual_permit
    );
    let result = f.engine.run(&f.path, "", Some(&f.stage.session_id)).await?;
    assert_ne!(result.session.status, "completed");
    assert!(
        result.summary.contains("unresolved_effect_dependencies"),
        "{}",
        result.summary
    );
    let final_trace = f.store.trace(&f.stage.session_id)?;
    assert_eq!(final_trace.tasks.len(), 1);
    assert!(final_trace
        .tasks
        .iter()
        .all(|t| t.state == TaskState::Ready && t.attempts == 0));
    assert_eq!(f.requests()?, requests);
    assert_eq!(
        final_trace
            .decisions
            .iter()
            .filter(|d| d.kind == "plan_review")
            .count(),
        1
    );
    assert!(final_trace.decisions.iter().any(|d| d
        .links
        .unresolved_effect_dependencies
        .as_ref()
        .is_some_and(|b| b.proposal == command.proposal
            && b.invocation_ids == vec![f.stage.failures[0].invocation_id.clone()])));
    for invocation in original.invocations {
        assert_eq!(
            serde_json::to_value(f.store.invocation(&f.stage.session_id, &invocation.id)?)?,
            serde_json::to_value(invocation)?
        );
    }
    assert!(f.store.observations()?.is_empty());
    Ok(())
}

#[tokio::test]
async fn native_fresh_review_owner_holds_have_a_reachable_nonretry_release() -> Result<()> {
    let f = Fixture::new().await?;
    f.owner(OwnerTeamAction::Add {
        agent_id: "fresh".into(),
    })?;
    let before = f.requests()?;
    for action in [
        RecoveryControl::Wait {
            condition: "Owner waits for inspection".into(),
        },
        RecoveryControl::Pause,
    ] {
        f.control(action)?;
        assert!(f
            .engine
            .review_saved_plan_fresh(&f.command()?)
            .await
            .unwrap_err()
            .to_string()
            .contains("owner_hold"));
        assert!(f
            .control(RecoveryControl::Continue)
            .unwrap_err()
            .to_string()
            .contains("uncertain_effects"));
        f.control(RecoveryControl::ReleaseHold)?;
        let released = f
            .engine
            .recovery_stages(&f.stage.session_id)?
            .into_iter()
            .find(|s| s.id == f.stage.id)
            .unwrap();
        assert_eq!(released.status, f.stage.status);
        assert_eq!(released.wait_reason, f.stage.wait_reason);
        assert_eq!(released.condition, f.stage.condition);
        assert_eq!(released.failures, f.stage.failures);
        assert!(!released.manual_permit);
        assert!(released.effect_resolution.is_none());
        assert_eq!(f.requests()?, before);
    }
    f.owner(OwnerTeamAction::Pause)?;
    assert!(f
        .engine
        .review_saved_plan_fresh(&f.command()?)
        .await
        .unwrap_err()
        .to_string()
        .contains("owner_paused"));
    f.owner(OwnerTeamAction::Continue)?;
    assert_eq!(f.requests()?, before);
    assert!(
        f.engine
            .review_saved_plan_fresh(&f.command()?)
            .await?
            .record
            .approved
    );
    Ok(())
}

#[tokio::test]
async fn native_fresh_review_rejects_foreign_stale_self_unknown_and_exhausted_inputs() -> Result<()>
{
    let f = Fixture::new().await?;
    f.owner(OwnerTeamAction::Add {
        agent_id: "fresh".into(),
    })?;
    let before = f.requests()?;
    let good = f.command()?;
    for mutation in 0..5 {
        let mut command = good.clone();
        match mutation {
            0 => command.session_id = new_id(),
            1 => command.expected_revision += 1,
            2 => command.proposal.plan.summary.push_str(" changed"),
            3 => command.reviewer_id = "author".into(),
            _ => command.reviewer_id = "failed".into(),
        }
        assert!(f.engine.review_saved_plan_fresh(&command).await.is_err());
    }
    assert_eq!(f.requests()?, before);
    let receipt = f.engine.review_saved_plan_fresh(&good).await?;
    let mut conflict = good.clone();
    conflict.reviewer_id = "author".into();
    assert!(f
        .engine
        .review_saved_plan_fresh(&conflict)
        .await
        .unwrap_err()
        .to_string()
        .contains("owner_command_conflict"));
    assert_eq!(
        Store::open(&f.store.home)?.fresh_plan_review_receipt(&good)?,
        Some(receipt)
    );
    for (turns, unknown) in [(40, true), (4, false)] {
        let f = Fixture::configured(turns, unknown).await?;
        f.owner(OwnerTeamAction::Add {
            agent_id: "fresh".into(),
        })?;
        let calls = f.requests()?;
        assert!(f
            .engine
            .review_saved_plan_fresh(&f.command()?)
            .await
            .is_err());
        assert_eq!(f.requests()?, calls);
        assert!(f.store.tasks(&f.stage.session_id)?.is_empty());
    }
    Ok(())
}

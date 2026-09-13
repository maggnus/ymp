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

fn reopen(f: &Fixture) -> Result<Engine> {
    let (events, _) = mpsc::unbounded_channel();
    let mut engine = Engine::new(
        Store::open(&f.store.home)?,
        f.engine.config.clone(),
        events,
        CancellationToken::new(),
    )?;
    engine.use_memory = false;
    Ok(engine)
}
fn current_command(f: &Fixture) -> Result<ContinueWithCurrentFilesCommand> {
    Ok(ContinueWithCurrentFilesCommand {
        command_id: new_id(),
        context: f
            .engine
            .current_files_context(&f.stage.session_id, &f.stage.id)?,
    })
}

#[tokio::test]
async fn current_files_owner_decision_survives_restart_and_finishes_native_legacy_work(
) -> Result<()> {
    let mut f = Fixture::new().await?;
    f.warm_fresh_context().await?;
    let reviewed = f.engine.review_saved_plan_fresh(&f.command()?).await?;
    let before = f.store.trace(&f.stage.session_id)?;
    let native_before = f.requests()?;
    let command = current_command(&f)?;
    let receipt = f.engine.continue_with_current_files(&command)?;
    assert_eq!(
        f.requests()?,
        native_before,
        "Authorization itself invokes no backend"
    );
    assert_eq!(f.engine.continue_with_current_files(&command)?, receipt);
    let engine = reopen(&f)?;
    assert_eq!(engine.continue_with_current_files(&command)?, receipt);
    let stage = engine
        .recovery_stages(&f.stage.session_id)?
        .into_iter()
        .find(|s| s.id == f.stage.id)
        .unwrap();
    assert!(!stage.manual_permit);
    assert!(stage.effect_resolution.is_none());
    assert_eq!(stage.failures, f.stage.failures);
    let result = engine.run(&f.path, "", Some(&f.stage.session_id)).await?;
    assert_eq!(result.session.status, "completed", "{}", result.summary);
    assert_eq!(
        std::fs::read_to_string(f.path.join("current-result.txt"))?,
        "New work from current files; historical effects remain unknown.\n"
    );
    let after = f.store.trace(&f.stage.session_id)?;
    assert!(after.tasks.iter().all(|t| t.state == TaskState::Accepted));
    assert_eq!(after.tasks.len(), 1);
    assert_eq!(
        after.budget.as_ref().unwrap().limits,
        before.budget.as_ref().unwrap().limits
    );
    assert!(
        after.budget.as_ref().unwrap().admitted_invocations
            > before.budget.as_ref().unwrap().admitted_invocations
    );
    assert_eq!(
        after
            .decisions
            .iter()
            .filter(|d| d.kind == "plan_proposed")
            .count(),
        1
    );
    assert_eq!(
        after
            .decisions
            .iter()
            .filter(|d| d.kind == "plan_review")
            .count(),
        1
    );
    assert_eq!(
        after
            .decisions
            .iter()
            .filter(|d| d.kind == "owner_current_files_authorized")
            .count(),
        1
    );
    assert!(after
        .decisions
        .iter()
        .filter_map(|d| d.links.workspace_access.as_ref())
        .all(|a| a.local_effect_scope.is_none()));
    assert!(engine
        .recovery_stages(&f.stage.session_id)?
        .iter()
        .all(|s| s.effect_resolution.is_none()));
    assert_eq!(
        engine
            .recovery_stages(&f.stage.session_id)?
            .into_iter()
            .find(|s| s.id == f.stage.id)
            .unwrap()
            .failures,
        f.stage.failures
    );
    assert!(after
        .assignments
        .iter()
        .any(|a| a.purpose == "execute" && a.agent_id != f.stage.failures[0].agent_id));
    assert!(f.requests()?[native_before.len()..]
        .iter()
        .all(|r| r["actor"] != "failed" && r["purpose"] != "plan"));
    assert_eq!(
        f.requests()?
            .iter()
            .filter(|r| r["purpose"] == "execute")
            .count(),
        1
    );
    for invocation in before.invocations {
        assert_eq!(
            serde_json::to_value(f.store.invocation(&f.stage.session_id, &invocation.id)?)?,
            serde_json::to_value(invocation)?
        );
    }
    assert!(after
        .decisions
        .iter()
        .any(|d| d.id == reviewed.review_record_id));
    assert!(f.store.observations()?.is_empty());
    assert_eq!(reopen(&f)?.continue_with_current_files(&command)?, receipt);
    Ok(())
}

#[tokio::test]
async fn current_files_authorization_rejects_changed_context_holds_and_unknown_termination(
) -> Result<()> {
    let f = Fixture::new().await?;
    let initial_calls = f.requests()?;
    let original = current_command(&f)?;
    for mutation in 0..4 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.context.session_id = new_id(),
            1 => changed.context.stage_revision += 1,
            2 => changed.context.failures.clear(),
            _ => changed.context.team_revision += 1,
        }
        assert!(f.engine.continue_with_current_files(&changed).is_err());
    }
    std::fs::write(
        f.path.join("new-file.txt"),
        "Changed while owner was reviewing",
    )?;
    assert!(f
        .engine
        .continue_with_current_files(&original)
        .unwrap_err()
        .to_string()
        .contains("stale_current_files"));
    assert!(f
        .store
        .current_files_authorizations(&f.stage.session_id)?
        .is_empty());
    for hold in [
        RecoveryControl::Pause,
        RecoveryControl::Wait {
            condition: "Owner hold".into(),
        },
    ] {
        f.control(hold)?;
        assert!(f
            .engine
            .continue_with_current_files(&current_command(&f)?)
            .unwrap_err()
            .to_string()
            .contains("owner_hold"));
        f.control(RecoveryControl::ReleaseHold)?;
        assert!(
            !f.engine
                .recovery_stages(&f.stage.session_id)?
                .into_iter()
                .find(|s| s.id == f.stage.id)
                .unwrap()
                .manual_permit
        );
    }
    f.owner(OwnerTeamAction::Pause)?;
    assert!(f
        .engine
        .continue_with_current_files(&current_command(&f)?)
        .is_err());
    f.owner(OwnerTeamAction::Continue)?;
    assert_eq!(f.requests()?, initial_calls);
    let command = current_command(&f)?;
    f.engine.continue_with_current_files(&command)?;
    let mut conflict = command.clone();
    conflict.context.failures.clear();
    assert!(f
        .engine
        .continue_with_current_files(&conflict)
        .unwrap_err()
        .to_string()
        .contains("owner_command_conflict"));
    assert!(f
        .control(RecoveryControl::Retry)
        .unwrap_err()
        .to_string()
        .contains("uncertain_effects"));
    let unknown = Fixture::configured(40, true).await?;
    assert!(unknown
        .engine
        .continue_with_current_files(&current_command(&unknown)?)
        .is_err());
    assert!(unknown
        .store
        .current_files_authorizations(&unknown.stage.session_id)?
        .is_empty());
    Ok(())
}

#[tokio::test]
async fn a_later_uncertain_failure_is_not_covered_by_the_previous_current_files_decision(
) -> Result<()> {
    let mut f = Fixture::new().await?;
    f.warm_fresh_context().await?;
    f.engine.review_saved_plan_fresh(&f.command()?).await?;
    let old = current_command(&f)?;
    let old_receipt = f.engine.continue_with_current_files(&old)?;
    std::fs::write(
        f.log.parent().unwrap().join("control.json"),
        json!({"fail_execute":true}).to_string(),
    )?;
    let result = f.engine.run(&f.path, "", Some(&f.stage.session_id)).await?;
    assert_ne!(result.session.status, "completed");
    assert!(
        f.path.join("current-result.txt").exists(),
        "The new failed call had a real local effect"
    );
    let after_failure = f.requests()?;
    let failures = f.store.current_files_failures(&f.stage.session_id)?;
    assert!(failures
        .iter()
        .any(|failure| !old.context.failures.contains(failure)));
    assert_eq!(reopen(&f)?.continue_with_current_files(&old)?, old_receipt);
    let resumed = reopen(&f)?
        .run(&f.path, "", Some(&f.stage.session_id))
        .await?;
    assert_ne!(resumed.session.status, "completed");
    assert!(
        resumed.summary.contains("unresolved_effect_dependencies"),
        "{}",
        resumed.summary
    );
    assert_eq!(
        f.requests()?,
        after_failure,
        "Old owner decision cannot authorize another call after new uncertainty"
    );
    assert_eq!(
        f.store
            .current_files_authorizations(&f.stage.session_id)?
            .len(),
        1
    );
    assert!(f
        .store
        .tasks(&f.stage.session_id)?
        .iter()
        .any(|t| t.state == TaskState::Running));
    Ok(())
}

#[tokio::test]
async fn current_files_authorization_cannot_enlarge_the_original_budget() -> Result<()> {
    let f = Fixture::configured(5, false).await?;
    f.owner(OwnerTeamAction::Add {
        agent_id: "fresh".into(),
    })?;
    f.engine.review_saved_plan_fresh(&f.command()?).await?;
    let before = f.store.session_budget(&f.stage.session_id)?.unwrap();
    let calls = f.requests()?;
    f.engine
        .continue_with_current_files(&current_command(&f)?)?;
    let result = f.engine.run(&f.path, "", Some(&f.stage.session_id)).await?;
    assert_ne!(result.session.status, "completed");
    let after = f.store.session_budget(&f.stage.session_id)?.unwrap();
    assert!(after.last_denial.is_some(), "{}", result.summary);
    assert_eq!(after.limits, before.limits);
    assert_eq!(after.admitted_invocations, before.admitted_invocations);
    assert_eq!(f.requests()?, calls);
    assert!(!f.path.join("current-result.txt").exists());
    Ok(())
}

#[tokio::test]
async fn native_fresh_negative_verdict_and_malformed_attempt_cannot_be_shopped_or_duplicated(
) -> Result<()> {
    for malformed in [false, true] {
        let f = Fixture::new().await?;
        f.owner(OwnerTeamAction::Add {
            agent_id: "fresh".into(),
        })?;
        std::fs::write(
            f.log.parent().unwrap().join("control.json"),
            json!({"approved":false,"malformed":malformed}).to_string(),
        )?;
        let command = f.command()?;
        let result = f.engine.review_saved_plan_fresh(&command).await;
        let calls = f.requests()?;
        if malformed {
            assert!(result.is_err());
            assert!(reopen(&f)?
                .review_saved_plan_fresh(&command)
                .await
                .unwrap_err()
                .to_string()
                .contains("fresh_review_already_attempted"));
        } else {
            let receipt = result?;
            assert!(!receipt.record.approved);
            assert!(receipt.record.reason.contains("preserves this objection"));
            assert_eq!(
                reopen(&f)?.review_saved_plan_fresh(&command).await?,
                receipt
            );
            assert!(f
                .engine
                .review_saved_plan_fresh(&f.command()?)
                .await
                .is_err());
            f.control(RecoveryControl::Continue)?;
            assert_ne!(
                f.engine
                    .run(&f.path, "", Some(&f.stage.session_id))
                    .await?
                    .session
                    .status,
                "completed"
            );
            let verdicts = f
                .store
                .decisions(&f.stage.session_id)?
                .into_iter()
                .filter(|d| d.kind == "plan_review")
                .collect::<Vec<_>>();
            assert_eq!(verdicts.len(), 1);
            assert_eq!(verdicts[0].outcome, Some(DecisionOutcome::Rejected));
        }
        assert_eq!(f.requests()?, calls);
    }
    Ok(())
}

struct ChangedNativeBackend;
impl ymp_providers::ExecutionBackend for ChangedNativeBackend {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "fixture.new-native-build".into(),
            version: "2".into(),
        }
    }
    fn workspace_access(&self, request: &ymp_providers::TurnRequest) -> WorkspaceAccess {
        ymp_providers::ExecutionBackend::workspace_access(
            &ymp_providers::NativeExecutionBackend,
            request,
        )
    }
    fn execute(
        &self,
        request: ymp_providers::TurnRequest,
        events: mpsc::UnboundedSender<ymp_providers::ProviderEvent>,
    ) -> ymp_providers::ExecutionFuture<'_> {
        ymp_providers::ExecutionBackend::execute(
            &ymp_providers::NativeExecutionBackend,
            request,
            events,
        )
    }
}
#[tokio::test]
async fn native_backend_version_change_preserves_old_origin_and_starts_fresh() -> Result<()> {
    let mut f = Fixture::new().await?;
    f.warm_fresh_context().await?;
    let previous = f.store.trace(&f.stage.session_id)?;
    f.engine = f
        .engine
        .with_execution_backend(std::sync::Arc::new(ChangedNativeBackend))?;
    let before = f.requests()?.len();
    let receipt = f.engine.review_saved_plan_fresh(&f.command()?).await?;
    let invocation = f
        .store
        .invocation(&f.stage.session_id, &receipt.record.response.invocation_id)?;
    assert_eq!(invocation.execution_backend.as_ref().unwrap().version, "2");
    assert!(invocation.resumed_from.is_none());
    assert!(!f.requests()?[before..]
        .iter()
        .any(|r| r["method"] == "thread/resume"));
    for old in previous.invocations {
        assert_eq!(
            serde_json::to_value(f.store.invocation(&f.stage.session_id, &old.id)?)?,
            serde_json::to_value(old)?
        );
    }
    Ok(())
}

#[tokio::test]
async fn fresh_acp_reviewer_is_refused_before_native_call_despite_requested_read_only() -> Result<()>
{
    let mut f = Fixture::new().await?;
    let mut unsafe_agent = f
        .engine
        .config
        .agents
        .iter()
        .find(|a| a.id == "failed")
        .unwrap()
        .clone();
    unsafe_agent.id = "unsafe-reviewer".into();
    unsafe_agent.model = Some("fixture-model".into());
    f.engine.config.agents.push(unsafe_agent);
    ymp_providers::discovery::refresh_catalog(
        &mut f.engine.config,
        &f._temp.path().join("acp-catalog"),
        &f.path,
        &PathBuf::new(),
        ymp_providers::discovery::ScanOptions {
            provider: Some("failed-native".into()),
            timeout_secs: 5,
        },
        CancellationToken::new(),
    )
    .await?;
    f.owner(OwnerTeamAction::Add {
        agent_id: "unsafe-reviewer".into(),
    })?;
    let calls = f.requests()?;
    let mut command = f.command()?;
    command.reviewer_id = "unsafe-reviewer".into();
    let error = f
        .engine
        .review_saved_plan_fresh(&command)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("inspection_access"), "{error}");
    assert_eq!(f.requests()?, calls);
    Ok(())
}

#[tokio::test]
async fn current_files_decision_refuses_active_native_execution_and_unreleased_ownership(
) -> Result<()> {
    let f = Fixture::new().await?;
    f.owner(OwnerTeamAction::Add {
        agent_id: "fresh".into(),
    })?;
    let controls = f.log.parent().unwrap();
    std::fs::write(
        controls.join("control.json"),
        json!({"gate_review":true}).to_string(),
    )?;
    let engine = f.engine.clone();
    let command = f.command()?;
    let running = tokio::spawn(async move { engine.review_saved_plan_fresh(&command).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !controls.join("review-started").exists() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await?;
    let live = current_command(&f)?;
    assert!(f.engine.continue_with_current_files(&live).is_err());
    assert!(f
        .store
        .authorize_current_files(&live)
        .unwrap_err()
        .to_string()
        .contains("active_responsibility"));
    assert!(f
        .store
        .current_files_authorizations(&f.stage.session_id)?
        .is_empty());
    assert!(f
        .store
        .active_responsibilities(&f.stage.session_id)?
        .iter()
        .any(|r| r.kind == "workspace_access"));
    std::fs::write(controls.join("review-release"), "release")?;
    running.await??;
    assert!(f
        .engine
        .continue_with_current_files(&current_command(&f)?)
        .is_ok());
    Ok(())
}

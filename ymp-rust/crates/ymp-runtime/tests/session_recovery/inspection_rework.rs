//! R3: real trusted local-effect capture, independent inspection and safe continuation.
use super::*;

struct LocalEffectBackend {
    script: Arc<Script>,
    declare_scope: bool,
}
impl ExecutionBackend for LocalEffectBackend {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "fixture.enforced-local-effect".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        if request.purpose == "review_plan" {
            WorkspaceAccess::WriteAll
        } else {
            WorkspaceAccess::ReadAll
        }
    }
    fn local_effect_scope(&self, request: &TurnRequest) -> Option<LocalEffectScope> {
        // This compiled implementation has exactly one possible write. It runs
        // no shell, native provider, network client or detached work.
        (self.declare_scope && request.purpose == "review_plan").then(|| LocalEffectScope {
            files: vec!["review-side-effect.txt".into()],
        })
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            if request.purpose == "review_plan" {
                std::fs::write(
                    request.cwd.join("review-side-effect.txt"),
                    "known local fixture effect",
                )?;
            }
            self.script.execute(request, events).await
        })
    }
}
struct Fixture {
    _temp: tempfile::TempDir,
    path: std::path::PathBuf,
    store: Store,
    engine: Engine,
    script: Arc<Script>,
    original: SessionTrace,
    stage: RecoveryStage,
}
impl Fixture {
    async fn new(scope: bool, unverified: bool, gate: Option<Arc<Gate>>) -> Result<Self> {
        Self::with_limit(scope, unverified, gate, 80).await
    }
    async fn with_limit(
        scope: bool,
        unverified: bool,
        gate: Option<Arc<Gate>>,
        turns: usize,
    ) -> Result<Self> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("work");
        std::fs::create_dir(&path)?;
        let source = Store::open(&temp.path().join("source"))?;
        let failing = Arc::new(Script {
            failure_purpose: "review_plan",
            gate: None,
            mode: Mode::Unknown,
            writes: false,
            failures: AtomicUsize::new(1),
            calls: Mutex::new(vec![]),
        });
        let mut initial_engine = engine(source.clone(), failing.clone());
        initial_engine.config.limits.turns = turns;
        let initial = initial_engine
            .with_execution_backend(Arc::new(LocalEffectBackend {
                script: failing.clone(),
                declare_scope: scope,
            }))?
            .run(&path, "Inspect this directory", None)
            .await?;
        assert_ne!(initial.session.status, "completed");
        assert_eq!(failing.calls.lock().unwrap().len(), 2);
        let original = source.trace(&initial.session.id)?;
        // Replay the public historical format, as in the independent review.
        // No safety/resolution records are fabricated. Admitted scope, when
        // present, comes unchanged from actual execution backend enforcement.
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
            &json!("Inspect this directory"),
        )?;
        for invocation in &original.invocations {
            let mut assignment = original
                .assignments
                .iter()
                .find(|a| a.id == invocation.assignment_id)
                .unwrap()
                .clone();
            assignment.grant_ids.clear();
            assignment.state = InvocationState::Running;
            assignment.ended_at = None;
            let mut opened = invocation.clone();
            opened.state = InvocationState::Running;
            opened.ended_at = None;
            opened.terminal_reason = None;
            opened.usage = None;
            store.begin_invocation(&assignment, &opened)?;
            let terminal = if unverified && assignment.purpose == "review_plan" {
                InvocationState::Interrupted
            } else {
                invocation.state
            };
            store.finish_invocation(&session.id, &opened.id, terminal, Some(terminal.as_str()))?;
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
        assert!(store.recovery_stages(&session.id)?.is_empty());
        let script = Arc::new(Script {
            failure_purpose: "never",
            gate,
            mode: Mode::Unknown,
            writes: false,
            failures: AtomicUsize::new(0),
            calls: Mutex::new(vec![]),
        });
        let engine = engine(store.clone(), script.clone());
        assert_ne!(
            engine
                .run(&path, "", Some(&session.id))
                .await?
                .session
                .status,
            "completed"
        );
        assert!(script.calls.lock().unwrap().is_empty());
        let stage = engine
            .recovery_stages(&session.id)?
            .into_iter()
            .find(|s| s.purpose == "review_plan")
            .unwrap();
        Ok(Self {
            _temp: temp,
            path,
            store,
            engine,
            script,
            original,
            stage,
        })
    }
    fn command(&self) -> Result<RecoveryInspectionCommand> {
        let stage = self.current()?;
        Ok(RecoveryInspectionCommand {
            session_id: stage.session_id,
            stage_id: stage.id,
            expected_revision: stage.revision,
            command_id: new_id(),
        })
    }
    fn current(&self) -> Result<RecoveryStage> {
        Ok(self
            .engine
            .recovery_stages(&self.stage.session_id)?
            .into_iter()
            .find(|s| s.id == self.stage.id)
            .unwrap())
    }
    fn control(&self, action: RecoveryControl) -> Result<RecoveryControlReceipt> {
        let stage = self.current()?;
        self.engine.control_recovery(&RecoveryControlCommand {
            session_id: stage.session_id,
            stage_id: stage.id,
            expected_revision: stage.revision,
            command_id: new_id(),
            action,
        })
    }
}

#[tokio::test]
async fn r3_bound_local_inspection_preserves_uncertainty_and_enables_explicit_continuation(
) -> Result<()> {
    for hold in [
        RecoveryControl::Pause,
        RecoveryControl::Wait {
            condition: "Owner is inspecting the proposed work".into(),
        },
    ] {
        let f = Fixture::new(true, false, None).await?;
        assert!(f
            .control(RecoveryControl::Continue)
            .unwrap_err()
            .to_string()
            .contains("uncertain_effects"));
        f.control(hold)?;
        let held = f.current()?;
        let budget = f.store.session_budget(&held.session_id)?.unwrap();
        let command = f.command()?;
        assert!(held
            .manual_actions()
            .controls
            .contains(&RecoveryControlKind::InspectEffects));
        let receipt = f.engine.inspect_recovery(&command).await?;
        assert_eq!(f.engine.inspect_recovery(&command).await?, receipt);
        assert_eq!(
            Store::open(&f.store.home)?.recovery_inspection_receipt(&command)?,
            Some(receipt.clone())
        );
        let resolved = f.current()?;
        assert_eq!(resolved.status, held.status);
        assert_eq!(resolved.condition, held.condition);
        assert_eq!(resolved.wait_reason, held.wait_reason);
        assert!(!resolved.manual_permit);
        assert_eq!(resolved.failures, held.failures);
        assert_eq!(resolved.plan, held.plan);
        assert_eq!(resolved.recovery_attempts, held.recovery_attempts);
        assert!(resolved.effect_resolution.is_some());
        let after_budget = f.store.session_budget(&held.session_id)?.unwrap();
        assert_eq!(after_budget.limits, budget.limits);
        assert_eq!(
            after_budget.admitted_invocations,
            budget.admitted_invocations + 1
        );
        let mut stale = command.clone();
        stale.command_id = new_id();
        assert!(f
            .engine
            .inspect_recovery(&stale)
            .await
            .unwrap_err()
            .to_string()
            .contains("stale_inspection"));
        let mut foreign = command.clone();
        foreign.session_id = new_id();
        assert!(f.engine.inspect_recovery(&foreign).await.is_err());
        let trace = f.store.trace(&held.session_id)?;
        let record = trace
            .decisions
            .iter()
            .find(|d| d.id == receipt.inspection_id)
            .unwrap();
        let inspection = record.links.recovery_inspection.as_ref().unwrap();
        assert_eq!(inspection.command, command);
        assert_eq!(inspection.plan, held.plan);
        assert_eq!(inspection.effects[0].failure, held.failures[0]);
        assert_eq!(
            inspection.effects[0].files[0],
            FileSnapshot::capture(&f.path, std::path::Path::new("review-side-effect.txt"))?
        );
        assert_ne!(inspection.reviewer.agent_id, held.failures[0].agent_id);
        let producer = f
            .original
            .assignments
            .iter()
            .find(|a| a.purpose == "plan")
            .unwrap();
        assert_ne!(inspection.reviewer.agent_id, producer.agent_id);
        assert!(f
            .store
            .commit_recovery_inspection(record)
            .unwrap_err()
            .to_string()
            .contains("stale_inspection"));
        let mut wrong = record.clone();
        wrong.session_id = new_id();
        assert!(f.store.commit_recovery_inspection(&wrong).is_err());
        let mut wrong_version = record.clone();
        wrong_version.id = new_id();
        let changed = wrong_version.links.recovery_inspection.as_mut().unwrap();
        changed.command.command_id = new_id();
        changed.command.expected_revision = resolved.revision;
        changed.plan.as_mut().unwrap().revision += 1;
        assert!(f
            .store
            .commit_recovery_inspection(&wrong_version)
            .unwrap_err()
            .to_string()
            .contains("stale_inspection"));
        assert_ne!(
            f.engine
                .run(&f.path, "", Some(&held.session_id))
                .await?
                .session
                .status,
            "completed"
        );
        assert_eq!(
            f.script.calls.lock().unwrap().len(),
            1,
            "Inspection must not waive the owner hold"
        );
        f.control(RecoveryControl::Continue)?;
        let result = f.engine.run(&f.path, "", Some(&held.session_id)).await?;
        assert_eq!(result.session.status, "completed", "{}", result.summary);
        let calls = f.script.calls.lock().unwrap();
        assert!(!calls.iter().any(|r| r.purpose == "plan"));
        assert!(calls
            .iter()
            .filter(|r| r.purpose == "review_plan")
            .all(|r| r.read_only));
        assert_eq!(
            calls.iter().filter(|r| r.purpose == "review_plan").count(),
            1
        );
        assert_eq!(f.current()?.failures, held.failures);
        assert!(f.store.observations()?.is_empty());
    }
    Ok(())
}

struct InspectionResponseBackend {
    script: Arc<Script>,
    response: &'static str,
}
impl ExecutionBackend for InspectionResponseBackend {
    fn identity(&self) -> ExecutionBackendIdentity {
        self.script.identity()
    }
    fn workspace_access(&self, _: &TurnRequest) -> WorkspaceAccess {
        WorkspaceAccess::ReadAll
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            let mut result = self.script.execute(request, events).await?;
            result.text = self.response.into();
            Ok(result)
        })
    }
}
#[tokio::test]
async fn r3_model_safe_flag_rejection_and_write_capable_inspector_cannot_resolve_effects(
) -> Result<()> {
    for response in [
        r#"{"safe":true}"#,
        r#"{"approved":false,"reason":"Unexplained local effect"}"#,
    ] {
        let mut f = Fixture::new(true, false, None).await?;
        f.engine =
            f.engine
                .clone()
                .with_execution_backend(Arc::new(InspectionResponseBackend {
                    script: f.script.clone(),
                    response,
                }))?;
        assert!(f.engine.inspect_recovery(&f.command()?).await.is_err());
        assert!(f.current()?.effect_resolution.is_none());
        assert!(f.control(RecoveryControl::Continue).is_err());
        assert_eq!(f.script.calls.lock().unwrap().len(), 1);
    }
    let mut f = Fixture::new(true, false, None).await?;
    let writer = Arc::new(Script {
        failure_purpose: "never",
        gate: None,
        mode: Mode::Unknown,
        writes: true,
        failures: AtomicUsize::new(0),
        calls: Mutex::new(vec![]),
    });
    f.engine = f.engine.clone().with_execution_backend(writer.clone())?;
    assert!(f
        .engine
        .inspect_recovery(&f.command()?)
        .await
        .unwrap_err()
        .to_string()
        .contains("inspection_access"));
    assert!(writer.calls.lock().unwrap().is_empty());
    assert!(f.current()?.effect_resolution.is_none());
    Ok(())
}

#[tokio::test]
async fn r3_inspection_consumes_the_captured_budget_and_cannot_create_another_attempt() -> Result<()>
{
    let f = Fixture::with_limit(true, false, None, 4).await?;
    f.engine.inspect_recovery(&f.command()?).await?;
    f.engine.inspect_recovery(&f.command()?).await?;
    let spent = f.store.session_budget(&f.stage.session_id)?.unwrap();
    assert_eq!(spent.admitted_invocations, 4);
    assert!(f.engine.inspect_recovery(&f.command()?).await.is_err());
    assert_eq!(f.script.calls.lock().unwrap().len(), 2);
    assert_eq!(
        f.store.session_budget(&f.stage.session_id)?.unwrap().limits,
        spent.limits
    );
    f.control(RecoveryControl::Continue)?;
    assert_ne!(
        f.engine
            .run(&f.path, "", Some(&f.stage.session_id))
            .await?
            .session
            .status,
        "completed"
    );
    assert_eq!(f.script.calls.lock().unwrap().len(), 2);
    assert_eq!(
        f.store
            .session_budget(&f.stage.session_id)?
            .unwrap()
            .admitted_invocations,
        4
    );
    Ok(())
}

#[tokio::test]
async fn r3_missing_scope_and_unverified_termination_do_not_authorize_inspection_or_replay(
) -> Result<()> {
    for (scope, unverified) in [(false, false), (true, true)] {
        let f = Fixture::new(scope, unverified, None).await?;
        let before = f.store.trace(&f.stage.session_id)?;
        assert!(f
            .engine
            .inspect_recovery(&f.command()?)
            .await
            .unwrap_err()
            .to_string()
            .contains("uncertain_effects"));
        assert!(f
            .control(RecoveryControl::Retry)
            .unwrap_err()
            .to_string()
            .contains("uncertain_effects"));
        assert!(f.current()?.effect_resolution.is_none());
        assert_eq!(f.current()?.failures, f.stage.failures);
        assert_eq!(f.store.session_budget(&f.stage.session_id)?, before.budget);
        assert!(f.script.calls.lock().unwrap().is_empty());
        let view = f.engine.team_control(&f.stage.session_id)?;
        f.engine.owner_team_command(&team_command(
            &view,
            OwnerTeamAction::Replace {
                agent_id: f.stage.failures[0].agent_id.clone(),
                replacement_id: "reserve".into(),
            },
        ))?;
        let view = f.engine.team_control(&f.stage.session_id)?;
        f.engine
            .owner_team_command(&team_command(&view, OwnerTeamAction::Continue))?;
        assert_ne!(
            f.engine
                .run(&f.path, "", Some(&f.stage.session_id))
                .await?
                .session
                .status,
            "completed"
        );
        assert!(f.store.tasks(&f.stage.session_id)?.is_empty());
        assert!(f.script.calls.lock().unwrap().is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn r3_changed_inspected_files_reject_continue_and_atomic_replay_admission() -> Result<()> {
    let f = Fixture::new(true, false, None).await?;
    f.engine.inspect_recovery(&f.command()?).await?;
    let evidence = f.current()?.effect_resolution.unwrap();
    let path = f.path.join("review-side-effect.txt");
    std::fs::write(&path, "changed after inspection")?;
    assert!(f
        .control(RecoveryControl::Continue)
        .unwrap_err()
        .to_string()
        .contains("uncertain_effects"));
    std::fs::write(&path, "known local fixture effect")?;
    f.control(RecoveryControl::Continue)?;
    std::fs::write(&path, "changed after continuation, before admission")?;
    let before = f.script.calls.lock().unwrap().len();
    assert_ne!(
        f.engine
            .run(&f.path, "", Some(&f.stage.session_id))
            .await?
            .session
            .status,
        "completed"
    );
    assert_eq!(f.script.calls.lock().unwrap().len(), before);
    assert_eq!(f.current()?.effect_resolution.as_ref(), Some(&evidence));
    assert_eq!(f.current()?.failures, f.stage.failures);
    Ok(())
}

#[tokio::test]
async fn r3_inspection_commit_rejects_changed_data_and_racing_owner_pause() -> Result<()> {
    for pause in [false, true] {
        let gate = Arc::new(Gate {
            purpose: "review",
            started: tokio::sync::Semaphore::new(0),
            release: tokio::sync::Semaphore::new(0),
        });
        let f = Fixture::new(true, false, Some(gate.clone())).await?;
        let command = f.command()?;
        let engine = f.engine.clone();
        let work = tokio::spawn(async move { engine.inspect_recovery(&command).await });
        tokio::time::timeout(std::time::Duration::from_secs(10), gate.started.acquire())
            .await??
            .forget();
        if pause {
            f.control(RecoveryControl::Pause)?;
        } else {
            std::fs::write(
                f.path.join("review-side-effect.txt"),
                "changed during inspection",
            )?;
        }
        gate.release.add_permits(1);
        let error = tokio::time::timeout(std::time::Duration::from_secs(10), work)
            .await??
            .unwrap_err();
        assert!(error.to_string().contains("stale_inspection"), "{error}");
        assert!(f.current()?.effect_resolution.is_none());
        assert_eq!(f.current()?.failures, f.stage.failures);
        if pause {
            assert_eq!(f.current()?.status, RecoveryStatus::Paused);
        }
        assert_eq!(f.script.calls.lock().unwrap().len(), 1);
    }
    Ok(())
}

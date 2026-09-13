use super::{confirmation_tests::exact_contract, tests::RunFixture, *};
use ymp_providers::ExecutionFuture;

enum Script {
    Normal,
    InvalidPlan {
        revision_only: bool,
        duplicate: bool,
    },
    FailFirstPlan,
    FailFirstExecute,
    ChangeFile {
        path: PathBuf,
        phase: &'static str,
    },
}

struct ContractBackend {
    script: Script,
    requests: Mutex<Vec<TurnRequest>>,
    store: Store,
    plans: AtomicUsize,
    executions: AtomicUsize,
}

impl ExecutionBackend for ContractBackend {
    fn identity(&self) -> ExecutionBackendIdentity {
        NativeExecutionBackend.identity()
    }

    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        if matches!(&self.script,Script::ChangeFile {phase,..} if *phase==request.purpose) {
            WorkspaceAccess::WriteAll
        } else {
            NativeExecutionBackend.workspace_access(request)
        }
    }

    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            // This observes the production provider boundary, before its first response.
            let session = self
                .store
                .sessions(None)?
                .into_iter()
                .find(|s| s.status == "running")
                .unwrap();
            let trace = self.store.trace(&session.id)?;
            assert!(trace
                .decisions
                .iter()
                .any(|d| d.kind == "acceptance_contract_captured"));
            self.requests.lock().unwrap().push(request.clone());
            let revision =
                request.purpose == "plan" && self.plans.fetch_add(1, Ordering::SeqCst) > 0;
            if matches!(self.script, Script::FailFirstPlan)
                && request.purpose == "plan"
                && !revision
            {
                bail!("Scripted initial planning interruption");
            }
            if matches!(self.script, Script::FailFirstExecute)
                && request.purpose == "execute"
                && self.executions.fetch_add(1, Ordering::SeqCst) == 0
            {
                bail!("Scripted initial production interruption");
            }
            if let Script::ChangeFile { path, phase } = &self.script {
                if request.purpose == *phase {
                    std::fs::write(path, "changed after capture\n")?;
                }
            }
            let purpose = request.purpose.clone();
            let mut result = NativeExecutionBackend.execute(request, events).await?;
            if let Script::InvalidPlan {
                revision_only,
                duplicate,
            } = self.script
            {
                if purpose == "plan" && (!revision_only || revision) {
                    let mut plan: Value = serde_json::from_str(&result.text)?;
                    if duplicate {
                        let task = plan["tasks"][0].clone();
                        plan["tasks"].as_array_mut().unwrap().push(task);
                    } else {
                        plan["tasks"][0]["title"] = json!("A substituted target");
                    }
                    result.text = serde_json::to_string(&plan)?;
                }
            }
            Ok(result)
        })
    }
}

fn install(
    fixture: &mut RunFixture,
    contracts: Vec<AcceptanceContract>,
    script: Script,
) -> Arc<ContractBackend> {
    // Exercise the same typed, persisted configuration consumed by CLI and TUI.
    fixture.engine.config.acceptance_contracts = Some(contracts);
    fixture.engine.config.save(&fixture.store.home).unwrap();
    fixture.engine.config = Config::load(&fixture.store.home).unwrap();
    assert!(fixture.engine.acceptance_contracts.is_empty());
    let backend = Arc::new(ContractBackend {
        script,
        requests: Mutex::new(vec![]),
        store: fixture.store.clone(),
        plans: AtomicUsize::new(0),
        executions: AtomicUsize::new(0),
    });
    fixture.engine = fixture
        .engine
        .clone()
        .with_execution_backend(backend.clone())
        .unwrap();
    backend
}

fn captures(trace: &SessionTrace) -> Vec<CapturedAcceptanceContract> {
    trace
        .decisions
        .iter()
        .filter_map(|d| d.links.acceptance_contract.clone())
        .collect()
}

#[tokio::test]
async fn configured_invalid_contract_sets_leave_no_partial_session() {
    for duplicate in [false, true] {
        let mut f = RunFixture::new("", false);
        let mut second = exact_contract();
        if !duplicate {
            second.task_title = "Second required target".into();
            second.inputs.push("missing-source.txt".into());
        }
        f.engine.config.acceptance_contracts = Some(vec![exact_contract(), second]);
        let error = f
            .engine
            .run(&f.project, "Create a greeting", None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains(if duplicate {
            "Duplicate"
        } else {
            "Declared input is missing"
        }));
        assert!(f.store.sessions(None).unwrap().is_empty());
    }
}

#[tokio::test]
async fn configured_resume_revalidates_saved_plan_bindings_before_production_or_review() {
    for duplicate in [false, true] {
        let mut f = RunFixture::new("[mock:no-checks]", false);
        install(&mut f, vec![exact_contract()], Script::FailFirstExecute);
        let first = f.run().await;
        let before = f.store.trace(&first.session.id).unwrap();
        let mut task = before.tasks[0].clone();
        if duplicate {
            task.id = new_id();
        } else {
            task.title = "Substituted saved target".into();
        }
        f.store.save_task(&task).unwrap();
        let resumed = f
            .engine
            .run(&f.project, "", Some(&first.session.id))
            .await
            .unwrap();
        assert_eq!(resumed.session.status, "blocked");
        assert!(resumed
            .summary
            .contains("requires exactly one planned task"));
        let after = f.store.trace(&first.session.id).unwrap();
        assert_eq!(after.invocations.len(), before.invocations.len());
        assert!(f.store.observations().unwrap().is_empty());
    }
}

#[tokio::test]
async fn configured_requirements_reach_planning_revisions_without_reference_or_verifier_bytes() {
    let mut f = RunFixture::new("[mock:revise-plan][mock:no-checks]", false);
    std::fs::write(f.project.join("source.txt"), "PRIVATE_REFERENCE_OUTPUT").unwrap();
    let verifier = f.project.parent().unwrap().join("verifier.sh");
    std::fs::write(&verifier, "# PRIVATE_VERIFIER_CONTENT\nexit 0\n").unwrap();
    let mut contract = exact_contract();
    contract.criteria[0].description = "PUBLIC_CRITERION".into();
    contract.inputs.push("source.txt".into());
    let expected = b"PRIVATE_EXPECTED_BYTES".to_vec();
    contract.checks[0].assertion = CheckAssertion::ExactBytes {
        artifact: "greeting.txt".into(),
        expected: expected.clone(),
    };
    contract.checks.push(TrustedCheck {
        id: "private-verifier".into(),
        criterion_ids: vec!["greeting-content".into()],
        assertion: CheckAssertion::Command {
            program: "/bin/sh".into(),
            args: vec![verifier.display().to_string()],
            verifier_files: vec![verifier.clone()],
        },
    });
    let backend = install(&mut f, vec![contract.clone()], Script::Normal);
    let outcome = f.run().await;
    assert_eq!(outcome.session.status, "blocked");
    let requests = backend.requests.lock().unwrap();
    assert!(requests.iter().filter(|r| r.purpose == "plan").count() >= 2);
    for request in requests.iter() {
        for public in [
            "Create a greeting",
            "PUBLIC_CRITERION",
            "greeting.txt",
            "source.txt",
            "exactly one task",
        ] {
            assert!(
                request.prompt.contains(public),
                "Missing {public} in {}",
                request.purpose
            );
        }
        for private in [
            "PRIVATE_EXPECTED_BYTES",
            "PRIVATE_REFERENCE_OUTPUT",
            "PRIVATE_VERIFIER_CONTENT",
            verifier.to_str().unwrap(),
            &serde_json::to_string(&expected).unwrap(),
        ] {
            assert!(
                !request.prompt.contains(private),
                "Leaked {private} into {}",
                request.purpose
            );
        }
    }
    let trace = f.store.trace(&outcome.session.id).unwrap();
    assert_eq!(captures(&trace)[0].contract, contract);
    assert_eq!(
        captures(&trace)[0].inputs[0].bytes.as_deref(),
        Some(b"PRIVATE_REFERENCE_OUTPUT".as_slice())
    );
    assert!(
        trace
            .decisions
            .iter()
            .find(|d| d.kind == "acceptance_contract_captured")
            .unwrap()
            .created_at
            <= trace.invocations[0].started_at
    );
}

#[tokio::test]
async fn configured_missing_and_ambiguous_plan_targets_are_rejected_including_revisions() {
    for revision_only in [false, true] {
        for duplicate in [false, true] {
            let mut f = RunFixture::new(
                if revision_only {
                    "[mock:revise-plan]"
                } else {
                    ""
                },
                false,
            );
            let backend = install(
                &mut f,
                vec![exact_contract()],
                Script::InvalidPlan {
                    revision_only,
                    duplicate,
                },
            );
            let outcome = f.run().await;
            assert_eq!(outcome.session.status, "blocked", "{}", outcome.summary);
            let trace = f.store.trace(&outcome.session.id).unwrap();
            assert!(trace.tasks.is_empty());
            assert!(!f.project.join("greeting.txt").exists());
            assert!(backend
                .requests
                .lock()
                .unwrap()
                .iter()
                .all(|r| r.purpose != "execute"));
            assert!(
                f.store
                    .messages(&outcome.session.id, 0, 1000)
                    .unwrap()
                    .iter()
                    .any(|m| m.text.contains("requires exactly one planned task"))
                    || outcome
                        .summary
                        .contains("requires exactly one planned task")
            );
        }
    }
}

#[tokio::test]
async fn configured_resume_preserves_capture_and_rejects_replacement_before_invocation() {
    let mut f = RunFixture::new("[mock:no-checks]", false);
    install(&mut f, vec![exact_contract()], Script::FailFirstPlan);
    let first = f.run().await;
    assert_eq!(first.session.status, "blocked");
    let before = f.store.trace(&first.session.id).unwrap();
    let original = captures(&before);
    for change in [
        "expected",
        "title",
        "criterion",
        "artifact",
        "input",
        "check",
        "empty",
    ] {
        let mut replacement = original[0].contract.clone();
        match change {
            "expected" => {
                replacement.checks[0].assertion = CheckAssertion::ExactBytes {
                    artifact: "greeting.txt".into(),
                    expected: b"replaced".to_vec(),
                }
            }
            "title" => replacement.task_title.push_str(" changed"),
            "criterion" => replacement.criteria[0].description.push_str(" changed"),
            "artifact" => replacement.artifacts.push("other.txt".into()),
            "input" => replacement.inputs.push("missing.txt".into()),
            "check" => replacement.checks.clear(),
            _ => (),
        }
        f.engine.config.acceptance_contracts = Some(if change == "empty" {
            vec![]
        } else {
            vec![replacement]
        });
        let error = f
            .engine
            .run(&f.project, "", Some(&first.session.id))
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("immutable on resume"),
            "{change}: {error}"
        );
        let after = f.store.trace(&first.session.id).unwrap();
        assert_eq!(after.invocations.len(), before.invocations.len());
        assert_eq!(captures(&after), original);
    }
    f.engine.config.acceptance_contracts = None;
    f.engine.config.save(&f.store.home).unwrap();
    f.engine.config = Config::load(&f.store.home).unwrap();
    continue_planning_fixture(&f, &first.session.id);
    let resumed = f
        .engine
        .run(&f.project, "", Some(&first.session.id))
        .await
        .unwrap();
    assert_eq!(resumed.session.status, "completed", "{}", resumed.summary);
    let after = f.store.trace(&first.session.id).unwrap();
    assert_eq!(captures(&after), original);
    assert_eq!(f.store.observations().unwrap().len(), 1);
}

#[tokio::test]
async fn configured_input_and_verifier_drift_on_resume_is_not_recaptured_or_confirmed() {
    for changed in ["input", "verifier"] {
        let mut f = RunFixture::new("[mock:no-checks]", false);
        std::fs::write(f.project.join("source.txt"), "initial reference").unwrap();
        let verifier = f.project.parent().unwrap().join("verifier.sh");
        std::fs::write(&verifier, "exit 0\n").unwrap();
        let mut contract = exact_contract();
        contract.inputs.push("source.txt".into());
        contract.checks[0].assertion = CheckAssertion::Command {
            program: "/bin/sh".into(),
            args: vec![verifier.display().to_string()],
            verifier_files: vec![verifier.clone()],
        };
        install(&mut f, vec![contract], Script::FailFirstPlan);
        let first = f.run().await;
        let original = captures(&f.store.trace(&first.session.id).unwrap());
        std::fs::write(
            if changed == "input" {
                f.project.join("source.txt")
            } else {
                verifier
            },
            "changed\n",
        )
        .unwrap();
        continue_planning_fixture(&f, &first.session.id);
        let resumed = f
            .engine
            .run(&f.project, "", Some(&first.session.id))
            .await
            .unwrap();
        assert_eq!(
            resumed.session.status,
            if changed == "input" {
                "blocked"
            } else {
                "completed"
            },
            "{}",
            resumed.summary
        );
        assert!(resumed.summary.contains("unconfirmed"));
        let after = f.store.trace(&first.session.id).unwrap();
        assert_eq!(captures(&after), original);
        assert!(after.decisions.iter().any(|d| d.links.check.is_some()));
        assert!(after
            .decisions
            .iter()
            .filter_map(|d| d.links.check.as_ref())
            .all(|c| c.outcome == ConfirmationCheckOutcome::Inconclusive));
        assert!(after.decisions.iter().any(|d| d.kind == "final_accepted"
            && d.outcome
                == Some(DecisionOutcome::Accepted {
                    confirmation: ConfirmationStatus::Unconfirmed
                })));
        assert!(f.store.observations().unwrap().is_empty());
    }
}

#[tokio::test]
async fn configured_changes_during_production_or_review_do_not_create_credit() {
    for changed in ["input", "verifier", "artifact"] {
        let mut f = RunFixture::new("[mock:no-checks]", false);
        f.engine.config.limits.attempts = 1;
        std::fs::write(f.project.join("source.txt"), "initial reference").unwrap();
        let verifier = f.project.parent().unwrap().join("verifier.sh");
        std::fs::write(&verifier, "exit 0\n").unwrap();
        let mut contract = exact_contract();
        contract.inputs.push("source.txt".into());
        contract.checks.push(TrustedCheck {
            id: "verifier".into(),
            criterion_ids: vec!["greeting-content".into()],
            assertion: CheckAssertion::Command {
                program: "/bin/sh".into(),
                args: vec![verifier.display().to_string()],
                verifier_files: vec![verifier.clone()],
            },
        });
        let (path, phase) = match changed {
            "input" => (f.project.join("source.txt"), "execute"),
            "verifier" => (verifier, "execute"),
            _ => (f.project.join("greeting.txt"), "review"),
        };
        install(&mut f, vec![contract], Script::ChangeFile { path, phase });
        let outcome = f.run().await;
        let trace = f.store.trace(&outcome.session.id).unwrap();
        assert!(trace.decisions.iter().any(|d| d.links.check.is_some()));
        assert!(!trace.decisions.iter().any(|d| d.kind == "final_accepted"
            && d.outcome
                == Some(DecisionOutcome::Accepted {
                    confirmation: ConfirmationStatus::Confirmed
                })));
        assert!(f.store.observations().unwrap().is_empty());
        if changed == "artifact" {
            assert_eq!(outcome.session.status, "blocked");
        }
    }
}

fn continue_planning_fixture(f: &RunFixture, session: &str) {
    let stage = f
        .store
        .recovery_stages(session)
        .unwrap()
        .into_iter()
        .find(|s| s.purpose == "plan" && s.status != RecoveryStatus::Complete)
        .unwrap();
    f.engine
        .control_recovery(&RecoveryControlCommand {
            session_id: session.into(),
            stage_id: stage.id,
            expected_revision: stage.revision,
            command_id: new_id(),
            action: RecoveryControl::Continue,
        })
        .unwrap();
}

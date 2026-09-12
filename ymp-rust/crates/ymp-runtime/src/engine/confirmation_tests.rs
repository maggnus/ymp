use super::tests::RunFixture;
use super::*;

async fn await_phase(fixture: &mut RunFixture, phase: &str) {
    loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(10), fixture.events.recv())
            .await
            .expect("Run stopped producing events before the tested phase")
            .expect("Run event channel closed before the tested phase");
        if matches!(event, UiEvent::AgentStatus { ref status, .. } if status == phase) {
            return;
        }
        assert!(
            !matches!(event, UiEvent::Finished { .. }),
            "Run finished before phase {phase}"
        );
    }
}

pub(super) fn exact_contract() -> AcceptanceContract {
    AcceptanceContract {
        task_title: "Create a greeting".into(),
        criteria: vec![AcceptanceCriterion {
            id: "greeting-content".into(),
            description: "The greeting file contains exactly the requested greeting".into(),
        }],
        artifacts: vec!["greeting.txt".into()],
        inputs: vec![],
        checks: vec![TrustedCheck {
            id: "exact-greeting-v1".into(),
            criterion_ids: vec!["greeting-content".into()],
            assertion: CheckAssertion::ExactBytes {
                artifact: "greeting.txt".into(),
                expected: b"Hello from ymp\n".to_vec(),
            },
        }],
    }
}
fn third_agent(fixture: &mut RunFixture) {
    let mut agent = fixture.engine.config.agents[0].clone();
    agent.id = "three".into();
    agent.name = "three".into();
    fixture.engine.config.agents.push(agent);
    fixture.engine.config.team.push("three".into());
}
fn accepted(trace: &SessionTrace) -> &DecisionRecord {
    trace
        .decisions
        .iter()
        .find(|d| d.kind == "task_accepted")
        .unwrap()
}

#[tokio::test]
async fn confirmation_success_is_attributed_once_and_survives_narration_failure() {
    for marker in ["", "[mock:fail:synthesis]"] {
        let mut fixture = RunFixture::new(marker, false);
        fixture.engine.acceptance_contracts.push(exact_contract());
        let outcome = fixture.run().await;
        assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
        let trace = fixture.store.trace(&outcome.session.id).unwrap();
        assert_eq!(
            accepted(&trace).outcome,
            Some(DecisionOutcome::Accepted {
                confirmation: ConfirmationStatus::Confirmed
            })
        );
        let final_acceptance = trace
            .decisions
            .iter()
            .find(|d| d.kind == "final_accepted")
            .unwrap();
        assert_eq!(final_acceptance.outcome, accepted(&trace).outcome);
        let result = accepted(&trace).links.result.as_ref().unwrap();
        let producer = trace
            .assignments
            .iter()
            .find(|a| result.producer_assignment_ids.contains(&a.id))
            .unwrap();
        let final_review = trace
            .decisions
            .iter()
            .find(|d| d.kind == "final_review")
            .unwrap();
        assert_ne!(final_review.actor.as_ref(), Some(&producer.agent_id));
        let observations = fixture.store.observations().unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].competence, "implementation");
        assert_eq!(
            fixture
                .store
                .reputation(&observations[0].agent_version, "implementation", "simple")
                .unwrap()
                .successes,
            1
        );
        let reopened = Store::open(&fixture.store.home).unwrap();
        assert!(!reopened
            .observe_confirmed(&observations[0], &accepted(&trace).id)
            .unwrap());
        let mut duplicate = observations[0].clone();
        duplicate.id = new_id();
        assert!(reopened
            .observe_confirmed(&duplicate, &accepted(&trace).id)
            .is_err());
        assert_eq!(reopened.observations().unwrap().len(), 1);
        let check = trace
            .decisions
            .iter()
            .find_map(|d| d.links.check.as_ref())
            .unwrap();
        assert_eq!(check.outcome, ConfirmationCheckOutcome::Passed);
        assert_eq!(
            check.artifacts_after[0].bytes.as_deref(),
            Some(b"Hello from ymp\n".as_slice())
        );
        assert_eq!(accepted(&trace).links.confirmation_ids.len(), 1);
        if !marker.is_empty() {
            assert!(outcome.summary.contains("Confirmation: Confirmed"));
        }
    }
}

#[tokio::test]
async fn confirmation_coverage_and_agreement_do_not_imply_quality() {
    let mut fixture = RunFixture::new("[mock:dispute]", false);
    third_agent(&mut fixture);
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    assert!(trace
        .decisions
        .iter()
        .any(|d| d.kind == "candidate_arbitration"));
    assert!(fixture.store.observations().unwrap().is_empty());
    let mut fixture = RunFixture::new("", false);
    let mut contract = exact_contract();
    contract.criteria.push(AcceptanceCriterion {
        id: "useful".into(),
        description: "The result serves the qualitative purpose".into(),
    });
    fixture.engine.acceptance_contracts.push(contract);
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "completed");
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    assert_eq!(
        accepted(&trace).outcome,
        Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Unconfirmed
        })
    );
    assert_eq!(
        accepted(&trace).links.confirmation_ids.len(),
        1,
        "Partial evidence remains inspectable"
    );
    assert_eq!(
        trace
            .decisions
            .iter()
            .find(|d| d.kind == "final_accepted")
            .unwrap()
            .outcome,
        accepted(&trace).outcome
    );
    assert!(fixture.store.observations().unwrap().is_empty());
}

#[tokio::test]
async fn confirmation_failed_applicable_assertion_overrides_every_approving_agent() {
    let mut fixture = RunFixture::new("[mock:no-checks][mock:broken-output]", false);
    fixture.engine.acceptance_contracts.push(exact_contract());
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "blocked");
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    assert_eq!(trace.tasks[0].state, TaskState::Blocked);
    assert!(trace
        .decisions
        .iter()
        .filter(|d| d.kind == "candidate_review")
        .all(|d| matches!(d.outcome, Some(DecisionOutcome::Accepted { .. }))));
    assert_eq!(
        trace
            .decisions
            .iter()
            .filter(|d| d.kind == "task_rejected")
            .count(),
        2
    );
    assert!(trace
        .decisions
        .iter()
        .filter_map(|d| d.links.check.as_ref())
        .all(|c| c.outcome == ConfirmationCheckOutcome::Failed));
    assert!(fixture.store.observations().unwrap().is_empty());
}

#[tokio::test]
async fn confirmation_external_data_is_resolved_from_actual_supplied_bytes() {
    for supplied in [
        b"Hello from ymp\n".as_slice(),
        b"Unrelated external dataset\n".as_slice(),
    ] {
        let mut fixture = RunFixture::new("[mock:no-checks]", false);
        std::fs::write(fixture.project.join("source.txt"), supplied).unwrap();
        let mut contract = exact_contract();
        contract.inputs.push("source.txt".into());
        contract.checks[0].assertion = CheckAssertion::MatchesInput {
            artifact: "greeting.txt".into(),
            input: "source.txt".into(),
        };
        fixture.engine.acceptance_contracts.push(contract);
        let outcome = fixture.run().await;
        assert_eq!(
            outcome.session.status,
            if supplied == b"Hello from ymp\n" {
                "completed"
            } else {
                "blocked"
            }
        );
        let trace = fixture.store.trace(&outcome.session.id).unwrap();
        let check = trace
            .decisions
            .iter()
            .find_map(|d| d.links.check.as_ref())
            .unwrap();
        assert_eq!(check.inputs[0].bytes.as_deref(), Some(supplied));
        assert_eq!(check.inputs[0].sha256, Some(bytes_digest(supplied)));
    }
}

#[tokio::test]
async fn confirmation_pinned_validator_captures_actual_process_and_data() {
    let mut fixture = RunFixture::new("[mock:no-checks]", false);
    std::fs::write(fixture.project.join("source.txt"), b"Hello from ymp\n").unwrap();
    // The verifier lives outside the writer's directory and is pinned at capture.
    let validator = fixture.project.parent().unwrap().join("validator.py");
    std::fs::write(&validator, "import pathlib,sys\np=pathlib.Path(sys.argv[1])\na=(p/'greeting.txt').read_bytes(); b=(p/'source.txt').read_bytes()\nprint('compared', len(a), len(b))\nsys.exit(0 if a == b else 1)\n").unwrap();
    let mut contract = exact_contract();
    contract.inputs.push("source.txt".into());
    contract.checks[0].assertion = CheckAssertion::Command {
        program: "/usr/bin/python3".into(),
        args: vec![validator.display().to_string(), "{workdir}".into()],
        verifier_files: vec![validator],
    };
    fixture.engine.acceptance_contracts.push(contract);
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    assert_eq!(
        accepted(&trace).outcome,
        Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Confirmed
        })
    );
    let check = trace
        .decisions
        .iter()
        .find_map(|d| d.links.check.as_ref())
        .unwrap();
    assert_eq!(check.stdout, b"compared 15 15\n");
    assert!(check.stderr.is_empty());
    assert_eq!(check.exit_code, Some(0));
}

#[tokio::test]
async fn confirmation_stale_artifact_requires_a_new_attempt_and_new_review() {
    let mut fixture = RunFixture::new("", false);
    fixture.engine.acceptance_contracts.push(exact_contract());
    let engine = fixture.engine.clone();
    let project = fixture.project.clone();
    let run = tokio::spawn(async move { engine.run(&project, "Create a greeting", None).await });
    await_phase(&mut fixture, "review").await;
    std::fs::write(
        fixture.project.join("greeting.txt"),
        "changed after checking\n",
    )
    .unwrap();
    let outcome = run.await.unwrap().unwrap();
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    let rejected = trace
        .decisions
        .iter()
        .find(|d| d.kind == "task_rejected")
        .unwrap();
    assert_eq!(rejected.links.result.as_ref().unwrap().version, 1);
    assert_eq!(accepted(&trace).links.result.as_ref().unwrap().version, 2);
    let observations = fixture.store.observations().unwrap();
    assert_eq!(observations.len(), 1);
    assert!(observations[0].id.contains(":2:"));
    std::fs::write(fixture.project.join("greeting.txt"), "changed later").unwrap();
    assert_eq!(
        fixture
            .store
            .confirmation_grade(
                &outcome.session.id,
                accepted(&trace).links.result.as_ref().unwrap()
            )
            .unwrap()
            .0,
        ConfirmationStatus::Unconfirmed
    );
    assert!(fixture
        .store
        .observe_confirmed(&observations[0], &accepted(&trace).id)
        .is_err());
}

#[tokio::test]
async fn confirmation_storage_rejects_forged_result_review_and_credit_links() {
    let mut fixture = RunFixture::new("", false);
    fixture.engine.acceptance_contracts.push(exact_contract());
    let outcome = fixture.run().await;
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    let baseline = serde_json::to_value(&trace).unwrap();
    assert!(
        fixture.store.save_task(&trace.tasks[0]).is_err(),
        "Plain task writes must not bypass acceptance binding"
    );
    let mut unknown = accepted(&trace).links.result.clone().unwrap();
    unknown.id = "invented-result".into();
    assert!(fixture
        .store
        .confirmation_grade(&outcome.session.id, &unknown)
        .is_err());
    let review = trace
        .decisions
        .iter()
        .find(|d| d.kind == "candidate_review")
        .unwrap();
    let producer = trace
        .assignments
        .iter()
        .find(|a| a.purpose == "execute")
        .unwrap();
    for mismatch in 0..5 {
        let mut forged = if mismatch < 2 {
            review.clone()
        } else {
            accepted(&trace).clone()
        };
        forged.id = new_id();
        match mismatch {
            0 => forged.actor = Some(producer.agent_id.clone()),
            1 => forged
                .links
                .result
                .as_mut()
                .unwrap()
                .summary
                .push_str(" invented result"),
            2 => forged.links.confirmation_ids.clear(),
            3 => forged.links.review_ids = vec![accepted(&trace).id.clone()],
            _ => {
                forged.kind = "reputation_observed".into();
                forged.links.observation_id = Some("invented-credit".into());
            }
        }
        assert!(
            fixture.store.record_decision(&forged).is_err(),
            "forgery {mismatch} was accepted"
        );
        assert_eq!(
            serde_json::to_value(fixture.store.trace(&outcome.session.id).unwrap()).unwrap(),
            baseline
        );
    }
}

#[tokio::test]
async fn confirmation_final_review_excludes_all_aggregate_producers() {
    let fixture = RunFixture::new("[mock:split-writers]", false);
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    assert_eq!(trace.tasks.len(), 2);
    assert!(trace.tasks.iter().all(|t| t.state == TaskState::Accepted));
    let final_review = trace
        .assignments
        .iter()
        .find(|a| a.purpose == "final_review")
        .unwrap();
    assert!(trace
        .assignments
        .iter()
        .filter(|a| a.purpose == "execute")
        .all(|a| a.agent_id != final_review.agent_id));
    let accepted = trace
        .decisions
        .iter()
        .find(|d| d.kind == "final_accepted")
        .unwrap();
    assert_eq!(
        accepted
            .links
            .result
            .as_ref()
            .unwrap()
            .producer_assignment_ids
            .len(),
        2
    );
}

struct AlternateChecker {
    calls: AtomicUsize,
    dishonest: bool,
}
impl ConfirmationChecker for AlternateChecker {
    fn identity(&self) -> CheckerIdentity {
        CheckerIdentity {
            id: "tests.alternate".into(),
            version: "1".into(),
        }
    }
    fn execute<'a>(
        &'a self,
        check: &'a TrustedCheck,
        directory: &'a Path,
        artifacts: &'a [FileSnapshot],
        inputs: &'a [FileSnapshot],
        cancel: CancellationToken,
    ) -> crate::CheckFuture<'a> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if self.dishonest {
                return Ok(crate::CheckExecution {
                    exit_code: Some(0),
                    stdout: b"unsupported plugin claim".to_vec(),
                    stderr: vec![],
                });
            }
            BuiltinConfirmationChecker
                .execute(check, directory, artifacts, inputs, cancel)
                .await
        })
    }
}
#[tokio::test]
async fn confirmation_alternative_executor_cannot_bypass_runtime_evidence_guards() {
    for dishonest in [false, true] {
        let mut fixture = RunFixture::new(
            if dishonest {
                "[mock:no-checks][mock:broken-output]"
            } else {
                "[mock:no-checks]"
            },
            false,
        );
        let checker = Arc::new(AlternateChecker {
            calls: AtomicUsize::new(0),
            dishonest,
        });
        fixture.engine.confirmation_checker = checker.clone();
        fixture.engine.acceptance_contracts.push(exact_contract());
        let outcome = fixture.run().await;
        assert_eq!(checker.calls.load(Ordering::SeqCst), 1);
        let trace = fixture.store.trace(&outcome.session.id).unwrap();
        let captured = trace
            .decisions
            .iter()
            .find_map(|d| d.links.acceptance_contract.as_ref())
            .unwrap();
        assert_eq!(captured.checker, checker.identity());
        if dishonest {
            assert_eq!(outcome.session.status, "blocked");
            assert!(outcome
                .summary
                .contains("Checker success contradicts captured typed assertion"));
            assert!(fixture.store.observations().unwrap().is_empty());
        } else {
            assert_eq!(outcome.session.status, "completed");
            assert_eq!(
                trace
                    .decisions
                    .iter()
                    .find_map(|d| d.links.check.as_ref())
                    .unwrap()
                    .checker,
                checker.identity()
            );
            assert_eq!(
                accepted(&trace).outcome,
                Some(DecisionOutcome::Accepted {
                    confirmation: ConfirmationStatus::Confirmed
                })
            );
        }
    }
}

#[tokio::test]
async fn confirmation_final_phase_and_review_outcome_are_bound_to_the_invocation() {
    let mut fixture = RunFixture::new("[mock:no-checks]", false);
    fixture.engine.acceptance_contracts.push(exact_contract());
    let outcome = fixture.run().await;
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    let candidate = trace
        .decisions
        .iter()
        .find(|d| d.kind == "candidate_review")
        .unwrap();
    let final_review = trace
        .decisions
        .iter()
        .find(|d| d.kind == "final_review")
        .unwrap();
    let final_acceptance = trace
        .decisions
        .iter()
        .find(|d| d.kind == "final_accepted")
        .unwrap();
    for forgery in 0..6 {
        let mut decision = match forgery {
            0 => accepted(&trace),
            1 => final_review,
            2 | 3 => candidate,
            _ => final_acceptance,
        }
        .clone();
        decision.id = new_id();
        match forgery {
            0 => decision.kind = "final_accepted".into(),
            1 => {
                decision.actor = candidate.actor.clone();
                decision.links.assignment_id = candidate.links.assignment_id.clone();
                decision.links.invocation_id = candidate.links.invocation_id.clone();
            }
            2 => {
                decision.outcome = Some(DecisionOutcome::Rejected);
                decision.reason = "Opposite judgment for an already observed invocation".into();
            }
            3 => {} // Even an identical assessment cannot consume the invocation again.
            4 => decision.outcome = Some(DecisionOutcome::Rejected),
            _ => decision.kind = "final_rejected".into(),
        }
        assert!(
            fixture.store.record_decision(&decision).is_err(),
            "Accepted review/phase forgery {forgery}"
        );
        assert_eq!(
            serde_json::to_value(fixture.store.trace(&outcome.session.id).unwrap()).unwrap(),
            serde_json::to_value(&trace).unwrap()
        );
    }
    let mut reopened_task = trace.tasks[0].clone();
    reopened_task.state = TaskState::Review;
    assert!(fixture.store.save_task(&reopened_task).is_err());
    let unchanged = fixture.store.trace(&outcome.session.id).unwrap();
    assert_eq!(
        serde_json::to_value(&unchanged).unwrap(),
        serde_json::to_value(&trace).unwrap()
    );
    if let Some(directory) = std::env::var_os("YMP_TEST_CAPTURE_DIR") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("accepted-attempt-reopen.json"),
            serde_json::to_vec_pretty(&json!({"before":trace,"after":unchanged})).unwrap(),
        )
        .unwrap();
    }
    // A fresh final review is valid, but its new aggregate supersedes the old one.
    let resumed = fixture
        .engine
        .run(&fixture.project, "", Some(&outcome.session.id))
        .await
        .unwrap();
    assert_eq!(resumed.session.status, "completed");
    let before = fixture.store.trace(&outcome.session.id).unwrap();
    let mut stale = final_acceptance.clone();
    stale.id = new_id();
    assert!(fixture.store.record_decision(&stale).is_err());
    assert_eq!(
        serde_json::to_value(fixture.store.trace(&outcome.session.id).unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
}

#[tokio::test]
async fn confirmation_delivery_rechecks_files_after_normal_and_failed_narration() {
    for confirmed in [false, true] {
        for fail_narration in [false, true] {
            for changed_file in ["greeting.txt", "source.txt"] {
                let mut fixture = RunFixture::new(
                    if fail_narration {
                        "[mock:fail:synthesis]"
                    } else {
                        ""
                    },
                    false,
                );
                std::fs::write(fixture.project.join("source.txt"), "supplied data\n").unwrap();
                let mut contract = exact_contract();
                contract.inputs.push("source.txt".into());
                if !confirmed {
                    contract.criteria.push(AcceptanceCriterion {
                        id: "quality".into(),
                        description: "Unconfirmed qualitative criterion".into(),
                    });
                }
                fixture.engine.acceptance_contracts.push(contract);
                let engine = fixture.engine.clone();
                let project = fixture.project.clone();
                let run = tokio::spawn(async move {
                    engine
                        .run(&project, "Create a greeting", None)
                        .await
                        .unwrap()
                });
                await_phase(&mut fixture, "synthesis").await;
                std::fs::write(
                    fixture.project.join(changed_file),
                    "changed during narration\n",
                )
                .unwrap();
                let outcome = run.await.unwrap();
                assert_eq!(outcome.session.status, "blocked", "{}", outcome.summary);
                assert!(outcome
                    .summary
                    .contains("current confirmation is unconfirmed"));
                assert!(!outcome.summary.ends_with("confirmation: confirmed."));
                let trace = fixture.store.trace(&outcome.session.id).unwrap();
                let historical = trace
                    .decisions
                    .iter()
                    .find(|d| d.kind == "final_accepted")
                    .unwrap();
                assert_eq!(
                    historical.outcome,
                    Some(DecisionOutcome::Accepted {
                        confirmation: if confirmed {
                            ConfirmationStatus::Confirmed
                        } else {
                            ConfirmationStatus::Unconfirmed
                        }
                    })
                );
                assert!(trace
                    .decisions
                    .iter()
                    .any(|d| d.kind == "result_invalidated"
                        && d.links.result == historical.links.result));
                assert!(!fixture
                    .store
                    .result_is_current(
                        &outcome.session.id,
                        historical.links.result.as_ref().unwrap()
                    )
                    .unwrap());
                assert_eq!(
                    fixture
                        .store
                        .confirmation_grade(
                            &outcome.session.id,
                            historical.links.result.as_ref().unwrap()
                        )
                        .unwrap()
                        .0,
                    ConfirmationStatus::Unconfirmed
                );
                assert_eq!(
                    fixture.store.observations().unwrap().len(),
                    usize::from(confirmed),
                    "Historical supported observations remain inspectable"
                );
                if let Some(directory) = std::env::var_os("YMP_TEST_CAPTURE_DIR") {
                    let directory = PathBuf::from(directory);
                    std::fs::create_dir_all(&directory).unwrap();
                    std::fs::write(directory.join(format!("delivery-{confirmed}-{fail_narration}-{changed_file}.json")), serde_json::to_vec_pretty(&json!({"outcome":outcome.summary,"status":outcome.session.status,"trace":trace})).unwrap()).unwrap();
                }
            }
        }
    }
}

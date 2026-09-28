//! Actual bounded A9 recovery, with paid Scripted execution and retained checks.
mod support;
use support::Directory;
use ymp_kernel as kernel;
use ymp_storage as storage;
#[allow(dead_code)]
#[path = "support/admission_fixture.rs"]
mod admission_fixture;
#[allow(dead_code)]
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use admission_fixture::Setup;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use ymp_domain::{
    Digest, Id, Prob, Proposal, Ref, Result,
    assignment::*,
    coordination::*,
    journal::{Capability, Decision, EscalationStep, MethodKind, PolicySelection},
    plan::*,
    resources::*,
    task::*,
    verification::*,
    workspace::*,
};
use ymp_kernel::{
    acceptance::*,
    decision::MethodDecision,
    gatekeeper::{AdmittedAssignment, Gatekeeper},
    journal::{ContentStore, ParameterSchemas},
    ports::{
        checks::*,
        execution::*,
        experience::CreditPolicy,
        planning::{BeliefModel, BeliefResponse, MethodRouter},
        progress::*,
        resources::CostModel,
    },
    progress::*,
    results::{PreparedAttempt, Results},
    treasury,
};
use ymp_runtime::{
    backends::scripted::{Scripted, ScriptedStep},
    checks::retained::RetainedBytes,
    clock::ManualClock,
    execution_host::{ExecutionHost, ExecutionStatus},
    policies::{
        award::FirstOffer,
        belief::{LikelihoodRatioTable, StrongestSupport},
        credit::ConfirmedOnly,
        diagnosis::{DirectFailuresOnly, RuleBasedDiagnoser},
        escalation::{DiagnosisFirstLadder, StopOnUncertainty},
        method::FixedMethod,
        progress::{AcceptedOnlyProgress, EvidenceDelta},
    },
};
use ymp_storage::{content::SqliteContent, journal::SqliteJournal};
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn real(v: f64) -> Real {
    Real::new(v).unwrap()
}
type Gate = Gatekeeper<SqliteJournal, SqliteContent>;
type Host = ExecutionHost<SqliteJournal, SqliteContent>;
type Authority = AcceptanceAuthority<SqliteJournal, SqliteContent>;
struct Backend {
    selection: PolicySelection,
    runs: Mutex<BTreeMap<Id<Invocation>, Scripted>>,
}
impl Backend {
    fn new() -> Self {
        Self {
            selection: PolicySelection::new(
                "ExecutionBackend",
                "RecoveryFixture",
                "1",
                serde_json::json!({}),
            )
            .unwrap(),
            runs: Mutex::new(BTreeMap::new()),
        }
    }
}
impl ExecutionBackend for Backend {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn start(&self, r: &ExecutionRequest<'_>) -> Result<BackendStart> {
        let mut steps = vec![];
        if r.assignment.role == RoleKind::Producer {
            steps.push(ScriptedStep::Write {
                path: WorkspacePath::new("file")?,
                bytes: if r.assignment.id.as_str() == "retry" {
                    b"fixed".to_vec()
                } else {
                    b"wrong".to_vec()
                },
            });
        } else if r.assignment.role == RoleKind::Reviewer && r.prompt.text.starts_with('[') {
            let (_, reference, proposal): (String, Ref, ReplacementProposal) =
                serde_json::from_str(&r.prompt.text).unwrap();
            let output = ReplacementVerdict {
                proposal: reference,
                old: proposal.old,
                new: proposal.new.reference(),
                criterion: proposal.criterion,
                result: proposal.result,
                approve: true,
                rationale:
                    "The full replacement preserves the criterion and corrects the defective oracle"
                        .into(),
            };
            steps.push(ScriptedStep::Emit(BackendObservation::Output(
                serde_json::to_string(&output).unwrap(),
            )));
        } else {
            steps.push(ScriptedStep::Emit(BackendObservation::Output(
                "Bounded work complete".into(),
            )));
        }
        steps.push(ScriptedStep::Complete {
            usage: Usage {
                input: 10,
                cache_read: 0,
                cache_write: 0,
                output: 0,
                reasoning: None,
            },
            coverage: Coverage::Complete,
        });
        let scripted = Scripted::new(steps)?;
        let started = scripted.start(r)?;
        self.runs
            .lock()
            .unwrap()
            .insert(r.invocation.clone(), scripted);
        Ok(started)
    }
    fn cancel(&self, h: &ExecutionHandle) -> Result<()> {
        self.runs.lock().unwrap()[&h.invocation].cancel(h)
    }
    fn events(&self, h: &ExecutionHandle, a: u64, l: usize) -> Result<Vec<BackendEvent>> {
        self.runs.lock().unwrap()[&h.invocation].events(h, a, l)
    }
    fn receipt(&self, h: &ExecutionHandle) -> Result<Receipt> {
        self.runs.lock().unwrap()[&h.invocation].receipt(h)
    }
}
struct Broken;
impl CheckRunner for Broken {
    fn environment(&self) -> Result<CheckEnvironment> {
        let mut env = RetainedBytes.environment()?;
        env.identity
            .insert("availability".into(), "fixture unavailable".into());
        Ok(env)
    }
    fn run(&self, e: &CheckExecution<'_>, _: &dyn ContentStore) -> Result<CheckObservation> {
        Ok(CheckObservation {
            check: e.check.reference(),
            target: e.target.reference()?,
            environment: Digest::of_value(e.environment)?,
            kind: CheckObservationKind::Error {
                class: ErrorClass::Environment,
                reason: "Fixture environment is unavailable".into(),
            },
            stdout: vec![],
            stderr: vec![],
        })
    }
}
fn finish(host: &Host, mut live: ymp_runtime::execution_host::LiveInvocation<SqliteJournal>) {
    host.start(&mut live).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    while host.poll(&mut live).unwrap() != ExecutionStatus::Finished {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
fn admit(
    s: &Setup<SqliteJournal>,
    gate: &Arc<Gate>,
    name: &str,
    kind: ContributionKind,
    subject: Option<ContributionSubject>,
    basis: Vec<Ref>,
    role: RoleKind,
) -> AdmittedAssignment<SqliteJournal> {
    let view = gate.view(&s.session).unwrap();
    let needs = BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]);
    let award = if basis.is_empty() {
        s.award_targets(
            name,
            kind,
            needs,
            subject,
            view.criteria().iter().map(|c| c.id.clone()).collect(),
        )
    } else {
        ymp_kernel::arbiter::Arbiter::new(s.journal.clone())
            .propose(
                &s.session,
                view.revision(),
                view.latest_at() + 1,
                Contribution {
                    id: id(name),
                    session: s.session.clone(),
                    kind,
                    targets: view.criteria().iter().map(|c| c.id.clone()).collect(),
                    subject,
                    needs,
                    forecast: Forecast {
                        p_success: Prob::new(0.5).unwrap(),
                        delta_belief: BTreeMap::new(),
                        source: ForecastSource::Model(s.cost.selection().policy.clone()),
                    },
                    cost: CostEstimate {
                        expected: real(10.0),
                        p90: real(10.0),
                    },
                    difficulty: Difficulty::Simple,
                    proposed_by: ContributionAuthor::Runtime,
                    basis,
                },
            )
            .unwrap();
        s.award_existing(name, &id(name))
    };
    let (revision, at, mut request) = s.request_with_gate(gate, name, award, "file");
    request.role = role;
    let mut prepared = gate.prepare(&s.session, revision, at, request).unwrap();
    gate.admit(&mut prepared).unwrap()
}
fn begin_call(
    s: &Setup<SqliteJournal>,
    gate: &Arc<Gate>,
    host: &Host,
    clock: &ManualClock,
    admitted: AdmittedAssignment<SqliteJournal>,
    name: &str,
    prompt: Prompt,
) {
    clock
        .advance_to(gate.view(&s.session).unwrap().latest_at())
        .unwrap();
    finish(
        host,
        host.attach(
            admitted,
            id(&format!("call-{name}")),
            id(&format!("receipt-{name}")),
            prompt,
        )
        .unwrap(),
    );
}
fn check(s: &Setup<SqliteJournal>, authority: &Authority, name: &str, bytes: &[u8]) -> Check {
    let view = s.gate.view(&s.session).unwrap();
    let mut check = Check {
        id: id(name),
        criterion: id("criterion"),
        criterion_version: view.criteria()[0].reference().unwrap().version,
        spec: CheckSpec::ExactBytes {
            path: WorkspacePath::new("file").unwrap(),
            digest: Digest::of(bytes),
        },
        author: CheckAuthor::User,
        independence: Independence::Trusted,
        visibility: CheckVisibility::Visible,
        needs: BTreeSet::from([Capability::ReadFiles]),
        verifier: None,
        version: Digest::of(b"pending"),
    };
    check.version = check.content_version().unwrap();
    let policy = view.policies()["VerificationDesigner"].clone();
    authority
        .register_check(
            &s.control,
            RegisterCheck {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                proposal: Proposal {
                    value: check,
                    rationale: "Explicit criterion oracle".into(),
                    basis: vec![],
                    policy: policy.policy.clone(),
                },
                effective: policy,
            },
        )
        .unwrap()
}
fn run(
    s: &Setup<SqliteJournal>,
    authority: &Authority,
    check: &Check,
    name: &str,
    target: &Id<Snapshot>,
    role: CheckRunRole,
    runner: &dyn CheckRunner,
) -> CheckRun {
    let view = s.gate.view(&s.session).unwrap();
    authority
        .run(
            &s.control,
            RunCheck {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                id: id(name),
                check: check.reference(),
                target: target.clone(),
                role,
            },
            runner,
        )
        .unwrap()
}
fn monitor(s: &Setup<SqliteJournal>, model: &dyn ProgressMonitor) -> Ref {
    let progress = s.intake.progress();
    let view = s.gate.view(&s.session).unwrap();
    let input = progress.input(&s.session, view.latest_at() + 1).unwrap();
    let proposal = model.assess(&input).unwrap();
    let reference = progress
        .assess(
            &s.control,
            MonitorRequest {
                expected_revision: view.revision(),
                input,
                proposal,
            },
        )
        .unwrap();
    let view = s.gate.view(&s.session).unwrap();
    let input = progress.input(&s.session, view.latest_at() + 1).unwrap();
    assert_eq!(
        progress
            .assess(
                &s.control,
                MonitorRequest {
                    expected_revision: view.revision(),
                    proposal: model.assess(&input).unwrap(),
                    input
                }
            )
            .unwrap_err()
            .code,
        "progress_no_new_work"
    );
    reference
}
fn estimates(
    s: &Setup<SqliteJournal>,
    context: &ApplicabilityContext,
) -> Vec<VerificationEstimate> {
    s.intake
        .progress()
        .verification_inputs(&s.session, Some(context))
        .unwrap()
        .into_iter()
        .map(|input| {
            let proposal = s.cost.estimate(&input).unwrap();
            VerificationEstimate {
                decision: Decision {
                    input: Digest::of_value(&input).unwrap(),
                    outcome: proposal.value.clone(),
                    proposal,
                    effective: s.cost.selection().clone(),
                    selection_change: None,
                },
                input,
            }
        })
        .collect()
}
fn diagnose(
    s: &Setup<SqliteJournal>,
    context: &ApplicabilityContext,
    model: &dyn FailureDiagnoser,
    expected: Diagnosis,
) -> Ref {
    let progress = s.intake.progress();
    let view = s.gate.view(&s.session).unwrap();
    let estimates = estimates(s, context);
    let input = progress
        .diagnosis_input(&s.session, Some(context), &estimates, view.latest_at() + 1)
        .unwrap();
    let proposal = model.diagnose(&input).unwrap();
    assert_eq!(proposal.value, expected);
    progress
        .diagnose(
            &s.control,
            DiagnosisRequest {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                progress: view.progress().monitor().unwrap().0.clone(),
                context: Some(context.clone()),
                estimates,
                input,
                proposal,
            },
        )
        .unwrap()
}
fn escalation(
    s: &Setup<SqliteJournal>,
    diagnosis: &Ref,
    model: &dyn EscalationPolicy,
    expected: EscalationStep,
) -> Ref {
    let p = s.intake.progress();
    let view = s.gate.view(&s.session).unwrap();
    let input = p.escalation_input(&s.session, diagnosis).unwrap();
    let proposal = model.next(&input).unwrap();
    assert_eq!(proposal.value.step, expected);
    assert!(proposal.value.limitation.is_none());
    p.escalate(
        &s.control,
        EscalationRequest {
            expected_revision: view.revision(),
            at: view.latest_at() + 1,
            input,
            proposal,
        },
    )
    .unwrap()
}
fn select(s: &Setup<SqliteJournal>, selection: &PolicySelection) {
    let view = s.gate.view(&s.session).unwrap();
    s.intake
        .progress()
        .select(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            selection.clone(),
        )
        .unwrap();
}
fn scope(
    s: &Setup<SqliteJournal>,
    result: &ymp_domain::result::ResultVersion,
    check: &Check,
    environment: Digest,
) -> ApplicabilityContext {
    ApplicabilityContext {
        result: result.reference().unwrap(),
        criteria: s
            .gate
            .view(&s.session)
            .unwrap()
            .criteria()
            .iter()
            .map(|c| c.reference().unwrap())
            .collect(),
        environments: BTreeMap::from([(check.reference(), BTreeSet::from([environment]))]),
    }
}
fn evidence(
    s: &Setup<SqliteJournal>,
    authority: &Authority,
    result: &ymp_domain::result::ResultVersion,
    run: &CheckRun,
    name: &str,
) {
    let view = s.gate.view(&s.session).unwrap();
    authority
        .evidence(
            &s.control,
            EvidenceRequest {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                id: id(name),
                criterion: view.criteria()[0].reference().unwrap(),
                result: result.reference().unwrap(),
                runs: vec![run.id.clone()],
                reviews: vec![],
            },
        )
        .unwrap();
}
fn ledger(
    s: &Setup<SqliteJournal>,
    authority: &Authority,
    scope: &ApplicabilityContext,
    model: &dyn BeliefModel,
) {
    let view = s.gate.view(&s.session).unwrap();
    let at = view.latest_at() + 1;
    let rules = AssessmentRules {
        mutation_threshold: Prob::new(0.8).unwrap(),
    };
    let responses = authority
        .ledger_inputs(&s.session, scope, &rules, at)
        .unwrap()
        .into_iter()
        .map(|(id, input)| {
            (
                id,
                BeliefResponse {
                    input: Digest::of_value(&input).unwrap(),
                    proposal: model.update(&input).unwrap(),
                },
            )
        })
        .collect();
    authority
        .record_ledger(
            &s.control,
            ymp_kernel::ledger::LedgerRequest {
                expected_revision: view.revision(),
                at,
                context: scope.clone(),
                rules,
                responses,
            },
            (view.policies()["BeliefModel"] != *model.selection())
                .then(|| model.selection().clone()),
        )
        .unwrap();
}
#[allow(clippy::too_many_arguments)]
fn produced(
    s: &Setup<SqliteJournal>,
    gate: &Arc<Gate>,
    host: &Host,
    clock: &ManualClock,
    results: &Results<SqliteJournal, SqliteContent>,
    admitted: AdmittedAssignment<SqliteJournal>,
    name: &str,
    prompt: Prompt,
) -> (PreparedAttempt, ymp_domain::result::ResultVersion) {
    let at = gate.view(&s.session).unwrap().latest_at();
    clock.advance_to(at).unwrap();
    gate.workspace()
        .snapshot_unstarted(
            admitted.files.as_ref().unwrap(),
            id(&format!("before-{name}")),
            at,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let attempt = results
        .prepare_attempt(
            &admitted.grant,
            view.revision(),
            at,
            id(&format!("attempt-{name}")),
            id(&format!("before-{name}")),
            id(&format!("after-{name}")),
        )
        .unwrap();
    results.begin(&attempt).unwrap();
    begin_call(s, gate, host, clock, admitted, name, prompt);
    let at = gate.view(&s.session).unwrap().latest_at();
    let result = results
        .submit(
            &attempt,
            at,
            id(&format!("result-{name}")),
            BTreeSet::from([ymp_domain::result::Artifact {
                path: WorkspacePath::new("file").unwrap(),
                digest: Digest::of(if name == "retry" {
                    b"fixed".as_slice()
                } else {
                    b"wrong".as_slice()
                }),
            }]),
            "Immutable bounded candidate".into(),
        )
        .unwrap();
    (attempt, result)
}
#[test]
fn real_boundaries_repair_replace_retry_and_stop_without_erasing_history() {
    let root = Directory::new();
    let database = Directory::new();
    std::fs::write(root.0.join("file"), b"base").unwrap();
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("ExecutionBackend", "RecoveryFixture", "1", |_| Ok(()))
        .unwrap();
    let journal = Arc::new(SqliteJournal::open(database.database(), schemas).unwrap());
    let backend = Arc::new(Backend::new());
    let delta = EvidenceDelta::new(MonitorParameters {
        epsilon: real(0.001),
        stall_limit: 2,
    })
    .unwrap();
    let accepted_only = AcceptedOnlyProgress::new(MonitorParameters {
        epsilon: real(0.001),
        stall_limit: 2,
    })
    .unwrap();
    let rule = RuleBasedDiagnoser::new(DiagnosisParameters {
        p_min: Prob::new(0.2).unwrap(),
        mutation_threshold: Prob::new(0.8).unwrap(),
    })
    .unwrap();
    let direct = DirectFailuresOnly::new(DiagnosisParameters {
        p_min: Prob::new(0.2).unwrap(),
        mutation_threshold: Prob::new(0.8).unwrap(),
    })
    .unwrap();
    let parameters = EscalationParameters {
        max_uses: 1,
        max_cost: real(20.0),
        timeout_ms: 1000,
    };
    let ladder = DiagnosisFirstLadder::new(parameters.clone()).unwrap();
    let stop = StopOnUncertainty::new(parameters).unwrap();
    let belief = LikelihoodRatioTable::standard().unwrap();
    let credit = ConfirmedOnly::new().unwrap();
    let method = FixedMethod::with_ladder(
        MethodKind::SoloWithVerifier,
        vec![
            EscalationStep::FixEnvironment,
            EscalationStep::ReplaceCheck,
            EscalationStep::Retry,
            EscalationStep::AddVerifier,
            EscalationStep::StopPreserving,
        ],
    )
    .unwrap();
    let mut constraints = fixture::default_constraints(BTreeSet::from([
        Capability::ReadFiles,
        Capability::WriteFiles,
    ]));
    constraints.budget = real(80.0);
    constraints.verification_reserve = real(10.0);
    constraints.attempt_limit = 2;
    let mut s = Setup::new_with_selections(
        journal.clone(),
        &journal,
        &root,
        true,
        "progress",
        Some(constraints),
        FirstOffer::with_commitment_terms(CommitmentTerms {
            lease_duration: 90,
            renewal_duration: 10,
            renew_on: BTreeSet::new(),
            renewals: 0,
            release_delta: real(1.0),
        })
        .unwrap(),
        vec![
            backend.selection().clone(),
            delta.selection().clone(),
            rule.selection().clone(),
            ladder.selection().clone(),
            belief.selection().clone(),
            credit.selection().clone(),
            method.selection().clone(),
            PolicySelection::new(
                "VerificationDesigner",
                "ExplicitVisible",
                "1",
                serde_json::json!({}),
            )
            .unwrap(),
        ],
    );
    let gate = Arc::new(std::mem::replace(
        &mut s.gate,
        Gatekeeper::new(journal.clone(), Arc::new(journal.content_store())),
    ));
    let authority = s.intake.acceptance(Arc::new(journal.content_store()));
    let progress = s.intake.progress();
    let results = Results::new(journal.clone(), gate.clone()).unwrap();
    let original = s.profile.clone();
    s.switch_to_new_agent("reviewer");
    let independent = s.profile.clone();
    s.profile = original.clone();
    let view = gate.view(&s.session).unwrap();
    let mut criteria = view.criteria().to_vec();
    criteria.push(Criterion {
        id: id("inspection"),
        text: "Independent inspection remains required".into(),
        kind: CriterionKind::NewBehavior,
        weight: real(1.0),
        required: true,
        origin: CriterionOrigin::User,
        needs_class: BTreeSet::from([EvidenceClass::Inspection]),
    });
    s.intake
        .refine(
            &s.control,
            ymp_kernel::intake::IntakeRefinement {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                constraints: view.task().unwrap().constraints.clone(),
                criteria,
                reason: "Two explicit user requirements".into(),
                note: ymp_kernel::intake::IntakeNote::Assumption(Assumption {
                    text: "The retained file is the artifact".into(),
                    criterion: None,
                    reason: "Bound the local scenario".into(),
                }),
            },
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    s.intake
        .plans()
        .method(
            &s.control,
            MethodDecision {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                input: view.digest().unwrap(),
                proposal: method.choose(&view).unwrap(),
            },
            None,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let at = view.latest_at() + 1;
    s.intake
        .plans()
        .team(
            &s.control,
            view.revision(),
            at,
            Team {
                id: id("team"),
                session: s.session.clone(),
                revision: 1,
                members: vec![original.agent.clone(), independent.agent.clone()]
                    .into_iter()
                    .map(|agent| Membership {
                        agent,
                        joined: at,
                        left: None,
                        reason: "Initial independent members".into(),
                    })
                    .collect(),
            },
        )
        .unwrap();
    let old = check(&s, &authority, "old-check", b"impossible");
    let clock = Arc::new(ManualClock::new(at));
    let host = ExecutionHost::new(
        journal.clone(),
        gate.clone(),
        backend,
        Arc::new(
            ymp_runtime::policies::resources::PriceWeighted::from_selection(s.cost.selection())
                .unwrap(),
        ),
        clock.clone(),
    )
    .unwrap();
    let planner = admit(
        &s,
        &gate,
        "plan",
        ContributionKind::Plan,
        None,
        vec![],
        RoleKind::Planner,
    );
    let view = gate.view(&s.session).unwrap();
    let item = WorkItem {
        id: id("item"),
        plan: id("plan"),
        title: "Produce the retained artifact".into(),
        targets: view.criteria().iter().map(|c| c.id.clone()).collect(),
        deps: BTreeSet::new(),
        needs: BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]),
        writes: BTreeSet::from([WorkspacePath::new("file").unwrap()]),
        state: WorkState::Open,
        attempts: vec![],
        accepted: None,
        parent: None,
    };
    let plan = results
        .commit_plan(
            &planner.grant,
            view.revision(),
            view.latest_at(),
            Plan {
                id: id("plan"),
                session: s.session.clone(),
                version: 1,
                items: BTreeSet::from([item.id.clone()]),
                rationale: "Complete initial work scope".into(),
                author: id("plan"),
            },
            item.clone(),
        )
        .unwrap();
    begin_call(
        &s,
        &gate,
        &host,
        &clock,
        planner,
        "plan",
        Prompt {
            text: "Plan one bounded item".into(),
            basis: vec![plan],
        },
    );
    let producer = admit(
        &s,
        &gate,
        "produce",
        ContributionKind::Produce,
        Some(ContributionSubject::WorkItem(item.reference().unwrap())),
        vec![],
        RoleKind::Producer,
    );
    let (first, candidate) = produced(
        &s,
        &gate,
        &host,
        &clock,
        &results,
        producer,
        "produce",
        Prompt {
            text: "Produce the declared file".into(),
            basis: vec![item.reference().unwrap()],
        },
    );
    let error1 = run(
        &s,
        &authority,
        &old,
        "error-one",
        &candidate.after,
        CheckRunRole::Candidate,
        &Broken,
    );
    let mut context = scope(&s, &candidate, &old, error1.env.clone());
    monitor(&s, &delta);
    let error2 = run(
        &s,
        &authority,
        &old,
        "error-two",
        &candidate.after,
        CheckRunRole::Candidate,
        &Broken,
    );
    monitor(&s, &delta);
    assert!(
        gate.view(&s.session)
            .unwrap()
            .progress()
            .monitor()
            .unwrap()
            .2
            .outcome
            .stall
    );
    let diagnosis = diagnose(&s, &context, &rule, Diagnosis::Environment);
    let step = escalation(&s, &diagnosis, &ladder, EscalationStep::FixEnvironment);
    let view = gate.view(&s.session).unwrap();
    let repaired = progress
        .fix_environment(
            &s.control,
            &authority,
            &step,
            &error2.reference().unwrap(),
            id("repaired"),
            view.latest_at() + 1,
            &RetainedBytes,
        )
        .unwrap();
    assert_eq!(repaired.outcome, CheckOutcome::Fail);
    context = scope(&s, &candidate, &old, repaired.env.clone());
    select(&s, direct.selection());
    monitor(&s, &delta);
    let diagnosis = diagnose(&s, &context, &direct, Diagnosis::Unknown);
    select(&s, stop.selection());
    escalation(&s, &diagnosis, &stop, EscalationStep::StopPreserving);
    select(&s, ladder.selection());
    let diagnosis = diagnose(&s, &context, &direct, Diagnosis::Unknown);
    let step = escalation(&s, &diagnosis, &ladder, EscalationStep::AddVerifier);
    let view = gate.view(&s.session).unwrap();
    let input = treasury::estimate_view(
        &view,
        &ResourceDemand {
            contribution: id("research"),
            kind: ContributionKind::Research,
            difficulty: Difficulty::Simple,
            provider: id("provider"),
            profile: independent.clone(),
        },
    )
    .unwrap();
    let proposal = s.cost.estimate(&input).unwrap();
    let estimate = VerificationEstimate {
        decision: Decision {
            input: Digest::of_value(&input).unwrap(),
            outcome: proposal.value.clone(),
            proposal,
            effective: s.cost.selection().clone(),
            selection_change: None,
        },
        input,
    };
    let work = progress
        .add_verifier(
            &s.control,
            &step,
            independent.clone(),
            id("research"),
            estimate,
            view.latest_at() + 1,
        )
        .unwrap();
    s.profile = independent.clone();
    let award = s.award_existing("research", &work.contribution.id);
    let (revision, at, mut request) = s.request_with_gate(&gate, "research", award, "file");
    request.role = RoleKind::Researcher;
    let mut prepared = gate.prepare(&s.session, revision, at, request).unwrap();
    let admitted = gate.admit(&mut prepared).unwrap();
    begin_call(&s, &gate, &host, &clock, admitted, "research", work.prompt);
    run(
        &s,
        &authority,
        &old,
        "control-failure",
        &candidate.before,
        CheckRunRole::Control,
        &RetainedBytes,
    );
    select(&s, rule.selection());
    monitor(&s, &delta);
    let diagnosis = diagnose(&s, &context, &rule, Diagnosis::CheckDefect);
    let step = escalation(&s, &diagnosis, &ladder, EscalationStep::ReplaceCheck);
    let view = gate.view(&s.session).unwrap();
    let mut new = old.clone();
    new.id = id("replacement");
    new.spec = CheckSpec::ExactBytes {
        path: WorkspacePath::new("file").unwrap(),
        digest: Digest::of(b"fixed"),
    };
    new.version = new.content_version().unwrap();
    let proposal = ReplacementProposal {
        step: step.clone(),
        old: old.reference(),
        new: new.clone(),
        criterion: view.criteria()[0].reference().unwrap(),
        result: candidate.reference().unwrap(),
        contract: view.contract().unwrap().reference(),
        defect: vec![
            view.check_runs()[&id("control-failure")]
                .reference()
                .unwrap(),
        ],
        reason: "Replace the defective oracle with the full owner proposal".into(),
    };
    let staged = progress
        .propose_replacement(&s.control, view.revision(), view.latest_at() + 1, proposal)
        .unwrap();
    let reviewer = admit(
        &s,
        &gate,
        "replace-review",
        ContributionKind::Review,
        Some(ContributionSubject::ResultVersion(
            candidate.reference().unwrap(),
        )),
        vec![staged.clone()],
        RoleKind::Reviewer,
    );
    begin_call(
        &s,
        &gate,
        &host,
        &clock,
        reviewer,
        "replace-review",
        progress.replacement_prompt(&s.session, &staged).unwrap(),
    );
    let view = gate.view(&s.session).unwrap();
    progress
        .replace_check(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            &staged,
            &id("call-replace-review"),
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    assert!(!view.contract().unwrap().checks.contains(&old.id.erased()));
    assert!(view.checks().contains_key(&old.id));
    let failed = run(
        &s,
        &authority,
        &new,
        "new-failure",
        &candidate.after,
        CheckRunRole::Candidate,
        &RetainedBytes,
    );
    context = scope(&s, &candidate, &new, failed.env.clone());
    evidence(&s, &authority, &candidate, &failed, "failing-evidence");
    ledger(&s, &authority, &context, &belief);
    monitor(&s, &delta);
    let diagnosis = diagnose(&s, &context, &rule, Diagnosis::ArtifactDefect);
    let step = escalation(&s, &diagnosis, &ladder, EscalationStep::Retry);
    let view = gate.view(&s.session).unwrap();
    assert_eq!(
        progress
            .retry(
                &s.control,
                &step,
                &gate,
                &results,
                &first,
                id("retry"),
                &s.cost,
                view.latest_at()
            )
            .unwrap_err()
            .code,
        "retry_lease"
    );
    let retry_at = view.coordination().commitments()[&id("produce")]
        .lease
        .expires
        + 1;
    s.profile = original;
    let work = progress
        .retry(
            &s.control,
            &step,
            &gate,
            &results,
            &first,
            id("retry"),
            &s.cost,
            retry_at,
        )
        .unwrap();
    assert!(work.prompt.basis.contains(&failed.reference().unwrap()));
    let award = s.award_existing("retry", &work.contribution.id);
    let (revision, at, request) = s.request_with_gate(&gate, "retry", award, "file");
    let mut prepared = gate.prepare(&s.session, revision, at, request).unwrap();
    let retry = gate.admit(&mut prepared).unwrap();
    let (_, accepted_candidate) = produced(
        &s,
        &gate,
        &host,
        &clock,
        &results,
        retry,
        "retry",
        work.prompt,
    );
    assert!(
        progress
            .retry(
                &s.control,
                &step,
                &gate,
                &results,
                &first,
                id("forbidden-third"),
                &s.cost,
                gate.view(&s.session).unwrap().latest_at()
            )
            .is_err()
    );
    let passed = run(
        &s,
        &authority,
        &new,
        "passing-retry",
        &accepted_candidate.after,
        CheckRunRole::Candidate,
        &RetainedBytes,
    );
    context = scope(&s, &accepted_candidate, &new, passed.env.clone());
    evidence(
        &s,
        &authority,
        &accepted_candidate,
        &passed,
        "passing-evidence",
    );
    ledger(&s, &authority, &context, &belief);
    select(&s, accepted_only.selection());
    monitor(&s, &accepted_only);
    let view = gate.view(&s.session).unwrap();
    assert!(
        view.progress()
            .monitor()
            .unwrap()
            .2
            .outcome
            .record
            .progress
            .get()
            > 0.0
    );
    assert!(view.progress().monitor().unwrap().2.outcome.stall_count > 0);
    ledger(
        &s,
        &authority,
        &context,
        &StrongestSupport::standard().unwrap(),
    );
    select(&s, delta.selection());
    monitor(&s, &delta);
    ledger(&s, &authority, &context, &belief);
    monitor(&s, &delta);
    assert_eq!(progress.ledger(&s.session).unwrap().stall_count, 0);
    s.profile = independent;
    let reviewer = admit(
        &s,
        &gate,
        "final-review",
        ContributionKind::Review,
        Some(ContributionSubject::ResultVersion(
            accepted_candidate.reference().unwrap(),
        )),
        vec![],
        RoleKind::Reviewer,
    );
    let view = gate.view(&s.session).unwrap();
    authority
        .review(
            &gate,
            &reviewer.grant,
            ReviewRequest {
                expected_revision: view.revision(),
                at: view.latest_at(),
                id: id("approval"),
                result: accepted_candidate.reference().unwrap(),
                criteria: view
                    .criteria()
                    .iter()
                    .map(|c| c.reference().unwrap())
                    .collect(),
                verdict: ReviewVerdict::Approve,
                findings: vec![],
                basis: vec![id("passing-evidence")],
            },
        )
        .unwrap();
    begin_call(
        &s,
        &gate,
        &host,
        &clock,
        reviewer,
        "final-review",
        Prompt {
            text: "Review the exact retained result".into(),
            basis: vec![accepted_candidate.reference().unwrap()],
        },
    );
    let view = gate.view(&s.session).unwrap();
    let rules = AssessmentRules {
        mutation_threshold: Prob::new(0.8).unwrap(),
    };
    let acceptance = authority
        .acceptance_value(
            &s.session,
            id("accepted"),
            &context,
            &rules,
            view.latest_at() + 1,
        )
        .unwrap();
    assert_eq!(acceptance.decision, AcceptanceDecision::Accepted);
    let at = view.latest_at() + 1;
    authority
        .accept(
            &s.control,
            AcceptanceRequest {
                expected_revision: view.revision(),
                at,
                id: acceptance.id.clone(),
                context: context.clone(),
                rules,
                credit: CreditResponse {
                    input: credit_input(&acceptance).unwrap(),
                    proposal: credit.creditable(acceptance.grade).unwrap(),
                },
            },
            None,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    ymp_kernel::arbiter::Arbiter::new(journal.clone())
        .discharge(
            &gate,
            &s.session,
            view.revision(),
            at,
            &id("retry"),
            &acceptance.reference().unwrap(),
        )
        .unwrap();
    monitor(&s, &delta);
    let diagnosis = diagnose(&s, &context, &rule, Diagnosis::BudgetExhausted);
    select(&s, stop.selection());
    let step = escalation(&s, &diagnosis, &stop, EscalationStep::StopPreserving);
    let before = gate.view(&s.session).unwrap();
    assert_eq!(before.treasury().unwrap().budget.spent.get(), 60.0);
    progress
        .stop_preserving(
            &s.control,
            &step,
            &s.treasury,
            &s.budget_control,
            before.latest_at() + 1,
        )
        .unwrap();
    let after = gate.view(&s.session).unwrap();
    assert_eq!(
        after.treasury().unwrap().reporting_mode,
        Some(ReportingMode::Deterministic)
    );
    assert_eq!(
        after.results().items()[&id("item")].accepted.as_ref(),
        Some(&accepted_candidate.id)
    );
    assert_eq!(after.results().results().len(), 2);
    assert_eq!(
        after.treasury().unwrap().budget.spent,
        before.treasury().unwrap().budget.spent
    );
    assert_eq!(
        authority.ledger(&s.session).unwrap().entries[&id("inspection")].status,
        LedgerStatus::Unmet
    );
}

#[test]
fn profile_growth_and_cross_session_reporting_cannot_bypass_owner_boundaries() {
    use ymp_kernel::ports::resources::ResourcePolicy;
    use ymp_runtime::memory_journal::MemoryJournal;
    let root = Directory::new();
    let database = Directory::new();
    let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let journal = Arc::new(MemoryJournal::new());
    let monitor_policy = EvidenceDelta::new(MonitorParameters {
        epsilon: real(0.001),
        stall_limit: 1,
    })
    .unwrap();
    let diagnoser = RuleBasedDiagnoser::new(DiagnosisParameters {
        p_min: Prob::new(0.2).unwrap(),
        mutation_threshold: Prob::new(0.8).unwrap(),
    })
    .unwrap();
    let escalation_policy = DiagnosisFirstLadder::new(EscalationParameters {
        max_uses: 1,
        max_cost: real(20.0),
        timeout_ms: 1000,
    })
    .unwrap();
    let method = FixedMethod::new(MethodKind::Solo).unwrap();
    let mut s = Setup::new_with_selections(
        journal.clone(),
        &store,
        &root,
        false,
        "guard-session",
        None,
        FirstOffer::with_commitment_terms(CommitmentTerms {
            lease_duration: 20,
            renewal_duration: 10,
            renew_on: BTreeSet::new(),
            renewals: 0,
            release_delta: real(1.0),
        })
        .unwrap(),
        vec![
            monitor_policy.selection().clone(),
            diagnoser.selection().clone(),
            escalation_policy.selection().clone(),
            method.selection().clone(),
        ],
    );
    let view = s.gate.view(&s.session).unwrap();
    s.intake
        .plans()
        .method(
            &s.control,
            MethodDecision {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                input: view.digest().unwrap(),
                proposal: method.choose(&view).unwrap(),
            },
            None,
        )
        .unwrap();
    let award = s.award("original");
    let (revision, at, request) = s.request("original", award);
    let mut prepared = s.gate.prepare(&s.session, revision, at, request).unwrap();
    let original = s.gate.admit(&mut prepared).unwrap();
    let view = s.gate.view(&s.session).unwrap();
    let expires = view.coordination().commitments()[&id("original")]
        .lease
        .expires
        + 1;
    ymp_kernel::arbiter::Arbiter::new(journal.clone())
        .tick(&s.gate, &s.session, expires)
        .unwrap();
    let view = s.gate.view(&s.session).unwrap();
    let proof = s
        .treasury
        .never_started(&s.session, &id("original"))
        .unwrap();
    s.treasury
        .release_unstarted(&s.session, view.revision(), view.latest_at(), &proof)
        .unwrap();
    let view = s.gate.view(&s.session).unwrap();
    let prior = view.registry().unwrap().clone();
    let mut facts = prior.input.facts;
    let mut offering = facts.discoveries[0].offerings[0].clone();
    offering.model = Some("larger-offered-model".into());
    facts.discoveries[0].offerings.push(offering);
    let registry = ymp_kernel::registry::Registry::new(journal.clone());
    let input = registry
        .prepare(&s.session, facts, view.latest_at() + 1)
        .unwrap();
    let responses = ymp_kernel::registry::readiness_views(&input)
        .iter()
        .map(|input| ymp_kernel::registry::ReadinessResponse {
            profile: input.profile.clone(),
            input: Digest::of_value(input).unwrap(),
            proposal: Proposal {
                value: ymp_domain::identity::Readiness::Ready,
                rationale: "Both Scripted profiles are actually offered in this fixture".into(),
                basis: vec![],
                policy: prior.effective.policy.clone(),
            },
        })
        .collect();
    registry
        .record(
            &s.session,
            view.revision(),
            view.latest_at() + 1,
            input,
            prior.effective,
            responses,
        )
        .unwrap();
    s.profile.model = "larger-offered-model".into();
    let award = s.award("growth");
    let (revision, at, request) = s.request("growth", award);
    let mut prepared = s.gate.prepare(&s.session, revision, at, request).unwrap();
    assert_eq!(
        s.gate.admit(&mut prepared).err().unwrap().code,
        "diagnosis_required"
    );
    s.profile = original.assignment.profile;
    let progress = s.intake.progress();
    let view = s.gate.view(&s.session).unwrap();
    let input = progress.input(&s.session, view.latest_at() + 1).unwrap();
    let monitored = progress
        .assess(
            &s.control,
            MonitorRequest {
                expected_revision: view.revision(),
                proposal: monitor_policy.assess(&input).unwrap(),
                input,
            },
        )
        .unwrap();
    let view = s.gate.view(&s.session).unwrap();
    let input = progress
        .diagnosis_input(&s.session, None, &[], view.latest_at() + 1)
        .unwrap();
    assert_eq!(
        diagnoser.diagnose(&input).unwrap().value,
        Diagnosis::BudgetExhausted
    );
    let diagnosis = progress
        .diagnose(
            &s.control,
            DiagnosisRequest {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                progress: monitored,
                context: None,
                estimates: vec![],
                proposal: diagnoser.diagnose(&input).unwrap(),
                input,
            },
        )
        .unwrap();
    let view = s.gate.view(&s.session).unwrap();
    let input = progress.escalation_input(&s.session, &diagnosis).unwrap();
    let step = progress
        .escalate(
            &s.control,
            EscalationRequest {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                proposal: escalation_policy.next(&input).unwrap(),
                input,
            },
        )
        .unwrap();
    let other = Setup::new_named(journal.clone(), &store, &root, false, "other-budget");
    let before = s.gate.view(&s.session).unwrap();
    assert_eq!(
        progress
            .stop_preserving(
                &s.control,
                &step,
                &other.treasury,
                &other.budget_control,
                before.latest_at() + 1
            )
            .unwrap_err()
            .code,
        "owner_authority"
    );
    assert!(
        other
            .gate
            .view(&other.session)
            .unwrap()
            .treasury()
            .unwrap()
            .reporting_mode
            .is_none()
    );
    assert_eq!(s.gate.view(&s.session).unwrap(), before);
    progress
        .stop_preserving(
            &s.control,
            &step,
            &s.treasury,
            &s.budget_control,
            before.latest_at() + 1,
        )
        .unwrap();
    let view = s.gate.view(&s.session).unwrap();
    assert!(
        view.treasury()
            .unwrap()
            .remaining(Purpose::Production)
            .unwrap()
            .get()
            > 10.0
    );
    let demand = ResourceDemand {
        contribution: id("forbidden"),
        kind: ContributionKind::Plan,
        difficulty: Difficulty::Simple,
        provider: id("provider"),
        profile: s.profile.clone(),
    };
    let input = treasury::estimate_view(&view, &demand).unwrap();
    let estimate = s.cost.estimate(&input).unwrap();
    let allowance_input =
        treasury::allowance_view(&view, &demand, estimate.value.clone(), view.latest_at() + 1)
            .unwrap();
    let allowance = s.resource.allowance(&allowance_input).unwrap();
    assert_eq!(
        s.treasury
            .reserve(
                &s.session,
                view.revision(),
                view.latest_at() + 1,
                treasury::ReserveRequest {
                    id: id("forbidden"),
                    assignment: id("forbidden"),
                    demand,
                    estimate: ymp_kernel::ports::resources::ResourceResponse {
                        input: Digest::of_value(&input).unwrap(),
                        proposal: estimate
                    },
                    allowance: ymp_kernel::ports::resources::ResourceResponse {
                        input: Digest::of_value(&allowance_input).unwrap(),
                        proposal: allowance
                    }
                }
            )
            .unwrap_err()
            .code,
        "reporting_only"
    );
    assert_eq!(s.gate.view(&s.session).unwrap(), view);
}

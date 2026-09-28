//! Real retained candidate, final rechecks and independent paid final review.
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
    journal::{Capability, MethodKind, PolicySelection},
    plan::*,
    report::*,
    resources::*,
    task::Real,
    verification::*,
    workspace::*,
};
use ymp_kernel::{
    acceptance::*,
    decision::MethodDecision,
    finalization::*,
    gatekeeper::{AdmittedAssignment, Gatekeeper},
    journal::ParameterSchemas,
    ports::{
        checks::CheckRunner,
        execution::*,
        experience::CreditPolicy,
        planning::{BeliefModel, BeliefResponse, MethodRouter},
        reporting::*,
        resources::CostModel,
    },
    results::Results,
};
use ymp_runtime::{
    backends::scripted::{Scripted, ScriptedStep},
    checks::retained::RetainedBytes,
    clock::ManualClock,
    execution_host::{ExecutionHost, ExecutionStatus},
    policies::{
        award::FirstOffer,
        belief::LikelihoodRatioTable,
        claim_audit::{ConservativeAudit, EvidenceClassRules},
        context::{CompactContext, CriteriaProjection},
        credit::ConfirmedOnly,
        method::FixedMethod,
        narrative::{DeterministicReport, Narrator},
        reviewer::{AnyNonProducer, LeastUsedReviewer},
    },
};
use ymp_storage::{content::SqliteContent, journal::SqliteJournal};
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn real(n: f64) -> Real {
    Real::new(n).unwrap()
}
type Gate = Gatekeeper<SqliteJournal, SqliteContent>;
type Host = ExecutionHost<SqliteJournal, SqliteContent>;
struct Backend {
    selection: PolicySelection,
    runs: Mutex<BTreeMap<Id<Invocation>, Scripted>>,
}
impl Backend {
    fn new() -> Self {
        Self {
            selection: PolicySelection::new(
                "ExecutionBackend",
                "FinalReportFixture",
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
        match r.assignment.role {
            RoleKind::Producer => steps.push(ScriptedStep::Write {
                path: WorkspacePath::new("file")?,
                bytes: b"candidate".to_vec(),
            }),
            RoleKind::FinalReviewer => {
                let prompt: serde_json::Value = serde_json::from_str(&r.prompt.text).unwrap();
                let aggregate: FinalAggregate = serde_json::from_value(
                    prompt.as_array().unwrap().last().unwrap()["aggregate"].clone(),
                )
                .unwrap();
                steps.push(ScriptedStep::Emit(BackendObservation::Output(
                    serde_json::to_string(&FinalVerdict {
                        id: id("final-review"),
                        aggregate: aggregate.reference()?,
                        verdict: ReviewVerdict::Approve,
                        basis: vec![],
                        rationale:
                            "Review the exact retained final scope and its rerun observations"
                                .into(),
                    })
                    .unwrap(),
                )));
            }
            RoleKind::Narrator if r.assignment.id.as_str() == "failed-narration" => {
                steps.push(ScriptedStep::Emit(BackendObservation::Usage {
                    usage: Usage {
                        input: 2,
                        cache_read: 0,
                        cache_write: 0,
                        output: 0,
                        reasoning: None,
                    },
                    turns: 1,
                }));
                steps.push(ScriptedStep::Emit(BackendObservation::Terminal(
                    InvocationTerminal::Failed(ErrorClass::Content),
                )));
            }
            RoleKind::Narrator => {
                let prompt: serde_json::Value = serde_json::from_str(&r.prompt.text).unwrap();
                let purpose = prompt.as_array().unwrap().last().unwrap();
                let facts: NarrativeInput =
                    serde_json::from_value(purpose["input"].clone()).unwrap();
                let runs: Vec<CheckRun> = serde_json::from_value(purpose["runs"].clone()).unwrap();
                let candidate = runs
                    .iter()
                    .find(|r| r.role == CheckRunRole::Candidate)
                    .unwrap();
                let baseline = runs
                    .iter()
                    .find(|r| r.role == CheckRunRole::Baseline)
                    .unwrap();
                let claims = if r.assignment.id.as_str() == "narration" {
                    vec![DraftClaim{text:"The candidate program runs.".into(),assertion:Assertion::Executed{run:candidate.reference()?,program:true}},DraftClaim{text:"The recorded change changed this check from failing on its baseline to passing on the integrated snapshot.".into(),assertion:Assertion::Causal{before:baseline.reference()?,after:candidate.reference()?}}]
                } else {
                    let acceptance = facts.final_acceptance.unwrap();
                    vec![
                        DraftClaim {
                            text: format!(
                                "Final result accepted; confirmation grade: {:?}.",
                                acceptance.grade
                            ),
                            assertion: Assertion::Accepted {
                                acceptance: acceptance.reference()?,
                            },
                        },
                        DraftClaim {
                            text: "All cases always work.".into(),
                            assertion: Assertion::Scope { universal: true },
                        },
                    ]
                };
                steps.push(ScriptedStep::Emit(BackendObservation::Output(
                    serde_json::to_string(&ReportDraft { claims }).unwrap(),
                )));
            }
            _ => steps.push(ScriptedStep::Emit(BackendObservation::Output(
                "Bounded work complete".into(),
            ))),
        }
        steps.push(ScriptedStep::Complete {
            usage: Usage {
                input: 2,
                cache_read: 0,
                cache_write: 0,
                output: 0,
                reasoning: None,
            },
            coverage: Coverage::Complete,
        });
        let backend = Scripted::new(steps)?;
        let started = backend.start(r)?;
        self.runs
            .lock()
            .unwrap()
            .insert(r.invocation.clone(), backend);
        Ok(started)
    }
    fn cancel(&self, h: &ExecutionHandle) -> Result<()> {
        self.runs.lock().unwrap()[&h.invocation].cancel(h)
    }
    fn events(&self, h: &ExecutionHandle, a: u64, l: usize) -> Result<Vec<BackendEvent>> {
        self.runs.lock().unwrap()[&h.invocation].events(h, a, l)
    }
    fn receipt(&self, h: &ExecutionHandle) -> Result<Receipt> {
        let mut receipt = self.runs.lock().unwrap()[&h.invocation].receipt(h)?;
        if h.invocation.as_str() == "call-failed-narration" {
            receipt.coverage = Coverage::Complete;
        }
        Ok(receipt)
    }
}
fn execute(
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
    let mut live = host
        .attach(
            admitted,
            id(&format!("call-{name}")),
            id(&format!("receipt-{name}")),
            prompt,
        )
        .unwrap();
    host.start(&mut live).unwrap();
    let limit = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        let status = host.poll(&mut live).unwrap();
        if status == ExecutionStatus::Finished {
            break;
        }
        assert!(
            std::time::Instant::now() < limit,
            "{name}: {status:?}; {:?}",
            host.snapshot(&live).unwrap().diagnostics
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
#[allow(clippy::too_many_arguments)]
fn admitted(
    s: &Setup<SqliteJournal>,
    gate: &Arc<Gate>,
    name: &str,
    kind: ContributionKind,
    role: RoleKind,
    subject: Option<ContributionSubject>,
    basis: Vec<Ref>,
    readonly: bool,
) -> AdmittedAssignment<SqliteJournal> {
    let view = gate.view(&s.session).unwrap();
    let needs = if readonly {
        BTreeSet::from([Capability::ReadFiles])
    } else {
        BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles])
    };
    let arbiter = ymp_kernel::arbiter::Arbiter::new(s.journal.clone());
    arbiter
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
                needs: needs.clone(),
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
    let award = s.award_existing(name, &id(name));
    let (revision, at, mut request) = s.request_with_files(gate, name, award, "file", !readonly);
    request.role = role;
    if readonly {
        request.access = needs;
        request.files = Some(
            gate.workspace()
                .prepare_mediation(
                    &s.session,
                    revision,
                    at,
                    ymp_kernel::workspace_guard::LockRequest {
                        assignment: id(name),
                        workspace: id("workspace"),
                        profile: s.profile.clone(),
                        paths: vec![(WorkspacePath::root(), LockMode::Read)],
                    },
                    s.provider.clone(),
                )
                .unwrap(),
        );
    }
    let mut prepared = gate.prepare(&s.session, revision, at, request).unwrap();
    gate.admit(&mut prepared).unwrap()
}
#[test]
fn final_scope_requires_fresh_evidence_and_an_independent_paid_review() {
    let root = Directory::new();
    let database = Directory::new();
    std::fs::write(root.0.join("file"), b"base").unwrap();
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("ExecutionBackend", "FinalReportFixture", "1", |_| Ok(()))
        .unwrap();
    let journal = Arc::new(SqliteJournal::open(database.database(), schemas).unwrap());
    let backend = Arc::new(Backend::new());
    let belief = LikelihoodRatioTable::standard().unwrap();
    let credit = ConfirmedOnly::new().unwrap();
    let method = FixedMethod::new(MethodKind::SoloWithVerifier).unwrap();
    let composer = CriteriaProjection::new(4).unwrap();
    let compact = CompactContext::new(4).unwrap();
    let narrator = Narrator::new().unwrap();
    let auditor = EvidenceClassRules::new().unwrap();
    let conservative = ConservativeAudit::new().unwrap();
    let first_reviewer = AnyNonProducer::new().unwrap();
    let least = LeastUsedReviewer::new().unwrap();
    let mut constraints = fixture::default_constraints(BTreeSet::from([
        Capability::ReadFiles,
        Capability::WriteFiles,
    ]));
    constraints.max_members = 3;
    let mut s = Setup::new_with_selections(
        journal.clone(),
        &journal,
        &root,
        true,
        "final",
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
            belief.selection().clone(),
            credit.selection().clone(),
            method.selection().clone(),
            composer.selection().clone(),
            first_reviewer.selection().clone(),
            narrator.selection().clone(),
            auditor.selection().clone(),
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
    let producer = s.profile.clone();
    s.switch_to_new_agent("reviewer-a");
    let reviewer = s.profile.clone();
    s.switch_to_new_agent("reviewer-b");
    let final_profile = s.profile.clone();
    s.profile = producer.clone();
    let authority = s.intake.acceptance(Arc::new(journal.content_store()));
    let finalizer = s.intake.finalization(Arc::new(journal.content_store()));
    let results = Results::new(journal.clone(), gate.clone()).unwrap();
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
                members: vec![
                    producer.agent.clone(),
                    reviewer.agent.clone(),
                    final_profile.agent.clone(),
                ]
                .into_iter()
                .map(|agent| Membership {
                    agent,
                    joined: at,
                    left: None,
                    reason: "Initial independent review members".into(),
                })
                .collect(),
            },
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let mut check = Check {
        id: id("check"),
        criterion: id("criterion"),
        criterion_version: view.criteria()[0].reference().unwrap().version,
        spec: CheckSpec::ExactBytes {
            path: WorkspacePath::new("file").unwrap(),
            digest: Digest::of(b"candidate"),
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
                    value: check.clone(),
                    rationale: "Explicit content check".into(),
                    basis: vec![],
                    policy: policy.policy.clone(),
                },
                effective: policy,
            },
        )
        .unwrap();
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
    let planner = admitted(
        &s,
        &gate,
        "plan",
        ContributionKind::Plan,
        RoleKind::Planner,
        None,
        vec![],
        false,
    );
    let view = gate.view(&s.session).unwrap();
    let item = WorkItem {
        id: id("item"),
        plan: id("plan"),
        title: "Produce one retained file".into(),
        targets: BTreeSet::from([id("criterion")]),
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
                rationale: "Initial explicit graph".into(),
                author: id("plan"),
            },
            item.clone(),
        )
        .unwrap();
    execute(
        &s,
        &gate,
        &host,
        &clock,
        planner,
        "plan",
        Prompt {
            text: "Commit the graph".into(),
            basis: vec![plan],
        },
    );
    let admitted_producer = admitted(
        &s,
        &gate,
        "produce",
        ContributionKind::Produce,
        RoleKind::Producer,
        Some(ContributionSubject::WorkItem(item.reference().unwrap())),
        vec![],
        false,
    );
    let at = gate.view(&s.session).unwrap().latest_at();
    gate.workspace()
        .snapshot_unstarted(admitted_producer.files.as_ref().unwrap(), id("before"), at)
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let attempt = results
        .prepare_attempt(
            &admitted_producer.grant,
            view.revision(),
            at,
            id("attempt"),
            id("before"),
            id("after"),
        )
        .unwrap();
    results.begin(&attempt).unwrap();
    execute(
        &s,
        &gate,
        &host,
        &clock,
        admitted_producer,
        "produce",
        Prompt {
            text: "Produce the file".into(),
            basis: vec![item.reference().unwrap()],
        },
    );
    let at = gate.view(&s.session).unwrap().latest_at();
    let result = results
        .submit(
            &attempt,
            at,
            id("candidate"),
            BTreeSet::from([ymp_domain::result::Artifact {
                path: WorkspacePath::new("file").unwrap(),
                digest: Digest::of(b"candidate"),
            }]),
            "Retained candidate".into(),
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let run = authority
        .run(
            &s.control,
            RunCheck {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                id: id("candidate-check"),
                check: check.reference(),
                target: result.after.clone(),
                role: CheckRunRole::Candidate,
            },
            &RetainedBytes,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let evidence = authority
        .evidence(
            &s.control,
            EvidenceRequest {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                id: id("candidate-evidence"),
                criterion: view.criteria()[0].reference().unwrap(),
                result: result.reference().unwrap(),
                runs: vec![run.id.clone()],
                reviews: vec![],
            },
        )
        .unwrap();
    s.profile = reviewer;
    let reviewed = admitted(
        &s,
        &gate,
        "candidate-review",
        ContributionKind::Review,
        RoleKind::Reviewer,
        Some(ContributionSubject::ResultVersion(
            result.reference().unwrap(),
        )),
        vec![],
        true,
    );
    let view = gate.view(&s.session).unwrap();
    authority
        .review(
            &gate,
            &reviewed.grant,
            ReviewRequest {
                expected_revision: view.revision(),
                at: view.latest_at(),
                id: id("candidate-review"),
                result: result.reference().unwrap(),
                criteria: vec![view.criteria()[0].reference().unwrap()],
                verdict: ReviewVerdict::Approve,
                findings: vec![],
                basis: vec![evidence.evidence.id.clone()],
            },
        )
        .unwrap();
    execute(
        &s,
        &gate,
        &host,
        &clock,
        reviewed,
        "candidate-review",
        Prompt {
            text: "Review the retained candidate".into(),
            basis: vec![result.reference().unwrap()],
        },
    );
    let view = gate.view(&s.session).unwrap();
    let context = ApplicabilityContext {
        result: result.reference().unwrap(),
        criteria: BTreeSet::from([view.criteria()[0].reference().unwrap()]),
        environments: BTreeMap::from([(check.reference(), BTreeSet::from([run.env]))]),
    };
    let rules = AssessmentRules {
        mutation_threshold: Prob::new(0.8).unwrap(),
    };
    let at = view.latest_at() + 1;
    let acceptance = authority
        .acceptance_value(&s.session, id("candidate-accepted"), &context, &rules, at)
        .unwrap();
    authority
        .accept(
            &s.control,
            AcceptanceRequest {
                expected_revision: view.revision(),
                at,
                id: acceptance.id.clone(),
                context,
                rules: rules.clone(),
                credit: CreditResponse {
                    input: credit_input(&acceptance).unwrap(),
                    proposal: credit.creditable(acceptance.grade).unwrap(),
                },
            },
            None,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    finalizer
        .control(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            Continuation::Continue,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let aggregate = finalizer
        .capture(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            id("workspace"),
            id("integrated"),
            vec![FinalCheck {
                check: check.reference(),
                environment: RetainedBytes.environment().unwrap(),
            }],
            s.provider.as_ref(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(aggregate.producers, BTreeSet::from([producer.agent]));
    let context = ymp_kernel::finalization::context(&aggregate).unwrap();
    let view = gate.view(&s.session).unwrap();
    assert!(applicable_evidence(&view, &context).unwrap().is_empty());
    assert!(
        finalizer
            .final_acceptance_value(&s.session, id("too-early"), &rules, view.latest_at())
            .is_err()
    );
    finalizer
        .recheck(&s.control, view.latest_at() + 1, &RetainedBytes, true)
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let input = finalizer.reviewer_input(&s.session).unwrap();
    let first = first_reviewer.pick(&input).unwrap();
    assert_eq!(first.value, Some(id("reviewer-a")));
    finalizer
        .record_reviewer(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            input,
            first,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    finalizer
        .select(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            least.selection().clone(),
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let input = finalizer.reviewer_input(&s.session).unwrap();
    let chosen = least.pick(&input).unwrap();
    assert_eq!(chosen.value, Some(final_profile.agent.clone()));
    let choice = finalizer
        .record_reviewer(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            input,
            chosen,
        )
        .unwrap();
    s.profile = final_profile;
    let final_review = admitted(
        &s,
        &gate,
        "final-review",
        ContributionKind::Review,
        RoleKind::FinalReviewer,
        None,
        vec![aggregate.reference().unwrap(), choice],
        true,
    );
    let input = finalizer
        .context_input(&s.session, &final_review.assignment.id)
        .unwrap();
    assert!(
        input
            .retained
            .iter()
            .any(|r| r.bytes.as_deref() == Some(b"candidate"))
    );
    assert_ne!(
        composer.prompt(&input).unwrap().value.text,
        compact.prompt(&input).unwrap().value.text
    );
    let view = gate.view(&s.session).unwrap();
    let prompt = finalizer
        .record_context(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            input.clone(),
            composer.prompt(&input).unwrap(),
        )
        .unwrap();
    execute(
        &s,
        &gate,
        &host,
        &clock,
        final_review,
        "final-review",
        prompt,
    );
    let view = gate.view(&s.session).unwrap();
    let review = finalizer
        .record_review(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            &id("call-final-review"),
        )
        .unwrap();
    assert!(!aggregate.producers.contains(&review.reviewer));
    let view = gate.view(&s.session).unwrap();
    let at = view.latest_at() + 1;
    let responses = authority
        .ledger_inputs(&s.session, &context, &rules, at)
        .unwrap()
        .into_iter()
        .map(|(id, input)| {
            (
                id,
                BeliefResponse {
                    input: Digest::of_value(&input).unwrap(),
                    proposal: belief.update(&input).unwrap(),
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
                context: context.clone(),
                rules: rules.clone(),
                responses,
            },
            None,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let acceptance = finalizer
        .final_acceptance_value(
            &s.session,
            id("final-accepted"),
            &rules,
            view.latest_at() + 1,
        )
        .unwrap();
    assert_eq!(acceptance.decision, AcceptanceDecision::Accepted);
    assert_eq!(
        acceptance.subject,
        AcceptanceSubject::FinalAggregate(s.session.clone())
    );
    let at = view.latest_at() + 1;
    authority
        .finalize(
            &s.control,
            AcceptanceRequest {
                expected_revision: view.revision(),
                at,
                id: acceptance.id.clone(),
                context,
                rules,
                credit: CreditResponse {
                    input: credit_input(&acceptance).unwrap(),
                    proposal: credit.creditable(acceptance.grade).unwrap(),
                },
            },
        )
        .unwrap();
    assert_eq!(
        gate.view(&s.session)
            .unwrap()
            .finalization()
            .acceptance
            .as_ref()
            .unwrap()
            .acceptance,
        acceptance
    );
    let view = gate.view(&s.session).unwrap();
    finalizer
        .prepare_report(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            id("report"),
            &s.treasury,
            &s.budget_control,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    finalizer
        .select(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            compact.selection().clone(),
        )
        .unwrap();
    for (name, correcting) in [("narration", false), ("correction", true)] {
        let view = gate.view(&s.session).unwrap();
        let work = finalizer
            .narration_work(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                id(name),
                s.profile.clone(),
                correcting,
                &s.cost,
            )
            .unwrap();
        let award = s.award_existing(name, &work.contribution.id);
        let (revision, at, mut request) = s.request_with_files(&gate, name, award, "file", false);
        request.role = RoleKind::Narrator;
        request.access = BTreeSet::from([Capability::ReadFiles]);
        request.files = Some(
            gate.workspace()
                .prepare_mediation(
                    &s.session,
                    revision,
                    at,
                    ymp_kernel::workspace_guard::LockRequest {
                        assignment: id(name),
                        workspace: id("workspace"),
                        profile: s.profile.clone(),
                        paths: vec![(WorkspacePath::root(), LockMode::Read)],
                    },
                    s.provider.clone(),
                )
                .unwrap(),
        );
        let mut prepared = gate.prepare(&s.session, revision, at, request).unwrap();
        let admitted = gate.admit(&mut prepared).unwrap();
        let input = finalizer
            .context_input(&s.session, &admitted.assignment.id)
            .unwrap();
        let view = gate.view(&s.session).unwrap();
        let prompt = finalizer
            .record_context(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                input.clone(),
                compact.prompt(&input).unwrap(),
            )
            .unwrap();
        execute(&s, &gate, &host, &clock, admitted, name, prompt);
        let invocation = id(&format!("call-{name}"));
        let input = finalizer
            .narrative_input(&s.session, Some(&invocation))
            .unwrap();
        let view = gate.view(&s.session).unwrap();
        finalizer
            .record_narrative(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                input.clone(),
                Some(invocation),
                narrator.compose(&input).unwrap(),
                None,
            )
            .unwrap();
        let inputs = finalizer.claim_inputs(&s.session).unwrap();
        let proposals = inputs
            .iter()
            .map(|i| {
                if correcting {
                    conservative.audit(i).unwrap()
                } else {
                    auditor.audit(i).unwrap()
                }
            })
            .collect::<Vec<_>>();
        if !correcting {
            assert!(matches!(proposals[0].value, ClaimAudit::Unsupported(_)));
            assert_eq!(proposals[1].value, ClaimAudit::Valid);
            // Pure policy guard over the actual consumer input: even retained runs
            // cannot establish causality when their execution environments differ.
            let mut mismatched = inputs[1].clone();
            let baseline = mismatched
                .runs
                .iter_mut()
                .find(|run| run.role == CheckRunRole::Baseline)
                .unwrap();
            baseline.env = Digest::of(b"different-environment");
            let replacement = baseline.reference().unwrap();
            if let Assertion::Causal { before, .. } = &mut mismatched.claim.assertion {
                *before = replacement;
            }
            assert!(matches!(
                auditor.audit(&mismatched).unwrap().value,
                ClaimAudit::Unsupported(_)
            ));
        }
        let view = gate.view(&s.session).unwrap();
        finalizer
            .audit(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                inputs,
                proposals,
            )
            .unwrap();
        if !correcting {
            let view = gate.view(&s.session).unwrap();
            finalizer
                .select(
                    &s.control,
                    view.revision(),
                    view.latest_at() + 1,
                    conservative.selection().clone(),
                )
                .unwrap();
            let inputs = finalizer.claim_inputs(&s.session).unwrap();
            let proposals = inputs
                .iter()
                .map(|i| conservative.audit(i).unwrap())
                .collect::<Vec<_>>();
            assert!(
                proposals
                    .iter()
                    .all(|p| matches!(p.value, ClaimAudit::Unsupported(_)))
            );
            let view = gate.view(&s.session).unwrap();
            finalizer
                .audit(
                    &s.control,
                    view.revision(),
                    view.latest_at() + 1,
                    inputs,
                    proposals,
                )
                .unwrap();
        }
    }
    let view = gate.view(&s.session).unwrap();
    assert!(
        finalizer
            .narration_work(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                id("forbidden-third"),
                s.profile.clone(),
                true,
                &s.cost
            )
            .is_err()
    );
    let report = finalizer
        .deliver(&s.control, view.revision(), view.latest_at() + 1)
        .unwrap();
    assert_eq!(report.accounting.spent.get(), 12.0);
    assert!(!report.accounting.unknown);
    assert_eq!(
        report.report.grade,
        ConfirmationGrade::Confirmed(ConfirmationBasis::TrustedCheck)
    );
    assert!(report.report.unmet.is_empty());
    assert!(
        report
            .claims
            .iter()
            .any(|c| c.text.starts_with("Uncertain:"))
    );
    assert!(report.claims.iter().all(|c| c.audit == ClaimAudit::Valid));
    assert!(render_report(&report).contains("Cost units settled: 12"));
    assert_eq!(report.retained, vec![result.reference().unwrap()]);
}

#[test]
fn failed_narrator_delivers_accounted_facts_and_pending_work_obeys_policy() {
    let root = Directory::new();
    let database = Directory::new();
    std::fs::write(root.0.join("file"), b"base").unwrap();
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("ExecutionBackend", "FinalReportFixture", "1", |_| Ok(()))
        .unwrap();
    let journal = Arc::new(SqliteJournal::open(database.database(), schemas).unwrap());
    let backend = Arc::new(Backend::new());
    let composer = CompactContext::new(2).unwrap();
    let narrator = Narrator::new().unwrap();
    let deterministic = DeterministicReport::new().unwrap();
    let auditor = EvidenceClassRules::new().unwrap();
    let mut s = Setup::new_with_selections(
        journal.clone(),
        &journal,
        &root,
        true,
        "failed-report",
        None,
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
            composer.selection().clone(),
            narrator.selection().clone(),
            auditor.selection().clone(),
        ],
    );
    let gate = Arc::new(std::mem::replace(
        &mut s.gate,
        Gatekeeper::new(journal.clone(), Arc::new(journal.content_store())),
    ));
    let finalizer = s.intake.finalization(Arc::new(journal.content_store()));
    let view = gate.view(&s.session).unwrap();
    finalizer
        .control(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            Continuation::Continue,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    finalizer
        .prepare_report(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            id("failed-report"),
            &s.treasury,
            &s.budget_control,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let work = finalizer
        .narration_work(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            id("failed-narration"),
            s.profile.clone(),
            false,
            &s.cost,
        )
        .unwrap();
    let award = s.award_existing("failed-narration", &work.contribution.id);
    let view = gate.view(&s.session).unwrap();
    finalizer
        .select(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            deterministic.selection().clone(),
        )
        .unwrap();
    let (revision, at, mut denied) =
        s.request_with_files(&gate, "failed-narration", award.clone(), "file", false);
    denied.role = RoleKind::Narrator;
    denied.access = BTreeSet::from([Capability::ReadFiles]);
    denied.files = Some(
        gate.workspace()
            .prepare_mediation(
                &s.session,
                revision,
                at,
                ymp_kernel::workspace_guard::LockRequest {
                    assignment: id("failed-narration"),
                    workspace: id("workspace"),
                    profile: s.profile.clone(),
                    paths: vec![(WorkspacePath::root(), LockMode::Read)],
                },
                s.provider.clone(),
            )
            .unwrap(),
    );
    let mut denied = gate.prepare(&s.session, revision, at, denied).unwrap();
    assert_eq!(
        gate.admit(&mut denied).err().unwrap().code,
        "narration_authority"
    );
    let view = gate.view(&s.session).unwrap();
    finalizer
        .select(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            narrator.selection().clone(),
        )
        .unwrap();
    let award = s.award_existing("retry-award", &work.contribution.id);
    let (revision, at, mut request) =
        s.request_with_files(&gate, "failed-narration", award, "file", false);
    request.role = RoleKind::Narrator;
    request.access = BTreeSet::from([Capability::ReadFiles]);
    request.files = Some(
        gate.workspace()
            .prepare_mediation(
                &s.session,
                revision,
                at,
                ymp_kernel::workspace_guard::LockRequest {
                    assignment: id("failed-narration"),
                    workspace: id("workspace"),
                    profile: s.profile.clone(),
                    paths: vec![(WorkspacePath::root(), LockMode::Read)],
                },
                s.provider.clone(),
            )
            .unwrap(),
    );
    let mut prepared = gate.prepare(&s.session, revision, at, request).unwrap();
    let admitted = gate.admit(&mut prepared).unwrap();
    let input = finalizer
        .context_input(&s.session, &admitted.assignment.id)
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let prompt = finalizer
        .record_context(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            input.clone(),
            composer.prompt(&input).unwrap(),
        )
        .unwrap();
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
    execute(
        &s,
        &gate,
        &host,
        &clock,
        admitted,
        "failed-narration",
        prompt,
    );
    assert!(
        finalizer
            .narrative_input(&s.session, Some(&id("call-failed-narration")))
            .is_err()
    );
    let input = finalizer.narrative_input(&s.session, None).unwrap();
    let view = gate.view(&s.session).unwrap();
    finalizer
        .record_narrative(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            input.clone(),
            None,
            deterministic.compose(&input).unwrap(),
            Some(deterministic.selection().clone()),
        )
        .unwrap();
    let inputs = finalizer.claim_inputs(&s.session).unwrap();
    let proposals = inputs.iter().map(|i| auditor.audit(i).unwrap()).collect();
    let view = gate.view(&s.session).unwrap();
    finalizer
        .audit(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            inputs,
            proposals,
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let report = finalizer
        .deliver(&s.control, view.revision(), view.latest_at() + 1)
        .unwrap();
    assert!(matches!(
        report.outcome,
        ymp_domain::task::SessionStatus::Blocked(_)
    ));
    assert_eq!(report.accounting.spent.get(), 2.0);
    assert_eq!(report.accounting.held.get(), 0.0);
    assert!(!report.accounting.unknown);
    assert!(report.acceptance.is_none());
    assert_eq!(report.report.unmet, vec![id("criterion")]);
    assert_eq!(
        gate.view(&s.session)
            .unwrap()
            .execution()
            .invocations()
            .len(),
        1
    );
}

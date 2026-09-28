//! Paid Planner output, initial team and replaceable work selection through real consumers.
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
    Digest, Id, Prob, Ref, Result,
    assignment::*,
    coordination::*,
    journal::{Capability, MethodKind, PolicySelection},
    plan::*,
    resources::*,
    task::*,
    workspace::WorkspacePath,
};
use ymp_kernel::{
    decision::MethodDecision,
    gatekeeper::Gatekeeper,
    journal::ParameterSchemas,
    plans::PaidRequest,
    ports::{execution::*, planning::*, resources::CostModel},
};
use ymp_runtime::{
    backends::scripted::{Scripted, ScriptedStep},
    clock::ManualClock,
    execution_host::{ExecutionHost, ExecutionStatus},
    policies::{
        award::FirstOffer,
        contribution::{FixedWorkflow, OrdinalValue},
        intake::{NoQuestions, VoiClarification},
        method::FixedMethod,
        planner::{AsNeededDecomposition, ExplicitPlan},
    },
};
use ymp_storage::{content::SqliteContent, journal::SqliteJournal};
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn real(n: f64) -> Real {
    Real::new(n).unwrap()
}
fn definition(
    prompt: &PlanningPrompt,
    assignment: Id<Assignment>,
    session: Id,
    path: &str,
) -> PlanDefinition {
    PlanDefinition {
        plan: Plan {
            id: id("plan"),
            session,
            version: 1,
            items: BTreeSet::from([id("item")]),
            rationale: "One complete initial task".into(),
            author: assignment,
        },
        items: vec![WorkItem {
            id: id("item"),
            plan: id("plan"),
            title: "Implement the complete goal".into(),
            targets: prompt.criteria.iter().map(|c| c.id.clone()).collect(),
            deps: BTreeSet::new(),
            needs: BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]),
            writes: BTreeSet::from([WorkspacePath::new(path).unwrap()]),
            state: WorkState::Open,
            attempts: vec![],
            accepted: None,
            parent: None,
        }],
    }
}
struct PaidFixture {
    selection: PolicySelection,
    runs: Mutex<BTreeMap<Id<Invocation>, Scripted>>,
}
impl PaidFixture {
    fn new() -> Self {
        Self {
            selection: PolicySelection::new(
                "ExecutionBackend",
                "PaidPlanningFixture",
                "1",
                serde_json::json!({}),
            )
            .unwrap(),
            runs: Mutex::new(BTreeMap::new()),
        }
    }
}
impl ExecutionBackend for PaidFixture {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn start(&self, request: &ExecutionRequest<'_>) -> Result<BackendStart> {
        if request.assignment.role == RoleKind::Producer {
            let scripted = Scripted::new(vec![
                ScriptedStep::Write {
                    path: WorkspacePath::new("file")?,
                    bytes: b"candidate".to_vec(),
                },
                ScriptedStep::Complete {
                    usage: Usage {
                        input: 2,
                        cache_read: 0,
                        cache_write: 0,
                        output: 0,
                        reasoning: None,
                    },
                    coverage: Coverage::Complete,
                },
            ])?;
            let started = scripted.start(request)?;
            self.runs
                .lock()
                .unwrap()
                .insert(request.invocation.clone(), scripted);
            return Ok(started);
        }
        let prompt: PlanningPrompt = serde_json::from_str(&request.prompt.text).unwrap();
        let output = if prompt.effective.policy.port == "IntakePolicy" {
            serde_json::to_string(&IntakeOutput {
                criteria: vec![Criterion {
                    id: id(&format!("derived-{}", request.invocation)),
                    text: "The submitted file has inspectable behavior".into(),
                    kind: CriterionKind::NewBehavior,
                    weight: real(1.0),
                    required: true,
                    origin: CriterionOrigin::User,
                    needs_class: BTreeSet::from([EvidenceClass::Inspection]),
                }],
                questions: vec![Question {
                    question: format!("Which behavior for {}?", request.invocation),
                    p_misinterpretation: Prob::new(0.8).unwrap(),
                    rework_cost: real(10.0),
                    assumption: "Preserve the stated interface".into(),
                    reason: "Bound this proof of concept".into(),
                }],
            })
            .unwrap()
        } else {
            serde_json::to_string(&definition(
                &prompt,
                request.assignment.id.clone(),
                request.assignment.session.clone(),
                "file",
            ))
            .unwrap()
        };
        let scripted = Scripted::new(vec![
            ScriptedStep::Emit(BackendObservation::Output(output)),
            ScriptedStep::Complete {
                usage: Usage {
                    input: 2,
                    cache_read: 0,
                    cache_write: 0,
                    output: 0,
                    reasoning: None,
                },
                coverage: Coverage::Complete,
            },
        ])?;
        let started = scripted.start(request)?;
        self.runs
            .lock()
            .unwrap()
            .insert(request.invocation.clone(), scripted);
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
fn call(
    s: &Setup<SqliteJournal>,
    gate: &Arc<Gatekeeper<SqliteJournal, SqliteContent>>,
    host: &ExecutionHost<SqliteJournal, SqliteContent>,
    clock: &ManualClock,
    port: &str,
    name: &str,
) -> PaidPlanningView {
    let plans = s.intake.plans();
    let view = gate.view(&s.session).unwrap();
    let award = s.award_targets(
        name,
        ContributionKind::Plan,
        BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]),
        None,
        view.criteria().iter().map(|c| c.id.clone()).collect(),
    );
    let (revision, at, mut request) = s.request_with_gate(gate, name, award, "file");
    request.role = RoleKind::Planner;
    let mut prepared = gate.prepare(&s.session, revision, at, request).unwrap();
    let admitted = gate.admit(&mut prepared).unwrap();
    clock.advance_to(at).unwrap();
    let prompt = plans.prompt(&s.session, port).unwrap();
    let mut live = host
        .attach(
            admitted,
            id(&format!("call-{name}")),
            id(&format!("receipt-{name}")),
            prompt,
        )
        .unwrap();
    assert!(
        plans
            .paid_input(&s.session, &id(&format!("call-{name}")), port)
            .is_err()
    );
    let view = gate.view(&s.session).unwrap();
    let change = NoQuestions::new().unwrap();
    assert_eq!(
        plans
            .select(&s.control, view.revision(), at, change.selection().clone())
            .unwrap_err()
            .code,
        "planning_boundary"
    );
    host.start(&mut live).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    while host.poll(&mut live).unwrap() != ExecutionStatus::Finished {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    plans
        .paid_input(&s.session, &id(&format!("call-{name}")), port)
        .unwrap()
}
#[test]
fn paid_planning_preserves_user_criteria_and_changes_real_strategy_decisions() {
    for control in [false, true] {
        run(control);
    }
}
fn run(control: bool) {
    let root = Directory::new();
    let database = Directory::new();
    std::fs::write(root.0.join("file"), b"base").unwrap();
    std::fs::write(root.0.join("control"), b"base").unwrap();
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("ExecutionBackend", "PaidPlanningFixture", "1", |_| Ok(()))
        .unwrap();
    let journal = Arc::new(SqliteJournal::open(database.database(), schemas).unwrap());
    let store = journal.clone();
    let backend = Arc::new(PaidFixture::new());
    let voi = VoiClarification::new(real(2.0)).unwrap();
    let no_questions = NoQuestions::new().unwrap();
    let planner = AsNeededDecomposition::new().unwrap();
    let method = FixedMethod::new(MethodKind::Solo).unwrap();
    let with_verifier = FixedMethod::new(MethodKind::SoloWithVerifier).unwrap();
    let params = ContributionParameters {
        expected: real(1.0),
        p90: real(2.0),
        p_success: Prob::new(0.5).unwrap(),
        delta_belief: real(0.2),
    };
    let ordinal = OrdinalValue::new(params.clone()).unwrap();
    let workflow = FixedWorkflow::new(params).unwrap();
    let mut s = Setup::new_with_selections(
        journal.clone(),
        &store,
        &root,
        true,
        "planning",
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
            voi.selection().clone(),
            planner.selection().clone(),
            method.selection().clone(),
            ordinal.selection().clone(),
            ymp_runtime::policies::belief::LikelihoodRatioTable::standard()
                .unwrap()
                .selection()
                .clone(),
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
        Gatekeeper::new(journal.clone(), Arc::new(store.content_store())),
    ));
    let plans = s.intake.plans();
    let initial = gate.view(&s.session).unwrap().criteria()[0].clone();
    let original = s.profile.clone();
    s.switch_to_new_agent("verifier");
    s.profile = original;
    let view = gate.view(&s.session).unwrap();
    plans
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
    plans
        .method(
            &s.control,
            MethodDecision {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                input: view.digest().unwrap(),
                proposal: with_verifier.choose(&view).unwrap(),
            },
            Some(with_verifier.selection().clone()),
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    assert_eq!(view.decisions().len(), 2);
    let at = view.latest_at() + 1;
    plans
        .team(
            &s.control,
            view.revision(),
            at,
            Team {
                id: id("team"),
                session: s.session.clone(),
                revision: 1,
                members: vec![s.profile.agent.clone(), id("verifier")]
                    .into_iter()
                    .map(|agent| Membership {
                        agent,
                        joined: at,
                        left: None,
                        reason: "Initial independent team".into(),
                    })
                    .collect(),
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
    if control {
        let view = gate.view(&s.session).unwrap();
        plans
            .select(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                no_questions.selection().clone(),
            )
            .unwrap();
    }
    let first = call(&s, &gate, &host, &clock, "IntakePolicy", "extract-one");
    assert!(
        gate.view(&s.session)
            .unwrap()
            .resolve(&first.assignment)
            .is_err()
    );
    let proposal = if control {
        no_questions.criteria(&first).unwrap()
    } else {
        voi.criteria(&first).unwrap()
    };
    assert_eq!(
        matches!(proposal.value.questions[0], QuestionDecision::Assume(_)),
        control
    );
    let mut forged = proposal.clone();
    forged.value.criteria[1].origin = CriterionOrigin::Derived(Ref {
        id: id("foreign"),
        version: Digest::of(b"foreign"),
    });
    let view = gate.view(&s.session).unwrap();
    assert_eq!(
        plans
            .extract(
                &s.control,
                PaidRequest {
                    expected_revision: view.revision(),
                    at: view.latest_at(),
                    source: first.clone(),
                    proposal: forged
                }
            )
            .unwrap_err()
            .code,
        "criteria_provenance"
    );
    let mut foreign = first.clone();
    foreign.assignment = Ref {
        id: id("foreign-assignment"),
        version: Digest::of(b"foreign"),
    };
    let forged_proposal = if control {
        no_questions.criteria(&foreign).unwrap()
    } else {
        voi.criteria(&foreign).unwrap()
    };
    assert!(
        plans
            .extract(
                &s.control,
                PaidRequest {
                    expected_revision: view.revision(),
                    at: view.latest_at(),
                    source: foreign,
                    proposal: forged_proposal
                }
            )
            .is_err()
    );
    let mut missing = first.clone();
    missing.invocation = id("missing-invocation");
    assert!(
        plans
            .extract(
                &s.control,
                PaidRequest {
                    expected_revision: view.revision(),
                    at: view.latest_at(),
                    source: missing,
                    proposal: proposal.clone()
                }
            )
            .is_err()
    );
    assert_eq!(gate.view(&s.session).unwrap(), view);
    plans
        .extract(
            &s.control,
            PaidRequest {
                expected_revision: view.revision(),
                at: view.latest_at(),
                source: first.clone(),
                proposal,
            },
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    assert_eq!(view.criteria()[0], initial);
    view.resolve(&first.assignment).unwrap();
    assert!(
        plans
            .paid_input(&s.session, &first.invocation, "IntakePolicy")
            .is_err()
    );
    assert_eq!(
        view.task().unwrap().goal.assumptions.len(),
        usize::from(control)
    );
    if !control {
        register_planning_check(&s, &store, "passing", b"candidate");
    }
    let current_prompt: PlanningPrompt =
        serde_json::from_str(&plans.prompt(&s.session, "Planner").unwrap().text).unwrap();
    let explicit = ExplicitPlan::new(definition(
        &current_prompt,
        id("control-author"),
        s.session.clone(),
        "control",
    ))
    .unwrap();
    if control {
        plans
            .select(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                explicit.selection().clone(),
            )
            .unwrap();
    }
    let source = call(&s, &gate, &host, &clock, "Planner", "plan-one");
    let standard = planner.plan(&source).unwrap();
    let proposal = if control {
        explicit.plan(&source).unwrap()
    } else {
        standard.clone()
    };
    let view = gate.view(&s.session).unwrap();
    let mut uncovered = proposal.clone();
    uncovered.value.items[0].targets.remove(&initial.id);
    assert_eq!(
        plans
            .plan(
                &s.control,
                PaidRequest {
                    expected_revision: view.revision(),
                    at: view.latest_at(),
                    source: source.clone(),
                    proposal: uncovered
                }
            )
            .unwrap_err()
            .code,
        "plan_coverage"
    );
    let mut unsupported = proposal.clone();
    unsupported.value.items[0].needs.insert(Capability::Network);
    assert_eq!(
        plans
            .plan(
                &s.control,
                PaidRequest {
                    expected_revision: view.revision(),
                    at: view.latest_at(),
                    source: source.clone(),
                    proposal: unsupported
                }
            )
            .unwrap_err()
            .code,
        "plan_scope"
    );
    let mut cycle = proposal.value.clone();
    let mut other = cycle.items[0].clone();
    other.id = id("other");
    other.deps.insert(id("item"));
    cycle.items[0].deps.insert(other.id.clone());
    cycle.plan.items.insert(other.id.clone());
    cycle.items.push(other);
    assert_eq!(
        ymp_kernel::plans::validate_definition(&view, &cycle, &proposal.value.plan.author)
            .unwrap_err()
            .code,
        "plan_cycle"
    );
    if control {
        assert_ne!(
            proposal.value.items[0].writes,
            standard.value.items[0].writes
        );
    }
    let mut stale_source = source.clone();
    stale_source.journal = Digest::of(b"obsolete work boundary");
    assert!(
        plans
            .plan(
                &s.control,
                PaidRequest {
                    expected_revision: view.revision(),
                    at: view.latest_at(),
                    source: stale_source,
                    proposal: proposal.clone()
                }
            )
            .is_err()
    );
    plans
        .plan(
            &s.control,
            PaidRequest {
                expected_revision: view.revision(),
                at: view.latest_at(),
                source,
                proposal,
            },
        )
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    assert_eq!(view.execution().invocations().len(), 2);
    assert_eq!(view.treasury().unwrap().budget.spent.get(), 4.0);
    assert!(
        view.results().items()[&id("item")]
            .writes
            .contains(&WorkspacePath::new(if control { "control" } else { "file" }).unwrap())
    );
    let input = plans.contributions(&s.session).unwrap();
    let proposal = ordinal.next(&input).unwrap();
    assert_eq!(proposal.value[0].kind, ContributionKind::Produce);
    let mut one_member = input.clone();
    one_member.slots = 2;
    for candidate in &mut one_member.candidates {
        candidate.eligible = BTreeSet::from([s.profile.agent.clone()]);
    }
    assert_eq!(ordinal.next(&one_member).unwrap().value.len(), 1);
    assert_eq!(workflow.next(&one_member).unwrap().value.len(), 1);
    let mut protected = input.clone();
    protected.production_remaining = real(0.0);
    assert!(ordinal.next(&protected).unwrap().value.is_empty());
    assert!(workflow.next(&protected).unwrap().value.is_empty());
    let mut occupied = input.clone();
    occupied.slots = 0;
    assert!(ordinal.next(&occupied).unwrap().value.is_empty());
    let view = gate.view(&s.session).unwrap();
    plans
        .record_contributions(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            input,
            proposal,
        )
        .unwrap();
    if !control {
        let view = gate.view(&s.session).unwrap();
        plans
            .select(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                workflow.selection().clone(),
            )
            .unwrap();
        let input = plans.contributions(&s.session).unwrap();
        assert!(!input.limitations.is_empty());
        let proposal = workflow.next(&input).unwrap();
        assert_eq!(proposal.value[0].kind, ContributionKind::Clarify);
        let view = gate.view(&s.session).unwrap();
        plans
            .record_contributions(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                input,
                proposal,
            )
            .unwrap();
        let view = gate.view(&s.session).unwrap();
        plans
            .select(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                ordinal.selection().clone(),
            )
            .unwrap();
        verify_then_diagnose(&s, &gate, &host, &clock, &store);
    }
    let view = gate.view(&s.session).unwrap();
    assert_eq!(view.criteria()[0], initial);
    s.intake
        .refine(
            &s.control,
            ymp_kernel::intake::IntakeRefinement {
                expected_revision: view.revision(),
                at: view.latest_at() + 1,
                constraints: view.task().unwrap().constraints.clone(),
                criteria: view.criteria().to_vec(),
                reason: "Record an explicit user answer without rewriting Derived provenance"
                    .into(),
                note: ymp_kernel::intake::IntakeNote::Clarification(Clarification {
                    question: "Which target?".into(),
                    answer: "The stated one".into(),
                    at: view.latest_at() + 1,
                }),
            },
        )
        .unwrap();
    assert!(
        plans
            .contributions(&s.session)
            .unwrap()
            .limitations
            .iter()
            .any(|l| l.contains("stale criterion contract"))
    );
}
fn register_planning_check(
    s: &Setup<SqliteJournal>,
    store: &SqliteJournal,
    name: &str,
    bytes: &[u8],
) {
    use ymp_domain::verification::*;
    use ymp_kernel::acceptance::RegisterCheck;
    let authority = s.intake.acceptance(Arc::new(store.content_store()));
    {
        let view = s.gate.view(&s.session).unwrap();
        let policy = view.policies()["VerificationDesigner"].clone();
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
        authority
            .register_check(
                &s.control,
                RegisterCheck {
                    expected_revision: view.revision(),
                    at: view.latest_at() + 1,
                    proposal: ymp_domain::Proposal {
                        value: check,
                        rationale: "Explicit candidate check".into(),
                        basis: vec![],
                        policy: policy.policy.clone(),
                    },
                    effective: policy,
                },
            )
            .unwrap();
    }
}
fn verify_then_diagnose(
    s: &Setup<SqliteJournal>,
    gate: &Arc<Gatekeeper<SqliteJournal, SqliteContent>>,
    host: &ExecutionHost<SqliteJournal, SqliteContent>,
    clock: &ManualClock,
    store: &SqliteJournal,
) {
    use ymp_domain::{result::Artifact, verification::*};
    use ymp_kernel::{
        acceptance::{ApplicabilityContext, EvidenceRequest, RunCheck},
        ledger::LedgerRequest,
        results::Results,
    };
    let plans = s.intake.plans();
    let authority = s.intake.acceptance(Arc::new(store.content_store()));
    let results = Results::new(s.journal.clone(), gate.clone()).unwrap();
    let view = gate.view(&s.session).unwrap();
    let item = view.results().items()[&id("item")].clone();
    let award = s.award_targets(
        "produce",
        ContributionKind::Produce,
        item.needs.clone(),
        Some(ContributionSubject::WorkItem(item.reference().unwrap())),
        item.targets.clone(),
    );
    let (revision, at, request) = s.request_with_gate(gate, "produce", award, "file");
    let mut prepared = gate.prepare(&s.session, revision, at, request).unwrap();
    let admitted = gate.admit(&mut prepared).unwrap();
    clock.advance_to(at).unwrap();
    gate.workspace()
        .snapshot_unstarted(admitted.files.as_ref().unwrap(), id("before"), at)
        .unwrap();
    let view = gate.view(&s.session).unwrap();
    let attempt = results
        .prepare_attempt(
            &admitted.grant,
            view.revision(),
            at,
            id("attempt"),
            id("before"),
            id("after"),
        )
        .unwrap();
    results.begin(&attempt).unwrap();
    let mut live = host
        .attach(
            admitted,
            id("production-call"),
            id("production-receipt"),
            Prompt {
                text: "Produce the exact declared item".into(),
                basis: vec![item.reference().unwrap()],
            },
        )
        .unwrap();
    host.start(&mut live).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    while host.poll(&mut live).unwrap() != ExecutionStatus::Finished {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let at = gate.view(&s.session).unwrap().latest_at();
    let result = results
        .submit(
            &attempt,
            at,
            id("candidate"),
            BTreeSet::from([Artifact {
                path: WorkspacePath::new("file").unwrap(),
                digest: Digest::of(b"candidate"),
            }]),
            "Real retained producer output".into(),
        )
        .unwrap();
    // Adding a new check changes the contract but preserves the plan's task and criteria.
    register_planning_check(s, store, "failing", b"wrong");
    let mut context = ApplicabilityContext {
        result: result.reference().unwrap(),
        criteria: gate
            .view(&s.session)
            .unwrap()
            .criteria()
            .iter()
            .map(|c| c.reference().unwrap())
            .collect(),
        environments: BTreeMap::new(),
    };
    for name in ["passing", "failing"] {
        let view = gate.view(&s.session).unwrap();
        let check = view.checks()[&id(name)].clone();
        let run = authority
            .run(
                &s.control,
                RunCheck {
                    expected_revision: view.revision(),
                    at: view.latest_at() + 1,
                    id: id(&format!("run-{name}")),
                    check: check.reference(),
                    target: result.after.clone(),
                    role: CheckRunRole::Candidate,
                },
                &ymp_runtime::checks::retained::RetainedBytes,
            )
            .unwrap();
        let view = gate.view(&s.session).unwrap();
        let evidence = authority
            .evidence(
                &s.control,
                EvidenceRequest {
                    expected_revision: view.revision(),
                    at: view.latest_at() + 1,
                    id: id(&format!("evidence-{name}")),
                    criterion: view.criteria()[0].reference().unwrap(),
                    result: result.reference().unwrap(),
                    runs: vec![run.id],
                    reviews: vec![],
                },
            )
            .unwrap();
        context.environments.insert(
            check.reference(),
            BTreeSet::from([evidence.scope.environment.unwrap()]),
        );
        let model = ymp_runtime::policies::belief::LikelihoodRatioTable::standard().unwrap();
        let view = gate.view(&s.session).unwrap();
        let at = view.latest_at() + 1;
        let rules = AssessmentRules {
            mutation_threshold: Prob::new(0.8).unwrap(),
        };
        let responses = authority
            .ledger_inputs(&s.session, &context, &rules, at)
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
                LedgerRequest {
                    expected_revision: view.revision(),
                    at,
                    context: context.clone(),
                    rules,
                    responses,
                },
                None,
            )
            .unwrap();
        let input = plans.contributions(&s.session).unwrap();
        let policy = OrdinalValue::new(
            serde_json::from_value(
                gate.view(&s.session).unwrap().policies()["ContributionPolicy"]
                    .parameters
                    .clone(),
            )
            .unwrap(),
        )
        .unwrap();
        let proposal = policy.next(&input).unwrap();
        let expected = if name == "passing" {
            ContributionKind::Verify
        } else {
            ContributionKind::Diagnose
        };
        assert_eq!(proposal.value[0].kind, expected);
        if name == "passing" {
            let selected = input
                .candidates
                .iter()
                .find(|c| c.contribution.id == proposal.value[0].id)
                .unwrap();
            assert!(!selected.eligible.contains(&result.producer));
            assert!(selected.eligible.contains(&id("verifier")));
        }
        let view = gate.view(&s.session).unwrap();
        plans
            .record_contributions(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                input,
                proposal,
            )
            .unwrap();
    }
}

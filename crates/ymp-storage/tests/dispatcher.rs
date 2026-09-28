//! Fixed workflow through actual journal, filesystem, admission and execution consumers.
mod support;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use support::Directory;
use ymp_domain::{
    Digest, Id, Prob, Result,
    assignment::*,
    coordination::*,
    identity::*,
    journal::{Capability, MethodKind, PolicySelection},
    plan::*,
    resources::*,
    task::*,
    verification::*,
    workspace::*,
};
use ymp_kernel::{
    intake::IntakeRequest,
    journal::{Journal, ParameterSchemas},
    ports::{execution::*, planning::*, progress::*},
    session::{SessionDefinition, VisibleCheck},
};
use ymp_runtime::{
    backends::scripted::{Scripted, ScriptedStep},
    checks::retained::RetainedBytes,
    clock::ManualClock,
    dispatcher::{Dispatcher, SessionPolicies, SessionStart},
    policies::*,
    workspace::direct::Direct,
};
use ymp_storage::{content::SqliteContent, journal::SqliteJournal};
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn real(n: f64) -> Real {
    Real::new(n).unwrap()
}
fn policies() -> SessionPolicies {
    SessionPolicies {
        readiness: Box::new(ymp_runtime::readiness::StaticDependencyProbe::new().unwrap()),
        intake: Box::new(intake::NoQuestions::new().unwrap()),
        method: Box::new(method::FixedMethod::new(MethodKind::SoloWithVerifier).unwrap()),
        planner: Box::new(planner::AsNeededDecomposition::new().unwrap()),
        contributions: Box::new(
            contribution::FixedWorkflow::new(ContributionParameters {
                expected: real(10.0),
                p90: real(10.0),
                p_success: Prob::new(0.5).unwrap(),
                delta_belief: real(0.1),
            })
            .unwrap(),
        ),
        belief: Box::new(belief::LikelihoodRatioTable::standard().unwrap()),
        credit: Box::new(credit::ConfirmedOnly::new().unwrap()),
        monitor: Box::new(
            progress::EvidenceDelta::new(MonitorParameters {
                epsilon: real(0.001),
                stall_limit: 3,
            })
            .unwrap(),
        ),
        diagnosis: Box::new(
            diagnosis::RuleBasedDiagnoser::new(DiagnosisParameters {
                p_min: Prob::new(0.1).unwrap(),
                mutation_threshold: Prob::new(0.8).unwrap(),
            })
            .unwrap(),
        ),
        escalation: Box::new(
            escalation::DiagnosisFirstLadder::new(EscalationParameters {
                max_uses: 1,
                max_cost: real(10.0),
                timeout_ms: 10000,
            })
            .unwrap(),
        ),
        award: Arc::new(
            award::FirstOffer::with_commitment_terms(CommitmentTerms {
                lease_duration: 10000,
                renewal_duration: 10000,
                renew_on: BTreeSet::new(),
                renewals: 0,
                release_delta: real(1.0),
            })
            .unwrap(),
        ),
        cost: Arc::new(
            resources::PriceWeighted::new(PriceWeightedParameters {
                expected_input: 10,
                expected_output: 0,
                p90_factor: real(1.0),
            })
            .unwrap(),
        ),
        resources: Box::new(
            resources::PurposeBounded::new(PurposeBoundedParameters {
                max_cost: real(100.0),
                timeout: 10000,
                native_turns: 2,
                output_chars: 16000,
                report_call_cost: real(10.0),
            })
            .unwrap(),
        ),
        context: Box::new(context::CriteriaProjection::new(4).unwrap()),
        reviewer: Box::new(reviewer::AnyNonProducer::new().unwrap()),
        narrative: Box::new(narrative::Narrator::new().unwrap()),
        audit: Box::new(claim_audit::EvidenceClassRules::new().unwrap()),
    }
}
struct Backend {
    selection: PolicySelection,
    runs: Mutex<BTreeMap<Id<Invocation>, Scripted>>,
    wrong_bytes: bool,
    wrong_first: bool,
    needs_evidence: bool,
    pause: Option<RoleKind>,
    paused: Mutex<BTreeSet<Id<Invocation>>>,
}
impl Backend {
    fn new() -> Self {
        Self {
            selection: PolicySelection::new(
                "ExecutionBackend",
                "SessionFixture",
                "1",
                serde_json::json!({}),
            )
            .unwrap(),
            runs: Mutex::new(BTreeMap::new()),
            wrong_bytes: false,
            wrong_first: false,
            needs_evidence: false,
            pause: None,
            paused: Mutex::new(BTreeSet::new()),
        }
    }
}
impl ExecutionBackend for Backend {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn start(&self, r: &ExecutionRequest<'_>) -> Result<BackendStart> {
        if self.pause == Some(r.assignment.role) {
            self.paused.lock().unwrap().insert(r.invocation.clone());
        }
        let data: serde_json::Value = serde_json::from_str(&r.prompt.text).unwrap();
        let purpose = data.as_array().unwrap().last().unwrap();
        assert_eq!(
            data[2]["author"],
            serde_json::to_value(&r.assignment.agent).unwrap()
        );
        assert_eq!(data[2]["untrusted"], true);
        let output = if purpose["port"] == "IntakePolicy" {
            serde_json::to_string(&IntakeOutput {
                criteria: vec![],
                questions: vec![],
            })
            .unwrap()
        } else if purpose["operation"] == "production" {
            "Produced the requested file".into()
        } else if purpose["operation"] == "candidate_verification"
            || purpose["operation"] == "recovery_research"
        {
            "Inspected the exact candidate and visible check".into()
        } else if purpose["operation"] == "candidate_review" {
            serde_json::to_string(&ymp_kernel::acceptance::CandidateVerdict {
                id: id(&format!("review-{}", Digest::of(r.invocation.as_str()))),
                result: serde_json::from_value(purpose["result"].clone()).unwrap(),
                criteria: serde_json::from_value(purpose["criteria"].clone()).unwrap(),
                verdict: if self.needs_evidence {
                    ReviewVerdict::NeedsEvidence
                } else {
                    ReviewVerdict::Approve
                },
                findings: vec![],
                basis: serde_json::from_value::<Vec<ymp_domain::Ref>>(data[7].clone())
                    .unwrap()
                    .into_iter()
                    .map(|r| Id::new(r.id.as_str()).unwrap())
                    .collect(),
                rationale: "Checked the exact retained candidate".into(),
            })
            .unwrap()
        } else if purpose["operation"] == "final_review" {
            let aggregate: ymp_domain::report::FinalAggregate =
                serde_json::from_value(purpose["aggregate"].clone()).unwrap();
            serde_json::to_string(&ymp_kernel::finalization::FinalVerdict {
                id: id("final-review"),
                aggregate: aggregate.reference().unwrap(),
                verdict: ReviewVerdict::Approve,
                basis: serde_json::from_value::<Vec<ymp_domain::Ref>>(data[7].clone())
                    .unwrap()
                    .into_iter()
                    .map(|r| Id::new(r.id.as_str()).unwrap())
                    .collect(),
                rationale: "Checked the integrated retained bytes".into(),
            })
            .unwrap()
        } else if r.assignment.role == RoleKind::Narrator {
            serde_json::to_string(&ymp_kernel::ports::reporting::ReportDraft {
                claims: vec![ymp_kernel::ports::reporting::DraftClaim {
                    text: "Uncertain: no broader claim is established by this statement.".into(),
                    assertion: ymp_kernel::ports::reporting::Assertion::Uncertainty,
                }],
            })
            .unwrap()
        } else {
            let input: PlanningPrompt =
                serde_json::from_value(purpose["planning"].clone()).unwrap();
            serde_json::to_string(&PlanDefinition {
                plan: Plan {
                    id: id("plan"),
                    session: r.assignment.session.clone(),
                    version: 1,
                    items: BTreeSet::from([id("item")]),
                    rationale: "One explicit artifact work item".into(),
                    author: r.assignment.id.clone(),
                },
                items: vec![WorkItem {
                    id: id("item"),
                    plan: id("plan"),
                    title: "Write the checked file".into(),
                    targets: input.criteria.iter().map(|c| c.id.clone()).collect(),
                    deps: BTreeSet::new(),
                    needs: BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]),
                    writes: BTreeSet::from([WorkspacePath::new("file").unwrap()]),
                    state: WorkState::Open,
                    attempts: vec![],
                    accepted: None,
                    parent: None,
                }],
            })
            .unwrap()
        };
        let mut steps = vec![];
        if r.assignment.role == RoleKind::Producer {
            steps.push(ScriptedStep::Write {
                path: WorkspacePath::new("file").unwrap(),
                bytes: if self.wrong_bytes
                    || (self.wrong_first && purpose.get("recovery").is_none())
                {
                    b"defect".to_vec()
                } else {
                    b"candidate".to_vec()
                },
            });
        }
        steps.extend(vec![
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
        ]);
        let backend = Scripted::new(steps)?;
        let started = backend.start(r)?;
        self.runs
            .lock()
            .unwrap()
            .insert(r.invocation.clone(), backend);
        Ok(started)
    }
    fn cancel(&self, h: &ExecutionHandle) -> Result<()> {
        self.paused.lock().unwrap().remove(&h.invocation);
        self.runs.lock().unwrap()[&h.invocation].cancel(h)
    }
    fn events(&self, h: &ExecutionHandle, a: u64, l: usize) -> Result<Vec<BackendEvent>> {
        if self.paused.lock().unwrap().contains(&h.invocation) {
            return Ok(vec![]);
        }
        self.runs.lock().unwrap()[&h.invocation].events(h, a, l)
    }
    fn receipt(&self, h: &ExecutionHandle) -> Result<Receipt> {
        self.runs.lock().unwrap()[&h.invocation].receipt(h)
    }
}
fn criterion(name: &str) -> Criterion {
    Criterion {
        id: id(name),
        text: "The declared file contains the requested bytes".into(),
        kind: CriterionKind::NewBehavior,
        weight: real(1.0),
        required: true,
        origin: CriterionOrigin::User,
        needs_class: BTreeSet::new(),
    }
}
fn request() -> SessionStart {
    let caps = BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]);
    SessionStart {
        session: id("session"),
        task: IntakeRequest {
            task: Task {
                id: id("task"),
                goal: Goal {
                    request: "Write candidate to file".into(),
                    assumptions: vec![],
                    clarifications: vec![],
                },
                contract: id("contract"),
                constraints: Constraints {
                    budget: real(100.0),
                    verification_reserve: real(20.0),
                    deadline: None,
                    pins: Pins::unrestricted(),
                    allowed: caps.clone(),
                    parallel_limit: 2,
                    attempt_limit: 2,
                    max_members: 2,
                },
            },
            criteria: vec![criterion("user")],
        },
        facts: RegistryFacts {
            agents: ["a", "b"]
                .into_iter()
                .map(|name| Agent {
                    id: id(name),
                    name: name.into(),
                    provider: id("provider"),
                    defaults: ProfileSettings::default(),
                    instructions: "Follow the exact role response schema".into(),
                    enabled: true,
                })
                .collect(),
            discoveries: vec![Discovery {
                provider: Provider {
                    id: id("provider"),
                    kind: ProviderKind::Scripted,
                    version: None,
                    capabilities: Some(caps),
                },
                offerings: vec![ModelOffering {
                    provider: id("provider"),
                    model: Some("fixture".into()),
                    family: None,
                    efforts: None,
                    default_effort: None,
                }],
                default_model: Some("fixture".into()),
                adapter_available: true,
                source: DiscoverySource::ScriptedFixture,
                method: "Explicit session protocol fixture".into(),
                observed_at: 1,
            }],
            dependencies: vec![],
        },
        definition: SessionDefinition {
            offer_window_ms: 1,
            workspace: id("workspace"),
            checks: ["user"]
                .into_iter()
                .map(|name| VisibleCheck {
                    criterion: criterion(name).reference().unwrap(),
                    spec: CheckSpec::ExactBytes {
                        path: WorkspacePath::new("file").unwrap(),
                        digest: Digest::of(b"candidate"),
                    },
                })
                .collect(),
            rules: AssessmentRules {
                mutation_threshold: Prob::new(0.8).unwrap(),
            },
        },
        pricebook: PriceBook {
            version: "relative".into(),
            rates: vec![],
            fallback: Rates::fallback(),
        },
        unknown_usage: UnknownUsage::Stop,
    }
}
fn initial<J: Journal + 'static>(
    journal: Arc<J>,
    reopen: impl Fn() -> Arc<J>,
    content: Arc<SqliteContent>,
    root: &Directory,
    report_recovery: bool,
) {
    let clock = Arc::new(ManualClock::new(1));
    let backend = Arc::new(Backend::new());
    let provider = Arc::new(Direct::open(&root.0, CaptureLimits::default()).unwrap());
    let mut session = Dispatcher::start(
        journal.clone(),
        content.clone(),
        request(),
        policies(),
        backend.clone(),
        provider.clone(),
        Arc::new(RetainedBytes),
        clock.clone(),
    )
    .unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut reopened_production = false;
    let mut reopened_report = false;
    while session.view().unwrap().finalization().delivered.is_none() {
        if let ymp_runtime::dispatcher::Tick::Waiting { until } = session.tick().unwrap() {
            clock.advance_to(until).unwrap();
        }
        let view = session.view().unwrap();
        if !reopened_production && !view.results().results().is_empty() {
            drop(session);
            session = Dispatcher::recover(
                reopen(),
                content.clone(),
                id("session"),
                ymp_runtime::application::RecoveryIntent::Continue,
                policies(),
                backend.clone(),
                provider.clone(),
                Arc::new(RetainedBytes),
                clock.clone(),
            )
            .unwrap();
            reopened_production = true;
        } else if report_recovery
            && !reopened_report
            && view
                .execution()
                .invocations()
                .get(&id("call-narration"))
                .is_some_and(|r| {
                    r.terminal == Some(InvocationTerminal::Completed) && r.confirmed_terminal
                })
        {
            drop(session);
            session = Dispatcher::recover(
                reopen(),
                content.clone(),
                id("session"),
                ymp_runtime::application::RecoveryIntent::Report,
                policies(),
                backend.clone(),
                provider.clone(),
                Arc::new(RetainedBytes),
                clock.clone(),
            )
            .unwrap();
            reopened_report = true;
        }
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let view = session.view().unwrap();
    assert_eq!(view.criteria().len(), 1);
    assert_eq!(view.execution().invocations().len(), 7);
    assert_eq!(view.treasury().unwrap().budget.spent.get(), 14.0);
    assert!(
        view.results()
            .items()
            .values()
            .all(|i| i.accepted.is_some())
    );
    assert!(
        view.coordination()
            .commitments()
            .values()
            .all(|c| c.state == CommitmentState::Discharged)
    );
    assert_eq!(
        view.finalization()
            .acceptance
            .as_ref()
            .unwrap()
            .acceptance
            .decision,
        AcceptanceDecision::Accepted
    );
    assert_eq!(
        view.finalization().delivered.as_ref().unwrap().outcome,
        SessionStatus::Delivered
    );
    assert_eq!(view.results().items().len(), 1);
    assert!(reopened_production);
    assert_eq!(reopened_report, report_recovery);
    let revision = view.revision();
    drop(session);
    let mut read = Dispatcher::recover(
        reopen(),
        content,
        id("session"),
        ymp_runtime::application::RecoveryIntent::Report,
        policies(),
        backend.clone(),
        provider,
        Arc::new(RetainedBytes),
        clock,
    )
    .unwrap();
    assert!(matches!(
        read.tick().unwrap(),
        ymp_runtime::dispatcher::Tick::Delivered(_)
    ));
    assert_eq!(read.view().unwrap().revision(), revision);
    assert_eq!(backend.runs.lock().unwrap().len(), 7);
}
#[test]
fn dispatcher_delivers_checked_producer_reviewer_session_on_both_journals() {
    for memory in [true, false] {
        let root = Directory::new();
        let database = Directory::new();
        std::fs::write(root.0.join("file"), b"baseline").unwrap();
        let mut schemas = ParameterSchemas::default();
        schemas
            .register("ExecutionBackend", "SessionFixture", "1", |_| Ok(()))
            .unwrap();
        let store = SqliteJournal::open(database.database(), schemas.clone()).unwrap();
        let content = Arc::new(store.content_store());
        if memory {
            let journal = Arc::new(
                ymp_runtime::memory_journal::MemoryJournal::with_binding_identity(schemas).unwrap(),
            );
            initial(journal.clone(), || journal.clone(), content, &root, false);
        } else {
            initial(
                Arc::new(SqliteJournal::open(database.database(), schemas.clone()).unwrap()),
                || Arc::new(SqliteJournal::open(database.database(), schemas.clone()).unwrap()),
                content,
                &root,
                true,
            );
        }
    }
}

fn stopped<J: Journal + 'static>(journal: Arc<J>, content: Arc<SqliteContent>, root: &Directory) {
    let clock = Arc::new(ManualClock::new(1));
    let mut fixture = Backend::new();
    fixture.pause = Some(RoleKind::Planner);
    let backend = Arc::new(fixture);
    let mut session = Dispatcher::start(
        journal,
        content,
        request(),
        policies(),
        backend.clone(),
        Arc::new(Direct::open(&root.0, CaptureLimits::default()).unwrap()),
        Arc::new(RetainedBytes),
        clock.clone(),
    )
    .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while backend.runs.lock().unwrap().is_empty() {
        if let ymp_runtime::dispatcher::Tick::Waiting { until } = session.tick().unwrap() {
            clock.advance_to(until).unwrap();
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let owner = session.owner();
    session.application().interrupt(&owner, 1).unwrap();
    let expected = session
        .view()
        .unwrap()
        .treasury()
        .unwrap()
        .budget
        .id
        .clone();
    while session.view().unwrap().finalization().delivered.is_none() {
        session.tick().unwrap();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let view = session.view().unwrap();
    assert_eq!(view.status(), Some(SessionStatus::Cancelled));
    assert_eq!(
        view.finalization().delivered.as_ref().unwrap().outcome,
        SessionStatus::Cancelled
    );
    assert_eq!(
        view.policies()["NarrativeComposer"].policy.implementation,
        "DeterministicReport"
    );
    assert_eq!(view.treasury().unwrap().budget.id, expected);
    assert_eq!(backend.runs.lock().unwrap().len(), 1);
    assert!(view.finalization().aggregate.is_none());
    assert_eq!(std::fs::read(root.0.join("file")).unwrap(), b"baseline");
}
#[test]
fn stopped_dispatcher_delivers_without_another_model_call() {
    for memory in [true, false] {
        let root = Directory::new();
        let database = Directory::new();
        std::fs::write(root.0.join("file"), b"baseline").unwrap();
        let mut schemas = ParameterSchemas::default();
        schemas
            .register("ExecutionBackend", "SessionFixture", "1", |_| Ok(()))
            .unwrap();
        let store = SqliteJournal::open(database.database(), schemas.clone()).unwrap();
        let content = Arc::new(store.content_store());
        if memory {
            stopped(
                Arc::new(
                    ymp_runtime::memory_journal::MemoryJournal::with_binding_identity(schemas)
                        .unwrap(),
                ),
                content,
                &root,
            );
        } else {
            stopped(Arc::new(store), content, &root);
        }
    }
}

fn failure(case: u8) {
    let defect = case == 0;
    let limited = case == 1;
    let missing_final = case == 2;
    let root = Directory::new();
    let database = Directory::new();
    std::fs::write(root.0.join("file"), b"baseline").unwrap();
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("ExecutionBackend", "SessionFixture", "1", |_| Ok(()))
        .unwrap();
    let store = SqliteJournal::open(database.database(), schemas.clone()).unwrap();
    let content = Arc::new(store.content_store());
    let journal = Arc::new(
        ymp_runtime::memory_journal::MemoryJournal::with_binding_identity(schemas).unwrap(),
    );
    let mut request = request();
    if limited {
        request.task.task.constraints.budget = real(33.0);
        request.task.task.constraints.verification_reserve = real(1.0);
    }
    let mut fixture = Backend::new();
    fixture.wrong_bytes = defect;
    let backend = Arc::new(fixture);
    let clock = Arc::new(ManualClock::new(1));
    let mut pool_refreshed = false;
    let mut capacity_observed = false;
    let mut session = Dispatcher::start(
        journal,
        content,
        request,
        policies(),
        backend.clone(),
        Arc::new(Direct::open(&root.0, CaptureLimits::default()).unwrap()),
        Arc::new(RetainedBytes),
        clock.clone(),
    )
    .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while session.view().unwrap().finalization().delivered.is_none() {
        if let ymp_runtime::dispatcher::Tick::Waiting { until } = session.tick().unwrap() {
            clock.advance_to(until).unwrap();
        }
        let view = session.view().unwrap();
        if limited
            && !capacity_observed
            && !view.results().plans().is_empty()
            && view.treasury().unwrap().reporting_mode.is_none()
        {
            assert_eq!(
                view.treasury()
                    .unwrap()
                    .remaining(Purpose::Verification)
                    .unwrap(),
                real(9.0)
            );
            assert_eq!(view.treasury().unwrap().budget.spent, real(4.0));
            capacity_observed = true;
        }
        if missing_final
            && !pool_refreshed
            && view
                .acceptances()
                .values()
                .any(|a| a.acceptance.decision == AcceptanceDecision::Accepted)
        {
            let mut facts = view.registry().unwrap().input.facts.clone();
            facts
                .agents
                .iter_mut()
                .find(|a| a.id != view.results().results().values().next().unwrap().producer)
                .unwrap()
                .enabled = false;
            let app = session.application();
            let input = app
                .registry_input(&id("session"), facts, view.latest_at())
                .unwrap();
            let probe = ymp_runtime::readiness::StaticDependencyProbe::new().unwrap();
            let responses = ymp_kernel::registry::readiness_views(&input)
                .iter()
                .map(|v| ymp_runtime::readiness::readiness_response(&probe, v).unwrap())
                .collect();
            app.record_pool(
                &id("session"),
                view.revision(),
                view.latest_at(),
                input,
                ymp_kernel::ports::checks::ReadinessProbe::selection(&probe).clone(),
                responses,
            )
            .unwrap();
            pool_refreshed = true;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let view = session.view().unwrap();
    let report = view.finalization().delivered.as_ref().unwrap();
    assert!(
        matches!(report.outcome, SessionStatus::Blocked(_)),
        "case={case} outcome={:?}",
        report.outcome
    );
    assert_eq!(
        view.policies()["NarrativeComposer"].policy.implementation,
        if missing_final {
            "Narrator"
        } else {
            "DeterministicReport"
        }
    );
    assert!(!report.report.unmet.is_empty());
    assert!(view.finalization().acceptance.is_none());
    assert!(!view.results().plans().is_empty());
    assert!(report.accounting.spent.get() > 0.0);
    assert!(
        missing_final
            || view
                .progress()
                .history
                .iter()
                .any(|(_, e)| matches!(e, ymp_kernel::progress::ProgressRecorded::Diagnosis(_)))
    );
    if defect {
        assert!(
            view.reviews()
                .values()
                .any(|r| r.review.verdict == ReviewVerdict::Approve)
        );
        assert!(
            view.acceptances()
                .values()
                .any(|a| matches!(a.acceptance.decision, AcceptanceDecision::Rejected(_)))
        );
        assert_eq!(view.results().results().len(), 1);
        assert_eq!(backend.runs.lock().unwrap().len(), 5);
    } else if limited {
        assert!(capacity_observed);
        assert_eq!(backend.runs.lock().unwrap().len(), 2);
    } else {
        assert!(pool_refreshed);
        assert_eq!(
            report.outcome,
            SessionStatus::Blocked("final_review_pending".into())
        );
        assert_eq!(report.accepted_sources.len(), 1);
        assert_eq!(backend.runs.lock().unwrap().len(), 6);
    }
}

#[test]
fn failure_check_defeats_approval_in_dispatcher() {
    failure(0);
}
#[test]
fn failure_insufficient_verification_preserves_paid_work() {
    failure(1);
}
#[test]
fn unavailable_final_reviewer_preserves_accepted_source() {
    failure(2);
}

fn recovery_work(retry: bool) {
    use ymp_domain::journal::EscalationStep;
    let root = Directory::new();
    let database = Directory::new();
    std::fs::write(root.0.join("file"), b"baseline").unwrap();
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("ExecutionBackend", "SessionFixture", "1", |_| Ok(()))
        .unwrap();
    let store = SqliteJournal::open(database.database(), schemas.clone()).unwrap();
    let content = Arc::new(store.content_store());
    let journal = Arc::new(
        ymp_runtime::memory_journal::MemoryJournal::with_binding_identity(schemas).unwrap(),
    );
    let mut policy = policies();
    policy.method = Box::new(
        method::FixedMethod::with_ladder(
            MethodKind::SoloWithVerifier,
            vec![
                if retry {
                    EscalationStep::Retry
                } else {
                    EscalationStep::AddVerifier
                },
                EscalationStep::StopPreserving,
            ],
        )
        .unwrap(),
    );
    let mut fixture = Backend::new();
    fixture.wrong_first = retry;
    fixture.needs_evidence = !retry;
    let backend = Arc::new(fixture);
    let clock = Arc::new(ManualClock::new(1));
    let mut session = Dispatcher::start(
        journal,
        content,
        request(),
        policy,
        backend.clone(),
        Arc::new(Direct::open(&root.0, CaptureLimits::default()).unwrap()),
        Arc::new(RetainedBytes),
        clock.clone(),
    )
    .unwrap();
    // This replay-heavy path competes with the complete two-journal scenario in
    // the workspace run. This is a hang watchdog, not a session or lease limit.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(240);
    while session.view().unwrap().finalization().delivered.is_none() {
        if let ymp_runtime::dispatcher::Tick::Waiting { until } = session.tick().unwrap() {
            clock.advance_to(until).unwrap();
        }
        if std::time::Instant::now() >= deadline {
            let current = session.view().unwrap();
            panic!(
                "recovery stalled: revision={}, status={:?}, calls={}",
                current.revision(),
                current.status(),
                current.execution().invocations().len(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let view = session.view().unwrap();
    let work = view
        .progress()
        .history
        .iter()
        .find_map(|(_, e)| {
            if let ymp_kernel::progress::ProgressRecorded::Action { outcome, .. } = e {
                if let ymp_kernel::progress::RecoveryOutcome::Work(w) = outcome.as_ref() {
                    Some(w)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    let call = view
        .execution()
        .invocations()
        .values()
        .find(|c| c.dispatch.assignment.contribution == work.contribution.id)
        .unwrap();
    assert_eq!(call.terminal, Some(InvocationTerminal::Completed));
    let source = call.end.as_ref().unwrap();
    let consumed = view
        .session_state()
        .recovered
        .iter()
        .find(|(r, _)| r == source)
        .unwrap();
    let revision = view.revision();
    assert_eq!(
        session
            .application()
            .recovery_consumed(&session.owner(), view.latest_at() + 1, source.clone())
            .unwrap(),
        consumed.1
    );
    assert_eq!(session.view().unwrap().revision(), revision);
    ymp_domain::journal::encode(view.session_state()).unwrap();
    assert!(
        work.prompt
            .basis
            .iter()
            .all(|r| call.dispatch.prompt.basis.contains(r))
    );
    if retry {
        assert_eq!(view.results().attempts().len(), 2);
        assert_eq!(view.results().results().len(), 2);
        let previous = &view.results().attempts()[work.retry_of.as_ref().unwrap()].attempt;
        assert_eq!(previous.outcome, AttemptOutcome::Abandoned);
        assert_eq!(
            view.coordination().commitments()[&view.admission().assignments()
                [&previous.assignment]
                .intent
                .assignment
                .commitment]
                .state,
            CommitmentState::Expired
        );
        assert_eq!(
            view.finalization().delivered.as_ref().unwrap().outcome,
            SessionStatus::Delivered
        );
    } else {
        assert_eq!(call.dispatch.assignment.role, RoleKind::Researcher);
        assert_eq!(
            view.coordination().commitments()[&call.dispatch.assignment.commitment].state,
            CommitmentState::Discharged
        );
        assert!(matches!(
            view.finalization().delivered.as_ref().unwrap().outcome,
            SessionStatus::Blocked(_)
        ));
        assert_eq!(backend.runs.lock().unwrap().len(), 6);
    }
}
#[test]
fn retry_waits_for_expiry_and_submits_a_distinct_result() {
    recovery_work(true);
}
#[test]
fn add_verifier_completes_paid_independent_research() {
    recovery_work(false);
}

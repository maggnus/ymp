//! An actual producer's immutable candidate survives later workspace edits.
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
    sync::Arc,
};
use ymp_domain::{
    Digest, Id, Proposal, Ref,
    assignment::*,
    coordination::*,
    journal::{Capability, PolicySelection},
    plan::*,
    resources::*,
    result::*,
    task::{EvidenceClass, Real},
    verification::*,
    workspace::*,
};
use ymp_kernel::{
    acceptance::{
        AcceptanceAuthority, ApplicabilityContext, EvidenceRecorded, EvidenceRequest,
        RegisterCheck, ReviewRequest, RunCheck, applicable_evidence, evidence_applicable,
        review_applicable,
    },
    gatekeeper::Gatekeeper,
    journal::{Journal, ParameterSchemas},
    ports::{execution::ExecutionBackend, resources::CostModel},
    results::Results,
};
use ymp_runtime::{
    backends::scripted::{Scripted, ScriptedStep},
    checks::{process::ProcessRunner, retained::RetainedBytes},
    clock::ManualClock,
    execution_host::{ExecutionHost, ExecutionStatus},
    policies::award::FirstOffer,
};
use ymp_storage::journal::SqliteJournal;
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn finish(
    host: &ExecutionHost<SqliteJournal, ymp_storage::content::SqliteContent>,
    live: &mut ymp_runtime::execution_host::LiveInvocation<SqliteJournal>,
) {
    // Invocation deadlines use ManualClock. This separate watchdog only detects
    // stalled test orchestration while debug SQLite replays the growing history.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    while host.poll(live).unwrap() != ExecutionStatus::Finished {
        assert!(
            std::time::Instant::now() < deadline,
            "{:?}",
            host.snapshot(live).unwrap()
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
#[test]
fn producer_candidate_retains_bytes_and_abandoned_retry_keeps_distinct_history() {
    let root = Directory::new();
    let database = Directory::new();
    std::fs::write(root.0.join("file"), b"baseline").unwrap();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let backend = Arc::new(
        Scripted::new(vec![
            ScriptedStep::Write {
                path: WorkspacePath::new("file").unwrap(),
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
        ])
        .unwrap(),
    );
    let mut s = Setup::new_with_selections(
        journal.clone(),
        &journal,
        &root,
        true,
        "results",
        None,
        FirstOffer::with_commitment_terms(CommitmentTerms {
            lease_duration: 90,
            renewal_duration: 10,
            renew_on: BTreeSet::new(),
            renewals: 0,
            release_delta: Real::new(1.0).unwrap(),
        })
        .unwrap(),
        vec![
            backend.selection().clone(),
            PolicySelection::new(
                "VerificationDesigner",
                "ExplicitVisible",
                "1",
                serde_json::json!({}),
            )
            .unwrap(),
        ],
    );
    // The fixture's replacement gate only reads state to build proposals; all
    // real grants, file handles and consumers retain the original issuer.
    let gate = Arc::new(std::mem::replace(
        &mut s.gate,
        Gatekeeper::new(journal.clone(), Arc::new(journal.content_store())),
    ));
    let results = Results::new(journal.clone(), gate.clone()).unwrap();
    let authority = s.intake.acceptance(Arc::new(journal.content_store()));
    let expected = register_bytes(&s, &authority, "expected-bytes", b"candidate");
    let contradicted = register_bytes(&s, &authority, "contradicted-bytes", b"baseline");
    let mut prior_evidence: Option<EvidenceRecorded> = None;
    s.files = false;
    let award = s.award("planning");
    let (revision, at, mut request) = s.request("planner", award);
    request.access = BTreeSet::from([Capability::ReadFiles]);
    request.files = Some(
        gate.workspace()
            .prepare_mediation(
                &s.session,
                revision,
                at,
                ymp_kernel::workspace_guard::LockRequest {
                    assignment: id("planner"),
                    workspace: id("workspace"),
                    profile: s.profile.clone(),
                    paths: vec![(WorkspacePath::new("file").unwrap(), LockMode::Read)],
                },
                s.provider.clone(),
            )
            .unwrap(),
    );
    let mut admission = gate.prepare(&s.session, revision, at, request).unwrap();
    let planner = gate.admit(&mut admission).unwrap();
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
    let view = gate.view(&s.session).unwrap();
    let basis = results
        .commit_plan(
            &planner.grant,
            view.revision(),
            at,
            Plan {
                id: id("plan"),
                session: s.session.clone(),
                version: 1,
                items: BTreeSet::from([id("item")]),
                rationale: "Explicit initial graph".into(),
                author: id("untrusted-author"),
            },
            item.clone(),
        )
        .unwrap();
    let clock = Arc::new(ManualClock::new(at));
    let host = ExecutionHost::new(
        journal.clone(),
        gate.clone(),
        backend.clone(),
        Arc::new(
            ymp_runtime::policies::resources::PriceWeighted::from_selection(s.cost.selection())
                .unwrap(),
        ),
        clock.clone(),
    )
    .unwrap();
    let mut planning = host
        .attach(
            planner,
            id("planning-call"),
            id("planning-receipt"),
            Prompt {
                text: "Record the explicit graph".into(),
                basis: vec![basis],
            },
        )
        .unwrap();
    host.start(&mut planning).unwrap();
    finish(&host, &mut planning);
    assert_eq!(
        gate.view(&s.session).unwrap().results().plans()[&id("plan")]
            .plan
            .author,
        id("planner")
    );

    s.files = true;
    for number in 1..=2 {
        let before = id(&format!("before-{number}"));
        let after = id(&format!("after-{number}"));
        let view = gate.view(&s.session).unwrap();
        let at = view.latest_at() + 1;
        let stale_before = id(&format!("stale-{number}"));
        gate.workspace()
            .snapshot(
                &s.session,
                view.revision(),
                at,
                &id("workspace"),
                stale_before.clone(),
                s.provider.as_ref(),
            )
            .unwrap();
        let name = format!("production-{number}");
        let award = s.award_subject(
            &name,
            Some(ContributionSubject::WorkItem(item.reference().unwrap())),
        );
        let (revision, at, request) = s.request_with_gate(gate.as_ref(), &name, award, "file");
        let mut admission = gate.prepare(&s.session, revision, at, request).unwrap();
        let producer = gate.admit(&mut admission).unwrap();
        clock.advance_to(at).unwrap();
        gate.workspace()
            .snapshot_unstarted(producer.files.as_ref().unwrap(), before.clone(), at)
            .unwrap();
        let view = gate.view(&s.session).unwrap();
        assert_eq!(
            results
                .prepare_attempt(
                    &producer.grant,
                    view.revision(),
                    at,
                    id("stale-baseline"),
                    stale_before,
                    id("stale-after")
                )
                .err()
                .unwrap()
                .code,
            "attempt_binding"
        );
        let attempt = results
            .prepare_attempt(
                &producer.grant,
                view.revision(),
                at,
                id(&format!("attempt-{number}")),
                before.clone(),
                after.clone(),
            )
            .unwrap();
        assert_eq!(
            results
                .prepare_attempt(
                    &producer.grant,
                    view.revision(),
                    at,
                    id("missing-baseline"),
                    id("missing"),
                    id("unused-after")
                )
                .err()
                .unwrap()
                .code,
            "snapshot_missing"
        );
        results.begin(&attempt).unwrap();
        let foreign = Results::new(journal.clone(), gate.clone()).unwrap();
        assert_eq!(
            foreign
                .submit(
                    &attempt,
                    at,
                    id("foreign-result"),
                    BTreeSet::new(),
                    "Foreign consumer".into()
                )
                .unwrap_err()
                .code,
            "attempt_owner"
        );
        let mut live = host
            .attach(
                producer,
                id(&format!("call-{number}")),
                id(&format!("receipt-{number}")),
                Prompt {
                    text: "Produce the declared file".into(),
                    basis: vec![item.reference().unwrap()],
                },
            )
            .unwrap();
        assert!(
            results
                .submit(
                    &attempt,
                    at,
                    id(&format!("early-{number}")),
                    BTreeSet::from([Artifact {
                        path: WorkspacePath::new("file").unwrap(),
                        digest: Digest::of(b"candidate")
                    }]),
                    "Too early".into()
                )
                .is_err()
        );
        host.start(&mut live).unwrap();
        finish(&host, &mut live);
        let artifact = Artifact {
            path: WorkspacePath::new("file").unwrap(),
            digest: Digest::of(b"candidate"),
        };
        let at = gate.view(&s.session).unwrap().latest_at();
        let bad = Artifact {
            digest: Digest::of(b"baseline"),
            ..artifact.clone()
        };
        assert_eq!(
            results
                .submit(
                    &attempt,
                    at,
                    id(&format!("bad-{number}")),
                    BTreeSet::from([bad]),
                    "Wrong digest".into()
                )
                .unwrap_err()
                .code,
            "artifact_binding"
        );
        let result = results
            .submit(
                &attempt,
                at,
                id(&format!("result-{number}")),
                BTreeSet::from([artifact.clone()]),
                "A retained candidate".into(),
            )
            .unwrap();
        assert_eq!(result.producer, s.profile.agent);
        assert_eq!(result.profile, s.profile);
        assert_eq!(result.before, before);
        assert_eq!(result.after, after);
        let supporting = check_evidence(&s, &authority, &result, &expected, number);
        let view = gate.view(&s.session).unwrap();
        assert_eq!(supporting.evidence.class, EvidenceClass::Executed);
        assert_eq!(supporting.evidence.independence, Independence::Trusted);
        assert_eq!(
            supporting.evidence.discrimination.baseline_fails,
            Some(true)
        );
        assert!(supporting.evidence.discrimination.candidate_passes);
        assert!(supporting.evidence.discrimination.mutation_score.is_none());
        assert!(evidence_applicable(&view, &supporting, &assessment(&supporting)).unwrap());
        assert_eq!(
            applicable_evidence(&view, &assessment(&supporting)).unwrap(),
            vec![&supporting]
        );
        let mut other_environment = assessment(&supporting);
        other_environment.environments.insert(
            supporting.scope.check.clone().unwrap(),
            BTreeSet::from([Digest::of(b"another environment")]),
        );
        assert!(
            applicable_evidence(&view, &other_environment)
                .unwrap()
                .is_empty()
        );
        if let Some(prior) = &prior_evidence {
            assert!(!evidence_applicable(&view, prior, &assessment(&supporting)).unwrap());
            assert!(view.evidence().contains_key(&prior.evidence.id));
        }
        prior_evidence = Some(supporting.clone());
        if number == 1 {
            check_negative_sources(
                &s,
                &authority,
                &result,
                &expected,
                &contradicted,
                &supporting,
            );
            record_independent_review(
                &mut s,
                &gate,
                &host,
                &clock,
                &authority,
                &result,
                &supporting,
            );
        }
        let at = gate.view(&s.session).unwrap().latest_at();
        let view = gate.view(&s.session).unwrap();
        assert_eq!(
            view.results().items()[&id("item")].state,
            WorkState::InReview
        );
        assert!(view.results().items()[&id("item")].accepted.is_none());
        assert_eq!(
            view.coordination().commitments()[&id(&name)].state,
            CommitmentState::Active
        );
        assert_eq!(
            results
                .submit(
                    &attempt,
                    at,
                    result.id.clone(),
                    BTreeSet::from([artifact]),
                    "Changed immutable summary".into()
                )
                .unwrap_err()
                .code,
            "result_conflict"
        );
        std::fs::write(
            root.0.join("file"),
            format!("later workspace edit {number}"),
        )
        .unwrap();
        assert_eq!(
            results
                .read_artifact(&s.session, &result.id, &WorkspacePath::new("file").unwrap())
                .unwrap(),
            b"candidate"
        );
        results
            .abandon(&attempt, at, "Preserve this candidate and retry".into())
            .unwrap();
        let reopened =
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        let view = reopened.view(&s.session, None).unwrap();
        assert_eq!(
            view.results().attempts()[attempt.id()].attempt.outcome,
            AttemptOutcome::Abandoned
        );
        assert_eq!(view.results().results()[&result.id], result);
        assert_eq!(
            view.results().items()[&id("item")].reference().unwrap(),
            item.reference().unwrap()
        );
        assert_eq!(view.results().results().len(), number);
        assert_eq!(
            gate.workspace()
                .read_artifact(&s.session, &before, &WorkspacePath::new("file").unwrap())
                .unwrap(),
            if number == 1 {
                b"baseline".to_vec()
            } else {
                b"later workspace edit 1".to_vec()
            }
        );
        // Abandonment preserves responsibility; the existing P2 expiration
        // permits a later new assignment without inventing successful discharge.
        let expired_at = view.coordination().commitments()[&id(&name)].lease.expires + 1;
        clock.advance_to(expired_at).unwrap();
        ymp_kernel::arbiter::Arbiter::new(journal.clone())
            .tick(gate.as_ref(), &s.session, expired_at)
            .unwrap();
    }
}

type Authority = AcceptanceAuthority<SqliteJournal, ymp_storage::content::SqliteContent>;
fn register_bytes(
    s: &Setup<SqliteJournal>,
    authority: &Authority,
    name: &str,
    bytes: &[u8],
) -> Check {
    let view = s.gate.view(&s.session).unwrap();
    let policy = PolicySelection::new(
        "VerificationDesigner",
        "ExplicitVisible",
        "1",
        serde_json::json!({}),
    )
    .unwrap();
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
                proposal: Proposal {
                    value: check,
                    rationale: "Explicit content criterion".into(),
                    basis: vec![],
                    policy: policy.policy.clone(),
                },
                effective: policy,
            },
        )
        .unwrap()
}
fn recorded_run(
    s: &Setup<SqliteJournal>,
    authority: &Authority,
    name: &str,
    check: &Check,
    target: &Id<Snapshot>,
    role: CheckRunRole,
    runner: &dyn ymp_kernel::ports::checks::CheckRunner,
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
fn evidence_request(
    s: &Setup<SqliteJournal>,
    name: &str,
    result: &ResultVersion,
    runs: Vec<Id<CheckRun>>,
    reviews: Vec<Id<Review>>,
) -> EvidenceRequest {
    let view = s.gate.view(&s.session).unwrap();
    EvidenceRequest {
        expected_revision: view.revision(),
        at: view.latest_at() + 1,
        id: id(name),
        criterion: view.criteria()[0].reference().unwrap(),
        result: result.reference().unwrap(),
        runs,
        reviews,
    }
}
fn check_evidence(
    s: &Setup<SqliteJournal>,
    authority: &Authority,
    result: &ResultVersion,
    check: &Check,
    number: usize,
) -> EvidenceRecorded {
    let baseline = recorded_run(
        s,
        authority,
        &format!("baseline-{number}"),
        check,
        &result.before,
        CheckRunRole::Baseline,
        &RetainedBytes,
    );
    let candidate = recorded_run(
        s,
        authority,
        &format!("candidate-{number}"),
        check,
        &result.after,
        CheckRunRole::Candidate,
        &RetainedBytes,
    );
    authority
        .evidence(
            &s.control,
            evidence_request(
                s,
                &format!("support-{number}"),
                result,
                vec![baseline.id, candidate.id],
                vec![],
            ),
        )
        .unwrap()
}
fn check_negative_sources(
    s: &Setup<SqliteJournal>,
    authority: &Authority,
    result: &ResultVersion,
    check: &Check,
    negative: &Check,
    supporting: &EvidenceRecorded,
) {
    let other = recorded_run(
        s,
        authority,
        "other-snapshot",
        check,
        &result.before,
        CheckRunRole::Candidate,
        &RetainedBytes,
    );
    assert_eq!(
        authority
            .evidence(
                &s.control,
                evidence_request(s, "wrong-snapshot", result, vec![other.id], vec![])
            )
            .unwrap_err()
            .code,
        "evidence_run"
    );
    let failed = recorded_run(
        s,
        authority,
        "failed-candidate",
        negative,
        &result.after,
        CheckRunRole::Candidate,
        &RetainedBytes,
    );
    let evidence = authority
        .evidence(
            &s.control,
            evidence_request(s, "contradiction", result, vec![failed.id], vec![]),
        )
        .unwrap();
    assert_eq!(evidence.evidence.polarity, Polarity::Contradicts);
    let view = s.gate.view(&s.session).unwrap();
    assert!(!evidence_applicable(&view, &evidence, &assessment(supporting)).unwrap());
    let runner = ProcessRunner::new(CheckLimits {
        timeout_ms: 1000,
        output_bytes: 4096,
    })
    .unwrap();
    let baseline = recorded_run(
        s,
        authority,
        "other-environment",
        check,
        &result.before,
        CheckRunRole::Baseline,
        &runner,
    );
    assert_eq!(
        authority
            .evidence(
                &s.control,
                evidence_request(
                    s,
                    "mixed-environments",
                    result,
                    vec![id("candidate-1"), baseline.id],
                    vec![]
                )
            )
            .unwrap_err()
            .code,
        "evidence_environment"
    );
    let mut forged = supporting.clone();
    forged.evidence.class = EvidenceClass::Browser;
    assert!(
        !evidence_applicable(
            &s.gate.view(&s.session).unwrap(),
            &forged,
            &assessment(supporting)
        )
        .unwrap()
    );
}
fn reviewer_request(
    s: &mut Setup<SqliteJournal>,
    gate: &Gatekeeper<SqliteJournal, ymp_storage::content::SqliteContent>,
    name: &str,
    result: &Ref,
) -> (u64, u64, ymp_kernel::gatekeeper::AdmissionRequest) {
    let award = s.award_kind(
        name,
        ContributionKind::Review,
        BTreeSet::from([Capability::ReadFiles]),
        Some(ContributionSubject::ResultVersion(result.clone())),
    );
    let files = s.files;
    s.files = false;
    let (revision, at, mut request) = s.request_with_gate(gate, name, award, "file");
    s.files = files;
    request.role = RoleKind::Reviewer;
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
                    paths: vec![(WorkspacePath::new("file").unwrap(), LockMode::Read)],
                },
                s.provider.clone(),
            )
            .unwrap(),
    );
    (revision, at, request)
}
fn record_independent_review(
    s: &mut Setup<SqliteJournal>,
    gate: &Arc<Gatekeeper<SqliteJournal, ymp_storage::content::SqliteContent>>,
    host: &ExecutionHost<SqliteJournal, ymp_storage::content::SqliteContent>,
    clock: &ManualClock,
    authority: &Authority,
    result: &ResultVersion,
    supporting: &EvidenceRecorded,
) {
    let (revision, at, request) =
        reviewer_request(s, gate, "self-review", &result.reference().unwrap());
    let mut denied = gate.prepare(&s.session, revision, at, request).unwrap();
    assert_eq!(
        gate.admit(&mut denied).err().unwrap().code,
        "review_independence"
    );
    let original = s.profile.clone();
    s.switch_to_new_agent("independent-reviewer");
    let (revision, at, request) =
        reviewer_request(s, gate, "review-assignment", &result.reference().unwrap());
    let mut admission = gate.prepare(&s.session, revision, at, request).unwrap();
    let reviewer = gate.admit(&mut admission).unwrap();
    let view = gate.view(&s.session).unwrap();
    let review = authority
        .review(
            gate,
            &reviewer.grant,
            ReviewRequest {
                expected_revision: view.revision(),
                at,
                id: id("review"),
                result: result.reference().unwrap(),
                criteria: vec![view.criteria()[0].reference().unwrap()],
                verdict: ReviewVerdict::Approve,
                findings: vec![Finding {
                    criterion: Some(id("criterion")),
                    text: "The byte comparison covers the submitted file".into(),
                    severity: FindingSeverity::Advisory,
                    proposed_check: Some(CheckSpec::ExactBytes {
                        path: WorkspacePath::new("file").unwrap(),
                        digest: Digest::of(b"candidate"),
                    }),
                }],
                basis: vec![supporting.evidence.id.clone()],
            },
        )
        .unwrap();
    assert_eq!(review.review.reviewer, s.profile.agent);
    assert_ne!(review.review.reviewer, result.producer);
    let inspection = authority
        .evidence(
            &s.control,
            evidence_request(
                s,
                "inspection",
                result,
                vec![],
                vec![review.review.id.clone()],
            ),
        )
        .unwrap();
    assert_eq!(inspection.evidence.class, EvidenceClass::Inspection);
    assert_eq!(
        inspection.evidence.independence,
        Independence::IndependentVisible
    );
    assert!(inspection.evidence.runs.is_empty());
    assert!(!inspection.evidence.discrimination.candidate_passes);
    let view = gate.view(&s.session).unwrap();
    let context = assessment(supporting);
    assert!(review_applicable(&view, &review, &context).unwrap());
    assert!(evidence_applicable(&view, &inspection, &context).unwrap());
    let mut changed = context.clone();
    changed.environments.insert(
        supporting.scope.check.clone().unwrap(),
        BTreeSet::from([Digest::of(b"changed assessment environment")]),
    );
    assert!(!review_applicable(&view, &review, &changed).unwrap());
    assert!(!evidence_applicable(&view, &inspection, &changed).unwrap());
    assert!(!evidence_applicable(&view, supporting, &changed).unwrap());
    assert!(view.reviews().contains_key(&review.review.id));
    assert!(view.results().items()[&result.item].accepted.is_none());
    // Raw adapter data cannot promote a review statement to Trusted evidence.
    let mut event = s
        .journal
        .read(&s.session)
        .unwrap()
        .events
        .last()
        .unwrap()
        .clone();
    event.seq = view.revision() + 1;
    event.input = Some(view.digest().unwrap());
    if let ymp_kernel::events::Event::EvidenceRecorded { data, .. } = &mut event.payload {
        data.evidence.id = id("forged-trust");
        data.evidence.independence = Independence::Trusted;
    } else {
        panic!("Expected evidence event");
    }
    assert_eq!(
        s.journal
            .append(&s.session, view.revision(), &[event])
            .unwrap_err()
            .code,
        "evidence_attribution"
    );
    assert_eq!(gate.view(&s.session).unwrap(), view);
    clock.advance_to(view.latest_at()).unwrap();
    let mut live = host
        .attach(
            reviewer,
            id("review-call"),
            id("review-receipt"),
            Prompt {
                text: "Review the exact retained candidate".into(),
                basis: vec![result.reference().unwrap()],
            },
        )
        .unwrap();
    host.start(&mut live).unwrap();
    finish(host, &mut live);
    assert_eq!(
        gate.view(&s.session).unwrap().coordination().commitments()[&id("review-assignment")].state,
        CommitmentState::Discharged
    );
    s.profile = original;
}

fn assessment(evidence: &EvidenceRecorded) -> ApplicabilityContext {
    ApplicabilityContext {
        result: evidence.scope.result.clone(),
        criteria: BTreeSet::from([evidence.scope.criterion.clone()]),
        environments: BTreeMap::from([(
            evidence.scope.check.clone().unwrap(),
            BTreeSet::from([evidence.scope.environment.clone().unwrap()]),
        )]),
    }
}

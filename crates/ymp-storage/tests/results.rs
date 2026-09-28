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
use std::{collections::BTreeSet, sync::Arc};
use ymp_domain::{
    Digest, Id, assignment::*, coordination::*, journal::Capability, plan::*, resources::*,
    result::*, task::Real, workspace::*,
};
use ymp_kernel::{
    gatekeeper::Gatekeeper,
    journal::{Journal, ParameterSchemas},
    ports::{execution::ExecutionBackend, resources::CostModel},
    results::Results,
};
use ymp_runtime::{
    backends::scripted::{Scripted, ScriptedStep},
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
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
        vec![backend.selection().clone()],
    );
    // The fixture's replacement gate only reads state to build proposals; all
    // real grants, file handles and consumers retain the original issuer.
    let gate = Arc::new(std::mem::replace(
        &mut s.gate,
        Gatekeeper::new(journal.clone(), Arc::new(journal.content_store())),
    ));
    let results = Results::new(journal.clone(), gate.clone()).unwrap();
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

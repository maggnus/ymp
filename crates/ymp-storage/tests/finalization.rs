//! Physical finalization scope is separate from global work and financial liveness.
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
use std::sync::Arc;
use ymp_domain::{Id, workspace::*};
use ymp_kernel::ports::reporting::{ClaimAuditor, NarrativeComposer};
use ymp_kernel::{finalization::Continuation, journal::ParameterSchemas};
use ymp_runtime::policies::{claim_audit::EvidenceClassRules, narrative::DeterministicReport};
use ymp_storage::journal::SqliteJournal;
fn id<T>(text: &str) -> Id<T> {
    Id::new(text).unwrap()
}
#[test]
fn final_capture_blocks_held_writes_but_not_an_unrelated_target() {
    let a = Directory::new();
    let b = Directory::new();
    let database = Directory::new();
    std::fs::write(a.0.join("file"), b"a").unwrap();
    std::fs::write(b.0.join("file"), b"b").unwrap();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let s = admission_fixture::Setup::new_named(journal.clone(), &journal, &a, true, "writer");
    let award = s.award("write");
    let (revision, at, request) = s.request("write", award);
    let mut prepared = s.gate.prepare(&s.session, revision, at, request).unwrap();
    let _writer = s.gate.admit(&mut prepared).unwrap();
    let finalizer = s.intake.finalization(Arc::new(journal.content_store()));
    let view = s.gate.view(&s.session).unwrap();
    finalizer
        .control(
            &s.control,
            view.revision(),
            view.latest_at() + 1,
            Continuation::Continue,
        )
        .unwrap();
    let view = s.gate.view(&s.session).unwrap();
    assert!(
        finalizer
            .capture(
                &s.control,
                view.revision(),
                view.latest_at() + 1,
                id("workspace"),
                id("blocked-snapshot"),
                vec![],
                s.provider.as_ref()
            )
            .unwrap()
            .is_none()
    );
    let view = s.gate.view(&s.session).unwrap();
    assert!(!view.snapshots().contains_key(&id("blocked-snapshot")));
    assert!(
        matches!(view.finalization().outcome,Some(ymp_domain::task::SessionStatus::Blocked(ref reason)) if reason=="effects_uncertain")
    );
    let deterministic = DeterministicReport::new().unwrap();
    let auditor = EvidenceClassRules::new().unwrap();
    let other = admission_fixture::Setup::new_with_selections(
        journal.clone(),
        &journal,
        &b,
        true,
        "isolated",
        None,
        ymp_runtime::policies::award::FirstOffer::new().unwrap(),
        vec![
            deterministic.selection().clone(),
            auditor.selection().clone(),
        ],
    );
    let other_final = other.intake.finalization(Arc::new(journal.content_store()));
    let view = other.gate.view(&other.session).unwrap();
    other_final
        .control(
            &other.control,
            view.revision(),
            view.latest_at() + 1,
            Continuation::Continue,
        )
        .unwrap();
    let view = other.gate.view(&other.session).unwrap();
    assert!(
        other_final
            .capture(
                &other.control,
                view.revision(),
                view.latest_at() + 1,
                id("workspace"),
                id("captured"),
                vec![],
                other.provider.as_ref()
            )
            .unwrap()
            .is_none()
    );
    let view = other.gate.view(&other.session).unwrap();
    assert!(view.snapshots().contains_key(&id("captured")));
    assert!(view.finalization().fence.is_some());
    assert!(
        other
            .gate
            .workspace()
            .prepare_mediation(
                &other.session,
                view.revision(),
                view.latest_at() + 1,
                ymp_kernel::workspace_guard::LockRequest {
                    assignment: id("late-write"),
                    workspace: id("workspace"),
                    profile: other.profile.clone(),
                    paths: vec![(WorkspacePath::new("file").unwrap(), LockMode::Write)]
                },
                other.provider.clone()
            )
            .is_err()
    );
    other_final
        .control(
            &other.control,
            view.revision(),
            view.latest_at() + 1,
            Continuation::Stopped,
        )
        .unwrap();
    let view = other.gate.view(&other.session).unwrap();
    other_final
        .control(
            &other.control,
            view.revision(),
            view.latest_at() + 1,
            Continuation::NoAuthority,
        )
        .unwrap();
    let view = other.gate.view(&other.session).unwrap();
    assert!(
        other_final
            .control(
                &other.control,
                view.revision(),
                view.latest_at() + 1,
                Continuation::Continue
            )
            .is_err()
    );
    let before_calls = view.execution().invocations().len();
    other_final
        .prepare_report(
            &other.control,
            view.revision(),
            view.latest_at() + 1,
            id("stopped-report"),
            &other.treasury,
            &other.budget_control,
        )
        .unwrap();
    let view = other.gate.view(&other.session).unwrap();
    let input = other_final.narrative_input(&other.session, None).unwrap();
    other_final
        .record_narrative(
            &other.control,
            view.revision(),
            view.latest_at() + 1,
            input.clone(),
            None,
            deterministic.compose(&input).unwrap(),
            None,
        )
        .unwrap();
    let view = other.gate.view(&other.session).unwrap();
    let inputs = other_final.claim_inputs(&other.session).unwrap();
    let proposals = inputs
        .iter()
        .map(|input| auditor.audit(input).unwrap())
        .collect();
    other_final
        .audit(
            &other.control,
            view.revision(),
            view.latest_at() + 1,
            inputs,
            proposals,
        )
        .unwrap();
    let view = other.gate.view(&other.session).unwrap();
    let report = other_final
        .deliver(&other.control, view.revision(), view.latest_at() + 1)
        .unwrap();
    assert_eq!(report.outcome, ymp_domain::task::SessionStatus::Cancelled);
    assert!(report.acceptance.is_none());
    assert_eq!(report.report.unmet, vec![id("criterion")]);
    assert_eq!(
        other
            .gate
            .view(&other.session)
            .unwrap()
            .execution()
            .invocations()
            .len(),
        before_calls
    );
    assert!(
        other
            .gate
            .view(&other.session)
            .unwrap()
            .finalization()
            .fence
            .is_none()
    );
}

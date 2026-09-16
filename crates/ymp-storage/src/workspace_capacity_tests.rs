use ymp_kernel as kernel;
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use crate::{journal::SqliteJournal, test_support::Directory};
use std::sync::Arc;
use ymp_domain::{
    Id,
    journal::{Actor, Envelope},
    workspace::*,
};
use ymp_kernel::{
    events::Event,
    journal::{Journal, ParameterSchemas},
    ports::execution::WorkspaceProvider,
    workspace_locks::{Cessation, CessationRecord, LockAcquisition, LockChange, attribution},
};
use ymp_runtime::workspace::direct::Direct;
fn id<T>(value: &str) -> Id<T> {
    Id::new(value).unwrap()
}
fn event(journal: &SqliteJournal, session: &Id, change: LockChange) -> Envelope<Event> {
    let view = journal.view(session, None).unwrap();
    Envelope {
        seq: view.revision() + 1,
        session: session.clone(),
        at: 5,
        actor: Actor::Runtime,
        policy: None,
        input: None,
        refs: attribution(&view, &change).unwrap(),
        payload: Event::LockChanged { version: 1, change },
    }
}
#[test]
fn unrelated_appends_cannot_consume_the_slots_needed_to_revoke_and_release_access() {
    let root = Directory::new();
    let database = Directory::new();
    let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let mut journal =
        SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    journal.event_limit = 8; // Exercise the actual adapter boundary with a small test-only limit.
    let journal = Arc::new(journal);
    let session = id("capacity");
    let (guard, profile) = fixture::open(
        journal.clone(),
        Arc::new(journal.content_store()),
        &session,
        &provider,
    );
    let view = guard.view(&session).unwrap();
    let workspace = &view.workspaces()[&id("workspace")];
    let lock = PathLock {
        path: WorkspacePath::root(),
        mode: LockMode::Write,
        holder: id("assignment"),
    };
    let acquired = event(
        &journal,
        &session,
        LockChange::Acquired(Box::new(LockAcquisition {
            mediated_owner: None,
            workspace: id("workspace"),
            assignment: id("assignment"),
            profile,
            requested: vec![lock.clone()],
            effective: vec![ObservedPathLock {
                lock,
                observation: provider
                    .observe_paths(&[WorkspacePath::root()])
                    .unwrap()
                    .remove(0),
            }],
            basis: vec![workspace.reference().unwrap()],
        })),
    );
    journal.append(&session, 4, &[acquired]).unwrap();
    // A non-ownership pool refresh would consume a slot needed for control.
    let registry = ymp_kernel::registry::Registry::new(journal.clone());
    let prior = guard.view(&session).unwrap().registry().unwrap().clone();
    let input = registry
        .prepare(&session, prior.input.facts.clone(), 6)
        .unwrap();
    let responses = ymp_kernel::registry::readiness_views(&input)
        .iter()
        .map(|v| ymp_kernel::registry::ReadinessResponse {
            profile: v.profile.clone(),
            input: ymp_domain::Digest::of_value(v).unwrap(),
            proposal: ymp_domain::Proposal {
                value: ymp_domain::identity::Readiness::Ready,
                rationale: "Repeated fixture readiness".into(),
                basis: vec![],
                policy: prior.effective.policy.clone(),
            },
        })
        .collect();
    assert_eq!(
        registry
            .record(&session, 5, 6, input, prior.effective, responses)
            .unwrap_err()
            .code,
        "journal_limit"
    );
    assert_eq!(journal.read(&session).unwrap().revision, 5);
    guard
        .authorize_access(
            &session,
            5,
            6,
            id("assignment"),
            id("invocation"),
            &provider,
        )
        .unwrap();
    guard
        .revoke_access(&session, 6, 7, id("assignment"), "Disconnected".into())
        .unwrap();
    let view = guard.view(&session).unwrap();
    let prior = &view.path_locks()[&id("assignment")];
    // Synthetic trusted cessation record; no native invocation is claimed.
    let released = event(
        &journal,
        &session,
        LockChange::Released(CessationRecord {
            assignment: id("assignment"),
            workspace: id("workspace"),
            invocation: Some(id("invocation")),
            state: prior.last.clone(),
            kind: Cessation::Terminated,
            basis: vec![prior.last.clone()],
        }),
    );
    journal.append(&session, 7, &[released]).unwrap();
    assert_eq!(journal.read(&session).unwrap().revision, 8);
    assert!(
        guard.view(&session).unwrap().path_locks()[&id("assignment")]
            .released
            .is_some()
    );
}

#[test]
fn creation_is_denied_before_io_without_room_to_publish_and_release() {
    for event_limit in [9, 11] {
        let root = Directory::new();
        let database = Directory::new();
        let provider = Arc::new(Direct::open(&root.0, CaptureLimits::default()).unwrap());
        let mut sqlite =
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        sqlite.event_limit = event_limit;
        let journal = Arc::new(sqlite);
        let session = id("creation-capacity");
        let (guard, profile) = fixture::open(
            journal.clone(),
            Arc::new(journal.content_store()),
            &session,
            provider.as_ref(),
        );
        guard
            .bind_workspace(&session, 4, 4, &id("workspace"), provider.as_ref())
            .unwrap();
        let access = guard
            .mediate(
                &session,
                5,
                5,
                ymp_kernel::workspace_guard::LockRequest {
                    assignment: id("assignment"),
                    workspace: id("workspace"),
                    profile,
                    paths: vec![(WorkspacePath::new("file").unwrap(), LockMode::Write)],
                },
                provider.clone(),
            )
            .unwrap();
        guard
            .authorize_access(
                &session,
                6,
                6,
                id("assignment"),
                id("invocation"),
                provider.as_ref(),
            )
            .unwrap();
        let written = access.write(&WorkspacePath::new("file").unwrap(), b"created", 12);
        if event_limit == 9 {
            assert!(written.is_err());
            assert!(!root.0.join("file").exists());
            assert_eq!(guard.view(&session).unwrap().revision(), 7);
            access.resolve_creation(13).unwrap();
        } else {
            written.unwrap();
            assert_eq!(std::fs::read(root.0.join("file")).unwrap(), b"created");
            assert_eq!(guard.view(&session).unwrap().revision(), 9);
            let registry = ymp_kernel::registry::Registry::new(journal.clone());
            let prior = guard.view(&session).unwrap().registry().unwrap().clone();
            let input = registry
                .prepare(&session, prior.input.facts.clone(), 13)
                .unwrap();
            let responses = ymp_kernel::registry::readiness_views(&input)
                .iter()
                .map(|view| ymp_kernel::registry::ReadinessResponse {
                    profile: view.profile.clone(),
                    input: ymp_domain::Digest::of_value(view).unwrap(),
                    proposal: ymp_domain::Proposal {
                        value: ymp_domain::identity::Readiness::Ready,
                        rationale: "Repeated fixture readiness".into(),
                        basis: vec![],
                        policy: prior.effective.policy.clone(),
                    },
                })
                .collect();
            let denied = registry
                .record(&session, 9, 13, input, prior.effective, responses)
                .unwrap_err();
            assert_eq!(denied.code, "journal_limit");
            guard
                .revoke_access(
                    &session,
                    9,
                    14,
                    id("assignment"),
                    "Stop after creation".into(),
                )
                .unwrap();
        }
        let proof = guard.withdraw_mediated(&access).unwrap();
        guard
            .release(
                &session,
                guard.view(&session).unwrap().revision(),
                15,
                &proof,
            )
            .unwrap();
        assert!(
            guard.view(&session).unwrap().path_locks()[&id("assignment")]
                .released
                .is_some()
        );
    }
}

use crate as storage;
#[allow(dead_code)]
#[path = "../tests/support/admission_fixture.rs"]
mod admission_fixture;
#[test]
fn admitted_assignments_retain_capacity_for_cross_service_revocation_and_release() {
    for files in [false, true] {
        let root = Directory::new();
        let database = Directory::new();
        let mut sqlite =
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        sqlite.event_limit = 32;
        let journal = Arc::new(sqlite);
        let s = admission_fixture::Setup::new(journal.clone(), &journal, &root, files);
        let award = s.award("work");
        let (expected, at, request) = s.request("assignment", award);
        let mut prepared = s.gate.prepare(&s.session, expected, at, request).unwrap();
        let admitted = s.gate.admit(&mut prepared).unwrap();
        let registry = ymp_kernel::registry::Registry::new(journal.clone());
        loop {
            let view = s.gate.view(&s.session).unwrap();
            let at = view.latest_at() + 1;
            let prior = view.registry().unwrap().clone();
            let input = registry
                .prepare(&s.session, prior.input.facts.clone(), at)
                .unwrap();
            let responses = ymp_kernel::registry::readiness_views(&input)
                .iter()
                .map(|view| ymp_kernel::registry::ReadinessResponse {
                    profile: view.profile.clone(),
                    input: ymp_domain::Digest::of_value(view).unwrap(),
                    proposal: ymp_domain::Proposal {
                        value: ymp_domain::identity::Readiness::Ready,
                        rationale: "Repeated fixture readiness".into(),
                        basis: vec![],
                        policy: prior.effective.policy.clone(),
                    },
                })
                .collect();
            match registry.record(
                &s.session,
                view.revision(),
                at,
                input,
                prior.effective,
                responses,
            ) {
                Ok(_) => {}
                Err(error) => {
                    assert_eq!(error.code, "journal_limit");
                    break;
                }
            }
        }
        let view = s.gate.view(&s.session).unwrap();
        let at = view.latest_at() + 1;
        s.gate
            .revoke(
                &s.session,
                view.revision(),
                at,
                &admitted.assignment.id,
                "Stop near journal capacity".into(),
            )
            .unwrap();
        assert!(
            s.gate
                .authorize(
                    &admitted.grant,
                    ymp_domain::assignment::TeamOperation::BoardRead,
                    at
                )
                .is_err()
        );
        if let Some(access) = admitted.files {
            let proof = s.gate.workspace().withdraw_mediated(&access).unwrap();
            s.gate
                .workspace()
                .release(
                    &s.session,
                    s.gate.view(&s.session).unwrap().revision(),
                    at,
                    &proof,
                )
                .unwrap();
        }
        let treasury = ymp_kernel::treasury::Treasury::new(journal.clone());
        let proof = treasury
            .never_started(&s.session, &id("assignment"))
            .unwrap();
        treasury
            .release_unstarted(
                &s.session,
                s.gate.view(&s.session).unwrap().revision(),
                at,
                &proof,
            )
            .unwrap();
        let view = s.gate.view(&s.session).unwrap();
        assert_eq!(
            view.admission().assignments()[&id("assignment")]
                .intent
                .assignment
                .state,
            ymp_domain::assignment::AssignmentState::Revoked
        );
        assert_eq!(
            view.treasury().unwrap().accounts[&id("assignment")]
                .reservation
                .state,
            ymp_domain::resources::ReservationState::Released
        );
    }
}

#[test]
fn financial_closure_consumes_its_reserved_slots_at_the_journal_limit() {
    for unknown_first in [false, true] {
        let root = Directory::new();
        let database = Directory::new();
        let mut sqlite =
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        sqlite.event_limit = 32;
        let journal = Arc::new(sqlite);
        let s = admission_fixture::Setup::new(journal.clone(), &journal, &root, false);
        let award = s.award("work");
        let (expected, at, request) = s.request("assignment", award);
        let mut prepared = s.gate.prepare(&s.session, expected, at, request).unwrap();
        let admitted = s.gate.admit(&mut prepared).unwrap();
        let registry = ymp_kernel::registry::Registry::new(journal.clone());
        loop {
            let view = s.gate.view(&s.session).unwrap();
            let at = view.latest_at() + 1;
            let prior = view.registry().unwrap().clone();
            let input = registry
                .prepare(&s.session, prior.input.facts.clone(), at)
                .unwrap();
            let responses = ymp_kernel::registry::readiness_views(&input)
                .iter()
                .map(|view| ymp_kernel::registry::ReadinessResponse {
                    profile: view.profile.clone(),
                    input: ymp_domain::Digest::of_value(view).unwrap(),
                    proposal: ymp_domain::Proposal {
                        value: ymp_domain::identity::Readiness::Ready,
                        rationale: "Repeated fixture readiness".into(),
                        basis: vec![],
                        policy: prior.effective.policy.clone(),
                    },
                })
                .collect();
            match registry.record(
                &s.session,
                view.revision(),
                at,
                input,
                prior.effective,
                responses,
            ) {
                Ok(_) => {}
                Err(error) => {
                    assert_eq!(error.code, "journal_limit");
                    break;
                }
            }
        }
        let treasury = ymp_kernel::treasury::Treasury::new(journal.clone());
        let view = s.gate.view(&s.session).unwrap();
        let at = view.latest_at() + 1;
        treasury
            .authorize(
                &s.session,
                view.revision(),
                at,
                id("assignment"),
                id("invocation"),
            )
            .unwrap();
        if unknown_first {
            treasury
                .observe(
                    &s.session,
                    s.gate.view(&s.session).unwrap().revision(),
                    at,
                    id("assignment"),
                    ymp_domain::resources::Receipt {
                        id: id("receipt"),
                        invocation: id("invocation"),
                        usage: ymp_domain::resources::Usage {
                            input: 0,
                            cache_read: 0,
                            cache_write: 0,
                            output: 0,
                            reasoning: None,
                        },
                        coverage: ymp_domain::resources::Coverage::Unknown,
                        cost: None,
                    },
                    &[],
                )
                .unwrap();
        }
        treasury
            .observe(
                &s.session,
                s.gate.view(&s.session).unwrap().revision(),
                at,
                id("assignment"),
                ymp_domain::resources::Receipt {
                    id: id("receipt"),
                    invocation: id("invocation"),
                    usage: ymp_domain::resources::Usage {
                        input: 5,
                        cache_read: 0,
                        cache_write: 0,
                        output: 0,
                        reasoning: None,
                    },
                    coverage: ymp_domain::resources::Coverage::Complete,
                    cost: None,
                },
                &[],
            )
            .unwrap();
        let view = s.gate.view(&s.session).unwrap();
        let response = ymp_runtime::policies::resources::cost_response(
            &s.cost,
            &ymp_kernel::treasury::cost_view(&view, &id("assignment")).unwrap(),
        )
        .unwrap();
        treasury
            .settle(&s.session, view.revision(), at, id("assignment"), response)
            .unwrap();
        s.gate
            .revoke(
                &s.session,
                s.gate.view(&s.session).unwrap().revision(),
                at,
                &admitted.assignment.id,
                "After accounted execution".into(),
            )
            .unwrap();
        assert_eq!(
            s.gate
                .view(&s.session)
                .unwrap()
                .treasury()
                .unwrap()
                .accounts[&id("assignment")]
                .reservation
                .state,
            ymp_domain::resources::ReservationState::Settled
        );
    }
}

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

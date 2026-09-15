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
    // Snapshot publication would leave too few events for the outstanding owner.
    assert!(
        guard
            .snapshot(&session, 5, 6, &id("workspace"), id("extra"), &provider)
            .is_err()
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

//! Trusted owner recovery and atomic stop through both journal adapters.
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
use ymp_domain::{
    Id,
    assignment::TeamOperation,
    resources::{ReportingMode, ReservationState},
    task::SessionStatus,
};
use ymp_kernel::{
    journal::{Journal, ParameterSchemas},
    session::SessionChange,
};
use ymp_runtime::{
    application::{Application, RecoveryIntent},
    memory_journal::MemoryJournal,
};
use ymp_storage::journal::SqliteJournal;
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn controls<J: Journal + 'static>(journal: Arc<J>, store: &SqliteJournal, root: &Directory) {
    let s = admission_fixture::Setup::new(journal.clone(), store, root, true);
    let award = s.award("work");
    let (revision, at, request) = s.request("work", award);
    let mut prepared = s.gate.prepare(&s.session, revision, at, request).unwrap();
    let admitted = s.gate.admit(&mut prepared).unwrap();
    let app = Application::new(journal.clone());
    let recovered = app
        .recover(&s.session, at, RecoveryIntent::Continue)
        .unwrap();
    let before = app.view(&s.session, None).unwrap();
    let original = before.treasury().unwrap().budget.clone();
    // A stale owner timestamp/revision cannot starve stop behind newer work facts.
    journal
        .session_control(
            &s.session,
            at + 100,
            SessionChange::Phase(SessionStatus::Running),
        )
        .unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let writer_journal = journal.clone();
    let writer_session = s.session.clone();
    let entered = barrier.clone();
    let writer = std::thread::spawn(move || {
        entered.wait();
        for _ in 0..16 {
            if writer_journal
                .session_control(
                    &writer_session,
                    at + 100,
                    SessionChange::Phase(SessionStatus::Running),
                )
                .is_err()
            {
                break;
            }
        }
    });
    barrier.wait();
    let stopped = app.interrupt(&recovered.control, 1).unwrap();
    writer.join().unwrap();
    assert!(recovered.control.stopped());
    let view = app.view(&s.session, None).unwrap();
    assert!(view.owner_stopped());
    assert_eq!(view.status(), Some(SessionStatus::Cancelled));
    assert!(
        s.gate
            .authorize(&admitted.grant, TeamOperation::BoardRead, view.latest_at())
            .is_err()
    );
    assert!(view.path_locks()[&id("work")].released.is_none());
    assert_eq!(view.treasury().unwrap().budget.held, original.held);
    assert_eq!(
        view.treasury().unwrap().accounts[&id("work")]
            .reservation
            .state,
        ReservationState::Held
    );
    assert_eq!(app.interrupt(&recovered.control, 1).unwrap(), stopped);
    assert_eq!(
        app.view(&s.session, None).unwrap().revision(),
        view.revision()
    );
    let reopened = Application::new(journal.clone());
    assert!(reopened.interrupt(&recovered.control, 1).is_err());
    let observed = reopened
        .recover(&s.session, 1, RecoveryIntent::Observe)
        .unwrap();
    assert!(observed.control.stopped());
    let continued = reopened
        .recover(&s.session, 1, RecoveryIntent::Continue)
        .unwrap();
    assert!(!continued.control.stopped());
    assert!(recovered.control.stopped());
    let view = reopened.view(&s.session, None).unwrap();
    assert!(
        s.gate
            .authorize(&admitted.grant, TeamOperation::BoardRead, view.latest_at())
            .is_err()
    );
    assert_eq!(view.treasury().unwrap().budget, original);
    reopened
        .treasury()
        .start_reporting(
            continued.budget.as_ref().unwrap(),
            view.revision(),
            view.latest_at(),
            ReportingMode::Deterministic,
        )
        .unwrap();
    assert!(
        reopened
            .recover(&s.session, 1, RecoveryIntent::Continue)
            .is_err()
    );
    assert_eq!(
        reopened
            .view(&s.session, None)
            .unwrap()
            .treasury()
            .unwrap()
            .budget
            .id,
        original.id
    );
}
#[test]
fn owner_recovery_and_atomic_stop_preserve_budget_and_holds() {
    for memory in [true, false] {
        let root = Directory::new();
        let database = Directory::new();
        std::fs::write(root.0.join("file"), b"owned").unwrap();
        let store = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        if memory {
            let journal = Arc::new(
                MemoryJournal::with_binding_identity(ParameterSchemas::default()).unwrap(),
            );
            controls(journal, &store, &root);
        } else {
            let journal = Arc::new(
                SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap(),
            );
            controls(journal, &store, &root);
        }
    }
}

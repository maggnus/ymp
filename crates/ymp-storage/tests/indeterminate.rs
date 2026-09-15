mod support;
use std::{collections::BTreeMap, sync::Arc};
use ymp_domain::{
    Id, Proposal,
    journal::{EscalationStep, Method, MethodKind},
};
use ymp_kernel::{
    decision::{DecisionConsumer, MethodDecision},
    journal::{AppendResolution, Journal, ParameterSchemas},
};
use ymp_storage::journal::SqliteJournal;

#[test]
fn lost_acknowledgement_resolves_after_a_later_append_without_repeating_events() {
    let directory = support::Directory::new();
    let journal =
        Arc::new(SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap());
    let session = Id::new("acknowledgement").unwrap();
    let opening = support::opening(&session);
    assert_eq!(
        journal
            .resolve_append(&session, 0, std::slice::from_ref(&opening))
            .unwrap(),
        AppendResolution::Absent
    );
    journal
        .append(&session, 0, std::slice::from_ref(&opening))
        .unwrap();
    let second =
        Arc::new(SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap());
    let view = second.view(&session, None).unwrap();
    let policy = support::policy("SoloWithVerifier").policy;
    let method = Method {
        id: Id::new("method").unwrap(),
        kind: MethodKind::SoloWithVerifier,
        ladder: vec![EscalationStep::StopPreserving],
        params: BTreeMap::new(),
        policy: policy.clone(),
    };
    DecisionConsumer::new(second.clone())
        .record_method(
            &session,
            MethodDecision {
                expected_revision: 1,
                at: 2,
                input: view.digest().unwrap(),
                proposal: Proposal {
                    value: method,
                    rationale: "Choose the method".into(),
                    basis: vec![],
                    policy,
                },
            },
            None,
        )
        .unwrap();
    drop(second);
    drop(journal);
    let journal = SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    assert_eq!(
        journal
            .resolve_append(&session, 0, std::slice::from_ref(&opening))
            .unwrap(),
        AppendResolution::Committed(1)
    );
    assert!(
        journal
            .append(&session, 0, std::slice::from_ref(&opening))
            .is_err()
    );
    let mut different = opening;
    different.at = 9;
    assert_eq!(
        journal.resolve_append(&session, 0, &[different]).unwrap(),
        AppendResolution::Conflict { revision: 2 }
    );
    assert_eq!(journal.read(&session).unwrap().revision, 2);
}

#[test]
fn corruption_during_resolution_is_never_reported_as_an_absent_append() {
    let directory = support::Directory::new();
    let journal = SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    let session = Id::new("corrupt-resolution").unwrap();
    let event = support::opening(&session);
    journal
        .append(&session, 0, std::slice::from_ref(&event))
        .unwrap();
    let connection = rusqlite::Connection::open(directory.database()).unwrap();
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .unwrap();
    connection
        .execute("DELETE FROM content_values", [])
        .unwrap();
    assert!(journal.resolve_append(&session, 0, &[event]).is_err());
}

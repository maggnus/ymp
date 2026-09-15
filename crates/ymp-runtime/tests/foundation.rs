use std::{
    collections::BTreeMap,
    sync::{Arc, Barrier},
    thread,
};
use ymp_domain::{
    Digest, Id, Proposal, Ref,
    journal::{
        Actor, Decision, Envelope, EscalationStep, Method, MethodParameters, PolicySelection,
        decode, encode,
    },
};
use ymp_kernel::{
    decision::{DecisionConsumer, MethodDecision},
    events::Event,
    journal::{Journal, ParameterSchemas},
    view::SessionView,
};
use ymp_runtime::memory_journal::MemoryJournal;

fn policy(name: &str, ladder: &[EscalationStep]) -> PolicySelection {
    PolicySelection::new(
        "MethodRouter",
        name,
        "1",
        serde_json::json!({"kind":"SoloWithVerifier","ladder":ladder}),
    )
    .unwrap()
}

// Local strategy implementations receive immutable projections and their explicit
// configuration. Neither receives a Journal or the owner's SessionControl.
struct Fixed {
    selection: PolicySelection,
}
impl Fixed {
    fn propose(&self, view: &SessionView) -> Proposal<Method> {
        let params = MethodParameters::from_selection(&self.selection).unwrap();
        Proposal {
            value: Method {
                id: Id::new("method").unwrap(),
                kind: params.kind,
                ladder: params.ladder,
                params: BTreeMap::new(),
                policy: self.selection.policy.clone(),
            },
            rationale: format!("Use the configured method at revision {}", view.revision()),
            basis: vec![],
            policy: self.selection.policy.clone(),
        }
    }
}
struct EvidenceFirst {
    selection: PolicySelection,
}
impl EvidenceFirst {
    fn propose(&self, view: &SessionView) -> Proposal<Method> {
        let params: EvidenceParameters =
            decode(&encode(&self.selection.parameters).unwrap()).unwrap();
        let mut ladder = vec![EscalationStep::StopPreserving];
        if params.verify_after_choice && view.method().is_some() {
            ladder.insert(0, EscalationStep::AddVerifier);
        }
        Proposal {
            value: Method {
                id: Id::new("method").unwrap(),
                kind: params.kind,
                ladder,
                params: BTreeMap::new(),
                policy: self.selection.policy.clone(),
            },
            rationale: "After a previous choice, obtain independent evidence before retrying"
                .into(),
            basis: view
                .method()
                .map(|m| {
                    vec![Ref {
                        id: m.id.erased(),
                        version: Digest::of_value(m).unwrap(),
                    }]
                })
                .unwrap_or_default(),
            policy: self.selection.policy.clone(),
        }
    }
}
fn request(view: &SessionView, proposal: Proposal<Method>) -> MethodDecision {
    MethodDecision {
        expected_revision: view.revision(),
        at: 2000 + view.revision(),
        input: view.digest().unwrap(),
        proposal,
    }
}

#[test]
fn real_consumer_preserves_decisions_across_authorized_policy_changes() {
    let journal = Arc::new(MemoryJournal::with_schemas(test_schemas()));
    let consumer = DecisionConsumer::new(journal.clone());
    let session = Id::new("run-a").unwrap();
    let first = policy(
        "FixedMethod",
        &[EscalationStep::Retry, EscalationStep::StopPreserving],
    );
    let owner = consumer
        .open(session.clone(), 1000, vec![first.clone()])
        .unwrap();
    let view = journal.view(&session, None).unwrap();
    let proposed = Fixed {
        selection: first.clone(),
    }
    .propose(&view);
    consumer
        .record_method(&session, request(&view, proposed.clone()), None)
        .unwrap();
    let historical = journal.view(&session, Some(2)).unwrap();
    assert_eq!(historical.decisions()[0].proposal, proposed);
    assert_eq!(historical.decisions()[0].effective, first);
    assert_eq!(historical.decisions()[0].input, view.digest().unwrap());
    assert_eq!(historical.decisions()[0].outcome, proposed.value);
    let second = PolicySelection::new(
        "MethodRouter",
        "EvidenceFirstTest",
        "1",
        serde_json::json!({"kind":"SoloWithVerifier","verify_after_choice":true}),
    )
    .unwrap();
    let proposed = EvidenceFirst {
        selection: second.clone(),
    }
    .propose(&historical);
    let before = journal.read(&session).unwrap();
    assert!(
        consumer
            .record_method(&session, request(&historical, proposed.clone()), None)
            .is_err()
    );
    assert_eq!(journal.read(&session).unwrap(), before);
    consumer
        .record_method(
            &session,
            request(&historical, proposed),
            Some((&owner, second.clone())),
        )
        .unwrap();
    let current = journal.view(&session, None).unwrap();
    assert_eq!(
        current.method().unwrap().ladder,
        vec![EscalationStep::AddVerifier, EscalationStep::StopPreserving]
    );
    assert_eq!(current.policies()["MethodRouter"], second);
    assert_eq!(journal.view(&session, Some(2)).unwrap(), historical);
    assert_eq!(current.decisions()[0].effective, first);
    assert_eq!(
        current.decisions()[1]
            .selection_change
            .as_ref()
            .unwrap()
            .boundary,
        2
    );
    let bytes = encode(&journal.read(&session).unwrap().events).unwrap();
    let events: Vec<Envelope<Event>> = decode(&bytes).unwrap();
    assert_eq!(
        SessionView::replay_with_schemas(session.clone(), &events, journal.schemas()).unwrap(),
        current
    );
    assert_eq!(
        current.digest().unwrap(),
        SessionView::replay_with_schemas(session, &events, journal.schemas())
            .unwrap()
            .digest()
            .unwrap()
    );
}

#[test]
fn wrong_refs_foreign_controls_and_invalid_proposals_leave_history_unchanged() {
    let journal = Arc::new(MemoryJournal::new());
    let consumer = DecisionConsumer::new(journal.clone());
    let session = Id::new("run-a").unwrap();
    let configured = policy("FixedMethod", &[EscalationStep::StopPreserving]);
    consumer
        .open(session.clone(), 1, vec![configured.clone()])
        .unwrap();
    let other_consumer = DecisionConsumer::new(Arc::new(MemoryJournal::new()));
    let foreign = other_consumer
        .open(session.clone(), 1, vec![configured.clone()])
        .unwrap();
    let view = journal.view(&session, None).unwrap();
    let proposed = Fixed {
        selection: configured.clone(),
    }
    .propose(&view);
    let saved = journal.read(&session).unwrap();
    let different = policy("Other", &[EscalationStep::Retry]);
    assert_eq!(
        consumer
            .record_method(
                &session,
                request(&view, proposed.clone()),
                Some((&foreign, different))
            )
            .unwrap_err()
            .code,
        "owner_authority"
    );
    for reference in [
        Ref {
            id: Id::new("absent").unwrap(),
            version: Digest::of(b"missing"),
        },
        Ref {
            id: saved.events[0].reference().unwrap().id,
            version: Digest::of(b"wrong version"),
        },
    ] {
        let mut bad = proposed.clone();
        bad.basis.push(reference);
        assert_eq!(
            consumer
                .record_method(&session, request(&view, bad), None)
                .unwrap_err()
                .code,
            "missing_ref"
        );
        assert_eq!(journal.read(&session).unwrap(), saved);
    }
    let mut bad = proposed.clone();
    bad.value.policy.version = "wrong".into();
    assert!(
        consumer
            .record_method(&session, request(&view, bad), None)
            .is_err()
    );
    let mut bad = request(&view, proposed.clone());
    bad.input = Digest::of(b"not the view");
    assert_eq!(
        consumer
            .record_method(&session, bad, None)
            .unwrap_err()
            .code,
        "input_view"
    );
    let mut bad = request(&view, proposed);
    bad.expected_revision = 0;
    assert_eq!(
        consumer
            .record_method(&session, bad, None)
            .unwrap_err()
            .code,
        "stale_revision"
    );
    assert_eq!(journal.read(&session).unwrap(), saved);
}

#[test]
fn concurrent_decisions_have_one_winner_at_the_same_revision() {
    let journal = Arc::new(MemoryJournal::new());
    let consumer = Arc::new(DecisionConsumer::new(journal.clone()));
    let session = Id::new("race").unwrap();
    let configured = policy("FixedMethod", &[EscalationStep::StopPreserving]);
    consumer
        .open(session.clone(), 1, vec![configured.clone()])
        .unwrap();
    let view = journal.view(&session, None).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let (consumer, session, view, barrier, selection) = (
                consumer.clone(),
                session.clone(),
                view.clone(),
                barrier.clone(),
                configured.clone(),
            );
            thread::spawn(move || {
                let proposed = Fixed { selection }.propose(&view);
                barrier.wait();
                consumer.record_method(&session, request(&view, proposed), None)
            })
        })
        .collect();
    let outcomes: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter_map(|r| r.as_ref().err())
            .next()
            .unwrap()
            .code,
        "stale_revision"
    );
    assert_eq!(journal.read(&session).unwrap().revision, 2);
}

#[test]
fn raw_adapter_batches_are_atomic_and_replay_rechecks_decision_metadata() {
    let journal = Arc::new(MemoryJournal::new());
    let consumer = DecisionConsumer::new(journal.clone());
    let session = Id::new("atomic").unwrap();
    let configured = policy("FixedMethod", &[EscalationStep::StopPreserving]);
    consumer
        .open(session.clone(), 1, vec![configured.clone()])
        .unwrap();
    let view = journal.view(&session, None).unwrap();
    let proposed = Fixed {
        selection: configured.clone(),
    }
    .propose(&view);
    let mut event = Envelope {
        seq: 2,
        session: session.clone(),
        at: 2,
        actor: Actor::Runtime,
        policy: Some(configured.policy.clone()),
        input: Some(view.digest().unwrap()),
        refs: vec![],
        payload: Event::MethodChosen {
            version: 1,
            decision: Box::new(Decision {
                outcome: proposed.value.clone(),
                proposal: proposed,
                effective: configured,
                input: view.digest().unwrap(),
                selection_change: None,
            }),
        },
    };
    let saved = journal.read(&session).unwrap();
    let mut bad = event.clone();
    bad.seq = 3;
    if let Event::MethodChosen { version, .. } = &mut bad.payload {
        *version = 999;
    }
    assert!(journal.append(&session, 1, &[event.clone(), bad]).is_err());
    assert_eq!(journal.read(&session).unwrap(), saved);
    if let Event::MethodChosen { decision, .. } = &mut event.payload {
        decision.outcome.ladder.push(EscalationStep::Retry);
    }
    assert!(journal.append(&session, 1, &[event]).is_err());
    assert_eq!(journal.read(&session).unwrap(), saved);
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceParameters {
    kind: ymp_domain::journal::MethodKind,
    verify_after_choice: bool,
}

fn test_schemas() -> ParameterSchemas {
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("MethodRouter", "EvidenceFirstTest", "1", |selection| {
            let params: EvidenceParameters = decode(&encode(&selection.parameters)?)?;
            if params.kind != ymp_domain::journal::MethodKind::SoloWithVerifier {
                return Err(ymp_domain::Denial::new(
                    "parameters",
                    "This test strategy requires SoloWithVerifier",
                ));
            }
            Ok(())
        })
        .unwrap();
    schemas
}

#[test]
fn unknown_policy_version_and_wrong_schema_do_not_open_a_session() {
    let journal = Arc::new(MemoryJournal::new());
    let consumer = DecisionConsumer::new(journal.clone());
    let session = Id::new("unopened").unwrap();
    let mut unknown = policy("FixedMethod", &[EscalationStep::StopPreserving]);
    unknown.policy.version = "999".into();
    assert!(consumer.open(session.clone(), 1, vec![unknown]).is_err());
    let malformed = PolicySelection::new(
        "MethodRouter",
        "FixedMethod",
        "1",
        serde_json::json!({"kind":"SoloWithVerifier","verify_after_choice":true}),
    )
    .unwrap();
    assert!(consumer.open(session.clone(), 1, vec![malformed]).is_err());
    assert_eq!(journal.read(&session).unwrap().revision, 0);
    assert!(journal.read(&session).unwrap().events.is_empty());
}

#[test]
fn a_registered_schema_for_another_port_cannot_choose_a_method() {
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("ResourcePolicy", "Test", "1", |_| Ok(()))
        .unwrap();
    let journal = Arc::new(MemoryJournal::with_schemas(schemas));
    let consumer = DecisionConsumer::new(journal.clone());
    let session = Id::new("wrong-port").unwrap();
    let configured = policy("FixedMethod", &[EscalationStep::StopPreserving]);
    let owner = consumer
        .open(session.clone(), 1, vec![configured.clone()])
        .unwrap();
    let view = journal.view(&session, None).unwrap();
    let wrong = PolicySelection::new("ResourcePolicy", "Test", "1", serde_json::json!({})).unwrap();
    let mut proposed = Fixed {
        selection: configured.clone(),
    }
    .propose(&view);
    proposed.policy = wrong.policy.clone();
    proposed.value.policy = wrong.policy.clone();
    let saved = journal.read(&session).unwrap();
    assert_eq!(
        consumer
            .record_method(
                &session,
                request(&view, proposed.clone()),
                Some((&owner, wrong.clone()))
            )
            .unwrap_err()
            .code,
        "policy_port"
    );
    let forged = Envelope {
        seq: 2,
        session: session.clone(),
        at: 2,
        actor: Actor::Runtime,
        policy: Some(wrong.policy.clone()),
        input: Some(view.digest().unwrap()),
        refs: vec![],
        payload: Event::MethodChosen {
            version: 1,
            decision: Box::new(Decision {
                outcome: proposed.value.clone(),
                proposal: proposed,
                effective: wrong,
                input: view.digest().unwrap(),
                selection_change: Some(ymp_domain::journal::SelectionChange {
                    previous: configured.policy,
                    boundary: 1,
                }),
            }),
        },
    };
    assert_eq!(
        journal.append(&session, 1, &[forged]).unwrap_err().code,
        "policy_port"
    );
    assert_eq!(journal.read(&session).unwrap(), saved);
    assert_eq!(journal.view(&session, None).unwrap(), view);
}

#[test]
fn an_adapter_success_cannot_authorize_an_invalid_method() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use ymp_kernel::journal::JournalRead;
    struct Permissive {
        history: JournalRead,
        schemas: ParameterSchemas,
        writes: AtomicUsize,
    }
    impl Journal for Permissive {
        fn schemas(&self) -> &ParameterSchemas {
            &self.schemas
        }
        fn read(&self, _: &Id) -> ymp_domain::Result<JournalRead> {
            Ok(self.history.clone())
        }
        fn append(&self, _: &Id, expected: u64, _: &[Envelope<Event>]) -> ymp_domain::Result<u64> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(expected + 1)
        }
    }
    let memory = Arc::new(MemoryJournal::new());
    let session = Id::new("adapter-boundary").unwrap();
    let selection = policy("FixedMethod", &[EscalationStep::StopPreserving]);
    DecisionConsumer::new(memory.clone())
        .open(session.clone(), 1, vec![selection.clone()])
        .unwrap();
    let view = memory.view(&session, None).unwrap();
    let adapter = Arc::new(Permissive {
        history: memory.read(&session).unwrap(),
        schemas: ParameterSchemas::default(),
        writes: AtomicUsize::new(0),
    });
    let consumer = DecisionConsumer::new(adapter.clone());
    let mut proposed = Fixed { selection }.propose(&view);
    proposed.value.kind = ymp_domain::journal::MethodKind::IndependentAttempts(0);
    assert_eq!(
        consumer
            .record_method(&session, request(&view, proposed), None)
            .unwrap_err()
            .code,
        "method"
    );
    assert_eq!(adapter.writes.load(Ordering::SeqCst), 0);
}

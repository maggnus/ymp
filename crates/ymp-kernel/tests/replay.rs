use ymp_domain::{
    Digest, Id,
    journal::{Actor, Envelope, PolicySelection, decode},
};
use ymp_kernel::{
    events::Event,
    journal::{JournalRead, ParameterSchemas, validate_append as checked_append},
    view::SessionView,
};

fn opened() -> Envelope<Event> {
    Envelope {
        seq: 1,
        session: Id::new("session-a").unwrap(),
        at: 1000,
        actor: Actor::Runtime,
        policy: None,
        input: None,
        refs: vec![],
        payload: Event::SessionOpened {
            version: 1,
            selections: vec![
                PolicySelection::new(
                    "MethodRouter",
                    "FixedMethod",
                    "1",
                    serde_json::json!({"kind":"SoloWithVerifier","ladder":["StopPreserving"]}),
                )
                .unwrap(),
            ],
        },
    }
}

#[test]
fn replay_rejects_wrong_session_order_head_and_unknown_versions() {
    let event = opened();
    let empty = JournalRead {
        revision: 0,
        events: vec![],
    };
    assert!(validate_append(&empty, &event.session, 0, std::slice::from_ref(&event)).is_ok());
    let mut wrong = event.clone();
    wrong.seq = 2;
    assert_eq!(
        validate_append(&empty, &event.session, 0, &[wrong])
            .unwrap_err()
            .code,
        "event_order"
    );
    assert!(
        SessionView::replay(Id::new("different").unwrap(), std::slice::from_ref(&event)).is_err()
    );
    assert!(SessionView::replay(event.session.clone(), &[event.clone(), event.clone()]).is_err());
    let read = JournalRead {
        revision: 2,
        events: vec![event.clone()],
    };
    assert_eq!(
        read.view(&event.session, None).unwrap_err().code,
        "journal_head"
    );
    let mut wrong = event.clone();
    if let Event::SessionOpened { version, .. } = &mut wrong.payload {
        *version = 2;
    }
    assert_eq!(
        validate_append(&empty, &event.session, 0, &[wrong])
            .unwrap_err()
            .code,
        "event_version"
    );
    let read = JournalRead {
        revision: 1,
        events: vec![event.clone()],
    };
    assert!(read.view(&event.session, Some(2)).is_err());
    assert_eq!(read.view(&event.session, Some(0)).unwrap().revision(), 0);
}

#[test]
fn malformed_parameters_and_duplicate_policy_selection_are_denied() {
    let event = opened();
    let mut wrong = event.clone();
    if let Event::SessionOpened { selections, .. } = &mut wrong.payload {
        selections[0].parameters =
            serde_json::json!({"kind":"SoloWithVerifier","ladder":[],"invented":true});
        selections[0].policy.params = Digest::of_value(&selections[0].parameters).unwrap();
    }
    assert!(SessionView::replay(event.session.clone(), &[wrong]).is_err());
    let mut wrong = event.clone();
    if let Event::SessionOpened { selections, .. } = &mut wrong.payload {
        selections.push(selections[0].clone());
    }
    assert!(SessionView::replay(event.session.clone(), &[wrong]).is_err());
    let mut wire = serde_json::to_value(&event).unwrap();
    wire["payload"]["SessionOpened"]["unexpected"] = true.into();
    assert!(decode::<Envelope<Event>>(&serde_json::to_vec(&wire).unwrap()).is_err());
    wire["payload"] = serde_json::json!({"UnknownFamily":{"version":1}});
    assert!(decode::<Envelope<Event>>(&serde_json::to_vec(&wire).unwrap()).is_err());
}

#[test]
fn an_invalid_tail_cannot_return_a_partly_applied_batch() {
    let first = opened();
    let mut second = first.clone();
    second.seq = 2;
    let empty = JournalRead {
        revision: 0,
        events: vec![],
    };
    assert!(validate_append(&empty, &first.session, 0, &[first.clone(), second]).is_err());
    assert_eq!(
        empty,
        JournalRead {
            revision: 0,
            events: vec![]
        }
    );
    assert!(validate_append(&empty, &first.session, 0, &[]).is_err());
}

#[test]
fn version_one_opening_bytes_remain_readable() {
    // Frozen new-implementation bytes, independent of the current encoder.
    let bytes = br#"{"seq":1,"session":"session-v1","at":1000,"actor":"Runtime","policy":null,"input":null,"refs":[],"payload":{"SessionOpened":{"version":1,"selections":[{"policy":{"port":"MethodRouter","impl":"FixedMethod","version":"1","params":"631c7a19f65d87d61c526535657b24c74a2828889b564f10004535ec34e79c47"},"parameters":{"kind":"SoloWithVerifier","ladder":["StopPreserving"]}}]}}}"#;
    let event: Envelope<Event> = decode(bytes).unwrap();
    let view = SessionView::replay(Id::new("session-v1").unwrap(), &[event]).unwrap();
    assert!(view.opened());
    assert_eq!(view.revision(), 1);
    assert_eq!(
        view.policies()["MethodRouter"].policy.implementation,
        "FixedMethod"
    );
}

fn validate_append(
    current: &JournalRead,
    session: &Id,
    expected: u64,
    events: &[Envelope<Event>],
) -> ymp_domain::Result<SessionView> {
    checked_append(
        current,
        session,
        expected,
        events,
        &ParameterSchemas::default(),
    )
}

#[test]
fn unknown_implementation_versions_and_schema_replacement_are_rejected() {
    let event = opened();
    let mut schemas = ParameterSchemas::default();
    assert!(
        schemas
            .register("MethodRouter", "FixedMethod", "1", |_| Ok(()))
            .is_err()
    );
    for (implementation, version) in [("FixedMethod", "999"), ("Unknown", "1")] {
        let mut invalid = event.clone();
        if let Event::SessionOpened { selections, .. } = &mut invalid.payload {
            selections[0].policy.implementation = implementation.into();
            selections[0].policy.version = version.into();
        }
        assert_eq!(
            SessionView::replay(event.session.clone(), &[invalid])
                .unwrap_err()
                .code,
            "policy_schema"
        );
    }
}

#[test]
fn version_one_decision_bytes_keep_their_original_input_digest() {
    // Frozen independently encoded new-implementation history, including a decision.
    let bytes = br#"[{"actor":"Runtime","at":1000,"input":null,"payload":{"SessionOpened":{"selections":[{"parameters":{"kind":"SoloWithVerifier","ladder":["StopPreserving"]},"policy":{"impl":"FixedMethod","params":"631c7a19f65d87d61c526535657b24c74a2828889b564f10004535ec34e79c47","port":"MethodRouter","version":"1"}}],"version":1}},"policy":null,"refs":[],"seq":1,"session":"session-v1"},{"actor":"Runtime","at":1001,"input":"399b82940360c243edc81c1d7971143e6fda9704e4b9e5562099db9cf8c8f08b","payload":{"MethodChosen":{"decision":{"effective":{"parameters":{"kind":"SoloWithVerifier","ladder":["StopPreserving"]},"policy":{"impl":"FixedMethod","params":"631c7a19f65d87d61c526535657b24c74a2828889b564f10004535ec34e79c47","port":"MethodRouter","version":"1"}},"input":"399b82940360c243edc81c1d7971143e6fda9704e4b9e5562099db9cf8c8f08b","outcome":{"id":"method-v1","kind":"SoloWithVerifier","ladder":["StopPreserving"],"params":{},"policy":{"impl":"FixedMethod","params":"631c7a19f65d87d61c526535657b24c74a2828889b564f10004535ec34e79c47","port":"MethodRouter","version":"1"}},"proposal":{"basis":[{"id":"event:f6ed439032d6eab25c80c350085f92f058ed0af72e58cf853c664466098e0b35","version":"66e81b6231dcd5687a9eb0af0d9ae47d8a1b97f21f226730156a967765d4a744"}],"policy":{"impl":"FixedMethod","params":"631c7a19f65d87d61c526535657b24c74a2828889b564f10004535ec34e79c47","port":"MethodRouter","version":"1"},"rationale":"Keep the initial method bounded","value":{"id":"method-v1","kind":"SoloWithVerifier","ladder":["StopPreserving"],"params":{},"policy":{"impl":"FixedMethod","params":"631c7a19f65d87d61c526535657b24c74a2828889b564f10004535ec34e79c47","port":"MethodRouter","version":"1"}}},"selection_change":null},"version":1}},"policy":{"impl":"FixedMethod","params":"631c7a19f65d87d61c526535657b24c74a2828889b564f10004535ec34e79c47","port":"MethodRouter","version":"1"},"refs":[{"id":"event:f6ed439032d6eab25c80c350085f92f058ed0af72e58cf853c664466098e0b35","version":"66e81b6231dcd5687a9eb0af0d9ae47d8a1b97f21f226730156a967765d4a744"}],"seq":2,"session":"session-v1"}]"#;
    let events: Vec<Envelope<Event>> = decode(bytes).unwrap();
    let view = SessionView::replay(Id::new("session-v1").unwrap(), &events).unwrap();
    assert_eq!(view.revision(), 2);
    assert_eq!(
        view.decisions()[0].input.as_str(),
        "399b82940360c243edc81c1d7971143e6fda9704e4b9e5562099db9cf8c8f08b"
    );
    assert_eq!(view.method().unwrap().id.as_str(), "method-v1");
}

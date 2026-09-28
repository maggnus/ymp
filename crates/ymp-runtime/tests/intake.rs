use std::{collections::BTreeSet, sync::Arc};
use ymp_domain::{
    Digest, Id, Ref,
    journal::{Actor, Capability, Envelope, decode, encode},
};
use ymp_kernel::{events::Event, journal::Journal};
use ymp_runtime::{application::*, memory_journal::MemoryJournal};

fn selection() -> PolicySelection {
    PolicySelection::new(
        "MethodRouter",
        "FixedMethod",
        "1",
        serde_json::json!({"kind":"SoloWithVerifier","ladder":["StopPreserving"]}),
    )
    .unwrap()
}
fn constraints() -> Constraints {
    Constraints {
        budget: Real::new(100.5).unwrap(),
        verification_reserve: Real::new(20.0).unwrap(),
        deadline: Some(10_000),
        pins: Pins {
            team_size: Some(2),
            roster: Some(BTreeSet::from([
                Id::new("a").unwrap(),
                Id::new("b").unwrap(),
            ])),
            models: Some(BTreeSet::from(["fixture-model".into()])),
            efforts: Some(BTreeSet::from(["low".into()])),
        },
        allowed: BTreeSet::from([Capability::ReadFiles, Capability::RunProcess]),
        parallel_limit: 2,
        attempt_limit: 3,
        max_members: 2,
    }
}
fn criterion(id: &str, kind: CriterionKind) -> Criterion {
    Criterion {
        id: Id::new(id).unwrap(),
        text: format!("Explicit {id} requirement"),
        kind,
        weight: Real::new(1.0).unwrap(),
        required: true,
        origin: CriterionOrigin::User,
        needs_class: BTreeSet::from([EvidenceClass::Executed]),
    }
}
fn input() -> IntakeRequest {
    IntakeRequest {
        task: Task {
            id: Id::new("task-a").unwrap(),
            goal: Goal {
                request: "  Добавить проверку: keep original bytes.\n".into(),
                assumptions: vec![],
                clarifications: vec![],
            },
            contract: Id::new("contract-a").unwrap(),
            constraints: constraints(),
        },
        criteria: vec![
            criterion("new", CriterionKind::NewBehavior),
            criterion("preserve", CriterionKind::Preserve),
            criterion("artifact", CriterionKind::ArtifactPresence),
            criterion("quality", CriterionKind::Quality),
            criterion("constraint", CriterionKind::Constraint),
        ],
    }
}
fn clarification() -> IntakeNote {
    IntakeNote::Clarification(Clarification {
        question: "Which input needs validation?".into(),
        answer: "The import input".into(),
        at: 1001,
    })
}

#[test]
fn application_opens_exact_intake_and_replays_its_versions() {
    let journal = Arc::new(MemoryJournal::new());
    let app = Application::new(journal.clone());
    let session = Id::new("intake").unwrap();
    let mut request = input();
    request.task.constraints.budget = Real::new(98.31267718040647).unwrap();
    request.criteria[0].weight = Real::new(98.31267718040647).unwrap();
    let expected_task = request.task.clone();
    let expected_criteria = request.criteria.clone();
    app.open(session.clone(), 1000, request, vec![selection()])
        .unwrap();
    let view = app.view(&session, None).unwrap();
    assert_eq!(view.revision(), 2);
    assert_eq!(view.task(), Some(&expected_task));
    assert_eq!(view.criteria(), expected_criteria);
    assert_eq!(view.status(), Some(SessionStatus::Intake));
    let contract = view.contract().unwrap();
    assert_eq!(
        contract.version,
        AcceptanceContract::content_version(&expected_task, &expected_criteria, &[]).unwrap()
    );
    let bytes = encode(&journal.read(&session).unwrap().events).unwrap();
    let events: Vec<Envelope<Event>> = decode(&bytes).unwrap();
    assert_eq!(SessionView::replay(session, &events).unwrap(), view);
}

#[test]
fn refinements_keep_the_original_goal_and_prior_contract_with_attributed_versions() {
    let journal = Arc::new(MemoryJournal::new());
    let app = Application::new(journal.clone());
    let session = Id::new("refinement").unwrap();
    let owner = app
        .open(session.clone(), 1000, input(), vec![selection()])
        .unwrap();
    let initial = app.view(&session, None).unwrap();
    let mut criteria = initial.criteria().to_vec();
    criteria[0].text = "Reject an empty import input".into();
    let mut refined_constraints = constraints();
    refined_constraints.attempt_limit = 2;
    app.refine(
        &owner,
        IntakeRefinement {
            expected_revision: 2,
            at: 1002,
            constraints: refined_constraints.clone(),
            criteria: criteria.clone(),
            reason: "The owner clarified which input to validate".into(),
            note: clarification(),
        },
    )
    .unwrap();
    let second = app.view(&session, None).unwrap();
    assert_eq!(
        second.task().unwrap().goal.request,
        initial.task().unwrap().goal.request
    );
    assert_eq!(second.task().unwrap().goal.clarifications.len(), 1);
    assert_ne!(
        second.contract().unwrap().version,
        initial.contract().unwrap().version
    );
    assert_ne!(
        second.criteria()[0].reference().unwrap(),
        initial.criteria()[0].reference().unwrap()
    );
    second
        .resolve(&initial.contract().unwrap().reference())
        .unwrap();
    second
        .resolve(&initial.criteria()[0].reference().unwrap())
        .unwrap();
    assert_eq!(app.view(&session, Some(2)).unwrap(), initial);
    let read = journal.read(&session).unwrap();
    assert!(matches!(
        read.events[2].payload,
        Event::ClarificationRecorded { .. }
    ));
    match &read.events[3].payload {
        Event::CriteriaCommitted { data, .. } => {
            assert_eq!(data.previous, Some(initial.contract().unwrap().reference()));
            assert_eq!(data.reason, "The owner clarified which input to validate");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    criteria.push(criterion("new-scope", CriterionKind::Constraint));
    app.refine(
        &owner,
        IntakeRefinement {
            expected_revision: 4,
            at: 1003,
            constraints: refined_constraints,
            criteria,
            reason: "Record the scope assumption".into(),
            note: IntakeNote::Assumption(Assumption {
                text: "Only UTF-8 inputs are supported".into(),
                criterion: Some(Id::new("new-scope").unwrap()),
                reason: "No encoding requirement was supplied".into(),
            }),
        },
    )
    .unwrap();
    let third = app.view(&session, None).unwrap();
    assert_eq!(third.task().unwrap().goal.assumptions.len(), 1);
    assert_ne!(
        third.contract().unwrap().version,
        second.contract().unwrap().version
    );
    assert_eq!(app.view(&session, Some(4)).unwrap(), second);
}

#[test]
fn invalid_limits_pins_and_criteria_cannot_leave_partial_intake() {
    for case in 0..7 {
        let journal = Arc::new(MemoryJournal::new());
        let app = Application::new(journal.clone());
        let session = Id::new("invalid").unwrap();
        let mut request = input();
        match case {
            0 => request.task.constraints.parallel_limit = 0,
            1 => request.task.constraints.verification_reserve = Real::new(101.0).unwrap(),
            2 => request.task.constraints.pins.team_size = Some(1),
            3 => request.criteria.push(request.criteria[0].clone()),
            4 => request.criteria.clear(),
            5 => request.task.goal.assumptions.push(Assumption {
                text: "A guess".into(),
                criterion: Some(Id::new("missing").unwrap()),
                reason: "Missing information".into(),
            }),
            6 => request.criteria[0].weight = Real::new(-1.0).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            app.open(session.clone(), 1000, request, vec![selection()])
                .is_err(),
            "case {case}"
        );
        assert_eq!(journal.read(&session).unwrap().revision, 0);
        assert!(journal.read(&session).unwrap().events.is_empty());
    }
    assert!(Real::new(f64::NAN).is_err());
    assert!(Real::new(f64::INFINITY).is_err());
    assert!(decode::<Real>(b"1e999").is_err());
}

#[test]
fn stale_or_invalid_refinement_and_a_foreign_owner_preserve_the_whole_journal() {
    let journal = Arc::new(MemoryJournal::new());
    let app = Application::new(journal.clone());
    let session = Id::new("controlled").unwrap();
    let owner = app
        .open(session.clone(), 1000, input(), vec![selection()])
        .unwrap();
    let before = journal.read(&session).unwrap();
    let foreign_app = Application::new(Arc::new(MemoryJournal::new()));
    let foreign = foreign_app
        .open(session.clone(), 1000, input(), vec![selection()])
        .unwrap();
    for case in 0..4 {
        let mut refinement = IntakeRefinement {
            expected_revision: 2,
            at: 1002,
            constraints: constraints(),
            criteria: input().criteria,
            reason: "Refine explicit intake".into(),
            note: clarification(),
        };
        match case {
            0 => refinement.expected_revision = 1,
            1 => refinement.constraints.attempt_limit = 0,
            2 => refinement.reason.clear(),
            _ => {}
        }
        let control = if case == 3 { &foreign } else { &owner };
        assert!(app.refine(control, refinement).is_err());
        assert_eq!(journal.read(&session).unwrap(), before);
    }
    let orphan_note = Envelope {
        seq: 3,
        session: session.clone(),
        at: 1002,
        actor: Actor::Runtime,
        policy: None,
        input: None,
        refs: vec![
            app.view(&session, None)
                .unwrap()
                .contract()
                .unwrap()
                .reference(),
        ],
        payload: Event::ClarificationRecorded {
            version: 1,
            clarification: Clarification {
                question: "A?".into(),
                answer: "B".into(),
                at: 1002,
            },
        },
    };
    assert!(journal.append(&session, 2, &[orphan_note]).is_err());
    assert_eq!(journal.read(&session).unwrap(), before);
}

#[test]
fn derived_provenance_round_trips_but_user_intake_cannot_impersonate_a_planner() {
    let assignment = Ref {
        id: Id::new("assignment-a").unwrap(),
        version: Digest::of(b"assignment version"),
    };
    let mut request = input();
    request.criteria[0].origin = CriterionOrigin::Derived(assignment.clone());
    let restored: Criterion = decode(&encode(&request.criteria[0]).unwrap()).unwrap();
    assert_eq!(restored.origin, CriterionOrigin::Derived(assignment));
    let journal = Arc::new(MemoryJournal::new());
    let app = Application::new(journal.clone());
    let session = Id::new("provenance").unwrap();
    assert!(
        app.open(session.clone(), 1000, request, vec![selection()])
            .is_err()
    );
    assert_eq!(journal.read(&session).unwrap().revision, 0);
    for status in [
        SessionStatus::Intake,
        SessionStatus::Running,
        SessionStatus::Finalizing,
        SessionStatus::Delivered,
        SessionStatus::Blocked("insufficient evidence".into()),
        SessionStatus::Cancelled,
    ] {
        assert_eq!(
            decode::<SessionStatus>(&encode(&status).unwrap()).unwrap(),
            status
        );
    }
}

#[test]
fn a_method_cannot_be_inserted_between_an_intake_note_and_its_contract() {
    use std::collections::BTreeMap;
    use ymp_domain::{
        Proposal,
        journal::{Decision, EscalationStep, Method, MethodKind},
    };
    let source = Arc::new(MemoryJournal::new());
    let app = Application::new(source.clone());
    let session = Id::new("atomic-refinement").unwrap();
    let owner = app
        .open(session.clone(), 1000, input(), vec![selection()])
        .unwrap();
    app.refine(
        &owner,
        IntakeRefinement {
            expected_revision: 2,
            at: 1002,
            constraints: constraints(),
            criteria: input().criteria,
            reason: "Record the answer".into(),
            note: clarification(),
        },
    )
    .unwrap();
    let source = source.read(&session).unwrap();
    let target = MemoryJournal::new();
    target.append(&session, 0, &source.events[..2]).unwrap();
    let before = target.read(&session).unwrap();
    let mut refs = BTreeSet::new();
    for event in &source.events[..3] {
        refs.insert(event.reference().unwrap());
        if let Event::CriteriaCommitted { data, .. } = &event.payload {
            refs.insert(data.contract.reference());
            refs.insert(Ref {
                id: data.task.id.erased(),
                version: Digest::of_value(&data.task).unwrap(),
            });
            for criterion in &data.criteria {
                refs.insert(criterion.reference().unwrap());
            }
        }
    }
    let input_digest = Digest::of_value(&("SessionView", 1_u32, &session, 3_u64, &refs)).unwrap();
    let effective = selection();
    let method = Method {
        id: Id::new("interleaved-method").unwrap(),
        kind: MethodKind::SoloWithVerifier,
        ladder: vec![EscalationStep::StopPreserving],
        params: BTreeMap::new(),
        policy: effective.policy.clone(),
    };
    let proposed = Proposal {
        value: method.clone(),
        rationale: "A decision in a partial revision".into(),
        basis: vec![],
        policy: effective.policy.clone(),
    };
    let method_event = Envelope {
        seq: 4,
        session: session.clone(),
        at: 1002,
        actor: Actor::Runtime,
        policy: Some(effective.policy.clone()),
        input: Some(input_digest.clone()),
        refs: vec![],
        payload: Event::MethodChosen {
            version: 2,
            decision: Box::new(Decision {
                proposal: proposed,
                effective,
                input: input_digest,
                outcome: method,
                selection_change: None,
            }),
        },
    };
    let mut last = source.events[3].clone();
    last.seq = 5;
    assert!(
        target
            .append(&session, 2, &[source.events[2].clone(), method_event, last])
            .is_err()
    );
    assert_eq!(target.read(&session).unwrap(), before);
}

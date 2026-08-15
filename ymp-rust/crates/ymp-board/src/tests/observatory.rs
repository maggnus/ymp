//! What a reading distinguishes, and what it refuses to claim.
//!
//! The reading separates publication, delivery, citation, a decision's claimed basis, revision,
//! challenge, refresh, result ancestry and verifier verdict, and it draws every one of them from a
//! record rather than from an inference. None of them is causal. The one causal kind it has is
//! drawn only from a controlled intervention that changed a named message for a named receiver,
//! under a matched budget, over more than one replication — and a reading that labels anything else
//! causal is refused by the same check, whoever built it.

use crate::observatory::{
    Basis, Edge, EdgeKind, MIN_CAUSAL_REPLICATIONS, NodeRef, ViewDefect, ViewState,
};
use crate::protocol::{
    BoardCommand, NoteAncestry, NoteVerdict, ReadBoard, RecordIntervention, RefreshSalience,
};
use crate::records::{
    InterventionKind, InterventionRecord, MessageKind, ObservedVerdict, Reference, Relation,
};
use crate::tests::{
    ALPHA, BETA, CONTROLLER, GAMMA, READ_LIMIT, SALIENCE_MS, digest, publication, publish, scoped,
    task_audience,
};
use crate::{BoardLedger, Payload};

const CANDIDATE: &str = "candidate-under-review";
const PARENT: &str = "candidate-parent";

/// One episode that produces every kind of edge the reading distinguishes: a hypothesis, its
/// delivery, a challenge to it, its author's own revision, a decision that cites a result and names
/// the evidence it claims to have used, a refresh, and the ancestry and verdict the control plane
/// recorded for that result.
fn episode() -> BoardLedger {
    let mut board = scoped();
    let mut hypothesis = publication("m-hypothesis", ALPHA, task_audience(), 64);
    hypothesis.kind = MessageKind::Hypothesis;
    publish(&mut board, hypothesis);

    board
        .deliver(&ReadBoard {
            reader: BETA.to_owned(),
            limit_bytes: READ_LIMIT,
        })
        .expect("a read is answered");

    let mut challenge = publication("m-challenge", BETA, task_audience(), 48);
    challenge.kind = MessageKind::Challenge;
    challenge.relation = Relation::Challenges {
        message_id: "m-hypothesis".to_owned(),
    };
    publish(&mut board, challenge);

    let mut revision = publication("m-revision", ALPHA, task_audience(), 48);
    revision.relation = Relation::Revises {
        message_id: "m-hypothesis".to_owned(),
    };
    publish(&mut board, revision);

    let mut decision = publication("m-decision", ALPHA, task_audience(), 96);
    decision.kind = MessageKind::Decision;
    decision.claimed_decision_basis = vec!["m-hypothesis".to_owned()];
    decision.references = vec![Reference::Candidate {
        candidate_digest: digest(CANDIDATE),
    }];
    publish(&mut board, decision);

    board
        .execute(&BoardCommand::RefreshSalience(RefreshSalience {
            message_id: "m-hypothesis-again".to_owned(),
            refreshes: "m-hypothesis".to_owned(),
            author: ALPHA.to_owned(),
            salience_ms: SALIENCE_MS,
        }))
        .expect("salience is bought again");

    board
        .execute(&BoardCommand::NoteAncestry(NoteAncestry {
            controller: CONTROLLER.to_owned(),
            candidate_digest: digest(CANDIDATE),
            parents: vec![digest(PARENT)],
        }))
        .expect("ancestry is mirrored in");
    board
        .execute(&BoardCommand::NoteVerdict(NoteVerdict {
            controller: CONTROLLER.to_owned(),
            candidate_digest: digest(CANDIDATE),
            verdict: ObservedVerdict::Passed,
        }))
        .expect("a verdict is mirrored in");
    board
}

fn record_intervention(
    board: &mut BoardLedger,
    intervention_id: &str,
    replications: u32,
    matched_budget: bool,
) {
    board
        .execute(&BoardCommand::RecordIntervention(RecordIntervention {
            controller: CONTROLLER.to_owned(),
            intervention_id: intervention_id.to_owned(),
            kind: InterventionKind::Replacement,
            subject_message: "m-hypothesis".to_owned(),
            receiver: BETA.to_owned(),
            replications,
            matched_budget,
        }))
        .expect("an intervention is recorded");
}

/// The reading tells the nine recorded kinds apart.
#[test]
fn the_reading_distinguishes_every_kind_it_records() {
    let board = episode();
    let view = ViewState::for_operator(&board);
    for kind in [
        EdgeKind::Publication,
        EdgeKind::Delivery,
        EdgeKind::Citation,
        EdgeKind::ClaimedBasis,
        EdgeKind::Revision,
        EdgeKind::Challenge,
        EdgeKind::Refresh,
        EdgeKind::Ancestry,
        EdgeKind::Verdict,
    ] {
        assert!(
            !view.edges_of(kind).is_empty(),
            "the reading records nothing of kind {}, so it cannot be told from the others",
            kind.as_str()
        );
    }
    assert!(
        view.check().is_ok(),
        "a reading built from the records alone does not pass its own check"
    );
}

/// A reading built from the records labels nothing causal, however much order and citation it
/// holds.
#[test]
fn order_and_citation_alone_never_produce_a_causal_label() {
    let mut board = episode();
    assert!(
        ViewState::for_operator(&board)
            .edges_of(EdgeKind::Influence)
            .is_empty(),
        "a reading with no intervention behind it labelled something causal"
    );

    record_intervention(&mut board, "intervention-single", 1, true);
    record_intervention(&mut board, "intervention-unmatched", 8, false);
    let view = ViewState::for_operator(&board);
    assert!(
        view.edges_of(EdgeKind::Influence).is_empty(),
        "one episode, or a comparison whose replications did not run under one budget, produced a \
         causal label"
    );
    assert!(view.check().is_ok());
}

/// A controlled intervention under a matched budget, replicated, is the one thing that labels
/// influence.
#[test]
fn a_replicated_matched_budget_intervention_is_what_labels_influence() {
    let mut board = episode();
    record_intervention(&mut board, "intervention-1", MIN_CAUSAL_REPLICATIONS, true);
    let view = ViewState::for_operator(&board);
    let influence = view.edges_of(EdgeKind::Influence);
    assert_eq!(
        influence.len(),
        1,
        "a replicated matched-budget intervention did not produce exactly one causal edge"
    );
    assert_eq!(
        influence[0].basis,
        Basis::Intervention {
            intervention_id: "intervention-1".to_owned()
        },
        "the causal edge does not name the intervention it rests on"
    );
    assert!(
        view.check().is_ok(),
        "a causal edge drawn from a valid intervention was refused"
    );
}

fn view_with(edges: Vec<Edge>, interventions: Vec<InterventionRecord>) -> ViewState {
    ViewState {
        reader: None,
        messages: Vec::new(),
        edges,
        interventions,
        assessments: Vec::new(),
        verdicts: Vec::new(),
    }
}

fn message_node(message_id: &str) -> NodeRef {
    NodeRef::Message {
        message_id: message_id.to_owned(),
    }
}

fn participant_node(participant: &str) -> NodeRef {
    NodeRef::Participant {
        participant_id: participant.to_owned(),
    }
}

fn intervention(
    intervention_id: &str,
    replications: u32,
    matched_budget: bool,
) -> InterventionRecord {
    InterventionRecord {
        intervention_id: intervention_id.to_owned(),
        kind: InterventionKind::Removal,
        subject_message: "m-hypothesis".to_owned(),
        receiver: BETA.to_owned(),
        replications,
        matched_budget,
        recorded_at: 0,
    }
}

/// A reading that calls an association causal is refused, whoever built it.
///
/// Each fixture here is a view state constructed by hand rather than by the builder, because that
/// is the case the check exists for: a reading is data, and a surface, a later change or a hand
/// edit can produce one the records do not support. Every one of the six is a different way of
/// claiming a cause, and every one of them is refused.
#[test]
fn a_reading_that_labels_an_association_causal_fails_the_check() {
    let causal_edge = |basis: Basis| Edge {
        kind: EdgeKind::Influence,
        from: message_node("m-hypothesis"),
        to: participant_node(BETA),
        basis,
    };

    let cases: Vec<(&str, ViewState, ViewDefect)> = vec![
        (
            "a causal edge resting on a declaration",
            view_with(vec![causal_edge(Basis::Declared)], Vec::new()),
            ViewDefect::CausalWithoutIntervention {
                kind: "influence",
                from: message_node("m-hypothesis"),
                to: participant_node(BETA),
            },
        ),
        (
            "a citation dressed up as an intervention",
            view_with(
                vec![Edge {
                    kind: EdgeKind::Citation,
                    from: message_node("m-decision"),
                    to: message_node("m-hypothesis"),
                    basis: Basis::Intervention {
                        intervention_id: "intervention-1".to_owned(),
                    },
                }],
                vec![intervention(
                    "intervention-1",
                    MIN_CAUSAL_REPLICATIONS,
                    true,
                )],
            ),
            ViewDefect::AssociationLabelledCausal {
                kind: "citation",
                intervention_id: "intervention-1".to_owned(),
            },
        ),
        (
            "a causal edge naming an intervention nobody recorded",
            view_with(
                vec![causal_edge(Basis::Intervention {
                    intervention_id: "intervention-absent".to_owned(),
                })],
                Vec::new(),
            ),
            ViewDefect::InterventionUnknown {
                intervention_id: "intervention-absent".to_owned(),
            },
        ),
        (
            "a causal edge borrowing another episode's intervention",
            view_with(
                vec![Edge {
                    kind: EdgeKind::Influence,
                    from: message_node("m-decision"),
                    to: participant_node(BETA),
                    basis: Basis::Intervention {
                        intervention_id: "intervention-1".to_owned(),
                    },
                }],
                vec![intervention(
                    "intervention-1",
                    MIN_CAUSAL_REPLICATIONS,
                    true,
                )],
            ),
            ViewDefect::InterventionSubjectMismatch {
                intervention_id: "intervention-1".to_owned(),
                from: message_node("m-decision"),
                to: participant_node(BETA),
            },
        ),
        (
            "a causal edge resting on an unmatched budget",
            view_with(
                vec![causal_edge(Basis::Intervention {
                    intervention_id: "intervention-1".to_owned(),
                })],
                vec![intervention(
                    "intervention-1",
                    MIN_CAUSAL_REPLICATIONS,
                    false,
                )],
            ),
            ViewDefect::InterventionUnmatchedBudget {
                intervention_id: "intervention-1".to_owned(),
            },
        ),
        (
            "a causal edge resting on a single episode",
            view_with(
                vec![causal_edge(Basis::Intervention {
                    intervention_id: "intervention-1".to_owned(),
                })],
                vec![intervention("intervention-1", 1, true)],
            ),
            ViewDefect::InterventionUnreplicated {
                intervention_id: "intervention-1".to_owned(),
                replications: 1,
            },
        ),
    ];

    for (described, view, expected) in cases {
        assert_eq!(
            view.check(),
            Err(expected),
            "{described} was accepted as a presentable reading"
        );
    }
}

/// The provenance of one result is walkable from what was said to what a protected query returned,
/// and every step of the walk is a record.
#[test]
fn a_result_is_walkable_from_message_to_decision_to_artifact_to_verdict() {
    let board = episode();
    let view = ViewState::for_operator(&board);
    let candidate = NodeRef::Candidate {
        candidate_digest: digest(CANDIDATE),
    };

    let basis = view
        .edges_of(EdgeKind::ClaimedBasis)
        .into_iter()
        .find(|edge| edge.from == message_node("m-decision"))
        .expect("the decision states the evidence it claims to have used");
    assert_eq!(basis.to, message_node("m-hypothesis"));

    let citation = view
        .edges_of(EdgeKind::Citation)
        .into_iter()
        .find(|edge| edge.from == message_node("m-decision") && edge.to == candidate)
        .expect("the decision points at the result it is about");
    assert_eq!(citation.basis, Basis::Declared);

    let ancestry = view
        .edges_of(EdgeKind::Ancestry)
        .into_iter()
        .find(|edge| edge.from == candidate)
        .expect("the result carries its parent forward");
    assert_eq!(
        ancestry.to,
        NodeRef::Candidate {
            candidate_digest: digest(PARENT)
        }
    );

    let verdict = view
        .edges_of(EdgeKind::Verdict)
        .into_iter()
        .find(|edge| edge.to == candidate)
        .expect("a protected query returned a verdict on the result");
    assert_eq!(verdict.basis, Basis::Declared);
    assert_eq!(
        view.verdicts,
        vec![(digest(CANDIDATE), ObservedVerdict::Passed)]
    );
}

/// A reading built for a participant carries what that participant may read and nothing else, so a
/// surface cannot show through the observatory what a read would refuse.
#[test]
fn a_reading_for_a_participant_carries_only_what_it_may_read() {
    let board = episode();
    let stranger = ViewState::for_reader(&board, GAMMA);
    assert!(
        stranger.messages.is_empty(),
        "a reading built for a stranger to every detailed scope carried its messages"
    );
    assert!(
        stranger
            .edges
            .iter()
            .all(|edge| edge.kind == EdgeKind::Ancestry || edge.kind == EdgeKind::Verdict),
        "a reading built for a stranger carried edges about a conversation it may not read"
    );

    let member = ViewState::for_reader(&board, BETA);
    assert!(
        member
            .messages
            .iter()
            .any(|message| message.message_id == "m-hypothesis"),
        "a reading built for a member of the scope is missing its messages"
    );
    assert!(
        member.messages.iter().all(|message| message.untrusted),
        "the reading presents a payload as something other than an untrusted claim"
    );
}

/// The payload never reaches the reading either: what a message view carries is the identity and
/// the length of the bytes.
#[test]
fn the_reading_shows_the_identity_of_a_payload_and_not_the_payload() {
    let mut board = scoped();
    let payload = Payload::of(b"an unusually quotable finding");
    let mut message = publication("m-1", ALPHA, task_audience(), payload.bytes());
    message.payload = payload.clone();
    publish(&mut board, message);

    let view = ViewState::for_operator(&board);
    let rendered = serde_json::to_string(&view).expect("a reading is representable");
    assert!(
        rendered.contains(payload.digest()),
        "the reading does not identify the payload it is about"
    );
    assert!(
        !rendered.contains("an unusually quotable finding"),
        "the reading carries the bytes of a payload"
    );
}

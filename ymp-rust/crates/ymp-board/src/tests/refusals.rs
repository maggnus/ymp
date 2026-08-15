//! What a refused command leaves behind, which is exactly what was there before it.
//!
//! Twenty-two malformed or cross-scope commands are put to the same board, one at a time, and
//! each one is required to be refused with the reason named and to leave the board byte for byte
//! as it stood. The comparison is over the whole serialized board rather than over the records the
//! command would have touched, so a refusal that wrote something somewhere else is caught too.
//!
//! Control and verifier state are unchanged by construction rather than by assertion: this crate
//! depends on no package that holds either, so there is no value of either kind in scope to change.
//! That absence is checked in `containment`.

use crate::protocol::{
    AdvanceClock, BoardCommand, BoardError, GrantAudience, NoteVerdict, ReadBoard, RefreshSalience,
};
use crate::records::{Audience, MessageKind, ObservedVerdict, Reference, Relation, Rights};
use crate::tests::{
    ALPHA, BETA, DELTA, GAMMA, GRANT_EXPIRY, OTHER_SCOPE, TASK_SCOPE, digest, publication, publish,
    scoped, task_audience,
};
use crate::{
    BoardLedger, MAX_IDENTIFIER_CHARS, MAX_PAYLOAD_BYTES, MAX_REFERENCES, MAX_SALIENCE_MS, Payload,
};

/// The board every refusal is put to: one detailed finding in the task scope, and one bounded
/// notice in project discovery.
fn stated() -> BoardLedger {
    let mut board = scoped();
    publish(&mut board, publication("m-1", ALPHA, task_audience(), 64));
    publish(
        &mut board,
        publication("m-notice", ALPHA, Audience::ProjectDiscovery, 32),
    );
    board
}

fn other_audience() -> Audience {
    Audience::Scope {
        scope_id: OTHER_SCOPE.to_owned(),
    }
}

/// Every one of these is refused, and none of them writes anything.
#[test]
fn a_malformed_or_cross_scope_command_is_refused_and_writes_nothing() {
    let non_canonical = {
        let mut command = publication("m-2", ALPHA, task_audience(), 8);
        command.payload = Payload::stated("not-a-digest", 8);
        command
    };
    let overlong = publication(
        &"m".repeat(MAX_IDENTIFIER_CHARS + 1),
        ALPHA,
        task_audience(),
        8,
    );
    let oversized = publication("m-2", ALPHA, task_audience(), MAX_PAYLOAD_BYTES + 1);
    let over_referenced = {
        let mut command = publication("m-2", ALPHA, task_audience(), 8);
        command.references = (0..=MAX_REFERENCES)
            .map(|index| Reference::Artifact {
                object_digest: digest(&format!("artifact-{index}")),
            })
            .collect();
        command
    };
    let unbounded_salience = {
        let mut command = publication("m-2", ALPHA, task_audience(), 8);
        command.salience_ms = MAX_SALIENCE_MS + 1;
        command
    };
    let stranger = publication("m-2", GAMMA, task_audience(), 8);
    let listener = publication("m-2", DELTA, task_audience(), 8);
    let to_a_stranger = publication(
        "m-2",
        ALPHA,
        Audience::Named {
            scope_id: TASK_SCOPE.to_owned(),
            recipients: vec![GAMMA.to_owned()],
        },
        8,
    );
    let cross_citation = {
        let mut command = publication("m-2", GAMMA, other_audience(), 8);
        command.references = vec![Reference::Message {
            message_id: "m-1".to_owned(),
        }];
        command
    };
    let challenge_without_target = {
        let mut command = publication("m-2", BETA, task_audience(), 8);
        command.kind = MessageKind::Challenge;
        command
    };
    let target_without_challenge = {
        let mut command = publication("m-2", BETA, task_audience(), 8);
        command.relation = Relation::Challenges {
            message_id: "m-1".to_owned(),
        };
        command
    };
    let revision_of_another = {
        let mut command = publication("m-2", BETA, task_audience(), 8);
        command.relation = Relation::Revises {
            message_id: "m-1".to_owned(),
        };
        command
    };
    let relation_across_audiences = {
        let mut command = publication("m-2", ALPHA, task_audience(), 8);
        command.relation = Relation::ReplyTo {
            message_id: "m-notice".to_owned(),
        };
        command
    };
    let basis_without_a_decision = {
        let mut command = publication("m-2", ALPHA, task_audience(), 8);
        command.claimed_decision_basis = vec!["m-1".to_owned()];
        command
    };
    let refresh_as_publication = {
        let mut command = publication("m-2", ALPHA, task_audience(), 8);
        command.relation = Relation::Refreshes {
            message_id: "m-1".to_owned(),
        };
        command
    };
    let unknown_scope = publication(
        "m-2",
        ALPHA,
        Audience::Scope {
            scope_id: "scope-that-was-never-opened".to_owned(),
        },
        8,
    );
    let duplicate = publication("m-1", ALPHA, task_audience(), 8);

    let cases: Vec<(&str, BoardCommand, BoardError)> = vec![
        (
            "a payload identified by something that is not a digest",
            BoardCommand::Publish(non_canonical),
            BoardError::InvalidDigest {
                kind: "payload_digest",
            },
        ),
        (
            "an identifier past the bound a record carries",
            BoardCommand::Publish(overlong),
            BoardError::InvalidIdentifier { kind: "message_id" },
        ),
        (
            "a payload past what a detailed audience admits",
            BoardCommand::Publish(oversized),
            BoardError::PayloadTooLarge {
                bytes: MAX_PAYLOAD_BYTES + 1,
                limit: MAX_PAYLOAD_BYTES,
            },
        ),
        (
            "more references than a message carries",
            BoardCommand::Publish(over_referenced),
            BoardError::TooManyEntries {
                kind: "references",
                limit: MAX_REFERENCES,
            },
        ),
        (
            "salience past what one publication buys",
            BoardCommand::Publish(unbounded_salience),
            BoardError::InvalidSalience,
        ),
        (
            "a stranger appending to a scope",
            BoardCommand::Publish(stranger),
            BoardError::NotAdmitted {
                participant: GAMMA.to_owned(),
                scope_id: TASK_SCOPE.to_owned(),
            },
        ),
        (
            "a listener appending to a scope it may only read",
            BoardCommand::Publish(listener),
            BoardError::PublishNotPermitted {
                participant: DELTA.to_owned(),
                scope_id: TASK_SCOPE.to_owned(),
            },
        ),
        (
            "a message naming a recipient that is not admitted",
            BoardCommand::Publish(to_a_stranger),
            BoardError::RecipientNotAdmitted {
                participant: GAMMA.to_owned(),
                scope_id: TASK_SCOPE.to_owned(),
            },
        ),
        (
            "a citation of a message in another scope",
            BoardCommand::Publish(cross_citation),
            BoardError::ReferenceNotAdmitted {
                message_id: "m-1".to_owned(),
            },
        ),
        (
            "a challenge that names nothing",
            BoardCommand::Publish(challenge_without_target),
            BoardError::ChallengeRelationMismatch,
        ),
        (
            "a message that challenges without being one",
            BoardCommand::Publish(target_without_challenge),
            BoardError::ChallengeRelationMismatch,
        ),
        (
            "a revision of somebody else's position",
            BoardCommand::Publish(revision_of_another),
            BoardError::RevisionOfAnother {
                message_id: "m-1".to_owned(),
                author: ALPHA.to_owned(),
            },
        ),
        (
            "a reply across two audiences",
            BoardCommand::Publish(relation_across_audiences),
            BoardError::RelationOutsideAudience {
                message_id: "m-notice".to_owned(),
            },
        ),
        (
            "claimed evidence on something that is not a decision",
            BoardCommand::Publish(basis_without_a_decision),
            BoardError::DecisionBasisOnOtherKind,
        ),
        (
            "a refresh published as an ordinary message",
            BoardCommand::Publish(refresh_as_publication),
            BoardError::RefreshIsNotPublication,
        ),
        (
            "an audience nobody opened",
            BoardCommand::Publish(unknown_scope),
            BoardError::Unknown {
                kind: "scope",
                id: "scope-that-was-never-opened".to_owned(),
            },
        ),
        (
            "a message identifier that is already recorded",
            BoardCommand::Publish(duplicate),
            BoardError::DuplicateIdentifier {
                kind: "message",
                id: "m-1".to_owned(),
            },
        ),
        (
            "a participant mirroring in a verdict the controller did not state",
            BoardCommand::NoteVerdict(NoteVerdict {
                controller: ALPHA.to_owned(),
                candidate_digest: digest("candidate"),
                verdict: ObservedVerdict::Passed,
            }),
            BoardError::NotAuthorized {
                participant: ALPHA.to_owned(),
            },
        ),
        (
            "a clock moving backwards",
            BoardCommand::AdvanceClock(AdvanceClock { to: 0 }),
            BoardError::ClockRegression { now: 1, to: 0 },
        ),
        (
            "a grant that expires before it is issued",
            BoardCommand::GrantAudience(GrantAudience {
                grant_id: "grant-late".to_owned(),
                scope_id: TASK_SCOPE.to_owned(),
                sponsor: crate::tests::ROOT.to_owned(),
                participant: GAMMA.to_owned(),
                rights: Rights::READ,
                expires_at: 0,
            }),
            BoardError::InvalidGrantWindow,
        ),
        (
            "a refresh of a message in a scope the author is a stranger to",
            BoardCommand::RefreshSalience(RefreshSalience {
                message_id: "m-2".to_owned(),
                refreshes: "m-1".to_owned(),
                author: GAMMA.to_owned(),
                salience_ms: 1_000,
            }),
            BoardError::ReferenceNotAdmitted {
                message_id: "m-1".to_owned(),
            },
        ),
        (
            "a read bounded by nothing",
            BoardCommand::ReadBoard(ReadBoard {
                reader: BETA.to_owned(),
                limit_bytes: 0,
            }),
            BoardError::InvalidDeliveryLimit,
        ),
    ];

    for (described, command, expected) in cases {
        let mut board = stated();
        // The clock is moved off zero once, so that the regression case has somewhere to fall back
        // from and every other case is put to the same board.
        board
            .execute(&BoardCommand::AdvanceClock(AdvanceClock { to: 1 }))
            .expect("the clock moves");
        let before = board.snapshot();
        let facts = board.facts().len();

        assert_eq!(
            board.decide(&command),
            Err(expected.clone()),
            "{described} was not refused with the reason it should be"
        );
        assert_eq!(
            board.execute(&command),
            Err(expected),
            "{described} was refused when decided and accepted when executed"
        );
        assert_eq!(
            board.snapshot(),
            before,
            "{described} changed the board despite being refused"
        );
        assert_eq!(
            board.facts().len(),
            facts,
            "{described} committed a fact despite being refused"
        );
    }
}

/// A refusal leaves no trace in the projections a surface reads either, not only in the records
/// underneath them.
#[test]
fn a_refused_command_changes_no_projection() {
    let mut board = stated();
    let projection: Vec<String> = board
        .active_projection(BETA)
        .into_iter()
        .map(|message| message.message_id.clone())
        .collect();
    let cursor = board.reader(BETA).map(|reader| reader.cursor);

    assert!(
        board
            .execute(&BoardCommand::Publish(publication(
                "m-2",
                GAMMA,
                task_audience(),
                8
            )))
            .is_err()
    );
    assert_eq!(
        board
            .active_projection(BETA)
            .into_iter()
            .map(|message| message.message_id.clone())
            .collect::<Vec<String>>(),
        projection,
        "a refused message appeared in what a participant is projected"
    );
    assert_eq!(
        board.reader(BETA).map(|reader| reader.cursor),
        cursor,
        "a refused command moved a delivery cursor"
    );
    assert!(
        board.conserves_allowance(),
        "a refused command lost track of communication capacity"
    );
}

/// A board rebuilt from the facts it committed is the board that committed them. Recovery reads the
/// records and nothing beside them, so a reader that lost every notification is a reader that is
/// behind rather than one that is wrong.
#[test]
fn a_board_rebuilt_from_its_own_facts_is_the_same_board() {
    let mut board = stated();
    board
        .deliver(&ReadBoard {
            reader: BETA.to_owned(),
            limit_bytes: crate::tests::READ_LIMIT,
        })
        .expect("a read is answered");
    board
        .execute(&BoardCommand::AdvanceClock(AdvanceClock {
            to: GRANT_EXPIRY,
        }))
        .expect("the clock moves");

    let mut rebuilt = BoardLedger::new(
        crate::tests::CONTROLLER,
        crate::tests::ROOT,
        crate::tests::root_total(),
    )
    .expect("a board opens");
    for fact in board.facts() {
        rebuilt.replay(fact).expect("a committed fact replays");
    }
    assert_eq!(
        rebuilt.snapshot(),
        board.snapshot(),
        "a board rebuilt from its own facts is not the board that committed them"
    );
    assert_eq!(
        rebuilt.reader(BETA).map(|reader| reader.cursor),
        board.reader(BETA).map(|reader| reader.cursor),
        "a board rebuilt from its own facts holds a different delivery cursor"
    );
}

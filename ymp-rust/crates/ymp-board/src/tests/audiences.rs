//! Who a message reaches.
//!
//! A detailed message reaches a participant that holds an explicit grant on its scope, that has not
//! expired, and nobody else. A project-discovery notice reaches every registered participant and is
//! bounded so that it announces rather than discloses. Each claim is stated with the check that
//! carries it switched off beside it, so that what the claim rests on is visible rather than
//! assumed.

use crate::ledger::DisabledChecks;
use crate::protocol::{BoardCommand, BoardError, ReadBoard};
use crate::records::{Audience, MessageKind, Reference, Relation, Rights};
use crate::tests::{
    ALPHA, BETA, DELTA, GAMMA, GRANT_EXPIRY, OTHER_SCOPE, READ_LIMIT, SALIENCE_MS, TASK_SCOPE,
    digest, publication, publish, scoped, task_audience,
};
use crate::{MAX_DISCOVERY_PAYLOAD_BYTES, MAX_DISCOVERY_REFERENCES, Payload};

fn read(board: &mut crate::BoardLedger, reader: &str) -> Vec<String> {
    board
        .deliver(&ReadBoard {
            reader: reader.to_owned(),
            limit_bytes: READ_LIMIT,
        })
        .expect("a read is answered")
        .messages
        .into_iter()
        .map(|message| message.message_id)
        .collect()
}

/// A detailed message is delivered to the participants admitted to its scope and to no one else.
///
/// The negative half switches off the admission check and reads with the same stranger. Without it
/// the same board hands a task finding to a participant that was never admitted to anything, which
/// is what the check is for.
#[test]
fn a_detailed_message_reaches_the_admitted_and_no_one_else() {
    let mut board = scoped();
    publish(
        &mut board,
        publication("m-task", ALPHA, task_audience(), 64),
    );

    assert_eq!(
        read(&mut board, BETA),
        vec!["m-task".to_owned()],
        "a participant holding a live grant on the scope was not delivered the message published \
         to it"
    );
    assert!(
        read(&mut board, GAMMA).is_empty(),
        "a participant holding a grant on another scope was delivered a detailed message from a \
         scope it was never admitted to"
    );

    let message = board.message("m-task").expect("the record stands");
    assert!(
        !board.may_read(GAMMA, message),
        "a stranger to the scope may read its detailed message"
    );

    let mut weakened = scoped();
    weakened.disable_checks(DisabledChecks {
        admission: true,
        ..DisabledChecks::default()
    });
    publish(
        &mut weakened,
        publication("m-task", ALPHA, task_audience(), 64),
    );
    assert_eq!(
        read(&mut weakened, GAMMA),
        vec!["m-task".to_owned()],
        "with the admission check switched off a stranger was still refused, so the check under \
         test is not what refuses one"
    );
}

/// A grant admits until its expiry and not one moment past it.
///
/// The negative half switches off the expiry check. Without it the same grant, at the same clock,
/// keeps admitting, so what an expiry means here is exactly this comparison.
#[test]
fn an_expired_grant_admits_nothing() {
    // Short enough that the grant expires while the message is still salient, so what stops the
    // delivery is the expiry and not the projection.
    let short = SALIENCE_MS / 2;
    let mut board = scoped();
    crate::tests::grant(&mut board, TASK_SCOPE, GAMMA, Rights::READ, short);
    publish(&mut board, publication("m-1", ALPHA, task_audience(), 64));

    board
        .execute(&BoardCommand::AdvanceClock(crate::AdvanceClock {
            to: short,
        }))
        .expect("the clock moves");
    assert!(
        read(&mut board, GAMMA).is_empty(),
        "a grant whose expiry has arrived still delivered a detailed message"
    );
    assert_eq!(
        read(&mut board, BETA),
        vec!["m-1".to_owned()],
        "a grant that is still live at the same clock stopped delivering, so what refused the \
         other one is not its expiry"
    );
    assert!(
        matches!(
            board.grant(&format!("late-{}", crate::tests::grant_id(TASK_SCOPE, GAMMA))),
            Some(grant) if !grant.is_live_at(board.now())
        ),
        "the expired grant does not read as expired"
    );

    let mut weakened = scoped();
    weakened.disable_checks(DisabledChecks {
        grant_expiry: true,
        ..DisabledChecks::default()
    });
    crate::tests::grant(&mut weakened, TASK_SCOPE, GAMMA, Rights::READ, short);
    publish(
        &mut weakened,
        publication("m-1", ALPHA, task_audience(), 64),
    );
    weakened
        .execute(&BoardCommand::AdvanceClock(crate::AdvanceClock {
            to: short,
        }))
        .expect("the clock moves");
    assert_eq!(
        read(&mut weakened, GAMMA),
        vec!["m-1".to_owned()],
        "with the expiry check switched off an expired grant still admitted nothing, so the check \
         under test is not what stops one"
    );
}

/// Project discovery announces that work or help exists. It is readable without any grant, and it
/// is bounded in payload and in references so that it cannot become a second, global detailed
/// audience.
///
/// The negative half switches off the bound. Without it the same notice carries a full finding to
/// every participant on the host, which is the globally readable board the design does not have.
#[test]
fn project_discovery_carries_a_bounded_summary_and_references_only() {
    let mut board = scoped();
    let mut notice = publication(
        "m-notice",
        ALPHA,
        Audience::ProjectDiscovery,
        MAX_DISCOVERY_PAYLOAD_BYTES,
    );
    notice.kind = MessageKind::HelpRequest;
    notice.references = vec![Reference::Candidate {
        candidate_digest: digest("candidate"),
    }];
    publish(&mut board, notice);

    assert_eq!(
        read(&mut board, GAMMA),
        vec!["m-notice".to_owned()],
        "a bounded discovery notice was not readable by a participant holding no grant"
    );

    let oversized = publication(
        "m-oversized",
        ALPHA,
        Audience::ProjectDiscovery,
        MAX_DISCOVERY_PAYLOAD_BYTES + 1,
    );
    assert_eq!(
        board.decide(&BoardCommand::Publish(oversized.clone())),
        Err(BoardError::PayloadTooLarge {
            bytes: MAX_DISCOVERY_PAYLOAD_BYTES + 1,
            limit: MAX_DISCOVERY_PAYLOAD_BYTES,
        }),
        "a discovery notice larger than the announcement bound was accepted"
    );

    let mut over_referenced = publication("m-refs", ALPHA, Audience::ProjectDiscovery, 16);
    over_referenced.references = (0..=MAX_DISCOVERY_REFERENCES)
        .map(|index| Reference::Artifact {
            object_digest: digest(&format!("artifact-{index}")),
        })
        .collect();
    assert_eq!(
        board.decide(&BoardCommand::Publish(over_referenced)),
        Err(BoardError::TooManyEntries {
            kind: "references",
            limit: MAX_DISCOVERY_REFERENCES,
        }),
        "a discovery notice carrying more references than the bound was accepted"
    );

    let mut weakened = scoped();
    weakened.disable_checks(DisabledChecks {
        discovery_bound: true,
        ..DisabledChecks::default()
    });
    assert!(
        weakened.decide(&BoardCommand::Publish(oversized)).is_ok(),
        "with the discovery bound switched off an oversized notice was still refused, so the bound \
         under test is not what refuses one"
    );
}

/// Admission to one scope says nothing about another. A stranger is neither delivered the other
/// scope's messages nor able to cite one, because citing a message it may not read would state
/// that it read it.
#[test]
fn a_grant_on_one_scope_exposes_no_other() {
    let mut board = scoped();
    publish(
        &mut board,
        publication("m-task", ALPHA, task_audience(), 64),
    );

    let mut citation = publication(
        "m-cross",
        GAMMA,
        Audience::Scope {
            scope_id: OTHER_SCOPE.to_owned(),
        },
        32,
    );
    citation.references = vec![Reference::Message {
        message_id: "m-task".to_owned(),
    }];
    assert_eq!(
        board.decide(&BoardCommand::Publish(citation)),
        Err(BoardError::ReferenceNotAdmitted {
            message_id: "m-task".to_owned()
        }),
        "a participant cited a message from a scope it holds no grant on"
    );

    let mut reply = publication("m-reply", GAMMA, task_audience(), 32);
    reply.relation = Relation::ReplyTo {
        message_id: "m-task".to_owned(),
    };
    assert_eq!(
        board.decide(&BoardCommand::Publish(reply)),
        Err(BoardError::NotAdmitted {
            participant: GAMMA.to_owned(),
            scope_id: TASK_SCOPE.to_owned(),
        }),
        "a participant appended to a scope it holds no grant on"
    );
}

/// A named audience narrows a scope and never widens one: its recipients must already be admitted,
/// and a member of the scope who is not named is not delivered it.
#[test]
fn a_named_audience_narrows_and_never_widens() {
    let mut board = scoped();
    let named = publication(
        "m-named",
        ALPHA,
        Audience::Named {
            scope_id: TASK_SCOPE.to_owned(),
            recipients: vec![BETA.to_owned()],
        },
        48,
    );
    publish(&mut board, named);

    assert_eq!(
        read(&mut board, BETA),
        vec!["m-named".to_owned()],
        "a named recipient was not delivered the message addressed to it"
    );
    assert!(
        read(&mut board, DELTA).is_empty(),
        "a scope member who was not named was delivered a message addressed to someone else"
    );

    let to_a_stranger = publication(
        "m-stranger",
        ALPHA,
        Audience::Named {
            scope_id: TASK_SCOPE.to_owned(),
            recipients: vec![GAMMA.to_owned()],
        },
        48,
    );
    assert_eq!(
        board.decide(&BoardCommand::Publish(to_a_stranger)),
        Err(BoardError::RecipientNotAdmitted {
            participant: GAMMA.to_owned(),
            scope_id: TASK_SCOPE.to_owned(),
        }),
        "a message named a recipient that holds no grant on the scope, which would have admitted \
         it by addressing it"
    );
}

/// Reading and publishing are separate rights. A participant admitted to listen is refused when it
/// speaks, and the refusal names the scope rather than pretending the participant is a stranger.
#[test]
fn reading_a_scope_is_not_publishing_to_it() {
    let board = scoped();
    assert_eq!(
        board.decide(&BoardCommand::Publish(publication(
            "m-from-reader",
            DELTA,
            task_audience(),
            16
        ))),
        Err(BoardError::PublishNotPermitted {
            participant: DELTA.to_owned(),
            scope_id: TASK_SCOPE.to_owned(),
        }),
        "a participant admitted to read a scope was allowed to append to it"
    );
}

/// A grant issued after a message was published still exposes it, because admission is answered
/// against the grant that is live when the read happens and the record it answers about is
/// permanent.
#[test]
fn a_later_grant_exposes_what_was_published_before_it() {
    let mut board = scoped();
    publish(
        &mut board,
        publication("m-task", ALPHA, task_audience(), 64),
    );
    crate::tests::grant(
        &mut board,
        TASK_SCOPE,
        GAMMA,
        Rights::READ,
        GRANT_EXPIRY / 2,
    );
    assert_eq!(
        read(&mut board, GAMMA),
        vec!["m-task".to_owned()],
        "a participant admitted after the message was published was not delivered it"
    );
}

/// A payload's identity travels; its bytes do not. What a delivery carries is the digest and the
/// length, so resolving the content is the reader's own act against the content-addressed store.
#[test]
fn a_delivery_carries_the_identity_of_a_payload_and_not_the_payload() {
    let mut board = scoped();
    let payload = Payload::of(b"a bounded finding");
    let mut message = publication("m-task", ALPHA, task_audience(), payload.bytes());
    message.payload = payload.clone();
    publish(&mut board, message);

    let delivered = board
        .deliver(&ReadBoard {
            reader: BETA.to_owned(),
            limit_bytes: READ_LIMIT,
        })
        .expect("a read is answered");
    let carried = delivered.messages.first().expect("one message");
    assert_eq!(carried.payload_digest, payload.digest());
    assert_eq!(carried.payload_bytes, payload.bytes());
    assert!(
        !board.snapshot().contains("a bounded finding"),
        "the board holds the bytes of a payload, and the plane is supposed to hold only its \
         identity and its length"
    );
}

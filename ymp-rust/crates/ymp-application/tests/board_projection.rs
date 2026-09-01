//! Acceptance: the application projects the persisted collaboration board for an operator
//! without handing the caller its ledger or store.

use tempfile::tempdir;
use ymp_application::{Application, ResolvedBoardProjection};
use ymp_board::budget::CommunicationAllowance;
use ymp_board::{
    Audience, Basis, BoardCommand, EdgeKind, InitialMember, MessageKind, NodeRef, OpenScope,
    Payload, Publish, Reference, RegisterParticipant, Relation, Rights, ScopeKind,
};
use ymp_domain::Budget;

const ROOT: &str = "participant-root";
const SCOPE: &str = "task-audience";

fn prepare_detailed_audience(application: &mut Application) {
    for participant in ["recipient-a", "recipient-b"] {
        application
            .record_board(&BoardCommand::RegisterParticipant(RegisterParticipant {
                participant_id: participant.to_owned(),
                sponsor: ROOT.to_owned(),
                endowment: CommunicationAllowance::ZERO,
            }))
            .expect("register named recipient");
    }
    application
        .record_board(&BoardCommand::OpenScope(OpenScope {
            controller: "ymp".to_owned(),
            scope_id: SCOPE.to_owned(),
            kind: ScopeKind::Task,
            sponsor: ROOT.to_owned(),
            review_policy: None,
            initial_members: vec![
                InitialMember {
                    grant_id: "grant-root".to_owned(),
                    participant: ROOT.to_owned(),
                    rights: Rights::READ_AND_PUBLISH,
                },
                InitialMember {
                    grant_id: "grant-recipient-a".to_owned(),
                    participant: "recipient-a".to_owned(),
                    rights: Rights::READ,
                },
                InitialMember {
                    grant_id: "grant-recipient-b".to_owned(),
                    participant: "recipient-b".to_owned(),
                    rights: Rights::READ,
                },
            ],
            member_expires_at: 10_000,
        }))
        .expect("open detailed audience");
}

fn publication(message_id: &str, audience: Audience, kind: MessageKind, payload: &[u8]) -> Publish {
    Publish {
        message_id: message_id.to_owned(),
        author: ROOT.to_owned(),
        audience,
        kind,
        payload: Payload::of(payload),
        salience_ms: 5_000,
        references: Vec::new(),
        relation: Relation::Standalone,
        claimed_decision_basis: Vec::new(),
    }
}

#[test]
fn operator_projection_is_attributed_untrusted_and_read_only() {
    let temporary = tempdir().expect("temporary directory");
    let mut application =
        Application::create(temporary.path(), "run-1", Budget::new(1, 1)).expect("create run");
    let artifact_digest = "a".repeat(64);
    let payload = b"untrusted observation";
    application
        .publish_board(
            &Publish {
                message_id: "message-1".to_owned(),
                author: "participant-root".to_owned(),
                audience: Audience::ProjectDiscovery,
                kind: MessageKind::Observation,
                payload: Payload::of(payload),
                salience_ms: 5_000,
                references: vec![Reference::Artifact {
                    object_digest: artifact_digest.clone(),
                }],
                relation: Relation::Standalone,
                claimed_decision_basis: Vec::new(),
            },
            payload,
        )
        .expect("record representative board message");

    let position = application.board_observation();
    assert_eq!(position.audit_messages, 1);
    assert_eq!(position.recorded_facts, position.committed_facts);
    let mut projection: ResolvedBoardProjection = application
        .operator_board_projection()
        .expect("resolve operator projection");

    projection
        .check()
        .expect("operator projection is presentable");
    assert_eq!(projection.reader, None);
    assert_eq!(projection.messages.len(), 1);
    assert_eq!(projection.messages[0].message.author, "participant-root");
    assert!(projection.messages[0].message.untrusted);
    assert_eq!(projection.messages[0].payload, payload);
    assert!(projection.edges.iter().any(|edge| {
        edge.kind == EdgeKind::Publication
            && edge.from
                == (NodeRef::Participant {
                    participant_id: "participant-root".to_owned(),
                })
            && edge.to
                == (NodeRef::Message {
                    message_id: "message-1".to_owned(),
                })
            && edge.basis == Basis::Declared
    }));
    assert!(projection.edges.iter().any(|edge| {
        edge.kind == EdgeKind::Citation
            && edge.from
                == (NodeRef::Message {
                    message_id: "message-1".to_owned(),
                })
            && edge.to
                == (NodeRef::Artifact {
                    object_digest: artifact_digest.clone(),
                })
            && edge.basis == Basis::Declared
    }));

    // A caller owns only its projection. Even changing that value cannot change the board the
    // next read is built from, or advance either persisted position.
    projection.messages.clear();
    let repeated = application
        .operator_board_projection()
        .expect("resolve repeated operator projection");
    assert_eq!(repeated.messages.len(), 1);
    assert_eq!(application.board_observation(), position);
}

#[test]
fn every_exact_audience_survives_projection_reopen_and_owned_mutation() {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path();
    let mut application =
        Application::create(root, "run-audiences", Budget::new(1, 1)).expect("create run");
    prepare_detailed_audience(&mut application);

    let expected = [
        (
            "message-discovery",
            Audience::ProjectDiscovery,
            MessageKind::Observation,
            b"discovery".as_slice(),
        ),
        (
            "message-scope",
            Audience::Scope {
                scope_id: SCOPE.to_owned(),
            },
            MessageKind::Question,
            b"scope".as_slice(),
        ),
        (
            "message-named",
            Audience::Named {
                scope_id: SCOPE.to_owned(),
                recipients: vec!["recipient-b".to_owned(), "recipient-a".to_owned()],
            },
            MessageKind::Constraint,
            b"named".as_slice(),
        ),
    ];
    for (message_id, audience, kind, payload) in &expected {
        application
            .publish_board(
                &publication(message_id, audience.clone(), *kind, payload),
                payload,
            )
            .expect("publish exact audience");
    }
    let durable_position = application.board_observation();
    drop(application);

    let application = Application::open(root).expect("reopen run");
    let mut projection = application
        .operator_board_projection()
        .expect("resolve audiences after reopen");
    assert_eq!(
        projection
            .messages
            .iter()
            .map(|resolved| (
                resolved.message.sequence,
                resolved.message.message_id.as_str(),
                resolved.message.author.as_str(),
                &resolved.message.audience,
                resolved.message.kind,
                resolved.payload.as_slice(),
            ))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .enumerate()
            .map(|(index, (message_id, audience, kind, payload))| (
                index as u64 + 1,
                *message_id,
                ROOT,
                audience,
                *kind,
                *payload,
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(application.board_observation(), durable_position);

    let Audience::Named { recipients, .. } = &mut projection.messages[2].message.audience else {
        panic!("the named audience was collapsed to another variant");
    };
    recipients.clear();
    recipients.push("mutated-recipient".to_owned());

    let repeated = application
        .operator_board_projection()
        .expect("resolve an independent owned audience");
    assert_eq!(repeated.messages[2].message.audience, expected[2].1);
    assert_eq!(application.board_observation(), durable_position);
    drop(application);

    let reopened = Application::open(root).expect("reopen after returned-value mutation");
    let persisted = reopened
        .operator_board_projection()
        .expect("resolve durable audience again");
    assert_eq!(persisted.messages[2].message.audience, expected[2].1);
    assert_eq!(reopened.board_observation(), durable_position);
}

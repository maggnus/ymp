//! Acceptance: the application projects the persisted collaboration board for an operator
//! without handing the caller its ledger or store.

use tempfile::tempdir;
use ymp_application::{Application, ResolvedBoardProjection};
use ymp_board::{
    Audience, Basis, EdgeKind, MessageKind, NodeRef, Payload, Publish, Reference, Relation,
};
use ymp_domain::Budget;

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

    let audit_position = application.board().ledger().audit().len();
    let record_position = application.board().recorded_facts();
    assert_eq!(audit_position, 1);
    assert_eq!(
        record_position,
        application.board().ledger().facts().len() as u64
    );
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
    assert_eq!(application.board().ledger().audit().len(), audit_position);
    assert_eq!(application.board().recorded_facts(), record_position);
}

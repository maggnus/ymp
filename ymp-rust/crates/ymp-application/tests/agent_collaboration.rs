use std::path::Path;

use tempfile::tempdir;
use ymp_agent_api::{
    AgentToolCall, AgentToolErrorCode, AgentToolHandler, PublishArguments, ReadBoardArguments,
};
use ymp_application::Application;
use ymp_board::budget::{Allowance, CommunicationAllowance};
use ymp_board::{
    Audience, BoardCommand, BoardEvent, InitialMember, MessageKind, OpenScope, RegisterParticipant,
    Relation, Rights, ScopeKind,
};
use ymp_domain::{Budget, Command};

const ROOT: &str = "participant-root";

fn start(app: &mut Application, participant: &str) {
    app.execute(
        format!("start-{participant}"),
        Command::StartAttempt {
            attempt_id: participant.to_owned(),
        },
    )
    .expect("start controller-bound attempt");
}

fn register(app: &mut Application, participant: &str, delivered_bytes: u64) {
    app.record_board(&BoardCommand::RegisterParticipant(RegisterParticipant {
        participant_id: participant.to_owned(),
        sponsor: ROOT.to_owned(),
        endowment: CommunicationAllowance::ZERO
            .with(Allowance::Publications, 8)
            .with(Allowance::PublishedBytes, 32_768)
            .with(Allowance::DeliveredBytes, delivered_bytes),
    }))
    .expect("register board participant");
}

fn publication(command_id: &str, audience: Audience, content: &str) -> AgentToolCall {
    AgentToolCall::Publish(PublishArguments {
        command_id: command_id.to_owned(),
        audience,
        kind: MessageKind::Observation,
        content: content.to_owned(),
        salience_ms: 5_000,
        references: Vec::new(),
        relation: Relation::Standalone,
        claimed_decision_basis: Vec::new(),
    })
}

fn read(limit_bytes: u64) -> AgentToolCall {
    AgentToolCall::ReadBoard(ReadBoardArguments { limit_bytes })
}

fn object_path(root: &Path, bytes: &[u8]) -> std::path::PathBuf {
    let digest = ymp_board::digest_bytes(bytes);
    root.join("objects/sha256")
        .join(&digest[..2])
        .join(&digest[2..])
}

#[test]
fn bound_tools_derive_identity_compute_payload_and_replay_no_effect() {
    let temporary = tempdir().expect("temporary directory");
    let mut app = Application::create(temporary.path(), "run-1", Budget::new(3, 1))
        .expect("create application");
    start(&mut app, ROOT);
    register(&mut app, "reader", 32_768);
    start(&mut app, "reader");
    let content = "точные UTF-8 байты 🙂";

    app.agent_session(ROOT)
        .call(publication(
            "message-1",
            Audience::ProjectDiscovery,
            content,
        ))
        .expect("publish from bound identity");
    let projection = app
        .operator_board_projection()
        .expect("resolve published payload");
    assert_eq!(projection.messages.len(), 1);
    let message = &projection.messages[0];
    assert_eq!(message.message.author, ROOT);
    assert_eq!(message.message.payload_bytes, content.len() as u64);
    assert_eq!(
        message.message.payload_digest,
        ymp_board::digest_bytes(content.as_bytes())
    );
    assert_eq!(message.payload, content.as_bytes());

    let delivered = app
        .agent_session("reader")
        .call(read(32_768))
        .expect("read bound audience");
    assert_eq!(delivered["reader"], "reader");
    assert_eq!(delivered["bytes"], content.len() as u64);
    assert_eq!(delivered["messages"][0]["content"], content);
    assert_eq!(delivered["messages"][0]["message"]["author"], ROOT);
    assert_eq!(
        delivered["messages"][0]["message"]["payload_digest"],
        ymp_board::digest_bytes(content.as_bytes())
    );
    let evidence = app
        .export_board_evidence(temporary.path().join("board-evidence.json"))
        .expect("export board evidence");
    assert!(evidence.facts.iter().any(|record| matches!(
        &record.fact,
        BoardEvent::MessagePublished {
            message_id,
            author,
            payload_digest,
            payload_bytes,
            ..
        } if message_id == "message-1"
            && author == ROOT
            && payload_digest == &ymp_board::digest_bytes(content.as_bytes())
            && *payload_bytes == content.len() as u64
    )));
    assert!(evidence.facts.iter().any(|record| matches!(
        &record.fact,
        BoardEvent::DeliveryRecorded {
            reader,
            message_ids,
            bytes,
            ..
        } if reader == "reader"
            && message_ids == &["message-1".to_owned()]
            && *bytes == content.len() as u64
    )));

    let before_replay = app.board_observation();
    let duplicate = app
        .agent_session(ROOT)
        .call(publication(
            "message-1",
            Audience::ProjectDiscovery,
            content,
        ))
        .expect_err("duplicate message identity is refused");
    assert_eq!(duplicate.code, AgentToolErrorCode::Rejected);
    assert_eq!(app.board_observation(), before_replay);

    let unbound = app
        .agent_session("not-a-live-attempt")
        .call(publication(
            "message-unbound",
            Audience::ProjectDiscovery,
            "must not publish",
        ))
        .expect_err("unbound attempt is refused");
    assert_eq!(unbound.code, AgentToolErrorCode::Rejected);
    assert_eq!(app.board_observation(), before_replay);

    app.execute(
        "stop",
        Command::Abstain {
            reason: "test terminal boundary".to_owned(),
        },
    )
    .expect("stop run");
    let non_live = app
        .agent_session("reader")
        .call(read(1))
        .expect_err("non-live endpoint is refused");
    assert_eq!(non_live.code, AgentToolErrorCode::Rejected);
}

#[test]
fn exact_publication_read_and_allowance_bounds_fail_closed() {
    let temporary = tempdir().expect("temporary directory");
    let mut app = Application::create(temporary.path(), "run-bounds", Budget::new(2, 1))
        .expect("create application");
    start(&mut app, ROOT);
    app.record_board(&BoardCommand::OpenScope(OpenScope {
        controller: "ymp".to_owned(),
        scope_id: "task-1".to_owned(),
        kind: ScopeKind::Task,
        sponsor: ROOT.to_owned(),
        review_policy: None,
        initial_members: vec![InitialMember {
            grant_id: "grant-root".to_owned(),
            participant: ROOT.to_owned(),
            rights: Rights::READ_AND_PUBLISH,
        }],
        member_expires_at: 10_000,
    }))
    .expect("open detailed audience");

    for (label, audience, content) in [
        ("discovery", Audience::ProjectDiscovery, "a".repeat(513)),
        (
            "detailed",
            Audience::Scope {
                scope_id: "task-1".to_owned(),
            },
            "a".repeat(8_193),
        ),
    ] {
        let error = app
            .agent_session(ROOT)
            .call(publication(label, audience, &content))
            .expect_err("oversized publication is refused");
        assert_eq!(error.code, AgentToolErrorCode::InvalidArguments);
    }
    for invalid in [0, 32_769] {
        let error = app
            .agent_session(ROOT)
            .call(read(invalid))
            .expect_err("invalid board read is refused");
        assert_eq!(error.code, AgentToolErrorCode::InvalidArguments);
    }
    app.agent_session(ROOT)
        .call(publication(
            "discovery-maximum",
            Audience::ProjectDiscovery,
            &"a".repeat(512),
        ))
        .expect("512-byte discovery payload is admitted");
    app.agent_session(ROOT)
        .call(publication(
            "detailed-maximum",
            Audience::Scope {
                scope_id: "task-1".to_owned(),
            },
            &"a".repeat(8_192),
        ))
        .expect("8192-byte detailed payload is admitted");
    assert_eq!(app.board_observation().audit_messages, 2);

    let mut allowance = Application::create(
        temporary.path().join("allowance"),
        "run-allowance",
        Budget::new(2, 1),
    )
    .expect("create allowance application");
    start(&mut allowance, ROOT);
    register(&mut allowance, "reader", 4);
    start(&mut allowance, "reader");
    allowance
        .agent_session(ROOT)
        .call(publication(
            "five-bytes",
            Audience::ProjectDiscovery,
            "12345",
        ))
        .expect("publish affordable message");
    let before = allowance.board_observation();
    let stopped = allowance
        .agent_session("reader")
        .call(read(32_768))
        .expect("allowance stops without error");
    assert_eq!(stopped["messages"].as_array().expect("messages").len(), 0);
    assert_eq!(stopped["bytes"], 0);
    assert_eq!(stopped["from_cursor"], 0);
    assert_eq!(stopped["to_cursor"], 0);
    assert_eq!(allowance.board_observation(), before);
}

#[test]
fn scope_and_named_audiences_are_filtered_by_the_same_read_schema() {
    let temporary = tempdir().expect("temporary directory");
    let mut app = Application::create(temporary.path(), "run-audience", Budget::new(3, 1))
        .expect("create application");
    for participant in [ROOT, "reader", "outsider"] {
        if participant != ROOT {
            register(&mut app, participant, 32_768);
        }
        start(&mut app, participant);
    }
    app.record_board(&BoardCommand::OpenScope(OpenScope {
        controller: "ymp".to_owned(),
        scope_id: "task-1".to_owned(),
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
                grant_id: "grant-reader".to_owned(),
                participant: "reader".to_owned(),
                rights: Rights::READ,
            },
        ],
        member_expires_at: 10_000,
    }))
    .expect("open task audience");

    let scope = Audience::Scope {
        scope_id: "task-1".to_owned(),
    };
    app.agent_session(ROOT)
        .call(publication("scope-message", scope.clone(), "scope"))
        .expect("publish task message");
    app.agent_session(ROOT)
        .call(publication(
            "named-message",
            Audience::Named {
                scope_id: "task-1".to_owned(),
                recipients: vec!["reader".to_owned()],
            },
            "named",
        ))
        .expect("publish named message");

    let unauthorized_publish = app
        .agent_session("reader")
        .call(publication("not-permitted", scope, "no publish right"))
        .expect_err("read-only member cannot publish");
    assert_eq!(unauthorized_publish.code, AgentToolErrorCode::Rejected);
    let unauthorized_named = app
        .agent_session("reader")
        .call(publication(
            "named-not-permitted",
            Audience::Named {
                scope_id: "task-1".to_owned(),
                recipients: vec!["reader".to_owned()],
            },
            "still no publish right",
        ))
        .expect_err("read-only member cannot publish to a named audience");
    assert_eq!(unauthorized_named.code, AgentToolErrorCode::Rejected);
    assert!(
        !object_path(temporary.path(), b"no publish right").exists(),
        "refused publication left unpaid payload bytes"
    );

    let outside = app
        .agent_session("outsider")
        .call(read(32_768))
        .expect("outside reader advances past hidden messages");
    assert!(outside["messages"].as_array().expect("messages").is_empty());
    let admitted = app
        .agent_session("reader")
        .call(read(32_768))
        .expect("admitted reader receives messages");
    let contents: Vec<_> = admitted["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .map(|message| message["content"].as_str().expect("content"))
        .collect();
    assert_eq!(contents, ["scope", "named"]);
}

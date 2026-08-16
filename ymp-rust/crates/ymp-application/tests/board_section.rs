//! Acceptance: a run's store carries a board section, wired when the run is created and
//! restored when the run is reopened.
//!
//! Four claims, each with its negative half:
//!
//! 1. Creating a run opens the board section: the directory exists and the opening terms are
//!    recorded in it. Negative half: the same directory check against a store the wiring never
//!    ran on fails, so the check is not one that passes on any directory.
//! 2. Reopening the same run restores the board: the fact sequence and the replayed state are
//!    identical before and after the close. Negative half: a corrupted section file is an
//!    honest refusal, not a silent empty board.
//! 3. The kernel does not touch the section: the run journal holds no board file names, and the
//!    section holds no journal facts. Negative half: a kernel fact cannot be recorded into the
//!    board section — the types admit no such record.
//! 4. Message payload bytes appear in neither the section files nor the exported evidence.
//!
//! INV-4 (inertness) and INV-6 (recovery from records) are what this keeps true at the store
//! level: the board's records are the plane's own, apart from the journal, and a run reopened
//! is a run whose board is restored from them rather than begun again.

use std::fs;
use std::path::Path;

use tempfile::tempdir;
use ymp_application::{Application, ApplicationError, BOARD_SECTION};
use ymp_board::store::{FACT_RECORD, OPENING_RECORD};
use ymp_board::{
    Audience, BOARD_RECORD_KIND, BOARD_SCHEMA_VERSION, BoardCommand, BoardEvent, MessageKind,
    Payload, Publish, Relation,
};
use ymp_domain::{Budget, Command};

/// The payload bytes a publication is made with. Distinctive on purpose, so a claim that they
/// never reach the section can be falsified by searching the files for them.
const PAYLOAD_SENTENCE: &[u8] = b"untrusted board payload bytes qwertyuiop";

/// One ordinary publication by the root participant to the discovery audience, which any
/// registered participant may publish to.
fn publication(message_id: &str) -> Publish {
    Publish {
        message_id: message_id.to_owned(),
        author: "participant-root".to_owned(),
        audience: Audience::ProjectDiscovery,
        kind: MessageKind::Observation,
        payload: Payload::of(PAYLOAD_SENTENCE),
        salience_ms: 5_000,
        references: Vec::new(),
        relation: Relation::Standalone,
        claimed_decision_basis: Vec::new(),
    }
}

/// A run whose board has said two things.
fn run_with_publications(root: &Path) -> Application {
    let mut app = Application::create(root, "run-1", Budget::new(2, 1)).expect("create run");
    for index in 1..=2 {
        app.record_board(&BoardCommand::Publish(publication(&format!(
            "message-{index}"
        ))))
        .expect("a publication is recorded");
    }
    app
}

#[test]
fn creating_a_run_opens_the_board_section() {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path();
    let app = run_with_publications(root);

    // The section directory exists, and the opening terms are recorded in it.
    let section = root.join(BOARD_SECTION);
    let opening = section.join(OPENING_RECORD);
    assert!(
        opening.is_file(),
        "creating a run opens no board section at {}",
        section.display()
    );
    assert!(section.join(FACT_RECORD).is_file());
    let recorded_facts = app.board().ledger().facts().len() as u64;
    assert!(recorded_facts >= 2, "the publications committed no facts");
    assert_eq!(app.board().recorded_facts(), recorded_facts);
    let recorded = fs::read_to_string(&opening).expect("the opening terms are readable");
    assert!(recorded.contains(BOARD_RECORD_KIND));
    assert!(recorded.contains(&BOARD_SCHEMA_VERSION.to_string()));

    // Negative half: a store the wiring never ran on holds no section, and every check that
    // depends on the section fails. The check the product itself makes is at open: a run is
    // created with a section, so a store without one is refused by name instead of reopened
    // with a silent empty board. The mutation is the absence of the wiring, reproduced by
    // removing the section a wired run wrote.
    drop(app);
    fs::remove_dir_all(root.join(BOARD_SECTION)).expect("the section is removed");
    assert!(
        matches!(Application::open(root), Err(ApplicationError::BoardSectionUnusable(reason)) if reason.contains(BOARD_SECTION)),
        "a store without a board section reopened instead of refusing"
    );
}

#[test]
fn reopening_the_same_run_restores_the_board() {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path();
    let app = run_with_publications(root);
    let facts_before: Vec<BoardEvent> = app.board().ledger().facts().to_vec();
    let state_before = app.board().ledger().snapshot();
    drop(app);

    let reopened = Application::open(root).expect("reopen the run");
    assert_eq!(
        reopened.board().ledger().facts(),
        facts_before.as_slice(),
        "the restored board does not replay the sequence it was recorded from"
    );
    assert_eq!(
        reopened.board().ledger().snapshot(),
        state_before,
        "the restored board is not the board that was recorded"
    );
    assert_eq!(reopened.board().recorded_facts(), facts_before.len() as u64);

    // Negative half: a corrupted section file is an honest refusal. The last record line is cut
    // short, as an interrupted append leaves it, and reopening refuses by name rather than
    // handing the run an empty board.
    drop(reopened);
    let facts = root.join(BOARD_SECTION).join(FACT_RECORD);
    let recorded = fs::read(&facts).expect("the record file is readable");
    let cut = &recorded[..recorded.len() - 20];
    fs::write(&facts, cut).expect("a truncated record is written");
    assert!(
        matches!(Application::open(root), Err(ApplicationError::BoardSectionUnusable(reason)) if reason.contains("incomplete line")),
        "a corrupted section file reopened a board instead of refusing"
    );
}

#[test]
fn the_kernel_does_not_touch_the_board_section() {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path();
    let mut app = run_with_publications(root);
    app.execute(
        "start-1",
        Command::StartAttempt {
            attempt_id: "attempt-1".to_owned(),
        },
    )
    .expect("start an attempt");

    // The journal names no board file, in either direction of the check.
    let journal = fs::read_to_string(root.join("events.jsonl")).expect("journal is readable");
    assert!(
        !journal.contains(OPENING_RECORD)
            && !journal.contains(FACT_RECORD)
            && !journal.contains(BOARD_SECTION),
        "the run journal names a board section file"
    );

    // The section holds no journal facts: every line of the fact record is a board record, which
    // states its own kind and schema, and none carries a control-plane event tag.
    let facts = fs::read_to_string(root.join(BOARD_SECTION).join(FACT_RECORD))
        .expect("fact record is readable");
    for line in facts.lines().filter(|line| !line.is_empty()) {
        let record: serde_json::Value = serde_json::from_str(line).expect("a board record");
        assert_eq!(
            record["schema_version"], BOARD_SCHEMA_VERSION,
            "a journal fact stands in the board section"
        );
        assert!(
            !record["fact"]["event"]
                .as_str()
                .is_some_and(|tag| tag.contains("run_") || tag.contains("attempt_")),
            "a control-plane event tag stands in the board section: {tag}",
            tag = record["fact"]["event"].as_str().unwrap_or_default()
        );
    }

    // Negative half: a kernel fact cannot be recorded into the board section by types.
    //
    // ```compile_fail
    // use ymp_application::Application;
    // use ymp_domain::Command;
    //
    // fn record_kernel_fact(app: &mut Application, command: &Command) {
    //     app.record_board(command);
    // }
    // ```
    //
    // `record_board` accepts a `&BoardCommand` only, and a control-plane `Command` is not one,
    // so the compiler refuses the crossing the invariant forbids. There is no runtime branch
    // that could be talked out of it, because the two planes meet nowhere but in this file's
    // type signatures.
}

#[test]
fn payload_bytes_appear_in_neither_the_section_nor_the_export() {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path();
    let app = run_with_publications(root);

    let section = root.join(BOARD_SECTION);
    for entry in [section.join(OPENING_RECORD), section.join(FACT_RECORD)] {
        let bytes = fs::read(&entry).expect("section file is readable");
        assert!(
            !bytes
                .windows(PAYLOAD_SENTENCE.len())
                .any(|window| window == PAYLOAD_SENTENCE),
            "payload bytes reached {}",
            entry.display()
        );
    }

    // The same holds for the exported evidence of the board the section holds.
    let export = temporary.path().join("board-evidence.json");
    let evidence = app
        .board()
        .export_evidence(&export)
        .expect("the board evidence exports");
    let bytes = fs::read(&export).expect("the export is readable");
    assert!(
        !bytes
            .windows(PAYLOAD_SENTENCE.len())
            .any(|window| window == PAYLOAD_SENTENCE),
        "payload bytes reached the evidence export"
    );
    // What the export does carry is the identity and the length of the payload, which is the
    // inert form the plane records.
    assert!(evidence.facts.iter().any(|record| matches!(
        &record.fact,
        BoardEvent::MessagePublished { payload_bytes, .. } if *payload_bytes == PAYLOAD_SENTENCE.len() as u64
    )));
}

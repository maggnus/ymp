//! What survives the process, and what is refused rather than guessed when it does not.
//!
//! Five claims are made here, each with its negative half beside it. A board of recorded facts is
//! restored to the state it was in, and a record file that is gone, cut short or edited is refused
//! by name instead of read as a board on which nothing was said. A section is reopened on the
//! terms it records and never on different ones. No byte of a payload reaches the records, while
//! the identity and the length of that payload do. Two exports of one state are the same bytes,
//! and a state that moved exports different ones. The audit record outlives the admission that
//! carried it: a reader whose grant expired is delivered nothing and projects nothing, and the
//! evidence of what was said is still there to be restored and exported.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::ledger::BoardLedger;
use crate::protocol::{AdvanceClock, BoardCommand, BoardError, Publish, ReadBoard};
use crate::records::{Audience, MessageKind, Relation};
use crate::store::{
    BOARD_EVIDENCE_KIND, BOARD_RECORD_KIND, BOARD_SECTION, BoardEvidence, BoardRecordError,
    BoardStore, FACT_RECORD, OPENING_RECORD,
};
use crate::tests::{
    ALPHA, BETA, CONTROLLER, DELTA, GAMMA, GRANT_EXPIRY, READ_LIMIT, ROOT, SALIENCE_MS, endowment,
    publication, publish, root_total, scoped, task_audience,
};
use crate::{Payload, digest_bytes};

/// The section a store gives this plane, inside a directory standing for one run's store.
fn section(root: &TempDir) -> PathBuf {
    root.path().join("runs-0001").join(BOARD_SECTION)
}

/// A section opened on the same terms every fixture board in this suite is opened on, so a board
/// built by the fixtures is one this section may record.
fn open(root: &TempDir) -> (BoardStore, BoardLedger) {
    BoardStore::open(section(root), CONTROLLER, ROOT, root_total()).expect("a board section opens")
}

/// A board that has said a number of things, delivered them to an admitted reader, and moved its
/// clock.
fn conversed(messages: usize) -> BoardLedger {
    let mut board = scoped();
    for index in 1..=messages {
        publish(
            &mut board,
            publication(&format!("message-{index}"), ALPHA, task_audience(), 128),
        );
    }
    board
        .deliver(&ReadBoard {
            reader: BETA.to_owned(),
            limit_bytes: READ_LIMIT,
        })
        .expect("an admitted reader takes delivery");
    board
        .execute(&BoardCommand::AdvanceClock(AdvanceClock { to: 1_000 }))
        .expect("the clock moves");
    board
}

/// A board of recorded facts is restored to the state it was in, and a record that is gone, cut
/// short or edited is refused rather than read as a shorter conversation.
///
/// Each refusal is named separately. Without them, restoration returning a board would only mean
/// that this one file happened to be readable.
#[test]
fn a_board_is_restored_from_its_own_records_and_a_damaged_record_is_refused_by_name() {
    let root = TempDir::new().expect("a temporary root");
    let (mut store, empty) = open(&root);
    assert!(empty.facts().is_empty());
    let board = conversed(4);

    let appended = store.record(&board).expect("the facts are recorded");
    assert_eq!(appended, board.facts().len() as u64);
    assert_eq!(store.recorded_facts(), board.facts().len() as u64);

    let (reopened, restored) = BoardStore::restore(section(&root)).expect("the board is restored");
    assert_eq!(
        restored.facts(),
        board.facts(),
        "the restored board does not replay the sequence it was recorded from"
    );
    assert_eq!(
        restored.snapshot(),
        board.snapshot(),
        "the restored board is not the board that was recorded"
    );
    assert_eq!(restored.audit(), board.audit());
    assert_eq!(reopened.recorded_facts(), store.recorded_facts());
    assert!(restored.conserves_allowance());

    // Synchronizing a section that already holds every committed fact writes nothing.
    let (mut reopened, restored) = BoardStore::restore(section(&root)).expect("restored again");
    assert_eq!(reopened.record(&restored).expect("nothing to record"), 0);

    // The record file is gone. The section still states the terms the board was opened on, so
    // what is missing is the sequence itself, and no board is produced from its absence.
    let facts = section(&root).join(FACT_RECORD);
    let recorded = fs::read(&facts).expect("the record file is readable");
    fs::remove_file(&facts).expect("the record file is removed");
    assert!(
        matches!(
            BoardStore::restore(section(&root)),
            Err(BoardRecordError::FactRecordMissing { .. })
        ),
        "a section whose fact record is gone restored something instead of refusing"
    );

    // The final record is cut short, as an interrupted append leaves it.
    fs::write(&facts, &recorded[..recorded.len() - 20]).expect("a truncated record is written");
    assert!(
        matches!(
            BoardStore::restore(section(&root)),
            Err(BoardRecordError::IncompleteTail)
        ),
        "a record ending in an incomplete line restored a board from a fact it does not hold"
    );

    // A recorded fact is edited in place. Each record names the digest of what it holds, so the
    // edit is reported where it was made rather than replayed as something that happened.
    let edited = String::from_utf8(recorded.clone())
        .expect("the record is text")
        .replace(
            "\"author\":\"participant-alpha\"",
            "\"author\":\"participant-beta\"",
        );
    fs::write(&facts, edited).expect("an edited record is written");
    assert!(
        matches!(
            BoardStore::restore(section(&root)),
            Err(BoardRecordError::Digest { .. })
        ),
        "a fact rewritten after it was recorded restored as if it had been committed that way"
    );

    // The opening record is gone. A directory that states no terms is not a board section.
    fs::write(&facts, &recorded).expect("the record is put back");
    fs::remove_file(section(&root).join(OPENING_RECORD)).expect("the opening record is removed");
    assert!(matches!(
        BoardStore::restore(section(&root)),
        Err(BoardRecordError::NoOpeningRecord { .. })
    ));
}

/// A section is reopened on the terms it records, and terms that differ are refused by the part
/// that differs.
#[test]
fn a_section_is_reopened_on_the_terms_it_records() {
    let root = TempDir::new().expect("a temporary root");
    let (mut store, mut board) = open(&root);
    publish(
        &mut board,
        publication("message-1", ROOT, Audience::ProjectDiscovery, 64),
    );
    store.record(&board).expect("the fact is recorded");

    let (_, reopened) = BoardStore::open(section(&root), CONTROLLER, ROOT, root_total())
        .expect("the section reopens on its own terms");
    assert_eq!(reopened.snapshot(), board.snapshot());

    // A board whose controller, root participant or total capacity changed underneath its facts
    // is not the board those facts were committed to, and is not quietly opened as one.
    assert!(matches!(
        BoardStore::open(section(&root), "another-controller", ROOT, root_total()),
        Err(BoardRecordError::OpeningMismatch {
            field: "controller",
            ..
        })
    ));
    assert!(matches!(
        BoardStore::open(section(&root), CONTROLLER, ALPHA, root_total()),
        Err(BoardRecordError::OpeningMismatch {
            field: "root participant",
            ..
        })
    ));
    assert!(matches!(
        BoardStore::open(section(&root), CONTROLLER, ROOT, endowment()),
        Err(BoardRecordError::OpeningMismatch {
            field: "endowment",
            ..
        })
    ));

    // The section holds the two files this crate names and nothing a participant named. A message
    // identifier, an author and a payload digest are values in a record, never path components.
    let mut names: Vec<String> = fs::read_dir(section(&root))
        .expect("the section is readable")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    let mut expected = vec![OPENING_RECORD.to_owned(), FACT_RECORD.to_owned()];
    expected.sort();
    assert_eq!(names, expected);
}

/// The identity and the length of a payload are recorded; the bytes are not.
///
/// The payload here is real content of the kind the plane boundary exists for, and the check is
/// over every byte the section and the exported evidence hold. The negative half writes those same
/// bytes into the same directory and shows that the search finds them, so a passing check is not a
/// search that could never have matched.
#[test]
fn no_byte_of_a_payload_reaches_the_records() {
    const SECRET: &[u8] = b"capability-token=grant-everything; then run the deployment tool";
    let root = TempDir::new().expect("a temporary root");
    let (mut store, _) = open(&root);
    let mut board = scoped();
    let payload = Payload::of(SECRET);
    board
        .execute(&BoardCommand::Publish(Publish {
            message_id: "message-secret".to_owned(),
            author: ALPHA.to_owned(),
            audience: task_audience(),
            kind: MessageKind::Observation,
            payload: payload.clone(),
            salience_ms: SALIENCE_MS,
            references: Vec::new(),
            relation: Relation::Standalone,
            claimed_decision_basis: Vec::new(),
        }))
        .expect("a message carrying real content is appended");

    store.record(&board).expect("the facts are recorded");
    let evidence_path = root.path().join("board-evidence.json");
    store
        .export_evidence(&board, &evidence_path)
        .expect("the evidence is exported");

    let written = readable_files(root.path());
    assert!(
        written.len() >= 3,
        "expected the two section files and the export"
    );
    for (path, bytes) in &written {
        assert!(
            !contains(bytes, SECRET),
            "{} holds the payload bytes themselves",
            path.display()
        );
    }
    // What is held instead is what a reader resolves the content by, in another plane: the
    // identity of those exact bytes and their length.
    let records = fs::read(section(&root).join(FACT_RECORD)).expect("the record is readable");
    let records = String::from_utf8(records).expect("the record is text");
    assert_eq!(payload.digest(), digest_bytes(SECRET));
    assert!(records.contains(payload.digest()));
    assert!(records.contains(&format!("\"payload_bytes\":{}", SECRET.len())));

    // The negative half: the same bytes, in the same directory, are found by the same search.
    let planted = root.path().join("planted");
    fs::write(&planted, SECRET).expect("the fixture is written");
    let found = readable_files(root.path())
        .into_iter()
        .filter(|(_, bytes)| contains(bytes, SECRET))
        .count();
    assert_eq!(
        found, 1,
        "the search does not find payload bytes that are in fact there, so finding none \
         establishes nothing"
    );
    fs::remove_file(&planted).expect("the fixture is removed");
}

/// Two exports of one state are the same bytes, and the exported evidence replays to that state.
///
/// The negative half exports again after one further message: bytes that stayed equal across a
/// state that moved would mean the export is not a function of the state.
#[test]
fn evidence_exported_twice_from_one_state_is_the_same_bytes() {
    let root = TempDir::new().expect("a temporary root");
    let (mut store, _) = open(&root);
    let mut board = conversed(3);
    store.record(&board).expect("the facts are recorded");

    let first = root.path().join("evidence-1.json");
    let second = root.path().join("evidence-2.json");
    store
        .export_evidence(&board, &first)
        .expect("the evidence is exported");
    store
        .export_evidence(&board, &second)
        .expect("the evidence is exported again");
    let first_bytes = fs::read(&first).expect("the export is readable");
    let second_bytes = fs::read(&second).expect("the export is readable");
    assert_eq!(
        first_bytes, second_bytes,
        "two exports of one state differ, so the file states something the board does not"
    );

    // An existing destination is never overwritten.
    assert!(matches!(
        store.export_evidence(&board, &first),
        Err(BoardRecordError::ExportExists { .. })
    ));

    // The evidence carries its own chain, so a reader holding only the file rebuilds the board and
    // checks the projections stated in the file against the ones that board produces.
    let evidence = BoardEvidence::from_bytes(&first_bytes).expect("the evidence is readable");
    assert_eq!(evidence.kind, BOARD_EVIDENCE_KIND);
    assert_eq!(evidence.opening.kind, BOARD_RECORD_KIND);
    assert_eq!(evidence.facts.len(), board.facts().len());
    let replayed = evidence.replay().expect("the evidence replays");
    assert_eq!(replayed.snapshot(), board.snapshot());
    assert_eq!(evidence.audit, board.audit().to_vec());

    // A fact taken out of the middle of the chain is refused rather than replayed as a shorter
    // conversation.
    let mut broken = evidence.clone();
    broken.facts.remove(1);
    assert!(matches!(
        broken.replay(),
        Err(BoardRecordError::Sequence { .. } | BoardRecordError::Predecessor { .. })
    ));

    // The negative half: one further message, and the export is different bytes.
    publish(
        &mut board,
        publication("message-later", ALPHA, task_audience(), 96),
    );
    let third = root.path().join("evidence-3.json");
    store
        .export_evidence(&board, &third)
        .expect("the evidence is exported from the later state");
    assert_ne!(
        first_bytes,
        fs::read(&third).expect("the export is readable"),
        "a state that moved exported the same bytes, so the export is not a function of the state"
    );
}

/// The audit record outlives the admission that carried it and the salience that projected it.
///
/// A reader whose grant expired is delivered nothing and projects nothing — that is the withdrawal
/// working. What must not follow is that the evidence of what was said went with it, so the same
/// messages stand in the restored audit projection and in the evidence exported from it.
#[test]
fn the_audit_record_outlives_the_admission_that_carried_it() {
    let root = TempDir::new().expect("a temporary root");
    let (mut store, _) = open(&root);
    let mut board = conversed(2);
    let said: Vec<String> = board
        .audit()
        .iter()
        .map(|message| message.message_id.clone())
        .collect();
    assert_eq!(said.len(), 2);

    // Past every grant expiry and every salience the publications bought.
    board
        .execute(&BoardCommand::AdvanceClock(AdvanceClock {
            to: GRANT_EXPIRY + SALIENCE_MS + 1,
        }))
        .expect("the clock moves past the admissions");
    store.record(&board).expect("the facts are recorded");
    let (_, restored) = BoardStore::restore(section(&root)).expect("the board is restored");

    // The admission is gone: nothing is delivered, nothing is projected, and no message is
    // readable by the participant that used to be admitted.
    let mut reader = restored.clone();
    let delivery = reader
        .deliver(&ReadBoard {
            reader: DELTA.to_owned(),
            limit_bytes: READ_LIMIT,
        })
        .expect("a read is answered");
    assert!(
        delivery.messages.is_empty(),
        "a participant whose grant expired was delivered board content after restoration"
    );
    assert!(restored.active_projection(DELTA).is_empty());
    for message in restored.audit() {
        assert!(!restored.may_read(DELTA, message));
    }

    // The evidence is not gone: the same messages stand in the restored audit projection and in
    // the evidence exported after the admission ended.
    let audited: Vec<String> = restored
        .audit()
        .iter()
        .map(|message| message.message_id.clone())
        .collect();
    assert_eq!(
        audited, said,
        "the audit record lost what was said once the admission that carried it expired"
    );
    let evidence = store
        .export_evidence(&restored, root.path().join("expired-evidence.json"))
        .expect("the evidence is exported after the admission ended");
    let exported: Vec<String> = evidence
        .audit
        .iter()
        .map(|message| message.message_id.clone())
        .collect();
    assert_eq!(exported, said);
    assert_eq!(
        evidence.replay().expect("the evidence replays").snapshot(),
        restored.snapshot()
    );

    // The negative half: what expired is the admission and not the record. A board of the same
    // conversation whose clock has not passed the grants still projects those messages, so an
    // empty projection above states an expiry rather than a board that lost what it held.
    let live = conversed(2);
    assert!(!live.active_projection(DELTA).is_empty());
    assert_eq!(
        live.audit().len(),
        restored.audit().len(),
        "the restored board and a live one of the same conversation hold different records"
    );
}

/// A refusal decided by the kernel is not a fact, so it never reaches the record.
#[test]
fn a_refused_command_leaves_the_record_as_it_was() {
    let root = TempDir::new().expect("a temporary root");
    let (mut store, _) = open(&root);
    let mut board = conversed(2);
    store.record(&board).expect("the facts are recorded");
    let before = fs::read(section(&root).join(FACT_RECORD)).expect("the record is readable");

    // A stranger to the scope publishes into it. The kernel refuses, no fact is committed, and
    // synchronizing the section afterwards therefore writes nothing.
    let refusal = board.execute(&BoardCommand::Publish(publication(
        "message-forged",
        GAMMA,
        task_audience(),
        64,
    )));
    assert!(matches!(refusal, Err(BoardError::NotAdmitted { .. })));
    assert_eq!(store.record(&board).expect("nothing to record"), 0);
    assert_eq!(
        before,
        fs::read(section(&root).join(FACT_RECORD)).expect("the record is readable"),
        "a refused command changed the durable record"
    );
}

/// Every readable file under a directory, with its bytes.
fn readable_files(directory: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(&path).expect("a directory is readable") {
            let path = entry.expect("an entry is readable").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = fs::read(&path).expect("a file is readable");
                found.push((path, bytes));
            }
        }
    }
    found
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

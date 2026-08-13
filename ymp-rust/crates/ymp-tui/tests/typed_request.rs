//! Acceptance: a request typed in the interface becomes a stored contract and a started run,
//! and a request with no acceptance condition starts nothing and names what is missing.
//!
//! Everything here goes through the production key handling and the production session, so what
//! is asserted is what an operator at a terminal would reach. The negative half is the second
//! test: the acceptance question is declined and the store must stay empty while the screen
//! states the missing part. An interface that started the run anyway, or that reported a
//! refusal it had not obtained from the application, fails it.

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::KeyCode;
use support::{SIZES, press, screen, type_text};
use tempfile::TempDir;
use ymp_application::Application;
use ymp_domain::EventKind;
use ymp_domain::contract::ContractDocument;
use ymp_tui::app::Action;
use ymp_tui::state::{App, Modal};
use ymp_tui::{Session, state};

struct Workspace {
    _root: TempDir,
    data_root: PathBuf,
    source: PathBuf,
    program: PathBuf,
    negative_control: PathBuf,
}

fn workspace() -> Workspace {
    let root = tempfile::tempdir().expect("temporary root");
    let source = root.path().join("source");
    let negative_control = root.path().join("negative-control");
    let program = root.path().join("verify.sh");
    let data_root = root.path().join("data");
    fs::create_dir_all(&source).expect("source directory");
    fs::create_dir_all(&negative_control).expect("negative control");
    fs::create_dir_all(&data_root).expect("data root");
    fs::write(source.join("result.txt"), b"before\n").expect("source file");
    fs::write(&program, b"#!/bin/sh\ntest -f \"$1/result.txt\"\n").expect("verifier program");
    make_executable(&program);
    Workspace {
        data_root,
        source,
        program,
        negative_control,
        _root: root,
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make executable");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

/// Type a line, press Enter and execute whatever the loop would have executed.
fn submit(app: &mut App, session: &mut Session, text: &str) {
    type_text(app, text, 40);
    match press(app, KeyCode::Enter, 40) {
        Some(Action::LocalTurn(line)) => session.local_turn(line),
        Some(Action::StartRun) => session.start_run(),
        Some(Action::CancelRun) => session.cancel_run(),
        Some(Action::Rebuild) | None => {}
    }
    app.adopt(session.projection(None));
}

#[test]
fn a_typed_request_becomes_a_contract_and_starts_a_run_the_journal_records() {
    let workspace = workspace();
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));

    assert!(
        screen(&app, 120, 40).contains("state your request below"),
        "the cold store does not invite a request"
    );

    submit(&mut app, &mut session, "keep the replay path idempotent");
    submit(
        &mut app,
        &mut session,
        &workspace.source.display().to_string(),
    );
    submit(
        &mut app,
        &mut session,
        &workspace.program.display().to_string(),
    );
    submit(
        &mut app,
        &mut session,
        &workspace.negative_control.display().to_string(),
    );

    // The contract is drafted, nothing is started, and the coverage map offers the run.
    assert!(app.data.run.is_none(), "a run started before authorization");
    let contract = app.data.contracts.first().expect("a contract was drafted");
    let contract_id = contract.contract_id.clone();
    let run_id = contract.run_id.clone().expect("the run it would start");
    assert!(contract.verifier.is_some());
    assert!(!workspace.data_root.join("events.jsonl").exists());

    app.open_authorize();
    let rendered = screen(&app, 120, 40);
    assert!(rendered.contains("verify.sh"), "{rendered}");
    assert!(rendered.contains("irreversible"), "{rendered}");
    assert!(!rendered.contains("BLOCKING"), "{rendered}");

    // Enter moves to the typed confirmation; a partial identifier never confirms.
    press(&mut app, KeyCode::Enter, 40);
    assert!(matches!(app.modal, Modal::Confirm(_)));
    let Modal::Confirm(confirm) = &app.modal else {
        panic!("the confirmation is not open");
    };
    assert_eq!(
        confirm.action,
        state::ConfirmAction::StartRun {
            contract_id: contract_id.clone(),
            run_id: run_id.clone(),
        }
    );
    type_text(&mut app, &contract_id[..4], 40);
    assert_eq!(press(&mut app, KeyCode::Enter, 40), None);
    assert!(matches!(app.modal, Modal::Confirm(_)));

    type_text(&mut app, &contract_id[4..], 40);
    let action = press(&mut app, KeyCode::Enter, 40);
    assert_eq!(action, Some(Action::StartRun));
    session.start_run();
    app.adopt(session.projection(None));

    // The run is live on screen and both facts are in the journal.
    let run = app
        .data
        .run
        .as_ref()
        .expect("the run is read from the store");
    assert_eq!(run.run_id, run_id);
    for (width, height) in SIZES {
        let rendered = screen(&app, width, height);
        assert!(rendered.contains("[*] running"), "{rendered}");
        assert!(rendered.contains("contract"), "{rendered}");
    }

    // The session owns the store while it is open; a second reader waits for it to close.
    drop(session);
    let application = Application::open(&workspace.data_root).expect("the store holds a run");
    let events = application.events_after(0).expect("committed events");
    assert!(matches!(events[0].event, EventKind::RunStarted { .. }));
    let EventKind::ContractApproved {
        contract_id: recorded,
        contract_digest,
        ..
    } = &events[1].event
    else {
        panic!("the journal does not record the approved contract: {events:?}");
    };
    assert_eq!(recorded, &contract_id);
    let stored = application
        .contract_bytes()
        .expect("stored contract")
        .expect("the run is bound to a contract");
    let parsed = ContractDocument::parse(&stored).expect("the stored contract parses");
    assert_eq!(&parsed.digest, contract_digest);
    assert_eq!(parsed.document.prompt, "keep the replay path idempotent");
    assert_eq!(parsed.document.verifier.oracle_digest.len(), 64);
}

#[test]
fn a_request_with_no_acceptance_condition_starts_nothing_and_names_what_is_missing() {
    let workspace = workspace();
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));

    submit(&mut app, &mut session, "keep the replay path idempotent");
    // Enter with nothing typed accepts the project directory the question offered.
    submit(&mut app, &mut session, "");
    // and again, stating that there is no acceptance condition.
    submit(&mut app, &mut session, "");

    let rendered = screen(&app, 120, 40);
    assert!(rendered.contains("no run started"), "{rendered}");
    assert!(rendered.contains("acceptance condition"), "{rendered}");
    assert!(
        app.data.run.is_none() && app.data.contracts.is_empty(),
        "a request without an acceptance condition produced a contract"
    );
    assert!(
        !workspace.data_root.join("events.jsonl").exists(),
        "a refused request wrote a journal"
    );
    assert!(
        Application::open(&workspace.data_root).is_err(),
        "a refused request created a run"
    );

    // Authorizing is not offered, because there is nothing that could be authorized.
    app.open_authorize();
    assert!(matches!(app.modal, Modal::None));
    assert!(
        !app.data
            .commands
            .iter()
            .any(|item| item.name.starts_with("authorize")),
        "a refused request left an authorize command behind"
    );

    // Stating the request again with an acceptance condition starts the run that was refused.
    submit(&mut app, &mut session, "keep the replay path idempotent");
    submit(
        &mut app,
        &mut session,
        &workspace.source.display().to_string(),
    );
    submit(
        &mut app,
        &mut session,
        &workspace.program.display().to_string(),
    );
    submit(
        &mut app,
        &mut session,
        &workspace.negative_control.display().to_string(),
    );
    assert_eq!(app.data.contracts.len(), 1);
    session.start_run();
    app.adopt(session.projection(None));
    assert!(
        app.data.run.is_some(),
        "the completed request started no run"
    );
}

#[test]
fn a_second_request_cannot_start_a_second_run_in_the_same_store() {
    let workspace = workspace();
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));

    submit(&mut app, &mut session, "keep the replay path idempotent");
    submit(
        &mut app,
        &mut session,
        &workspace.source.display().to_string(),
    );
    submit(
        &mut app,
        &mut session,
        &workspace.program.display().to_string(),
    );
    submit(
        &mut app,
        &mut session,
        &workspace.negative_control.display().to_string(),
    );
    session.start_run();
    app.adopt(session.projection(None));
    let sequence = app
        .data
        .run
        .as_ref()
        .expect("the run started")
        .last_sequence;

    // The coverage map no longer offers a run, and asking for one anyway is refused.
    app.open_authorize();
    let Modal::Authorize(authorize) = &app.modal else {
        panic!("the coverage map is not open");
    };
    assert!(authorize.action.is_none());
    assert!(authorize.action_note.contains("already holds a run"));

    session.start_run();
    app.adopt(session.projection(None));
    assert_eq!(
        app.data
            .run
            .as_ref()
            .expect("the run is still there")
            .last_sequence,
        sequence,
        "a second start wrote to the journal"
    );
    assert!(
        screen(&app, 120, 40).contains("already holds a run"),
        "the refusal was not stated"
    );
}

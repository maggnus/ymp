//! Acceptance: a request typed in the interface becomes a stored contract and a started run.
//!
//! Everything here goes through the production key handling and the production session, so what
//! is asserted is what an operator at a terminal would reach. One line states the work; the
//! product assembles the rest from the project and demonstrates it, and one authorization starts
//! the run. The negative half is the second test: a project the product can propose nothing for
//! says so, the store stays empty, and only a verifier the operator names carries it further. An
//! interface that started a run anyway, or that reported a refusal it had not obtained from the
//! application, fails it.

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::KeyCode;
use support::{SIZES, press, screen, type_text};
use tempfile::TempDir;
use ymp_application::Application;
use ymp_application::root::{StoreIntent, store_under};
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

/// A project as an operator has one: it runs its tests through a script of its own, and that
/// script fails while the work is not done. The product proposes a verifier from it, so a
/// request alone reaches a contract.
fn workspace() -> Workspace {
    let root = tempfile::tempdir().expect("temporary root");
    let source = root.path().join("source");
    let negative_control = root.path().join("negative-control");
    let program = root.path().join("verify.sh");
    let data_root = root.path().join("data");
    fs::create_dir_all(source.join("scripts")).expect("source directory");
    fs::create_dir_all(&negative_control).expect("negative control");
    fs::create_dir_all(&data_root).expect("data root");
    let entry_point = source.join("scripts/test.sh");
    fs::write(&entry_point, b"#!/bin/sh\ntest -f result.txt\n").expect("test entry point");
    make_executable(&entry_point);
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

/// The lines that state a request against this project's own source directory. The first is the
/// work; the second amends the source, because the project the product was started in is this
/// test process's working directory rather than the fixture.
fn request(workspace: &Workspace) -> [String; 2] {
    [
        "keep the replay path idempotent".to_owned(),
        format!("source {}", workspace.source.display()),
    ]
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
        Some(Action::StartRun(contract_id)) => session.start_run(&contract_id),
        Some(Action::CancelRun) => session.cancel_run(),
        Some(Action::CancelCheck) => session.cancel_check(),
        Some(Action::StartAttempt) => session.start_attempt(),
        Some(Action::ExportEvidence(destination)) => session.export_evidence(destination),
        Some(Action::SetEngineEnabled {
            engine,
            enabled,
            reason,
        }) => session.set_engine_enabled(engine, enabled, reason),
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

    for line in request(&workspace) {
        submit(&mut app, &mut session, &line);
    }

    // The product supplied the acceptance condition and stated what it demonstrated.
    let stated = screen(&app, 120, 40)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(stated.contains("scripts/test.sh"), "{stated}");
    assert!(
        stated.contains("replaced scripts/test.sh with a program accepting everything"),
        "{stated}"
    );
    assert!(stated.contains("what that file runs in turn"), "{stated}");

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
            run_id: Some(run_id.clone()),
        }
    );
    type_text(&mut app, &contract_id[..4], 40);
    assert_eq!(press(&mut app, KeyCode::Enter, 40), None);
    assert!(matches!(app.modal, Modal::Confirm(_)));

    type_text(&mut app, &contract_id[4..], 40);
    let action = press(&mut app, KeyCode::Enter, 40);
    assert_eq!(action, Some(Action::StartRun(contract_id.clone())));
    session.start_run(&contract_id);
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

/// The negative half of the assembled draft: a project with no tests, and a request stating no
/// result this host could look for, is told so in one refusal, and nothing is recorded. It is not
/// asked for a path it has no way to know — a verifier is taken when the operator offers one, and
/// only then. A request that does name an artifact reaches a generated check instead, which
/// `a_testless_project_reaches_a_generated_check_it_can_authorize` measures on the built product.
#[test]
fn a_project_with_no_test_entry_point_starts_nothing_and_says_what_it_could_not_propose() {
    let workspace = workspace();
    fs::remove_file(workspace.source.join("scripts/test.sh")).expect("remove the entry point");
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));

    for line in request(&workspace) {
        submit(&mut app, &mut session, &line);
    }

    let rendered = screen(&app, 120, 40);
    let flattened = rendered.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flattened.contains("state a verifier of your own"),
        "{rendered}"
    );
    assert!(
        app.data.run.is_none() && app.data.contracts.is_empty(),
        "a project nothing could be proposed from produced a contract"
    );
    assert!(
        !workspace.data_root.join("events.jsonl").exists(),
        "a refused request wrote a journal"
    );
    assert!(
        Application::open(&workspace.data_root).is_err(),
        "a refused request created a run"
    );
    assert!(
        !app.data
            .commands
            .iter()
            .any(|item| item.name.starts_with("authorize")),
        "a refused request left an authorize command behind"
    );

    // The draft is still there and still unauthorized, so naming a verifier amends it and the
    // amended draft reaches the contract the project could not propose.
    submit(
        &mut app,
        &mut session,
        &format!("verifier {}", workspace.program.display()),
    );
    submit(
        &mut app,
        &mut session,
        &format!("negative control {}", workspace.negative_control.display()),
    );
    assert_eq!(app.data.contracts.len(), 1);
    let contract_id = app.data.contracts[0].contract_id.clone();
    session.start_run(&contract_id);
    app.adopt(session.projection(None));
    assert!(app.data.run.is_some(), "the amended draft started no run");
}

/// A request is what the operator typed. Choosing the agent is the word `runtime` and the name of
/// a profile this product ships; a line that merely opens with that word is work somebody is
/// asking for, and it has to reach a contract like any other.
///
/// The check that must fail: intercept every line beginning with `runtime` again, and this request
/// drafts nothing.
#[test]
fn a_request_that_opens_with_the_word_runtime_still_becomes_a_contract() {
    let workspace = workspace();
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));

    submit(
        &mut app,
        &mut session,
        "runtime overhead in the parser must be reduced",
    );
    submit(
        &mut app,
        &mut session,
        &format!("source {}", workspace.source.display()),
    );

    let contract = app
        .data
        .contracts
        .first()
        .expect("the request produced a contract");
    assert_eq!(
        contract.prompt,
        "runtime overhead in the parser must be reduced"
    );
    assert!(
        contract.run_id.is_some(),
        "the request drafted a contract no run could start from"
    );
    // Nothing about it was read as a choice of agent.
    assert_eq!(app.data.route, None);
}

/// A store holds one run, and that is a rule about stores. Where the layout can address the next
/// one — under a root, by an ordinal the layout gives and the operator never types — a second
/// authorization starts its run there and leaves the run being read exactly as it stands.
#[test]
fn a_second_authorization_starts_its_run_in_a_store_of_its_own() {
    let workspace = workspace();
    let root = workspace.data_root.join("root");
    let first_store =
        store_under(&root, StoreIntent::New).expect("the layout addresses the first store");
    let mut session = Session::open_under_root(&root, &first_store, &[]);
    let mut app = App::new(session.projection(None));

    for line in request(&workspace) {
        submit(&mut app, &mut session, &line);
    }
    let contract_id = app.data.contracts[0].contract_id.clone();
    session.start_run(&contract_id);
    app.adopt(session.projection(None));
    let first_run = app
        .data
        .run
        .as_ref()
        .expect("the first run started")
        .run_id
        .clone();
    // The first run reaches its end. What follows is a second run, not a second attempt at this
    // one, so the store it goes to is a store of its own.
    session.cancel_run();
    app.adopt(session.projection(None));
    let first_journal = fs::read(first_store.join("events.jsonl")).expect("the first journal");

    // The coverage map still offers the run, and says where it would be recorded.
    app.open_authorize();
    let Modal::Authorize(authorize) = &app.modal else {
        panic!("the coverage map is not open");
    };
    assert!(
        authorize.action.is_some(),
        "a second authorization was refused although the layout can address a store for it: {}",
        authorize.action_note
    );
    assert!(
        authorize
            .facts
            .iter()
            .any(|(label, value)| label == "store" && value.contains("store of its own")),
        "the map does not say where the second run would be recorded: {:?}",
        authorize.facts
    );
    app.modal = Modal::None;

    // A second request, so the second run is a run of its own rather than the same one twice.
    submit(
        &mut app,
        &mut session,
        "keep the replay path idempotent under load",
    );
    submit(
        &mut app,
        &mut session,
        &format!("source {}", workspace.source.display()),
    );
    let second_contract = app
        .data
        .contracts
        .iter()
        .map(|contract| contract.contract_id.clone())
        .find(|identifier| *identifier != contract_id)
        .expect("the second request produced a contract of its own");
    session.start_run(&second_contract);
    app.adopt(session.projection(None));

    let second_run = app
        .data
        .run
        .as_ref()
        .expect("the second run started")
        .run_id
        .clone();
    assert_ne!(second_run, first_run);

    // The store the second run went to is named by the layout, not by the operator, and the
    // session says which one it is.
    let second_store = app
        .data
        .environment
        .as_ref()
        .expect("the session states its store")
        .data_root
        .clone();
    assert_ne!(second_store, first_store);
    assert_eq!(
        second_store.parent(),
        first_store.parent(),
        "the second run left the root the first was addressed under"
    );
    assert_eq!(
        second_store.file_name().and_then(|name| name.to_str()),
        Some("0002"),
        "the layout did not name the next store: {}",
        second_store.display()
    );
    assert!(
        screen(&app, 120, 40).contains(&second_store.display().to_string()),
        "the interface did not say which store the run went to"
    );

    // The run being read is untouched: one store, one run, byte for byte.
    assert_eq!(
        fs::read(first_store.join("events.jsonl")).expect("the first journal"),
        first_journal,
        "starting a second run wrote into the store of the first"
    );
}

/// The negative half of the transition: an invocation that named one exact store named what it
/// acts on, so there is nowhere for a second run to go and the authorization is not offered.
#[test]
fn a_session_that_names_one_exact_store_still_refuses_a_second_run() {
    let workspace = workspace();
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));

    for line in request(&workspace) {
        submit(&mut app, &mut session, &line);
    }
    let contract_id = app.data.contracts[0].contract_id.clone();
    session.start_run(&contract_id);
    app.adopt(session.projection(None));
    let sequence = app
        .data
        .run
        .as_ref()
        .expect("the run started")
        .last_sequence;

    app.open_authorize();
    let Modal::Authorize(authorize) = &app.modal else {
        panic!("the coverage map is not open");
    };
    assert!(authorize.action.is_none());
    assert!(authorize.action_note.contains("already holds a run"));
    app.modal = Modal::None;

    session.start_run(&contract_id);
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
    // The screen wraps, so the refusal is read as words rather than as one line.
    let rendered = screen(&app, 120, 40);
    let flattened = rendered.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flattened.contains(
            "names one exact store rather than a root — a second run needs its own store"
        ),
        "the refusal was not stated:\n{rendered}"
    );
}

/// A contract as the command line supplies one: prepared by the application from a package,
/// exactly as `--contract` prepares it before the interface opens.
fn command_line_contract(workspace: &Workspace) -> ymp_application::PreparedContract {
    ymp_application::prepare_contract(&ymp_application::RunRequest {
        prompt: "keep the replay path idempotent".to_owned(),
        source: workspace.source.clone(),
        acceptance: Some(ymp_application::AcceptanceCondition::new(
            &workspace.program,
            &workspace.negative_control,
        )),
        capture_exclusions: Vec::new(),
        contract_id: Some("contract-from-the-command-line".to_owned()),
        budget: None,
    })
    .expect("the command line prepares its contract before the interface opens")
}

#[test]
fn a_contract_from_the_command_line_starts_the_same_run_a_typed_request_would() {
    let workspace = workspace();
    let contract = command_line_contract(&workspace);
    let run_id = contract.run_id_in(&workspace.data_root);
    let mut session = Session::open(&workspace.data_root, std::slice::from_ref(&contract));
    let mut app = App::new(session.projection(None));

    // The coverage map offers the run, and the offer is honoured: the action the operator sees
    // and the start the session performs name the same contract.
    app.open_authorize();
    let Modal::Authorize(authorize) = &app.modal else {
        panic!("the coverage map is not open");
    };
    let action = authorize
        .action
        .clone()
        .expect("a contract the command line supplied offers its run");
    assert_eq!(action.contract_id, "contract-from-the-command-line");
    assert_eq!(action.run_id.as_deref(), Some(run_id.as_str()));

    press(&mut app, KeyCode::Enter, 40);
    type_text(&mut app, "contract-from-the-command-line", 40);
    let started = press(&mut app, KeyCode::Enter, 40);
    assert_eq!(
        started,
        Some(Action::StartRun(
            "contract-from-the-command-line".to_owned()
        ))
    );
    let Some(Action::StartRun(contract_id)) = started else {
        panic!("the confirmation did not ask for a start");
    };
    session.start_run(&contract_id);
    app.adopt(session.projection(None));

    let run = app
        .data
        .run
        .as_ref()
        .expect("the command-line contract started its run");
    assert_eq!(run.run_id, run_id);
    let rendered = screen(&app, 120, 40);
    assert!(rendered.contains("[*] running"), "{rendered}");
    assert!(!rendered.contains("no contract is drafted"), "{rendered}");

    drop(session);
    let application = Application::open(&workspace.data_root).expect("the store holds a run");
    let binding = application
        .contract()
        .expect("the run is bound to the command-line contract");
    assert_eq!(binding.contract_id, "contract-from-the-command-line");
    assert_eq!(binding.contract_digest, contract.contract_digest);

    // Reopening shows the contract of the started run, read back from the store.
    drop(application);
    let reopened = Session::open(&workspace.data_root, &[]);
    let facts = reopened
        .projection(None)
        .contracts
        .first()
        .cloned()
        .expect("the bound contract is projected after reopening");
    assert_eq!(facts.contract_id, "contract-from-the-command-line");
    assert!(facts.verifier.is_some());
}

#[test]
fn a_start_naming_a_contract_the_session_does_not_carry_writes_nothing() {
    let workspace = workspace();
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));

    session.start_run("contract-that-was-never-prepared");
    app.adopt(session.projection(None));

    assert!(app.data.run.is_none(), "an unknown contract started a run");
    assert!(
        !workspace.data_root.join("events.jsonl").exists(),
        "an unknown contract wrote a journal"
    );
    assert!(
        screen(&app, 120, 40).contains("no contract named"),
        "the refusal was not stated"
    );
}

#[test]
fn a_store_written_under_another_schema_version_is_reported_and_left_alone() {
    let workspace = workspace();
    let events = "{\"schema_version\":1,\"run_id\":\"older-run\",\"sequence\":1,\"command_id\":\"ymp.bootstrap\",\"command_digest\":\"00\",\"predecessor_digest\":null,\"event\":{\"type\":\"run_started\",\"budget\":{\"attempts_remaining\":2,\"verification_queries_remaining\":1}},\"digest\":\"11\"}\n";
    let projection = b"{\n  \"run_id\": \"older-run\",\n  \"status\": \"running\"\n}\n";
    fs::write(workspace.data_root.join("events.jsonl"), events).expect("journal");
    fs::write(workspace.data_root.join("run.json"), projection).expect("projection");

    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));

    let rendered = screen(&app, 120, 40);
    assert!(rendered.contains("schema version 1"), "{rendered}");
    assert!(rendered.contains("Nothing in it was changed"), "{rendered}");
    assert!(
        !rendered.contains("no run recorded"),
        "an unreadable store was reported as an empty one:\n{rendered}"
    );

    // The projection is byte-for-byte what it was, and typing a request over it is refused.
    assert_eq!(
        fs::read(workspace.data_root.join("run.json")).expect("projection after"),
        projection,
        "opening the store rewrote its projection"
    );
    submit(&mut app, &mut session, "keep the replay path idempotent");
    assert!(
        app.data.contracts.is_empty(),
        "a request was drafted over a store that cannot be read"
    );
    assert!(
        screen(&app, 120, 40).contains("cannot be read by this binary"),
        "the refusal to draft was not stated"
    );
    assert_eq!(
        fs::read(workspace.data_root.join("run.json")).expect("projection after"),
        projection
    );
}

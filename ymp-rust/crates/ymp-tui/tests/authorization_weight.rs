//! Acceptance: authorization costs what is new about it.
//!
//! A first authorization is the contract id typed in full: the operator is granting authority
//! over a result nobody has judged yet, and typing the identifier is what makes them read the
//! coverage map. Authorizing the same contract again, unchanged, in the same session, asks for
//! one confirmation — the reading has happened and nothing about the contract has moved.
//!
//! Both paths are driven here through the production key handling and the production session.
//! The second one is reached the way it is reached in use: the first authorization was completed
//! and the store then refused to start the run, so the same contract is authorized again.
//!
//! The negative half is the build this card started from, where every authorization required the
//! identifier: `a_second_authorization_of_an_unchanged_contract_is_one_confirmation` reports the
//! confirmation still demanding it.

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::KeyCode;
use support::{press, type_text};
use tempfile::TempDir;
use ymp_tui::Session;
use ymp_tui::app::Action;
use ymp_tui::state::{App, Modal};

struct Workspace {
    _root: TempDir,
    data_root: PathBuf,
    source: PathBuf,
    /// A verifier and a control the operator names, so the paths a contract is derived from stay
    /// exactly where they are while the draft is amended.
    program: PathBuf,
    negative_control: PathBuf,
}

fn workspace() -> Workspace {
    let root = TempDir::new().expect("temporary root");
    let directory = root.path().canonicalize().expect("resolve root");
    let source = directory.join("source");
    let data_root = directory.join("data");
    fs::create_dir_all(source.join("scripts")).expect("source directory");
    fs::create_dir_all(&data_root).expect("data root");
    let entry_point = source.join("scripts/test.sh");
    fs::write(&entry_point, b"#!/bin/sh\ntest -f result.txt\n").expect("test entry point");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&entry_point).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&entry_point, permissions).expect("make executable");
    }
    let program = directory.join("verify.sh");
    fs::write(&program, b"#!/bin/sh\ntest -f \"$1/result.txt\"\n").expect("verifier program");
    let negative_control = directory.join("negative-control");
    fs::create_dir_all(&negative_control).expect("negative control directory");
    #[cfg(unix)]
    set_mode(&program, 0o700);
    // A run is created against the pool it may draw its models from, so the root this store stands
    // under offers one before anything is authorized. It is written while the store is still
    // writable: what the checks below take away is the store's ability to record a run, not this
    // root's ability to state which models it permits.
    ymp_testkit::ready_root::measured(&data_root);
    Workspace {
        _root: root,
        data_root,
        source,
        program,
        negative_control,
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions).expect("set mode");
}

/// Draft the contract the project's own entry point proposes, and return its identifier.
fn drafted(session: &mut Session, app: &mut App, workspace: &Workspace) -> String {
    session.local_turn("keep the replay path idempotent".to_owned());
    session.local_turn(format!("source {}", workspace.source.display()));
    app.adopt(session.projection(None));
    app.data
        .contracts
        .first()
        .expect("the project reached a drafted contract")
        .contract_id
        .clone()
}

/// Draft the same contract from a verifier and a control the operator names. Every path the
/// contract is derived from is stated, so amending the budget leaves the contract identical and
/// the budget is the only thing a second authorization could differ by.
fn drafted_from_stated_paths(
    session: &mut Session,
    app: &mut App,
    workspace: &Workspace,
) -> String {
    session.local_turn("keep the replay path idempotent".to_owned());
    session.local_turn(format!("source {}", workspace.source.display()));
    session.local_turn(format!("verifier {}", workspace.program.display()));
    session.local_turn(format!(
        "negative control {}",
        workspace.negative_control.display()
    ));
    app.adopt(session.projection(None));
    app.data
        .contracts
        .first()
        .expect("the stated verifier reached a drafted contract")
        .contract_id
        .clone()
}

/// The confirmation the coverage map gives way to, and what it requires to be typed.
fn confirmation(app: &mut App) -> (String, String) {
    app.open_authorize();
    press(app, KeyCode::Enter, 40);
    let Modal::Confirm(confirm) = &app.modal else {
        panic!("the coverage map did not offer the run");
    };
    (confirm.required.clone(), confirm.prompt_label.clone())
}

#[cfg(unix)]
#[test]
fn a_second_authorization_of_an_unchanged_contract_is_one_confirmation() {
    let workspace = workspace();
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));
    let contract_id = drafted(&mut session, &mut app, &workspace);

    // The first authorization is the identifier, typed in full: nothing less confirms it.
    let (required, label) = confirmation(&mut app);
    assert_eq!(required, contract_id);
    assert!(label.contains("type the contract id"), "{label}");
    type_text(&mut app, &contract_id[..4], 40);
    assert_eq!(
        press(&mut app, KeyCode::Enter, 40),
        None,
        "a partial identifier confirmed a first authorization"
    );
    type_text(&mut app, &contract_id[4..], 40);
    assert_eq!(
        press(&mut app, KeyCode::Enter, 40),
        Some(Action::StartRun(contract_id.clone()))
    );

    // The store cannot take the run, so the authorization is spent on nothing and the contract
    // is still there to be authorized.
    set_mode(&workspace.data_root, 0o500);
    session.start_run(&contract_id);
    set_mode(&workspace.data_root, 0o700);
    app.adopt(session.projection(None));
    assert!(
        app.data.run.is_none(),
        "the store took a run it could not write"
    );

    // The second authorization of that same contract is one confirmation.
    let (required, label) = confirmation(&mut app);
    assert!(
        required.is_empty(),
        "re-authorizing an unchanged contract still demanded {required}"
    );
    assert!(
        label.contains("authorized in this session"),
        "the confirmation does not say why it is lighter: {label}"
    );
    assert_eq!(
        press(&mut app, KeyCode::Enter, 40),
        Some(Action::StartRun(contract_id.clone())),
        "one confirmation did not authorize the contract"
    );

    // And it is the same authorization: the run it starts is the run that contract names.
    session.start_run(&contract_id);
    app.adopt(session.projection(None));
    assert!(
        app.data.run.is_some(),
        "the re-authorized contract started no run"
    );
    assert!(workspace.data_root.join("events.jsonl").exists());
}

/// An authorization grants a budget as well as a contract, and the budget is carried beside the
/// contract rather than inside it: amending it leaves the contract digest exactly as it was.
///
/// The scenario is the reviewer's. A draft is authorized, the budget is then amended upward, and
/// the second authorization must cost the contract id in full — the confirmation that says
/// nothing has changed since would otherwise start a run spending five hundred attempts on one
/// keystroke.
#[cfg(unix)]
#[test]
fn amending_the_budget_after_an_authorization_costs_the_identifier_again() {
    let workspace = workspace();
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));
    let contract_id = drafted_from_stated_paths(&mut session, &mut app, &workspace);
    let digest = app.data.contracts[0].contract_digest.clone();

    // Authorized once, against the budget the draft carried.
    let (required, _) = confirmation(&mut app);
    assert_eq!(required, contract_id);
    set_mode(&workspace.data_root, 0o500);
    session.start_run(&contract_id);
    set_mode(&workspace.data_root, 0o700);
    app.adopt(session.projection(None));
    let (required, _) = confirmation(&mut app);
    assert!(
        required.is_empty(),
        "the second authorization of an unchanged draft was not the lighter one"
    );

    // The draft is amended to spend five hundred times as much. The contract itself is
    // untouched — same identifier, same digest — so only a rule that names the budget can tell
    // this authorization from the one that was given.
    session.local_turn("attempts 500".to_owned());
    app.adopt(session.projection(None));
    let amended = app
        .data
        .contracts
        .first()
        .expect("the amended draft is still a contract");
    assert_eq!(amended.contract_id, contract_id);
    assert_eq!(
        amended.contract_digest, digest,
        "the amendment changed the contract, so the budget rule is not what is measured here"
    );
    assert_eq!(
        amended
            .budget
            .as_ref()
            .expect("the amended budget")
            .attempts_remaining,
        500
    );
    assert!(
        !amended.previously_authorized,
        "a draft that would spend five hundred attempts inherited an authorization given for one"
    );

    let (required, label) = confirmation(&mut app);
    assert_eq!(
        required, contract_id,
        "the amended budget was authorized without the contract id"
    );
    assert!(
        label.contains("type the contract id"),
        "the confirmation still claims nothing changed: {label}"
    );
    assert_eq!(
        press(&mut app, KeyCode::Enter, 40),
        None,
        "one keystroke authorized a run that spends five hundred attempts"
    );
}

/// A contract nobody has authorized in this session is authorized by typing its identifier,
/// whatever else the session has authorized.
#[test]
fn a_contract_this_session_never_authorized_costs_its_identifier() {
    let workspace = workspace();
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));
    let contract_id = drafted(&mut session, &mut app, &workspace);

    let (required, _) = confirmation(&mut app);
    assert_eq!(required, contract_id);
    assert!(
        app.data
            .contracts
            .first()
            .is_some_and(|contract| !contract.previously_authorized),
        "an unauthorized contract was recorded as previously authorized"
    );
}

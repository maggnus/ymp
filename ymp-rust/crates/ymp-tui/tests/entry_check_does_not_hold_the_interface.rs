//! Acceptance: validating a typed answer never freezes the dialogue.
//!
//! The answer that needs a verifier run is taken on the thread that draws; the run itself is
//! not. What the interface does with the answer is driven here exactly as the event loop drives
//! it — `begin_turn` returns the work, the work runs elsewhere, and `finish_check` takes its
//! outcome — so what is asserted is what an operator at a terminal reaches.
//!
//! Three things are required of that arrangement, and each is measured rather than assumed:
//!
//! * taking the answer returns at once, while the program it names is still running;
//! * the interface redraws while it runs, and the row that states the wait changes, so a wait
//!   cannot be mistaken for a freeze;
//! * Esc ends the wait at once, the draft it was deciding is not shown as a contract, and the
//!   outcome that arrives afterwards decides nothing.
//!
//! The negative half is the build this card started from, where the verifier ran inside the
//! event loop's own call: `taking_the_answer_returns_before_the_verifier_does` reports the take
//! itself lasting as long as the program, which for the default limit is a sixty-second hold.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use support::{press, screen};
use tempfile::TempDir;
use ymp_tui::Session;
use ymp_tui::app::Action;
use ymp_tui::state::App;

/// How long the verifier of this fixture takes to decide. Long enough that a hold would be
/// unmistakable, short enough to keep the narrow suite fast.
const VERIFIER_SECONDS: u64 = 3;

/// What a step that must not wait for the verifier is allowed to take.
const IMMEDIATE: Duration = Duration::from_millis(750);

struct Workspace {
    _root: TempDir,
    data_root: PathBuf,
    source: PathBuf,
    program: PathBuf,
    negative_control: PathBuf,
}

/// A project whose verifier decides slowly: it waits, then rejects a directory with no result.
fn workspace() -> Workspace {
    let root = TempDir::new().expect("temporary root");
    let directory = root.path().canonicalize().expect("resolve root");
    let source = directory.join("source");
    let negative_control = directory.join("negative-control");
    let data_root = directory.join("data");
    let program = directory.join("verify.sh");
    fs::create_dir_all(&source).expect("source directory");
    fs::create_dir_all(&negative_control).expect("negative control directory");
    fs::create_dir_all(&data_root).expect("data root");
    fs::write(source.join("result.txt"), b"result\n").expect("source file");
    write_program(
        &program,
        &format!("sleep {VERIFIER_SECONDS}\ntest -f \"$1/result.txt\"\n"),
    );
    Workspace {
        _root: root,
        data_root,
        source,
        program,
        negative_control,
    }
}

fn write_program(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}")).expect("write program");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).expect("make executable");
    }
}

/// State the request and the two values that make the check decidable, and return the work the
/// last line produced together with how long taking that line took.
///
/// Every line is a turn of the interface's own dialogue: the first states the work, the rest
/// amend the draft it opened, and each one supersedes the work the line before it asked for.
fn up_to_the_check(
    session: &mut Session,
    app: &mut App,
    workspace: &Workspace,
) -> (ymp_tui::app::PendingCheck, Duration) {
    for line in [
        "keep the replay path idempotent".to_owned(),
        format!("source {}", workspace.source.display()),
        format!("verifier {}", workspace.program.display()),
    ] {
        session.begin_turn(line);
    }
    let started = Instant::now();
    let pending = session
        .begin_turn(format!(
            "negative control {}",
            workspace.negative_control.display()
        ))
        .expect("the amended draft has to be demonstrated before it can be shown");
    let taken = started.elapsed();
    app.adopt(session.projection(None));
    (pending, taken)
}

#[test]
fn taking_the_answer_returns_before_the_verifier_does() {
    let workspace = workspace();
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));

    let (pending, taken) = up_to_the_check(&mut session, &mut app, &workspace);
    assert!(
        taken < IMMEDIATE,
        "taking the answer waited {taken:?} for a program that takes {VERIFIER_SECONDS} s"
    );
    assert!(
        pending.waiting_for().contains("verify.sh"),
        "the wait does not name the program it is waiting for: {}",
        pending.waiting_for()
    );

    // The interface states the wait and keeps drawing under it: the row advances between
    // heartbeats, which is what tells a wait apart from a freeze.
    let mut frames = Vec::new();
    for _ in 0..3 {
        frames.push(screen(&app, 120, 40));
        app.working_ticks += 1;
    }
    assert!(
        frames[0].contains("verify.sh") && frames[0].contains("Esc cancels"),
        "the interface does not state what it is waiting for:\n{}",
        frames[0]
    );
    assert_ne!(
        frames[0], frames[1],
        "the interface drew the same frame while it was waiting"
    );

    // Scrolling still reaches the view while the program runs.
    let before = app.follow;
    press(&mut app, KeyCode::Up, 40);
    assert_ne!(app.follow, before, "the view did not answer a key");

    // And the outcome, when it arrives, is what decides the answer.
    let outcome = pending.run();
    session.finish_check(outcome);
    app.adopt(session.projection(None));
    assert!(
        app.data.working.is_none(),
        "the interface is still waiting after the check decided"
    );
    let contract = app
        .data
        .contracts
        .first()
        .expect("the demonstrated verifier reached a drafted contract");
    assert!(contract.verifier.is_some());
    assert!(app.data.run.is_none(), "drafting a contract started a run");
}

#[test]
fn esc_cancels_a_running_check_and_returns_to_the_draft() {
    let workspace = workspace();
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));
    let (pending, _) = up_to_the_check(&mut session, &mut app, &workspace);

    // The check is running on its own thread, as the event loop runs it.
    let worker = std::thread::spawn(move || pending.run());

    let started = Instant::now();
    let action = press(&mut app, KeyCode::Esc, 40);
    assert_eq!(action, Some(Action::CancelCheck));
    session.cancel_check();
    app.adopt(session.projection(None));
    let cancelled = started.elapsed();
    assert!(
        cancelled < IMMEDIATE,
        "cancelling waited {cancelled:?} for the program to finish"
    );

    let rendered = screen(&app, 120, 40);
    assert!(rendered.contains("the check was cancelled"), "{rendered}");
    assert!(
        app.data.working.is_none(),
        "the interface still states a wait it was told to end"
    );
    assert!(
        app.data.contracts.is_empty(),
        "a cancelled check left a contract behind"
    );

    // The abandoned run decides nothing when it finally returns.
    session.finish_check(worker.join().expect("the abandoned check ended on its own"));
    app.adopt(session.projection(None));
    assert!(
        app.data.contracts.is_empty(),
        "a cancelled check drafted a contract"
    );
    assert!(
        !workspace.data_root.join("events.jsonl").exists(),
        "a cancelled check wrote a journal"
    );
}

/// A verifier that accepts the negative control is refused, and the refusal arrives through the
/// same path: nothing about the answer is settled until the outcome comes back.
#[test]
fn a_refusal_from_the_check_reaches_the_transcript_and_ends_the_draft() {
    let workspace = workspace();
    write_program(&workspace.program, "sleep 1\nexit 0\n");
    let mut session = Session::open(&workspace.data_root, &[]);
    let mut app = App::new(session.projection(None));
    let (pending, _) = up_to_the_check(&mut session, &mut app, &workspace);

    session.finish_check(pending.run());
    app.adopt(session.projection(None));

    // The transcript is wrapped to the width it is drawn at, so the refusal is looked for with
    // its line breaks and indentation collapsed.
    let rendered = screen(&app, 120, 40);
    let flattened = rendered.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flattened.contains("accepted the negative control"),
        "{rendered}"
    );
    assert!(
        app.data.contracts.is_empty(),
        "a verifier that decides nothing reached a contract"
    );
    assert!(app.data.working.is_none());
}

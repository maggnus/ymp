//! Acceptance: starting the product against a real store renders the transcript-first path,
//! and the one irreversible command it offers is committed by the application, not simulated.
//!
//! This is the live half of the state-binding claim: the cancellation the operator confirms
//! becomes a journal event, and the outcome on screen is the outcome the domain recorded.

mod support;

use crossterm::event::KeyCode;
use support::{SIZES, buffer, open_command, press, screen, type_text};
use ymp_application::Application;
use ymp_domain::{Budget, RunStatus};
use ymp_tui::state::{App, Modal};
use ymp_tui::{Session, ui};

fn store() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().expect("temporary store");
    let root = directory.path().join("data");
    std::fs::create_dir_all(&root).expect("data root");
    let application =
        Application::create(&root, "live-run", Budget::new(2, 1)).expect("committed run");
    drop(application);
    (directory, root)
}

fn render(app: &App, width: u16, height: u16) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
        .expect("test terminal");
    terminal
        .draw(|frame| ui::render(frame, app, &ymp_tui::theme::UNICODE))
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_store_with_no_run_opens_on_the_transcript_and_states_the_absence() {
    let directory = tempfile::tempdir().expect("temporary store");
    let root = directory.path().join("empty");
    std::fs::create_dir_all(&root).expect("data root");

    let session = Session::open(&root, &[]);
    let app = App::new(session.projection(None));
    for (width, height) in SIZES {
        let rendered = render(&app, width, height);
        // The absence is stated once, by the status line and the header, and the transcript
        // spends its opening on the logo, the basics and the invitation instead.
        assert!(rendered.contains("idle · no run"), "{rendered}");
        assert!(rendered.contains("no contract · no run"), "{rendered}");
        assert!(rendered.contains("state your request below"), "{rendered}");
        assert!(rendered.contains("/commands"), "{rendered}");
        assert!(
            !rendered.contains("no run recorded"),
            "the transcript restated the absence the status line already carries:\n{rendered}"
        );
        assert!(
            !rendered.contains("no contract drafted"),
            "the transcript restated an absence with nothing to do about it:\n{rendered}"
        );
    }
}

#[test]
fn a_committed_run_is_read_from_the_journal_at_startup() {
    let (_directory, root) = store();
    let session = Session::open(&root, &[]);
    let app = App::new(session.projection(None));

    let facts = app.data.run.as_ref().expect("the run was read");
    assert_eq!(facts.run_id, "live-run");
    assert_eq!(facts.status, RunStatus::Running);

    let rendered = render(&app, 120, 40);
    assert!(rendered.contains("live-run [*] running"), "{rendered}");
    assert!(rendered.contains("run live-run started"), "{rendered}");
}

#[test]
fn confirming_the_cancellation_records_it_and_the_screen_reports_the_recorded_outcome() {
    let (_directory, root) = store();
    let mut session = Session::open(&root, &[]);
    let mut app = App::new(session.projection(None));

    open_command(&mut app, "cancel live-run", 40);
    assert!(matches!(app.modal, Modal::Confirm(_)));
    type_text(&mut app, "live-run", 40);
    let action = press(&mut app, KeyCode::Enter, 40);
    assert_eq!(action, Some(ymp_tui::app::Action::CancelRun));

    session.cancel_run();
    app.adopt(session.projection(None));

    let facts = app.data.run.as_ref().expect("the run is still readable");
    assert_eq!(facts.status, RunStatus::Cancelled);

    for (width, height) in SIZES {
        let rendered = render(&app, width, height);
        assert!(rendered.contains("[x] cancelled"), "{rendered}");
        assert!(
            !rendered.contains("infrastructure_error"),
            "an operator cancellation was reported as a machinery failure:\n{rendered}"
        );
    }

    // The outcome is durable: a second reader of the same store sees the same terminal state.
    drop(session);
    let reopened = Session::open(&root, &[]);
    assert_eq!(
        reopened
            .projection(None)
            .run
            .expect("the run is still recorded")
            .status,
        RunStatus::Cancelled
    );
}

#[test]
fn the_live_screen_never_exceeds_the_terminal_it_was_given() {
    let (_directory, root) = store();
    let session = Session::open(&root, &[]);
    let app = App::new(session.projection(None));
    for (width, height) in SIZES {
        for line in buffer(&app, width, height) {
            assert_eq!(line.chars().count(), width as usize, "{line:?}");
        }
        assert_eq!(screen(&app, width, height).lines().count(), height as usize);
    }
}

//! Acceptance: while a provider is being measured, every surface that draws it says so, a second
//! ask is folded into the running measurement, and the keyboard keeps reaching the interface.
//!
//! Nothing here starts an engine. The act that asks for a measurement returns the work rather than
//! doing it, so a check can hold that work unrun and draw the exact state an operator stands in
//! while the engines are answering — at both declared terminal sizes and with no process, no
//! sleep and no clock in the assertions.
//!
//! * `/providers`, the provider's own card and `/runtimes` state the measurement; the row that is
//!   being measured states that instead of the value the measurement is replacing.
//! * A second Enable or Refresh while one is running starts nothing: it is folded into the running
//!   measurement, and the surfaces say how many asks that measurement answers.
//! * The row that states the wait offers no key to end it, because no key does; every other key
//!   still reaches the surface the operator is standing on.

mod support;

use std::fs;
use std::path::PathBuf;

use crossterm::event::KeyCode;
use support::{SIZES, buffer, open_command, press, screen};
use tempfile::TempDir;
use ymp_runtime_registry::{ProviderFamily, RegistryAddress};
use ymp_tui::Session;
use ymp_tui::app::{Action, PendingMeasurement};
use ymp_tui::state::{App, PageKind, Surface};

/// A product root of this check's own. Nothing here reads or writes the owner's own root.
struct Root {
    _directory: TempDir,
    root: PathBuf,
}

impl Root {
    fn new() -> Self {
        let directory = TempDir::new().expect("temporary directory");
        let root = directory.path().join("root");
        fs::create_dir_all(&root).expect("root directory");
        Self {
            _directory: directory,
            root,
        }
    }

    /// A session over this root with the engine records read, which is the state the event loop
    /// starts every session in: the records are read and nothing is measured.
    fn session(&self) -> Session {
        let mut session = Session::open_under_root(&self.root, &self.root.join("runs/0001"), &[]);
        session.set_runtimes(ymp_tui::runtimes::read_all(&RegistryAddress::Root(
            self.root.clone(),
        )));
        session
    }
}

/// Enable the account and hold the measurement it asked for unrun, which is the state the
/// interface is in for as long as the engines take to answer.
fn measuring(session: &mut Session, app: &mut App) -> PendingMeasurement {
    let pending = session
        .set_provider_enabled(ProviderFamily::Anthropic, true, None)
        .expect("enabling an account asks for a measurement of it");
    assert!(session.is_measuring(), "the session states no measurement");
    app.adopt(session.projection_for(None, app.provider_index));
    pending
}

/// The three surfaces that draw an account state that it is being measured, at both sizes.
///
/// The check that must fail: draw the recorded state while the measurement that replaces it is
/// running, and each of these surfaces reports as current a value the operator has already asked
/// to have replaced.
#[test]
fn every_surface_that_draws_an_account_states_that_it_is_being_measured() {
    for (width, height) in SIZES {
        let root = Root::new();
        let mut session = root.session();
        let mut app = App::new(session.projection(None));
        let _pending = measuring(&mut session, &mut app);

        open_command(&mut app, "providers", height);
        let shown = screen(&app, width, height);
        assert!(
            shown.contains("measuring…"),
            "the provider table does not state the measurement at {width}x{height}:\n{shown}"
        );
        assert!(
            shown.contains("the engines that reach it are being started"),
            "the table does not say what is running at {width}x{height}:\n{shown}"
        );

        press(&mut app, KeyCode::Enter, height);
        app.adopt(session.projection_for(None, app.provider_index));
        assert_eq!(app.surface, Surface::Page(PageKind::Provider));
        let shown = screen(&app, width, height);
        assert!(
            shown.contains("measuring…"),
            "the provider card does not state the measurement at {width}x{height}:\n{shown}"
        );

        press(&mut app, KeyCode::Esc, height);
        open_command(&mut app, "runtimes", height);
        assert_eq!(app.surface, Surface::Page(PageKind::Runtimes));
        let shown = screen(&app, width, height);
        assert!(
            shown.contains("measuring…"),
            "the engines page does not state the measurement at {width}x{height}:\n{shown}"
        );

        // The transcript states it too, and offers no key to end it: nothing ends a measurement,
        // so a key named here would be a key that changes nothing.
        press(&mut app, KeyCode::Esc, height);
        let shown = screen(&app, width, height);
        assert!(
            shown.contains("measuring anthropic"),
            "the conversation does not state what is running at {width}x{height}:\n{shown}"
        );
        assert!(
            !shown.contains("Esc cancels"),
            "the row offers a key that ends nothing at {width}x{height}:\n{shown}"
        );
    }
}

/// The surfaces of a measurement are drawn identically twice and never exceed the terminal.
#[test]
fn the_measuring_surfaces_are_deterministic_and_never_exceed_the_terminal() {
    for (width, height) in SIZES {
        let root = Root::new();
        let mut session = root.session();
        let mut app = App::new(session.projection(None));
        let _pending = measuring(&mut session, &mut app);

        for page in ["providers", "runtimes"] {
            let mut app = app.clone();
            open_command(&mut app, page, height);
            let first = buffer(&app, width, height);
            assert_eq!(first, buffer(&app, width, height), "{page} drew twice");
            assert_eq!(first.len(), height as usize);
            for line in &first {
                assert_eq!(
                    line.chars().count(),
                    width as usize,
                    "{page} drifted in width while measuring: {line:?}"
                );
            }
        }
    }
}

/// A second ask while a measurement is running starts nothing, and the surfaces state which
/// measurement answered it.
///
/// The check that must fail: start a second probe for the second ask, and holding the key down on
/// the provider card starts one engine invocation per repeat.
#[test]
fn a_second_ask_is_folded_into_the_running_measurement_and_the_surfaces_say_so() {
    let root = Root::new();
    let mut session = root.session();
    let mut app = App::new(session.projection(None));
    let _pending = measuring(&mut session, &mut app);

    assert!(
        session
            .refresh_provider_models(ProviderFamily::Anthropic)
            .is_none(),
        "a second ask started a second measurement"
    );
    assert!(
        session
            .set_provider_enabled(ProviderFamily::OpenAi, true, None)
            .is_none(),
        "enabling a second account while a measurement runs started another one"
    );
    assert_eq!(
        session
            .measuring()
            .expect("one measurement is running")
            .folded,
        2,
        "the running measurement does not carry the asks it answered"
    );

    app.adopt(session.projection_for(None, app.provider_index));
    open_command(&mut app, "providers", 40);
    let shown = screen(&app, 120, 40);
    assert!(
        shown.contains("2 further ask(s) arrived while this measurement was running"),
        "the table does not state that the asks were folded in:\n{shown}"
    );
    assert!(
        shown.contains("nothing further was started"),
        "the table does not say that the second ask started nothing:\n{shown}"
    );
}

/// The keyboard reaches the interface while a measurement runs: keys are answered as they arrive
/// rather than held and replayed once the engines answer.
#[test]
fn keys_reach_the_surfaces_while_a_measurement_is_running() {
    let root = Root::new();
    let mut session = root.session();
    let mut app = App::new(session.projection(None));
    let _pending = measuring(&mut session, &mut app);
    open_command(&mut app, "providers", 40);

    let selected = app.selection_of(PageKind::Providers);
    press(&mut app, KeyCode::Down, 40);
    assert_ne!(
        app.selection_of(PageKind::Providers),
        selected,
        "the table did not answer a key while a measurement was running"
    );

    // Esc is the key the surface binds it to and not a way out of the wait, because there is no
    // way out of this wait to offer.
    assert_eq!(press(&mut app, KeyCode::Esc, 40), None);
    assert_eq!(
        app.surface,
        Surface::Transcript,
        "Esc did not leave the page"
    );
    assert_ne!(
        press(&mut app, KeyCode::Esc, 40),
        Some(Action::CancelCheck),
        "Esc claimed to cancel a measurement nothing can cancel"
    );

    // A line still types while the engines answer.
    press(&mut app, KeyCode::Char('h'), 40);
    assert_eq!(app.prompt.buffer, "h", "the prompt did not take a key");
}

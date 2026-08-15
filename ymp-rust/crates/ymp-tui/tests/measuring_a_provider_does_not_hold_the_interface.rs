//! Acceptance: enabling a provider does not freeze the interface.
//!
//! The engine that reaches the account is a real program here — a fixture planted on this check's
//! own search path that takes [`ENGINE_SECONDS`] to answer — so what is measured is the arrangement
//! an operator meets and not a stand-in for it. The interface is driven exactly as the event loop
//! drives it: the act returns the measurement, the measurement runs on a worker, and its outcome is
//! taken through `finish_measurement` when it arrives.
//!
//! Four things are required of that arrangement, and each is measured rather than assumed:
//!
//! * taking the decision returns at once, while the engine it started is still running;
//! * the surfaces state that the account is being measured, and the interface redraws under it, so
//!   a wait cannot be mistaken for a freeze;
//! * a key pressed while the engine is running is answered then, not held and replayed after;
//! * the observation lands when the engine answers, and every surface is re-read from the records.
//!
//! The negative half is the build this card started from, where the probe ran inside the act's own
//! call: `taking_the_decision_returns_before_the_engine_does` reports the take itself lasting as
//! long as the engine, which on the owner's host was measured at 29–42 s of frozen interface.

mod support;

use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use support::{press, screen};
use ymp_runtime_registry::{ProviderFamily, RegistryAddress};
use ymp_tui::Session;
use ymp_tui::app::{MeasurementOutcome, Selected};
use ymp_tui::state::{App, PageKind};

/// How long the planted engine takes to answer `--version`. Long enough that a held interface
/// would be unmistakable, short enough to keep the narrow suite fast.
const ENGINE_SECONDS: u64 = 2;

/// What a step that must not wait for the engine is allowed to take.
const IMMEDIATE: Duration = Duration::from_millis(750);

/// How long the outcome is waited for once the interface has been driven. It is far above the
/// engine's own time: what it bounds is a check that hangs, not the measurement.
const OUTCOME_WAIT: Duration = Duration::from_secs(60);

fn plant(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}")).expect("write the planted engine");
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make the planted engine executable");
}

/// Where the operator is standing, as the event loop reports it to the session: on the row of the
/// provider table whose properties are open, and on no other.
fn standing_on(app: &App) -> Selected {
    Selected {
        provider: app.provider_index,
        ..Selected::none()
    }
}

#[test]
fn taking_the_decision_returns_before_the_engine_does() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let base = temporary.path();
    let root = base.join("root");
    let home = base.join("home");
    let search_path = base.join("bin");
    let started_log = base.join("started.log");
    for directory in [&root, &home, &search_path] {
        fs::create_dir_all(directory).expect("fixture directory");
    }
    // The engine that reaches anthropic, planted as a program that records being started and then
    // takes its time. The release it prints is not one this build admits, so the measurement ends
    // at the version probe: what is under test is the wait, not what the engine serves.
    plant(
        &search_path.join("claude"),
        &format!(
            "printf '%s %s\\n' claude \"$*\" >> \"{}\"\nsleep {ENGINE_SECONDS}\nprintf '%s\\n' \
             '1.2.3 (claude)'\n",
            started_log.display()
        ),
    );

    // This binary holds one test, so the search path and home it sets are read by nothing else.
    // The home is this check's own, so nothing here reads the owner's credentials or their root.
    let path = format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", search_path.display());
    unsafe {
        std::env::set_var("PATH", &path);
        std::env::set_var("HOME", &home);
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }

    let address = RegistryAddress::Root(root.clone());
    let mut session = Session::open_under_root(&root, &root.join("runs/0001"), &[]);
    session.set_runtimes(ymp_tui::runtimes::read_all(&address));
    let mut app = App::new(session.projection(None));

    // The operator enables the account, which is the act that measures it.
    let taking = Instant::now();
    let pending = session
        .set_provider_enabled(ProviderFamily::Anthropic, true, None)
        .expect("enabling an account asks for a measurement of it");
    let taken = taking.elapsed();
    assert!(
        taken < IMMEDIATE,
        "taking the decision waited {taken:?} for an engine that takes {ENGINE_SECONDS} s"
    );

    // The measurement runs on a worker and reports back as an event, exactly as the event loop
    // arranges it.
    let (deliver, outcomes) = mpsc::channel::<MeasurementOutcome>();
    session.start_measurement(pending, move |outcome| {
        let _ = deliver.send(outcome);
    });
    app.adopt(session.projection_for(standing_on(&app)));
    app.surface = ymp_tui::state::Surface::Page(PageKind::Providers);

    // The engine is running. Until it has actually started there is nothing to be held by, so the
    // interface is driven only once this check knows a program is out there.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !started_log.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        started_log.exists(),
        "the planted engine was never started, so nothing was measured"
    );
    assert!(
        outcomes.try_recv().is_err(),
        "the engine answered before the interface was driven; raise ENGINE_SECONDS"
    );

    // The interface states what is running and keeps drawing under it: the mark advances between
    // heartbeats, which is what tells a wait apart from a freeze.
    let drawing = Instant::now();
    let mut frames = Vec::new();
    for _ in 0..3 {
        frames.push(screen(&app, 120, 40));
        app.working_ticks += 1;
    }
    assert!(
        frames[0].contains("measuring…"),
        "the table does not state that the account is being measured:\n{}",
        frames[0]
    );
    assert_ne!(
        frames[0], frames[1],
        "the interface drew the same frame while the engine was running"
    );

    // A key pressed while the engine is running is answered while it is running.
    let selected = app.selection_of(PageKind::Providers);
    press(&mut app, KeyCode::Down, 40);
    assert_ne!(
        app.selection_of(PageKind::Providers),
        selected,
        "the interface did not answer a key while the engine was running"
    );
    let interacting = drawing.elapsed();
    assert!(
        interacting < IMMEDIATE,
        "drawing and answering a key took {interacting:?} while the engine was running"
    );
    assert!(
        outcomes.try_recv().is_err(),
        "the engine answered while the interface was being driven; raise ENGINE_SECONDS"
    );

    // And the outcome, when it arrives, is what records the observation and re-reads the records.
    let outcome = outcomes
        .recv_timeout(OUTCOME_WAIT)
        .expect("the measurement returned its outcome through the channel");
    session.finish_measurement(outcome);
    app.adopt(session.projection_for(standing_on(&app)));
    assert!(
        !session.is_measuring(),
        "the interface still states a measurement that has landed"
    );

    let record = fs::read_to_string(root.join("providers").join("anthropic.json"))
        .expect("the observation was recorded");
    assert!(
        record.contains("observed_at_ms"),
        "the observation carries no moment: {record}"
    );
    let shown = screen(&app, 120, 40);
    assert!(
        !shown.contains("measuring…"),
        "the table still states a measurement that has landed:\n{shown}"
    );
    assert!(
        shown.contains("ago"),
        "the row states no age for an observation just taken:\n{shown}"
    );
    let started: Vec<String> = fs::read_to_string(&started_log)
        .expect("the invocation log")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        started.len(),
        1,
        "the account was measured other than once: {started:?}"
    );

    // The observation is where the pools are resolved against the catalog it left, and this is the
    // path the interface takes to one: the outcome came back from a worker, and the reply states
    // what the pools hold after that resolution rather than before it. The planted engine serves
    // no model, so the catalog offers nothing and this root holds no pool — which is the state
    // P3 states, not a resolution that failed to run.
    let stated = session
        .projection(None)
        .entries
        .iter()
        .map(|entry| format!("{entry:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stated.contains("no pool stands under this root"),
        "the reply the measurement landed with does not state what the pools hold:\n{stated}"
    );
}

//! Acceptance: quitting while a provider is being measured leaves no engine process behind.
//!
//! A measurement runs on a worker, and that worker is inside the engine it started. A quit that
//! simply returned would end the interface while a program it started was still running — the
//! condition W1-APP-02k rules out for a managed run, applied here to the one other place this
//! product starts a process of its own.
//!
//! The evidence is the operating system's and not the product's: the planted engine writes its own
//! process identifier before it sleeps, and what is asserted after the quit is that no process
//! answers to it. The bound is the one a managed run's shutdown already carries.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ymp_runtime_registry::{ProviderFamily, RegistryAddress};
use ymp_runtime_supervisor::CONTROLLER_SHUTDOWN_LIMIT;
use ymp_tui::Session;
use ymp_tui::app::MeasurementShutdown;

/// How long the planted engine sleeps before answering. Under the shutdown bound, so the quit
/// waits it out rather than giving up on it; long enough that the quit really does arrive while
/// the engine is running.
const ENGINE_SECONDS: u64 = 2;

fn plant(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}")).expect("write the planted engine");
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make the planted engine executable");
}

/// Whether a process answers to this identifier, asked of the operating system.
fn is_running(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .output()
        .expect("ask the operating system about the process")
        .status
        .success()
}

#[test]
fn quitting_while_an_engine_is_being_measured_leaves_no_engine_process() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let base = temporary.path();
    let root = base.join("root");
    let home = base.join("home");
    let search_path = base.join("bin");
    let pid_file = base.join("engine.pid");
    for directory in [&root, &home, &search_path] {
        fs::create_dir_all(directory).expect("fixture directory");
    }
    plant(
        &search_path.join("claude"),
        &format!(
            "printf '%s\\n' \"$$\" > \"{}\"\nsleep {ENGINE_SECONDS}\nprintf '%s\\n' '1.2.3 \
             (claude)'\n",
            pid_file.display()
        ),
    );

    // This binary holds one test, so the search path and home it sets are read by nothing else.
    let path = format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", search_path.display());
    unsafe {
        std::env::set_var("PATH", &path);
        std::env::set_var("HOME", &home);
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }

    let address = RegistryAddress::Root(root.clone());
    let mut session = Session::open_under_root(&root, &root.join("runs/0001"), &[]);
    session.set_runtimes(ymp_tui::runtimes::read_all(&address));
    let pending = session
        .set_provider_enabled(ProviderFamily::Anthropic, true, None)
        .expect("enabling an account asks for a measurement of it");
    session.start_measurement(pending, |_| {});

    // The engine is running and has said which process it is.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !pid_file.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let pid = fs::read_to_string(&pid_file)
        .expect("the planted engine recorded its process")
        .trim()
        .to_owned();
    assert!(
        is_running(&pid),
        "the engine was already gone, so this check would pass without a quit ending anything"
    );
    assert!(session.is_measuring(), "the session states no measurement");

    // The operator quits. This is what the event loop does on its way out.
    let quitting = Instant::now();
    let ending = session.end_measurement();
    let waited = quitting.elapsed();

    assert_eq!(
        ending,
        MeasurementShutdown::Ended,
        "the quit did not establish that the measurement had ended"
    );
    assert!(
        waited < CONTROLLER_SHUTDOWN_LIMIT,
        "the quit waited {waited:?}, past the {CONTROLLER_SHUTDOWN_LIMIT:?} a shutdown is bounded \
         by"
    );
    assert!(
        !is_running(&pid),
        "process {pid} — the engine this session started — outlived the session that started it"
    );
    assert!(
        !session.is_measuring(),
        "the session still states a measurement it has ended"
    );
    assert_eq!(
        session.end_measurement(),
        MeasurementShutdown::Nothing,
        "a second quit reported an ending for a measurement that was already over"
    );
}

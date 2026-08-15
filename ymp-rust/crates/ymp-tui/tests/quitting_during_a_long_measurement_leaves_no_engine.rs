//! Acceptance: quitting while a provider is being measured leaves no engine process behind, even
//! when the engine answers later than the wait for it lasts.
//!
//! Its sibling check plants an engine that answers inside that wait, so the quit ends by joining a
//! worker that finished of its own accord. This one plants an engine that answers long after it,
//! which is the case the wait alone cannot settle: past the bound the worker is still inside the
//! process it started, and a quit that only stopped waiting would return while that process kept
//! running. What must hold instead is the rule W1-APP-02k established for a managed run — no
//! process a run started outlives it — with the shutdown still bounded.
//!
//! The evidence is the operating system's and not the product's: the planted engine writes the
//! identifier of its own process and of the one it starts beneath it before either sleeps, and what
//! is asserted after the quit is that no process answers to either. The second identifier is what
//! makes this a check on the process tree: the engine's own child is orphaned the moment the engine
//! ends, so a shutdown that reached only what it started directly would leave it running.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ymp_runtime_registry::{ProviderFamily, RegistryAddress};
use ymp_tui::Session;
use ymp_tui::app::{MEASUREMENT_SHUTDOWN_LIMIT, MeasurementShutdown};

/// How long the planted engine sleeps before answering. Far past the wait a quit gives a
/// measurement, so the quit reaches the point where waiting is over and the engine is still there.
const ENGINE_SECONDS: u64 = 20;

fn plant(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}")).expect("write the planted engine");
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make the planted engine executable");
}

/// Whether a program is still running under this identifier, asked of the operating system.
///
/// The question is put to the process table rather than to a signal, because the two answers a
/// signal cannot tell apart matter here. A process that has ended and is waiting to be collected by
/// its parent still accepts a signal of zero, and it is not a program that is still running: the
/// engine this check plants ends beneath a worker that collects it moments later, so a signal would
/// report the defect this check exists to find at a moment when there is none.
fn is_running(pid: &str) -> bool {
    let state = Command::new("/bin/ps")
        .args(["-p", pid, "-o", "state="])
        .output()
        .expect("ask the operating system about the process");
    let state = String::from_utf8_lossy(&state.stdout).trim().to_owned();
    !state.is_empty() && !state.starts_with('Z')
}

/// The identifier a file holds, once it holds one. The engine creates the file and writes to it as
/// two acts, so a reader that stopped at the name would read an empty file and ask the operating
/// system about nothing.
fn recorded(file: &Path) -> String {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let held = fs::read_to_string(file)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if !held.is_empty() || Instant::now() >= deadline {
            assert!(!held.is_empty(), "the planted engine recorded no process");
            return held;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// What the process table holds about this identifier, so a survivor is reported as the operating
/// system describes it rather than as a bare number.
fn described(pid: &str) -> String {
    let output = Command::new("/bin/ps")
        .args(["-p", pid, "-o", "pid=,ppid=,pgid=,state=,command="])
        .output()
        .expect("read the process table");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn quitting_while_a_slow_engine_is_being_measured_leaves_no_engine_process() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let base = temporary.path();
    let root = base.join("root");
    let home = base.join("home");
    let search_path = base.join("bin");
    let pid_file = base.join("engine.pid");
    let beneath_file = base.join("beneath.pid");
    for directory in [&root, &home, &search_path] {
        fs::create_dir_all(directory).expect("fixture directory");
    }
    plant(
        &search_path.join("claude"),
        &format!(
            "printf '%s\\n' \"$$\" > \"{}\"\nsleep {ENGINE_SECONDS} &\nprintf '%s\\n' \"$!\" > \
             \"{}\"\nwait\nprintf '%s\\n' '1.2.3 (claude)'\n",
            pid_file.display(),
            beneath_file.display()
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

    // The engine is running and has said which processes it is made of. A file that exists is not
    // yet a file that has been written, so what is waited for is content and not the name.
    let pid = recorded(&pid_file);
    let beneath = recorded(&beneath_file);
    for process in [&pid, &beneath] {
        assert!(
            is_running(process),
            "process {process} of the engine was already gone, so this check would pass without a \
             quit ending anything"
        );
    }
    assert!(session.is_measuring(), "the session states no measurement");

    // The operator quits. This is what the event loop does on its way out.
    let quitting = Instant::now();
    let ending = session.end_measurement();
    let waited = quitting.elapsed();

    for process in [&pid, &beneath] {
        assert!(
            !is_running(process),
            "process {process} — of the engine this session started — outlived the session that \
             started it: {}",
            described(process)
        );
    }
    assert!(
        matches!(ending, MeasurementShutdown::Terminated(_)),
        "the quit did not state that it ended the engines the measurement had left running: \
         {ending:?}"
    );
    assert!(
        waited < MEASUREMENT_SHUTDOWN_LIMIT,
        "the quit waited {waited:?}, past the {MEASUREMENT_SHUTDOWN_LIMIT:?} a shutdown during a \
         measurement is bounded by"
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

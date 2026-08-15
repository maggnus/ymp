//! Acceptance: a quit ends the engines of a measurement even where ending one of them lets the next
//! one start.
//!
//! A measurement is not one process. The driver reads the models a build serves in batches: it
//! starts a group of them, waits, and starts the next group when that one is done. Ending what a
//! single reading of the process table found would therefore end one batch and release the next,
//! and the quit would return having read its own snapshot as empty while the measurement carried on
//! starting processes behind it.
//!
//! The planted engine is that shape and nothing more: it starts one process, waits for it, and
//! starts another when it ends. It also declines the request to end, which is what a program is
//! entitled to do with that request and what leaves the succession running long enough to be
//! observed. Every process it ever starts writes its own identifier down, and what is asserted
//! after the quit is that none of them — nor the engine — is still running.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ymp_runtime_registry::{ProviderFamily, RegistryAddress};
use ymp_tui::Session;
use ymp_tui::app::{MEASUREMENT_SHUTDOWN_LIMIT, MeasurementShutdown};

/// How long each process the engine starts sleeps. Far past the wait a quit gives a measurement, so
/// none of them ends of its own accord within the shutdown.
const STEP_SECONDS: u64 = 20;

fn plant(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}")).expect("write the planted engine");
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make the planted engine executable");
}

/// Whether a program is still running under this identifier, asked of the process table rather than
/// of a signal: a process that has ended and is waiting to be collected by its parent still accepts
/// a signal of zero, and it is not a program that is still running.
fn is_running(pid: &str) -> bool {
    let state = Command::new("/bin/ps")
        .args(["-p", pid, "-o", "state="])
        .output()
        .expect("ask the operating system about the process");
    let state = String::from_utf8_lossy(&state.stdout).trim().to_owned();
    !state.is_empty() && !state.starts_with('Z')
}

fn described(pid: &str) -> String {
    let output = Command::new("/bin/ps")
        .args(["-p", pid, "-o", "pid=,ppid=,pgid=,state=,command="])
        .output()
        .expect("read the process table");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The identifiers a file holds, once it holds at least one. A file that exists is not yet a file
/// that has been written, so the check waits for content rather than for the name.
fn recorded(file: &Path, within: Duration) -> Vec<String> {
    let deadline = Instant::now() + within;
    loop {
        let held: Vec<String> = fs::read_to_string(file)
            .unwrap_or_default()
            .lines()
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect();
        if !held.is_empty() || Instant::now() >= deadline {
            return held;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn quitting_ends_the_engine_and_everything_it_starts_while_it_is_ending() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let base = temporary.path();
    let root = base.join("root");
    let home = base.join("home");
    let search_path = base.join("bin");
    let pid_file = base.join("engine.pid");
    let started_file = base.join("started.pid");
    for directory in [&root, &home, &search_path] {
        fs::create_dir_all(directory).expect("fixture directory");
    }
    plant(
        &search_path.join("claude"),
        &format!(
            // The request to end is answered with a command that does nothing, rather than by
            // ignoring the signal: a disposition of "ignore" is inherited by everything the engine
            // starts, and the processes of the succession would then decline the request too, which
            // is not the shape this check is about.
            "printf '%s\\n' \"$$\" > \"{}\"\ntrap ':' TERM\nwhile true\ndo\n  sleep \
             {STEP_SECONDS} &\n  printf '%s\\n' \"$!\" >> \"{}\"\n  wait\ndone\n",
            pid_file.display(),
            started_file.display()
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

    // The engine is running and has started the first of its processes.
    let waiting = Duration::from_secs(30);
    let engine = recorded(&pid_file, waiting)
        .pop()
        .expect("the planted engine recorded its process");
    assert!(
        !recorded(&started_file, waiting).is_empty(),
        "the planted engine started nothing, so this check would pass without a quit reaching a \
         succession that never happened"
    );
    assert!(
        is_running(&engine),
        "the engine was already gone, so this check would pass without a quit ending anything"
    );
    assert!(session.is_measuring(), "the session states no measurement");

    // The operator quits. This is what the event loop does on its way out.
    let quitting = Instant::now();
    let ending = session.end_measurement();
    let waited = quitting.elapsed();

    let started = recorded(&started_file, Duration::ZERO);
    assert!(
        started.len() > 1,
        "the engine started no process after the quit began ending it, so the succession this \
         check exists for was never put to the shutdown: {started:?}"
    );
    for process in std::iter::once(&engine).chain(started.iter()) {
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
}

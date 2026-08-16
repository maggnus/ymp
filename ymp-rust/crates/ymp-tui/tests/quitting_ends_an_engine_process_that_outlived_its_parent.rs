//! Acceptance: a quit ends a process of the measurement that outlives the process which started it.
//!
//! A quit ends the engines by reading the process table, and what that reading can attribute rests
//! on links the operating system keeps only while both ends are alive. A process whose parent dies
//! is handed to the first process on the machine, and from that moment the table says nothing about
//! where it came from. A shutdown that worked out afresh, on each reading, which processes are its
//! own would therefore lose exactly the process that outlives the one it was started by — and,
//! finding nothing left, report an ending it had not established.
//!
//! The planted engine is that shape: it starts a process that declines the request to end, and ends
//! itself on that request. What is asserted after the quit is that the process which outlived it is
//! gone, and that the quit did not state an ending while it was still there.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ymp_runtime_registry::{ProviderFamily, RegistryAddress};
use ymp_tui::Session;
use ymp_tui::app::{MEASUREMENT_SHUTDOWN_LIMIT, MeasurementShutdown};

/// How long the process that outlives the engine sleeps. Far past the wait a quit gives a
/// measurement, so it ends because the shutdown ended it and for no other reason.
const OUTLIVING_SECONDS: u64 = 20;

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

/// The identifier a file holds, once it holds one. A file that exists is not yet a file that has
/// been written, so what is waited for is content and not the name.
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

#[test]
fn quitting_ends_a_process_of_the_measurement_that_outlives_the_engine() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let base = temporary.path();
    let root = base.join("root");
    let home = base.join("home");
    let search_path = base.join("bin");
    let pid_file = base.join("engine.pid");
    let outliving_file = base.join("outliving.pid");
    for directory in [&root, &home, &search_path] {
        fs::create_dir_all(directory).expect("fixture directory");
    }
    plant(
        &search_path.join("claude"),
        // The engine starts one process that declines the request to end and outlives it. The
        // engine itself answers that request in the ordinary way, so the moment it is asked to end
        // is the moment the operating system stops saying where the other process came from.
        //
        // That process is given output of its own. Holding the engine's would keep the driver
        // reading, and the shutdown would then be stopped by a worker that never came back rather
        // than by the process it failed to reach — which is the same defect reported as a different
        // one.
        &format!(
            "printf '%s\\n' \"$$\" > \"{}\"\n/bin/sh -c 'trap \"\" TERM; printf \"%s\\n\" \"$$\" > \
             \"{}\"; sleep {OUTLIVING_SECONDS}' >/dev/null 2>&1 &\nwait\nprintf '%s\\n' '1.2.3 \
             (claude)'\n",
            pid_file.display(),
            outliving_file.display()
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

    let engine = recorded(&pid_file);
    let outliving = recorded(&outliving_file);
    for process in [&engine, &outliving] {
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

    assert!(
        !is_running(&outliving),
        "process {outliving} — started by the engine of this measurement and left behind when that \
         engine ended — outlived the session that started it: {}. The quit reported {ending:?}",
        described(&outliving)
    );
    assert!(
        !is_running(&engine),
        "the engine {engine} outlived the session that started it: {}",
        described(&engine)
    );
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

#![forbid(unsafe_code)]
#![cfg(unix)]

//! Acceptance: a run that cannot admit a utility it observes its own processes with refuses to
//! start, and names the program and the reason.
//!
//! A managed run answers two questions with programs it executes on its own behalf: which processes
//! exist, and which of them hold the run's marker open. Where one of those programs stands
//! somewhere this account can write, the answer it gives is an answer this account chose. The rule
//! that admits it therefore refuses it — and the refusal has to stop the run, because the shape of
//! the swallowed answer is "no descendants", which reads exactly like a clean termination.
//!
//! The check that must fail: let the admission of the process-table reader be passed over instead
//! of refused, and `a_run_that_cannot_admit_its_process_table_reader_refuses_to_start` reports a run
//! that started, while `termination_that_cannot_read_the_process_table_is_not_reported_clean`
//! reports a termination that claimed success over a process that is still running. Both exit
//! non-zero.
//!
//! The utility is placed in this account's temporary tree, outside the repository, and is a working
//! copy of the platform's own program: a run that refuses it refuses because of where it stands and
//! not because the copy does not work.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use ymp_application::Application;
use ymp_domain::Budget;
use ymp_runtime_api::{
    LaunchChain, ProgramRole, RuntimeEventKind, RuntimeKind, configure_process_group,
    create_launch_marker, managed_launch_command, place_lifecycle_utility_for_fixture,
    register_launch_marker, terminate_process_tree,
};
use ymp_runtime_claude::{ClaudeProfile, ClaudeRuntime};
use ymp_runtime_supervisor::{
    ManagedCandidateRequest, ManagedContract, ManagedRunEvent, admit_runtime_start,
    start_managed_candidate,
};

/// Where the platform keeps the reader of the process table, and where this check puts it back.
const PROCESS_TABLE: &str = "/bin/ps";

/// The placement is a property of the whole process, so the cases take it in turn.
fn placement() -> MutexGuard<'static, ()> {
    static PLACEMENT: OnceLock<Mutex<()>> = OnceLock::new();
    PLACEMENT
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Puts a working process-table reader in a directory this account owns and hands that location to
/// the run. It forwards to the platform's own program, so it answers every question the run asks:
/// a refusal is then about where the program stands and not about what it does.
fn place_the_reader_where_this_account_can_write(root: &Path) -> PathBuf {
    let directory = root.join("utilities");
    fs::create_dir_all(&directory).expect("create the directory the utility is placed in");
    let placed = directory.join("ps");
    fs::write(&placed, format!("#!/bin/sh\nexec {PROCESS_TABLE} \"$@\"\n"))
        .expect("write the placed process-table reader");
    fs::set_permissions(&placed, fs::Permissions::from_mode(0o755))
        .expect("make the placed program executable");
    let works = Command::new(&placed)
        .args(["-A", "-o", "pid="])
        .stderr(Stdio::null())
        .output()
        .expect("run the placed utility");
    assert!(
        works.status.success() && !works.stdout.is_empty(),
        "the placed utility does not work, so a refusal would not be about where it stands"
    );
    place_lifecycle_utility_for_fixture(ProgramRole::ProcessTable, vec![placed.clone()]);
    placed
}

fn restore_the_platform_reader() {
    place_lifecycle_utility_for_fixture(
        ProgramRole::ProcessTable,
        vec![PathBuf::from(PROCESS_TABLE)],
    );
}

/// A runtime executable that answers the probe and nothing else. The run under test never reaches
/// the launch, so this is all the profile needs to be ready.
fn probe_answering_runtime(path: &Path) -> PathBuf {
    fs::write(
        path,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '2.1.227 (Claude Code)'
  exit 0
elif [ "$1" = "auth" ]; then
  printf '%s\n' '{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}'
  exit 0
fi
exit 9
"#,
    )
    .expect("write the runtime fixture");
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .expect("make the fixture executable");
    path.to_owned()
}

/// A runtime executable that answers the probe, records the identifier of its own process, and then
/// stays alive. The run under test is stopped by the controller rather than by the runtime, which is
/// the path on which a failure to end the processes used to be discarded.
///
/// The identifier is what the check ends the fixture by afterwards. The managed process leads its
/// own process group and this script is what replaces it, so the recorded identifier names both the
/// process and the group, and nothing outside the run is named at all.
fn waiting_runtime(path: &Path, own_process: &Path) -> PathBuf {
    fs::write(
        path,
        format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '2.1.227 (Claude Code)'
  exit 0
elif [ "$1" = "auth" ]; then
  printf '%s\n' '{{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}}'
  exit 0
fi
input=$(cat)
printf '%s\n' '{COORDINATED_INIT}'
printf '%s\n' "$$" > '{}'
sleep 120
"#,
            own_process.display()
        ),
    )
    .expect("write the runtime fixture");
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .expect("make the fixture executable");
    path.to_owned()
}

/// The event a Claude launch reports before anything else, which the driver requires before it
/// accepts any later event.
///
/// The configuration it reports is the one the profile admits. A record the profile refuses would
/// end the run with a protocol failure the moment the driver read it, and the case this file
/// measures — a cancellation that cannot establish what it left running — would never be reached.
const COORDINATED_INIT: &str = r#"{"type":"system","subtype":"init","session_id":"session-unestablished","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":["Bash","Edit","Glob","Grep","Read","Write","mcp__ymp__read_control","mcp__ymp__read_events","mcp__ymp__submit","mcp__ymp__yield"],"mcp_servers":[{"name":"ymp","status":"connected"}],"slash_commands":[],"plugins":[],"skills":[]}"#;

/// Ends the process group of the run this check started, and nothing else.
///
/// The managed process is launched into a process group of its own and the fixture replaces that
/// process, so its recorded identifier names exactly the group the run created. Ending that group
/// reaches every process the run started and no process it did not: a check that instead searched
/// the whole process table for a command it recognised would answer for processes belonging to
/// whatever else was running on the machine.
fn end_the_process_group_of_this_run(leader: u32) {
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &format!("-{leader}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Reads the identifier the fixture recorded for its own process, which is also the leader of the
/// group the run was launched into.
fn process_of_this_run(recorded: &Path) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Ok(text) = fs::read_to_string(recorded)
            && let Ok(pid) = text.trim().parse::<u32>()
        {
            return pid;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "the managed runtime never recorded its own process in {}",
        recorded.display()
    );
}

fn is_alive(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// How long a process must go on existing before this file will say it was left alone.
///
/// A signal reaches a process before the operating system stops reporting it: between the two the
/// process is dying and still answers every liveness question. A single reading taken at that
/// moment cannot tell "untouched" from "already killed", so every claim that something survived is
/// read against this window instead.
const SURVIVAL_WINDOW: Duration = Duration::from_secs(2);

/// Whether the process goes on existing for the whole window, rather than at the instant of asking.
fn outlives_the_window(pid: u32) -> bool {
    let deadline = Instant::now() + SURVIVAL_WINDOW;
    while Instant::now() < deadline {
        if !is_alive(pid) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    is_alive(pid)
}

/// The same reading for a process this file started itself, which it must reap to learn that the
/// process has ended.
fn child_outlives_the_window(child: &mut Child) -> bool {
    let deadline = Instant::now() + SURVIVAL_WINDOW;
    while Instant::now() < deadline {
        if child
            .try_wait()
            .expect("read the process started outside the run")
            .is_some()
        {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child
        .try_wait()
        .expect("read the process started outside the run")
        .is_none()
}

/// Ordinary daemonisation, written in whichever stock interpreter exposes `setsid`. The surviving
/// process owns its own session and is reparented to init at once, so nothing the operating system
/// still reports connects it to the run: the marker it carries is the only thing that does, and
/// finding the marker's holders is what the unadmittable utility makes impossible. A missing
/// interpreter fails the check rather than skipping it.
fn detaching_command() -> (&'static str, &'static str, &'static str) {
    const CANDIDATES: [(&str, &str, &str); 2] = [
        (
            "/usr/bin/perl",
            "-e",
            "use POSIX; exit 0 if fork(); POSIX::setsid() or die \"setsid\"; exit 0 if fork(); open(my $handle, \">\", $ARGV[0]) or die \"pid file\"; print $handle \"$$\\n\"; close $handle; sleep 300;",
        ),
        (
            "/usr/bin/python3",
            "-c",
            "import os, sys, time\nif os.fork(): raise SystemExit(0)\nos.setsid()\nif os.fork(): raise SystemExit(0)\nopen(sys.argv[1], \"w\").write(str(os.getpid()) + \"\\n\")\ntime.sleep(300)",
        ),
    ];
    CANDIDATES
        .into_iter()
        .find(|(program, _, _)| Path::new(program).is_file())
        .expect("a stock interpreter that can call setsid")
}

/// Starts a managed process the way the runtime drivers start one, so it carries the run's marker
/// and leads its own process group, and has it create one descendant that detaches itself.
fn managed_run_with_a_detached_descendant(pid_file: &Path) -> Child {
    let (interpreter, flag, script) = detaching_command();
    let marker = create_launch_marker().expect("create the run's marker");
    let chain = LaunchChain::default()
        .admit()
        .expect("admit the programs the launch enters");
    let mut command = managed_launch_command(
        Path::new("/bin/sh"),
        &[
            "-c".to_owned(),
            format!(
                "\"{interpreter}\" \"{flag}\" '{script}' \"{}\" </dev/null >/dev/null 2>&1\nsleep 300\n",
                pid_file.display()
            ),
        ],
        &marker,
        &chain,
    )
    .expect("build the managed launch command");
    command.stdout(Stdio::null()).stderr(Stdio::null());
    configure_process_group(&mut command);
    let child = command.spawn().expect("start the managed process");
    register_launch_marker(&child, marker);
    child
}

/// Waits until the detached descendant has recorded itself and been reparented to init, so the case
/// under test is the one described rather than a race with it.
fn detached_descendant(pid_file: &Path) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Ok(text) = fs::read_to_string(pid_file)
            && let Ok(pid) = text.trim().parse::<u32>()
            && is_alive(pid)
        {
            return pid;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "the managed run never recorded a detached descendant in {}",
        pid_file.display()
    );
}

fn kill(pid: u32) {
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// The gate itself, which is what the command line passes before it builds any runtime driver. A
/// command that starts a runtime therefore refuses on the same ground as the controller, and this
/// is where that is measured for both.
#[test]
fn the_gate_every_start_passes_refuses_an_unadmittable_utility() {
    let _placement = placement();
    let temporary = tempfile::tempdir().expect("temporary directory");
    let placed = place_the_reader_where_this_account_can_write(temporary.path());
    let refusal = admit_runtime_start(RuntimeKind::ClaudeCode);
    restore_the_platform_reader();

    let error =
        refusal.expect_err("no runtime may start where the run cannot observe its own processes");
    let reported = format!("{error:#}");
    assert!(
        reported.contains("process table reader") && reported.contains("this account can write"),
        "the refusal does not name the program and the reason: {reported}"
    );
    assert!(
        reported.contains(&placed.display().to_string()),
        "the refusal does not name where the program stands: {reported}"
    );
    admit_runtime_start(RuntimeKind::ClaudeCode)
        .expect("the platform's own utilities are admitted once they answer again");
}

#[test]
fn a_run_that_cannot_admit_its_process_table_reader_refuses_to_start() {
    let _placement = placement();
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    fs::create_dir(&source).expect("source directory");
    fs::write(source.join("input.txt"), b"before\n").expect("source file");
    let runtime = probe_answering_runtime(&temporary.path().join("claude-probe-only"));
    let application = Arc::new(Mutex::new(
        Application::create(
            temporary.path().join("data"),
            "run-unadmittable-reader",
            Budget::new(1, 1),
        )
        .expect("create application"),
    ));

    let placed = place_the_reader_where_this_account_can_write(temporary.path());
    let refusal = start_managed_candidate(
        Arc::clone(&application),
        Box::new(
            ClaudeRuntime::with_profile(&runtime, ClaudeProfile::default())
                .without_delegated_credential(),
        ),
        ManagedCandidateRequest {
            contract: ManagedContract {
                contract_id: "contract-unadmittable-reader".to_owned(),
                contract_digest: "d".repeat(64),
                source,
                prompt: "never reached".to_owned(),
                capture_exclusions: Vec::new(),
                verifier: None,
            },
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    );
    restore_the_platform_reader();

    let error = refusal
        .err()
        .expect("a run that cannot see its own processes must not start");
    let reported = format!("{error:#}");
    assert!(
        reported.contains("process table reader"),
        "the refusal does not name the program: {reported}"
    );
    assert!(
        reported.contains(&placed.display().to_string()),
        "the refusal does not name where the program stands: {reported}"
    );
    assert!(
        reported.contains("this account can write"),
        "the refusal does not state the reason: {reported}"
    );

    let application = application.lock().expect("application lock");
    assert!(
        application.state().active_attempts.is_empty(),
        "the run started an attempt it cannot observe"
    );
}

#[test]
fn termination_that_cannot_read_the_process_table_is_not_reported_clean() {
    let _placement = placement();
    let temporary = tempfile::tempdir().expect("temporary directory");
    let pid_file = temporary.path().join("descendant.pid");
    let mut child = managed_run_with_a_detached_descendant(&pid_file);
    let descendant = detached_descendant(&pid_file);

    place_the_reader_where_this_account_can_write(temporary.path());
    let outcome = terminate_process_tree(&mut child);
    let survived = outlives_the_window(descendant);
    restore_the_platform_reader();

    let Err(error) = outcome else {
        // What the accepted base does here: the reading of the marker's holders is abandoned, the
        // signal reaches only the process group the descendant has left, and the run reports a
        // clean termination over a process it started that is still running.
        let alive = if survived { "alive" } else { "already gone" };
        kill(descendant);
        let _ = terminate_process_tree(&mut child);
        panic!(
            "termination was reported clean while the descendant it started was {alive}, on a \
             process table that could not be read"
        );
    };
    assert!(
        error.to_string().contains("process table reader"),
        "the failure does not name the program that could not be admitted: {error}"
    );
    assert!(
        survived,
        "the descendant was gone before the reading failed, so the case is not the one under test"
    );

    // The same termination, once the platform's own reader answers again, ends every process the
    // run started and reports it. The refusal above is therefore about the utility rather than
    // about the processes.
    let ended = terminate_process_tree(&mut child);
    let deadline = Instant::now() + Duration::from_secs(10);
    while is_alive(descendant) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let outlived = is_alive(descendant);
    kill(descendant);
    ended.expect("terminate the managed run");
    assert!(
        !outlived,
        "the detached descendant outlived the termination of the run that started it"
    );
}

/// Acceptance: the refusal reaches the run's outcome from the paths that cannot return it.
///
/// A cancellation, a time limit and a session being dropped all end the process tree while they are
/// already reporting something else, so none of them can hand a failure back to a caller. They used
/// to throw it away, which is the defect this card exists to remove: the run then reported the
/// outcome it was already reporting, over processes it never established were gone.
///
/// The check that must fail: discard the result at the cancellation path again, and this reports a
/// run that ended without saying what it left running, with a non-zero exit.
///
/// The order this case depends on is stated rather than raced for. The run is cancelled only after
/// it has reported that it started, so the record the driver reads is behind it and no later reading
/// can end the run on some other ground; and a process outside the run, started from the same
/// executable, is held alive across the whole case, so that ending the run is measured to reach that
/// run's own process group and nothing else.
#[test]
fn a_cancelled_run_that_cannot_establish_termination_reports_it() {
    let _placement = placement();
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    fs::create_dir(&source).expect("source directory");
    fs::write(source.join("input.txt"), b"before\n").expect("source file");
    let recorded = temporary.path().join("runtime.process");
    let runtime = waiting_runtime(&temporary.path().join("claude-waiting"), &recorded);
    let mut stranger = Command::new(&runtime)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start a process outside the run from the same executable");
    let application = Arc::new(Mutex::new(
        Application::create(
            temporary.path().join("data"),
            "run-unestablished-termination",
            Budget::new(1, 1),
        )
        .expect("create application"),
    ));

    let handle = start_managed_candidate(
        Arc::clone(&application),
        Box::new(
            ClaudeRuntime::with_profile(&runtime, ClaudeProfile::default())
                .without_delegated_credential(),
        ),
        ManagedCandidateRequest {
            contract: ManagedContract {
                contract_id: "contract-unestablished-termination".to_owned(),
                contract_digest: "d".repeat(64),
                source,
                prompt: "wait to be stopped".to_owned(),
                capture_exclusions: Vec::new(),
                verifier: None,
            },
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    )
    .expect("start the managed run");

    // The run's own report that it started, rather than a file the fixture writes beside it. The
    // driver has then already read and admitted the session record, so cancelling here cannot be
    // overtaken by a reading that ends the run on a different ground.
    let mut reported = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut running = false;
    while !running && Instant::now() < deadline {
        while let Some(event) = handle.try_next() {
            running |= matches!(
                &event,
                ManagedRunEvent::Runtime(runtime_event)
                    if matches!(&runtime_event.event, RuntimeEventKind::Started { .. })
            );
            reported.push(event);
        }
        if handle.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        running,
        "the managed run never reported that it started; it reported {reported:?}"
    );
    let leader = process_of_this_run(&recorded);

    place_the_reader_where_this_account_can_write(temporary.path());
    handle.cancel("the check stopped the run").expect("cancel");
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline && !handle.is_finished() {
        while let Some(event) = handle.try_next() {
            reported.push(event);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    while let Some(event) = handle.try_next() {
        reported.push(event);
    }
    let finished = handle.is_finished();
    restore_the_platform_reader();
    handle.join().expect("join the supervisor worker");
    end_the_process_group_of_this_run(leader);

    // What the run left behind is ended by naming its own process group, so a process this check
    // started outside the run — from the same executable, and therefore indistinguishable from the
    // run's own by the command it reports — is still alive. This is the measurement, not the
    // convention: it fails if the cleanup ever answers for the machine rather than for this run.
    let outside_the_run_survived = child_outlives_the_window(&mut stranger);
    let _ = stranger.kill();
    let _ = stranger.wait();
    assert!(
        outside_the_run_survived,
        "ending the run reached a process it did not start"
    );

    assert!(finished, "the cancelled run never reached an outcome");
    let failure = reported.iter().find_map(|event| match event {
        ManagedRunEvent::Failed { detail } => Some(detail.clone()),
        _ => None,
    });
    let detail = failure.unwrap_or_else(|| {
        panic!(
            "the run reported no failure while it could not establish that it left nothing \
             running; it reported {reported:?}"
        )
    });
    assert!(
        detail.contains("termination_unestablished") && detail.contains("process table reader"),
        "the reported failure does not say what could not be established: {detail}; the run \
         reported {reported:?}"
    );
}

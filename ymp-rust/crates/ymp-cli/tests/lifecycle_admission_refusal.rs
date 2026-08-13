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
    LaunchChain, ProgramRole, configure_process_group, create_launch_marker,
    managed_launch_command, place_lifecycle_utility_for_fixture, register_launch_marker,
    terminate_process_tree,
};
use ymp_runtime_claude::{ClaudeProfile, ClaudeRuntime};
use ymp_runtime_supervisor::{ManagedCandidateRequest, ManagedContract, start_managed_candidate};

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

fn is_alive(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
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
    let survived = is_alive(descendant);
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

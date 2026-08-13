#![forbid(unsafe_code)]

//! Every terminal outcome of a managed run must leave no process the run started, including a
//! descendant that created its own session and was reparented to init. The proof is read from the
//! operating system's process table after the supervisor has finished, never from the supervisor's
//! own report.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use ymp_application::Application;
use ymp_domain::Budget;
use ymp_runtime_api::{RuntimeDriver, RuntimeEventKind};
use ymp_runtime_claude::{ClaudeProfile, ClaudeRuntime};
use ymp_runtime_codex::{CodexProfile, CodexRuntime};
use ymp_runtime_supervisor::{
    ManagedCandidateRequest, ManagedContract, ManagedRunEvent, ManagedRunHandle,
    start_managed_candidate,
};

const COORDINATED_INIT: &str = r#"{"type":"system","subtype":"init","session_id":"session-lifecycle","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":["Bash","Edit","Glob","Grep","Read","Write","mcp__ymp__read_control","mcp__ymp__read_events","mcp__ymp__submit","mcp__ymp__yield"],"mcp_servers":[{"name":"ymp","status":"connected"}],"slash_commands":[],"plugins":[],"skills":[]}"#;

/// The terminal outcomes a managed run can reach.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stop {
    Success,
    Error,
    Cancellation,
    Timeout,
    Budget,
}

impl Stop {
    /// The keyword the fixture reads from the prompt to select its behaviour.
    const fn keyword(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Cancellation => "cancel",
            Self::Timeout => "timeout",
            Self::Budget => "budget",
        }
    }

    const fn wall_time_limit_ms(self) -> u64 {
        if matches!(self, Self::Timeout) {
            8_000
        } else {
            60_000
        }
    }
}

/// A shell cannot create a session, so the descendant that detaches is written in whichever stock
/// interpreter exposes `setsid`. Missing interpreters fail the test instead of skipping it, because
/// a skipped check would report the escape as closed without measuring it.
fn session_detaching_command() -> (&'static str, &'static str, &'static str) {
    const CANDIDATES: [(&str, &str, &str); 2] = [
        (
            "/usr/bin/perl",
            "-e",
            "use POSIX; POSIX::setsid() or die \"setsid\"; open(my $handle, \">\", $ARGV[0]) or die \"pid file\"; print $handle \"$$\\n\"; close $handle; sleep 300;",
        ),
        (
            "/usr/bin/python3",
            "-c",
            "import os, sys, time; os.setsid(); open(sys.argv[1], \"w\").write(str(os.getpid()) + \"\\n\"); time.sleep(300)",
        ),
    ];
    CANDIDATES
        .into_iter()
        .find(|(program, _, _)| Path::new(program).is_file())
        .expect("a stock interpreter that can call setsid")
}

/// Writes the helper the managed agent runs in the background. The helper starts a process that
/// creates its own session, keeps it visible under a live parent for a few readings of the process
/// table, and then exits, so the session-owning process is reparented to init well before the run
/// reaches any terminal outcome. That is exactly the escape the review measured, and it means the
/// only thing that can still link the process to the run is the record kept while it was reachable.
/// Its standard streams are detached so that holding the agent's pipes open cannot mask the
/// lifecycle question with an unrelated read that never ends.
fn detaching_helper(path: &Path, pid_file: &Path, orphaned_file: &Path) -> PathBuf {
    let (interpreter, flag, script) = session_detaching_command();
    fs::write(
        path,
        format!(
            "#!/bin/sh\n\"{interpreter}\" \"{flag}\" '{script}' \"{pid}\" \
             </dev/null >/dev/null 2>&1 &\n{}sleep 0.4\nprintf 'orphaned\\n' > \"{orphaned}\"\n\
             exit 0\n",
            wait_for(pid_file),
            pid = pid_file.display(),
            orphaned = orphaned_file.display(),
        ),
    )
    .expect("write detaching helper");
    executable(path)
}

/// A bounded shell wait for a file to become non-empty. Waiting for the state instead of pausing for
/// a fixed time keeps the scenario the same on a loaded machine.
fn wait_for(path: &Path) -> String {
    format!(
        "waited=0\nwhile [ ! -s '{}' ] && [ $waited -lt 300 ]; do sleep 0.05; \
         waited=$((waited+1)); done\n",
        path.display()
    )
}

fn executable(path: &Path) -> PathBuf {
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make executable");
    path.to_path_buf()
}

/// The lines every fixture runs before it reaches its terminal behaviour. The fixture waits until
/// the helper reports that the session-owning process has been orphaned, and then until the test
/// opens the gate. Every outcome under test therefore starts from the same state, and the reading
/// that establishes the escape cannot be overtaken by the outcome it is meant to precede.
fn detach_prelude(helper: &Path, orphaned_file: &Path, gate_file: &Path) -> String {
    format!(
        "'{}' &\n{}{}",
        helper.display(),
        wait_for(orphaned_file),
        wait_for(gate_file)
    )
}

fn managed_source(root: &Path, name: &str) -> PathBuf {
    let source = root.join(name);
    fs::create_dir(&source).expect("source directory");
    fs::write(source.join("input.txt"), b"before\n").expect("source file");
    source
}

fn contract(contract_id: &str, source: PathBuf, prompt: &str) -> ManagedContract {
    ManagedContract {
        contract_id: contract_id.to_owned(),
        contract_digest: "d".repeat(64),
        source,
        prompt: prompt.to_owned(),
        capture_exclusions: Vec::new(),
        verifier: None,
    }
}

#[derive(Clone, Debug)]
struct ProcessEntry {
    pid: u32,
    parent: u32,
    group: u32,
}

fn process_table() -> Vec<ProcessEntry> {
    let output = Command::new("/bin/ps")
        .args(["-A", "-o", "pid=,ppid=,pgid="])
        .stderr(Stdio::null())
        .output()
        .expect("read the process table");
    assert!(output.status.success(), "ps exited with {}", output.status);
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let entries: Vec<ProcessEntry> = text
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse().ok()?;
            let parent = fields.next()?.parse().ok()?;
            let group = fields.next()?.parse().ok()?;
            Some(ProcessEntry { pid, parent, group })
        })
        .collect();
    assert!(!entries.is_empty(), "the process table could not be read");
    entries
}

fn entry(pid: u32) -> Option<ProcessEntry> {
    process_table().into_iter().find(|entry| entry.pid == pid)
}

fn read_descendant_pid(path: &Path) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Ok(text) = fs::read_to_string(path)
            && let Ok(pid) = text.trim().parse()
        {
            return pid;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "the managed agent never recorded a detached descendant in {}",
        path.display()
    );
}

/// Confirms the precondition every case shares: the process the run created is alive, owns its own
/// session and has been reparented to init, so nothing in the live process table connects it to the
/// run any more.
fn await_orphaned(pid: u32) -> ProcessEntry {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let observed = entry(pid).expect("the detached descendant is in the process table");
        if observed.parent == 1 {
            return observed;
        }
        assert!(
            Instant::now() < deadline,
            "the detached descendant was never reparented to init, so the escape is not reproduced"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Reads the process table until the descendant is gone, and reports what is still there when it is
/// not. The bound is generous relative to the supervisor's own termination bound, so a failure here
/// means the process survived rather than that the reading was early.
fn assert_absent_from_process_table(pid: u32, label: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let observed = entry(pid);
        let Some(observed) = observed else {
            return;
        };
        if Instant::now() >= deadline {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", &pid.to_string()])
                .status();
            panic!(
                "{label}: process {pid} started by the managed run is still in the process table \
                 (parent {}, group {})",
                observed.parent, observed.group
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

struct Observed {
    candidate: Option<String>,
    runtime: Vec<RuntimeEventKind>,
}

/// Drives the run to its terminal outcome, cancelling it once the descendant exists when the
/// outcome under test is a controller-issued stop.
fn drive(
    handle: ManagedRunHandle,
    pid_file: &Path,
    gate_file: &Path,
    cancel: Option<&str>,
) -> (u32, Observed) {
    let descendant = read_descendant_pid(pid_file);
    let observed_entry = await_orphaned(descendant);
    assert_eq!(
        observed_entry.group, descendant,
        "the descendant did not leave the managed process group, so the escape is not reproduced"
    );
    fs::write(gate_file, b"observed\n").expect("open the observation gate");
    if let Some(reason) = cancel {
        handle.cancel(reason).expect("cancel the managed run");
    }
    let mut observed = Observed {
        candidate: None,
        runtime: Vec::new(),
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let collect = |event, observed: &mut Observed| match event {
        ManagedRunEvent::CandidateAvailable {
            candidate_digest, ..
        } => observed.candidate = Some(candidate_digest),
        ManagedRunEvent::Runtime(event) => observed.runtime.push(event.event),
        ManagedRunEvent::Failed { .. } | ManagedRunEvent::Finished => {}
    };
    while Instant::now() < deadline && !handle.is_finished() {
        while let Some(event) = handle.try_next() {
            collect(event, &mut observed);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    while let Some(event) = handle.try_next() {
        collect(event, &mut observed);
    }
    assert!(
        handle.is_finished(),
        "the managed run did not reach a terminal outcome"
    );
    handle.join().expect("join the supervisor worker");
    (descendant, observed)
}

fn assert_reached(stop: Stop, observed: &Observed) {
    let reached = observed.runtime.iter().any(|event| match stop {
        Stop::Success => matches!(event, RuntimeEventKind::Completed { .. }),
        Stop::Error | Stop::Budget => matches!(event, RuntimeEventKind::Failed { .. }),
        Stop::Cancellation => matches!(event, RuntimeEventKind::Cancelled { .. }),
        Stop::Timeout => matches!(event, RuntimeEventKind::TimedOut { .. }),
    });
    assert!(
        reached,
        "the run did not reach the {:?} outcome under test; observed {:?}",
        stop, observed.runtime
    );
    if stop == Stop::Success {
        assert!(
            observed.candidate.is_some(),
            "the successful run recorded no candidate"
        );
    }
}

fn application(root: &Path, label: &str) -> Arc<Mutex<Application>> {
    Arc::new(Mutex::new(
        Application::create(
            root.join(format!("data-{label}")),
            format!("run-{label}"),
            Budget::new(1, 1),
        )
        .expect("create application"),
    ))
}

fn start(
    application: Arc<Mutex<Application>>,
    driver: Box<dyn RuntimeDriver>,
    source: PathBuf,
    label: &str,
    prompt: &str,
) -> ManagedRunHandle {
    start_managed_candidate(
        application,
        driver,
        ManagedCandidateRequest {
            contract: contract(&format!("lifecycle-{label}"), source, prompt),
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    )
    .expect("start the managed candidate")
}

fn claude_fixture(path: &Path, helper: &Path, orphaned_file: &Path, gate_file: &Path) -> PathBuf {
    let prelude = detach_prelude(helper, orphaned_file, gate_file);
    fs::write(
        path,
        format!(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '2.1.227 (Claude Code)'
  exit 0
elif [ "$1" = "auth" ]; then
  printf '%s\n' '{{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}}'
  exit 0
fi
config=''
previous=''
for argument in "$@"; do
  if [ "$previous" = '--mcp-config' ]; then config=$argument; fi
  previous=$argument
done
bridge=$(printf '%s' "$config" | sed -n 's/.*"command":"\([^"]*\)".*/\1/p')
input=$(cat)
{prelude}
printf '%s\n' '{COORDINATED_INIT}'
case "$input" in
  *success*)
    responses=$({{
      printf '%s\n' '{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-11-25"}}}}'
      printf '%s\n' '{{"jsonrpc":"2.0","method":"notifications/initialized"}}'
      printf '%s\n' '{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"submit","arguments":{{"command_id":"lifecycle-submit"}}}}}}'
    }} | "$bridge" internal agent-mcp) || exit 41
    printf '%s' "$responses" | grep -q '"snapshot_digest"' || exit 42
    printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.031,"usage":{{"input_tokens":11,"cache_creation_input_tokens":3,"cache_read_input_tokens":4,"output_tokens":5,"output_tokens_details":{{"thinking_tokens":2}}}}}}'
    ;;
  *error*)
    printf '%s\n' '{{"type":"result","subtype":"error_during_execution","is_error":true,"terminal_reason":"runtime_error","total_cost_usd":0.004,"usage":{{"input_tokens":9,"cache_creation_input_tokens":1,"cache_read_input_tokens":1,"output_tokens":2,"output_tokens_details":{{"thinking_tokens":0}}}}}}'
    exit 1
    ;;
  *budget*)
    printf '%s\n' '{{"type":"result","subtype":"error_max_budget_usd","is_error":true,"terminal_reason":"budget_exhausted","total_cost_usd":0.031,"usage":{{"input_tokens":11,"cache_creation_input_tokens":3,"cache_read_input_tokens":4,"output_tokens":5,"output_tokens_details":{{"thinking_tokens":2}}}}}}'
    exit 1
    ;;
  *) sleep 300 ;;
esac
"##
        ),
    )
    .expect("write the Claude fixture");
    executable(path)
}

fn codex_fixture(path: &Path, helper: &Path, orphaned_file: &Path, gate_file: &Path) -> PathBuf {
    let prelude = detach_prelude(helper, orphaned_file, gate_file);
    fs::write(
        path,
        format!(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  input=$(cat)
  bridge=''
  for argument in "$@"; do
    case "$argument" in
      mcp_servers.ymp.command=*)
        bridge=${{argument#mcp_servers.ymp.command=}}
        bridge=${{bridge#\"}}
        bridge=${{bridge%\"}}
        ;;
    esac
  done
{prelude}
  printf '%s\n' '{{"type":"thread.started","thread_id":"thread-lifecycle"}}'
  case "$input" in
    *success*)
      responses=$({{
        printf '%s\n' '{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-11-25"}}}}'
        printf '%s\n' '{{"jsonrpc":"2.0","method":"notifications/initialized"}}'
        printf '%s\n' '{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"submit","arguments":{{"command_id":"lifecycle-submit"}}}}}}'
      }} | "$bridge" internal agent-mcp) || exit 41
      printf '%s' "$responses" | grep -q '"snapshot_digest"' || exit 42
      printf '%s\n' '{{"type":"item.completed","item":{{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{{"command_id":"lifecycle-submit"}},"result":{{"committed":true}},"error":null}}}}'
      printf '%s\n' '{{"type":"turn.completed","usage":{{"input_tokens":11,"cached_input_tokens":4,"output_tokens":5,"reasoning_output_tokens":2,"cost_microusd":31,"protected_queries":3}}}}'
      ;;
    *error*)
      printf '%s\n' '{{"type":"turn.failed","usage":{{"input_tokens":11,"cached_input_tokens":4,"output_tokens":5,"reasoning_output_tokens":2,"cost_microusd":31,"protected_queries":3}},"error":{{"code":"fixture"}}}}'
      ;;
    *) sleep 300 ;;
  esac
fi
"##
        ),
    )
    .expect("write the Codex fixture");
    executable(path)
}

fn claude_run(stop: Stop) {
    let label = format!("claude-{}", stop.keyword());
    let temporary = tempfile::tempdir().expect("temporary directory");
    let pid_file = temporary.path().join("descendant.pid");
    let orphaned_file = temporary.path().join("descendant.orphaned");
    let gate_file = temporary.path().join("descendant.gate");
    let helper = detaching_helper(
        &temporary.path().join("detach.sh"),
        &pid_file,
        &orphaned_file,
    );
    let fixture = claude_fixture(
        &temporary.path().join("claude-lifecycle"),
        &helper,
        &orphaned_file,
        &gate_file,
    );
    let profile = ClaudeProfile {
        wall_time_limit_ms: stop.wall_time_limit_ms(),
        ..ClaudeProfile::default()
    };
    let handle = start(
        application(temporary.path(), &label),
        Box::new(ClaudeRuntime::with_profile(&fixture, profile).without_delegated_credential()),
        managed_source(temporary.path(), "source"),
        &label,
        &format!("{} lifecycle", stop.keyword()),
    );
    let cancel = matches!(stop, Stop::Cancellation).then_some("controller stopped the run");
    let (descendant, observed) = drive(handle, &pid_file, &gate_file, cancel);
    assert_reached(stop, &observed);
    assert_absent_from_process_table(descendant, &label);
}

fn codex_run(stop: Stop) {
    let label = format!("codex-{}", stop.keyword());
    let temporary = tempfile::tempdir().expect("temporary directory");
    let pid_file = temporary.path().join("descendant.pid");
    let orphaned_file = temporary.path().join("descendant.orphaned");
    let gate_file = temporary.path().join("descendant.gate");
    let helper = detaching_helper(
        &temporary.path().join("detach.sh"),
        &pid_file,
        &orphaned_file,
    );
    let fixture = codex_fixture(
        &temporary.path().join("codex-lifecycle"),
        &helper,
        &orphaned_file,
        &gate_file,
    );
    let profile = CodexProfile {
        wall_time_limit_ms: stop.wall_time_limit_ms(),
        ..CodexProfile::default()
    };
    let handle = start(
        application(temporary.path(), &label),
        Box::new(CodexRuntime::with_profile(&fixture, profile)),
        managed_source(temporary.path(), "source"),
        &label,
        &format!("{} lifecycle", stop.keyword()),
    );
    // The Codex profile has no budget record of its own, so its budget stop is the run-level stop
    // the controller issues; it differs from a cancellation only in the recorded reason.
    let cancel = match stop {
        Stop::Cancellation => Some("controller stopped the run"),
        Stop::Budget => Some("run budget exhausted"),
        _ => None,
    };
    let (descendant, observed) = drive(handle, &pid_file, &gate_file, cancel);
    let reached = if stop == Stop::Budget {
        Stop::Cancellation
    } else {
        stop
    };
    assert_reached(reached, &observed);
    assert_absent_from_process_table(descendant, &label);
}

#[test]
fn claude_success_leaves_no_detached_descendant() {
    claude_run(Stop::Success);
}

#[test]
fn claude_error_leaves_no_detached_descendant() {
    claude_run(Stop::Error);
}

#[test]
fn claude_cancellation_leaves_no_detached_descendant() {
    claude_run(Stop::Cancellation);
}

#[test]
fn claude_timeout_leaves_no_detached_descendant() {
    claude_run(Stop::Timeout);
}

#[test]
fn claude_budget_stop_leaves_no_detached_descendant() {
    claude_run(Stop::Budget);
}

#[test]
fn codex_success_leaves_no_detached_descendant() {
    codex_run(Stop::Success);
}

#[test]
fn codex_error_leaves_no_detached_descendant() {
    codex_run(Stop::Error);
}

#[test]
fn codex_cancellation_leaves_no_detached_descendant() {
    codex_run(Stop::Cancellation);
}

#[test]
fn codex_timeout_leaves_no_detached_descendant() {
    codex_run(Stop::Timeout);
}

#[test]
fn codex_budget_stop_leaves_no_detached_descendant() {
    codex_run(Stop::Budget);
}

/// The bound of the check in the other direction: terminating one managed run must not reach a
/// process that run did not start. A sibling process of the supervisor, and the child that sibling
/// spawns, both outlive the managed run they were never part of.
#[test]
fn termination_spares_processes_the_managed_run_did_not_start() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let sibling_pid_file = temporary.path().join("sibling.pid");
    let mut sibling = Command::new("/bin/sh")
        .arg("-c")
        .arg("sleep 120 & printf '%s\\n' \"$!\" > \"$1\"; sleep 120")
        .arg("sibling")
        .arg(&sibling_pid_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn an unmanaged sibling");
    let sibling_child = read_descendant_pid(&sibling_pid_file);

    let pid_file = temporary.path().join("descendant.pid");
    let orphaned_file = temporary.path().join("descendant.orphaned");
    let gate_file = temporary.path().join("descendant.gate");
    let helper = detaching_helper(
        &temporary.path().join("detach.sh"),
        &pid_file,
        &orphaned_file,
    );
    let fixture = codex_fixture(
        &temporary.path().join("codex-lifecycle"),
        &helper,
        &orphaned_file,
        &gate_file,
    );
    let handle = start(
        application(temporary.path(), "spare"),
        Box::new(CodexRuntime::with_profile(
            &fixture,
            CodexProfile {
                wall_time_limit_ms: 20_000,
                ..CodexProfile::default()
            },
        )),
        managed_source(temporary.path(), "source"),
        "spare",
        "success lifecycle",
    );
    let (descendant, observed) = drive(handle, &pid_file, &gate_file, None);
    assert_reached(Stop::Success, &observed);
    assert_absent_from_process_table(descendant, "spare");

    let table = process_table();
    let survived = |pid: u32| table.iter().any(|entry| entry.pid == pid);
    let sibling_alive = survived(sibling.id());
    let sibling_child_alive = survived(sibling_child);
    let _ = sibling.kill();
    let _ = sibling.wait();
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &sibling_child.to_string()])
        .status();
    assert!(
        sibling_alive,
        "terminating the managed run killed a sibling process it did not start"
    );
    assert!(
        sibling_child_alive,
        "terminating the managed run killed a process below a sibling it did not start"
    );
}

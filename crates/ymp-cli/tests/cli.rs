#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn run(arguments: &[OsString]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ymp"))
        .args(arguments)
        .output()
        .expect("ymp executable runs")
}

#[test]
fn no_arguments_and_help_report_commands_storage_and_current_limit() {
    for arguments in [Vec::new(), vec![OsString::from("--help")]] {
        let output = run(&arguments);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");
        assert!(
            stdout.contains("ymp run \"<task>\" [--check \"<shell command>\"] [--data-dir PATH]")
        );
        assert!(stdout.contains("ymp show <session-id> [--data-dir PATH]"));
        assert!(stdout.contains("./.ymp/sessions"));
        assert!(stdout.contains("does not establish acceptance or confirmation"));
    }
}

#[test]
fn version_uses_the_application_name_and_package_version() {
    let output = run(&[OsString::from("--version")]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("version is UTF-8"),
        format!("ymp {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn invalid_commands_and_arguments_fail_honestly() {
    let unsupported = run(&[OsString::from("run")]);
    assert_eq!(unsupported.status.code(), Some(2));
    assert!(unsupported.stdout.is_empty());
    let stderr = String::from_utf8(unsupported.stderr).expect("error is UTF-8");
    assert!(stderr.contains("run requires task text"));
    assert!(stderr.contains("Usage: ymp run"));

    let additional = run(&[OsString::from("--help"), OsString::from("extra")]);
    assert_eq!(additional.status.code(), Some(2));
    assert!(additional.stdout.is_empty());
    let stderr = String::from_utf8(additional.stderr).expect("error is UTF-8");
    assert!(stderr.contains("--help does not accept additional arguments"));

    let unknown = run(&[OsString::from("unknown")]);
    assert_eq!(unknown.status.code(), Some(2));
    let stderr = String::from_utf8(unknown.stderr).expect("error is UTF-8");
    assert!(stderr.contains("unsupported command 'unknown'"));
}

#[cfg(unix)]
#[test]
fn non_unicode_argument_fails_without_panicking() {
    use std::os::unix::ffi::OsStringExt;

    let output = run(&[OsString::from_vec(vec![0xff, b'x'])]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("lossy error is UTF-8");
    assert!(stderr.contains("command must be valid UTF-8"));
    assert!(!stderr.contains("panicked"));
}

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock follows the Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("ymp-cli-real-{}-{unique}", std::process::id()));
        std::fs::create_dir(&path).expect("test directory is created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    #[cfg(unix)]
    fn executable(&self, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = self.0.join(name);
        std::fs::write(&path, body).expect("scripted executable is written");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("scripted executable permissions are set");
        path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn signal_pid(pid: u32, signal: &str) -> bool {
    Command::new("/bin/kill")
        .args(["-s", signal, &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(unix)]
fn wait_for_pid_file(path: &Path, timeout: Duration) -> Option<u32> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(value) = std::fs::read_to_string(path)
            && let Ok(pid) = value.trim().parse::<u32>()
        {
            return Some(pid);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
fn wait_for_pid_exit(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !signal_pid(pid, "0") {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
#[test]
fn sigint_cancels_child_and_records_observed_termination_before_exit_130() {
    let directory = TestDir::new();
    let data_dir = directory.path().join("sessions");
    let provider_pid = directory.path().join("provider-pid");
    let descendant_pid = directory.path().join("descendant-pid");
    let provider_exited = directory.path().join("provider-exited");
    let check_marker = directory.path().join("interrupted-check-ran");
    let codex = directory.executable(
        "codex",
        "#!/bin/sh\n\
         if [ \"$1\" = \"--version\" ]; then printf 'codex-cli scripted\\n'; exit 0; fi\n\
         printf '%s\\n' \"$$\" > \"$YMP_TEST_PROVIDER_PID\"\n\
         /bin/sh -c 'trap \"\" TERM INT; exec >/dev/null 2>&1; exec /bin/sleep 30' &\n\
         descendant=$!\n\
         printf '%s\\n' \"$descendant\" > \"$YMP_TEST_DESCENDANT_PID\"\n\
         trap 'printf exited > \"$YMP_TEST_PROVIDER_EXITED\"; exit 0' TERM\n\
         wait \"$descendant\"\n",
    );
    let _path_spoof = directory.executable("ps", "#!/bin/sh\nexit 0\n");
    let search_path = format!(
        "{}:{}",
        codex.parent().expect("script directory").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_ymp"))
        .current_dir(directory.path())
        .env("PATH", search_path)
        .env("YMP_TEST_PROVIDER_PID", &provider_pid)
        .env("YMP_TEST_DESCENDANT_PID", &descendant_pid)
        .env("YMP_TEST_PROVIDER_EXITED", &provider_exited)
        .args([
            OsString::from("run"),
            OsString::from("wait until interrupted"),
            OsString::from("--check"),
            OsString::from("printf ran > interrupted-check-ran"),
            OsString::from("--data-dir"),
            data_dir.as_os_str().to_owned(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("ymp child starts");
    let provider = wait_for_pid_file(&provider_pid, Duration::from_secs(15));
    let descendant = wait_for_pid_file(&descendant_pid, Duration::from_secs(15));
    if provider.is_none() || descendant.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        if let Some(pid) = provider {
            let _ = signal_pid(pid, "KILL");
        }
        if let Some(pid) = descendant {
            let _ = signal_pid(pid, "KILL");
        }
        panic!("the local provider and its descendant must publish their PIDs");
    }
    let provider = provider.expect("provider PID was checked");
    let descendant = descendant.expect("descendant PID was checked");

    if !signal_pid(child.id(), "INT") {
        let _ = child.kill();
        let _ = child.wait();
        let _ = signal_pid(provider, "KILL");
        let _ = signal_pid(descendant, "KILL");
        panic!("SIGINT must reach the exact ymp child PID");
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while child
        .try_wait()
        .expect("ymp child status is readable")
        .is_none()
        && Instant::now() < deadline
    {
        thread::sleep(Duration::from_millis(10));
    }
    if child
        .try_wait()
        .expect("ymp child status is readable")
        .is_none()
    {
        let _ = child.kill();
        let _ = signal_pid(provider, "KILL");
        let _ = signal_pid(descendant, "KILL");
    }
    let output = child.wait_with_output().expect("ymp child is collected");

    let provider_stopped = wait_for_pid_exit(provider, Duration::from_secs(5));
    let descendant_stopped = wait_for_pid_exit(descendant, Duration::from_secs(5));
    if !provider_stopped {
        let _ = signal_pid(provider, "KILL");
    }
    if !descendant_stopped {
        let _ = signal_pid(descendant, "KILL");
    }

    assert_eq!(output.status.code(), Some(130));
    assert!(output.stderr.is_empty());
    assert!(
        provider_exited.exists(),
        "SIGTERM reaches the provider child"
    );
    assert!(provider_stopped, "the provider child must be gone");
    assert!(descendant_stopped, "the provider descendant must be gone");
    let stdout = String::from_utf8(output.stdout).expect("run output is UTF-8");
    assert!(stdout.contains("Outcome: cancelled"), "{stdout}");
    assert!(
        !check_marker.exists(),
        "an interrupted run must not execute --check"
    );
    let session_id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Session: "))
        .expect("run report contains the session ID");
    let shown = run(&[
        OsString::from("show"),
        OsString::from(session_id),
        OsString::from("--data-dir"),
        data_dir.as_os_str().to_owned(),
    ]);
    assert!(
        shown.status.success(),
        "show succeeds: {}",
        String::from_utf8_lossy(&shown.stderr)
    );
    let shown = String::from_utf8(shown.stdout).expect("show output is UTF-8");
    assert!(shown.contains("Outcome: cancelled"), "{shown}");
    assert!(shown.contains("Accounted: yes"), "{shown}");
}

#[cfg(target_os = "linux")]
#[test]
fn linux_internal_sandbox_allows_workspace_writes_and_protects_journal_and_host_proc() {
    let directory = TestDir::new();
    let protected = directory.path().join("sessions");
    std::fs::create_dir(&protected).expect("protected directory is created");
    let journal = protected.join("journal.db");
    std::fs::write(&journal, b"authoritative").expect("journal fixture is written");
    let marker = directory.path().join("started");
    let allowed = directory.path().join("allowed");
    let script = format!(
        "printf allowed > {allowed:?} && test ! -e /proc/{}/root && \
         if printf changed > {journal:?}; then exit 9; else exit 0; fi",
        std::process::id()
    );

    let output = Command::new(env!("CARGO_BIN_EXE_ymp"))
        .current_dir(directory.path())
        .args([
            OsString::from("__ymp_internal_check_sandbox"),
            protected.as_os_str().to_owned(),
            marker.as_os_str().to_owned(),
            OsString::from("/bin/sh"),
            OsString::from(script),
        ])
        .output()
        .expect("internal sandbox helper runs");
    if output.status.code() == Some(125) && !marker.exists() {
        assert_eq!(
            std::fs::read(&journal).expect("journal fixture reads"),
            b"authoritative"
        );
        return;
    }

    assert!(
        output.status.success(),
        "sandbox helper failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(marker.is_file());
    assert_eq!(
        std::fs::read(allowed).expect("ordinary workspace write reads"),
        b"allowed"
    );
    assert_eq!(
        std::fs::read(journal).expect("journal fixture reads"),
        b"authoritative"
    );
}

/// REAL PROVIDER INVOCATION — SPENDS REAL QUOTA.
///
/// The unattended suite never runs this test. It requires an installed,
/// natively authenticated `codex` executable and explicit selection:
///
/// ```text
/// cargo test -p ymp-cli --test cli real_run_invokes_codex_checks_echo -- --ignored --nocapture
/// ```
#[test]
#[ignore = "spends real provider quota; requires the owner's explicit go"]
fn real_run_invokes_codex_checks_echo_and_persists_a_session() {
    let directory = TestDir::new();
    let data_dir = directory.path().join("sessions");
    let output = Command::new(env!("CARGO_BIN_EXE_ymp"))
        .current_dir(directory.path())
        .args([
            OsString::from("run"),
            OsString::from("Reply with the single word: pong"),
            OsString::from("--check"),
            OsString::from("echo ok"),
            OsString::from("--data-dir"),
            data_dir.as_os_str().to_owned(),
        ])
        .output()
        .expect("ymp executable runs");

    assert!(
        output.status.success(),
        "real run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("output is UTF-8");
    assert!(stdout.contains("Outcome: completed"));
    assert!(stdout.contains("Criterion requested-work-completed: satisfied with evidence"));
    assert!(stdout.contains("Check command: \"echo ok\""));
    assert!(stdout.contains("Exit code: 0"));
    assert!(data_dir.join("journal.db").is_file());
}

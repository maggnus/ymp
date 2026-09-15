#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

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
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::process::{Command, Output};

fn run(arguments: &[OsString]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ymp"))
        .args(arguments)
        .output()
        .expect("ymp executable runs")
}

#[test]
fn no_arguments_and_help_report_the_current_limit() {
    for arguments in [Vec::new(), vec![OsString::from("--help")]] {
        let output = run(&arguments);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout).expect("help is UTF-8");
        assert!(stdout.contains("Usage: ymp [--help | --version]"));
        assert!(stdout.contains("does not execute agents or tasks"));
        assert!(stdout.contains("state is not persisted"));
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
fn unsupported_and_additional_arguments_fail_honestly() {
    let unsupported = run(&[OsString::from("run")]);
    assert_eq!(unsupported.status.code(), Some(2));
    assert!(unsupported.stdout.is_empty());
    let stderr = String::from_utf8(unsupported.stderr).expect("error is UTF-8");
    assert!(stderr.contains("unsupported argument 'run'"));
    assert!(stderr.contains("Usage: ymp"));

    let additional = run(&[OsString::from("--help"), OsString::from("extra")]);
    assert_eq!(additional.status.code(), Some(2));
    assert!(additional.stdout.is_empty());
    let stderr = String::from_utf8(additional.stderr).expect("error is UTF-8");
    assert!(stderr.contains("expected at most one argument, received 2"));
}

#[cfg(unix)]
#[test]
fn non_unicode_argument_fails_without_panicking() {
    use std::os::unix::ffi::OsStringExt;

    let output = run(&[OsString::from_vec(vec![0xff, b'x'])]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("lossy error is UTF-8");
    assert!(stderr.contains("unsupported argument"));
    assert!(!stderr.contains("panicked"));
}

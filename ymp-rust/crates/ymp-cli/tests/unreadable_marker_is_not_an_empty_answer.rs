#![forbid(unsafe_code)]
#![cfg(unix)]

//! A marker this account may not read must not be reported as a marker nobody holds.
//!
//! The utility that lists the holders of a file exits with code one and prints nothing both when no
//! process holds the file and when the file could not be read at all. The lookup must separate the
//! two, because the first ends a run and the second establishes nothing. The measurement is taken
//! against a real marker on disk whose access is really denied, never against a stub.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use ymp_runtime_api::marker_holders;

/// The measurement is only meaningful for an account the file system can refuse. The superuser
/// reads a marker of mode zero, so the denial under test never happens and the check would report a
/// property it did not establish.
fn refuse_the_superuser() {
    let uid = Command::new("id")
        .arg("-u")
        .output()
        .expect("read the running account's identifier");
    let uid = String::from_utf8_lossy(&uid.stdout).trim().to_owned();
    assert_ne!(
        uid, "0",
        "this check must run as an account the file system can refuse; as the superuser the \
         denial under test does not occur"
    );
}

/// Denies every access to the marker, so that reading it fails for this account.
fn deny_access(marker: &Path) {
    fs::set_permissions(marker, fs::Permissions::from_mode(0o000))
        .expect("deny every access to the marker");
}

#[test]
fn a_marker_this_account_cannot_read_is_reported_as_a_failure_and_not_as_no_holders() {
    refuse_the_superuser();
    let directory = tempfile::tempdir().expect("a directory for the marker");
    let marker = directory.path().join("launch.marker");
    fs::write(&marker, b"").expect("create the marker");
    deny_access(&marker);

    // What the operating system's own utility reports about this marker: the same exit code and the
    // same empty output it gives when nothing holds the marker at all.
    if let Ok(output) = Command::new("/usr/sbin/lsof")
        .arg("-t")
        .arg(&marker)
        .output()
    {
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
    }

    let refusal = marker_holders(&marker)
        .expect_err("a marker that could not be read is not the answer that nobody holds it");
    let refusal = refusal.to_string();
    assert!(
        refusal.contains("could not be read"),
        "the refusal must name the unread marker as the reason: {refusal}"
    );
    assert!(
        refusal.contains(&marker.display().to_string()),
        "the refusal must name the marker it is about: {refusal}"
    );

    fs::set_permissions(&marker, fs::Permissions::from_mode(0o600))
        .expect("restore access so the directory can be removed");
}

#[test]
fn a_readable_marker_no_process_holds_is_still_the_answer_that_nobody_holds_it() {
    let directory = tempfile::tempdir().expect("a directory for the marker");
    let marker = directory.path().join("launch.marker");
    fs::write(&marker, b"").expect("create the marker");

    assert_eq!(
        marker_holders(&marker).expect("a readable marker nobody holds is an answer"),
        Vec::<u32>::new()
    );
}

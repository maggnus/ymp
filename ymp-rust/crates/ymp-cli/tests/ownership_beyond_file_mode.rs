#![forbid(unsafe_code)]
#![cfg(unix)]

//! Acceptance: the rule that admits a program by its location reads the access-control entries of
//! every ancestor of that location, not only their modes.
//!
//! The mode of a directory describes what its owner, its group and everyone else may do. It does
//! not describe an entry that names one account and grants it write access on top of that, so a
//! directory whose mode shows `rwxr-xr-x` can still be one this account is free to write. A rule
//! that read the mode alone would admit a program standing there and record a digest bound to
//! nothing.
//!
//! The check that must fail: read the entries out of the location rule, and
//! `an_ancestor_that_grants_write_through_an_access_control_entry_is_refused` reports a refusal
//! that names only the ownership of the directory, with a non-zero exit.
//!
//! The directory is built outside the repository, under this account's temporary tree, and it is
//! removed with it. Nothing here writes to a system directory.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use ymp_runtime_api::{AdmittedProgram, ProgramRequirement, ProgramRole};

/// Grants this account write access to the directory through an access-control entry, leaving its
/// mode exactly as it stood.
#[cfg(target_os = "macos")]
fn grant_write_through_an_entry(directory: &Path) {
    let account = std::env::var("USER").expect("the account this check runs under");
    let status = Command::new("/bin/chmod")
        .arg("+a")
        .arg(format!(
            "{account} allow write,add_file,add_subdirectory,delete_child"
        ))
        .arg(directory)
        .status()
        .expect("run the platform's access-control utility");
    assert!(
        status.success(),
        "the access-control entry could not be added, so the case is not constructed"
    );
}

/// The same grant where the platform keeps access-control entries in a separate utility. Its
/// absence fails the check rather than skipping it: a skipped check would report the rule as
/// covering entries without having measured one.
#[cfg(not(target_os = "macos"))]
fn grant_write_through_an_entry(directory: &Path) {
    let account = std::env::var("USER").expect("the account this check runs under");
    let status = Command::new("/usr/bin/setfacl")
        .arg("-m")
        .arg(format!("u:{account}:rwx"))
        .arg(directory)
        .status()
        .expect("run the platform's access-control utility");
    assert!(
        status.success(),
        "the access-control entry could not be added, so the case is not constructed"
    );
}

fn mode_of(path: &Path) -> u32 {
    fs::metadata(path).expect("metadata").permissions().mode() & 0o7777
}

#[test]
fn an_ancestor_that_grants_write_through_an_access_control_entry_is_refused() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let directory = temporary.path().join("mode-looks-safe");
    fs::create_dir(&directory).expect("create the directory under test");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).expect("safe-looking mode");
    let program = directory.join("ps");
    fs::copy("/bin/ps", &program).expect("a working program to admit");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("program mode");

    let mode = mode_of(&directory);
    assert_eq!(
        mode & 0o022,
        0,
        "the directory under test must grant nothing beyond its owner by its mode"
    );

    // Before the entry exists the location is refused on its ownership alone, and the refusal says
    // nothing about entries. This is what makes the difference measured below the entry itself.
    let before = AdmittedProgram::admit(
        ProgramRole::ProcessTable,
        &program,
        &ProgramRequirement::SystemPath,
    )
    .expect_err("a location outside the system tree is never admitted");
    assert!(
        !before.to_string().contains("access-control entry"),
        "the location carries no entry yet, but the refusal names one: {before}"
    );

    grant_write_through_an_entry(&directory);
    assert_eq!(
        mode_of(&directory),
        mode,
        "the entry must be added without changing the mode, or the case is not the one under test"
    );

    let after = AdmittedProgram::admit(
        ProgramRole::ProcessTable,
        &program,
        &ProgramRequirement::SystemPath,
    )
    .expect_err("a location whose entries grant write access is never admitted");
    let reported = after.to_string();
    assert!(
        reported.contains("access-control entry"),
        "the refusal does not name the entry that grants write access: {reported}"
    );
    assert!(
        reported.contains(&directory.display().to_string()),
        "the refusal does not name the directory that carries the entry: {reported}"
    );
}

/// The negative control for the check above. A rule that refused every location would satisfy it
/// while reading nothing at all, so the platform's own utilities must still be admitted.
#[test]
fn a_location_only_the_superuser_can_write_is_still_admitted() {
    for program in ["/bin/ps", "/bin/kill", "/bin/ls", "/bin/sh"] {
        AdmittedProgram::admit(
            ProgramRole::ProcessTable,
            Path::new(program),
            &ProgramRequirement::SystemPath,
        )
        .unwrap_or_else(|error| panic!("{program} is no longer admitted: {error}"));
    }
}

#![forbid(unsafe_code)]

//! Reproduces, on the built product, the substitution that a name resolved through the environment
//! search path allows: a program planted in a directory this account owns answers to the name the
//! run uses to build its workspace baseline.
//!
//! The planted program is fully functional — it forwards to the system program — so a run that
//! refuses it refuses because of its identity and not because the substitute failed to work.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write program");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make executable");
}

/// Writes the contract package the start path requires, and returns its path.
fn contract_package(root: &Path) -> PathBuf {
    let source = root.join("source");
    let negative_control = root.join("negative-control");
    fs::create_dir_all(&source).expect("source");
    fs::create_dir_all(&negative_control).expect("negative control");
    fs::write(source.join("input.txt"), b"before\n").expect("source file");
    let program = root.join("verify.sh");
    executable(&program, "#!/bin/sh\nexit 0\n");
    let oracle_digest = ymp_domain::digest_bytes(&fs::read(&program).expect("program bytes"));
    let package = root.join("contract.json");
    fs::write(
        &package,
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 2,
            "contract_id": "contract-planted-search-path",
            "source": "source",
            "prompt": "build the baseline",
            "verifier": {
                "program": "verify.sh",
                "negative_control": "negative-control",
                "oracle_digest": oracle_digest,
                "wall_time_ms": 60_000,
                "output_limit_bytes": 1024
            }
        }))
        .expect("package bytes"),
    )
    .expect("write package");
    package
}

#[test]
fn a_program_planted_in_the_search_path_never_builds_a_managed_workspace() {
    let root = tempfile::tempdir().expect("temporary directory");
    let package = contract_package(root.path());

    let planted_directory = root.path().join("planted");
    fs::create_dir(&planted_directory).expect("planted directory");
    let executions = root.path().join("planted.executions");
    executable(
        &planted_directory.join("git"),
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec /usr/bin/git \"$@\"\n",
            executions.display()
        ),
    );

    // A run is created against the pool it may draw its models from, so this store's root offers
    // one before the invocation. What this check is about stands after the creation: the workspace
    // program the run would build its private history with.
    ymp_testkit::ready_root::measured(&root.path().join("data"));

    let outcome = Command::new(env!("CARGO_BIN_EXE_ymp"))
        .args(["--data-root"])
        .arg(root.path().join("data"))
        .arg("--contract")
        .arg(&package)
        .args(["internal", "managed-candidate-smoke", "--runtime", "codex"])
        .env("PATH", {
            let mut search = planted_directory.clone().into_os_string();
            search.push(":/usr/bin:/bin");
            search
        })
        .env("HOME", root.path())
        .output()
        .expect("run the built product");

    let diagnostic = String::from_utf8_lossy(&outcome.stderr).to_string();
    assert!(
        !outcome.status.success(),
        "the run accepted a planted program: {diagnostic}"
    );
    assert!(
        diagnostic.contains("this account can write"),
        "the refusal does not name the binding that failed: {diagnostic}"
    );
    assert!(
        !executions.exists(),
        "the planted program was executed {} time(s)",
        fs::read_to_string(&executions)
            .expect("execution record")
            .lines()
            .count()
    );
}

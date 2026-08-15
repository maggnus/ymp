//! Acceptance: the reader the schema document describes is the reader the product reaches. A
//! package is loaded at each schema version, and the version the document names must be the one
//! the reader accepts.
//!
//! The structural half this file used to hold — a scan for a run started outside the
//! contract-bound scenario — is retired, and the reason is recorded here because this is where a
//! reader looks for it. That scan cut every file at the first `#[cfg(test)]` and read nothing
//! after it, so a start written below a test module passed it; it also dropped `ymp-testkit` from
//! the reading because the crate was named in the scan itself, which is an exemption granted by
//! declaration rather than a fact about the product. Its subject is now held by
//! `no_crate_the_shipped_binary_reaches_starts_a_run_without_a_contract` in
//! `ymp-cli/tests/one_command_path.rs`, which reads the source as elements rather than text, never
//! cuts a file at a marker, and derives the crates it reads from the manifests instead of a list.
//!
//! What the retirement gives up, stated rather than implied. The retired scan also read the crates
//! the shipped executable does not link. Of those, `ymp-testkit` is the one crate that links
//! `ymp-application` and can therefore name the constructor at all, and
//! `the_fixture_crate_stays_outside_the_shipped_binary` pins both its exclusion from the closure
//! and the contractless start it holds. The remaining crates outside the closure are the tools
//! under `tools/`, and none of them links `ymp-application`, so the constructor is out of their
//! reach by compilation rather than by a check. That last fact is read from the manifests today
//! and is not pinned by any test; a tool that starts linking `ymp-application` would leave it
//! unread.

use std::fs;
use std::path::{Path, PathBuf};

use ymp_domain::contract::CONTRACT_SCHEMA_VERSION;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn the_reader_accepts_the_version_the_schema_document_names() {
    let document = fs::read_to_string(workspace_root().join("SCHEMA.md")).expect("SCHEMA.md");
    assert_eq!(
        CONTRACT_SCHEMA_VERSION, 2,
        "the reader moved away from the version this test pins"
    );
    assert!(
        document.contains("Contract-record version 1 made the verifier optional. Version 2"),
        "SCHEMA.md does not name version 2 as the required contract record"
    );

    let root = tempfile::tempdir().expect("temporary root");
    let source = root.path().join("source");
    let negative_control = root.path().join("negative-control");
    let program = root.path().join("verify.sh");
    fs::create_dir_all(&source).expect("source");
    fs::create_dir_all(&negative_control).expect("negative control");
    fs::write(&program, b"#!/bin/sh\nexit 1\n").expect("program");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions).expect("make executable");
    }
    let oracle_digest = ymp_domain::digest_bytes(&fs::read(&program).expect("program bytes"));

    let package = |version: u32| {
        serde_json::to_vec(&serde_json::json!({
            "schema_version": version,
            "contract_id": "contract-versioned",
            "source": "source",
            "prompt": "keep the replay path idempotent",
            "verifier": {
                "program": "verify.sh",
                "negative_control": "negative-control",
                "oracle_digest": oracle_digest,
                "wall_time_ms": 60_000,
                "output_limit_bytes": 1024
            }
        }))
        .expect("package bytes")
    };
    let path = root.path().join("contract.json");

    // The version the document requires is the one the product loads.
    fs::write(&path, package(CONTRACT_SCHEMA_VERSION)).expect("write package");
    let prepared = ymp_application::load_contract_package(&path).expect("current version loads");
    assert_eq!(prepared.contract_id(), "contract-versioned");

    // The superseded version is refused, and the refusal names it.
    fs::write(&path, package(1)).expect("write package");
    let reported = ymp_application::load_contract_package(&path)
        .expect_err("the superseded version must be refused")
        .to_string();
    assert!(
        reported.contains("unsupported contract schema version 1"),
        "{reported}"
    );
}

//! Acceptance: one implementation starts every run, and the reader the schema document
//! describes is the reader the product reaches.
//!
//! Two halves. The structural half walks the shipped source of every workspace crate and rejects
//! a run started outside the contract-bound scenario: reintroduce
//! `Application::create(&root, "run", budget)` in any product module — the terminal interface,
//! the command line, a runtime driver — and `every_start_path_goes_through_one_scenario` reports
//! it with a non-zero exit. The behavioural half loads a package at each schema version and
//! requires the version the document names to be the one the reader accepts.

use std::fs;
use std::path::{Path, PathBuf};

use ymp_domain::contract::CONTRACT_SCHEMA_VERSION;

/// Ways to start a run that do not carry an approved contract.
const UNBOUND_STARTS: [&str; 2] = ["Application::create(", "Application::create_with_config("];

/// Where the one scenario lives, and the crate that defines the constructor it calls.
const SCENARIO: [&str; 2] = [
    "ymp-application/src/contract.rs",
    "ymp-application/src/lib.rs",
];

/// A fixture crate, not a product path. It is a dependency of the command line and no command
/// calls it; folding it into a development dependency is left as separate work and is reported
/// with this card rather than hidden here.
const FIXTURES: &str = "ymp-testkit/";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn shipped_sources() -> Vec<(String, String)> {
    let root = workspace_root();
    let mut sources = Vec::new();
    for group in ["crates", "tools"] {
        let Ok(entries) = fs::read_dir(root.join(group)) else {
            continue;
        };
        for entry in entries.flatten() {
            collect(&entry.path().join("src"), &root, &mut sources);
        }
    }
    sources.sort();
    sources
}

fn collect(directory: &Path, root: &Path, sources: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, root, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let name = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            // Only shipped code is scanned: a unit-test module compiled with the crate builds
            // fixtures, and a fixture run is not a product start path.
            let text = fs::read_to_string(&path).expect("readable source");
            let shipped = text
                .find("#[cfg(test)]")
                .map_or(text.as_str(), |cut| &text[..cut])
                .to_owned();
            sources.push((name, shipped));
        }
    }
}

#[test]
fn every_start_path_goes_through_one_scenario() {
    let mut offenders = Vec::new();
    for (name, source) in shipped_sources() {
        if SCENARIO.iter().any(|allowed| name.ends_with(allowed)) || name.contains(FIXTURES) {
            continue;
        }
        for (number, line) in source.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for start in UNBOUND_STARTS {
                if line.contains(start) {
                    offenders.push(format!("{name}:{}: {}", number + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these product modules start a run outside the contract-bound scenario:\n{}",
        offenders.join("\n")
    );
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

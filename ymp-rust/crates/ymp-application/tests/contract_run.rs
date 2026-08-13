//! Acceptance: a typed request becomes a stored contract and a started run, and a request
//! without a checkable acceptance condition starts nothing and says exactly what it lacks.
//!
//! The negative half is the second test: it drives the same use case with the acceptance
//! condition removed and requires both a refusal naming the missing part and a data root that
//! holds no journal afterwards. A guard that merely reported the refusal while creating the run
//! would fail on the second assertion.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use ymp_application::{
    AcceptanceCondition, Application, ContractRequestError, RunRequest, load_contract_package,
    prepare_contract,
};
use ymp_domain::EventKind;
use ymp_domain::contract::{CONTRACT_SCHEMA_VERSION, ContractDocument};

struct Fixture {
    _root: TempDir,
    data_root: PathBuf,
    source: PathBuf,
    program: PathBuf,
    negative_control: PathBuf,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().expect("temporary root");
    let source = root.path().join("source");
    let negative_control = root.path().join("negative-control");
    let program = root.path().join("verify.sh");
    fs::create_dir_all(&source).expect("source directory");
    fs::create_dir_all(&negative_control).expect("negative control directory");
    fs::write(source.join("result.txt"), b"before\n").expect("source file");
    fs::write(&program, b"#!/bin/sh\ntest -f \"$1/result.txt\"\n").expect("verifier program");
    make_executable(&program);
    Fixture {
        data_root: root.path().join("data"),
        source,
        program,
        negative_control,
        _root: root,
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make executable");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

fn request(fixture: &Fixture, acceptance: Option<AcceptanceCondition>) -> RunRequest {
    RunRequest {
        prompt: "keep the replay path idempotent".to_owned(),
        source: fixture.source.clone(),
        acceptance,
        capture_exclusions: vec!["target".to_owned()],
        contract_id: None,
        budget: None,
    }
}

fn acceptance(fixture: &Fixture) -> AcceptanceCondition {
    AcceptanceCondition::new(&fixture.program, &fixture.negative_control)
}

#[test]
fn a_typed_request_becomes_a_stored_contract_and_a_started_run() {
    let fixture = fixture();
    let prepared = prepare_contract(&request(&fixture, Some(acceptance(&fixture))))
        .expect("prepared contract");

    assert!(prepared.contract_id().starts_with("contract-"));
    assert_eq!(prepared.oracle_digest().len(), 64);
    assert_eq!(
        prepared.verifier().negative_control,
        fixture.negative_control.canonicalize().expect("canonical")
    );

    let (application, outcome) =
        Application::create_with_contract(&fixture.data_root, &prepared).expect("started run");
    assert!(!outcome.replayed);

    let events = application.events_after(0).expect("committed events");
    assert!(
        matches!(events[0].event, EventKind::RunStarted { .. }),
        "{:?}",
        events[0].event
    );
    let EventKind::ContractApproved {
        contract_id,
        contract_digest,
        oracle_digest,
    } = &events[1].event
    else {
        panic!("the journal does not record the approved contract: {events:?}");
    };
    assert_eq!(contract_id, prepared.contract_id());
    assert_eq!(contract_digest, &prepared.contract_digest);
    assert_eq!(oracle_digest, prepared.oracle_digest());
    assert_eq!(application.state().run_id, prepared.run_id());

    // The stored contract is the exact bytes that were digested, and it carries the verifier
    // reference, its digest and the negative control.
    let stored = application
        .contract_bytes()
        .expect("stored contract")
        .expect("the run is bound to a contract");
    assert_eq!(stored, prepared.bytes());
    let parsed = ContractDocument::parse(&stored).expect("stored contract parses");
    assert_eq!(parsed.digest, prepared.contract_digest);
    assert_eq!(parsed.document.verifier.oracle_digest, *oracle_digest);

    // The binding survives recovery from the journal alone.
    drop(application);
    let recovered = Application::open(&fixture.data_root).expect("reopened store");
    let binding = recovered.contract().expect("the binding was recovered");
    assert_eq!(binding.contract_digest, prepared.contract_digest);
}

#[test]
fn a_request_without_an_acceptance_condition_starts_no_run_and_names_what_is_missing() {
    let fixture = fixture();
    let error =
        prepare_contract(&request(&fixture, None)).expect_err("the request must be refused");
    let reported = error.to_string();
    assert!(
        matches!(error, ContractRequestError::Missing(_)),
        "{reported}"
    );
    assert!(reported.contains("acceptance condition"), "{reported}");
    assert!(reported.contains("no run started"), "{reported}");
    assert!(
        !fixture.data_root.join("events.jsonl").exists(),
        "a refused request left a journal behind"
    );

    // The same refusal names the part that is missing when only the negative control is absent.
    let mut condition = acceptance(&fixture);
    condition.negative_control = PathBuf::new();
    let reported = prepare_contract(&request(&fixture, Some(condition)))
        .expect_err("a verifier without a negative control must be refused")
        .to_string();
    assert!(reported.contains("negative control"), "{reported}");
}

#[test]
fn a_package_without_a_verifier_is_rejected_at_load() {
    let fixture = fixture();
    let package = fixture.source.parent().expect("root").join("contract.json");
    fs::write(
        &package,
        serde_json::to_vec(&serde_json::json!({
            "schema_version": CONTRACT_SCHEMA_VERSION,
            "contract_id": "contract-from-a-file",
            "source": fixture.source,
            "prompt": "keep the replay path idempotent"
        }))
        .expect("package bytes"),
    )
    .expect("write package");

    let reported = load_contract_package(&package)
        .expect_err("a package without an acceptance condition must be rejected")
        .to_string();
    assert!(reported.contains("acceptance condition"), "{reported}");

    // A package whose oracle digest names something other than its verifier program is rejected:
    // the stored contract may not carry a claim the program contradicts.
    let package_with = |oracle_digest: String| {
        serde_json::to_vec(&serde_json::json!({
            "schema_version": CONTRACT_SCHEMA_VERSION,
            "contract_id": "contract-from-a-file",
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
    fs::write(&package, package_with("a".repeat(64))).expect("write package");
    let reported = load_contract_package(&package)
        .expect_err("a mismatched oracle digest must be rejected")
        .to_string();
    assert!(
        reported.contains("does not identify the verifier"),
        "{reported}"
    );

    // The same package with the digest of its own program loads, keeps its identifier and
    // resolves its paths against the package file.
    let program_digest = ymp_domain::digest_bytes(&fs::read(&fixture.program).expect("program"));
    fs::write(&package, package_with(program_digest.clone())).expect("write package");
    let prepared = load_contract_package(&package).expect("package loads");
    assert_eq!(prepared.contract_id(), "contract-from-a-file");
    assert_eq!(prepared.oracle_digest(), program_digest);
    assert_eq!(
        prepared.verifier().program,
        fixture.program.canonicalize().expect("canonical")
    );
}

#[test]
fn a_verifier_that_is_not_an_executable_file_starts_no_run() {
    let fixture = fixture();
    let mut condition = acceptance(&fixture);
    condition.program = fixture.source.clone();
    let reported = prepare_contract(&request(&fixture, Some(condition)))
        .expect_err("a directory cannot decide a candidate")
        .to_string();
    assert!(reported.contains("not an executable file"), "{reported}");
    assert!(
        !fixture.data_root.exists(),
        "a refused request wrote a store"
    );
}

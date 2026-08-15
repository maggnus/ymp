//! Acceptance: authorizing one contract twice produces two runs that can be told apart.
//!
//! A run identifier derived from the contract alone named the contract, not the run: two stores
//! under one root held two runs both recorded as `run-<contract digest>`, so a journal, a run
//! projection or an export naming one of them named the other just as well. The identifier now
//! states the contract the run is judged against and the store its record lives in, and a store
//! holds exactly one run.
//!
//! The negative half is measured rather than assumed: both stores are read back, the earlier
//! derivation is recomputed from the contract digest each store's own approval records, and the
//! two results are required to be the same string — the collision the earlier build wrote. The
//! same test then requires the identifiers the two runs actually carry to differ.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use ymp_application::root::{DataRoot, StoreIntent};
use ymp_application::{AcceptanceCondition, Application, RunRequest, prepare_contract, run_stem};
use ymp_domain::{EventEnvelope, EventKind, RunState};

struct Fixture {
    _root: TempDir,
    root: PathBuf,
    project: PathBuf,
    source: PathBuf,
    program: PathBuf,
    negative_control: PathBuf,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().expect("temporary root");
    let source = root.path().join("source");
    let project = root.path().join("project");
    let negative_control = root.path().join("negative-control");
    let program = root.path().join("verify.sh");
    fs::create_dir_all(&source).expect("source directory");
    fs::create_dir_all(&project).expect("project directory");
    fs::create_dir_all(&negative_control).expect("negative control directory");
    fs::write(source.join("result.txt"), b"before\n").expect("source file");
    fs::write(&program, b"#!/bin/sh\ntest -f \"$1/result.txt\"\n").expect("verifier program");
    make_executable(&program);
    Fixture {
        root: root.path().join("ymp-root"),
        project,
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

/// The store the layout addresses for the next run of this project, exactly as an invocation
/// that starts a run is given one.
fn next_store(fixture: &Fixture) -> PathBuf {
    DataRoot::open_for_project(&fixture.root, &fixture.project)
        .expect("the root is readable")
        .store(StoreIntent::New)
        .expect("the layout addresses the next store")
}

fn journal(store: &Path) -> Vec<EventEnvelope> {
    fs::read_to_string(store.join("events.jsonl"))
        .expect("the store holds a journal")
        .lines()
        .map(|line| serde_json::from_str(line).expect("every record is an envelope"))
        .collect()
}

/// The contract digest the run's own approval records, read back from its journal. Nothing about
/// the run identifier is taken on trust here: this is what the store itself states the run is
/// judged against.
fn approved_contract_digest(events: &[EventEnvelope]) -> String {
    events
        .iter()
        .find_map(|envelope| match &envelope.event {
            EventKind::ContractApproved {
                contract_digest, ..
            } => Some(contract_digest.clone()),
            _ => None,
        })
        .expect("the journal records the approved contract")
}

/// The run identifier the run projection carries — the record a report or an export reads without
/// replaying the journal.
fn projected_run_id(store: &Path) -> String {
    let bytes = fs::read(store.join("run.json")).expect("the store holds a run projection");
    let state: RunState = serde_json::from_slice(&bytes).expect("the projection parses");
    state.run_id
}

#[test]
fn two_authorizations_of_one_contract_are_two_distinguishable_runs() {
    let fixture = fixture();
    let prepared = prepare_contract(&RunRequest {
        prompt: "keep the replay path idempotent".to_owned(),
        source: fixture.source.clone(),
        acceptance: Some(AcceptanceCondition::new(
            &fixture.program,
            &fixture.negative_control,
        )),
        capture_exclusions: Vec::new(),
        contract_id: None,
        budget: None,
    })
    .expect("prepared contract");

    // Two authorizations of this one contract, each into the store the layout addresses for it.
    let first_store = next_store(&fixture);
    let (first, _) =
        Application::create_with_contract(&first_store, &prepared).expect("the first run starts");
    let first_run = first.state().run_id.clone();
    drop(first);

    let second_store = next_store(&fixture);
    let (second, _) =
        Application::create_with_contract(&second_store, &prepared).expect("the second run starts");
    let second_run = second.state().run_id.clone();
    drop(second);

    assert_ne!(
        first_store, second_store,
        "the layout gave both runs one store"
    );

    let first_events = journal(&first_store);
    let second_events = journal(&second_store);

    // The negative half, measured on the two runs that were just started: the earlier identifier
    // is recomputed from the contract digest each store records for itself, and it is one string
    // for both stores. That is the collision this task exists for.
    let earlier_first = run_stem(&approved_contract_digest(&first_events));
    let earlier_second = run_stem(&approved_contract_digest(&second_events));
    assert_eq!(
        earlier_first, earlier_second,
        "the contract-scoped identifier is expected to collide; it is what the runs are told \
         apart from"
    );

    // What the runs now carry: the same contract in both, and a different run in each.
    assert_ne!(
        first_run, second_run,
        "two runs of one contract carry one identifier: {first_run}"
    );
    assert!(
        first_run.starts_with(&earlier_first) && second_run.starts_with(&earlier_second),
        "a run identifier no longer names the contract it is judged against: {first_run}, \
         {second_run}"
    );

    // Every surface that records the identifier records the run's own, not the contract's: the
    // journal each store holds, and the projection a report or an export reads from it.
    assert!(
        first_events
            .iter()
            .all(|envelope| envelope.run_id == first_run),
        "the first journal mixes run identifiers"
    );
    assert!(
        second_events
            .iter()
            .all(|envelope| envelope.run_id == second_run),
        "the second journal mixes run identifiers"
    );
    assert_eq!(projected_run_id(&first_store), first_run);
    assert_eq!(projected_run_id(&second_store), second_run);

    // The identifier a decision surface states before the authorization is the identifier the run
    // then carries, for the store that authorization addresses.
    assert_eq!(prepared.run_id_in(&first_store), first_run);
    assert_eq!(prepared.run_id_in(&second_store), second_run);
}

#[test]
fn the_store_is_what_tells_two_runs_of_one_contract_apart() {
    let fixture = fixture();
    let prepared = prepare_contract(&RunRequest {
        prompt: "keep the replay path idempotent".to_owned(),
        source: fixture.source.clone(),
        acceptance: Some(AcceptanceCondition::new(
            &fixture.program,
            &fixture.negative_control,
        )),
        capture_exclusions: Vec::new(),
        contract_id: None,
        budget: None,
    })
    .expect("prepared contract");

    // One store is one run, so a redundant separator or a `.` in the path it was addressed with
    // does not make a second one.
    let store = next_store(&fixture);
    assert_eq!(
        prepared.run_id_in(&store),
        prepared.run_id_in(store.join("."))
    );

    // A store outside any root — what `--data-root` addresses — holds a run of its own, told from
    // the runs under the root by the same rule.
    let outside = fixture.project.join("standalone-store");
    assert_ne!(prepared.run_id_in(&store), prepared.run_id_in(&outside));

    // The contract is still named by the identifier: what differs between the two is the store.
    let stem = run_stem(&prepared.contract_digest);
    assert!(
        prepared.run_id_in(&store).starts_with(&stem)
            && prepared.run_id_in(&outside).starts_with(&stem)
    );
}

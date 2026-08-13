#![forbid(unsafe_code)]

//! Acceptance: the runtime whose launch this workspace does not attest cannot produce a candidate
//! in the shipped product, and the seams that exist for the checks cannot be reached from it.
//!
//! Two halves.
//!
//! The behavioural half hands the in-process fixture runtime to the controller's candidate start
//! and requires a refusal that names it: attestation is what binds the bytes that execute to the
//! bytes that were admitted, so a candidate produced without it would carry no evidence of the
//! program that wrote it. Its negative control hands over a runtime whose launch is attested and
//! requires the refusal to be a different one, because a check that refused every start would pass
//! the first half while proving nothing.
//!
//! The structural half walks the shipped source of every crate and rejects a module that names the
//! seams the checks use — the unattested candidate start and the placement of a lifecycle utility.
//! Add `start_unattested_managed_candidate(...)` to a product module and
//! `no_shipped_module_reaches_the_seams_the_checks_use` reports the file, the line and the name
//! with a non-zero exit.
//!
//! What the structural half lets through, stated rather than implied: code that is generated rather
//! than written, a name reached through an alias introduced by a macro, and the modules that define
//! the seams, which are named below and cannot be scanned for their own definition.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ymp_application::Application;
use ymp_domain::Budget;
use ymp_runtime_claude::{ClaudeProfile, ClaudeRuntime};
use ymp_runtime_fake::FakeRuntime;
use ymp_runtime_supervisor::{ManagedCandidateRequest, ManagedContract, start_managed_candidate};

/// The seams the checks use to drive the controller without an attested runtime, and to observe a
/// run that cannot admit one of its utilities. Neither is an operator capability.
const SEAMS: [&str; 2] = [
    "start_unattested_managed_candidate",
    "place_lifecycle_utility_for_fixture",
];

/// Where each seam is defined. A definition names the seam, and no scan can distinguish that from a
/// call, so the defining module is named here instead of exempted silently.
const DEFINITIONS: [&str; 2] = [
    "crates/ymp-runtime-supervisor/src/lib.rs",
    "crates/ymp-runtime-api/src/lib.rs",
];

/// The runtime whose launch is not attested, under the names a module would select it by.
const UNATTESTED_RUNTIME: [&str; 2] = ["FakeRuntime", "ymp_runtime_fake"];

/// The controller's candidate start, which that runtime may not reach.
const CANDIDATE_START: &str = "start_managed_candidate";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

/// Every shipped module of the workspace, as the source that stands before its own test module.
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
            let text = fs::read_to_string(&path).expect("readable source");
            let shipped = text
                .find("#[cfg(test)]")
                .map_or(text.as_str(), |cut| &text[..cut])
                .to_owned();
            sources.push((name, shipped));
        }
    }
}

fn code_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line))
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.starts_with("*") && !trimmed.starts_with("/*")
        })
}

#[test]
fn no_shipped_module_reaches_the_seams_the_checks_use() {
    let mut offenders = Vec::new();
    for (name, source) in shipped_sources() {
        if DEFINITIONS.contains(&name.as_str()) {
            continue;
        }
        for (number, line) in code_lines(&source) {
            for seam in SEAMS {
                if line.contains(seam) {
                    offenders.push(format!("{name}:{number}: {seam}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a shipped module reaches a seam that exists for the checks:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn no_shipped_module_takes_the_unattested_runtime_to_a_candidate() {
    let mut offenders = Vec::new();
    for (name, source) in shipped_sources() {
        let selects = code_lines(&source)
            .any(|(_, line)| UNATTESTED_RUNTIME.iter().any(|name| line.contains(name)));
        if !selects {
            continue;
        }
        for (number, line) in code_lines(&source) {
            if line.contains(CANDIDATE_START) {
                offenders.push(format!("{name}:{number}: {CANDIDATE_START}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a shipped module that selects the unattested runtime also starts a candidate:\n{}",
        offenders.join("\n")
    );
}

/// The scan reads the source that stands before a crate's own test module, and reads code rather
/// than prose. Without this the two checks above could pass because they read nothing.
#[test]
fn the_scan_reads_the_shipped_source_of_the_whole_workspace() {
    let sources = shipped_sources();
    assert!(
        sources.len() > 20,
        "the scan found {} modules, which is not this workspace",
        sources.len()
    );
    for definition in DEFINITIONS {
        let (_, source) = sources
            .iter()
            .find(|(name, _)| name == definition)
            .unwrap_or_else(|| panic!("{definition} was not scanned"));
        assert!(
            code_lines(source).any(|(_, line)| SEAMS.iter().any(|seam| line.contains(seam))),
            "{definition} no longer defines the seam it is exempted for"
        );
    }
    assert!(
        !code_lines("// start_unattested_managed_candidate in a comment\n").any(|(_, _)| true),
        "a comment was read as code"
    );
}

fn contract(source: PathBuf) -> ManagedContract {
    ManagedContract {
        contract_id: "contract-unattested".to_owned(),
        contract_digest: "d".repeat(64),
        source,
        prompt: "never reached".to_owned(),
        capture_exclusions: Vec::new(),
        verifier: None,
    }
}

fn prepared(root: &Path, label: &str) -> (Arc<Mutex<Application>>, PathBuf) {
    let source = root.join(format!("source-{label}"));
    fs::create_dir(&source).expect("source directory");
    fs::write(source.join("input.txt"), b"before\n").expect("source file");
    let application = Arc::new(Mutex::new(
        Application::create(
            root.join(format!("data-{label}")),
            format!("run-{label}"),
            Budget::new(1, 1),
        )
        .expect("create application"),
    ));
    (application, source)
}

#[test]
fn the_controller_refuses_a_candidate_from_a_runtime_it_does_not_attest() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (application, source) = prepared(temporary.path(), "unattested");
    let refusal = start_managed_candidate(
        Arc::clone(&application),
        Box::new(FakeRuntime::default()),
        ManagedCandidateRequest {
            contract: contract(source),
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    );
    let error = refusal
        .err()
        .expect("a runtime whose launch is not attested must not produce a candidate");
    let reported = format!("{error:#}");
    assert!(
        reported.contains("does not attest"),
        "the refusal does not state why the runtime was refused: {reported}"
    );
    let application = application.lock().expect("application lock");
    assert!(
        application.state().active_attempts.is_empty(),
        "the run started an attempt with a runtime it does not attest"
    );
}

/// The negative control for the refusal above: a runtime whose launch is attested reaches its own
/// probe, so the refusal is about attestation rather than about starting at all.
#[test]
fn a_runtime_whose_launch_is_attested_reaches_its_probe() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let (application, source) = prepared(temporary.path(), "attested");
    let missing = temporary.path().join("claude-that-is-not-installed");
    let refusal = start_managed_candidate(
        Arc::clone(&application),
        Box::new(
            ClaudeRuntime::with_profile(&missing, ClaudeProfile::default())
                .without_delegated_credential(),
        ),
        ManagedCandidateRequest {
            contract: contract(source),
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    );
    let error = refusal
        .err()
        .expect("a runtime executable that is absent cannot start a run either");
    let reported = format!("{error:#}");
    assert!(
        !reported.contains("does not attest"),
        "an attested runtime was refused as unattested: {reported}"
    );
}

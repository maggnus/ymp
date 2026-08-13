#![forbid(unsafe_code)]

//! Acceptance: every path in the shipped product that starts a runtime passes one gate, and the
//! runtime this workspace does not attest is not on the product's surface at all.
//!
//! The gate establishes two things: the runtime attests the programs its launch enters, and the
//! utilities the run observes and ends its own processes with are admitted. A start that skipped it
//! would run an agent this product can neither bind to admitted bytes nor observe, and any
//! candidate that run assembled would carry no evidence of the program that wrote it.
//!
//! Three halves, each of which can fail on its own.
//!
//! The surface half drives the built product: every command that takes a runtime is offered the
//! fixture runtime and must refuse the value, and no candidate may appear in the data root of such
//! an invocation. This is the reviewer's own reproduction, and it reads the surface rather than the
//! source.
//!
//! The behavioural half hands the fixture runtime to the controller's candidate start in process
//! and requires a refusal that names it; its negative control hands over an attested runtime and
//! requires the refusal to be a different one, because a check that refused every start would pass
//! while proving nothing.
//!
//! The structural half walks the shipped source of every crate. It rejects a module that names the
//! fixture runtime beside any start or candidate submission — the shape the defect took — a module
//! that starts a runtime it built without naming the gate, and any use of the seams that exist for
//! the checks. No file is excluded as a whole: a seam may appear only on the line that defines it,
//! so the module defining a seam is still read for every other use of it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use ymp_application::Application;
use ymp_domain::Budget;
use ymp_runtime_claude::{ClaudeProfile, ClaudeRuntime};
use ymp_runtime_fake::FakeRuntime;
use ymp_runtime_supervisor::{ManagedCandidateRequest, ManagedContract, start_managed_candidate};

/// The seams the checks use: an unattested candidate start, and the placement of a lifecycle
/// utility. Each may appear only where it is defined.
const SEAMS: [&str; 2] = [
    "start_unattested_managed_candidate",
    "place_lifecycle_utility_for_fixture",
];

/// The runtime whose launch is not attested, under the names a module would reach it by.
const UNATTESTED_RUNTIME: [&str; 2] = ["FakeRuntime", "ymp_runtime_fake"];

/// The crate that defines that runtime, which necessarily names it.
const UNATTESTED_RUNTIME_CRATE: &str = "crates/ymp-runtime-fake/";

/// The driver types a module can build a runtime from, beside the crate that defines each one.
const DRIVERS: [(&str, &str); 3] = [
    ("FakeRuntime", "crates/ymp-runtime-fake/"),
    ("CodexRuntime", "crates/ymp-runtime-codex/"),
    ("ClaudeRuntime", "crates/ymp-runtime-claude/"),
];

/// Ways a module starts a runtime or commits what one produced.
const STARTS: [&str; 5] = [
    ".start(",
    "start_prepared(",
    "start_managed_candidate",
    "submit_workspace_candidate",
    "start_unattested_managed_candidate",
];

/// The helpers that end the processes a run started. A result of one of them is the only ground on
/// which a run may say it left nothing running, so no module may throw one away.
const TERMINATION: [&str; 2] = ["terminate_process_tree(", "end_process_tree"];

/// The ways a result is thrown away on the line that produced it.
const DISCARDS: [&str; 2] = ["let _ =", ".ok()"];

/// The gate every such module must call. The call is what is looked for, not the name: a module
/// that imported the gate and never called it would otherwise satisfy this check by its import.
const GATE: &str = "admit_runtime_start(";

/// The commands of the built product that take a runtime, and whether each also takes a workspace
/// and a prompt of its own.
const RUNTIME_COMMANDS: [(&str, bool, bool); 3] = [
    ("runtime-smoke", true, true),
    ("managed-runtime-smoke", true, false),
    ("managed-candidate-smoke", false, false),
];

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

fn names(source: &str, wanted: &str) -> bool {
    code_lines(source).any(|(_, line)| line.contains(wanted))
}

#[test]
fn a_seam_the_checks_use_appears_only_where_it_is_defined() {
    let mut offenders = Vec::new();
    for (name, source) in shipped_sources() {
        for (number, line) in code_lines(&source) {
            for seam in SEAMS {
                // The line that defines the seam necessarily names it. Every other use of it is an
                // offence, in the defining module as much as in any other.
                if line.contains(seam) && !line.contains(&format!("fn {seam}(")) {
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
fn no_shipped_module_names_the_unattested_runtime_beside_a_start() {
    let mut offenders = Vec::new();
    for (name, source) in shipped_sources() {
        if name.starts_with(UNATTESTED_RUNTIME_CRATE) {
            continue;
        }
        if !UNATTESTED_RUNTIME
            .iter()
            .any(|runtime| names(&source, runtime))
        {
            continue;
        }
        for (number, line) in code_lines(&source) {
            for start in STARTS {
                if line.contains(start) {
                    offenders.push(format!("{name}:{number}: {start}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a shipped module that names the unattested runtime also starts a run or commits what one \
         produced:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn every_shipped_module_that_starts_a_runtime_names_the_gate() {
    let mut offenders = Vec::new();
    for (name, source) in shipped_sources() {
        let builds = DRIVERS.iter().any(|(driver, crate_prefix)| {
            !name.starts_with(crate_prefix) && names(&source, driver)
        });
        if !builds || names(&source, GATE) {
            continue;
        }
        for (number, line) in code_lines(&source) {
            if STARTS.iter().any(|start| line.contains(start)) {
                offenders.push(format!("{name}:{number}: starts a runtime without {GATE}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a shipped module builds a runtime driver and starts it without the gate:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn no_shipped_module_discards_a_termination_result() {
    let mut offenders = Vec::new();
    for (name, source) in shipped_sources() {
        for (number, line) in code_lines(&source) {
            if TERMINATION.iter().any(|helper| line.contains(helper))
                && DISCARDS.iter().any(|discard| line.contains(discard))
            {
                offenders.push(format!("{name}:{number}:{}", line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a shipped module throws away what it learned about the processes it started:\n{}",
        offenders.join("\n")
    );
}

/// The scan must read the workspace it claims to read. Without this the checks above could pass
/// because they found nothing at all.
#[test]
fn the_scan_reads_the_shipped_source_of_the_whole_workspace() {
    let sources = shipped_sources();
    assert!(
        sources.len() > 20,
        "the scan found {} modules, which is not this workspace",
        sources.len()
    );
    for (module, wanted) in [
        ("crates/ymp-cli/src/internal.rs", GATE),
        ("crates/ymp-runtime-supervisor/src/lib.rs", GATE),
        ("crates/ymp-runtime-fake/src/lib.rs", "FakeRuntime"),
    ] {
        let (_, source) = sources
            .iter()
            .find(|(name, _)| name == module)
            .unwrap_or_else(|| panic!("{module} was not scanned"));
        assert!(names(source, wanted), "{module} no longer names {wanted}");
    }
    assert!(
        !names(
            "// start_unattested_managed_candidate in a comment\n",
            SEAMS[0]
        ),
        "a comment was read as code"
    );
}

/// Writes the contract package the candidate-producing command requires.
fn contract_package(root: &Path) -> PathBuf {
    let source = root.join("source");
    let negative_control = root.join("negative-control");
    fs::create_dir_all(&source).expect("source");
    fs::create_dir_all(&negative_control).expect("negative control");
    fs::write(source.join("input.txt"), b"before\n").expect("source file");
    let program = root.join("verify.sh");
    fs::write(&program, "#!/bin/sh\nexit 0\n").expect("verifier program");
    let oracle_digest = ymp_domain::digest_bytes(&fs::read(&program).expect("program bytes"));
    let package = root.join("contract.json");
    fs::write(
        &package,
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 2,
            "contract_id": "contract-unattested-surface",
            "source": "source",
            "prompt": "assemble a candidate",
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
fn the_shipped_surface_offers_no_runtime_the_gate_would_refuse() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let package = contract_package(temporary.path());
    for (index, (command, takes_workspace, takes_prompt)) in RUNTIME_COMMANDS.iter().enumerate() {
        let data_root = temporary.path().join(format!("data-{index}"));
        let mut invocation = Command::new(env!("CARGO_BIN_EXE_ymp"));
        invocation
            .arg("--data-root")
            .arg(&data_root)
            .arg("--contract")
            .arg(&package)
            .args(["internal", command])
            .args(["--runtime", "fake"]);
        if *takes_workspace {
            invocation
                .arg("--workspace")
                .arg(temporary.path().join("workspace"));
        }
        if *takes_prompt {
            invocation.args(["--prompt", "assemble a candidate"]);
        }
        let outcome = invocation.output().expect("run the built product");
        let diagnostic = String::from_utf8_lossy(&outcome.stderr).into_owned();
        assert!(
            !outcome.status.success(),
            "{command} accepted the unattested runtime: {diagnostic}"
        );
        assert!(
            diagnostic.contains("invalid value 'fake'"),
            "{command} refused the unattested runtime for another reason: {diagnostic}"
        );
        assert!(
            !data_root.join("candidates").exists(),
            "{command} assembled a candidate with the unattested runtime"
        );

        let help = Command::new(env!("CARGO_BIN_EXE_ymp"))
            .args(["internal", command])
            .arg("--help")
            .output()
            .expect("read the command's own description of its values");
        let offered = String::from_utf8_lossy(&help.stdout).into_owned();
        assert!(
            offered.contains("codex") && !offered.contains("fake"),
            "{command} still offers the unattested runtime: {offered}"
        );
    }
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

/// The negative control for the refusal above: a runtime whose launch is attested passes the gate
/// and is stopped by its own probe, so the refusal is about attestation rather than about starting.
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

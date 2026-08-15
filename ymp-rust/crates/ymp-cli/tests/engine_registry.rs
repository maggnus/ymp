#![forbid(unsafe_code)]

//! Acceptance: runtime engines are managed entities, and the decision about them is durable.
//!
//! The registry under the product root holds one record per engine — the enabled flag with the
//! reason it carries, the properties a measurement wrote, and the models the engine can serve.
//! What this check drives is the built executable, so what it establishes is what an operator
//! gets: the decision survives the process that made it, the mirrored command reaches the same
//! registry the interface does, and a disabled engine is neither probed nor offered.
//!
//! The Claude engine is the one this host can start, so it is the one enabled and disabled here.
//! Codex is the engine the owner held back, and its seeded record is read rather than written.

use std::path::{Path, PathBuf};
use std::process::{Command as Process, Output};

use serde_json::Value;
use tempfile::TempDir;

fn ymp(root: &Path, arguments: &[&str]) -> Output {
    Process::new(env!("CARGO_BIN_EXE_ymp"))
        .arg("--root")
        .arg(root)
        .args(arguments)
        .output()
        .expect("run the ymp executable")
}

fn stated(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn record(root: &Path, engine: &str) -> Value {
    let path = root.join("runtimes").join(format!("{engine}.json"));
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("read the {engine} record at {}: {error}", path.display()));
    serde_json::from_slice(&bytes).expect("the record is readable JSON")
}

fn root() -> (TempDir, PathBuf) {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().join("root");
    (directory, root)
}

/// The engine the owner held back is disabled where no operator has stated otherwise, and the
/// reason it carries is the one an operator reads.
#[test]
fn the_registry_seeds_the_held_back_engine_as_disabled_with_its_reason() {
    let (_directory, root) = root();
    let disabled = ymp(
        &root,
        &["runtime", "disable", "claude-code", "--reason", "kept out"],
    );
    assert!(disabled.status.success(), "{}", stated(&disabled));

    // Reading the page now probes nothing: both engines are disabled, so the page is composed from
    // the registry alone. The Codex record was seeded rather than written by an operator.
    let page = ymp(&root, &["show", "runtimes"]);
    assert!(page.status.success(), "{}", stated(&page));
    let shown = stated(&page);
    assert!(shown.contains("usage limit until 2026-09-12"), "{shown}");
    assert!(shown.contains("kept out"), "{shown}");

    let codex = record(&root, "codex");
    assert_eq!(codex["enabled"], Value::Bool(false));
    assert_eq!(codex["disabled_reason"], "usage limit until 2026-09-12");
}

/// The mirrored command reaches the registry the interface reads, and the decision it takes is
/// durable: a later process — the one that draws the page — reads it back.
#[test]
fn the_mirrored_command_disables_and_enables_an_engine_durably() {
    let (_directory, root) = root();

    let disabled = ymp(
        &root,
        &[
            "runtime",
            "disable",
            "claude-code",
            "--reason",
            "held back for this check",
        ],
    );
    assert!(disabled.status.success(), "{}", stated(&disabled));
    let said = stated(&disabled);
    assert!(said.contains("held back for this check"), "{said}");
    let stored = record(&root, "claude-code");
    assert_eq!(stored["enabled"], Value::Bool(false));
    assert_eq!(stored["disabled_reason"], "held back for this check");

    // A disabled engine is not offered: the page states it as disabled rather than as ready, and
    // no managed engine is left for a run to be routed to.
    let page = ymp(&root, &["show", "runtimes"]);
    let shown = stated(&page);
    assert!(shown.contains("disabled"), "{shown}");
    assert!(
        shown.contains("· 1 ready · 2 unusable"),
        "a disabled engine was still counted as usable:\n{shown}"
    );

    let enabled = ymp(&root, &["runtime", "enable", "claude-code"]);
    assert!(enabled.status.success(), "{}", stated(&enabled));
    let stored = record(&root, "claude-code");
    assert_eq!(stored["enabled"], Value::Bool(true));
    assert_eq!(stored.get("disabled_reason"), None);
}

/// A name that selects no engine changes nothing and exits non-zero. The command refuses it rather
/// than resolving it to the nearest engine, exactly as the interface refuses the typed line.
#[test]
fn a_name_that_selects_no_engine_changes_nothing_and_exits_non_zero() {
    let (_directory, root) = root();
    ymp(
        &root,
        &["runtime", "disable", "claude-code", "--reason", "kept out"],
    );
    let before = record(&root, "claude-code");

    let refused = ymp(&root, &["runtime", "enable", "claude"]);
    assert!(
        !refused.status.success(),
        "a name that selects no engine was accepted:\n{}",
        stated(&refused)
    );
    let said = stated(&refused);
    assert!(said.contains("claude-code"), "{said}");
    assert_eq!(
        record(&root, "claude-code"),
        before,
        "a refused name still changed a record"
    );
}

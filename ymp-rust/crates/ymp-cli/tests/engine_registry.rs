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

fn ymp_at_store(store: &Path, arguments: &[&str]) -> Output {
    Process::new(env!("CARGO_BIN_EXE_ymp"))
        .arg("--data-root")
        .arg(store)
        .args(arguments)
        .output()
        .expect("run the ymp executable")
}

/// The one store the product addressed under this root.
fn only_store(root: &Path) -> PathBuf {
    let projects = std::fs::read_dir(root.join("projects"))
        .expect("the root addresses a project")
        .flatten()
        .map(|entry| entry.path())
        .next()
        .expect("one project");
    std::fs::read_dir(projects.join("runs"))
        .expect("the project addresses a store")
        .flatten()
        .map(|entry| entry.path())
        .next()
        .expect("one store")
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

    // An engine nothing was measured for carries no history of a list, so the page has the reason
    // to show where a sentence about how the list was recorded would otherwise stand.
    assert!(
        !shown.contains("recorded while the engine was enabled"),
        "an engine with no measured list claimed a history of one:\n{shown}"
    );

    let codex = record(&root, "codex");
    assert_eq!(codex["enabled"], Value::Bool(false));
    assert_eq!(codex["disabled_reason"], "usage limit until 2026-09-12");
    assert_eq!(codex["models"]["note"], Value::Null);
    assert_eq!(record(&root, "claude-code")["models"]["note"], Value::Null);
}

/// A root the operator named is where the decision is recorded and where it is read back.
///
/// This is the second scenario the review measured. A root nested inside another root's projects —
/// which is what an agent workspace under a store holds — was walked past on both sides: the
/// decision was written to the outer root without saying so, and the record standing at the named
/// root was then ignored, so a disabled engine started with a zero exit. Both halves are checked
/// here, and the reply names the file it wrote.
#[test]
fn a_root_named_inside_another_root_records_and_reads_its_own_decision() {
    let (_directory, outer) = root();
    // Materialise the outer root and its projects, so the nested root really does stand inside
    // them rather than in an empty directory that only looks like it.
    let seeded = ymp(&outer, &["runtime", "enable", "claude-code"]);
    assert!(seeded.status.success(), "{}", stated(&seeded));
    let inner = only_store(&outer)
        .join("workspaces")
        .join("w")
        .join("inner");
    std::fs::create_dir_all(&inner).expect("nested root directory");

    let disabled = ymp(
        &inner,
        &[
            "runtime",
            "disable",
            "claude-code",
            "--reason",
            "held back at the nested root",
        ],
    );
    assert!(disabled.status.success(), "{}", stated(&disabled));

    // The reply names the file it wrote, and that file is under the root that was named. The
    // transcript lays a long path out to the width it has, so what is compared is the tail that
    // identifies the record rather than the whole absolute path.
    let said = stated(&disabled);
    let written = inner.join("runtimes").join("claude-code.json");
    assert!(
        said.contains("recorded in") && said.contains("inner/runtimes/claude-code.json"),
        "the reply does not name the record it wrote:\n{said}"
    );
    assert!(written.is_file(), "no record stands at the named root");
    assert_eq!(
        record(&outer, "claude-code")["enabled"],
        Value::Bool(true),
        "the decision was written to the outer root instead of the one that was named"
    );

    // The record standing at the named root is read back, so the engine does not start.
    let workspace = inner.parent().expect("workspace").join("smoke");
    std::fs::create_dir_all(&workspace).expect("smoke workspace");
    let smoke = ymp(
        &inner,
        &[
            "internal",
            "runtime-smoke",
            "--runtime=claude",
            &format!("--workspace={}", workspace.display()),
            "--prompt=say done",
        ],
    );
    assert!(
        !smoke.status.success(),
        "a disabled engine started from the root that held it back:\n{}",
        stated(&smoke)
    );
    assert!(
        stated(&smoke).contains("held back at the nested root"),
        "the refusal does not name the decision that caused it:\n{}",
        stated(&smoke)
    );
}

/// A record decides nothing about whether its own list is current.
///
/// The hostile record here is the one a forger would write: it names the installed release, so a
/// rule that trusted the recorded version would read a foreign list as this build's catalog. What
/// decides instead is the digest of the installed executable, computed at every reading, so the
/// forged list is measured again and replaced. The record is forged in a sandbox root of this
/// check's own; nothing outside it is touched.
#[test]
fn a_forged_record_is_measured_again_instead_of_believed() {
    let (_directory, root) = root();
    let first = ymp(&root, &["show", "runtimes"]);
    assert!(first.status.success(), "{}", stated(&first));
    let measured = record(&root, "claude-code");
    let digest = measured["models"]["measured_for_digest"]
        .as_str()
        .expect("the measurement recorded the digest of the executable it read")
        .to_owned();
    assert!(
        measured["models"]["names"]
            .as_array()
            .expect("a measured list")
            .iter()
            .any(|name| name == "claude-sonnet-5"),
        "the first measurement recorded no sonnet route: {measured}"
    );

    // Forged: the release the record names is the installed one, and the list is another build's.
    forge(
        &root,
        "a-digest-of-some-other-build",
        "claude-from-another-build",
    );
    let refreshed = ymp(&root, &["show", "runtimes"]);
    let shown = stated(&refreshed);
    assert!(
        !shown.contains("claude-from-another-build"),
        "a forged record suppressed its own re-measurement:\n{shown}"
    );
    assert_eq!(
        record(&root, "claude-code")["models"]["measured_for_digest"],
        Value::String(digest.clone()),
        "the re-measurement did not record the digest of the installed executable"
    );

    // Positive half: with the digest left as the executable's own, the recorded list is read and
    // not measured again. That is what makes the refusal above the digest's answer rather than an
    // unconditional re-measurement that would have replaced any list at all.
    forge(&root, &digest, "claude-from-this-build");
    let reread = ymp(&root, &["show", "runtimes"]);
    assert!(
        stated(&reread).contains("claude-from-this-build"),
        "a record whose digest is the installed executable's was measured again:\n{}",
        stated(&reread)
    );
}

/// Write a record naming the installed release and holding one stated model.
fn forge(root: &Path, digest: &str, name: &str) {
    let path = root.join("runtimes").join("claude-code.json");
    let mut stored: Value =
        serde_json::from_slice(&std::fs::read(&path).expect("record bytes")).expect("record");
    stored["models"]["measured_for_digest"] = Value::String(digest.to_owned());
    stored["models"]["names"] = Value::Array(vec![Value::String(name.to_owned())]);
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&stored).expect("record bytes"),
    )
    .expect("write the forged record");
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
    //
    // The count is host-independent because both engines are held back here: Codex is seeded
    // disabled and Claude Code was just disabled above, so nothing is measured and nothing can be
    // ready, whatever this host has installed. The earlier count of one ready profile named the
    // in-process fixture, which is no longer offered as a capability of the product.
    let page = ymp(&root, &["show", "runtimes"]);
    let shown = stated(&page);
    assert!(shown.contains("disabled"), "{shown}");
    assert!(
        shown.contains("· 0 ready · 2 unusable"),
        "a disabled engine was still counted as usable:\n{shown}"
    );

    let enabled = ymp(&root, &["runtime", "enable", "claude-code"]);
    assert!(enabled.status.success(), "{}", stated(&enabled));
    let stored = record(&root, "claude-code");
    assert_eq!(stored["enabled"], Value::Bool(true));
    assert_eq!(stored.get("disabled_reason"), None);
}

/// The decision holds however the invocation addresses the state.
///
/// This is the scenario the review measured: an engine disabled under a root, then reached by
/// naming the store inside that root. The registry a store reaches is derived from the store, so
/// both the operator's commands and the product's own machinery read the one decision the root
/// holds, and neither starts a process the operator held back.
#[test]
fn addressing_the_store_instead_of_the_root_reaches_the_same_decision() {
    let (_directory, root) = root();
    let disabled = ymp(
        &root,
        &[
            "runtime",
            "disable",
            "claude-code",
            "--reason",
            "held back under the root",
        ],
    );
    assert!(disabled.status.success(), "{}", stated(&disabled));

    let store = only_store(&root);
    let workspace = root.parent().expect("the root has a parent").join("work");
    std::fs::create_dir_all(&workspace).expect("smoke workspace");
    let smoke_arguments = [
        "internal".to_owned(),
        "runtime-smoke".to_owned(),
        "--runtime=claude".to_owned(),
        format!("--workspace={}", workspace.display()),
        "--prompt=say done".to_owned(),
    ];
    let smoke_arguments: Vec<&str> = smoke_arguments.iter().map(String::as_str).collect();
    for arguments in [smoke_arguments.clone(), vec!["show", "runtimes"]] {
        let named_store = ymp_at_store(&store, &arguments);
        let said = stated(&named_store);
        assert!(
            said.contains("held back under the root"),
            "`{}` addressed a registry the root does not hold:\n{said}",
            arguments.join(" ")
        );
    }
    // The machinery refuses rather than starting the engine, and it says why.
    let smoke = ymp_at_store(&store, &smoke_arguments);
    assert!(
        !smoke.status.success(),
        "a held-back engine was started by addressing the store:\n{}",
        stated(&smoke)
    );
    assert!(
        !store.join("runtimes").exists(),
        "addressing the store wrote a registry of its own beside it"
    );

    // Positive half: admitting the engine under the root admits it for the store too, so what the
    // refusal above answered was the decision and not the way the invocation was written. This is
    // read rather than started — the point is which registry the store reaches, and starting an
    // engine to learn that would spend the operator's budget to answer a question about a file.
    let enabled = ymp(&root, &["runtime", "enable", "claude-code"]);
    assert!(enabled.status.success(), "{}", stated(&enabled));
    let page = ymp_at_store(&store, &["show", "runtimes"]);
    let shown = stated(&page);
    assert!(
        !shown.contains("held back under the root"),
        "an admitted engine was still held back for the store:\n{shown}"
    );
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

//! Acceptance: one root addresses many projects and many runs, and every name in it is derived
//! rather than supplied.
//!
//! The negative half of each check is stated beside it: a second run that would need an operator
//! to name a directory, two projects that would share one store, a root that is really a store,
//! and a root written by a layout this build does not read.

use std::fs;
use std::path::Path;

use tempfile::TempDir;
use ymp_storage::{DataRoot, RootError, StoreIntent};

/// Two runs of one project are two stores under one root, and neither name came from anyone.
#[test]
fn a_second_run_of_one_project_is_addressed_without_a_stated_name() {
    let host = TempDir::new().expect("temporary host");
    let project = directory(host.path(), "project");
    let root = project.join(".ymp");

    let first = open(&root, &project)
        .store(StoreIntent::New)
        .expect("first store");
    start(&first);
    let second = open(&root, &project)
        .store(StoreIntent::New)
        .expect("second store");

    assert_ne!(first, second, "the second run reused the first store");
    assert!(first.starts_with(&root) && second.starts_with(&root));
    assert_eq!(
        first.parent(),
        second.parent(),
        "the two runs left one project"
    );
    assert_eq!(
        vec!["0001", "0002"],
        names(first.parent().expect("runs directory")),
        "the layout wrote a run name this build does not derive"
    );

    // The negative half: without a run committed into it, the addressed store is where the next
    // run belongs. An empty directory per invocation is exactly the litter this layout removes.
    let third = open(&root, &project)
        .store(StoreIntent::New)
        .expect("third store");
    assert_eq!(
        second, third,
        "an unused store was abandoned rather than used"
    );
}

/// Reuse of a store nothing was started into is not a claim, and the layout says so.
///
/// Two invocations that reach such a store are given the same directory. That is what the writer
/// lock is for: the second one is refused loudly instead of being handed a run of its own. The
/// negative half is the ordinal past a committed run, which is claimed rather than shared.
#[test]
fn a_store_nothing_was_started_into_is_shared_rather_than_claimed() {
    let host = TempDir::new().expect("temporary host");
    let project = directory(host.path(), "project");
    let root = project.join(".ymp");

    let first = open(&root, &project)
        .store(StoreIntent::New)
        .expect("first store");
    let second = open(&root, &project)
        .store(StoreIntent::New)
        .expect("second store");
    assert_eq!(
        first, second,
        "the layout separated two starts over one empty store; the writer lock is what separates \
         them, and the layout must not claim otherwise"
    );

    // The negative half: once a run is committed, the next start is given a directory of its own
    // rather than the one holding that run.
    start(&first);
    let third = open(&root, &project)
        .store(StoreIntent::New)
        .expect("third store");
    assert_ne!(
        first, third,
        "a start was given the store that already holds a run"
    );
}

/// A project directory the root materialized carries the marker naming what it stands for, on
/// every path that materializes it — a reader's included.
#[test]
fn a_materialized_project_directory_carries_its_marker() {
    let host = TempDir::new().expect("temporary host");
    let project = directory(host.path(), "project");
    let root = project.join(".ymp");

    let addressed = open(&root, &project);
    addressed
        .store(StoreIntent::Current)
        .expect("a reader addresses a store");

    let directory = addressed
        .runs_directory()
        .parent()
        .expect("the project directory")
        .to_path_buf();
    assert!(
        directory.is_dir(),
        "the reader named a store without materializing its project"
    );
    let marker = directory.join("project.json");
    assert!(
        marker.is_file(),
        "a read left the project directory without its marker: {:?}",
        names(&directory)
    );

    let recorded: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker).expect("marker bytes")).expect("readable marker");
    assert_eq!(
        Some(project.to_string_lossy().as_ref()),
        recorded
            .get("project_path")
            .and_then(|value| value.as_str()),
        "the marker names a directory other than the one it stands for"
    );
}

/// A reader acts on the run the project is on, never on an empty store beside it.
#[test]
fn a_reader_is_addressed_to_the_run_the_project_is_on() {
    let host = TempDir::new().expect("temporary host");
    let project = directory(host.path(), "project");
    let root = project.join(".ymp");

    let fresh = open(&root, &project)
        .store(StoreIntent::Current)
        .expect("first reader");
    assert!(
        !fresh.exists(),
        "reading a project with no run created a store"
    );

    let first = open(&root, &project)
        .store(StoreIntent::New)
        .expect("first store");
    start(&first);
    assert_eq!(
        first,
        open(&root, &project)
            .store(StoreIntent::Current)
            .expect("reader")
    );

    let second = open(&root, &project)
        .store(StoreIntent::New)
        .expect("second store");
    start(&second);
    assert_eq!(
        second,
        open(&root, &project)
            .store(StoreIntent::Current)
            .expect("reader"),
        "a reader was left on the run before the one just started"
    );
}

/// Two projects sharing one root keep disjoint state, including two projects whose directories
/// carry the same name.
#[test]
fn two_projects_under_one_root_keep_disjoint_state() {
    let host = TempDir::new().expect("temporary host");
    let root = directory(host.path(), "root");
    let first = directory(&directory(host.path(), "work"), "ymp");
    let second = directory(&directory(host.path(), "archive"), "ymp");

    let first_store = open(&root, &first)
        .store(StoreIntent::New)
        .expect("first store");
    let second_store = open(&root, &second)
        .store(StoreIntent::New)
        .expect("second store");

    assert_ne!(
        open(&root, &first).project_segment(),
        open(&root, &second).project_segment(),
        "two projects named ymp were addressed as one"
    );
    assert!(!first_store.starts_with(&second_store));
    assert!(!second_store.starts_with(&first_store));
    assert_eq!(
        2,
        names(&root.join("projects")).len(),
        "one root did not hold two projects"
    );
}

/// A store written by the earlier layout is read where it stands or refused with the reason
/// named; it is never opened as if it were a root.
#[test]
fn a_store_is_never_opened_as_a_root() {
    let host = TempDir::new().expect("temporary host");
    let project = directory(host.path(), "project");
    let legacy = directory(&project, ".ymp-data");
    start(&legacy);

    let error =
        DataRoot::open_for_project(&legacy, &project).expect_err("a store was accepted as a root");
    assert!(
        matches!(error, RootError::StoreAsRoot { .. }),
        "unexpected refusal: {error}"
    );
    assert!(
        error.to_string().contains("--data-root"),
        "the refusal does not name how the store is read where it stands: {error}"
    );
    assert_eq!(
        vec!["events.jsonl"],
        names(&legacy),
        "the refused store was written into"
    );
}

/// A root this build does not read is refused rather than reinterpreted.
#[test]
fn a_root_of_another_layout_version_is_refused() {
    let host = TempDir::new().expect("temporary host");
    let project = directory(host.path(), "project");
    let root = directory(&project, ".ymp");
    fs::write(
        root.join("root.json"),
        br#"{"schema_version":99,"kind":"ymp-root"}"#,
    )
    .expect("marker of another version");

    let error =
        DataRoot::open_for_project(&root, &project).expect_err("a later layout was accepted");
    assert!(
        matches!(error, RootError::UnsupportedLayout { found: 99, .. }),
        "unexpected refusal: {error}"
    );

    // The negative half: the marker this build writes is accepted, so the refusal above is about
    // the version rather than about reading the marker at all.
    fs::write(
        root.join("root.json"),
        br#"{"schema_version":1,"kind":"ymp-root"}"#,
    )
    .expect("marker of this version");
    DataRoot::open_for_project(&root, &project).expect("this build's own root");
}

/// The default root stands at the operator's home, so what an earlier build left beside a project
/// is refused rather than found: both the store the earliest builds wrote there and the root the
/// build before this one wrote there.
#[test]
fn state_an_earlier_build_left_beside_the_project_is_refused_by_the_default_root() {
    let host = TempDir::new().expect("temporary host");
    let project = directory(host.path(), "project");
    let root = directory(host.path(), "home").join(".ymp");

    // Nothing beside the project: the default begins.
    DataRoot::refuse_earlier_layout_beside(&project, &root).expect("a clean project begins");

    // The root the previous default wrote beside the project.
    let earlier = directory(&project, ".ymp");
    fs::write(
        earlier.join("root.json"),
        br#"{"schema_version":1,"kind":"ymp-root"}"#,
    )
    .expect("the earlier root marker");
    let error = DataRoot::refuse_earlier_layout_beside(&project, &root)
        .expect_err("the default began beside a root an earlier build wrote");
    assert!(
        matches!(error, RootError::EarlierRoot { .. }),
        "unexpected refusal: {error}"
    );
    let reason = error.to_string();
    assert!(
        reason.contains(".ymp") && reason.contains("--root"),
        "the refusal does not name the root and both ways to proceed: {reason}"
    );

    // A root does not refuse itself: an environment that points the default back at that same
    // directory has named it deliberately.
    DataRoot::refuse_earlier_layout_beside(&project, &earlier)
        .expect("the addressed root refused itself");

    // The store the earliest builds wrote beside the project, named as a store rather than a root.
    let legacy = directory(&project, ".ymp-data");
    start(&legacy);
    let error = DataRoot::refuse_earlier_layout_beside(&project, &earlier)
        .expect_err("the default began beside a store the earliest layout wrote");
    let reason = error.to_string();
    assert!(
        matches!(error, RootError::LegacyStore { .. })
            && reason.contains(".ymp-data")
            && reason.contains("--data-root"),
        "the refusal does not name the store and how it is read where it stands: {reason}"
    );

    // Neither was written into, and neither was copied anywhere.
    assert_eq!(vec!["root.json"], names(&earlier));
    assert_eq!(vec!["events.jsonl"], names(&legacy));
}

fn open(root: &Path, project: &Path) -> DataRoot {
    DataRoot::open_for_project(root, project).expect("open the root")
}

fn directory(parent: &Path, name: &str) -> std::path::PathBuf {
    let path = parent.join(name);
    fs::create_dir_all(&path).expect("directory");
    fs::canonicalize(&path).expect("canonical directory")
}

/// The one file that makes a directory a store holding a run.
fn start(store: &Path) {
    fs::create_dir_all(store).expect("store directory");
    fs::write(store.join("events.jsonl"), b"{}\n").expect("journal");
}

fn names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .expect("read the directory")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name != "project.json")
        .collect();
    names.sort();
    names
}

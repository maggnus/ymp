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

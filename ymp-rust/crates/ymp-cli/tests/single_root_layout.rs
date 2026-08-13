//! Acceptance: everything the product writes in the directory it was started from lives under one
//! `.ymp` root, and that root carries many projects and many runs without a directory name the
//! operator had to invent.
//!
//! Every check here drives the built executable from a project directory, exactly as an operator
//! does, and states its negative half against the same executable:
//!
//! * two runs of one project land in two stores under one root — against one named store, where
//!   the second run is refused;
//! * two projects keep disjoint state — proved by driving both and diffing what each tree records;
//! * nothing is written beside the root in the launch directory — against a named store, which is
//!   the sibling directory this layout removes;
//! * a store written by the earlier layout is read where it stands or refused with the reason
//!   named, and never copied.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Two runs of one project
// ---------------------------------------------------------------------------

#[test]
fn two_runs_of_one_project_land_in_two_stores_under_one_root() {
    let host = Host::new();
    let project = host.project("one-project");

    host.start(&project, &[], "first").assert_started();
    host.start(&project, &[], "second").assert_started();

    let stores = host.stores(&project);
    assert_eq!(
        2,
        stores.len(),
        "two runs did not produce two stores; the root holds {:?}",
        relative_paths(&project.join(".ymp"))
    );
    assert_eq!(
        vec!["0001", "0002"],
        stores
            .iter()
            .map(|store| file_name(store))
            .collect::<Vec<_>>(),
        "a store carries a name this layout does not derive"
    );
    assert_eq!(
        stores[0].parent(),
        stores[1].parent(),
        "the two runs of one project were filed under two projects"
    );

    let identifiers: BTreeSet<String> = stores.iter().map(|store| run_id(store)).collect();
    assert_eq!(
        2,
        identifiers.len(),
        "the two stores record one run: {identifiers:?}"
    );
}

/// The negative half: one named store holds one run, so the second start is refused there. That
/// refusal is what made an operator invent a directory name per run.
#[test]
fn a_second_run_is_refused_by_the_one_store_an_operator_names() {
    let host = Host::new();
    let project = host.project("named-store");
    let store = project.join("named");

    let named = ["--data-root".to_owned(), store.display().to_string()];
    host.start(&project, &named, "first").assert_started();
    let refused = host.start(&project, &named, "second");

    assert!(
        !refused.output.status.success(),
        "a second run was committed into a store that already holds one"
    );
    assert!(
        refused.text().contains("a second run needs its own store"),
        "the refusal does not state why: {}",
        refused.text()
    );
    assert_eq!(
        1,
        journals(&store).len(),
        "the refused run wrote a second journal into the named store"
    );
}

// ---------------------------------------------------------------------------
// Two projects on one host
// ---------------------------------------------------------------------------

#[test]
fn two_projects_on_one_host_keep_disjoint_state() {
    let host = Host::new();
    let first = host.project("first-project");
    let second = host.project("second-project");

    host.start(&first, &[], "work-of-the-first")
        .assert_started();
    host.start(&second, &[], "work-of-the-second")
        .assert_started();

    let first_store = one_store(&host, &first);
    let second_store = one_store(&host, &second);
    assert!(!first_store.starts_with(&second_store));
    assert!(!second_store.starts_with(&first_store));

    // The trees are diffed by what they record, not only by where they sit: each project's tree
    // names its own run and nothing of the other's.
    let (first_id, second_id) = (run_id(&first_store), run_id(&second_store));
    assert_ne!(first_id, second_id, "two projects recorded one run");
    let first_tree = fs::read_to_string(first_store.join("events.jsonl")).expect("first journal");
    let second_tree =
        fs::read_to_string(second_store.join("events.jsonl")).expect("second journal");
    assert!(first_tree.contains(&first_id) && !first_tree.contains(&second_id));
    assert!(second_tree.contains(&second_id) && !second_tree.contains(&first_id));

    // Each project addressed its own root here. The same two projects under one shared root are
    // held apart by the project segment, which is derived from the directory rather than stated.
    let shared = host.host.path().join("shared-root");
    let named = |root: &Path| ["--root".to_owned(), root.display().to_string()];
    host.start(&first, &named(&shared), "shared-first")
        .assert_started();
    host.start(&second, &named(&shared), "shared-second")
        .assert_started();
    let segments = relative_paths(&shared.join("projects"))
        .into_iter()
        .filter(|path| !path.contains('/'))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        2,
        segments.len(),
        "one root filed two projects under {segments:?}"
    );
}

// ---------------------------------------------------------------------------
// Nothing beside the root
// ---------------------------------------------------------------------------

#[test]
fn nothing_is_written_beside_the_root_in_the_launch_directory() {
    let host = Host::new();
    let project = host.project("clean-launch");

    host.start(&project, &[], "first").assert_started();
    host.start(&project, &[], "second").assert_started();
    host.command(&project, &["show".to_owned(), "events".to_owned()]);

    let written = relative_paths(&project);
    assert!(!written.is_empty(), "the runs wrote nothing at all");
    let outside: Vec<&String> = written
        .iter()
        .filter(|path| *path != ".ymp" && !path.starts_with(".ymp/"))
        .collect();
    assert!(
        outside.is_empty(),
        "the product wrote outside the root: {outside:?}"
    );

    // The negative half: a named store is the sibling directory this layout removes, and today's
    // default named exactly one such sibling.
    let sibling = host.project("named-sibling");
    host.start(
        &sibling,
        &["--data-root".to_owned(), ".ymp-data".to_owned()],
        "first",
    )
    .assert_started();
    let beside = relative_paths(&sibling)
        .into_iter()
        .filter(|path| !path.starts_with(".ymp/") && path != ".ymp")
        .collect::<Vec<_>>();
    assert!(
        beside.iter().any(|path| path.starts_with(".ymp-data")),
        "the named store did not write beside the root: {beside:?}"
    );
}

// ---------------------------------------------------------------------------
// A store written by the earlier layout
// ---------------------------------------------------------------------------

#[test]
fn a_store_of_the_earlier_layout_is_read_where_it_stands_and_never_copied() {
    let host = Host::new();
    let project = host.project("earlier-layout");
    let legacy = project.join(".ymp-data");

    host.start(
        &project,
        &["--data-root".to_owned(), ".ymp-data".to_owned()],
        "earlier work",
    )
    .assert_started();
    let before = fs::read_to_string(legacy.join("events.jsonl")).expect("the earlier journal");

    // The default root refuses to begin beside it, and names both ways out.
    let refused = host.command(&project, &["show".to_owned(), "events".to_owned()]);
    assert!(
        !refused.output.status.success(),
        "the product began beside a store of the earlier layout"
    );
    let reason = refused.text();
    assert!(
        reason.contains(".ymp-data") && reason.contains("--data-root") && reason.contains("--root"),
        "the refusal does not name the store and both ways to proceed: {reason}"
    );
    assert!(
        !project.join(".ymp").exists(),
        "the refusal created the root it refused to begin"
    );

    // Read where it stands: the earlier store answers, unchanged.
    let read = host.command(
        &project,
        &[
            "--data-root".to_owned(),
            ".ymp-data".to_owned(),
            "show".to_owned(),
            "events".to_owned(),
        ],
    );
    assert!(
        read.output.status.success(),
        "the earlier store was not read: {}",
        read.text()
    );
    assert!(
        read.text().contains("run.started"),
        "reading the earlier store showed no run: {}",
        read.text()
    );

    // Naming the new root proceeds and leaves the earlier store alone; nothing was copied into
    // the new root, which holds no run until one is started there.
    let named = host.command(
        &project,
        &[
            "--root".to_owned(),
            ".ymp".to_owned(),
            "show".to_owned(),
            "events".to_owned(),
        ],
    );
    assert!(
        !named.output.status.success(),
        "a root holding no run reported a page it does not have"
    );
    assert!(
        journals(&project.join(".ymp")).is_empty(),
        "the earlier store was copied into the new root"
    );
    assert_eq!(
        before,
        fs::read_to_string(legacy.join("events.jsonl")).expect("the earlier journal"),
        "the earlier store was written into"
    );

    // Once the root is declared, the default invocation no longer asks the question again.
    let again = host.command(&project, &["show".to_owned(), "events".to_owned()]);
    assert!(
        !again.text().contains("earlier layout"),
        "the declared root still refuses to begin: {}",
        again.text()
    );
}

// ---------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------

struct Host {
    host: TempDir,
    source: PathBuf,
    verifier: PathBuf,
    negative_control: PathBuf,
}

struct Run {
    output: Output,
}

impl Run {
    fn text(&self) -> String {
        format!(
            "{}{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        )
    }

    fn assert_started(&self) {
        assert!(
            self.output.status.success(),
            "the run was not started: {}",
            self.text()
        );
    }
}

impl Host {
    fn new() -> Self {
        let host = TempDir::new().expect("temporary host");
        let source = host.path().join("source");
        let negative_control = host.path().join("negative-control");
        let verifier = host.path().join("verify.sh");
        fs::create_dir_all(&source).expect("source directory");
        fs::create_dir_all(&negative_control).expect("negative control directory");
        fs::write(source.join("input.txt"), b"before\n").expect("source file");
        fs::write(&verifier, b"#!/bin/sh\nexit 0\n").expect("verifier program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&verifier).expect("metadata").permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&verifier, permissions).expect("make executable");
        }
        Self {
            host,
            source,
            verifier,
            negative_control,
        }
    }

    /// An empty directory an operator would start the product in.
    fn project(&self, name: &str) -> PathBuf {
        let path = self.host.path().join("projects").join(name);
        fs::create_dir_all(&path).expect("project directory");
        fs::canonicalize(&path).expect("canonical project directory")
    }

    /// A contract package under a stated prompt, so two starts of one project are two runs.
    fn package(&self, prompt: &str) -> PathBuf {
        let path = self.host.path().join(format!("{}.json", digest(prompt)));
        let oracle = ymp_domain::digest_bytes(&fs::read(&self.verifier).expect("verifier bytes"));
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 2,
                "contract_id": digest(prompt),
                "source": self.source,
                "prompt": prompt,
                "verifier": {
                    "program": self.verifier,
                    "negative_control": self.negative_control,
                    "oracle_digest": oracle,
                    "wall_time_ms": 60_000,
                    "output_limit_bytes": 1024
                }
            }))
            .expect("package bytes"),
        )
        .expect("write the package");
        path
    }

    /// The executable, started in a project directory as an operator starts it.
    fn command(&self, project: &Path, arguments: &[String]) -> Run {
        let output = Command::new(env!("CARGO_BIN_EXE_ymp"))
            .current_dir(project)
            .args(arguments)
            .output()
            .expect("run the ymp executable");
        Run { output }
    }

    /// Start one run of one contract, through the confirmation the interface requires.
    fn start(&self, project: &Path, before: &[String], prompt: &str) -> Run {
        let package = self.package(prompt);
        let contract_id = digest(prompt);
        let mut arguments = before.to_vec();
        arguments.extend([
            "--contract".to_owned(),
            package.display().to_string(),
            "start".to_owned(),
            contract_id.clone(),
            format!("--confirm={contract_id}"),
        ]);
        self.command(project, &arguments)
    }

    /// Every store the root holds for this project, in the order they were claimed.
    fn stores(&self, project: &Path) -> Vec<PathBuf> {
        let mut stores = journals(&project.join(".ymp"));
        stores.sort();
        stores
    }
}

fn one_store(host: &Host, project: &Path) -> PathBuf {
    let stores = host.stores(project);
    assert_eq!(1, stores.len(), "expected one store, found {stores:?}");
    stores.into_iter().next().expect("one store")
}

fn digest(prompt: &str) -> String {
    format!(
        "contract-{}",
        &ymp_domain::digest_bytes(prompt.as_bytes())[..8]
    )
}

fn run_id(store: &Path) -> String {
    let bytes = fs::read(store.join("run.json")).expect("the run projection");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("readable projection");
    value
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .expect("the projection names its run")
        .to_owned()
}

/// Every store under a tree, named by the directory holding its journal.
fn journals(tree: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(tree, &mut |path| {
        if path.file_name().is_some_and(|name| name == "events.jsonl") {
            found.push(
                path.parent()
                    .expect("a journal has a directory")
                    .to_path_buf(),
            );
        }
    });
    found
}

/// Every path a tree holds, relative to it, with `/` as the separator.
fn relative_paths(tree: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    walk(tree, &mut |path| {
        if let Ok(relative) = path.strip_prefix(tree) {
            paths.push(relative.to_string_lossy().replace('\\', "/"));
        }
    });
    paths.sort();
    paths
}

fn walk(tree: &Path, visit: &mut impl FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(tree) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        visit(&path);
        if path.is_dir() {
            walk(&path, visit);
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .expect("a named directory")
        .to_string_lossy()
        .into_owned()
}

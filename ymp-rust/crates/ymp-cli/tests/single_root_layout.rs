//! Acceptance: everything the product writes lives under one root, and that root carries many
//! projects and many runs without a directory name the operator had to invent.
//!
//! The root itself stands at the operator's home rather than beside the project;
//! `state_lives_under_the_home_root.rs` owns that outcome and the launch directory it leaves
//! empty. What is checked here is the layout inside the root, whichever directory it stands in.
//!
//! Every check here drives the built executable from a project directory, exactly as an operator
//! does, and states its negative half against the same executable:
//!
//! * two runs of one project land in two stores under one root — against one named store, where
//!   the second run is refused;
//! * two projects keep disjoint state under the one root they share — proved by driving both and
//!   diffing what each tree records;
//! * state an earlier build wrote beside the project is read where it stands or refused with the
//!   reason named, and never copied.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

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
        relative_paths(&host.root())
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

/// Several starts at once over a project holding no run reach one empty store together, because
/// a store nothing was started into is reused rather than claimed. The layout does not separate
/// them; the writer lock does, and it refuses loudly. Whatever the interleaving, no store ends up
/// holding two runs and no start fails without saying why.
///
/// The negative half is the claim this replaces: the layout was documented as separating every
/// pair of concurrent starts by giving them different ordinals. Driving three at once over an
/// empty project shows they are given one instead, and the sharing itself is pinned without any
/// timing by the `ymp-storage` check that two consecutive starts over an empty store are given
/// the same directory.
#[test]
fn concurrent_starts_over_an_empty_store_never_commit_two_runs_into_one() {
    const STARTS: usize = 3;
    let host = Host::new();
    let project = host.project("concurrent-start");

    // Every package is written before any process exists, so the processes overlap on the store
    // rather than on the fixture.
    let invocations: Vec<Vec<String>> = (0..STARTS)
        .map(|index| host.start_arguments(&[], &format!("concurrent work {index}")))
        .collect();
    let running: Vec<Child> = invocations
        .iter()
        .map(|arguments| host.spawn(&project, arguments))
        .collect();
    let outcomes: Vec<Run> = running
        .into_iter()
        .map(|child| Run {
            output: child.wait_with_output().expect("collect the invocation"),
        })
        .collect();

    let started = outcomes
        .iter()
        .filter(|outcome| outcome.output.status.success())
        .count();
    assert!(started >= 1, "no start committed a run at all");

    let stores = host.stores(&project);
    assert_eq!(
        started,
        stores.len(),
        "{started} starts committed runs into {} stores",
        stores.len()
    );
    for store in &stores {
        let recorded = runs_recorded(store);
        assert_eq!(
            1,
            recorded.len(),
            "one store records more than one run: {recorded:?}"
        );
    }

    for refused in outcomes
        .iter()
        .filter(|outcome| !outcome.output.status.success())
    {
        let reason = refused.text();
        assert!(
            reason.contains("data root is already owned by another foreground process")
                || reason.contains("a second run needs its own store"),
            "a start failed without naming the store it lost: {reason}"
        );
    }
}

/// A read in a project holding no run materializes that project's directory, and a directory the
/// root materialized carries the marker naming what it stands for.
#[test]
fn a_read_leaves_no_project_directory_without_its_marker() {
    let host = Host::new();
    let project = host.project("read-only");

    host.command(&project, &["show".to_owned(), "events".to_owned()]);

    let written = relative_paths(&host.root());
    let markers: Vec<&String> = written
        .iter()
        .filter(|path| path.ends_with("/project.json"))
        .collect();
    assert_eq!(
        1,
        markers.len(),
        "the read materialized a project directory without its marker: {written:?}"
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

    // Both projects addressed the one root the host carries, and are held apart inside it by the
    // project segment, which is derived from the directory rather than stated.
    let segments = relative_paths(&host.root().join("projects"))
        .into_iter()
        .filter(|path| !path.contains('/'))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        2,
        segments.len(),
        "one root filed two projects under {segments:?}"
    );

    // The negative half: a root named per project keeps them apart by standing apart, which is
    // what made an operator name a directory in the first place.
    let apart = host.host.path().join("root-of-its-own");
    host.start(
        &first,
        &["--root".to_owned(), apart.display().to_string()],
        "apart",
    )
    .assert_started();
    assert_eq!(
        1,
        relative_paths(&apart.join("projects"))
            .into_iter()
            .filter(|path| !path.contains('/'))
            .count(),
        "a root named for one project holds another project's state"
    );
}

// ---------------------------------------------------------------------------
// State an earlier build wrote beside the project
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
        "the refusal created a root beside the store it refused to begin"
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

    // Naming a root proceeds and leaves the earlier store alone; nothing was copied into it, and
    // it holds no run until one is started there.
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
        "the earlier store was copied into the named root"
    );
    assert_eq!(
        before,
        fs::read_to_string(legacy.join("events.jsonl")).expect("the earlier journal"),
        "the earlier store was written into"
    );

    // The root that invocation named is itself state beside the project, so the default keeps
    // refusing rather than adopting it — and names it, not only the store, as the thing it found.
    let again = host.command(&project, &["show".to_owned(), "events".to_owned()]);
    assert!(
        !again.output.status.success() && again.text().contains(".ymp"),
        "the default adopted state standing beside the project: {}",
        again.text()
    );
}

// ---------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------

struct Host {
    host: TempDir,
    /// The home directory every invocation of this fixture sees. The default root stands in it, so
    /// the check drives the default the operator has without reaching the home this test runs in.
    home: PathBuf,
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
        let home = host.path().join("home");
        let source = host.path().join("source");
        let negative_control = host.path().join("negative-control");
        let verifier = host.path().join("verify.sh");
        fs::create_dir_all(&home).expect("home directory");
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
            home,
            source,
            verifier,
            negative_control,
        }
    }

    /// The root the default addresses for every invocation of this fixture.
    fn root(&self) -> PathBuf {
        self.home.join(".ymp")
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

    /// The executable, started in a project directory as an operator starts it, with a home
    /// directory of the fixture's own.
    fn command(&self, project: &Path, arguments: &[String]) -> Run {
        let output = self
            .invocation(project, arguments)
            .output()
            .expect("run the ymp executable");
        Run { output }
    }

    fn invocation(&self, project: &Path, arguments: &[String]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ymp"));
        command
            .current_dir(project)
            .env("HOME", &self.home)
            .env_remove("YMP_HOME")
            .args(arguments);
        command
    }

    /// Start one run of one contract, through the confirmation the interface requires.
    fn start(&self, project: &Path, before: &[String], prompt: &str) -> Run {
        let arguments = self.start_arguments(before, prompt);
        self.command(project, &arguments)
    }

    /// The invocation that starts one run, prepared before any process is spawned.
    fn start_arguments(&self, before: &[String], prompt: &str) -> Vec<String> {
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
        arguments
    }

    /// The executable, left running so several invocations overlap.
    fn spawn(&self, project: &Path, arguments: &[String]) -> Child {
        self.invocation(project, arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn the ymp executable")
    }

    /// Every store the root holds for this project, in the order they were claimed.
    ///
    /// The root carries every project of this host, so the stores are selected by the marker each
    /// project directory carries — the same statement an operator reads the tree by.
    fn stores(&self, project: &Path) -> Vec<PathBuf> {
        let mut stores = journals(&project_directory(&self.root(), project));
        stores.sort();
        stores
    }
}

/// The directory this root holds one project's runs under, found by the marker that names it.
fn project_directory(root: &Path, project: &Path) -> PathBuf {
    let projects = root.join("projects");
    let stated = project.to_string_lossy().into_owned();
    let Ok(entries) = fs::read_dir(&projects) else {
        return projects.join("no-project-directory");
    };
    for entry in entries.flatten() {
        let marker = entry.path().join("project.json");
        let Ok(bytes) = fs::read(&marker) else {
            continue;
        };
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("a readable marker");
        if value
            .get("project_path")
            .and_then(serde_json::Value::as_str)
            == Some(stated.as_str())
        {
            return entry.path();
        }
    }
    projects.join("no-project-directory")
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

/// Every run identifier a store's journal records. One run per store means one identifier.
fn runs_recorded(store: &Path) -> BTreeSet<String> {
    let journal = fs::read_to_string(store.join("events.jsonl")).expect("a committed journal");
    journal
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let record: serde_json::Value = serde_json::from_str(line).expect("a readable record");
            record
                .get("run_id")
                .and_then(serde_json::Value::as_str)
                .expect("every record names its run")
                .to_owned()
        })
        .collect()
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

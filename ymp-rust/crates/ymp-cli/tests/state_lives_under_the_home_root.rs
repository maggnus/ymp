#![forbid(unsafe_code)]

//! Acceptance: the product's durable state and default outputs live under the operator's home
//! root. The directory the product was started in receives nothing unless the operator explicitly
//! chooses it as an export or application destination.
//!
//! Every check drives the built executable from a project directory with a home directory of the
//! fixture's own, so what the default addresses is the default an operator has and no state of
//! this host is read or written. Each states its negative half against the same executable:
//!
//! * a run started from a clean directory leaves it exactly as it was, and the home root holds
//!   `root.json` and this project's `runs/0001` — against `--root .ymp`, which is the sibling
//!   directory the previous default wrote and which this outcome removes;
//! * a stated `YMP_HOME` is the root, and the home directory is then left alone;
//! * a draft is assembled under the root rather than beside the project it copies;
//! * a default export stays under the run store and leaves the launch directory untouched; an
//!   explicitly named destination reaches exactly the directory the operator selected.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;
use ymp_storage::{DataRoot, StoreIntent};

/// What an export directory is called when the operator names none, followed by the run it carries.
const EXPORT_PREFIX: &str = "ymp-evidence-";

// ---------------------------------------------------------------------------
// A run leaves the launch directory as it found it
// ---------------------------------------------------------------------------

#[test]
fn a_run_started_from_a_clean_directory_leaves_it_untouched() {
    let host = Host::new();
    let project = host.project("clean-launch");

    host.start(&project, &[], "first").assert_succeeded();
    host.command(&project, &["show".to_owned(), "events".to_owned()]);

    assert!(
        entries(&project).is_empty(),
        "the product wrote into the directory it was started in: {:?}",
        relative_paths(&project)
    );

    // The state is where the operator's other tools keep theirs: one root at home, holding this
    // project's first run.
    assert!(
        host.root().join("root.json").is_file(),
        "the home root carries no layout marker: {:?}",
        relative_paths(&host.root())
    );
    let runs = host.runs_directory(&project);
    assert_eq!(
        vec!["0001"],
        names(&runs),
        "the project's runs are named by something this layout does not derive"
    );
    assert!(
        runs.join("0001/events.jsonl").is_file(),
        "the first run committed no journal under the home root"
    );

    // The negative half: the root this outcome moved is still reachable by name, and naming it
    // beside the project reproduces exactly the sibling directory an operator used to be left
    // with.
    let beside = host.project("root-beside-the-project");
    host.start(&beside, &["--root".to_owned(), ".ymp".to_owned()], "beside")
        .assert_succeeded();
    assert_eq!(
        vec![".ymp".to_owned()],
        entries(&beside),
        "naming the root beside the project wrote something else there"
    );
}

// ---------------------------------------------------------------------------
// A stated root
// ---------------------------------------------------------------------------

#[test]
fn a_root_stated_by_the_environment_is_the_one_addressed() {
    let host = Host::new();
    let project = host.project("stated-root");
    let stated = host.host.path().join("state-of-this-environment");

    // The root this environment states is the one the run is created under, so it is the one put
    // into the state a measured account leaves behind.
    host.offers_models(&stated);
    let mut invocation = host.invocation(&project, &host.start_arguments(&[], "first"));
    invocation.env("YMP_HOME", &stated);
    let started = Run {
        output: invocation.output().expect("run the ymp executable"),
    };
    started.assert_succeeded();

    assert!(
        stated.join("root.json").is_file(),
        "the stated root carries no layout marker: {:?}",
        relative_paths(&stated)
    );
    assert!(
        !journals(&stated).is_empty(),
        "the run was committed somewhere other than the stated root"
    );

    // The negative half: the home root is what a stated one replaces, so it was never addressed,
    // and neither was the launch directory.
    assert!(
        !host.root().exists(),
        "a stated root was addressed and the home root was written anyway"
    );
    assert!(entries(&project).is_empty());
}

// ---------------------------------------------------------------------------
// A draft is assembled under the root
// ---------------------------------------------------------------------------

#[test]
fn a_draft_is_assembled_under_the_root_rather_than_beside_the_project() {
    let host = Host::new();
    let project = host.project("drafting");
    // A project as an operator has one: it runs its tests through a script of its own, and that
    // script fails while the work is not done.
    fs::create_dir_all(project.join("scripts")).expect("scripts directory");
    fs::write(project.join("README.md"), b"a project\n").expect("project file");
    let entry_point = project.join("scripts/test.sh");
    fs::write(&entry_point, b"#!/bin/sh\ntest -f result.txt\n").expect("test entry point");
    executable(&entry_point);
    for (directory, bytes) in [
        (".ymp", b"old root bytes\n".as_slice()),
        (".ymp-data", b"old store bytes\n".as_slice()),
    ] {
        fs::create_dir_all(project.join(directory)).expect("old product directory");
        fs::write(project.join(directory).join("sentinel"), bytes).expect("old product sentinel");
    }
    let before = relative_paths(&project);

    let drafted = host.command(
        &project,
        &[
            "request".to_owned(),
            "--prompt=write the result of the work".to_owned(),
        ],
    );
    drafted.assert_succeeded();

    assert_eq!(
        before,
        relative_paths(&project),
        "assembling a draft wrote into the project it copied"
    );
    let assembled: Vec<String> = relative_paths(&host.root())
        .into_iter()
        .filter(|path| path.contains("/draft/"))
        .collect();
    assert!(
        !assembled.is_empty(),
        "the draft was assembled nowhere under the root: {:?}",
        relative_paths(&host.root())
    );
    assert!(
        assembled
            .iter()
            .all(|path| !path.contains("/negative-control/.ymp/")
                && !path.contains("/negative-control/.ymp-data/")),
        "old product data was copied into a draft control: {assembled:?}"
    );
    assert_eq!(
        fs::read(project.join(".ymp/sentinel")).expect("old root sentinel"),
        b"old root bytes\n"
    );
    assert_eq!(
        fs::read(project.join(".ymp-data/sentinel")).expect("old store sentinel"),
        b"old store bytes\n"
    );
}

// ---------------------------------------------------------------------------
// A default export remains under the home root
// ---------------------------------------------------------------------------

#[test]
fn a_default_export_stays_under_the_run_store_and_an_explicit_one_goes_where_selected() {
    let host = Host::new();
    let project = host.project("delivery");

    // A run the product accepted, standing in the store the default addresses for this project.
    let store = DataRoot::open_for_project(&host.root(), &project)
        .expect("the home root")
        .store(StoreIntent::New)
        .expect("the project's first store");
    ymp_testkit::run_accepted_demo(&store).expect("an accepted run");

    // The negative half, stated before the export: nothing of that run reached the project.
    assert!(
        entries(&project).is_empty(),
        "an accepted run left state in the launch directory: {:?}",
        relative_paths(&project)
    );

    let exported = host.command(&project, &["export".to_owned()]);
    exported.assert_succeeded();

    assert!(
        entries(&project).is_empty(),
        "a default export wrote into the launch directory: {:?}",
        relative_paths(&project)
    );
    let delivered = entries(&store.join("exports"));
    assert_eq!(
        1,
        delivered.len(),
        "the default export was not filed under the run store: {delivered:?}"
    );
    let export = store.join("exports").join(&delivered[0]);
    assert!(
        delivered[0].starts_with(EXPORT_PREFIX),
        "the delivered directory does not name the run it carries: {delivered:?}"
    );
    assert_eq!(
        "pub fn answer() -> u8 { 42 }\n",
        fs::read_to_string(export.join("candidate/src/lib.rs")).expect("the exported candidate"),
        "the export carries something other than the accepted candidate"
    );
    for part in ["manifest.json", "events.jsonl", "candidate-manifest.json"] {
        assert!(
            export.join(part).is_file(),
            "the export carries no {part}: {:?}",
            relative_paths(&export)
        );
    }

    // The positive half for delivery outside the root: the path is reached only when the operator
    // names it, and exactly that path receives the bundle.
    let selected = project.join("selected-evidence");
    let selected_export = host.command(
        &project,
        &["export".to_owned(), format!("--to={}", selected.display())],
    );
    selected_export.assert_succeeded();
    assert_eq!(
        vec!["selected-evidence".to_owned()],
        entries(&project),
        "an explicitly selected export wrote anywhere else in the launch directory"
    );
    assert!(selected.join("candidate/src/lib.rs").is_file());
}

/// The delivery the owner asked for: the result stands where the operator works, as files, with
/// no directory of the product's own around it.
///
/// The negative half is the export as it is delivered by default, driven against the same run in
/// the check above: a directory named after the run, carrying the journal and the manifests
/// beside the candidate.
#[test]
fn an_applied_export_puts_the_candidate_into_the_launch_directory_itself() {
    let host = Host::new();
    let project = host.project("in-place");

    let store = DataRoot::open_for_project(&host.root(), &project)
        .expect("the home root")
        .store(StoreIntent::New)
        .expect("the project's first store");
    ymp_testkit::run_accepted_demo(&store).expect("an accepted run");
    assert!(
        entries(&project).is_empty(),
        "an accepted run left state in the launch directory: {:?}",
        relative_paths(&project)
    );

    let applied = host.command(&project, &["export".to_owned(), "--apply".to_owned()]);
    applied.assert_succeeded();

    assert_eq!(
        vec![
            "EVIDENCE.txt".to_owned(),
            "README.md".to_owned(),
            "src".to_owned(),
            "src/lib.rs".to_owned()
        ],
        relative_paths(&project),
        "the launch directory holds something other than the candidate's files"
    );
    assert_eq!(
        "pub fn answer() -> u8 { 42 }\n",
        fs::read_to_string(project.join("src/lib.rs")).expect("the applied candidate"),
        "what was applied is not the accepted candidate"
    );

    // Applying again over the operator's own file is refused by name, and the file stands.
    fs::write(project.join("src/lib.rs"), b"the operator's own work\n").expect("project file");
    let refused = host.command(&project, &["export".to_owned(), "--apply".to_owned()]);
    assert!(
        !refused.output.status.success(),
        "applying over a project file was not refused: {}",
        refused.text()
    );
    assert!(
        refused.text().contains("src/lib.rs"),
        "the refusal does not name the file that stopped it: {}",
        refused.text()
    );
    assert_eq!(
        "the operator's own work\n",
        fs::read_to_string(project.join("src/lib.rs")).expect("project file"),
        "a refused application changed a project file"
    );
}

// ---------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------

struct Host {
    host: TempDir,
    /// The home directory every invocation of this fixture sees, so the default root is the
    /// fixture's and the home this test runs in is neither read nor written.
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

    fn assert_succeeded(&self) {
        assert!(
            self.output.status.success(),
            "the invocation did not succeed: {}",
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
        for directory in [&home, &source, &negative_control] {
            fs::create_dir_all(directory).expect("fixture directory");
        }
        fs::write(source.join("input.txt"), b"before\n").expect("source file");
        fs::write(&verifier, b"#!/bin/sh\nexit 0\n").expect("verifier program");
        executable(&verifier);
        Self {
            host,
            home,
            source,
            verifier,
            negative_control,
        }
    }

    /// The root the default addresses when nothing states another.
    fn root(&self) -> PathBuf {
        self.home.join(".ymp")
    }

    /// Put a root into the state one measured account leaves behind, so a run can be created under
    /// it at all: the pool a run draws its models from is frozen when the run is created, and a
    /// root that offers nothing creates nothing.
    fn offers_models(&self, root: &Path) {
        ymp_testkit::ready_root::measured_with_no_engine_admitted(root);
    }

    /// The root an invocation with these leading arguments reads its pools under: the root it
    /// names, taken relative to the project as the command line takes it, or the default root.
    fn addressed(&self, project: &Path, before: &[String]) -> PathBuf {
        before
            .iter()
            .position(|argument| argument == "--root")
            .and_then(|index| before.get(index + 1))
            .map(|value| {
                let path = PathBuf::from(value);
                match path.is_absolute() {
                    true => path,
                    false => project.join(path),
                }
            })
            .unwrap_or_else(|| self.root())
    }

    /// The directory the root holds one project's runs under.
    fn runs_directory(&self, project: &Path) -> PathBuf {
        DataRoot::open_for_project(&self.root(), project)
            .expect("the home root")
            .runs_directory()
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

    fn invocation(&self, project: &Path, arguments: &[String]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ymp"));
        command
            .current_dir(project)
            .env("HOME", &self.home)
            .env_remove("YMP_HOME")
            .args(arguments);
        command
    }

    /// The executable, started in a project directory as an operator starts it.
    fn command(&self, project: &Path, arguments: &[String]) -> Run {
        Run {
            output: self
                .invocation(project, arguments)
                .output()
                .expect("run the ymp executable"),
        }
    }

    /// Start one run of one contract, through the confirmation the interface requires.
    /// Start one run. The root this invocation addresses offers models first, since a run is
    /// created against the pool it may draw them from.
    fn start(&self, project: &Path, before: &[String], prompt: &str) -> Run {
        self.offers_models(&self.addressed(project, before));
        let arguments = self.start_arguments(before, prompt);
        self.command(project, &arguments)
    }

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
}

fn digest(prompt: &str) -> String {
    format!(
        "contract-{}",
        &ymp_domain::digest_bytes(prompt.as_bytes())[..8]
    )
}

#[cfg(unix)]
fn executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make executable");
}

#[cfg(not(unix))]
fn executable(_path: &Path) {}

/// What a directory holds, by name, sorted.
fn entries(directory: &Path) -> Vec<String> {
    names(directory)
}

fn names(directory: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
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
    let mut paths = BTreeSet::new();
    walk(tree, &mut |path| {
        if let Ok(relative) = path.strip_prefix(tree) {
            paths.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    });
    paths.into_iter().collect()
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

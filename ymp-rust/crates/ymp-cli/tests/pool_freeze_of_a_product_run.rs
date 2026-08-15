#![forbid(unsafe_code)]

//! Acceptance, driven on the built product: creating a run fixes what it may draw its models from,
//! and nothing that happens to the pool afterwards reaches it.
//!
//! What this check drives is the `ymp` executable against a product root, a home and a search path
//! of its own, with the engines this build manages planted as programs that answer a version probe
//! and nothing else. Nothing here reads or writes the owner's own root, and no engine installed on
//! the host running this can be reached by name.
//!
//! * **The first negative half** is the state of a host nothing is enabled on: the request is
//!   drafted, the contract id is typed in full, and no run is created. What the operator reads is
//!   the held state naming `/providers` and not a technical refusal, and the root holds no journal.
//! * The positive half follows the same operator path after one account has been enabled and
//!   measured: the run is created, and its journal carries the pool by value — the pool's own
//!   digest, its entries in the pool's declared order, and the entry the run ignites on.
//! * **The second negative half** is the freeze itself: holding the account back afterwards moves
//!   the live pool's digest, and the record in the run's journal is required to be exactly what it
//!   was. A build that read the live pool where a run reads its snapshot fails here.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Output};

use serde_json::Value;
use tempfile::TempDir;

/// A product root, a home, a project and a search path of this check's own.
struct Host {
    _directory: TempDir,
    root: PathBuf,
    home: PathBuf,
    bin: PathBuf,
    project: PathBuf,
}

impl Host {
    fn new() -> Self {
        let directory = TempDir::new().expect("temporary directory");
        let base = directory
            .path()
            .canonicalize()
            .expect("resolve the fixture");
        let host = Self {
            root: base.join("root"),
            home: base.join("home"),
            bin: base.join("bin"),
            project: base.join("project"),
            _directory: directory,
        };
        for path in [&host.home, &host.bin, &host.project] {
            fs::create_dir_all(path).expect("fixture directory");
        }
        // A project as an operator has one: a source tree with a way of running its tests, which
        // is what the product proposes an acceptance condition from.
        fs::create_dir_all(host.project.join("scripts")).expect("project scripts");
        fs::write(host.project.join("README.md"), b"a project\n").expect("project file");
        let entry_point = host.project.join("scripts/test.sh");
        fs::write(&entry_point, b"#!/bin/sh\ntest -f result.txt\n").expect("test entry point");
        executable(&entry_point);
        for program in ["claude", "codex"] {
            host.plant(program);
        }
        host
    }

    /// An engine that answers a version probe and serves no model of its own. A model list is
    /// recorded separately, exactly as a build that measured one would have recorded it.
    fn plant(&self, program: &str) {
        let path = self.bin.join(program);
        fs::write(
            &path,
            format!("#!/bin/sh\necho '1.2.3 ({program})'\n").as_bytes(),
        )
        .expect("plant the engine");
        executable(&path);
    }

    fn ymp(&self, arguments: &[&str]) -> Output {
        let path = match std::env::var_os("PATH") {
            Some(existing) => format!("{}:{}", self.bin.display(), existing.to_string_lossy()),
            None => self.bin.display().to_string(),
        };
        Process::new(env!("CARGO_BIN_EXE_ymp"))
            .current_dir(&self.project)
            .arg("--root")
            .arg(&self.root)
            .args(arguments)
            .env("PATH", path)
            .env("HOME", &self.home)
            .env("YMP_HOME", self.home.join("ymp"))
            .output()
            .expect("run the ymp executable")
    }

    /// Record a measured model list against the engine. The planted engine answers no model probe,
    /// so this is how a catalog with entries in it comes about here.
    fn record_models(&self, names: &[&str]) {
        let path = self.root.join("runtimes").join("claude-code.json");
        let mut stored: Value =
            serde_json::from_slice(&fs::read(&path).expect("record bytes")).expect("record");
        stored["models"]["source"] = Value::String("measured".to_owned());
        stored["models"]["names"] = Value::Array(
            names
                .iter()
                .map(|name| Value::String((*name).to_owned()))
                .collect(),
        );
        fs::write(
            &path,
            serde_json::to_vec_pretty(&stored).expect("record bytes"),
        )
        .expect("write the record");
    }

    /// A root whose account is enabled and whose engine serves the named models — the state one
    /// measurement of an account leaves behind, reached through the product's own commands.
    fn measured(&self, names: &[&str]) {
        let enabled = self.ymp(&["provider", "enable", "anthropic"]);
        assert!(enabled.status.success(), "{}", stated(&enabled));
        self.record_models(names);
        let refreshed = self.ymp(&["provider", "refresh", "anthropic"]);
        assert!(refreshed.status.success(), "{}", stated(&refreshed));
    }

    /// The pool record as this root holds it now.
    fn pool_record(&self) -> Value {
        let path = self.root.join("pools").join("default.json");
        serde_json::from_slice(&fs::read(&path).expect("the pool record")).expect("readable JSON")
    }

    /// Draft a request and authorize it, as an operator does: one command states the work, the
    /// next types the contract id in full.
    fn request_and_start(&self) -> (Output, Output) {
        let drafted = self.ymp(&["request", "--prompt=keep the replay path idempotent"]);
        assert!(drafted.status.success(), "{}", stated(&drafted));
        let identifier = contract_id(&stated(&drafted));
        let started = self.ymp(&[
            "start",
            "--prompt=keep the replay path idempotent",
            &format!("--confirm={identifier}"),
        ]);
        (drafted, started)
    }

    /// Every journal this root holds, in the order the layout names its stores.
    fn journals(&self) -> Vec<PathBuf> {
        let mut found = Vec::new();
        collect_journals(&self.root, &mut found);
        found.sort();
        found
    }
}

fn collect_journals(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(listing) = fs::read_dir(directory) else {
        return;
    };
    for entry in listing.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_journals(&path, found);
        } else if path.file_name().is_some_and(|name| name == "events.jsonl") {
            found.push(path);
        }
    }
}

fn executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).expect("make executable");
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn stated(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The transcript with its line breaks and indentation collapsed, since it is wrapped to the width
/// a command lays its surfaces out at.
fn flattened(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn contract_id(transcript: &str) -> String {
    transcript
        .split_whitespace()
        .find(|word| word.starts_with("contract-"))
        .expect("the transcript states a contract id")
        .to_owned()
}

/// The record one run's journal carries for the pool it was created under.
fn frozen_record(journal: &Path) -> Value {
    fs::read_to_string(journal)
        .expect("the journal is readable")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("every line is an event envelope"))
        .find(|record| record["event"]["type"] == "pool_frozen")
        .expect("the run's journal carries the pool it was created under")["event"]
        .clone()
}

/// A run is created against the pool as it stood, and holding the account back afterwards reaches
/// the pool and not the run.
#[test]
fn a_run_created_through_the_product_carries_the_pool_it_was_created_under() {
    let host = Host::new();
    host.measured(&["claude-opus-5", "claude-sonnet-5"]);
    let record = host.pool_record();

    let (_, started) = host.request_and_start();
    assert!(
        started.status.success(),
        "the run was not created: {}",
        stated(&started)
    );

    let journals = host.journals();
    assert_eq!(journals.len(), 1, "this root holds {journals:?}");
    let frozen = frozen_record(&journals[0]);

    assert_eq!(frozen["pool"], "default");
    assert_eq!(
        frozen["digest"], record["resolved"]["digest"],
        "the run states a digest the pool never computed"
    );
    assert_eq!(
        frozen["entries"], record["resolved"]["entries"],
        "the run froze entries the pool did not resolve to, or in another order"
    );
    assert_eq!(frozen["origin"]["provider"], "anthropic");
    assert_eq!(frozen["origin"]["engine"], "claude-code");
    assert_eq!(
        frozen["origin"]["model"], "claude-opus-5",
        "the run does not ignite on the first live entry of the declared order"
    );

    // Holding the account back moves the live pool. The run already created reads its own record
    // and is untouched by it.
    let held = host.ymp(&["provider", "disable", "anthropic"]);
    assert!(held.status.success(), "{}", stated(&held));
    assert_ne!(
        host.pool_record()["resolved"]["digest"],
        record["resolved"]["digest"],
        "holding the account back left the pool's digest where it was"
    );
    assert_eq!(
        frozen_record(&journals[0]),
        frozen,
        "a change to the pool reached a run already created from it"
    );
}

/// A root that can offer a run nothing creates none, and says so in the words the brief states
/// rather than as a failure of the machinery.
#[test]
fn a_root_with_no_enabled_provider_creates_no_run_and_names_where_one_is_enabled() {
    let host = Host::new();

    let (_, refused) = host.request_and_start();
    let transcript = flattened(&stated(&refused));
    assert!(
        !refused.status.success(),
        "a run was created on a root with no enabled provider:\n{transcript}"
    );
    assert!(
        transcript.contains("your goal is held"),
        "the answer is not the held state the brief states:\n{transcript}"
    );
    assert!(
        transcript.contains("/providers"),
        "the answer does not name where a provider is enabled:\n{transcript}"
    );
    assert!(
        transcript.contains("nothing has started and nothing has left this host"),
        "the answer does not state that nothing was spent:\n{transcript}"
    );
    assert!(
        host.journals().is_empty(),
        "a refused creation wrote a journal: {:?}",
        host.journals()
    );

    // The same operator path, on the same root, once one account has been enabled and measured:
    // the run is created and carries the pool that measurement produced.
    host.measured(&["claude-opus-5"]);
    let (_, started) = host.request_and_start();
    assert!(
        started.status.success(),
        "the run was not created after a provider was enabled: {}",
        stated(&started)
    );
    let journals = host.journals();
    assert_eq!(journals.len(), 1, "this root holds {journals:?}");
    assert_eq!(
        frozen_record(&journals[0])["digest"],
        host.pool_record()["resolved"]["digest"]
    );
}

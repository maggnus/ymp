#![forbid(unsafe_code)]

//! Acceptance: the pool a run recruits from is resolved wherever a provider is observed, and every
//! act on a pool is a command of the same executable.
//!
//! What this check drives is the built executable against a product root, a home and a search path
//! of its own, with the engines this build manages planted as programs that record every
//! invocation of themselves. Nothing here reads or writes the owner's own root, and no engine
//! outside this directory can be reached by name.
//!
//! * **The negative half**: on a fresh root the operator's first observation of an account is what
//!   creates the pool. The check that must fail: leave the reconciler off the observation path,
//!   and the root holds no `pools/default.json` after the account has been enabled and measured —
//!   the operator is then told to create a pool the product is supposed to create for them.
//! * The positive half stands beside it: the pool that appears tracks the catalog, holds exactly
//!   the entries the catalog holds, and is created by nothing the operator typed.
//! * Holding the last account back leaves that pool standing and empty, never deleted, with every
//!   entry it permits still named and carrying the measured reason.
//! * The acts of the properties view are commands: permitting an entry, taking one out and stating
//!   the ceilings. Taking one out replaces the whole-catalog form with the list left behind, and
//!   the reply says so.
//! * A pool this root does not hold, and a model no entry of a pool names, are refused with what
//!   the root does hold rather than resolved to the nearest one.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Output};

use serde_json::Value;
use tempfile::TempDir;

/// A product root, a home and a search path of this check's own.
struct Host {
    _directory: TempDir,
    root: PathBuf,
    home: PathBuf,
    bin: PathBuf,
    marker: PathBuf,
}

impl Host {
    fn new() -> Self {
        let directory = TempDir::new().expect("temporary directory");
        let root = directory.path().join("root");
        let home = directory.path().join("home");
        let bin = directory.path().join("bin");
        let marker = directory.path().join("started.log");
        fs::create_dir_all(&home).expect("home directory");
        fs::create_dir_all(&bin).expect("bin directory");
        let host = Self {
            _directory: directory,
            root,
            home,
            bin,
            marker,
        };
        for program in ["claude", "codex"] {
            host.plant(program);
        }
        host
    }

    fn plant(&self, program: &str) {
        let path = self.bin.join(program);
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf '%s %s\\n' \"{program}\" \"$*\" >> \"{}\"\necho '1.2.3 \
                 ({program})'\n",
                self.marker.display()
            ),
        )
        .expect("plant the engine");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path).expect("metadata").permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&path, permissions).expect("make the engine executable");
        }
    }

    fn ymp(&self, arguments: &[&str]) -> Output {
        let path = match std::env::var_os("PATH") {
            Some(existing) => format!("{}:{}", self.bin.display(), existing.to_string_lossy()),
            None => self.bin.display().to_string(),
        };
        Process::new(env!("CARGO_BIN_EXE_ymp"))
            .arg("--root")
            .arg(&self.root)
            .args(arguments)
            .env("PATH", path)
            .env("HOME", &self.home)
            .env("YMP_HOME", self.home.join("ymp"))
            .output()
            .expect("run the ymp executable")
    }

    /// The pool record, which is the fact this check is about.
    fn pool_record(&self) -> Value {
        let path = self.pool_path();
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        serde_json::from_slice(&bytes).expect("the record is readable JSON")
    }

    fn pool_path(&self) -> PathBuf {
        self.root.join("pools").join("default.json")
    }

    /// Record a measured model list against the engine, as a measurement of a build that serves
    /// them would have recorded it. The planted engine answers no model probe, so this is how a
    /// catalog with entries in it comes about here.
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

    /// A root whose account is enabled and whose engine serves two models — the state one
    /// measurement of an account leaves behind, reached only through the product's own commands.
    fn measured(&self) -> &Self {
        assert!(
            self.ymp(&["provider", "enable", "anthropic"])
                .status
                .success()
        );
        self.record_models(&["claude-opus-5", "claude-sonnet-5"]);
        assert!(
            self.ymp(&["provider", "refresh", "anthropic"])
                .status
                .success()
        );
        self
    }
}

fn stated(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn exists(path: &Path) -> bool {
    path.exists()
}

/// The entries one pool resolved to, in its declared order.
fn entries(record: &Value) -> Vec<String> {
    record["resolved"]["entries"]
        .as_array()
        .expect("the resolved entries")
        .iter()
        .map(|entry| entry["model"].as_str().expect("the model").to_owned())
        .collect()
}

/// On a fresh root the operator is never asked to create a pool, and the observation that finds
/// the first admissible entry is what creates it.
///
/// The check that must fail: leave `Pools::reconcile` off the observation path, and this root
/// holds no pool after the account has been enabled and measured.
#[test]
fn the_observation_that_finds_the_first_entry_creates_the_pool() {
    let host = Host::new();

    // Nothing has been measured: the root holds no pool, the page says what creates one, and
    // reading it creates nothing.
    let page = host.ymp(&["show", "pools"]);
    assert!(page.status.success(), "{}", stated(&page));
    let shown = stated(&page);
    assert!(shown.contains("no pool stands under this root"), "{shown}");
    assert!(shown.contains("/providers"), "{shown}");
    assert!(
        !exists(&host.root.join("pools")),
        "reading the pools created one"
    );

    // Enabling the account measures it. The planted engine serves no model, so the catalog offers
    // nothing and there is still no pool — which is the state P3 states, not a missing wiring.
    let enabled = host.ymp(&["provider", "enable", "anthropic"]);
    assert!(enabled.status.success(), "{}", stated(&enabled));
    assert!(
        stated(&enabled).contains("no pool stands under this root"),
        "the reply does not state what the pools now hold:\n{}",
        stated(&enabled)
    );
    assert!(!exists(&host.pool_path()));

    // The engine's measurement now serves two models. Measuring the account again is an
    // observation, and the pool is resolved against what that observation leaves in the catalog.
    host.record_models(&["claude-opus-5", "claude-sonnet-5"]);
    let refreshed = host.ymp(&["provider", "refresh", "anthropic"]);
    assert!(refreshed.status.success(), "{}", stated(&refreshed));
    assert!(
        exists(&host.pool_path()),
        "the observation left this root without the pool the product creates for the operator: {}",
        stated(&refreshed)
    );

    let record = host.pool_record();
    assert_eq!(record["pool"], "default");
    assert_eq!(record["declared"]["models"]["selection"], "all_admissible");
    assert_eq!(record["resolved"]["tracking"], Value::Bool(true));
    assert_eq!(record["resolved"]["admissible"], 2);
    assert_eq!(entries(&record), ["claude-opus-5", "claude-sonnet-5"]);
    assert_eq!(record["declared"]["capacity"]["max_agents"], 6);
    assert!(
        stated(&refreshed).contains("default ready"),
        "the reply does not state what the pools now hold:\n{}",
        stated(&refreshed)
    );

    // The table and the properties view state it, and neither of them creates anything.
    let listed = host.ymp(&["show", "pools"]);
    assert!(listed.status.success(), "{}", stated(&listed));
    let shown = stated(&listed);
    assert!(shown.contains("default"), "{shown}");
    assert!(shown.contains("all admissible"), "{shown}");
    assert!(shown.contains("ready"), "{shown}");
    assert!(shown.contains("not who works"), "{shown}");

    let properties = host.ymp(&["show", "pool", "--pool", "default"]);
    assert!(properties.status.success(), "{}", stated(&properties));
    let shown = stated(&properties);
    assert!(shown.contains("claude-opus-5"), "{shown}");
    assert!(shown.contains("permitted · offered"), "{shown}");
    assert!(shown.contains("up to 6 participants"), "{shown}");
    assert!(shown.contains("what a run freezes"), "{shown}");
}

/// The enable transition resolves the pools too: an account held back and enabled again leaves the
/// pool it permits ready, and the pool the operator never created appears on that transition.
#[test]
fn the_enable_transition_resolves_the_pools() {
    let host = Host::new();
    assert!(
        host.ymp(&["provider", "enable", "anthropic"])
            .status
            .success()
    );
    host.record_models(&["claude-opus-5", "claude-sonnet-5"]);

    // Held back, this root offers nothing, so there is no pool to create and none is created.
    let disabled = host.ymp(&[
        "provider",
        "disable",
        "anthropic",
        "--reason",
        "kept out for this check",
    ]);
    assert!(disabled.status.success(), "{}", stated(&disabled));
    assert!(
        !exists(&host.pool_path()),
        "a pool was created for a catalog that offers nothing"
    );

    // Enabling is an observation, and it is the transition that creates the pool.
    let enabled = host.ymp(&["provider", "enable", "anthropic"]);
    assert!(enabled.status.success(), "{}", stated(&enabled));
    assert!(
        exists(&host.pool_path()),
        "the enable transition left this root without a pool: {}",
        stated(&enabled)
    );
    let record = host.pool_record();
    assert_eq!(record["resolved"]["admissible"], 2);
    assert!(
        stated(&enabled).contains("default ready"),
        "{}",
        stated(&enabled)
    );
}

/// Holding the last account back leaves the pool standing and empty — never deleted — with every
/// entry it permits still named and carrying the measured reason.
#[test]
fn holding_the_last_account_back_leaves_the_pool_standing_and_empty() {
    let host = Host::new();
    host.measured();
    assert_eq!(host.pool_record()["resolved"]["admissible"], 2);

    let disabled = host.ymp(&[
        "provider",
        "disable",
        "anthropic",
        "--reason",
        "kept out for this check",
    ]);
    assert!(disabled.status.success(), "{}", stated(&disabled));
    assert!(
        exists(&host.pool_path()),
        "holding the last account back deleted the pool"
    );

    let record = host.pool_record();
    assert_eq!(record["resolved"]["admissible"], 0);
    assert_eq!(
        entries(&record),
        ["claude-opus-5", "claude-sonnet-5"],
        "the pool grew shorter instead of stating why its entries are not offered"
    );
    let states: Vec<String> = record["resolved"]["states"]
        .as_array()
        .expect("the states")
        .iter()
        .map(|state| state["state"].as_str().expect("a state").to_owned())
        .collect();
    assert!(states.contains(&"empty".to_owned()), "{states:?}");

    let listed = host.ymp(&["show", "pools"]);
    let shown = stated(&listed);
    assert!(shown.contains("empty"), "{shown}");
    assert!(shown.contains("kept out for this check"), "{shown}");
}

/// Taking an entry out of a pool that follows the catalog leaves the list the operator left, and
/// says so; permitting it again puts it back at the end of that list.
#[test]
fn taking_an_entry_out_leaves_the_list_the_operator_left() {
    let host = Host::new();
    host.measured();
    assert_eq!(
        host.pool_record()["resolved"]["tracking"],
        Value::Bool(true)
    );

    let excluded = host.ymp(&["pool", "exclude", "default", "claude-opus-5"]);
    assert!(excluded.status.success(), "{}", stated(&excluded));
    let said = stated(&excluded);
    assert!(said.contains("no longer permits claude-opus-5"), "{said}");
    assert!(
        said.contains("no longer follows the catalog"),
        "the reply does not state what the edit replaced:\n{said}"
    );

    let record = host.pool_record();
    assert_eq!(record["resolved"]["tracking"], Value::Bool(false));
    assert_eq!(record["declared"]["models"]["selection"], "explicit");
    assert_eq!(entries(&record), ["claude-sonnet-5"]);

    // The entry that left is still a row of the properties view, because that row is what would
    // permit it again.
    let properties = host.ymp(&["show", "pool", "--pool", "default"]);
    let shown = stated(&properties);
    assert!(shown.contains("claude-opus-5"), "{shown}");
    assert!(shown.contains("not permitted"), "{shown}");
    assert!(shown.contains("explicit"), "{shown}");

    let permitted = host.ymp(&["pool", "permit", "default", "claude-opus-5"]);
    assert!(permitted.status.success(), "{}", stated(&permitted));
    let record = host.pool_record();
    assert_eq!(
        entries(&record),
        ["claude-sonnet-5", "claude-opus-5"],
        "a permitted entry joined the list somewhere other than the end"
    );
    assert_eq!(
        record["resolved"]["tracking"],
        Value::Bool(false),
        "permitting an entry put the pool back to following the catalog"
    );
}

/// The ceilings are stated by a command of the same executable, and moving one leaves the entries
/// — and the digest a run freezes — exactly as they were.
#[test]
fn the_ceilings_are_stated_and_move_no_entry() {
    let host = Host::new();
    host.measured();
    let before = host.pool_record();

    let capacity = host.ymp(&["pool", "set-capacity", "default", "--participants", "4"]);
    assert!(capacity.status.success(), "{}", stated(&capacity));
    assert!(
        stated(&capacity).contains("up to 4 participants"),
        "{}",
        stated(&capacity)
    );
    // The reply is wrapped to the width a command lays its transcript out at, so the sentence is
    // read from the flattened output.
    let said = stated(&capacity);
    let flattened = said.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flattened.contains("both are ceilings and neither is a target"),
        "the reply does not say what a ceiling is:\n{said}"
    );

    let after = host.pool_record();
    assert_eq!(after["declared"]["capacity"]["max_agents"], 4);
    assert_eq!(
        after["declared"]["capacity"]["max_concurrent_attempts"],
        before["declared"]["capacity"]["max_concurrent_attempts"],
        "stating one ceiling moved the other"
    );
    assert_eq!(
        after["resolved"]["digest"], before["resolved"]["digest"],
        "a ceiling moved the digest a run freezes"
    );
    assert_eq!(after["resolved"]["tracking"], Value::Bool(true));
}

/// A pool this root does not hold, and a model no entry of a pool names, change nothing and exit
/// non-zero with what the root does hold.
#[test]
fn a_row_that_selects_nothing_is_refused_rather_than_resolved() {
    let host = Host::new();
    host.measured();
    let before = host.pool_record();

    let unknown_pool = host.ymp(&["pool", "exclude", "cheap", "claude-opus-5"]);
    assert!(!unknown_pool.status.success(), "{}", stated(&unknown_pool));
    assert!(
        stated(&unknown_pool).contains("default"),
        "the refusal does not name what this root holds:\n{}",
        stated(&unknown_pool)
    );

    let unknown_model = host.ymp(&["pool", "exclude", "default", "nemotron-ultra"]);
    assert!(
        !unknown_model.status.success(),
        "{}",
        stated(&unknown_model)
    );
    assert!(
        stated(&unknown_model).contains("claude-opus-5"),
        "the refusal does not name what the catalog holds:\n{}",
        stated(&unknown_model)
    );

    // A command that states no pool shows nothing and says which value it needs.
    let unstated = host.ymp(&["show", "pool"]);
    assert!(!unstated.status.success(), "{}", stated(&unstated));
    assert!(
        stated(&unstated).contains("--pool"),
        "{}",
        stated(&unstated)
    );

    assert_eq!(
        host.pool_record(),
        before,
        "a refused command changed the record"
    );
}

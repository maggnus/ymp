#![forbid(unsafe_code)]

//! Acceptance: a provider is measured on the enable transition and on an explicit refresh, and at
//! no other moment.
//!
//! What this check drives is the built executable against a product root of its own, with the
//! engines this build manages planted on the search path as programs that record every invocation
//! of themselves. What an operator gets is therefore what is measured here: **the marker file is
//! the evidence**, and its absence is the evidence that nothing was started.
//!
//! * The negative half: on a fresh root, reading the supported list and reading the catalog start
//!   no engine and write neither an engine record nor a provider record. The check that must fail:
//!   measure the providers when either page is composed, and the marker appears before any
//!   provider was enabled.
//! * The positive half stands beside it so the negative one cannot pass by measuring nothing at
//!   all: enabling the provider starts the engine that reaches it, records what it found, and
//!   stamps the observation with the moment it was taken.
//! * The engine of a provider that stays disabled is never started, on any of these paths.
//! * The catalog is derived when it is read, and a route that serves no model reads as a route
//!   that serves no model rather than as a shorter catalog.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Output};

use serde_json::Value;
use tempfile::TempDir;

/// A product root, a home and a search path of this check's own. Nothing here reads or writes the
/// owner's own root, and no engine outside this directory can be reached by name.
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
        // Both engines this build manages, planted as programs that record being started. A
        // release is printed for whatever is asked, so the measurement gets far enough to be
        // worth recording; what is asserted is that the program ran at all.
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

    /// Which engines have been started since the last look, in the order they were started.
    fn started(&self) -> Vec<String> {
        fs::read_to_string(&self.marker)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn started_nothing(&self) -> bool {
        self.started().is_empty()
    }

    fn provider_record(&self, provider: &str) -> Value {
        let path = self.root.join("providers").join(format!("{provider}.json"));
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        serde_json::from_slice(&bytes).expect("the record is readable JSON")
    }

    fn engine_record(&self, engine: &str) -> Value {
        let path = self.root.join("runtimes").join(format!("{engine}.json"));
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        serde_json::from_slice(&bytes).expect("the record is readable JSON")
    }

    /// Record a measured model list against the engine, as a measurement of a build that serves
    /// them would have recorded it.
    fn record_models(&self, engine: &str, names: &[&str]) {
        let path = self.root.join("runtimes").join(format!("{engine}.json"));
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

/// The whole supported list is readable on a fresh root, and reading it starts nothing.
#[test]
fn reading_the_supported_list_and_the_catalog_measures_nothing() {
    let host = Host::new();

    let page = host.ymp(&["show", "providers"]);
    assert!(page.status.success(), "{}", stated(&page));
    let shown = stated(&page);
    for provider in ["anthropic", "openai"] {
        assert!(shown.contains(provider), "{shown}");
    }
    assert!(shown.contains("disabled"), "{shown}");
    assert!(shown.contains("not enabled"), "{shown}");

    let catalog = host.ymp(&["show", "models"]);
    assert!(catalog.status.success(), "{}", stated(&catalog));
    assert!(
        stated(&catalog).contains("not enabled"),
        "{}",
        stated(&catalog)
    );

    let properties = host.ymp(&["show", "provider", "--provider", "anthropic"]);
    assert!(properties.status.success(), "{}", stated(&properties));
    let shown = stated(&properties);
    assert!(
        shown.contains("enabling sends repository content"),
        "the consequence of enabling is not stated on the properties view:\n{shown}"
    );
    assert!(shown.contains("before a run starts"), "{shown}");

    assert!(
        host.started_nothing(),
        "an engine was started before any provider was enabled: {:?}",
        host.started()
    );
    assert!(
        !exists(&host.root.join("providers")),
        "reading the provider level wrote a provider record"
    );
    assert!(
        !exists(&host.root.join("runtimes")),
        "reading the provider level wrote an engine record"
    );
}

/// A command that states no provider shows nothing and says which value it needs.
#[test]
fn the_properties_page_states_which_provider_it_shows() {
    let host = Host::new();
    let refused = host.ymp(&["show", "provider"]);
    assert!(!refused.status.success(), "{}", stated(&refused));
    assert!(
        stated(&refused).contains("--provider"),
        "{}",
        stated(&refused)
    );
    assert!(host.started_nothing());
}

/// Enabling is what measures: the engine that reaches the provider is started, what it found is
/// recorded, and the observation carries the moment it was taken.
#[test]
fn enabling_a_provider_measures_it_and_records_when_it_was_measured() {
    let host = Host::new();
    let enabled = host.ymp(&["provider", "enable", "anthropic"]);
    assert!(enabled.status.success(), "{}", stated(&enabled));
    let said = stated(&enabled);
    assert!(said.contains("the anthropic provider is enabled"), "{said}");
    assert!(
        said.contains("repository content"),
        "the reply does not restate what enabling permits:\n{said}"
    );

    let started = host.started();
    assert!(
        started.iter().any(|line| line.starts_with("claude ")),
        "the engine that reaches anthropic was not started: {started:?}"
    );
    assert!(
        !started.iter().any(|line| line.starts_with("codex ")),
        "the engine of a provider nobody enabled was started: {started:?}"
    );

    let record = host.provider_record("anthropic");
    assert_eq!(record["enabled"], Value::Bool(true));
    let observed = record["observed_at_ms"]
        .as_u64()
        .expect("the observation records when it was taken");
    assert!(observed > 0, "the observation is stamped with nothing");
    assert!(
        record["routes"]
            .as_array()
            .expect("the routes the provider is reached by")
            .iter()
            .any(|route| route["engine"] == "claude-code"),
        "{record}"
    );
    // The engine was measured, so its record now stands under this root.
    assert!(exists(&host.root.join("runtimes").join("claude-code.json")));

    // Measuring again advances the observation, and the age of the earlier one is what it
    // replaces. Nothing else about the decision changes.
    let refreshed = host.ymp(&["provider", "refresh", "anthropic"]);
    assert!(refreshed.status.success(), "{}", stated(&refreshed));
    let again = host.provider_record("anthropic");
    assert!(
        again["observed_at_ms"].as_u64().expect("a later moment") >= observed,
        "measuring again left the observation at the moment of the first one"
    );
    assert_eq!(again["enabled"], Value::Bool(true));
}

/// A provider that is not enabled is not measured, however it is asked.
#[test]
fn refreshing_a_provider_that_is_not_enabled_measures_nothing_and_exits_non_zero() {
    let host = Host::new();
    let refused = host.ymp(&["provider", "refresh", "openai"]);
    assert!(
        !refused.status.success(),
        "a provider nobody enabled was measured:\n{}",
        stated(&refused)
    );
    let said = stated(&refused);
    assert!(said.contains("not enabled"), "{said}");
    assert!(
        host.started_nothing(),
        "a refused refresh still started an engine: {:?}",
        host.started()
    );

    // A name that selects no provider changes nothing and exits non-zero.
    let unknown = host.ymp(&["provider", "enable", "nvidia"]);
    assert!(!unknown.status.success(), "{}", stated(&unknown));
    assert!(
        stated(&unknown).contains("anthropic"),
        "{}",
        stated(&unknown)
    );
    assert!(host.started_nothing());
    assert!(!exists(&host.root.join("providers")));
}

/// Disabling keeps everything the provider measured and takes its models out of the offer.
#[test]
fn disabling_a_provider_keeps_its_measurements_and_offers_nothing() {
    let host = Host::new();
    assert!(
        host.ymp(&["provider", "enable", "anthropic"])
            .status
            .success()
    );
    host.record_models("claude-code", &["claude-opus-5", "claude-sonnet-5"]);

    // The catalog is derived when it is read, so the list recorded a moment ago is the list the
    // page states — without anything being measured again.
    let started = host.started().len();
    let catalog = host.ymp(&["show", "models"]);
    assert!(catalog.status.success(), "{}", stated(&catalog));
    let shown = stated(&catalog);
    assert!(shown.contains("claude-opus-5"), "{shown}");
    assert!(shown.contains("claude-sonnet-5"), "{shown}");
    assert_eq!(
        host.started().len(),
        started,
        "reading the catalog started an engine"
    );

    let disabled = host.ymp(&[
        "provider",
        "disable",
        "anthropic",
        "--reason",
        "kept out for this check",
    ]);
    assert!(disabled.status.success(), "{}", stated(&disabled));
    let record = host.provider_record("anthropic");
    assert_eq!(record["enabled"], Value::Bool(false));
    assert_eq!(record["disabled_reason"], "kept out for this check");
    assert!(
        record["observed_at_ms"].as_u64().is_some(),
        "disabling erased the observation: {record}"
    );

    let catalog = host.ymp(&["show", "models"]);
    let shown = stated(&catalog);
    assert!(
        shown.contains("claude-opus-5"),
        "a disabled provider hid the models it measured instead of stating why they are not \
         offered:\n{shown}"
    );
    assert!(shown.contains("not offered"), "{shown}");
    assert!(shown.contains("kept out for this check"), "{shown}");
}

/// The engine the owner held back stays held back when its provider is enabled, and its recorded
/// reason is what the provider surfaces state.
///
/// Enabling an account is not a statement about which engines this host may start: the two
/// decisions are recorded apart, and the one the operator took about the engine reaches the
/// provider level unchanged.
#[test]
fn enabling_a_provider_does_not_admit_an_engine_the_owner_held_back() {
    let host = Host::new();
    let enabled = host.ymp(&["provider", "enable", "openai"]);
    assert!(enabled.status.success(), "{}", stated(&enabled));
    assert!(
        host.started_nothing(),
        "a held-back engine was started by enabling the provider it reaches: {:?}",
        host.started()
    );

    assert_eq!(
        host.engine_record("codex")["enabled"],
        Value::Bool(false),
        "enabling the account admitted the engine the owner held back"
    );
    assert_eq!(
        host.engine_record("codex")["disabled_reason"],
        "usage limit until 2026-09-12"
    );

    let properties = host.ymp(&["show", "provider", "--provider", "openai"]);
    let shown = stated(&properties);
    assert!(
        shown.contains("usage limit until 2026-09-12"),
        "the decision the owner recorded does not reach the provider view:\n{shown}"
    );
    assert!(shown.contains("not admitted"), "{shown}");
}

/// An engine record that leaves the root takes its models out of the catalog, and the route it
/// served them through stays and says so.
#[test]
fn a_route_that_serves_nothing_reads_as_such_and_never_as_a_shorter_catalog() {
    let host = Host::new();
    assert!(
        host.ymp(&["provider", "enable", "anthropic"])
            .status
            .success()
    );
    host.record_models("claude-code", &["claude-opus-5"]);
    assert!(stated(&host.ymp(&["show", "models"])).contains("claude-opus-5"));
    assert!(host.engine_record("claude-code")["models"]["names"].is_array());

    fs::remove_file(host.root.join("runtimes").join("claude-code.json"))
        .expect("remove the engine record");
    let catalog = host.ymp(&["show", "models"]);
    assert!(catalog.status.success(), "{}", stated(&catalog));
    let shown = stated(&catalog);
    assert!(
        !shown.contains("claude-opus-5"),
        "a removed record still served its models:\n{shown}"
    );
    assert!(
        shown.contains("no models"),
        "the catalog was answered short, with nothing in it saying so:\n{shown}"
    );
    assert!(shown.contains("claude-code"), "{shown}");
    assert!(
        shown.contains("no record"),
        "the reason the route serves nothing is not stated:\n{shown}"
    );
}

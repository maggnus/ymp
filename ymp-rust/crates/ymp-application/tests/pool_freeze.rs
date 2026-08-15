#![forbid(unsafe_code)]

//! Acceptance: a run is created against the pool as it stood, and nothing that happens to the pool
//! afterwards reaches it.
//!
//! Everything here runs against a product root of this check's own — a temporary directory, never
//! the operator's `~/.ymp` — whose engines are records rather than programs: no engine on the host
//! running this is discovered, started or probed, and no model list is measured from one. What the
//! fixture stands for is a root the operator has enabled an account on and measured once, which is
//! the state P2 and P3 leave behind.
//!
//! * The positive half: the run's journal carries the pool it was created under, by value — the
//!   same entries in the same order, the pool's own digest, and the entry the run ignites on.
//! * **The negative half is the freeze itself**: with the pool still standing where it was, the
//!   provider is disabled and an entry is taken out of the pool. Both move the live pool's digest,
//!   and the fact re-read from the run's own journal is required to be byte-for-byte what it was.
//!   A build that read the live pool where a run reads its snapshot fails here.
//! * The second negative half is the state before anything is enabled: a root that offers a run
//!   nothing creates no run at all, and what it states is the plain-words state naming
//!   `/providers` rather than a technical refusal.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use ymp_application::{
    AcceptanceCondition, Application, PoolFreezeRefused, RunRequest, freeze_under, prepare_contract,
};
use ymp_domain::EventKind;
use ymp_domain::pool::{EntryIdentity, FrozenPool};
use ymp_runtime_registry::{
    Engine, ModelCatalog, ModelSource, PoolEntry, PoolModels, PoolName, PoolRecord, Pools,
    Providers, Registry,
};

/// A product root of this check's own, with the store the run is recorded in beside it.
struct Host {
    _directory: TempDir,
    root: PathBuf,
    project: PathBuf,
    source: PathBuf,
    program: PathBuf,
    negative_control: PathBuf,
}

fn host() -> Host {
    let directory = TempDir::new().expect("temporary directory");
    let base = directory.path().canonicalize().expect("resolve the root");
    let root = base.join("root");
    let project = base.join("project");
    let source = project.join("source");
    let negative_control = project.join("negative-control");
    let program = project.join("verify.sh");
    fs::create_dir_all(&root).expect("product root");
    fs::create_dir_all(&source).expect("source");
    fs::create_dir_all(&negative_control).expect("negative control");
    fs::write(&program, b"#!/bin/sh\nexit 1\n").expect("verifier program");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions).expect("make the verifier executable");
    }
    Host {
        _directory: directory,
        root,
        project,
        source,
        program,
        negative_control,
    }
}

impl Host {
    fn registry(&self) -> Registry {
        Registry::under(&self.root)
    }

    fn providers(&self) -> Providers {
        Providers::under(&self.root)
    }

    fn pools(&self) -> Pools {
        Pools::under(&self.root)
    }

    /// One store per run, since a store holds exactly one run.
    fn store(&self, name: &str) -> PathBuf {
        self.project.join("stores").join(name)
    }

    /// A root with one enabled account whose engine serves the named models, as a measurement of a
    /// build that serves them would have recorded it. No process is started: the engine record is
    /// written directly, which is what makes this check the same on every host.
    fn measure(&self, engine: Engine, names: &[&str]) {
        self.registry()
            .update(engine, |record| {
                record.enabled = true;
                record.disabled_reason = None;
                record.properties.executable = Some(format!("/usr/local/bin/{}", engine.program()));
                record.properties.version = Some("1.2.3".to_owned());
                record.properties.executable_digest = Some(format!("{}-digest", engine.name()));
                record.properties.credential_origin =
                    Some("delegated_host_keychain_credential".to_owned());
                record.models = ModelCatalog {
                    source: ModelSource::Measured,
                    measured_for_version: Some("1.2.3".to_owned()),
                    measured_for_digest: Some(format!("{}-digest", engine.name())),
                    note: None,
                    names: names.iter().map(|name| (*name).to_owned()).collect(),
                };
            })
            .expect("record the measurement");
        self.providers()
            .set_enabled(engine.provider(), true, None)
            .expect("enable the account");
        self.observe();
    }

    /// The observation path, which is where the pools are resolved (P4).
    fn observe(&self) {
        let registry = self.registry();
        let providers = self.providers();
        providers.observe(&registry).expect("observe the providers");
        self.pools()
            .reconcile(&providers, &registry)
            .expect("resolve the pools");
    }

    fn default_pool(&self) -> PoolRecord {
        self.pools()
            .read(&PoolName::default_pool())
            .expect("read the default pool")
            .expect("this root holds the default pool")
    }

    /// Create one run in a store of its own, against the pool as it stands now.
    fn start(&self, store: &Path) -> Application {
        let prepared = prepare_contract(&RunRequest {
            prompt: "keep the replay path idempotent".to_owned(),
            source: self.source.clone(),
            acceptance: Some(AcceptanceCondition::new(
                &self.program,
                &self.negative_control,
            )),
            capture_exclusions: Vec::new(),
            contract_id: None,
            budget: None,
        })
        .expect("prepared contract");
        let frozen = freeze_under(&self.root, None).expect("the root freezes its pool");
        let (application, _) = Application::create_with_contract(store, &prepared, &frozen)
            .expect("the run is created");
        application
    }
}

/// The snapshot one run's journal states, read back from the record rather than from the process
/// that wrote it.
fn frozen_in_journal(store: &Path) -> FrozenPool {
    let application = Application::open(store).expect("reopen the store");
    let frozen = application
        .events_after(0)
        .expect("committed events")
        .into_iter()
        .find_map(|event| match event.event {
            EventKind::PoolFrozen(frozen) => Some(frozen),
            _ => None,
        })
        .expect("the run's journal carries the pool it was created under");
    // The projection a restart rebuilds states the same value, so nothing here depends on which of
    // the two a later reader happens to hold.
    assert_eq!(
        application.frozen_pool(),
        Some(&frozen),
        "the projection and the journal state different snapshots"
    );
    frozen
}

/// The triples one pool record resolved to, with what admission measured about each.
fn resolved(record: &PoolRecord) -> Vec<(String, String, String, bool, Option<String>)> {
    record
        .resolved
        .entries
        .iter()
        .map(|entry| {
            (
                entry.provider.clone(),
                entry.engine.clone(),
                entry.model.clone(),
                entry.admissible,
                entry.reason.clone(),
            )
        })
        .collect()
}

/// The same reading of a frozen snapshot, so the two can be compared as one value.
fn frozen_entries(frozen: &FrozenPool) -> Vec<(String, String, String, bool, Option<String>)> {
    frozen
        .entries
        .iter()
        .map(|entry| {
            (
                entry.provider.clone(),
                entry.engine.clone(),
                entry.model.clone(),
                entry.admissible,
                entry.reason.clone(),
            )
        })
        .collect()
}

/// The run carries the pool by value: the same entries in the same order, the pool's own digest,
/// and the first live entry as the one it ignites on.
#[test]
fn a_created_run_carries_the_pool_it_was_created_under() {
    let host = host();
    host.measure(Engine::ClaudeCode, &["claude-opus-5", "claude-sonnet-5"]);
    let record = host.default_pool();

    let store = host.store("first");
    let application = host.start(&store);

    // The three records of a creation, in the order the bootstrap sequence states them: the run
    // exists, it is bound to what judges it, and only then is what it may draw on fixed.
    let events = application.events_after(0).expect("committed events");
    assert!(matches!(events[0].event, EventKind::RunStarted { .. }));
    assert!(matches!(
        events[1].event,
        EventKind::ContractApproved { .. }
    ));
    assert!(
        matches!(events[2].event, EventKind::PoolFrozen(_)),
        "the freeze is not the third record of the creation: {:?}",
        events[2].event
    );
    drop(application);

    let frozen = frozen_in_journal(&store);
    assert_eq!(frozen.pool, "default");
    assert_eq!(
        frozen_entries(&frozen),
        resolved(&record),
        "the run froze entries the pool did not resolve to, or in another order"
    );
    assert_eq!(
        frozen.digest, record.resolved.digest,
        "the run states a digest the pool never computed"
    );
    assert_eq!(
        frozen.origin,
        EntryIdentity::new("anthropic", "claude-code", "claude-opus-5"),
        "the run does not ignite on the first live entry of the declared order"
    );
    assert!(frozen.permits(&EntryIdentity::new(
        "anthropic",
        "claude-code",
        "claude-sonnet-5"
    )));
    assert!(!frozen.permits(&EntryIdentity::new("openai", "codex", "gpt-5")));
}

/// A pool edited or a provider disabled after a run was created moves the live pool and reaches no
/// run already standing on it. The next run gets the pool as it stands then.
#[test]
fn what_changes_after_the_freeze_belongs_to_the_next_run() {
    let host = host();
    host.measure(Engine::ClaudeCode, &["claude-opus-5", "claude-sonnet-5"]);

    let first_store = host.store("first");
    drop(host.start(&first_store));
    let at_creation = frozen_in_journal(&first_store);

    // Taking an entry out of the pool moves the live digest. The pool stops following the catalog,
    // which is what an edit means; the run created before it is untouched.
    host.pools()
        .edit(
            &PoolName::default_pool(),
            &host.providers(),
            &host.registry(),
            |declared| {
                declared.models = PoolModels::explicit([PoolEntry {
                    provider: "anthropic".to_owned(),
                    engine: "claude-code".to_owned(),
                    model: "claude-sonnet-5".to_owned(),
                }]);
            },
        )
        .expect("edit the pool");
    let after_edit = host.default_pool();
    assert_ne!(
        after_edit.resolved.digest, at_creation.digest,
        "taking an entry out of the pool left its digest where it was"
    );
    assert_eq!(
        frozen_in_journal(&first_store),
        at_creation,
        "an edit of the pool reached a run already created from it"
    );

    // Holding the account back is the other way the live pool moves. It reaches the run no more
    // than the edit did.
    host.providers()
        .set_enabled(Engine::ClaudeCode.provider(), false, Some("held back"))
        .expect("hold the account back");
    host.observe();
    let after_disable = host.default_pool();
    assert_ne!(after_disable.resolved.digest, after_edit.resolved.digest);
    assert_eq!(
        frozen_in_journal(&first_store),
        at_creation,
        "disabling the provider reached a run already created from it"
    );

    // With nothing live, the root creates no second run at all — which is the same rule stated
    // from the other side.
    assert!(matches!(
        freeze_under(&host.root, None),
        Err(PoolFreezeRefused::NothingLive { .. })
    ));

    // Enabling the account again leaves the edited pool holding one entry. The next run freezes
    // that, and the run created before the edit still holds what it was created under.
    host.providers()
        .set_enabled(Engine::ClaudeCode.provider(), true, None)
        .expect("enable the account again");
    host.observe();
    let now = host.default_pool();
    assert_ne!(now.resolved.digest, at_creation.digest);

    let second_store = host.store("second");
    drop(host.start(&second_store));
    let second = frozen_in_journal(&second_store);
    assert_eq!(
        second.digest, now.resolved.digest,
        "the second run did not freeze the pool as it stands now"
    );
    assert_eq!(
        second.origin,
        EntryIdentity::new("anthropic", "claude-code", "claude-sonnet-5")
    );
    assert_eq!(
        frozen_in_journal(&first_store),
        at_creation,
        "a second run rewrote the first run's snapshot"
    );
}

/// A root that can offer a run nothing creates no run, and what it states is the state in plain
/// words with the command that supplies what is missing — never a technical refusal.
#[test]
fn a_root_that_offers_nothing_creates_no_run_and_says_so_in_plain_words() {
    let host = host();

    // Nothing observed at all: this root holds no pool, because the pool is created by the first
    // measurement of an account rather than by the operator.
    let refusal = freeze_under(&host.root, None).expect_err("a root with no pool freezes nothing");
    assert!(matches!(refusal, PoolFreezeRefused::NoPool));
    let stated = refusal.to_string();
    assert!(stated.contains("/providers"), "{stated}");
    assert!(stated.contains("your goal is held"), "{stated}");
    assert!(
        stated.contains("nothing has started and nothing has left this host"),
        "{stated}"
    );

    // A measured account, then held back: the pool stands, every entry it permits is named, and
    // none of them is live. The refusal carries the reason the record itself stated.
    host.measure(Engine::ClaudeCode, &["claude-opus-5"]);
    host.providers()
        .set_enabled(Engine::ClaudeCode.provider(), false, Some("held back"))
        .expect("hold the account back");
    host.observe();
    assert_eq!(
        host.default_pool().resolved.entries.len(),
        1,
        "the pool grew shorter instead of being marked"
    );

    let refusal =
        freeze_under(&host.root, None).expect_err("a pool with nothing live freezes nothing");
    let stated = refusal.to_string();
    assert!(
        matches!(refusal, PoolFreezeRefused::NothingLive { .. }),
        "{stated}"
    );
    assert!(stated.contains("/providers"), "{stated}");
    assert!(stated.contains("your goal is held"), "{stated}");
    assert!(
        !stated.contains("Error") && !stated.contains("error"),
        "the refusal reads as a technical failure: {stated}"
    );

    // And nothing was written for a run that was never created.
    let store = host.store("never");
    assert!(
        !store.join("events.jsonl").exists(),
        "a refused creation wrote a journal"
    );
}

/// A pool this root does not hold is refused rather than resolved to the nearest one: a run
/// created against a boundary nobody declared would carry a boundary the operator never stated.
#[test]
fn a_pool_this_root_does_not_hold_is_refused_rather_than_replaced_by_default() {
    let host = host();
    host.measure(Engine::ClaudeCode, &["claude-opus-5"]);

    let refusal = freeze_under(&host.root, Some("experiments"))
        .expect_err("a pool this root does not hold freezes nothing");
    let stated = refusal.to_string();
    assert!(
        matches!(refusal, PoolFreezeRefused::UnknownPool { .. }),
        "{stated}"
    );
    assert!(stated.contains("experiments"), "{stated}");
    assert!(stated.contains("/pools"), "{stated}");

    // The pool this root does hold is frozen by name exactly as it is by default.
    let named = freeze_under(&host.root, Some("default")).expect("the default pool freezes");
    assert_eq!(named.digest, host.default_pool().resolved.digest);
}

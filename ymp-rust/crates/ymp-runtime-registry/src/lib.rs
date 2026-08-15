#![forbid(unsafe_code)]

//! The engine registry: which runtime engines this host admits, and what was measured about them.
//!
//! An engine is a managed entity rather than a compiled-in constant, and it stands beneath the
//! provider it reaches. One record per engine and one per provider live under the product root:
//!
//! ```text
//! .ymp/
//!   pools/default.json              which of the catalog's entries a run may recruit from
//!   providers/anthropic.json        the account, its observed state and the engines that reach it
//!   providers/openai.json
//!   runtimes/claude-code.json       enabled flag, measured properties, measured model list
//!   runtimes/codex.json
//! ```
//!
//! The record holds three separable things:
//!
//! * **the enabled flag and the reason it carries** — the operator's decision, the only field the
//!   product never measures. A disabled engine is refused before anything is started
//!   ([`Registry::admit`]) and is not offered as a route, so an engine that must not be used is
//!   stopped in one place rather than in every caller;
//! * **measured properties** — the executable this host resolved, the release it reported, where
//!   its credential comes from and the bounds a managed invocation is held to. Every one of them
//!   is written from a measurement, never from a declaration;
//! * **the model list** — what the engine can serve, with the provenance of the list and the build
//!   it was measured against, so a stale list is visible as stale rather than read as current.
//!
//! Executable discovery belongs here too ([`discover`], [`resolve`]). An engine crate that
//! resolved its own executable from configuration would add a second selection channel that no
//! record states, so the registry owns the one channel and the record names its result.
//!
//! The engine records are the measured half of the level the collective design calls the model
//! catalog (`ymp-docs/design/COLLECTIVE-DESIGN.md`, §6). The other half is the account those
//! models are served by: [`provider`] holds the provider records, and [`catalog`] joins the two
//! into the triples the design calls catalog entries. All three levels record what exists and
//! whether an engine is admitted, and none of them decides which of those models a run may use.
//!
//! Which of them a run may use is the level above, and it is stated as its own record: [`pool`]
//! holds the agent pools, each naming the catalog entries it permits and the ceilings a run using
//! it is held to. That level decides which entries are permitted and nothing else — it instantiates
//! nothing, ranks nothing and starts nothing, exactly as the three beneath it do not.

pub mod catalog;
pub mod pool;
pub mod provider;

pub use catalog::{Availability, Catalog, CatalogEntry, CatalogRoute};
pub use pool::{
    DEFAULT_POOL, ObservedProvider, POOL_SCHEMA_VERSION, POOLS_DIRECTORY, PoolCapacity,
    PoolDeclaration, PoolEntry, PoolError, PoolModels, PoolName, PoolObservation, PoolRecord,
    PoolResolution, PoolResourceLimits, PoolState, PoolStateKind, Pools, ResolvedEntry,
};
pub use provider::{
    NOT_ENABLED_REASON, Observation, PROVIDER_SCHEMA_VERSION, PROVIDERS_DIRECTORY, ProviderError,
    ProviderFamily, ProviderRecord, ProviderRoute, ProviderState, Providers,
};

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The directory the registry keeps its records in, under the product root.
pub const REGISTRY_DIRECTORY: &str = "runtimes";

/// The file a product root carries, and the directory it addresses its projects under.
///
/// These mirror the layout `ymp-storage` writes. They are repeated here rather than shared,
/// because the registry must find a root by reading and `ymp-storage` materialises one by opening
/// it; a search that created what it was looking for would write a root under every path it walked
/// past. `crates/ymp-cli/tests/engine_registry.rs` drives the built product, so a layout that moved
/// would be caught there rather than here.
const LAYOUT_MARKER: &str = "root.json";
const PROJECTS_DIRECTORY: &str = "projects";

/// The record layout this build writes and reads. A record is not migrated.
pub const SCHEMA_VERSION: u32 = 1;

/// Why the Codex engine is disabled where no operator has stated otherwise. The account this host
/// authenticates with is over its usage limit until the date named, so an attempt routed to it
/// would spend the operator's time on a refusal from the provider.
pub const CODEX_DEFAULT_DISABLED_REASON: &str = "usage limit until 2026-09-12";

/// The engines this build manages.
///
/// The in-process fixture runtime is deliberately absent: it has no executable, no credential and
/// no models, so there is nothing about it to enable, measure or list.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Engine {
    Codex,
    ClaudeCode,
}

impl Engine {
    /// Every managed engine, in registry order. The order is the one every surface lists them in
    /// and the one a pool would later declare over.
    pub const ALL: [Self; 2] = [Self::Codex, Self::ClaudeCode];

    /// The engine name, spelled as the runtimes page spells it and as a command states it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude-code",
        }
    }

    /// The program the engine is installed as. Discovery searches for this name and nothing else.
    pub const fn program(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude",
        }
    }

    /// The engine a stated name selects. Spelling is exact: a name that selects no engine is
    /// refused rather than resolved to the nearest one.
    pub fn parse(value: &str) -> Result<Self, RegistryError> {
        Self::ALL
            .into_iter()
            .find(|engine| engine.name() == value.trim())
            .ok_or_else(|| RegistryError::UnknownEngine {
                name: value.trim().to_owned(),
                known: Self::ALL
                    .into_iter()
                    .map(Self::name)
                    .collect::<Vec<_>>()
                    .join(", "),
            })
    }

    /// Whether this engine is enabled where the registry holds no record of it.
    fn enabled_by_default(self) -> bool {
        !matches!(self, Self::Codex)
    }

    fn default_disabled_reason(self) -> Option<String> {
        match self {
            Self::Codex => Some(CODEX_DEFAULT_DISABLED_REASON.to_owned()),
            Self::ClaudeCode => None,
        }
    }
}

/// Where a model list came from.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    /// Nothing has been measured yet, or the engine was disabled when the catalog was refreshed.
    #[default]
    Unmeasured,
    /// The installed build named these models and refused every other candidate put to it.
    Measured,
    /// The installed build lists no catalog, so the list is the route the managed profile pins.
    Pinned,
    /// The build answered for every candidate it was asked about, but not every candidate the
    /// installed executable carries was asked. A list this incomplete is never read as the whole
    /// catalog, so the source says so instead of claiming a measurement of everything.
    Filtered,
}

impl ModelSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unmeasured => "unmeasured",
            Self::Measured => "measured",
            Self::Pinned => "pinned",
            Self::Filtered => "filtered",
        }
    }
}

/// The models an engine can serve, and where the list came from.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModelCatalog {
    pub source: ModelSource,
    /// The release the list was measured against, as that build reported it. It is stated for a
    /// reader; it decides nothing, because a record states it and a record can say anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_for_version: Option<String>,
    /// The digest of the executable the list was measured against.
    ///
    /// This decides one thing and it is worth naming exactly: whether the list belongs to the
    /// build that is installed now. It is computed from the installed file at every reading, so a
    /// record carrying another build's digest is measured again instead of being read as current.
    ///
    /// It says nothing about whether the names are the ones that build actually served. A record
    /// whose digest is honest and whose list was rewritten by hand is read as current, because the
    /// digest is of the executable and not of the measurement. Detecting that would need the
    /// record to be authenticated, which nothing here does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_for_digest: Option<String>,
    /// Why the list is empty or partial, when it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default)]
    pub names: Vec<String>,
}

impl ModelCatalog {
    /// Whether this list was measured against the executable that is installed now.
    ///
    /// The question is answered against the digest of that executable and nothing else, because
    /// the version a record names is only what the record says. What this establishes is that a
    /// list belongs to the installed build; it does not establish that the names in it are the
    /// ones that build served. An edited list under an honest digest is read as current, and
    /// nothing here would notice.
    ///
    /// That limit is bounded by what the list is allowed to decide. Admission does not read the
    /// catalog: whether an engine may be started is answered by the enabled flag alone, so a
    /// rewritten list cannot admit an engine the operator held back, and it cannot widen what a
    /// run may start. Which models a run may use is not decided in this crate at all.
    pub fn current_for(&self, executable_digest: &str) -> bool {
        self.source != ModelSource::Unmeasured
            && self.measured_for_digest.as_deref() == Some(executable_digest)
    }
}

/// What was measured about an engine. Every field is written from a measurement of this host.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct EngineProperties {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The digest of the executable these properties were measured from, computed from the file
    /// rather than reported by it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable_digest: Option<String>,
    /// Where the managed invocation's authentication material comes from, named rather than
    /// carried: the record states the origin and never the credential.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_budget_microusd: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_in_flight_overshoot_microusd: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_time_limit_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_limit_bytes: Option<u64>,
}

/// One engine as the registry holds it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EngineRecord {
    pub schema_version: u32,
    pub engine: String,
    pub enabled: bool,
    /// Why the engine is disabled. A refusal states this reason, so an operator is told what to
    /// change rather than that something is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    #[serde(default)]
    pub properties: EngineProperties,
    #[serde(default)]
    pub models: ModelCatalog,
}

impl EngineRecord {
    /// The record of an engine the registry holds none for.
    pub fn seeded(engine: Engine) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            engine: engine.name().to_owned(),
            enabled: engine.enabled_by_default(),
            disabled_reason: match engine.enabled_by_default() {
                true => None,
                false => engine.default_disabled_reason(),
            },
            properties: EngineProperties::default(),
            models: ModelCatalog::default(),
        }
    }

    /// Why this engine is not admitted, in the words the operator is shown.
    pub fn refusal_reason(&self) -> String {
        self.disabled_reason
            .clone()
            .unwrap_or_else(|| "no reason was recorded".to_owned())
    }
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error(
        "the {engine} engine is disabled in the registry — {reason}. Nothing is started: enable it \
         with `ymp runtime enable {engine}` or from /runtimes."
    )]
    Disabled { engine: String, reason: String },
    #[error("no runtime engine is named {name}; this build manages {known}")]
    UnknownEngine { name: String, known: String },
    #[error("the registry record {} is not readable: {reason}", path.display())]
    Unreadable { path: PathBuf, reason: String },
    #[error(
        "the registry record {} states schema version {found}; this build reads version {expected} \
         and migrates no record",
        path.display()
    )]
    UnsupportedSchema {
        path: PathBuf,
        found: u32,
        expected: u32,
    },
    #[error("registry I/O error at {}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
}

/// The engine registry under one product root.
#[derive(Clone, Debug)]
pub struct Registry {
    directory: PathBuf,
}

impl Registry {
    /// The registry under a root, taken literally.
    ///
    /// Callers that hold a path an invocation addressed want [`Registry::addressing`] instead: a
    /// path may be a store, and a store reads the registry of the root it stands under.
    pub fn under(root: impl AsRef<Path>) -> Self {
        Self {
            directory: root.as_ref().join(REGISTRY_DIRECTORY),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn path_of(&self, engine: Engine) -> PathBuf {
        self.directory.join(format!("{}.json", engine.name()))
    }

    /// Whether this root holds a record of its own for one engine.
    ///
    /// [`Registry::read`] answers the seeded record where none stands, which is what makes an
    /// engine usable before anything has been written about it. A caller that has to tell "nothing
    /// has been measured here" from "this is what was measured" asks this first: a record that was
    /// removed and a record that was never written are the same answer from `read`, and a surface
    /// that could not tell them apart would state a measured absence where there is none.
    pub fn has_record(&self, engine: Engine) -> bool {
        self.path_of(engine).is_file()
    }

    /// The record of one engine: what the registry holds, or the seeded record of an engine it
    /// holds nothing about.
    ///
    /// A record that exists and cannot be read is an error rather than a fallback to the seed. A
    /// registry that answered an unreadable disabled record with an enabled seed would admit the
    /// very engine the operator disabled.
    pub fn read(&self, engine: Engine) -> Result<EngineRecord, RegistryError> {
        let path = self.path_of(engine);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(EngineRecord::seeded(engine));
            }
            Err(source) => return Err(RegistryError::Io { path, source }),
        };
        let record: EngineRecord =
            serde_json::from_slice(&bytes).map_err(|error| RegistryError::Unreadable {
                path: path.clone(),
                reason: error.to_string(),
            })?;
        if record.schema_version != SCHEMA_VERSION {
            return Err(RegistryError::UnsupportedSchema {
                path,
                found: record.schema_version,
                expected: SCHEMA_VERSION,
            });
        }
        if record.engine != engine.name() {
            return Err(RegistryError::Unreadable {
                path,
                reason: format!(
                    "the record names the {} engine and stands in the place of {}",
                    record.engine,
                    engine.name()
                ),
            });
        }
        Ok(record)
    }

    /// Every managed engine, in registry order, each with the record the registry holds for it.
    pub fn read_all(&self) -> Vec<(Engine, Result<EngineRecord, RegistryError>)> {
        Engine::ALL
            .into_iter()
            .map(|engine| (engine, self.read(engine)))
            .collect()
    }

    /// Write one record, replacing it whole. The record is written beside its own path and renamed
    /// over it, so a reader sees the record before this write or the record after it.
    pub fn write(&self, engine: Engine, record: &EngineRecord) -> Result<(), RegistryError> {
        let path = self.path_of(engine);
        fs::create_dir_all(&self.directory).map_err(|source| RegistryError::Io {
            path: self.directory.clone(),
            source,
        })?;
        let mut bytes =
            serde_json::to_vec_pretty(record).map_err(|error| RegistryError::Unreadable {
                path: path.clone(),
                reason: error.to_string(),
            })?;
        bytes.push(b'\n');
        let staging = path.with_extension("json.writing");
        fs::write(&staging, &bytes).map_err(|source| RegistryError::Io {
            path: staging.clone(),
            source,
        })?;
        fs::rename(&staging, &path).map_err(|source| RegistryError::Io {
            path: path.clone(),
            source,
        })
    }

    /// Read one record, amend it, and write it back.
    pub fn update(
        &self,
        engine: Engine,
        amend: impl FnOnce(&mut EngineRecord),
    ) -> Result<EngineRecord, RegistryError> {
        let mut record = self.read(engine)?;
        amend(&mut record);
        record.schema_version = SCHEMA_VERSION;
        record.engine = engine.name().to_owned();
        self.write(engine, &record)?;
        Ok(record)
    }

    /// Enable or disable one engine. Disabling states a reason, because a refusal that names none
    /// tells the operator nothing about what to change.
    pub fn set_enabled(
        &self,
        engine: Engine,
        enabled: bool,
        reason: Option<&str>,
    ) -> Result<EngineRecord, RegistryError> {
        self.update(engine, |record| {
            record.enabled = enabled;
            record.disabled_reason = if enabled {
                None
            } else {
                Some(match reason.map(str::trim) {
                    Some(reason) if !reason.is_empty() => reason.to_owned(),
                    _ => "disabled by the operator".to_owned(),
                })
            };
            // A list measured while the engine was admitted says nothing about an engine that is
            // not, so it is kept and marked. A list that was never measured carries no such
            // history, and saying it has one would put that sentence where the reason the engine
            // is held back belongs.
            if !enabled && !record.models.names.is_empty() {
                record.models.note = Some("recorded while the engine was enabled".to_owned());
            }
        })
    }

    /// Admit one engine, or refuse it with the reason the registry records.
    ///
    /// Every path that would start an engine passes through here, so an engine the operator
    /// disabled cannot be reached by a caller that forgot to look.
    pub fn admit(&self, engine: Engine) -> Result<EngineRecord, RegistryError> {
        let record = self.read(engine)?;
        if !record.enabled {
            return Err(RegistryError::Disabled {
                engine: engine.name().to_owned(),
                reason: record.refusal_reason(),
            });
        }
        Ok(record)
    }
}

/// The product root a store stands under, or the store itself when it stands under none.
///
/// Which engines a host admits is one decision, and it is recorded under the root. An invocation
/// that names one exact store must therefore read that same decision, so the root is derived from
/// the store: a store addressed with `--data-root` reaches the registry its root holds, and an
/// engine the operator held back stays held back on every path that could start one.
///
/// The search begins at the path itself and stops at the first step that is a root. A path that
/// carries the layout marker **is** a root and nothing above it is asked about — a root nested
/// inside another root's projects, which is what an agent workspace holds, keeps its own decision
/// rather than inheriting the outer one. Above the zero step, a root is an ancestor that carries
/// the marker and addresses this store under its projects. A store standing under no such root —
/// one an earlier layout wrote, or one an operator keeps apart on purpose — has no root decision to
/// honour, and its registry stands beside it.
///
/// This derives a root from a **store**. A path an operator named as the root is a root already;
/// passing it through here would answer with a decision they did not make, so callers state which
/// of the two they hold through [`RegistryAddress`].
pub fn root_of_store(store: &Path) -> PathBuf {
    let store = store.canonicalize().unwrap_or_else(|_| store.to_path_buf());
    for ancestor in store.ancestors() {
        if !ancestor.join(LAYOUT_MARKER).is_file() {
            continue;
        }
        if ancestor == store || store.starts_with(ancestor.join(PROJECTS_DIRECTORY)) {
            return ancestor.to_path_buf();
        }
    }
    store
}

/// How an invocation addresses the engine registry.
///
/// The two are not interchangeable, and reading one as the other is how a decision goes missing.
/// A root an operator named is the registry: it is taken as stated, so `--root <dir>` records and
/// reads under `<dir>` whatever stands above it. A store is where a run's state lives, and the
/// decision that governs it belongs to the root it stands under, so that root is derived.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryAddress {
    /// A root the invocation named, taken exactly as stated.
    Root(PathBuf),
    /// A store the invocation named. The root it stands under is derived.
    Store(PathBuf),
}

impl RegistryAddress {
    /// The root this address resolves to.
    pub fn root(&self) -> PathBuf {
        match self {
            Self::Root(root) => root.clone(),
            Self::Store(store) => root_of_store(store),
        }
    }

    /// The registry this address reaches.
    pub fn registry(&self) -> Registry {
        Registry::under(self.root())
    }

    /// The provider records this address reaches. They stand under the same root the engine
    /// records do, so an invocation that names a store reads the accounts of the root that
    /// governs it rather than a set of its own.
    pub fn providers(&self) -> Providers {
        Providers::under(self.root())
    }

    /// The pool records this address reaches. They stand under the same root, for the same reason:
    /// which entries a run may recruit from is one decision per root, and a run addressed by its
    /// store reads the pools of the root that governs it.
    pub fn pools(&self) -> Pools {
        Pools::under(self.root())
    }
}

/// Where an engine's executable stands on this host.
///
/// This is the one discovery channel there is. An engine that also resolved its executable from a
/// configuration directory would be selectable by a path no record states, so nothing outside this
/// function searches for an engine.
pub fn discover(program: &str) -> PathBuf {
    resolve(PathBuf::from(program))
}

/// The executable a stated path or program name resolves to.
///
/// A path is taken as stated and canonicalised where it exists; a bare program name is searched
/// for on the process search path. A name that resolves to nothing is returned unchanged, so the
/// probe reports it as not installed rather than as some other file.
pub fn resolve(executable: PathBuf) -> PathBuf {
    if executable.is_absolute() || executable.components().count() > 1 {
        return executable.canonicalize().unwrap_or(executable);
    }
    let Some(path) = std::env::var_os("PATH") else {
        return executable;
    };
    std::env::split_paths(&path)
        .map(|directory| directory.join(&executable))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| candidate.canonicalize().ok())
        .unwrap_or(executable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> (tempfile::TempDir, Registry) {
        let root = tempfile::tempdir().expect("temporary root");
        let registry = Registry::under(root.path());
        (root, registry)
    }

    #[test]
    fn an_engine_the_registry_holds_nothing_about_takes_its_seeded_state() {
        let (_root, registry) = registry();
        let claude = registry.read(Engine::ClaudeCode).expect("claude record");
        assert!(claude.enabled);
        assert!(claude.disabled_reason.is_none());

        let codex = registry.read(Engine::Codex).expect("codex record");
        assert!(!codex.enabled, "codex is disabled on a host with no record");
        assert_eq!(
            codex.disabled_reason.as_deref(),
            Some(CODEX_DEFAULT_DISABLED_REASON)
        );
    }

    #[test]
    fn a_disabled_engine_is_refused_with_the_reason_the_registry_records() {
        let (_root, registry) = registry();
        registry
            .set_enabled(Engine::Codex, false, Some("usage limit until 2026-09-12"))
            .expect("disable codex");
        let error = registry
            .admit(Engine::Codex)
            .expect_err("codex is disabled");
        let stated = error.to_string();
        assert!(stated.contains("usage limit until 2026-09-12"), "{stated}");
        assert!(stated.contains("codex"), "{stated}");
        registry.admit(Engine::ClaudeCode).expect("claude admitted");
    }

    #[test]
    fn enabling_an_engine_clears_the_reason_and_survives_a_reread() {
        let (_root, registry) = registry();
        registry
            .set_enabled(Engine::Codex, true, None)
            .expect("enable codex");
        let record = registry.read(Engine::Codex).expect("codex record");
        assert!(record.enabled);
        assert!(record.disabled_reason.is_none());
        registry.admit(Engine::Codex).expect("codex admitted");
    }

    #[test]
    fn disabling_without_a_stated_reason_still_states_one() {
        let (_root, registry) = registry();
        let record = registry
            .set_enabled(Engine::ClaudeCode, false, Some("   "))
            .expect("disable claude");
        assert_eq!(
            record.disabled_reason.as_deref(),
            Some("disabled by the operator")
        );
    }

    /// A record that exists and cannot be read is never answered with the seed: an unreadable
    /// disabled record would otherwise admit the engine the operator disabled.
    #[test]
    fn an_unreadable_record_refuses_rather_than_falling_back_to_the_seed() {
        let (_root, registry) = registry();
        registry
            .set_enabled(Engine::ClaudeCode, false, Some("held back"))
            .expect("disable claude");
        fs::write(registry.path_of(Engine::ClaudeCode), b"{ not json").expect("corrupt the record");
        let error = registry
            .admit(Engine::ClaudeCode)
            .expect_err("an unreadable record is not an enabled engine");
        assert!(
            matches!(error, RegistryError::Unreadable { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn a_record_of_another_schema_version_is_refused_rather_than_migrated() {
        let (_root, registry) = registry();
        let mut record = EngineRecord::seeded(Engine::ClaudeCode);
        record.schema_version = SCHEMA_VERSION + 1;
        fs::create_dir_all(registry.directory()).expect("registry directory");
        fs::write(
            registry.path_of(Engine::ClaudeCode),
            serde_json::to_vec(&record).expect("record bytes"),
        )
        .expect("write record");
        let error = registry
            .read(Engine::ClaudeCode)
            .expect_err("a later record is not read");
        assert!(
            matches!(error, RegistryError::UnsupportedSchema { .. }),
            "{error:?}"
        );
    }

    /// The registry a store reaches is the one its root holds, whichever way the store was
    /// addressed. This is the property that keeps an engine the operator held back held back on
    /// every path: naming the store instead of the root no longer reaches a different registry.
    #[test]
    fn a_store_under_a_root_addresses_the_registry_that_root_holds() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("root");
        let store = root.join("projects").join("project-1").join("runs/0001");
        fs::create_dir_all(&store).expect("store directories");
        fs::write(root.join("root.json"), br#"{"schema_version":1}"#).expect("layout marker");
        assert_eq!(
            root_of_store(&store),
            root.canonicalize().expect("root"),
            "a store under a root addressed a registry of its own"
        );

        // A store standing under no root has no root decision to honour, and its registry stands
        // beside it. A marker that does not address this store under its projects is not its root.
        let apart = directory.path().join("apart");
        fs::create_dir_all(&apart).expect("store apart");
        assert_eq!(root_of_store(&apart), apart.canonicalize().expect("apart"));
        let beside = root.join("beside");
        fs::create_dir_all(&beside).expect("store beside the projects");
        assert_eq!(
            root_of_store(&beside),
            beside.canonicalize().expect("beside")
        );
    }

    /// A path that carries the layout marker is a root, and the search stops there.
    ///
    /// Both halves matter. A root nested inside another root's projects — which is what an agent
    /// workspace under a store holds — keeps its own decision instead of inheriting the outer
    /// one; and a path that is a root is never walked past, so a decision recorded at it is the
    /// one that is read back.
    #[test]
    fn a_path_that_is_itself_a_root_is_where_the_search_stops() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let outer = directory.path().join("outer");
        let workspace = outer
            .join("projects")
            .join("project-1")
            .join("runs/0001/workspaces/w");
        let inner = workspace.join("inner");
        let inner_store = inner.join("projects").join("project-2").join("runs/0001");
        fs::create_dir_all(&inner_store).expect("nested store directories");
        fs::write(outer.join("root.json"), br#"{"schema_version":1}"#).expect("outer marker");
        fs::write(inner.join("root.json"), br#"{"schema_version":1}"#).expect("inner marker");

        let inner = inner.canonicalize().expect("inner root");
        assert_eq!(
            root_of_store(&inner),
            inner,
            "a path carrying the layout marker was walked past as if it were a store"
        );
        assert_eq!(
            root_of_store(&inner_store),
            inner,
            "a store under a nested root inherited the outer root's decision"
        );

        // The workspace itself carries no marker, so it belongs to the root above it.
        assert_eq!(
            root_of_store(&workspace),
            outer.canonicalize().expect("outer root")
        );
    }

    /// A root an operator named is the registry. Deriving another one from it would record and
    /// read a decision they did not make — and a decision written where nothing reads it is a
    /// disabled engine that still starts.
    #[test]
    fn a_named_root_is_taken_as_stated_and_a_named_store_resolves_its_root() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let outer = directory.path().join("outer");
        let inner = outer.join("projects").join("project-1").join("inner");
        let store = outer.join("projects").join("project-1").join("runs/0001");
        fs::create_dir_all(&inner).expect("inner directories");
        fs::create_dir_all(&store).expect("store directories");
        fs::write(outer.join("root.json"), br#"{"schema_version":1}"#).expect("outer marker");

        let named = RegistryAddress::Root(inner.clone());
        assert_eq!(named.root(), inner, "a named root was resolved to another");
        assert_eq!(
            named.registry().path_of(Engine::ClaudeCode),
            inner.join("runtimes").join("claude-code.json")
        );

        // A store is the other case, and it does resolve: the decision that governs a run belongs
        // to the root its store stands under.
        assert_eq!(
            RegistryAddress::Store(store).root(),
            outer.canonicalize().expect("outer root")
        );
    }

    /// Disabling an engine nothing was measured for records no history it does not have, so the
    /// page has the reason to show rather than a sentence about a list that never existed.
    #[test]
    fn disabling_an_unmeasured_engine_records_no_history_of_a_list() {
        let (_root, registry) = registry();
        let record = registry
            .set_enabled(Engine::ClaudeCode, false, Some("held back"))
            .expect("disable claude");
        assert!(record.models.names.is_empty());
        assert_eq!(record.models.note, None, "{:?}", record.models);

        // A list that was measured keeps its history, because it was true when it was written.
        registry
            .update(Engine::ClaudeCode, |record| {
                record.models.source = ModelSource::Measured;
                record.models.names = vec!["claude-haiku-4-5".to_owned()];
            })
            .expect("record a measurement");
        let record = registry
            .set_enabled(Engine::ClaudeCode, false, Some("held back"))
            .expect("disable claude again");
        assert_eq!(
            record.models.note.as_deref(),
            Some("recorded while the engine was enabled")
        );
    }

    /// A record decides nothing about whether its own list is current. The digest is computed from
    /// the installed executable at every reading, so a record naming the installed release while
    /// holding another build's list is measured again instead of suppressing its re-measurement.
    #[test]
    fn a_recorded_version_cannot_declare_a_stale_list_current() {
        let forged = ModelCatalog {
            source: ModelSource::Measured,
            measured_for_version: Some("2.1.233 (Claude Code)".to_owned()),
            measured_for_digest: Some("a-digest-of-some-other-build".to_owned()),
            note: None,
            names: vec!["claude-from-another-build".to_owned()],
        };
        assert!(
            !forged.current_for("the-digest-of-the-installed-build"),
            "a record declared its own stale list current"
        );
        let honest = ModelCatalog {
            measured_for_digest: Some("the-digest-of-the-installed-build".to_owned()),
            ..forged
        };
        assert!(honest.current_for("the-digest-of-the-installed-build"));
    }

    #[test]
    fn measured_properties_and_a_model_list_survive_a_write_and_a_read() {
        let (_root, registry) = registry();
        registry
            .update(Engine::ClaudeCode, |record| {
                record.properties = EngineProperties {
                    executable: Some("/usr/local/bin/claude".to_owned()),
                    version: Some("2.1.233 (Claude Code)".to_owned()),
                    executable_digest: Some("build-digest".to_owned()),
                    credential_origin: Some("delegated_host_keychain_credential".to_owned()),
                    max_budget_microusd: Some(1_000_000),
                    max_in_flight_overshoot_microusd: Some(50_000),
                    wall_time_limit_ms: Some(600_000),
                    output_limit_bytes: Some(16 * 1024 * 1024),
                };
                record.models = ModelCatalog {
                    source: ModelSource::Measured,
                    measured_for_version: Some("2.1.233 (Claude Code)".to_owned()),
                    measured_for_digest: Some("build-digest".to_owned()),
                    note: None,
                    names: vec!["claude-haiku-4-5".to_owned(), "claude-sonnet-5".to_owned()],
                };
            })
            .expect("record the measurement");
        let record = registry.read(Engine::ClaudeCode).expect("claude record");
        assert_eq!(
            record.properties.version.as_deref(),
            Some("2.1.233 (Claude Code)")
        );
        assert!(record.models.current_for("build-digest"));
        assert!(!record.models.current_for("another-build-digest"));
        assert_eq!(record.models.names.len(), 2);
    }

    #[test]
    fn an_engine_name_selects_exactly_one_engine_or_none() {
        assert_eq!(
            Engine::parse("claude-code").expect("claude"),
            Engine::ClaudeCode
        );
        assert_eq!(Engine::parse(" codex ").expect("codex"), Engine::Codex);
        let error = Engine::parse("claude").expect_err("no engine is named claude");
        assert!(error.to_string().contains("claude-code"), "{error}");
    }

    #[test]
    fn discovery_resolves_a_named_path_and_leaves_an_unresolvable_name_as_it_stands() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let named = directory.path().join("engine");
        fs::write(&named, b"#!/bin/sh\n").expect("write the engine");
        assert_eq!(resolve(named.clone()), named.canonicalize().expect("named"));
        assert_eq!(
            discover("ymp-no-such-engine-on-this-host"),
            PathBuf::from("ymp-no-such-engine-on-this-host")
        );
    }
}

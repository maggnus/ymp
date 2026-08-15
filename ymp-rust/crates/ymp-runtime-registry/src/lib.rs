#![forbid(unsafe_code)]

//! The engine registry: which runtime engines this host admits, and what was measured about them.
//!
//! An engine is a managed entity rather than a compiled-in constant. One record per engine lives
//! under the product root:
//!
//! ```text
//! .ymp/
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
//! The registry is the level the collective design calls the model catalog
//! (`ymp-docs/design/COLLECTIVE-DESIGN.md`, §6). It deliberately stops short of the pool: it
//! records what an engine offers and whether the engine is admitted, and it decides nothing about
//! which of those models a run may use.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The directory the registry keeps its records in, under the product root.
pub const REGISTRY_DIRECTORY: &str = "runtimes";

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
}

impl ModelSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unmeasured => "unmeasured",
            Self::Measured => "measured",
            Self::Pinned => "pinned",
        }
    }
}

/// The models an engine can serve, and where the list came from.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModelCatalog {
    pub source: ModelSource,
    /// The release the list was measured against. A list measured against another build is stale
    /// and is refreshed rather than believed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_for_version: Option<String>,
    /// Why the list is empty or partial, when it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default)]
    pub names: Vec<String>,
}

impl ModelCatalog {
    /// Whether this list was measured against the release named.
    pub fn current_for(&self, version: &str) -> bool {
        self.source != ModelSource::Unmeasured
            && self.measured_for_version.as_deref() == Some(version)
    }
}

/// What was measured about an engine. Every field is written from a measurement of this host.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct EngineProperties {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
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
    /// The registry under a root. The root is a parameter and is never computed here: which root
    /// an invocation addresses is decided where the invocation is read.
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
            if !enabled {
                // A list measured while the engine was admitted says nothing about an engine that
                // is not. It is kept but marked, so re-enabling measures it again.
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

    #[test]
    fn measured_properties_and_a_model_list_survive_a_write_and_a_read() {
        let (_root, registry) = registry();
        registry
            .update(Engine::ClaudeCode, |record| {
                record.properties = EngineProperties {
                    executable: Some("/usr/local/bin/claude".to_owned()),
                    version: Some("2.1.233 (Claude Code)".to_owned()),
                    credential_origin: Some("delegated_host_keychain_credential".to_owned()),
                    max_budget_microusd: Some(1_000_000),
                    max_in_flight_overshoot_microusd: Some(50_000),
                    wall_time_limit_ms: Some(600_000),
                    output_limit_bytes: Some(16 * 1024 * 1024),
                };
                record.models = ModelCatalog {
                    source: ModelSource::Measured,
                    measured_for_version: Some("2.1.233 (Claude Code)".to_owned()),
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
        assert!(record.models.current_for("2.1.233 (Claude Code)"));
        assert!(!record.models.current_for("2.1.234 (Claude Code)"));
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

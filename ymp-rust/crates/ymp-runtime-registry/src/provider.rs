//! The provider level of the product root: which accounts this host reaches models through, and
//! what was measured about each of them.
//!
//! A provider is an account with an authentication state that exposes models. It is not a runtime
//! and not a model, and until this level existed it was implicit: the root held one record per
//! engine, and which account a spend would reach could only be inferred from the engine's name.
//! One record per provider now stands beside the engine records:
//!
//! ```text
//! .ymp/
//!   providers/anthropic.json        family, observed state, the engines that reach it
//!   providers/openai.json
//!   runtimes/claude-code.json       the engine records this level is observed from
//!   runtimes/codex.json
//! ```
//!
//! **Every provider is reached through an installed engine**
//! (`ymp-docs/design/COLLECTIVE-DESIGN.md`, §5), so an engine is an entity beneath a provider
//! rather than beside it: [`ProviderFamily::engines`] names the engines that reach one provider,
//! [`Engine::provider`] names the provider one engine reaches on its own credential, and each
//! [`ProviderRoute`] records what that engine measured.
//!
//! **A provider record is observed, never seeded.** Every field but the record's own identity is
//! derived from the measurements the engine records already carry ([`Providers::observe`]), so a
//! root that has observed nothing holds no provider record at all and [`Providers::read`] answers
//! `None`. An engine record is seeded because its enabled flag is an operator decision that has a
//! product default; a provider's state has no default, and writing one would put a declaration
//! where the level exists to hold a measurement.
//!
//! Observation opens no process and reaches no network: it reads the records under the root it is
//! given. What it can therefore state is bounded by what a probe wrote, and the reason on every
//! record says which measurement it came from.
//!
//! The design names four states a provider row can show — `ready`, `not configured`,
//! `needs authentication` and `unavailable`. This build writes three of them. Nothing here
//! authenticates against a provider, so no measurement distinguishes a credential the provider
//! rejects from one it accepts, and a record that claimed `needs authentication` would be stating
//! the result of a check no code performs.
//!
//! This level records where a credential is read from and never the credential itself, and it
//! decides nothing: which engines are admitted is still answered by the engine record's enabled
//! flag alone ([`Registry::admit`]), and which models a run may use is not answered under this
//! root at all.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Engine, EngineRecord, Registry, RegistryError};

/// The directory the provider records stand in, under the product root.
pub const PROVIDERS_DIRECTORY: &str = "providers";

/// The record layout this build writes and reads. A record is not migrated.
pub const PROVIDER_SCHEMA_VERSION: u32 = 1;

/// The provider families this host can reach.
///
/// The family is the vendor the account belongs to. In this build every provider is reached
/// through the engine of its own family, so the family also names the record; a route override
/// reaching one vendor's models through another vendor's engine
/// (`ymp-docs/design/COLLECTIVE-DESIGN.md`, §5) is a later unit, and the record shape already
/// keeps the two apart — [`ProviderRecord::family`] is the account's vendor and
/// [`ProviderRecord::routes`] names the engines that reach it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFamily {
    Anthropic,
    OpenAi,
}

impl ProviderFamily {
    /// Every provider this build reaches, in the order every surface lists them in.
    pub const ALL: [Self; 2] = [Self::Anthropic, Self::OpenAi];

    /// The provider name, spelled as a record and a command spell it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
        }
    }

    /// The provider a stated name selects. Spelling is exact: a name that selects no provider is
    /// refused rather than resolved to the nearest one.
    pub fn parse(value: &str) -> Result<Self, ProviderError> {
        Self::ALL
            .into_iter()
            .find(|provider| provider.name() == value.trim())
            .ok_or_else(|| ProviderError::UnknownProvider {
                name: value.trim().to_owned(),
                known: Self::ALL
                    .into_iter()
                    .map(Self::name)
                    .collect::<Vec<_>>()
                    .join(", "),
            })
    }

    /// The engines this build reaches the provider through, in registry order.
    ///
    /// This is the inverse of [`Engine::provider`] and the two are checked against each other, so
    /// an engine can never stand beneath one provider and be reached from another.
    pub const fn engines(self) -> &'static [Engine] {
        match self {
            Self::Anthropic => &[Engine::ClaudeCode],
            Self::OpenAi => &[Engine::Codex],
        }
    }
}

impl Engine {
    /// The provider this engine reaches on its own credential.
    ///
    /// It is a property of the driver rather than of this host: the Claude Code build
    /// authenticates against Anthropic and the Codex build against OpenAI, whatever any record
    /// says. What is measured on a host is whether that engine is installed and where its
    /// credential is read from, and those are the fields a [`ProviderRoute`] carries.
    pub const fn provider(self) -> ProviderFamily {
        match self {
            Self::ClaudeCode => ProviderFamily::Anthropic,
            Self::Codex => ProviderFamily::OpenAi,
        }
    }

    /// The compiled driver that supervises this engine's protocol.
    ///
    /// The driver is not the engine and not the model: it is this product's own implementation,
    /// internal to the build. It is named here so a reader can see the distinction the design
    /// keeps (`ymp-docs/design/COLLECTIVE-DESIGN.md`, §17), and it is deliberately absent from
    /// every stored record — a durable object naming a crate would make an internal boundary part
    /// of the layout.
    pub const fn driver(self) -> &'static str {
        match self {
            Self::ClaudeCode => "ymp-runtime-claude",
            Self::Codex => "ymp-runtime-codex",
        }
    }
}

/// What the observed measurements say about a provider, in the words a provider row shows.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderState {
    /// An enabled engine reported a release on this host and states where its credential is read
    /// from. Nothing here establishes that the provider accepts that credential.
    Ready,
    /// An enabled engine reported a release on this host and nothing states where a credential
    /// would be read from, so no spend could reach the account.
    NotConfigured,
    /// No engine that reaches this provider is both admitted and installed here.
    Unavailable,
}

impl ProviderState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NotConfigured => "not configured",
            Self::Unavailable => "unavailable",
        }
    }
}

/// One engine as the provider it reaches records it.
///
/// The fields are copies of what the engine record held when the observation was made, and they
/// are held here for one reason: they say which engine build the provider state was observed from.
/// An engine that has since been replaced makes the observation visibly older than what is
/// installed ([`ProviderRoute::current_for`]) instead of silently standing for it. Everything a
/// route decides is decided from the engine record, never from this copy.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderRoute {
    pub engine: String,
    /// The engine's admission decision as the registry recorded it.
    pub enabled: bool,
    /// Why this route carries no measurement: the reason the engine is disabled, or why its
    /// record could not be read. Either way the provider is never stated ready from this route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub withheld_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_executable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_digest: Option<String>,
    /// Where the managed invocation's authentication material comes from, named rather than
    /// carried: the record states the origin and never the credential.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_origin: Option<String>,
}

impl ProviderRoute {
    /// The route an engine record states.
    fn observed(engine: Engine, record: &EngineRecord) -> Self {
        Self {
            engine: engine.name().to_owned(),
            enabled: record.enabled,
            withheld_reason: match record.enabled {
                true => None,
                false => Some(record.refusal_reason()),
            },
            engine_executable: record.properties.executable.clone(),
            engine_version: record.properties.version.clone(),
            engine_digest: record.properties.executable_digest.clone(),
            credential_origin: record.properties.credential_origin.clone(),
        }
    }

    /// The route of an engine whose record could not be read.
    ///
    /// It carries no measurement and is not enabled, because a record that cannot be read is never
    /// answered with what it might have said. The reason is the one the read failed with.
    fn unreadable(engine: Engine, error: &RegistryError) -> Self {
        Self {
            engine: engine.name().to_owned(),
            enabled: false,
            withheld_reason: Some(error.to_string()),
            ..Self::default()
        }
    }

    /// Whether this route can carry a spend at all: the engine is admitted and reported a release
    /// on this host. It says nothing about a credential.
    pub fn usable(&self) -> bool {
        self.enabled && self.engine_version.is_some()
    }

    /// Whether the engine build this route was observed from is the one installed now.
    ///
    /// The digest is of the executable the observation read, so the caller supplies the digest of
    /// the file installed now; nothing here opens an executable.
    pub fn current_for(&self, executable_digest: &str) -> bool {
        self.engine_digest.as_deref() == Some(executable_digest)
    }
}

/// One provider as the product root holds it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderRecord {
    pub schema_version: u32,
    /// The record's own name, which is the file it stands in. A record found under another
    /// provider's name is refused rather than read as that provider's.
    pub provider: String,
    /// The vendor the account belongs to.
    pub family: ProviderFamily,
    pub state: ProviderState,
    /// What was measured, in one sentence an operator can act on.
    pub reason: String,
    /// The engines that reach this provider, in registry order.
    #[serde(default)]
    pub routes: Vec<ProviderRoute>,
}

impl ProviderRecord {
    /// The record a provider takes from what its engines measured.
    ///
    /// The order of the questions is the order an operator can act in: an engine held back is
    /// their own decision, an engine that reported no release is a host problem, and a missing
    /// credential is a provider problem. The first one that applies is the one stated.
    fn observed(family: ProviderFamily, routes: Vec<ProviderRoute>) -> Self {
        let ready = routes.iter().find_map(|route| {
            route
                .credential_origin
                .as_deref()
                .filter(|_| route.usable())
                .map(|origin| (route, origin))
        });
        let usable = routes.iter().find(|route| route.usable());
        let (state, reason) = match (ready, usable) {
            (Some((route, origin)), _) => (
                ProviderState::Ready,
                format!(
                    "reached through the {} engine; its credential is read from {origin}",
                    route.engine
                ),
            ),
            (None, Some(route)) => (
                ProviderState::NotConfigured,
                format!(
                    "the {} engine is installed here and nothing states where a credential would \
                     be read from, so no spend could reach this account",
                    route.engine
                ),
            ),
            (None, None) => (
                ProviderState::Unavailable,
                unavailable_reason(family, &routes),
            ),
        };
        Self {
            schema_version: PROVIDER_SCHEMA_VERSION,
            provider: family.name().to_owned(),
            family,
            state,
            reason,
            routes,
        }
    }

    /// The route one engine holds under this provider.
    pub fn route(&self, engine: Engine) -> Option<&ProviderRoute> {
        self.routes
            .iter()
            .find(|route| route.engine == engine.name())
    }
}

/// Why no engine reaches a provider, naming each engine that would have.
fn unavailable_reason(family: ProviderFamily, routes: &[ProviderRoute]) -> String {
    if routes.is_empty() {
        return format!("no engine this build manages reaches {}", family.name());
    }
    let stated: Vec<String> = routes
        .iter()
        .map(
            |route| match (&route.withheld_reason, &route.engine_executable) {
                (Some(reason), _) => format!("{}: {reason}", route.engine),
                (None, Some(executable)) => format!(
                    "{}: {executable} reported no release this build admits",
                    route.engine
                ),
                (None, None) => format!(
                    "{}: nothing has been measured about this engine on this host",
                    route.engine
                ),
            },
        )
        .collect();
    format!("no engine reaches it here — {}", stated.join("; "))
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("no provider is named {name}; this build reaches {known}")]
    UnknownProvider { name: String, known: String },
    /// An engine record this level stands on could not be read. Observation records such a failure
    /// as the route's own reason and still writes a record; reading the catalog fails instead,
    /// because a catalog missing one engine's models looks exactly like a catalog that has them
    /// all.
    #[error("an engine record this provider stands on could not be read: {source}")]
    Engine {
        #[from]
        source: RegistryError,
    },
    #[error("the provider record {} is not readable: {reason}", path.display())]
    Unreadable { path: PathBuf, reason: String },
    #[error(
        "the provider record {} states schema version {found}; this build reads version \
         {expected} and migrates no record",
        path.display()
    )]
    UnsupportedSchema {
        path: PathBuf,
        found: u32,
        expected: u32,
    },
    #[error("provider I/O error at {}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
}

/// The provider records under one product root.
///
/// Callers that hold a path an invocation addressed reach this through
/// [`crate::RegistryAddress::providers`], so the providers of a run are the ones its root holds,
/// exactly as its engine registry is.
#[derive(Clone, Debug)]
pub struct Providers {
    directory: PathBuf,
}

impl Providers {
    /// The provider records under a root, taken literally.
    pub fn under(root: impl AsRef<Path>) -> Self {
        Self {
            directory: root.as_ref().join(PROVIDERS_DIRECTORY),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn path_of(&self, family: ProviderFamily) -> PathBuf {
        self.directory.join(format!("{}.json", family.name()))
    }

    /// The record of one provider, or `None` where this root has observed none.
    ///
    /// A record that exists and cannot be read is an error rather than an absence: answering an
    /// unreadable record with `None` would read a broken record as a provider nobody has looked
    /// at yet, and the next observation would write over it.
    pub fn read(&self, family: ProviderFamily) -> Result<Option<ProviderRecord>, ProviderError> {
        let path = self.path_of(family);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(ProviderError::Io { path, source }),
        };
        let record: ProviderRecord =
            serde_json::from_slice(&bytes).map_err(|error| ProviderError::Unreadable {
                path: path.clone(),
                reason: error.to_string(),
            })?;
        if record.schema_version != PROVIDER_SCHEMA_VERSION {
            return Err(ProviderError::UnsupportedSchema {
                path,
                found: record.schema_version,
                expected: PROVIDER_SCHEMA_VERSION,
            });
        }
        if record.provider != family.name() {
            return Err(ProviderError::Unreadable {
                path,
                reason: format!(
                    "the record names the {} provider and stands in the place of {}",
                    record.provider,
                    family.name()
                ),
            });
        }
        Ok(Some(record))
    }

    /// Every provider this build reaches, in provider order, each with what this root holds for it.
    pub fn read_all(
        &self,
    ) -> Vec<(
        ProviderFamily,
        Result<Option<ProviderRecord>, ProviderError>,
    )> {
        ProviderFamily::ALL
            .into_iter()
            .map(|family| (family, self.read(family)))
            .collect()
    }

    /// Write one record, replacing it whole. The record is written beside its own path and renamed
    /// over it, so a reader sees the record before this write or the record after it.
    pub fn write(
        &self,
        family: ProviderFamily,
        record: &ProviderRecord,
    ) -> Result<(), ProviderError> {
        let path = self.path_of(family);
        fs::create_dir_all(&self.directory).map_err(|source| ProviderError::Io {
            path: self.directory.clone(),
            source,
        })?;
        let mut bytes =
            serde_json::to_vec_pretty(record).map_err(|error| ProviderError::Unreadable {
                path: path.clone(),
                reason: error.to_string(),
            })?;
        bytes.push(b'\n');
        let staging = path.with_extension("json.writing");
        fs::write(&staging, &bytes).map_err(|source| ProviderError::Io {
            path: staging.clone(),
            source,
        })?;
        fs::rename(&staging, &path).map_err(|source| ProviderError::Io {
            path: path.clone(),
            source,
        })
    }

    /// Observe every provider from the engine records this root holds, and write what was
    /// observed.
    ///
    /// This is the only writer of a provider record. It starts nothing, spends nothing and reaches
    /// no network: the measurements it states were made when the engines were probed, and it reads
    /// them where they were recorded. Running it twice over unchanged engine records writes the
    /// same records.
    pub fn observe(&self, registry: &Registry) -> Result<Vec<ProviderRecord>, ProviderError> {
        let mut observed = Vec::with_capacity(ProviderFamily::ALL.len());
        for family in ProviderFamily::ALL {
            let routes = family
                .engines()
                .iter()
                .map(|engine| match registry.read(*engine) {
                    Ok(record) => ProviderRoute::observed(*engine, &record),
                    Err(error) => ProviderRoute::unreadable(*engine, &error),
                })
                .collect();
            let record = ProviderRecord::observed(family, routes);
            self.write(family, &record)?;
            observed.push(record);
        }
        Ok(observed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelCatalog, ModelSource};

    fn root() -> (tempfile::TempDir, Registry, Providers) {
        let directory = tempfile::tempdir().expect("temporary root");
        let registry = Registry::under(directory.path());
        let providers = Providers::under(directory.path());
        (directory, registry, providers)
    }

    /// A measured, admitted engine with a credential, as a probe would have recorded it.
    fn measure(registry: &Registry, engine: Engine, credential: Option<&str>) {
        registry
            .update(engine, |record| {
                record.properties.executable = Some(format!("/usr/local/bin/{}", engine.program()));
                record.properties.version = Some("1.2.3".to_owned());
                record.properties.executable_digest = Some(format!("{}-digest", engine.name()));
                record.properties.credential_origin = credential.map(str::to_owned);
                record.models = ModelCatalog {
                    source: ModelSource::Measured,
                    measured_for_version: Some("1.2.3".to_owned()),
                    measured_for_digest: Some(format!("{}-digest", engine.name())),
                    note: None,
                    names: vec!["model-a".to_owned(), "model-b".to_owned()],
                };
            })
            .expect("record the measurement");
    }

    /// The negative half of this level: a root that holds engine records alone has no provider
    /// level at all, and the account an engine would reach is nowhere stated. Observation is what
    /// makes it readable, and it states the engine beneath the provider.
    #[test]
    fn a_root_holding_only_engine_records_holds_no_provider_until_it_is_observed() {
        let (_directory, registry, providers) = root();
        measure(
            &registry,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
        );
        assert!(
            registry
                .read(Engine::ClaudeCode)
                .expect("engine record")
                .properties
                .credential_origin
                .is_some(),
            "the engine measurement this test observes from is missing"
        );
        assert!(
            providers
                .read(ProviderFamily::Anthropic)
                .expect("provider read")
                .is_none(),
            "a provider record existed before anything observed one"
        );
        assert!(!providers.directory().exists());

        providers.observe(&registry).expect("observe the providers");
        let record = providers
            .read(ProviderFamily::Anthropic)
            .expect("provider read")
            .expect("anthropic is observed");
        assert_eq!(record.state, ProviderState::Ready, "{}", record.reason);
        assert_eq!(record.family, ProviderFamily::Anthropic);
        let route = record.route(Engine::ClaudeCode).expect("the engine route");
        assert!(route.usable());
        assert_eq!(
            route.credential_origin.as_deref(),
            Some("delegated_host_keychain_credential")
        );
        assert!(route.current_for("claude-code-digest"));
        assert!(!route.current_for("another-build"));
    }

    /// The state an operator can act on is the first one that applies, and the reason names what
    /// to change.
    #[test]
    fn a_provider_states_the_first_reason_an_operator_can_act_on() {
        let (_directory, registry, providers) = root();

        // Nothing measured at all: the engine is admitted and this host knows nothing about it.
        providers.observe(&registry).expect("observe");
        let record = providers
            .read(ProviderFamily::Anthropic)
            .expect("read")
            .expect("observed");
        assert_eq!(record.state, ProviderState::Unavailable);
        assert!(record.reason.contains("claude-code"), "{}", record.reason);

        // Measured and installed, with nothing stating where a credential is read from.
        measure(&registry, Engine::ClaudeCode, None);
        providers.observe(&registry).expect("observe");
        let record = providers
            .read(ProviderFamily::Anthropic)
            .expect("read")
            .expect("observed");
        assert_eq!(
            record.state,
            ProviderState::NotConfigured,
            "{}",
            record.reason
        );
        assert!(record.reason.contains("credential"), "{}", record.reason);

        // Held back by the operator: their decision is stated ahead of the host's state.
        measure(
            &registry,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
        );
        registry
            .set_enabled(
                Engine::ClaudeCode,
                false,
                Some("held back for this experiment"),
            )
            .expect("disable claude");
        providers.observe(&registry).expect("observe");
        let record = providers
            .read(ProviderFamily::Anthropic)
            .expect("read")
            .expect("observed");
        assert_eq!(record.state, ProviderState::Unavailable);
        assert!(
            record.reason.contains("held back for this experiment"),
            "{}",
            record.reason
        );
    }

    /// An engine record that cannot be read never states a provider ready. The observation records
    /// the read failure as the route's reason instead of answering with what the record might have
    /// said.
    #[test]
    fn an_unreadable_engine_record_never_states_its_provider_ready() {
        let (_directory, registry, providers) = root();
        measure(
            &registry,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
        );
        fs::write(registry.path_of(Engine::ClaudeCode), b"{ not json").expect("corrupt the record");
        providers.observe(&registry).expect("observe");
        let record = providers
            .read(ProviderFamily::Anthropic)
            .expect("read")
            .expect("observed");
        assert_eq!(record.state, ProviderState::Unavailable);
        let route = record.route(Engine::ClaudeCode).expect("the engine route");
        assert!(!route.enabled);
        assert!(route.credential_origin.is_none());
        assert!(
            route
                .withheld_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("not readable")),
            "{route:?}"
        );
    }

    /// Nothing at this level changes which engines are admitted. A provider that no credential
    /// reaches leaves the engine's admission exactly as its enabled flag states it.
    #[test]
    fn the_provider_level_changes_no_engine_admission() {
        let (_directory, registry, providers) = root();
        measure(&registry, Engine::ClaudeCode, None);
        providers.observe(&registry).expect("observe");
        let record = providers
            .read(ProviderFamily::Anthropic)
            .expect("read")
            .expect("observed");
        assert_ne!(record.state, ProviderState::Ready);
        registry
            .admit(Engine::ClaudeCode)
            .expect("an unconfigured provider held back an admitted engine");
    }

    /// Observing a provider and reading the catalog leave every engine record exactly as it stood.
    ///
    /// The admission decision this level reads is the operator's, and a level that rewrote the
    /// record while reading it would be taking that decision under the name of observing it.
    #[test]
    fn observing_a_provider_writes_into_no_engine_record() {
        let (_directory, registry, providers) = root();
        measure(
            &registry,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
        );
        let before: Vec<Vec<u8>> = Engine::ALL
            .iter()
            .map(|engine| fs::read(registry.path_of(*engine)).unwrap_or_default())
            .collect();

        providers.observe(&registry).expect("observe");
        crate::Catalog::read(&providers, &registry).expect("catalog");

        let after: Vec<Vec<u8>> = Engine::ALL
            .iter()
            .map(|engine| fs::read(registry.path_of(*engine)).unwrap_or_default())
            .collect();
        assert_eq!(before, after, "an engine record was rewritten by observing");
        registry.admit(Engine::ClaudeCode).expect("claude admitted");
    }

    /// A record standing under another provider's name is refused, and so is one of another
    /// schema version. Neither is migrated.
    #[test]
    fn a_record_of_another_provider_or_another_version_is_refused() {
        let (_directory, _registry, providers) = root();
        let mut record = ProviderRecord::observed(ProviderFamily::Anthropic, Vec::new());
        providers
            .write(ProviderFamily::OpenAi, &record)
            .expect("write the anthropic record in openai's place");
        let error = providers
            .read(ProviderFamily::OpenAi)
            .expect_err("a misplaced record was read as openai's");
        assert!(
            matches!(error, ProviderError::Unreadable { .. }),
            "{error:?}"
        );

        record.provider = ProviderFamily::OpenAi.name().to_owned();
        record.schema_version = PROVIDER_SCHEMA_VERSION + 1;
        providers
            .write(ProviderFamily::OpenAi, &record)
            .expect("write a later record");
        let error = providers
            .read(ProviderFamily::OpenAi)
            .expect_err("a later record was read");
        assert!(
            matches!(error, ProviderError::UnsupportedSchema { .. }),
            "{error:?}"
        );
    }

    /// Every engine stands beneath exactly one provider, and every provider reaches exactly the
    /// engines that name it. The two statements are separate code, so they are checked against
    /// each other.
    #[test]
    fn every_engine_stands_beneath_the_provider_that_names_it() {
        for engine in Engine::ALL {
            assert!(
                engine.provider().engines().contains(&engine),
                "{} stands beneath {} and is not reached from it",
                engine.name(),
                engine.provider().name()
            );
        }
        for family in ProviderFamily::ALL {
            for engine in family.engines() {
                assert_eq!(engine.provider(), family);
            }
        }
        assert_eq!(
            ProviderFamily::parse("anthropic").expect("anthropic"),
            ProviderFamily::Anthropic
        );
        let error = ProviderFamily::parse("nvidia").expect_err("no provider is named nvidia");
        assert!(error.to_string().contains("anthropic"), "{error}");
    }

    /// The driver is this product's own implementation and never becomes durable state. A record
    /// naming a crate would put an internal boundary into the layout.
    #[test]
    fn no_stored_record_names_a_driver() {
        let (_directory, registry, providers) = root();
        measure(
            &registry,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
        );
        providers.observe(&registry).expect("observe");
        for path in [
            providers.path_of(ProviderFamily::Anthropic),
            registry.path_of(Engine::ClaudeCode),
        ] {
            let stored = fs::read_to_string(&path).expect("stored record");
            assert!(
                !stored.contains("ymp-runtime-"),
                "{} names a driver: {stored}",
                path.display()
            );
        }
        assert_eq!(Engine::ClaudeCode.driver(), "ymp-runtime-claude");
    }
}

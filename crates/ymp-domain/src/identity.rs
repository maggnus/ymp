//! Persistent agents and explicitly observed native metadata. Unknown is not a default.

use crate::{Denial, Id, Result, journal::Capability, require_text};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileSettings {
    pub model: Option<String>,
    pub effort: Option<String>,
}
impl ProfileSettings {
    pub fn validate(&self) -> Result<()> {
        for value in [&self.model, &self.effort].into_iter().flatten() {
            require_text(value, 256)?;
        }
        Ok(())
    }
}

/// The three independent settings facts carried by Invocation. Lifecycle and
/// provider observation transitions are supplied by the execution-host task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationSettings {
    pub requested: ProfileSettings,
    pub sent: ProfileSettings,
    pub reported: ProfileSettings,
}
impl InvocationSettings {
    pub fn requested(requested: ProfileSettings) -> Result<Self> {
        requested.validate()?;
        Ok(Self {
            requested,
            sent: ProfileSettings::default(),
            reported: ProfileSettings::default(),
        })
    }
    pub fn validate(&self) -> Result<()> {
        self.requested.validate()?;
        self.sent.validate()?;
        self.reported.validate()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    pub id: Id<Agent>,
    pub name: String,
    pub provider: Id<Provider>,
    pub defaults: ProfileSettings,
    pub instructions: String,
    pub enabled: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    Codex,
    Claude,
    Glm,
    Scripted,
    Other(String),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub id: Id<Provider>,
    pub kind: ProviderKind,
    pub version: Option<String>,
    pub capabilities: Option<BTreeSet<Capability>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelOffering {
    pub provider: Id<Provider>,
    pub model: Option<String>,
    pub family: Option<String>,
    pub efforts: Option<BTreeSet<String>>,
    pub default_effort: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionProfile {
    pub agent: Id<Agent>,
    pub provider_version: Option<String>,
    pub model: String,
    pub family: Option<String>,
    pub effort: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoverySource {
    Native,
    ScriptedFixture,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Discovery {
    pub provider: Provider,
    pub offerings: Vec<ModelOffering>,
    pub default_model: Option<String>,
    pub adapter_available: bool,
    pub source: DiscoverySource,
    pub method: String,
    pub observed_at: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DependencyKind {
    Executable,
    File,
    Directory,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyState {
    Available,
    Missing,
    Unreadable,
    WrongKind,
    NotExecutable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyObservation {
    pub provider: Id<Provider>,
    pub model: Option<String>,
    pub path: PathBuf,
    pub kind: DependencyKind,
    pub state: DependencyState,
    pub observed_at: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryFacts {
    pub agents: Vec<Agent>,
    pub discoveries: Vec<Discovery>,
    pub dependencies: Vec<DependencyObservation>,
}
impl RegistryFacts {
    pub fn validate(&self, at: u64) -> Result<()> {
        let mut agents = BTreeSet::new();
        for agent in &self.agents {
            if !agents.insert(&agent.id) {
                return Err(Denial::new("registry", "Duplicate agent identity"));
            }
            require_text(&agent.name, 256)?;
            agent.defaults.validate()?;
            if agent.instructions.len() > 65_536 || agent.instructions.contains('\0') {
                return Err(Denial::new("registry", "Invalid agent instructions"));
            }
        }
        let mut providers = BTreeSet::new();
        for discovery in &self.discoveries {
            if !providers.insert(&discovery.provider.id) {
                return Err(Denial::new("registry", "Duplicate provider discovery"));
            }
            require_text(&discovery.method, 256)?;
            if discovery.observed_at > at {
                return Err(Denial::new(
                    "registry",
                    "Discovery cannot come from the future",
                ));
            }
            if let Some(version) = &discovery.provider.version {
                require_text(version, 256)?;
            }
            if let ProviderKind::Other(kind) = &discovery.provider.kind {
                require_text(kind, 256)?;
            }
            if (discovery.provider.kind == ProviderKind::Scripted)
                != (discovery.source == DiscoverySource::ScriptedFixture)
            {
                return Err(Denial::new(
                    "registry",
                    "Scripted fixtures and native provider observations cannot substitute for each other",
                ));
            }
            let mut models = BTreeSet::new();
            for offering in &discovery.offerings {
                if offering.provider != discovery.provider.id {
                    return Err(Denial::new("registry", "Offering names another provider"));
                }
                if let Some(model) = &offering.model {
                    require_text(model, 256)?;
                    if !models.insert(model) {
                        return Err(Denial::new("registry", "Duplicate native model identifier"));
                    }
                }
                if let Some(family) = &offering.family {
                    require_text(family, 256)?;
                }
                if let Some(efforts) = &offering.efforts {
                    for effort in efforts {
                        require_text(effort, 256)?;
                    }
                }
                if let Some(default) = &offering.default_effort {
                    require_text(default, 256)?;
                    if !offering
                        .efforts
                        .as_ref()
                        .is_some_and(|values| values.contains(default))
                    {
                        return Err(Denial::new(
                            "registry",
                            "Reported default effort is absent from the supported effort set",
                        ));
                    }
                }
            }
            if discovery
                .default_model
                .as_ref()
                .is_some_and(|default| !models.contains(default))
            {
                return Err(Denial::new(
                    "registry",
                    "Reported default model is absent from the offerings",
                ));
            }
        }
        let mut dependencies = BTreeSet::new();
        for dependency in &self.dependencies {
            if !providers.contains(&dependency.provider)
                || !dependency.path.is_absolute()
                || dependency.observed_at > at
            {
                return Err(Denial::new(
                    "registry",
                    "Dependency observation has an invalid provider, path or time",
                ));
            }
            if !dependencies.insert((
                &dependency.provider,
                &dependency.model,
                &dependency.path,
                &dependency.kind,
            )) {
                return Err(Denial::new("registry", "Duplicate dependency observation"));
            }
            if let Some(model) = &dependency.model {
                require_text(model, 256)?;
                if !self
                    .discoveries
                    .iter()
                    .find(|d| d.provider.id == dependency.provider)
                    .is_some_and(|d| d.offerings.iter().any(|o| o.model.as_ref() == Some(model)))
                {
                    return Err(Denial::new(
                        "registry",
                        "Dependency observation names an unknown model",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExclusionCode {
    Disabled,
    MissingProvider,
    MissingAdapter,
    UnknownCapabilities,
    MissingModel,
    ModelPin,
    EffortPin,
    RosterPin,
    MissingDependency,
    UnreadableDependency,
    InvalidDependency,
    Policy,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exclusion {
    pub code: ExclusionCode,
    pub message: String,
}
impl Exclusion {
    pub fn new(code: ExclusionCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Readiness {
    Ready,
    NotReady(Exclusion),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pool {
    pub eligible: BTreeSet<Id<Agent>>,
    pub offerings: BTreeMap<Id<Agent>, Vec<ModelOffering>>,
    pub excluded: BTreeMap<Id<Agent>, Exclusion>,
}
/// A read-only projection of backend/workspace enforcement, supplied by the workspace owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceCapabilities {
    pub workspace: Id,
    pub profile: ExecutionProfile,
    pub enforced: Option<BTreeSet<Capability>>,
}

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};

mod capabilities;
mod catalog;
mod execution;
mod pool;
pub use capabilities::*;
pub use catalog::*;
pub use execution::*;
pub use pool::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Codex,
    Claude,
    Acp,
    Mock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub kind: ProviderKind,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Names of inherited variables; values are never stored by discovery.
    #[serde(default)]
    pub env_refs: BTreeMap<String, String>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model: Option<String>,
    #[serde(default)]
    pub instructions: String,
    #[serde(default = "yes")]
    pub enabled: bool,
}

impl AgentProfile {
    pub fn version(&self, provider: &ProviderConfig) -> String {
        let mut hash = Sha256::new();
        hash.update(
            serde_json::to_vec(&(&self.id, provider, &self.model, &self.instructions))
                .expect("serializable profile"),
        );
        format!("{:x}", hash.finalize())[..24].to_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    pub parallel: usize,
    pub turns: usize,
    pub turn_timeout_secs: u64,
    pub attempts: usize,
    /// Absent on historical captures. New sessions resolve the resource defaults once.
    #[serde(default)]
    pub resources: Option<crate::ResourceLimits>,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            parallel: 3,
            turns: 200,
            turn_timeout_secs: 900,
            attempts: 3,
            resources: Some(crate::ResourceLimits::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Loaded only from the application-owned scan cache; never trusted from TOML.
    #[serde(skip)]
    pub native_catalog: NativeCatalogSnapshot,
    pub version: u32,
    /// Trusted checks for every new team session using this configuration.
    /// On resume, None keeps captured authority; Some must match it exactly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_contracts: Option<Vec<crate::AcceptanceContract>>,
    #[serde(default)]
    pub limits: Limits,
    pub providers: Vec<ProviderConfig>,
    /// Optional native offerings, keyed by provider ID. Absence means unknown.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capabilities: BTreeMap<String, ProviderCapabilities>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub execution: BTreeMap<String, AgentExecutionPolicy>,
    pub agents: Vec<AgentProfile>,
    /// Configured starting roster; neither the eligible pool nor a live session.
    pub team: Vec<String>,
    #[serde(default)]
    pub team_constraints: crate::TeamConstraints,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            native_catalog: NativeCatalogSnapshot::default(),
            version: 1,
            acceptance_contracts: None,
            team_constraints: crate::TeamConstraints::default(),
            limits: Limits::default(),
            capabilities: BTreeMap::new(),
            execution: BTreeMap::new(),
            providers: vec![
                ProviderConfig {
                    id: "codex".into(),
                    kind: ProviderKind::Codex,
                    command: "codex".into(),
                    args: vec![],
                    env_refs: BTreeMap::new(),
                    enabled: true,
                },
                ProviderConfig {
                    id: "claude".into(),
                    kind: ProviderKind::Claude,
                    command: "claude".into(),
                    args: vec![],
                    env_refs: BTreeMap::new(),
                    enabled: true,
                },
                ProviderConfig {
                    id: "glm".into(),
                    kind: ProviderKind::Acp,
                    command: "npx".into(),
                    args: vec!["--no-install".into(), "glm-acp-agent@1.3.0".into()],
                    env_refs: BTreeMap::new(),
                    enabled: false,
                },
            ],
            agents: vec![
                AgentProfile {
                    id: "codex".into(),
                    name: "Codex".into(),
                    provider: "codex".into(),
                    model: None,
                    instructions: "Be precise. Verify claims against the actual workspace.".into(),
                    enabled: true,
                },
                AgentProfile {
                    id: "claude".into(),
                    name: "Claude".into(),
                    provider: "claude".into(),
                    model: None,
                    instructions: "Consider alternatives and identify missing requirements.".into(),
                    enabled: true,
                },
                AgentProfile {
                    id: "glm".into(),
                    name: "GLM".into(),
                    provider: "glm".into(),
                    model: None,
                    instructions: "Look for edge cases and independently verify results.".into(),
                    enabled: false,
                },
            ],
            team: vec!["codex".into(), "claude".into()],
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        crate::validate_contract_targets(self.acceptance_contracts.as_deref().unwrap_or_default())?;
        self.team_constraints.validate()?;
        if self.version != 1 {
            bail!("Unsupported configuration version {}", self.version);
        }
        if self.limits.parallel == 0
            || self.limits.turns == 0
            || self.limits.attempts == 0
            || self.limits.turn_timeout_secs == 0
        {
            bail!("Limits must be positive");
        }
        if let Some(resources) = &self.limits.resources {
            resources.validate()?;
        }
        let mut ids = std::collections::HashSet::new();
        for provider in &self.providers {
            if provider.id.is_empty() || !ids.insert(&provider.id) {
                bail!("Duplicate or empty provider id");
            }
            if provider.command.is_empty() {
                bail!("Empty provider command");
            }
        }
        ids.clear();
        for (id, capabilities) in &self.capabilities {
            self.provider(id)?;
            capabilities
                .validate()
                .with_context(|| format!("Invalid capabilities for provider {id}"))?;
        }
        for agent in &self.agents {
            if agent.id.is_empty() || !ids.insert(&agent.id) {
                bail!("Duplicate or empty agent id");
            }
            self.provider(&agent.provider)?;
        }
        for (id, policy) in &self.execution {
            let agent = self.agent(id)?;
            policy.resolve(agent, &ModelEffort::default())?;
        }
        let mut team = std::collections::HashSet::new();
        for id in &self.team {
            if !team.insert(id) {
                bail!("Duplicate team member: {id}");
            }
            self.agent(id)?;
        }
        Ok(())
    }
    pub fn provider(&self, id: &str) -> Result<&ProviderConfig> {
        self.providers
            .iter()
            .find(|p| p.id == id)
            .with_context(|| format!("Unknown provider: {id}"))
    }
    pub fn agent(&self, id: &str) -> Result<&AgentProfile> {
        self.agents
            .iter()
            .find(|a| a.id == id)
            .with_context(|| format!("Unknown agent: {id}"))
    }
    pub fn members(&self) -> Vec<AgentProfile> {
        self.team
            .iter()
            .filter_map(|id| self.agent(id).ok())
            .filter(|a| a.enabled && self.provider(&a.provider).is_ok_and(|p| p.enabled))
            .cloned()
            .collect()
    }
    pub fn load(home: &std::path::Path) -> Result<Self> {
        let mut value: Self = toml::from_str(&std::fs::read_to_string(home.join("config.toml"))?)?;
        value.validate()?;
        value.native_catalog = NativeCatalogSnapshot::load(home)?;
        Ok(value)
    }
    pub fn save(&self, home: &std::path::Path) -> Result<()> {
        let _lock = configuration_lock(home)?;
        self.save_configuration(home)
    }
    /// Publish a scan without overwriting configuration edited during discovery.
    pub fn save_scanned_catalog(
        &self,
        home: &std::path::Path,
        expected: Option<&[u8]>,
    ) -> Result<()> {
        let _lock = configuration_lock(home)?;
        let actual = match std::fs::read(home.join("config.toml")) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if actual.as_deref() != expected {
            bail!("Configuration changed during native scan; refresh again to preserve the edit");
        }
        self.validate()?;
        self.native_catalog.save(home)?;
        self.save_configuration(home)
    }
    fn save_configuration(&self, home: &std::path::Path) -> Result<()> {
        self.validate()?;
        std::fs::create_dir_all(home)?;
        let tmp = home.join(format!("config.{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        std::fs::rename(tmp, home.join("config.toml"))?;
        Ok(())
    }
}

/// Release this save's ownership even if a fork-inherited descriptor remains
/// open until its child reaches exec. Closing alone waits for that descriptor.
struct ConfigurationLock {
    file: std::fs::File,
}

impl Drop for ConfigurationLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

fn configuration_lock(home: &std::path::Path) -> Result<ConfigurationLock> {
    std::fs::create_dir_all(home)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(home.join("configuration.lock"))?;
    fs2::FileExt::try_lock_exclusive(&file)
        .context("Another client is saving configuration; retry the edit")?;
    Ok(ConfigurationLock { file })
}

pub fn default_home() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("YMP_HOME") {
        return Ok(path.into());
    }
    Ok(PathBuf::from(std::env::var_os("HOME").context("HOME is unavailable")?).join(".ymp2"))
}

#[cfg(all(test, unix))]
mod lock_tests {
    use super::*;

    #[test]
    fn completed_configuration_owner_releases_before_inherited_descriptor_closes() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        let config = Config::default();
        let owner = configuration_lock(home).unwrap();
        // A cloned descriptor retains the owner's open file description just
        // as an inherited descriptor can, without a timing-dependent fork.
        let inherited = owner.file.try_clone().unwrap();
        assert!(
            config.save(home).is_err(),
            "live configuration owner must exclude another save"
        );
        drop(owner);
        let next = configuration_lock(home).expect(
            "completed configuration owner must release before an inherited descriptor closes",
        );
        assert!(
            config.save(home).is_err(),
            "replacement owner must remain exclusive"
        );
        drop(inherited);
        assert!(
            config.save(home).is_err(),
            "closing the old descriptor must not unlock the replacement owner"
        );
        drop(next);
        config.save(home).unwrap();
        assert_eq!(
            serde_json::to_value(Config::load(home).unwrap()).unwrap(),
            serde_json::to_value(config).unwrap()
        );
    }
}

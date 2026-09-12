use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};

mod capabilities;
mod pool;
pub use capabilities::*;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    pub parallel: usize,
    pub turns: usize,
    pub turn_timeout_secs: u64,
    pub attempts: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            parallel: 3,
            turns: 200,
            turn_timeout_secs: 900,
            attempts: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub limits: Limits,
    pub providers: Vec<ProviderConfig>,
    /// Optional native offerings, keyed by provider ID. Absence means unknown.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capabilities: BTreeMap<String, ProviderCapabilities>,
    pub agents: Vec<AgentProfile>,
    /// Configured starting roster; neither the eligible pool nor a live session.
    pub team: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            limits: Limits::default(),
            capabilities: BTreeMap::new(),
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
        let value: Self = toml::from_str(&std::fs::read_to_string(home.join("config.toml"))?)?;
        value.validate()?;
        Ok(value)
    }
    pub fn save(&self, home: &std::path::Path) -> Result<()> {
        self.validate()?;
        std::fs::create_dir_all(home)?;
        let tmp = home.join(format!("config.{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        std::fs::rename(tmp, home.join("config.toml"))?;
        Ok(())
    }
}

pub fn default_home() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("YMP_HOME") {
        return Ok(path.into());
    }
    Ok(PathBuf::from(std::env::var_os("HOME").context("HOME is unavailable")?).join(".ymp2"))
}

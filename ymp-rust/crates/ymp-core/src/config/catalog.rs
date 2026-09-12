//! Stored native observations are separate from editable capability claims.
use super::*;
use crate::ExecutionSettings;
use std::path::Path;

pub const NATIVE_CATALOG_FILE: &str = "provider-catalog.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NativeCatalogSnapshot {
    #[serde(default)]
    pub providers: BTreeMap<String, NativeProviderSnapshot>,
    /// Stable generated actors, never the active or starting roster.
    #[serde(default)]
    pub generated_agents: BTreeMap<String, NativeAgentBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeAgentBinding {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeProviderSnapshot {
    /// Binds the observation to the command, arguments and environment reference
    /// names that were scanned. No environment values are captured.
    pub provider_fingerprint: String,
    pub last_attempt: String,
    /// A bounded status code, never untrusted stderr or an authentication error.
    pub failure: Option<String>,
    pub catalog: Option<ProviderCapabilities>,
}

impl NativeCatalogSnapshot {
    pub fn load(home: &Path) -> Result<Self> {
        let bytes = match std::fs::read(home.join(NATIVE_CATALOG_FILE)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => return Err(error.into()),
        };
        let snapshot: Self = serde_json::from_slice(&bytes)?;
        for entry in snapshot.providers.values() {
            chrono::DateTime::parse_from_rfc3339(&entry.last_attempt)?;
            if let Some(catalog) = &entry.catalog {
                catalog.validate()?;
                if !matches!(catalog.source, CapabilitySource::NativeMetadata { .. }) {
                    bail!("Native scan cache contains an unobserved capability claim");
                }
            }
        }
        Ok(snapshot)
    }

    pub fn save(&self, home: &Path) -> Result<()> {
        std::fs::create_dir_all(home)?;
        let temporary = home.join(format!("provider-catalog.{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temporary, home.join(NATIVE_CATALOG_FILE))?;
        Ok(())
    }
}

pub fn provider_fingerprint(provider: &ProviderConfig) -> String {
    // Enablement is local policy, not a change to the queried native system.
    let bytes = serde_json::to_vec(&(
        &provider.id,
        &provider.kind,
        &provider.command,
        &provider.args,
        &provider.env_refs,
    ))
    .expect("serializable provider identity");
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentIdentityStatus {
    Native,
    /// A retained native observation after a failed refresh or after one day.
    Stale,
    Unknown,
    Unresolved,
    /// Deterministic local fixtures have no native provider identity.
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub name: String,
    pub configured_name: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub resolved_model: Option<String>,
    pub source: Option<CapabilitySource>,
    pub status: AgentIdentityStatus,
}

impl Config {
    pub fn native_provider_snapshot(&self, provider: &str) -> Option<&NativeProviderSnapshot> {
        let entry = self.native_catalog.providers.get(provider)?;
        (entry.provider_fingerprint == provider_fingerprint(self.provider(provider).ok()?))
            .then_some(entry)
    }

    pub fn provider_capabilities(&self, provider: &str) -> Option<&ProviderCapabilities> {
        self.native_provider_snapshot(provider)
            .and_then(|entry| entry.catalog.as_ref())
            .or_else(|| self.capabilities.get(provider))
    }

    /// Presentation for the effective assignment settings, with no process launch.
    /// Names from manual TOML catalogs never acquire native provenance.
    pub fn agent_identity(
        &self,
        profile: &AgentProfile,
        settings: &ExecutionSettings,
    ) -> AgentIdentity {
        let mut identity = AgentIdentity {
            name: settings
                .model
                .clone()
                .unwrap_or_else(|| "Unresolved native model".into()),
            configured_name: self
                .agent(&profile.id)
                .ok()
                .filter(|configured| configured.provider == profile.provider)
                .map_or_else(
                    || profile.name.clone(),
                    |configured| configured.name.clone(),
                ),
            model: settings.model.clone(),
            effort: settings.effort.clone(),
            resolved_model: None,
            source: None,
            status: if settings.model.is_some() {
                AgentIdentityStatus::Unknown
            } else {
                AgentIdentityStatus::Unresolved
            },
        };
        if self
            .provider(&profile.provider)
            .is_ok_and(|p| p.kind == ProviderKind::Mock)
        {
            identity.name = profile.name.clone();
            identity.status = AgentIdentityStatus::Local;
            return identity;
        }
        let Some(model) = settings.model.as_deref() else {
            return identity;
        };
        let Some(entry) = self.native_provider_snapshot(&profile.provider) else {
            return identity;
        };
        let Some(catalog) = &entry.catalog else {
            return identity;
        };
        let Some(offering) = catalog.model(model) else {
            return identity;
        };
        identity.name = offering
            .display_name
            .clone()
            .unwrap_or_else(|| offering.id.clone());
        identity.resolved_model = offering.resolved_model.clone();
        identity.source = Some(catalog.source.clone());
        let old = match &catalog.source {
            CapabilitySource::NativeMetadata { observed_at, .. } => {
                chrono::DateTime::parse_from_rfc3339(observed_at).map_or(true, |observed| {
                    chrono::Utc::now()
                        .signed_duration_since(observed)
                        .num_hours()
                        >= 24
                })
            }
            CapabilitySource::Configured => true,
        };
        identity.status = if entry.failure.is_some() || old {
            AgentIdentityStatus::Stale
        } else {
            AgentIdentityStatus::Native
        };
        identity
    }

    pub fn is_legacy_provider_placeholder(&self, profile: &AgentProfile) -> bool {
        profile.model.is_none()
            && Config::default().agents.iter().any(|generated| {
                generated.id == profile.id
                    && generated.provider == profile.provider
                    && generated.name == profile.name
            })
            && self
                .provider(&profile.provider)
                .is_ok_and(|p| p.kind != ProviderKind::Mock)
    }
}

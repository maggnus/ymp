mod catalog;
use anyhow::Result;
pub use catalog::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ymp_core::{
    AgentPool, CapabilitySource, Config, PoolAgent, PoolExclusion, PoolModelStatus, ProviderKind,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub id: String,
    pub available: bool,
    pub executable: Option<PathBuf>,
    pub detail: String,
}
pub fn executable(command: &str) -> Option<PathBuf> {
    let p = Path::new(command);
    if p.is_absolute() {
        return is_executable(p).then(|| p.into());
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(command))
        .find(|p| is_executable(p))
}
fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}
pub fn inspect(config: &Config) -> Vec<ProviderHealth> {
    config
        .providers
        .iter()
        .map(|p| {
            let path = executable(&p.command);
            ProviderHealth {
                id: p.id.clone(),
                available: path.is_some() || p.kind == ProviderKind::Mock,
                executable: path,
                detail: if p.enabled {
                    "Configured; authentication is checked by the agent on use"
                } else {
                    "Disabled"
                }
                .into(),
            }
        })
        .collect()
}

/// Inspect local eligibility without launching a process or checking authentication.
/// Session constraints are applied by admission; the configured starting roster
/// has no effect on this pool. Catalog claims remain explicitly configured.
pub fn inspect_pool(config: &Config) -> Result<AgentPool> {
    config.validate()?;
    let health = inspect(config);
    let mut capabilities = config.capabilities.clone();
    for catalog in capabilities.values_mut() {
        catalog.source = CapabilitySource::Configured;
    }
    for provider in &config.providers {
        if let Some(catalog) = config
            .native_provider_snapshot(&provider.id)
            .and_then(|entry| entry.catalog.as_ref())
        {
            capabilities.insert(provider.id.clone(), catalog.clone());
        }
    }
    let agents = config
        .agents
        .iter()
        .map(|profile| {
            let provider = config.provider(&profile.provider)?;
            let mut exclusions = Vec::new();
            if !profile.enabled {
                exclusions.push(PoolExclusion::AgentDisabled);
            }
            if !provider.enabled {
                exclusions.push(PoolExclusion::ProviderDisabled);
            }
            if !health.iter().any(|h| h.id == provider.id && h.available) {
                exclusions.push(PoolExclusion::ExecutableMissing);
            }
            let settings = config.execution_settings(profile, &Default::default())?;
            let identity = config.agent_identity(profile, &settings);
            if config.is_legacy_provider_placeholder(profile) && settings.model.is_none() {
                exclusions.push(PoolExclusion::NativeModelUnresolved);
            }
            let model_status = match (
                settings.model.as_deref(),
                capabilities.get(&profile.provider),
            ) {
                (None, catalog) => {
                    if catalog.is_some_and(|c| c.models_complete && c.models.is_empty()) {
                        exclusions.push(PoolExclusion::NoModelsAvailable);
                    }
                    PoolModelStatus::InheritedDefault
                }
                (Some(model), Some(catalog)) if catalog.model(model).is_some() => {
                    PoolModelStatus::Listed
                }
                (Some(_), Some(catalog)) if catalog.models_complete => {
                    exclusions.push(PoolExclusion::ModelUnlisted);
                    PoolModelStatus::Unlisted
                }
                _ => PoolModelStatus::Unknown,
            };
            let mut presented = profile.clone();
            presented.name = identity.name.clone();
            Ok(PoolAgent {
                identity,
                profile: presented,
                profile_version: profile.version(provider),
                exclusions,
                model_status,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(AgentPool {
        agents,
        capabilities,
    })
}

/// Discover a cached local GLM installation, without invoking npx's package installer.
pub fn discover_glm(config: &mut Config) -> Result<bool> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Ok(false);
    };
    let root = home.join(".npm/_npx");
    discover_glm_in(config, &root)
}

fn discover_glm_in(config: &mut Config, root: &Path) -> Result<bool> {
    if !config
        .providers
        .iter()
        .any(|p| p.id == "glm" && p.kind == ProviderKind::Acp)
    {
        return Ok(false);
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return Ok(false);
    };
    for entry in entries.flatten() {
        let dir = entry.path().join("node_modules/glm-acp-agent");
        let Ok(bytes) = std::fs::read(dir.join("package.json")) else {
            continue;
        };
        let Ok(package) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        let bin = package["bin"]
            .as_str()
            .or_else(|| package["bin"]["glm-acp-agent"].as_str());
        if let Some(bin) = bin {
            let path = dir.join(bin);
            if !path.is_file() {
                continue;
            }
            if let Some(provider) = config.providers.iter_mut().find(|p| p.id == "glm") {
                provider.kind = ProviderKind::Acp;
                provider.command = "node".into();
                provider.args = vec![path.to_string_lossy().into()];
                provider.enabled = true;
            }
            if let Some(agent) = config
                .agents
                .iter_mut()
                .find(|a| a.id == "glm" && a.provider == "glm")
            {
                agent.enabled = true;
            }
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ymp_core::{ModelCapabilities, ProviderCapabilities};

    #[test]
    #[cfg(unix)]
    fn pool_inspection_never_runs_the_provider_and_does_not_select_a_team() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("provider");
        let marker = temp.path().join("provider.started");
        std::fs::write(&script, "#!/bin/sh\n: > \"$0.started\"\nexit 99\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        // Prove the sentinel detects execution before relying on its absence.
        assert_eq!(
            std::process::Command::new(&script).status().unwrap().code(),
            Some(99)
        );
        assert!(marker.exists());
        std::fs::remove_file(&marker).unwrap();

        let mut config = Config::default();
        config.providers.truncate(1);
        config.providers[0].command = script.to_string_lossy().into();
        config.agents.truncate(1);
        let mut another = config.agents[0].clone();
        another.id = "second-context".into();
        another.name = "Existing second name".into();
        config.agents.push(another);
        config.team.clear();
        let before = serde_json::to_value(&config).unwrap();
        config.save(temp.path()).unwrap();
        let loaded = Config::load(temp.path()).unwrap();
        let pool = inspect_pool(&loaded).unwrap();
        assert_eq!(pool.eligible().count(), 1);
        assert!(pool.agents[0]
            .exclusions
            .contains(&PoolExclusion::NativeModelUnresolved));
        assert_eq!(
            pool.agents[0].profile.provider,
            pool.agents[1].profile.provider
        );
        assert_ne!(pool.agents[0].profile.id, pool.agents[1].profile.id);
        assert_eq!(
            pool.agents[1].identity.configured_name,
            "Existing second name"
        );
        assert_eq!(
            pool.agents[1].identity.status,
            ymp_core::AgentIdentityStatus::Unresolved
        );
        assert_eq!(
            pool.agents[0].model_status,
            PoolModelStatus::InheritedDefault
        );
        assert!(pool.capabilities.is_empty());
        assert!(loaded.members().is_empty());
        assert_eq!(serde_json::to_value(&loaded).unwrap(), before);
        assert!(
            !marker.exists(),
            "discovery or config inspection executed the provider"
        );
    }

    #[test]
    fn eligibility_distinguishes_disabled_missing_unknown_and_explicitly_unlisted() {
        let mut config = Config::default();
        config.providers[0].kind = ProviderKind::Mock;
        config.providers[0].command = "internal".into();
        config.providers[1].command = "/ymp-test/nonexistent-provider".into();
        config.agents[0].model = Some("fixture-model".into());
        let pool = inspect_pool(&config).unwrap();
        assert_eq!(pool.agents[0].model_status, PoolModelStatus::Unknown);
        assert!(pool.agents[0].exclusions.is_empty());
        assert!(pool.agents[1]
            .exclusions
            .contains(&PoolExclusion::ExecutableMissing));
        assert!(pool.agents[2]
            .exclusions
            .contains(&PoolExclusion::AgentDisabled));
        assert!(pool.agents[2]
            .exclusions
            .contains(&PoolExclusion::ProviderDisabled));

        let mut catalog = ProviderCapabilities::default();
        catalog.models.push(ModelCapabilities {
            picker_id: None,
            display_name: None,
            aliases: vec![],
            resolved_model: None,
            id: "other-fixture-model".into(),
            controls: None,
        });
        config.capabilities.insert("codex".into(), catalog);
        assert_eq!(
            inspect_pool(&config).unwrap().agents[0].model_status,
            PoolModelStatus::Unknown
        );
        config
            .capabilities
            .get_mut("codex")
            .unwrap()
            .models_complete = true;
        let pool = inspect_pool(&config).unwrap();
        assert_eq!(pool.agents[0].model_status, PoolModelStatus::Unlisted);
        assert!(pool.agents[0]
            .exclusions
            .contains(&PoolExclusion::ModelUnlisted));
        config.agents[0].model = Some("other-fixture-model".into());
        assert_eq!(
            inspect_pool(&config).unwrap().agents[0].model_status,
            PoolModelStatus::Listed
        );
        config.agents[0].model = None;
        config.capabilities.get_mut("codex").unwrap().models.clear();
        let pool = inspect_pool(&config).unwrap();
        assert!(pool.agents[0]
            .exclusions
            .contains(&PoolExclusion::NoModelsAvailable));
        config
            .capabilities
            .get_mut("codex")
            .unwrap()
            .models_complete = false;
        assert!(inspect_pool(&config).unwrap().agents[0]
            .exclusions
            .is_empty());
        config.providers[0].enabled = false;
        assert_eq!(inspect_pool(&config).unwrap().eligible().count(), 0);
    }

    #[test]
    fn configured_metadata_cannot_claim_to_be_an_observed_native_response() {
        let mut config = Config::default();
        config.capabilities.insert(
            "codex".into(),
            ProviderCapabilities {
                source: CapabilitySource::NativeMetadata {
                    method: "fixture/model-list".into(),
                    observed_at: "2026-09-12T00:00:00Z".into(),
                },
                ..Default::default()
            },
        );
        assert_eq!(
            inspect_pool(&config).unwrap().capabilities["codex"].source,
            CapabilitySource::Configured
        );
    }

    #[test]
    fn cached_installation_discovery_preserves_roster_ids_and_names() {
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("cached/node_modules/glm-acp-agent");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"bin":{"glm-acp-agent":"index.js"}}"#,
        )
        .unwrap();
        std::fs::write(
            package.join("index.js"),
            "throw new Error('must not execute');",
        )
        .unwrap();
        let mut config = Config::default();
        config.agents[2].name = "My GLM identity".into();
        let team = config.team.clone();
        let identities = config
            .agents
            .iter()
            .map(|a| (&a.id, &a.name))
            .map(|(id, name)| (id.clone(), name.clone()))
            .collect::<Vec<_>>();
        assert!(discover_glm_in(&mut config, temp.path()).unwrap());
        assert!(config.provider("glm").unwrap().enabled);
        assert!(config.agent("glm").unwrap().enabled);
        assert_eq!(config.team, team);
        assert_eq!(
            config
                .agents
                .iter()
                .map(|a| (a.id.clone(), a.name.clone()))
                .collect::<Vec<_>>(),
            identities
        );
        config.providers.retain(|p| p.id != "glm");
        assert!(!discover_glm_in(&mut config, temp.path()).unwrap());
    }

    #[test]
    fn fixed_execution_model_takes_precedence_over_a_stale_profile_default_in_pool_eligibility() {
        let mut config = Config::default();
        config.providers[0].kind = ProviderKind::Mock;
        config.agents[0].model = Some("stale-default".into());
        config.capabilities.insert(
            "codex".into(),
            ProviderCapabilities {
                models_complete: true,
                models: vec![ModelCapabilities {
                    picker_id: None,
                    display_name: None,
                    aliases: vec![],
                    resolved_model: None,
                    id: "fixed-model".into(),
                    controls: None,
                }],
                ..Default::default()
            },
        );
        config.execution.insert(
            "codex".into(),
            ymp_core::AgentExecutionPolicy {
                fixed: ymp_core::ModelEffort {
                    model: Some("fixed-model".into()),
                    effort: None,
                },
                ..Default::default()
            },
        );
        let pool = inspect_pool(&config).unwrap();
        assert_eq!(pool.agents[0].model_status, PoolModelStatus::Listed);
        assert!(pool.agents[0].exclusions.is_empty());
        assert_eq!(
            pool.agents[0].profile.model.as_deref(),
            Some("stale-default")
        );
    }
}

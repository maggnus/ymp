use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ymp_core::{Config, ProviderKind};

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
                available: path.is_some(),
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

/// Discover a cached local GLM installation, without invoking npx's package installer.
pub fn discover_glm(config: &mut Config) -> Result<bool> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Ok(false);
    };
    let root = home.join(".npm/_npx");
    let Ok(entries) = std::fs::read_dir(root) else {
        return Ok(false);
    };
    for entry in entries.flatten() {
        let dir = entry.path().join("node_modules/glm-acp-agent");
        let Ok(bytes) = std::fs::read(dir.join("package.json")) else {
            continue;
        };
        let package: serde_json::Value = serde_json::from_slice(&bytes)?;
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
            if let Some(agent) = config.agents.iter_mut().find(|a| a.id == "glm") {
                agent.enabled = true;
            }
            if !config.team.iter().any(|id| id == "glm") {
                config.team.push("glm".into());
            }
            return Ok(true);
        }
    }
    Ok(false)
}

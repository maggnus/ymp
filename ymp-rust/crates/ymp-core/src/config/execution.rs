//! Assignment choices and user constraints. Native adapters validate capabilities.
use crate::{AgentProfile, Config, ExecutionSettings, ProviderConfig};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEffort {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentExecutionPolicy {
    #[serde(default)]
    pub defaults: ModelEffort,
    /// Each present value is a singleton allowed set, never a fallback.
    #[serde(default)]
    pub fixed: ModelEffort,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignmentSettingsRule {
    pub agent_id: String,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    pub settings: ModelEffort,
}

impl ModelEffort {
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [("model", &self.model), ("effort", &self.effort)] {
            if value
                .as_ref()
                .is_some_and(|v| v.trim().is_empty() || v.trim() != v)
            {
                bail!("{name} must be a nonempty exact native identifier");
            }
        }
        Ok(())
    }
}

impl AgentExecutionPolicy {
    pub fn resolve(&self, agent: &AgentProfile, choice: &ModelEffort) -> Result<ExecutionSettings> {
        self.defaults.validate()?;
        self.fixed.validate()?;
        choice.validate()?;
        let resolve = |name: &str,
                       pin: &Option<String>,
                       value: &Option<String>,
                       default: Option<String>|
         -> Result<Option<String>> {
            if let (Some(pin), Some(value)) = (pin, value) {
                if pin != value {
                    bail!("Assignment {name} {value} violates fixed {name} {pin}");
                }
            }
            Ok(pin.clone().or_else(|| value.clone()).or(default))
        };
        Ok(ExecutionSettings {
            model: resolve(
                "model",
                &self.fixed.model,
                &choice.model,
                self.defaults.model.clone().or_else(|| agent.model.clone()),
            )?,
            effort: resolve(
                "effort",
                &self.fixed.effort,
                &choice.effort,
                self.defaults.effort.clone(),
            )?,
            permission_mode: None,
        })
    }
}

impl Config {
    pub fn execution_settings(
        &self,
        agent: &AgentProfile,
        choice: &ModelEffort,
    ) -> Result<ExecutionSettings> {
        self.execution
            .get(&agent.id)
            .cloned()
            .unwrap_or_default()
            .resolve(agent, choice)
    }
}

/// Separate execution observations from legacy profile-only competence. None
/// remains unknown; a native default is not promoted to an observed setting.
pub fn execution_config_version(
    agent: &AgentProfile,
    provider: &ProviderConfig,
    sent: &ExecutionSettings,
    reported: &ExecutionSettings,
    native_version: Option<&str>,
) -> String {
    let mut hash = Sha256::new();
    hash.update(
        serde_json::to_vec(&(
            "execution-v1",
            agent.version(provider),
            sent,
            reported,
            native_version,
        ))
        .expect("serializable execution settings"),
    );
    format!("{:x}", hash.finalize())[..24].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pins_are_singletons_and_defaults_only_fill_missing_choices() {
        let agent = Config::default().agents.remove(0);
        let mut p = AgentExecutionPolicy {
            defaults: ModelEffort {
                model: Some("default".into()),
                effort: Some("high".into()),
            },
            ..Default::default()
        };
        let choice = ModelEffort {
            model: Some("chosen".into()),
            effort: Some("low".into()),
        };
        assert_eq!(
            p.resolve(&agent, &choice).unwrap().model.as_deref(),
            Some("chosen")
        );
        p.fixed.effort = Some("max".into());
        assert!(p.resolve(&agent, &choice).is_err());
        assert_eq!(
            p.resolve(&agent, &ModelEffort::default())
                .unwrap()
                .effort
                .as_deref(),
            Some("max")
        );
        assert!(ModelEffort {
            effort: Some(" high".into()),
            ..Default::default()
        }
        .validate()
        .is_err());
    }
    #[test]
    fn versions_separate_effective_configuration_and_unknown_reports() {
        let c = Config::default();
        let a = &c.agents[0];
        let p = &c.providers[0];
        let mut sent = ExecutionSettings::default();
        let reported = ExecutionSettings::default();
        let v = execution_config_version(a, p, &sent, &reported, None);
        assert_ne!(v, a.version(p));
        sent.effort = Some("high".into());
        assert_ne!(v, execution_config_version(a, p, &sent, &reported, None));
        assert_ne!(
            execution_config_version(a, p, &sent, &reported, None),
            execution_config_version(a, p, &sent, &sent, None)
        );
    }
}

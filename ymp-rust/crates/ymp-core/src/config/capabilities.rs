//! Native offerings are metadata, not proof that a requested setting was sent.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilitySource {
    #[default]
    Configured,
    /// Reserved for an adapter that actually captured a native metadata response.
    NativeMetadata { method: String, observed_at: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    #[serde(default)]
    pub source: CapabilitySource,
    /// An incomplete list cannot establish that an unlisted model is unavailable.
    #[serde(default)]
    pub models_complete: bool,
    #[serde(default)]
    pub models: Vec<ModelCapabilities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    /// Exact native identifier, including a native alias when that is advertised.
    pub id: String,
    /// Verbatim native picker label; absence falls back to the native identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Native picker row identifier, which is not necessarily a wire model alias.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picker_id: Option<String>,
    /// Actual aliases explicitly advertised for this offering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_model: Option<String>,
    /// None is unknown; an empty list explicitly advertises no controls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controls: Option<Vec<NativeControl>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeControl {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub value_names: std::collections::BTreeMap<String, String>,
    /// Exact native field/configuration-option name. No universal effort ladder.
    pub id: String,
    pub values: NativeControlValues,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<NativeControlValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeControlValues {
    Choices { options: Vec<String> },
    Boolean,
    Integer { min: i64, max: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NativeControlValue {
    Choice(String),
    Boolean(bool),
    Integer(i64),
}

impl NativeControlValues {
    pub fn contains(&self, value: &NativeControlValue) -> bool {
        match (self, value) {
            (Self::Choices { options }, NativeControlValue::Choice(value)) => {
                options.contains(value)
            }
            (Self::Boolean, NativeControlValue::Boolean(_)) => true,
            (Self::Integer { min, max }, NativeControlValue::Integer(value)) => {
                (min..=max).contains(&value)
            }
            _ => false,
        }
    }
}

impl ProviderCapabilities {
    pub fn model(&self, id: &str) -> Option<&ModelCapabilities> {
        self.models.iter().find(|model| model.id == id).or_else(|| {
            self.models.iter().find(|model| {
                model.resolved_model.as_deref() == Some(id)
                    || model.aliases.iter().any(|alias| alias == id)
            })
        })
    }

    pub fn validate(&self) -> Result<()> {
        if let CapabilitySource::NativeMetadata {
            method,
            observed_at,
        } = &self.source
        {
            if method.trim().is_empty()
                || chrono::DateTime::parse_from_rfc3339(observed_at).is_err()
            {
                bail!("Native metadata needs a method and RFC3339 observation time");
            }
        }
        let mut models = HashSet::new();
        for model in &self.models {
            if model.id.trim().is_empty() || !models.insert(&model.id) {
                bail!("Duplicate or empty model id");
            }
            if model
                .display_name
                .as_ref()
                .is_some_and(|name| name.trim().is_empty())
                || model
                    .resolved_model
                    .as_ref()
                    .is_some_and(|id| id.trim().is_empty())
                || model.aliases.iter().any(|id| id.trim().is_empty())
            {
                bail!("Native labels and aliases cannot be empty");
            }
            let mut controls = HashSet::new();
            for control in model.controls.iter().flatten() {
                if control.id.trim().is_empty() || !controls.insert(&control.id) {
                    bail!("Duplicate or empty native control id");
                }
                match &control.values {
                    NativeControlValues::Choices { options } => {
                        let mut unique = HashSet::new();
                        if options.is_empty()
                            || options
                                .iter()
                                .any(|v| v.trim().is_empty() || !unique.insert(v))
                        {
                            bail!("Native choices must be nonempty and unique");
                        }
                    }
                    NativeControlValues::Integer { min, max } if min > max => {
                        bail!("Native integer range is reversed");
                    }
                    _ => {}
                }
                if control
                    .default
                    .as_ref()
                    .is_some_and(|v| !control.values.contains(v))
                {
                    bail!("Native control default is outside its supported values");
                }
            }
        }
        if let Some(default) = &self.default_model {
            if default.trim().is_empty() || (self.models_complete && self.model(default).is_none())
            {
                bail!("Default model is empty or absent from the complete model list");
            }
        }
        Ok(())
    }
}

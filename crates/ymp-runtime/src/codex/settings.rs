//! Native Codex model discovery and setting validation.

use std::collections::{BTreeSet, HashSet};
use std::time::Instant;

use serde_json::{Value, json};
use ymp_kernel::execution::{
    ModelOffering, OfferingId, SettingKey, SettingValue, Settings, SupportedControl,
};

use super::rpc::{RpcFailure, RpcProcess};

const MAX_MODEL_PAGES: usize = 32;
const MAX_MODELS: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ModelMetadata {
    id: String,
    efforts: Vec<String>,
    is_default: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ModelCatalog {
    models: Vec<ModelMetadata>,
}

impl ModelCatalog {
    pub(super) fn offering(&self, id: OfferingId) -> Result<ModelOffering, String> {
        let model_key = SettingKey::new("model").map_err(|error| error.to_string())?;
        let effort_key = SettingKey::new("effort").map_err(|error| error.to_string())?;
        let mut model_values = Vec::with_capacity(self.models.len());
        let mut common_efforts: Option<BTreeSet<String>> = None;
        for model in &self.models {
            model_values
                .push(SettingValue::new(model.id.clone()).map_err(|error| error.to_string())?);
            let efforts = model.efforts.iter().cloned().collect::<BTreeSet<_>>();
            common_efforts = Some(match common_efforts {
                None => efforts,
                Some(common) => common.intersection(&efforts).cloned().collect(),
            });
        }
        let mut controls = vec![SupportedControl::new(model_key, model_values)];
        if let Some(effort_values) = common_efforts.filter(|values| !values.is_empty()) {
            controls.push(SupportedControl::new(
                effort_key,
                effort_values
                    .into_iter()
                    .map(SettingValue::new)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())?,
            ));
        }
        ModelOffering::new(id, controls).map_err(|error| error.to_string())
    }

    pub(super) fn validate(&self, settings: &Settings) -> Result<(), String> {
        let model = setting(settings, "model");
        let effort = setting(settings, "effort");
        let selected = match model {
            Some(model) => Some(
                self.models
                    .iter()
                    .find(|metadata| metadata.id == model)
                    .ok_or_else(|| {
                        format!("model '{model}' is not advertised by native metadata")
                    })?,
            ),
            None => self.models.iter().find(|metadata| metadata.is_default),
        };
        if let Some(effort) = effort {
            let selected = selected.ok_or_else(|| {
                "Codex did not advertise a default model for effort validation".to_owned()
            })?;
            if !selected.efforts.iter().any(|supported| supported == effort) {
                return Err(format!(
                    "model '{}' does not advertise effort '{effort}'",
                    selected.id
                ));
            }
        }
        Ok(())
    }

    pub(super) fn verify_acknowledgement(
        &self,
        settings: &Settings,
        response: &Value,
    ) -> Result<(), String> {
        let requested_model = setting(settings, "model");
        let requested_effort = setting(settings, "effort");
        let native_model = response.get("model").and_then(Value::as_str);
        let native_effort = response.get("reasoningEffort").and_then(Value::as_str);

        if let Some(model) = native_model {
            let metadata = self
                .models
                .iter()
                .find(|metadata| metadata.id == model)
                .ok_or_else(|| {
                    format!("acknowledged model '{model}' is absent from native metadata")
                })?;
            if let Some(effort) = requested_effort
                && !metadata.efforts.iter().any(|supported| supported == effort)
            {
                return Err(format!(
                    "acknowledged model '{model}' does not advertise effort '{effort}'"
                ));
            }
            if requested_model.is_some_and(|requested| requested != model) {
                return Err("Codex acknowledged a different model than sent".to_owned());
            }
        } else if requested_effort.is_some() && requested_model.is_none() {
            return Err(
                "Codex did not identify the native default model for effort validation".to_owned(),
            );
        }
        if let (Some(requested), Some(actual)) = (requested_effort, native_effort)
            && requested != actual
        {
            return Err("Codex acknowledged a different effort than sent".to_owned());
        }
        Ok(())
    }
}

pub(super) fn setting<'a>(settings: &'a Settings, key: &str) -> Option<&'a str> {
    settings
        .get(&SettingKey::new(key).expect("static setting key is valid"))
        .map(SettingValue::as_str)
}

pub(super) fn load_catalog(
    process: &mut RpcProcess,
    deadline: Instant,
) -> Result<ModelCatalog, RpcFailure> {
    let mut models = Vec::new();
    let mut cursor = Value::Null;
    let mut seen_cursors = HashSet::new();
    let mut seen_models = HashSet::new();
    loop {
        if seen_cursors.len() >= MAX_MODEL_PAGES || models.len() >= MAX_MODELS {
            return Err(RpcFailure::protocol(
                "Codex model/list exceeded its metadata bound",
            ));
        }
        let page = process.request(
            "model/list",
            json!({"includeHidden": true, "cursor": cursor}),
            deadline,
        )?;
        let data = page
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| RpcFailure::protocol("Codex model/list omitted its data array"))?;
        for model in data {
            if models.len() >= MAX_MODELS {
                return Err(RpcFailure::protocol(
                    "Codex model/list exceeded its model bound",
                ));
            }
            let id = model
                .get("model")
                .and_then(Value::as_str)
                .filter(|id| !id.trim().is_empty())
                .ok_or_else(|| RpcFailure::protocol("Codex model/list omitted a model ID"))?;
            if !seen_models.insert(id.to_owned()) {
                return Err(RpcFailure::protocol("Codex model/list repeated a model ID"));
            }
            let efforts = model
                .get("supportedReasoningEfforts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|entry| entry.get("reasoningEffort").and_then(Value::as_str))
                .map(str::to_owned)
                .collect();
            models.push(ModelMetadata {
                id: id.to_owned(),
                efforts,
                is_default: model.get("isDefault").and_then(Value::as_bool) == Some(true),
            });
        }
        cursor = page.get("nextCursor").cloned().unwrap_or(Value::Null);
        if cursor.is_null() {
            break;
        }
        if !seen_cursors.insert(cursor.to_string()) {
            return Err(RpcFailure::protocol("Codex model/list repeated its cursor"));
        }
    }
    if models.is_empty() {
        return Err(RpcFailure::protocol("Codex model/list returned no models"));
    }
    models.sort_by_key(|model| !model.is_default);
    Ok(ModelCatalog { models })
}

#[cfg(test)]
mod tests {
    use super::{ModelCatalog, ModelMetadata};
    use ymp_kernel::execution::{SettingKey, SettingValue, Settings};

    fn catalog() -> ModelCatalog {
        ModelCatalog {
            models: vec![
                ModelMetadata {
                    id: "model-a".to_owned(),
                    efforts: vec!["low".to_owned(), "high".to_owned()],
                    is_default: true,
                },
                ModelMetadata {
                    id: "model-b".to_owned(),
                    efforts: vec!["max".to_owned()],
                    is_default: false,
                },
            ],
        }
    }

    fn settings(model: &str, effort: &str) -> Settings {
        Settings::from_pairs([
            (
                SettingKey::new("model").unwrap(),
                SettingValue::new(model).unwrap(),
            ),
            (
                SettingKey::new("effort").unwrap(),
                SettingValue::new(effort).unwrap(),
            ),
        ])
        .unwrap()
    }

    #[test]
    fn validation_preserves_model_specific_effort_support() {
        assert!(catalog().validate(&settings("model-b", "max")).is_ok());
        assert!(catalog().validate(&settings("model-b", "high")).is_err());
        assert!(catalog().validate(&settings("missing", "high")).is_err());
    }
}

use crate::{rpc::RpcProcess, ProviderEvent};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use ymp_core::*;

pub(crate) fn publish(
    catalog: &ProviderCapabilities,
    events: &mpsc::UnboundedSender<ProviderEvent>,
) {
    let _ = events.send(ProviderEvent::Capabilities(catalog.clone()));
}

pub(crate) fn native_catalog(method: &str) -> ProviderCapabilities {
    ProviderCapabilities {
        source: CapabilitySource::NativeMetadata {
            method: method.into(),
            observed_at: now(),
        },
        ..Default::default()
    }
}

pub(crate) async fn codex_catalog(
    proc: &mut RpcProcess,
    events: &mpsc::UnboundedSender<ProviderEvent>,
) -> Result<ProviderCapabilities> {
    let mut catalog = native_catalog("model/list");
    let mut cursor = Value::Null;
    let mut seen = std::collections::HashSet::new();
    loop {
        let page = proc
            .request(
                "model/list",
                json!({"includeHidden":true,"cursor":cursor}),
                events,
            )
            .await?;
        for model in page["data"]
            .as_array()
            .context("Codex model/list data missing")?
        {
            let id = model["model"]
                .as_str()
                .context("Codex model identifier missing")?;
            let options = model["supportedReasoningEfforts"].as_array().map(|levels| {
                levels
                    .iter()
                    .filter_map(|v| v["reasoningEffort"].as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            });
            let controls = options.map(|options| {
                if options.is_empty() {
                    vec![]
                } else {
                    vec![NativeControl {
                        id: "effort".into(),
                        values: NativeControlValues::Choices { options },
                        default: model["defaultReasoningEffort"]
                            .as_str()
                            .map(|s| NativeControlValue::Choice(s.into())),
                    }]
                }
            });
            // The picker row ID can differ from the actual wire model name.
            catalog.models.push(ModelCapabilities {
                id: id.into(),
                controls,
            });
            if model["isDefault"] == true {
                catalog.default_model = Some(id.into());
            }
        }
        cursor = page.get("nextCursor").cloned().unwrap_or(Value::Null);
        if cursor.is_null() {
            break;
        }
        if !seen.insert(cursor.to_string()) {
            bail!("Codex repeated its model-list cursor");
        }
    }
    catalog.models_complete = true;
    catalog.validate()?;
    publish(&catalog, events);
    Ok(catalog)
}

pub(crate) fn validate_choice(
    catalog: &ProviderCapabilities,
    model: &str,
    effort: Option<&str>,
    control_id: &str,
) -> Result<()> {
    let model = catalog
        .model(model)
        .with_context(|| format!("Model {model} is not advertised by native metadata"))?;
    if let Some(effort) = effort {
        let control = model
            .controls
            .as_ref()
            .and_then(|c| c.iter().find(|c| c.id == control_id))
            .with_context(|| {
                format!(
                    "Native {control_id} support was not observed for {}",
                    model.id
                )
            })?;
        if !control
            .values
            .contains(&NativeControlValue::Choice(effort.into()))
        {
            bail!("Unsupported {control_id} {effort} for model {}", model.id);
        }
    }
    Ok(())
}

pub(crate) fn acp_catalog(response: &Value, method: &str) -> Result<ProviderCapabilities> {
    let mut catalog = native_catalog(method);
    let current = response
        .pointer("/models/currentModelId")
        .and_then(Value::as_str);
    catalog.default_model = current.map(str::to_owned);
    for model in response
        .pointer("/models/availableModels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(id) = model["modelId"].as_str() {
            catalog.models.push(ModelCapabilities {
                id: id.into(),
                controls: if Some(id) == current {
                    acp_controls(&response["configOptions"])?
                } else {
                    None
                },
            });
        }
    }
    if let Some(id) = current.filter(|id| catalog.model(id).is_none()) {
        catalog.models.push(ModelCapabilities {
            id: id.into(),
            controls: acp_controls(&response["configOptions"])?,
        });
    }
    // ACP and GLM explicitly allow additional unlisted model IDs; callers with
    // effort still need this model's actual advertised configuration options.
    catalog.models_complete = false;
    catalog.validate()?;
    Ok(catalog)
}

pub(crate) fn acp_controls(options: &Value) -> Result<Option<Vec<NativeControl>>> {
    let Some(options) = options.as_array() else {
        return Ok(None);
    };
    let mut controls = vec![];
    for option in options {
        if option["id"] != "thought_level" || option["type"] != "select" {
            continue;
        }
        let values = option["options"]
            .as_array()
            .context("ACP thought_level options missing")?
            .iter()
            .filter_map(|v| v["value"].as_str().map(str::to_owned))
            .collect::<Vec<_>>();
        if values.is_empty() {
            bail!("ACP thought_level has no supported values");
        }
        controls.push(NativeControl {
            id: "thought_level".into(),
            values: NativeControlValues::Choices { options: values },
            default: None,
        });
    }
    Ok(Some(controls))
}

pub(crate) fn acp_effort(options: &Value) -> Option<String> {
    options
        .as_array()?
        .iter()
        .find(|o| o["id"] == "thought_level")?["currentValue"]
        .as_str()
        .map(str::to_owned)
}

pub(crate) fn refreshed_acp_model(
    catalog: &mut ProviderCapabilities,
    model: &str,
    options: &Value,
) -> Result<()> {
    let controls = acp_controls(options)?;
    if let Some(entry) = catalog.models.iter_mut().find(|m| m.id == model) {
        entry.controls = controls;
    } else {
        catalog.models.push(ModelCapabilities {
            id: model.into(),
            controls,
        });
    }
    catalog.source = CapabilitySource::NativeMetadata {
        method: "session/set_model + session/update config_option_update".into(),
        observed_at: now(),
    };
    Ok(())
}

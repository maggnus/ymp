use super::*;
use crate::{inspect_capabilities, TurnRequest};
use anyhow::{bail, ensure};
use tokio_util::sync::CancellationToken;
use ymp_core::*;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub provider: Option<String>,
    pub timeout_secs: u64,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            provider: None,
            timeout_secs: 30,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogScanReport {
    pub providers: Vec<ProviderScanReport>,
    pub created_agents: Vec<String>,
    pub migrated_agents: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderScanReport {
    pub provider: String,
    pub status: String,
    pub model_count: usize,
    pub retained_previous: bool,
}

/// One metadata-only query per enabled provider, serial and deadline bounded.
/// A failed provider retains its last applicable snapshot with an explicit stale
/// status. No native diagnostic text or environment values enter the cache.
pub async fn refresh_catalog(
    config: &mut Config,
    home: &Path,
    cwd: &Path,
    bridge: &Path,
    options: ScanOptions,
    cancel: CancellationToken,
) -> Result<CatalogScanReport> {
    ensure!(
        (1..=60).contains(&options.timeout_secs),
        "Catalog timeout must be between 1 and 60 seconds per provider"
    );
    config.validate()?;
    std::fs::create_dir_all(home)?;
    let scan_lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(home.join("catalog-scan.lock"))?;
    fs2::FileExt::try_lock_exclusive(&scan_lock)
        .map_err(|_| anyhow::anyhow!("Another native catalog scan is already running"))?;
    let expected = match std::fs::read(home.join("config.toml")) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if expected.is_some() {
        let stored = Config::load(home)?;
        ensure!(
            serde_json::to_value(&stored)? == serde_json::to_value(&config)?,
            "Configuration changed before native scan; reload it before refreshing"
        );
    }
    if let Some(id) = &options.provider {
        config.provider(id)?;
    }
    let providers = config
        .providers
        .iter()
        .filter(|p| {
            p.enabled
                && p.kind != ProviderKind::Mock
                && options.provider.as_ref().is_none_or(|id| id == &p.id)
        })
        .cloned()
        .collect::<Vec<_>>();
    ensure!(
        providers.len() <= 32,
        "Catalog scan supports at most 32 enabled providers at once"
    );
    let mut next = config.clone();
    let mut report = CatalogScanReport {
        providers: vec![],
        created_agents: vec![],
        migrated_agents: vec![],
    };
    for provider in providers {
        if cancel.is_cancelled() {
            bail!("Catalog scan cancelled");
        }
        let fingerprint = provider_fingerprint(&provider);
        let previous = next
            .native_provider_snapshot(&provider.id)
            .and_then(|entry| entry.catalog.clone());
        let request = TurnRequest {
            profile: AgentProfile {
                id: provider.id.clone(),
                name: provider.id.clone(),
                provider: provider.id.clone(),
                model: None,
                instructions: String::new(),
                enabled: true,
            },
            settings: Default::default(),
            provider: provider.clone(),
            cwd: cwd.into(),
            prompt: String::new(),
            purpose: "catalog_scan".into(),
            read_only: true,
            resume: None,
            usage_baseline: None,
            mcp: None,
            resource_controls: Default::default(),
            timeout_secs: options.timeout_secs,
            bridge: bridge.into(),
        };
        let result = inspect_capabilities(request, cancel.clone())
            .await
            .and_then(|catalog| {
                catalog.validate()?;
                ensure!(
                    matches!(catalog.source, CapabilitySource::NativeMetadata { .. }),
                    "Scan did not return native metadata"
                );
                ensure!(
                    catalog.models.len() <= 4096,
                    "Native catalog exceeds the model bound"
                );
                Ok(catalog)
            });
        if cancel.is_cancelled() {
            bail!("Catalog scan cancelled");
        }
        let (catalog, failure, status) = match result {
            Ok(catalog) => (Some(catalog), None, "updated"),
            Err(_) => (
                previous.clone(),
                Some("native_metadata_unavailable".into()),
                "failed",
            ),
        };
        let model_count = catalog.as_ref().map_or(0, |catalog| catalog.models.len());
        next.native_catalog.providers.insert(
            provider.id.clone(),
            NativeProviderSnapshot {
                provider_fingerprint: fingerprint,
                last_attempt: now(),
                failure,
                catalog,
            },
        );
        report.providers.push(ProviderScanReport {
            provider: provider.id.clone(),
            status: status.into(),
            model_count,
            retained_previous: status == "failed" && previous.is_some(),
        });
        if status == "updated" {
            reconcile_provider(&mut next, &provider.id, &mut report)?;
        }
    }
    next.validate()?;
    next.save_scanned_catalog(home, expected.as_deref())?;
    *config = next;
    Ok(report)
}

fn reconcile_provider(
    config: &mut Config,
    provider: &str,
    report: &mut CatalogScanReport,
) -> Result<()> {
    let catalog = config
        .native_provider_snapshot(provider)
        .and_then(|entry| entry.catalog.clone())
        .expect("successful scan");
    // A native default is preferred. A partial listing without one cannot
    // identify an installation default, so choose and explicitly send its first
    // offering for generated placeholders only; never change a custom profile.
    let selected = catalog
        .default_model
        .as_deref()
        .and_then(|id| catalog.model(id))
        .or_else(|| catalog.models.first());
    if let Some(selected) = selected {
        let migrations = config
            .agents
            .iter()
            .filter(|a| a.provider == provider && config.is_legacy_provider_placeholder(a))
            .map(|a| a.id.clone())
            .collect::<Vec<_>>();
        for id in migrations {
            let profile = config.agent(&id)?.clone();
            let effective = config.execution_settings(&profile, &Default::default())?;
            // An explicit policy already supplies a concrete execution model.
            // Materializing it again in the raw profile would change the version
            // of unchanged execution and disconnect its qualified experience.
            let model = if let Some(model) = effective.model {
                model
            } else {
                config
                    .agents
                    .iter_mut()
                    .find(|a| a.id == id)
                    .expect("existing actor")
                    .model = Some(selected.id.clone());
                selected.id.clone()
            };
            let changed = config
                .native_catalog
                .generated_agents
                .get(&id)
                .is_none_or(|binding| binding.provider != provider || binding.model != model);
            config.native_catalog.generated_agents.insert(
                id.clone(),
                NativeAgentBinding {
                    provider: provider.into(),
                    model,
                },
            );
            if changed {
                report.migrated_agents.push(id);
            }
        }
    }
    for offering in &catalog.models {
        // Resolved IDs and picker aliases represent one native offering. Existing
        // custom actors may share it without being merged, removed or renamed.
        let represented = config
            .agents
            .iter()
            .filter(|a| a.provider == provider)
            .any(|a| {
                config
                    .execution_settings(a, &Default::default())
                    .ok()
                    .and_then(|s| s.model)
                    .and_then(|id| catalog.model(&id))
                    .is_some_and(|m| m.id == offering.id)
            });
        if represented {
            continue;
        }
        let digest = content_digest(&serde_json::to_string(&(provider, &offering.id))?);
        let base = format!("native-{}", &digest[..24]);
        let mut id = base.clone();
        let mut suffix = 1;
        while config.agents.iter().any(|a| a.id == id) {
            id = format!("{base}-{suffix}");
            suffix += 1;
        }
        config.agents.push(AgentProfile {
            id: id.clone(),
            name: offering
                .display_name
                .clone()
                .unwrap_or_else(|| offering.id.clone()),
            provider: provider.into(),
            model: Some(offering.id.clone()),
            instructions: String::new(),
            enabled: true,
        });
        config.native_catalog.generated_agents.insert(
            id.clone(),
            NativeAgentBinding {
                provider: provider.into(),
                model: offering.id.clone(),
            },
        );
        report.created_agents.push(id);
    }
    // Starting roster, execution policies, constraints, original instructions,
    // disabled state and all session/history data remain untouched.
    Ok(())
}

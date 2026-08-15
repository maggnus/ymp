//! A product root a run can actually be created under.
//!
//! Since the pool a run draws its models from is frozen when the run is created, every check that
//! starts a run needs a root that offers at least one entry. Reaching that state by starting
//! engines would make each such check depend on what is installed on the host running it, so the
//! records are written directly instead: an admitted engine with a measured model list and a
//! credential, an enabled account observed from it, and the pool that observation resolves.
//!
//! This is the state one measurement of one account leaves behind. Nothing here reaches a network,
//! starts a process, or reads the operator's own root — the root is whatever the caller passes,
//! which in a check is a temporary directory.
//!
//! **What is already recorded is kept.** A check that has configured an engine of its own — a
//! fixture program on a search path of its own, probed through the product's own commands — has
//! recorded an executable this fixture must not replace. So the executable, its version and its
//! digest are filled in only where the record carries none, and the model list and the credential
//! are what this fixture states.

use std::path::Path;

use ymp_runtime_registry::{Engine, ModelCatalog, ModelSource, Pools, Providers, Registry};

/// The models the fixture engine serves. The first of them is the entry a run created under such a
/// root ignites on, since the ignition rule reads position and measured readiness alone.
pub const MODELS: [&str; 2] = ["claude-opus-5", "claude-sonnet-5"];

/// Put one root into the state a measured, enabled Anthropic account leaves it in, and resolve its
/// pools.
///
/// `root` is the product root. An invocation that names one exact store is governed by the root
/// that store stands under, which for a store outside any layout is the store itself.
pub fn measured(root: &Path) {
    measured_engine(root, Engine::ClaudeCode, &MODELS);
}

/// The same, for a stated engine and model list.
///
/// A check whose runtime profile is one particular engine states that engine here, so the pool it
/// creates does not enable a second profile beside the one the check routes through.
pub fn measured_engine(root: &Path, engine: Engine, models: &[&str]) {
    let registry = Registry::under(root);
    let providers = Providers::under(root);
    let digest = registry
        .read(engine)
        .ok()
        .and_then(|record| record.properties.executable_digest)
        .unwrap_or_else(|| format!("{}-digest", engine.name()));
    let recorded_digest = digest.clone();
    registry
        .update(engine, |record| {
            record.enabled = true;
            record.disabled_reason = None;
            record
                .properties
                .executable
                .get_or_insert_with(|| format!("/usr/local/bin/{}", engine.program()));
            record
                .properties
                .version
                .get_or_insert_with(|| "1.2.3".to_owned());
            record
                .properties
                .executable_digest
                .get_or_insert(recorded_digest);
            record.properties.credential_origin =
                Some("delegated_host_keychain_credential".to_owned());
            record.models = ModelCatalog {
                source: ModelSource::Measured,
                measured_for_version: record.properties.version.clone(),
                measured_for_digest: Some(digest.clone()),
                note: None,
                names: models.iter().map(|name| (*name).to_owned()).collect(),
            };
        })
        .expect("record the measurement");
    providers
        .set_enabled(engine.provider(), true, None)
        .expect("enable the account");
    providers
        .observe(&registry)
        .expect("observe the enabled account");
    Pools::under(root)
        .reconcile(&providers, &registry)
        .expect("resolve the pools of this root");
}

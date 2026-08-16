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

/// The same root, with the engine it measured taken off admission.
///
/// Which engine this host admits and which account is enabled are two decisions, and only the
/// second decides what a run may draw on: the pool stays alive, so a run is still created and still
/// freezes the entry it ignites on. What changes is that nothing on this host will start that
/// entry — the start refuses at the engine record, before a driver is built and before any process
/// exists.
///
/// It is what a check whose subject is not the participant stands on. Authorizing a run now starts
/// the participant the run ignites on, and a check standing on a root that admits an engine
/// installed beside it would start that engine's real agent: it would measure the machine, spend
/// the operator's account, and take as long as the agent takes. A check that does want a
/// participant serves the route with the fixture runtime instead.
pub fn measured_with_no_engine_admitted(root: &Path) {
    measured(root);
    hold_engine_back(root, Engine::ClaudeCode);
}

/// The same, for a stated engine and model list.
pub fn measured_engine_with_no_admission(root: &Path, engine: Engine, models: &[&str]) {
    measured_engine(root, engine, models);
    hold_engine_back(root, engine);
}

fn hold_engine_back(root: &Path, engine: Engine) {
    Registry::under(root)
        .update(engine, |record| {
            record.enabled = false;
            record.disabled_reason = Some("not admitted on the host this check runs on".to_owned());
        })
        .expect("take the engine off admission");
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

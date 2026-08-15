//! The model catalog: the union of the measured engine model lists, attributed to the provider
//! that serves them.
//!
//! A catalog entry is one triple — provider, engine, model — with the properties a later decision
//! reads and the reason it is or is not offered (`ymp-docs/design/COLLECTIVE-DESIGN.md`, §6). The
//! three levels stay apart in it: the provider is the account, the model is the capability source
//! that account exposes, the engine is the harness that reaches it, and the driver that supervises
//! the engine is internal and appears nowhere.
//!
//! **The catalog is derived, not stored.** The model names live in the engine records and the
//! attribution lives in the provider records; a third object holding the union would be a copy of
//! both, and a copy is what goes stale while reading as current. [`Catalog::read`] joins them at
//! the moment it is asked, so an engine disabled or re-measured a second ago is read as it stands
//! now.
//!
//! **An entry that is not admissible is present and marked, never dropped.** A surface that showed
//! only what is usable would answer the operator's question — why is this model not offered — with
//! silence.
//!
//! Three things this level deliberately does not do. It creates no participant: an entry is a
//! record of what exists, and nothing is instantiated by being in the catalog. It changes no
//! admission: whether an engine may be started is answered by [`crate::Registry::admit`] from the
//! enabled flag alone, and an entry marked unavailable here leaves that answer untouched. It
//! declares no permission: which of these entries a run may use is not decided under this root.
//!
//! One property is stated for what it is not. No pairing in this build carries a conformance
//! probe, because every entry here is an engine reaching its own vendor's account. When a route
//! override reaches one vendor's models through another vendor's engine, that pairing is probed in
//! its own right and its result becomes part of an entry's availability; until then no entry
//! claims a conformance result it does not have.

use crate::{
    Engine, ModelSource, ProviderError, ProviderFamily, ProviderState, Providers, Registry,
};

/// Whether an entry is offered, and why it is not when it is not.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Availability {
    /// The engine is admitted and reported a release, and the provider states where its credential
    /// is read from. Whether the model answers is measured when it is asked, not here.
    Admissible,
    /// The entry exists and is not offered. The reason is the measured one, in the words the
    /// record that holds it states.
    Unavailable { reason: String },
}

/// One usable triple as the root holds it: provider, engine, model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogEntry {
    pub provider: String,
    pub family: ProviderFamily,
    pub engine: Engine,
    pub model: String,
    /// Where the list this model stands in came from.
    pub source: ModelSource,
    /// The digest of the engine build the list was measured against, when one was recorded.
    pub measured_for_digest: Option<String>,
    pub availability: Availability,
}

impl CatalogEntry {
    pub fn is_admissible(&self) -> bool {
        self.availability == Availability::Admissible
    }

    /// Whether the list this entry stands in was measured against the engine build installed now.
    ///
    /// The caller supplies the digest of the installed executable, because nothing in this crate
    /// opens one. An entry that was never measured is never current.
    pub fn current_for(&self, executable_digest: &str) -> bool {
        self.source != ModelSource::Unmeasured
            && self.measured_for_digest.as_deref() == Some(executable_digest)
    }
}

/// The catalog of one product root.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    entries: Vec<CatalogEntry>,
}

impl Catalog {
    /// Join the observed providers with the measured engine lists.
    ///
    /// A provider this root has observed nothing about contributes nothing: an entry states which
    /// account would serve a model, and attributing one at read time would invent the very fact
    /// the provider record exists to hold. Observing is one call
    /// ([`Providers::observe`]), and it opens no process.
    pub fn read(providers: &Providers, registry: &Registry) -> Result<Self, ProviderError> {
        let mut entries = Vec::new();
        for family in ProviderFamily::ALL {
            let Some(record) = providers.read(family)? else {
                continue;
            };
            for route in &record.routes {
                let engine =
                    Engine::parse(&route.engine).map_err(|error| ProviderError::Unreadable {
                        path: providers.path_of(family),
                        reason: error.to_string(),
                    })?;
                // The engine record is read now rather than taken from the observation, so a list
                // re-measured or an engine held back since then is read as it stands. A record
                // that cannot be read fails the whole reading: skipping it would answer with a
                // shorter catalog that carries no sign of being short, which is the one way an
                // absent entry is worse than a marked one.
                let engine_record = registry.read(engine)?;
                let availability = if !engine_record.enabled {
                    Availability::Unavailable {
                        reason: format!(
                            "the {} engine is disabled — {}",
                            engine.name(),
                            engine_record.refusal_reason()
                        ),
                    }
                } else if record.state != ProviderState::Ready {
                    Availability::Unavailable {
                        reason: format!(
                            "{} is {} — {}",
                            record.provider,
                            record.state.label(),
                            record.reason
                        ),
                    }
                } else if engine_record.models.source == ModelSource::Unmeasured {
                    Availability::Unavailable {
                        reason: format!(
                            "the {} model list was not measured on this host",
                            engine.name()
                        ),
                    }
                } else {
                    Availability::Admissible
                };
                for model in &engine_record.models.names {
                    entries.push(CatalogEntry {
                        provider: record.provider.clone(),
                        family,
                        engine,
                        model: model.clone(),
                        source: engine_record.models.source,
                        measured_for_digest: engine_record.models.measured_for_digest.clone(),
                        availability: availability.clone(),
                    });
                }
            }
        }
        Ok(Self { entries })
    }

    /// Every entry, in provider order, then registry order, then the order the list was measured
    /// in. Nothing here orders entries by preference: a rank would be a decision this level does
    /// not take.
    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries one provider serves.
    pub fn of(&self, family: ProviderFamily) -> impl Iterator<Item = &CatalogEntry> {
        self.entries
            .iter()
            .filter(move |entry| entry.family == family)
    }

    /// The entries that are offered.
    pub fn admissible(&self) -> impl Iterator<Item = &CatalogEntry> {
        self.entries.iter().filter(|entry| entry.is_admissible())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelCatalog, ProviderRecord};
    use std::fs;

    fn root() -> (tempfile::TempDir, Registry, Providers) {
        let directory = tempfile::tempdir().expect("temporary root");
        let registry = Registry::under(directory.path());
        let providers = Providers::under(directory.path());
        (directory, registry, providers)
    }

    fn measure(registry: &Registry, engine: Engine, credential: Option<&str>, names: &[&str]) {
        registry
            .update(engine, |record| {
                record.enabled = true;
                record.disabled_reason = None;
                record.properties.executable = Some(format!("/usr/local/bin/{}", engine.program()));
                record.properties.version = Some("1.2.3".to_owned());
                record.properties.executable_digest = Some(format!("{}-digest", engine.name()));
                record.properties.credential_origin = credential.map(str::to_owned);
                record.models = ModelCatalog {
                    source: ModelSource::Measured,
                    measured_for_version: Some("1.2.3".to_owned()),
                    measured_for_digest: Some(format!("{}-digest", engine.name())),
                    note: None,
                    names: names.iter().map(|name| (*name).to_owned()).collect(),
                };
            })
            .expect("record the measurement");
    }

    /// The negative half: engine records alone hold model names that no account is attributed to,
    /// so there is no catalog entry to read. Observing the providers is what turns a measured list
    /// into triples, and each triple keeps provider, engine and model apart.
    #[test]
    fn a_measured_engine_list_becomes_a_catalog_entry_only_once_a_provider_is_observed() {
        let (_directory, registry, providers) = root();
        measure(
            &registry,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5", "claude-haiku-4-5"],
        );
        assert_eq!(
            registry
                .read(Engine::ClaudeCode)
                .expect("engine record")
                .models
                .names
                .len(),
            2,
            "the measured list this test attributes is missing"
        );
        assert!(
            Catalog::read(&providers, &registry)
                .expect("catalog")
                .is_empty(),
            "a model was attributed to an account nothing had observed"
        );

        providers.observe(&registry).expect("observe the providers");
        let catalog = Catalog::read(&providers, &registry).expect("catalog");
        let entries = catalog.entries();
        assert_eq!(entries.len(), 2, "{entries:?}");
        let entry = &entries[0];
        assert_eq!(entry.provider, "anthropic");
        assert_eq!(entry.engine, Engine::ClaudeCode);
        assert_eq!(entry.model, "claude-opus-5");
        assert_eq!(entry.source, ModelSource::Measured);
        assert!(entry.is_admissible(), "{:?}", entry.availability);
        assert!(entry.current_for("claude-code-digest"));
        assert!(!entry.current_for("another-build-digest"));
        assert_eq!(catalog.admissible().count(), 2);
        assert_eq!(catalog.of(ProviderFamily::OpenAi).count(), 0);
    }

    /// An entry that is not offered is present and carries the measured reason, and marking it
    /// changes no engine's admission.
    #[test]
    fn an_entry_that_is_not_offered_is_marked_rather_than_dropped() {
        let (_directory, registry, providers) = root();
        measure(&registry, Engine::ClaudeCode, None, &["claude-opus-5"]);
        providers.observe(&registry).expect("observe");
        let catalog = Catalog::read(&providers, &registry).expect("catalog");
        assert_eq!(catalog.entries().len(), 1);
        let entry = &catalog.entries()[0];
        assert!(!entry.is_admissible());
        assert_eq!(entry.model, "claude-opus-5");
        let Availability::Unavailable { reason } = &entry.availability else {
            panic!("an unconfigured provider offered its models: {entry:?}");
        };
        assert!(reason.contains("not configured"), "{reason}");
        registry
            .admit(Engine::ClaudeCode)
            .expect("marking an entry unavailable held back an admitted engine");

        // The engine's own decision is stated ahead of the provider's state, because it is the one
        // the operator took.
        registry
            .set_enabled(Engine::ClaudeCode, false, Some("held back"))
            .expect("disable claude");
        let catalog = Catalog::read(&providers, &registry).expect("catalog");
        let Availability::Unavailable { reason } = &catalog.entries()[0].availability else {
            panic!("a disabled engine offered its models");
        };
        assert!(reason.contains("held back"), "{reason}");
    }

    /// The join is made when the catalog is read, so a list re-measured after the last observation
    /// is the one that is read back.
    #[test]
    fn the_catalog_reads_the_engine_list_as_it_stands_now() {
        let (_directory, registry, providers) = root();
        measure(
            &registry,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        providers.observe(&registry).expect("observe");
        measure(
            &registry,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5", "claude-sonnet-5"],
        );
        let catalog = Catalog::read(&providers, &registry).expect("catalog");
        assert_eq!(catalog.entries().len(), 2, "{:?}", catalog.entries());
    }

    /// Both providers are held apart: two engines, two accounts, and every entry names which of
    /// them serves it.
    #[test]
    fn each_provider_carries_the_models_its_own_engine_serves() {
        let (_directory, registry, providers) = root();
        measure(
            &registry,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        measure(
            &registry,
            Engine::Codex,
            Some("delegated_home_credential"),
            &["gpt-5-codex"],
        );
        providers.observe(&registry).expect("observe");
        let catalog = Catalog::read(&providers, &registry).expect("catalog");
        let stated: Vec<(String, &str, String)> = catalog
            .entries()
            .iter()
            .map(|entry| {
                (
                    entry.provider.clone(),
                    entry.engine.name(),
                    entry.model.clone(),
                )
            })
            .collect();
        assert_eq!(
            stated,
            vec![
                (
                    "anthropic".to_owned(),
                    "claude-code",
                    "claude-opus-5".to_owned()
                ),
                ("openai".to_owned(), "codex", "gpt-5-codex".to_owned()),
            ]
        );
    }

    /// A provider record naming an engine this build does not manage is refused rather than
    /// resolved to the nearest one; a catalog that guessed would attribute a model to an account
    /// nothing measured.
    #[test]
    fn a_record_naming_an_unmanaged_engine_is_refused() {
        let (_directory, registry, providers) = root();
        providers.observe(&registry).expect("observe");
        let path = providers.path_of(ProviderFamily::Anthropic);
        let stored = fs::read_to_string(&path).expect("stored record");
        let mut record: ProviderRecord = serde_json::from_str(&stored).expect("record");
        record.routes[0].engine = "opencode".to_owned();
        providers
            .write(ProviderFamily::Anthropic, &record)
            .expect("write the amended record");
        let error =
            Catalog::read(&providers, &registry).expect_err("an unmanaged engine was resolved");
        assert!(
            matches!(error, ProviderError::Unreadable { .. }),
            "{error:?}"
        );
    }

    /// An engine record that cannot be read fails the reading rather than shortening the catalog.
    ///
    /// This is the reading that would otherwise look right and be wrong: the entries of the one
    /// engine whose record broke would be absent, and a caller counting what it received would see
    /// a complete-looking catalog of a provider that serves more than it was shown.
    #[test]
    fn an_unreadable_engine_record_fails_the_reading_rather_than_shortening_it() {
        let (_directory, registry, providers) = root();
        measure(
            &registry,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        measure(
            &registry,
            Engine::Codex,
            Some("delegated_home_credential"),
            &["gpt-5-codex"],
        );
        providers.observe(&registry).expect("observe");
        assert_eq!(
            Catalog::read(&providers, &registry)
                .expect("catalog")
                .entries()
                .len(),
            2
        );

        fs::write(registry.path_of(Engine::Codex), b"{ not json").expect("corrupt the record");
        let error = Catalog::read(&providers, &registry)
            .expect_err("a catalog missing one engine's models was answered as a whole one");
        assert!(matches!(error, ProviderError::Engine { .. }), "{error:?}");
    }
}

//! The provider level as the operator meets it: the supported accounts, one account's properties,
//! and the model catalog those accounts serve.
//!
//! Three surfaces, and one rule holds all of them together: **a provider is not measured before it
//! is enabled** (`ymp-docs/design/PRODUCT-BRIEF-collective-v2.md`, Part A §7). Reading these pages
//! starts no process and reaches no network — every value comes from the records under the product
//! root — so opening the table on a host where nothing is enabled discloses nothing and measures
//! nothing. Measuring is an act of its own, taken when a provider is enabled and when the operator
//! asks for the models to be refreshed, and both are keys on the provider's own properties view.
//!
//! * `/providers` lists the **full supported list**, whether or not anything is configured, so the
//!   operator sees the whole space rather than the part they have touched;
//! * a provider's properties state what was measured, when it was measured, and — above the key
//!   that enables it — what enabling permits ([decision
//!   D4](../../../../ymp-docs/design/COLLECTIVE-OWNER-DECISIONS.md): enabling a provider *is* the
//!   disclosure consent, and it is stated before the key and never in a dialogue after it);
//! * `/models` is the catalog derived from the provider and engine records at the moment it is
//!   read, with the age of every observation stated and a route that serves nothing drawn as a
//!   route that serves nothing.
//!
//! The supported list has one source, [`ProviderFamily::ALL`], so a provider this build learns to
//! reach is one entry there rather than a row on each of these surfaces.

use std::time::{Duration, SystemTime};

use ymp_runtime_registry::{
    Availability, Catalog, CatalogEntry, Engine, ProviderRecord, ProviderRoute, Registry,
    RegistryAddress,
};

/// The account an entry of the supported list stands for.
///
/// It is re-exported here because this module is where the interface states the provider level:
/// the command surface performs the interface's actions through the interface's own session and
/// never reaches durable state of its own, so it names this level here rather than naming the
/// registry crate (`crates/ymp-cli/tests/one_command_path.rs`).
pub use ymp_runtime_registry::ProviderFamily;

/// Every provider this build reaches, in the order every surface lists them.
///
/// One source: a provider this build learns to reach is one entry of the registry's own list and
/// appears on each of these surfaces without a line of its own.
pub fn supported() -> &'static [ProviderFamily] {
    &ProviderFamily::ALL
}

/// Where one provider stands in that list, which is the row a surface selects it by.
pub fn index_of(family: ProviderFamily) -> Option<usize> {
    supported().iter().position(|listed| *listed == family)
}

use crate::pages::{Body, Cell, Column, DescribeGroup, Page, Row};
use crate::style;
use crate::theme;

/// What one engine beneath a provider holds, read from the records and never from a process.
#[derive(Clone, Debug)]
pub struct RouteFacts {
    pub engine: Engine,
    /// The engine's own admission decision, which is not the provider's.
    pub admitted: bool,
    /// Why the engine is not admitted, when it is not, or why its record could not be read.
    pub withheld_reason: Option<String>,
    pub executable: Option<String>,
    pub version: Option<String>,
    pub credential_origin: Option<String>,
    /// How many models of the catalog this route contributes.
    pub models: usize,
    /// Why it contributes none, when it contributes none.
    pub without_models: Option<String>,
    /// Whether the observation was taken from the engine build installed now. `None` where one of
    /// the two digests is missing, so nothing is claimed either way.
    pub from_installed_build: Option<bool>,
}

/// One provider of the supported list, as this root holds it.
#[derive(Clone, Debug)]
pub struct ProviderFacts {
    pub family: ProviderFamily,
    pub record: ProviderRecord,
    /// How old the observation is, at the moment this reading was taken.
    pub age: Option<Duration>,
    pub models: usize,
    pub offered: usize,
    pub routes: Vec<RouteFacts>,
}

impl ProviderFacts {
    pub fn name(&self) -> &str {
        &self.record.provider
    }

    pub fn enabled(&self) -> bool {
        self.record.enabled
    }

    /// The models cell of a row: a count, or the dash of a provider nothing has measured.
    pub fn models_text(&self) -> String {
        match (self.record.enabled, self.record.observed_at_ms) {
            (false, _) | (_, None) => "—".to_owned(),
            _ => format!("{}", self.models),
        }
    }

    /// When this provider was last measured, in the words the properties view states.
    pub fn last_refresh(&self) -> String {
        match (self.record.observed_at_ms, self.age) {
            (None, _) => {
                "never — nothing about this provider has been measured on this host".to_owned()
            }
            (Some(_), None) => {
                "at a moment this record does not state — it was written before the observation \
                 was timed"
                    .to_owned()
            }
            (Some(_), Some(age)) => age_text(age),
        }
    }

    /// What enabling this provider permits, or what it already permits — decision D4.
    ///
    /// It is one sentence and it is stated wherever the key that takes the decision is, because
    /// this sentence **is** the disclosure notice: nothing is confirmed afterwards.
    pub fn disclosure(&self) -> String {
        match self.record.enabled {
            true => format!(
                "repository content of any workspace is sent to {}, including the bounded excerpts \
                 ymp reads to work out what \"done\" means before a run starts",
                self.name()
            ),
            false => format!(
                "enabling sends repository content from any workspace to {}, including the \
                 bounded excerpts ymp reads to work out what \"done\" means before a run starts",
                self.name()
            ),
        }
    }
}

/// One reading of the provider level: the supported list, the catalog, and the moment it was read.
#[derive(Clone, Debug)]
pub struct Report {
    pub providers: Vec<ProviderFacts>,
    /// The catalog entries, in provider order then registry order.
    pub entries: Vec<CatalogEntry>,
    /// Why the catalog could not be read, when it could not. A catalog missing one engine's models
    /// is never answered as a whole one, so the failure is stated rather than shortening the table.
    pub catalog_error: Option<String>,
}

impl Report {
    pub fn provider(&self, family: ProviderFamily) -> Option<&ProviderFacts> {
        self.providers
            .iter()
            .find(|provider| provider.family == family)
    }

    pub fn enabled_count(&self) -> usize {
        self.providers
            .iter()
            .filter(|provider| provider.enabled())
            .count()
    }

    pub fn ready_count(&self) -> usize {
        self.providers
            .iter()
            .filter(|provider| provider.record.display_state() == "ready")
            .count()
    }

    /// The engines of every enabled provider, which are the only engines this build measures
    /// without being asked to.
    pub fn engines_of_enabled_providers(&self) -> Vec<Engine> {
        self.providers
            .iter()
            .filter(|provider| provider.enabled())
            .flat_map(|provider| provider.family.engines().iter().copied())
            .collect()
    }
}

/// Read the provider level of one root. Opens no process, reaches no network, writes nothing.
///
/// `now` is the moment the ages are stated against, so a caller that must be reproducible states
/// its own clock rather than reading this one.
pub fn read(address: &RegistryAddress, now: SystemTime) -> Report {
    let providers = address.providers();
    let registry = address.registry();
    let catalog = Catalog::read(&providers, &registry);
    let catalog_error = catalog.as_ref().err().map(ToString::to_string);
    let catalog = catalog.unwrap_or_default();

    let facts = ProviderFamily::ALL
        .into_iter()
        .map(|family| {
            let record = providers
                .read_or_unobserved(family)
                .unwrap_or_else(|error| unreadable_record(family, &error.to_string()));
            let routes = family
                .engines()
                .iter()
                .map(|engine| route_facts(&registry, &catalog, &record, *engine))
                .collect();
            ProviderFacts {
                age: record.observation_age(now),
                models: catalog.of(family).count(),
                offered: catalog
                    .of(family)
                    .filter(|entry| entry.is_admissible())
                    .count(),
                routes,
                family,
                record,
            }
        })
        .collect();

    Report {
        providers: facts,
        entries: catalog.entries().to_vec(),
        catalog_error,
    }
}

/// The record of a provider whose file could not be read. Nothing about it is answered with what it
/// might have said: it is disabled, nothing is measured, and the read failure is its reason.
fn unreadable_record(family: ProviderFamily, reason: &str) -> ProviderRecord {
    let mut record = ProviderRecord::unobserved(family);
    record.disabled_reason = Some(reason.to_owned());
    record
}

fn route_facts(
    registry: &Registry,
    catalog: &Catalog,
    record: &ProviderRecord,
    engine: Engine,
) -> RouteFacts {
    let observed: Option<&ProviderRoute> = record.route(engine);
    let stored = registry.read(engine);
    let catalog_route = catalog
        .routes()
        .iter()
        .find(|route| route.engine == engine && route.family == record.family);
    match stored {
        Err(error) => RouteFacts {
            engine,
            admitted: false,
            withheld_reason: Some(error.to_string()),
            executable: None,
            version: None,
            credential_origin: None,
            models: 0,
            without_models: Some(error.to_string()),
            from_installed_build: None,
        },
        Ok(stored) => RouteFacts {
            engine,
            admitted: stored.enabled,
            withheld_reason: (!stored.enabled).then(|| stored.refusal_reason()),
            executable: stored.properties.executable.clone(),
            version: stored.properties.version.clone(),
            credential_origin: stored.properties.credential_origin.clone(),
            models: catalog_route.map_or(0, |route| route.models),
            without_models: catalog_route.and_then(|route| route.without_models.clone()),
            from_installed_build: observed
                .zip(stored.properties.executable_digest.as_deref())
                .map(|(observed, installed)| observed.current_for(installed)),
        },
    }
}

/// An age in the words a row states it. Whole units only: an age is read at a glance and a second
/// of precision on a measurement taken hours ago would be precision about nothing.
pub fn age_text(age: Duration) -> String {
    let seconds = age.as_secs();
    match seconds {
        0..=59 => format!("{seconds}s ago"),
        60..=3_599 => format!("{}m ago", seconds / 60),
        3_600..=86_399 => format!("{}h {}m ago", seconds / 3_600, (seconds % 3_600) / 60),
        _ => format!("{}d ago", seconds / 86_400),
    }
}

// ---------------------------------------------------------------------------
// `/providers` — the supported list
// ---------------------------------------------------------------------------

/// The `/providers` page: the fixed supported list, whatever this root has touched.
pub fn providers_page(report: &Report, status: String) -> Page {
    let columns = vec![
        Column {
            title: "PROVIDER",
            width: 16,
        },
        Column {
            title: "STATUS",
            width: 18,
        },
        Column {
            title: "MODELS",
            width: 0,
        },
    ];

    let rows = report
        .providers
        .iter()
        .map(|provider| Row {
            cells: vec![
                Cell::new(provider.name().to_owned(), theme::bold()),
                Cell::new(
                    provider.record.display_state(),
                    match provider.record.display_state() {
                        "ready" => theme::green(),
                        _ => theme::red(),
                    },
                ),
                Cell::new(provider.models_text(), theme::muted()),
            ],
            fix: (provider.record.display_state() != "ready").then(|| {
                style::spans(
                    &format!("↳ {}", provider.record.display_reason()),
                    theme::muted(),
                )
            }),
            dim: false,
        })
        .collect::<Vec<_>>();

    Page {
        breadcrumb: vec![
            "transcript".into(),
            format!(
                "providers(all)[{}] · {} enabled · {} ready",
                report.providers.len(),
                report.enabled_count(),
                report.ready_count()
            ),
        ],
        summary: Vec::new(),
        body: Body::Table { columns, rows },
        notes: vec![
            "this is the whole supported list, whether or not anything is configured".to_owned(),
            "nothing about a provider is measured before it is enabled: opening this table starts \
             no process and reaches no network"
                .to_owned(),
            "Enter opens a provider, where what enabling permits is stated above the key that \
             takes the decision"
                .to_owned(),
        ],
        footer: style::spans(&status, theme::muted()),
        keys: vec![("Enter", "properties"), ("Esc", "back")],
        selected: 0,
    }
}

// ---------------------------------------------------------------------------
// One provider's properties
// ---------------------------------------------------------------------------

/// The properties of one provider, with the acts that can be taken on it.
///
/// The keys are the whole reason this view exists rather than a row: what each of them does is
/// stated here, above them, and nothing is confirmed after one is pressed.
pub fn provider_page(provider: &ProviderFacts, status: String) -> Page {
    let mut fields: Vec<(String, Vec<ratatui::text::Span<'static>>)> = Vec::new();
    let value = |text: String| style::spans(&text, theme::text());

    fields.push((
        "status".to_owned(),
        style::spans(
            provider.record.display_state(),
            match provider.record.display_state() {
                "ready" => theme::green(),
                _ => theme::red(),
            },
        ),
    ));
    fields.push(("reason".to_owned(), value(provider.record.display_reason())));
    fields.push(("authentication".to_owned(), value(authentication(provider))));
    fields.push(("models".to_owned(), value(models_field(provider))));
    fields.push(("last refresh".to_owned(), value(provider.last_refresh())));
    for route in &provider.routes {
        fields.push(("reached by".to_owned(), value(reached_by(route))));
    }

    let groups = vec![DescribeGroup {
        title: format!("providers › {}", provider.name()),
        fields,
    }];

    // The consequence of each key, immediately above the keys themselves. It is a note rather
    // than a field of the group because a note is wrapped to the terminal and a field is not: this
    // sentence is the disclosure notice, so it has to be readable in full at the narrowest size
    // the product supports. This is what replaces the confirmation dialogue — it is read before
    // the act, not acknowledged after it.
    let mut notes = Vec::new();
    if provider.enabled() {
        notes.push(format!(
            "e disables {}: its models leave the offered catalog and every later pool, and \
             everything measured about it stays readable",
            provider.name()
        ));
        notes.push(format!(
            "r measures {} again: it starts the engines that reach it and replaces what was \
             measured with what they report",
            provider.name()
        ));
        notes.push(
            "the credential stays where the engine keeps it · ymp records where it is read from \
             and holds no copy of it"
                .to_owned(),
        );
    } else {
        notes.push(format!(
            "e enables {} — {}",
            provider.name(),
            provider.disclosure()
        ));
        notes.push(
            "enabling is what measures the provider: until it is pressed, nothing about this \
             account is started and nothing leaves this host"
                .to_owned(),
        );
    }

    Page {
        breadcrumb: vec![
            "transcript".into(),
            "providers(all)".into(),
            provider.name().to_owned(),
        ],
        summary: Vec::new(),
        body: Body::Describe { groups },
        notes,
        footer: style::spans(&status, theme::muted()),
        keys: match provider.enabled() {
            true => vec![("e", "disable"), ("r", "refresh models"), ("Esc", "back")],
            false => vec![("e", "enable"), ("Esc", "back")],
        },
        selected: 0,
    }
}

/// What the properties view states about authentication.
///
/// The credential is named and never carried, and a state no measurement produced is never
/// claimed: this build authenticates against no provider, so an account that answered nothing is
/// said to be unmeasured rather than said to need authentication.
fn authentication(provider: &ProviderFacts) -> String {
    if !provider.enabled() {
        return "not measured — a provider is not reached before it is enabled".to_owned();
    }
    match provider
        .routes
        .iter()
        .find_map(|route| route.credential_origin.clone())
    {
        Some(origin) => format!("engine store · {origin}"),
        None => "nothing states where a credential is read from".to_owned(),
    }
}

fn models_field(provider: &ProviderFacts) -> String {
    if !provider.enabled() {
        return "— · nothing has been measured".to_owned();
    }
    let mut stated = format!("{} · {} offered", provider.models, provider.offered);
    for route in &provider.routes {
        if let Some(reason) = &route.without_models {
            stated.push_str(&format!(" · {} serves none: {reason}", route.engine.name()));
        }
    }
    stated
}

/// The engine one route is reached through, most telling fact first.
///
/// The order is what survives a narrow terminal: which engine, which release and whether it is
/// admitted are the facts a decision rests on, and the path it was found at is the one that may be
/// cut off the right of an 80-column screen.
fn reached_by(route: &RouteFacts) -> String {
    let mut stated = route.engine.name().to_owned();
    match &route.version {
        Some(version) => stated.push_str(&format!(" · {version}")),
        None => stated.push_str(" · no release measured here"),
    }
    stated.push_str(match route.admitted {
        true => " · admitted",
        false => " · not admitted",
    });
    if route.from_installed_build == Some(false) {
        stated.push_str(" · observed from another build");
    }
    if let Some(reason) = &route.withheld_reason {
        stated.push_str(&format!(" — {reason}"));
    }
    if let Some(executable) = &route.executable {
        stated.push_str(&format!(" · {executable}"));
    }
    stated
}

// ---------------------------------------------------------------------------
// `/models` — the catalog
// ---------------------------------------------------------------------------

/// The `/models` page: what could be used, never what is running.
///
/// The rows are joined from the provider records and the engine records at the moment the page is
/// read, so an engine measured again a second ago is read as it stands. Nothing here creates a
/// participant, and the page says so where it would otherwise be misread.
pub fn models_page(report: &Report, status: String) -> Page {
    let columns = vec![
        Column {
            title: "MODEL",
            width: 28,
        },
        Column {
            title: "PROVIDER",
            width: 12,
        },
        Column {
            title: "STATUS",
            width: 0,
        },
    ];

    let mut rows = Vec::new();
    for provider in &report.providers {
        for entry in report
            .entries
            .iter()
            .filter(|entry| entry.family == provider.family)
        {
            let (state, reason) = match &entry.availability {
                Availability::Admissible => ("offered", None),
                Availability::Unavailable { reason } => ("not offered", Some(reason.clone())),
            };
            rows.push(Row {
                cells: vec![
                    Cell::new(entry.model.clone(), theme::bold()),
                    Cell::new(entry.provider.clone(), theme::muted()),
                    Cell::new(
                        state,
                        match reason.is_some() {
                            true => theme::red(),
                            false => theme::green(),
                        },
                    ),
                ],
                fix: reason.map(|reason| style::spans(&format!("↳ {reason}"), theme::muted())),
                dim: false,
            });
        }
        // A route that serves nothing is a row of its own. Without it the models of an engine
        // whose record left this root would simply stop appearing, and a shorter catalog reads
        // exactly like a complete one.
        for route in &provider.routes {
            let Some(reason) = &route.without_models else {
                continue;
            };
            rows.push(Row {
                cells: vec![
                    Cell::new("—", theme::muted()),
                    Cell::new(provider.name().to_owned(), theme::muted()),
                    Cell::new(format!("no models · {}", route.engine.name()), theme::red()),
                ],
                fix: Some(style::spans(&format!("↳ {reason}"), theme::muted())),
                dim: false,
            });
        }
        if !provider.enabled() {
            rows.push(Row {
                cells: vec![
                    Cell::new("—", theme::muted()),
                    Cell::new(provider.name().to_owned(), theme::muted()),
                    Cell::new("not enabled", theme::red()),
                ],
                fix: Some(style::spans(
                    &format!(
                        "↳ {} · nothing about it has been measured — /providers is where it is \
                         enabled",
                        provider.record.display_reason()
                    ),
                    theme::muted(),
                )),
                dim: false,
            });
        }
    }

    let offered = report
        .entries
        .iter()
        .filter(|entry| entry.is_admissible())
        .count();
    let mut notes = vec![
        "nothing here is an agent · a participant exists only when one is recruited and paid for"
            .to_owned(),
    ];
    if let Some(error) = &report.catalog_error {
        notes.push(format!(
            "the catalog could not be read in full, so no part of it is stated as complete — \
             {error}"
        ));
    }
    for provider in &report.providers {
        notes.push(format!(
            "{} · observed {}",
            provider.name(),
            provider.last_refresh()
        ));
    }
    notes.push(
        "an entry is what could be used, never what is running · a provider is measured again \
         from its own properties view"
            .to_owned(),
    );

    Page {
        breadcrumb: vec![
            "transcript".into(),
            format!("models(all)[{}] · {offered} offered", report.entries.len()),
        ],
        summary: Vec::new(),
        body: Body::Table { columns, rows },
        notes,
        footer: style::spans(&status, theme::muted()),
        keys: vec![("Esc", "back")],
        selected: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;
    use ymp_runtime_registry::{ModelCatalog, ModelSource, Registry};

    fn root() -> (tempfile::TempDir, RegistryAddress) {
        let directory = tempfile::tempdir().expect("temporary root");
        let address = RegistryAddress::Root(directory.path().to_path_buf());
        (directory, address)
    }

    fn measured(address: &RegistryAddress, engine: Engine, names: &[&str]) {
        address
            .registry()
            .update(engine, |record| {
                record.enabled = true;
                record.disabled_reason = None;
                record.properties.executable = Some(format!("/usr/local/bin/{}", engine.program()));
                record.properties.version = Some("1.2.3".to_owned());
                record.properties.executable_digest = Some("engine-digest".to_owned());
                record.properties.credential_origin =
                    Some("delegated_host_keychain_credential".to_owned());
                record.models = ModelCatalog {
                    source: ModelSource::Measured,
                    measured_for_version: Some("1.2.3".to_owned()),
                    measured_for_digest: Some("engine-digest".to_owned()),
                    note: None,
                    names: names.iter().map(|name| (*name).to_owned()).collect(),
                };
            })
            .expect("record the measurement");
    }

    fn rendered(page: &Page, width: u16, height: u16) -> String {
        page.layout(width, height)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A root nothing has been enabled on shows the whole supported list as disabled, and every
    /// row says why: not because the host failed, but because nobody has enabled it.
    #[test]
    fn a_root_with_nothing_enabled_lists_the_whole_supported_list_as_disabled() {
        let (_directory, address) = root();
        let report = read(&address, UNIX_EPOCH);
        assert_eq!(report.providers.len(), ProviderFamily::ALL.len());
        assert_eq!(report.enabled_count(), 0);
        assert!(report.engines_of_enabled_providers().is_empty());
        for provider in &report.providers {
            assert_eq!(provider.record.display_state(), "disabled");
            assert_eq!(provider.models_text(), "—");
        }

        let page = providers_page(&report, "idle".into());
        let shown = rendered(&page, 120, 40);
        for family in ProviderFamily::ALL {
            assert!(shown.contains(family.name()), "{shown}");
        }
        assert!(shown.contains("disabled"), "{shown}");
        assert!(shown.contains("not enabled"), "{shown}");
    }

    /// The disclosure sentence is on the properties view of a provider that is not enabled, and it
    /// sits directly above the key that enables it.
    ///
    /// The check that must fail: state the consequence anywhere but above the key — in a note that
    /// follows it, or in a dialogue after the press — and the operator reads it too late for it to
    /// be the consent decision D4 places here.
    #[test]
    fn the_disclosure_consequence_is_stated_above_the_key_that_enables() {
        let (_directory, address) = root();
        let report = read(&address, UNIX_EPOCH);
        let provider = report
            .provider(ProviderFamily::Anthropic)
            .expect("anthropic is supported");
        let page = provider_page(provider, "idle".into());

        assert_eq!(page.keys, vec![("e", "enable"), ("Esc", "back")]);
        let consequence = page
            .notes
            .first()
            .expect("the key's consequence is the first note above it");
        assert!(
            consequence.starts_with("e enables anthropic"),
            "{consequence}"
        );
        assert!(
            consequence.contains("sends repository content"),
            "{consequence}"
        );
        assert!(
            consequence.contains("before a run starts"),
            "the pre-run disclosure is not stated: {consequence}"
        );
        let shown = rendered(&page, 120, 40);
        assert!(shown.contains("last refresh"), "{shown}");
        assert!(shown.contains("never"), "{shown}");
    }

    /// An enabled provider states what it measured, when, and what its keys would do.
    #[test]
    fn an_enabled_provider_states_its_models_and_the_age_of_the_observation() {
        let (_directory, address) = root();
        measured(&address, Engine::ClaudeCode, &["claude-opus-5"]);
        let providers = address.providers();
        providers
            .set_enabled(ProviderFamily::Anthropic, true, None)
            .expect("enable anthropic");
        let observed = UNIX_EPOCH + Duration::from_secs(1_000);
        providers
            .observe_family(ProviderFamily::Anthropic, &address.registry(), observed)
            .expect("observe anthropic");

        let report = read(&address, observed + Duration::from_secs(90));
        let provider = report
            .provider(ProviderFamily::Anthropic)
            .expect("anthropic");
        assert_eq!(provider.record.display_state(), "ready");
        assert_eq!(provider.models_text(), "1");
        assert_eq!(provider.offered, 1);
        assert_eq!(provider.last_refresh(), "1m ago");

        let page = provider_page(provider, "idle".into());
        assert_eq!(
            page.keys,
            vec![("e", "disable"), ("r", "refresh models"), ("Esc", "back")]
        );
        let shown = rendered(&page, 120, 40);
        assert!(shown.contains("1m ago"), "{shown}");
        assert!(
            shown.contains("delegated_host_keychain_credential"),
            "{shown}"
        );

        let models = models_page(&report, "idle".into());
        let shown = rendered(&models, 120, 40);
        assert!(shown.contains("claude-opus-5"), "{shown}");
        assert!(shown.contains("offered"), "{shown}");
        assert!(shown.contains("anthropic · observed 1m ago"), "{shown}");
        // The provider nobody enabled is on the page as a provider nobody enabled, so a catalog
        // holding one account's models is not read as the whole of what this build reaches.
        assert!(shown.contains("openai"), "{shown}");
        assert!(shown.contains("not enabled"), "{shown}");
    }

    /// A route whose engine record left this root serves nothing, and the page says so instead of
    /// answering with a shorter catalog.
    #[test]
    fn a_route_that_serves_nothing_is_a_row_of_its_own() {
        let (_directory, address) = root();
        measured(&address, Engine::ClaudeCode, &["claude-opus-5"]);
        let providers = address.providers();
        providers
            .set_enabled(ProviderFamily::Anthropic, true, None)
            .expect("enable anthropic");
        providers
            .observe_family(ProviderFamily::Anthropic, &address.registry(), UNIX_EPOCH)
            .expect("observe anthropic");
        std::fs::remove_file(Registry::under(address.root()).path_of(Engine::ClaudeCode))
            .expect("remove the engine record");

        let report = read(&address, UNIX_EPOCH);
        let page = models_page(&report, "idle".into());
        let shown = rendered(&page, 120, 40);
        assert!(!shown.contains("claude-opus-5"), "{shown}");
        assert!(shown.contains("no models · claude-code"), "{shown}");
        assert!(
            shown.contains("no record of the claude-code engine"),
            "{shown}"
        );
    }

    #[test]
    fn an_age_is_stated_in_whole_units() {
        assert_eq!(age_text(Duration::from_secs(0)), "0s ago");
        assert_eq!(age_text(Duration::from_secs(59)), "59s ago");
        assert_eq!(age_text(Duration::from_secs(60)), "1m ago");
        assert_eq!(age_text(Duration::from_secs(3_600)), "1h 0m ago");
        assert_eq!(age_text(Duration::from_secs(90_000)), "1d ago");
    }
}

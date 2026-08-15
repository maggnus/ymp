//! The agent pool level as the operator meets it: which of the catalog's entries a run may
//! recruit from, and the ceilings it is held to.
//!
//! Two surfaces, and one sentence holds both of them together: **a pool is what the collective may
//! use, never who works** (`ymp-docs/design/COLLECTIVE-RESOURCES.md`, *AgentPool*). It names
//! entries; it names no participant, no count and no role, and nothing on either surface creates
//! one. Reading them starts no process and reaches no network — every value comes from the records
//! under the product root — and neither of them ranks anything: the order the entries stand in is
//! the order the pool declares them in, which is the catalog's for a pool that follows it and the
//! operator's for one they have edited.
//!
//! * `/pools` lists the pools this root holds. On a root where nothing has been measured it holds
//!   none and says why, because the `default` pool is created when the first provider is observed
//!   with a model an admitted engine can serve — never by the operator
//!   (`ymp-docs/design/COLLECTIVE-TUI.md`, S35);
//! * a pool's properties state what it permits, what it resolved to, the ceilings it carries and
//!   the digest a run would freeze, and every one of those is a row the keys act on (S36).
//!
//! **Editing is a key on the selected row and nothing is confirmed.** Permitting an entry or
//! taking one out is `Enter` on that entry's row, so no model name is ever typed; raising or
//! lowering a ceiling is `+` and `-` on the ceiling's own row. What each key would do is stated
//! above it on the same screen, which is what rule 1 of the operator surface puts in place of a
//! confirmation dialogue. The first edit of a pool that follows the catalog replaces the
//! whole-catalog form with the list the operator leaves, and the surface says so before the key is
//! pressed rather than afterwards.
//!
//! The rows are built once, by [`rows`], and both the page and the key handling read them from
//! there. A key therefore acts on exactly what is drawn: a surface that composed its rows in one
//! place and resolved a key press in another would be able to act on a row nobody could see.

use ymp_runtime_registry::{
    Availability, Catalog, CatalogEntry, PoolRecord, PoolStateKind, RegistryAddress, ResolvedEntry,
};

/// The pool vocabulary the command surface states its own arguments in.
///
/// It is re-exported here for the reason the provider level re-exports its own: the commands
/// perform the interface's actions through the interface's session and reach no durable state of
/// their own, so they name this level here rather than naming the registry crate
/// (`crates/ymp-cli/tests/one_command_path.rs`).
pub use ymp_runtime_registry::{PoolCapacity, PoolEntry, PoolName};

use crate::pages::{Body, Cell, Column, Page, Row};
use crate::style;
use crate::theme;

/// One pool as this root holds it.
///
/// It carries the record and nothing derived: what a pool permits, what it resolved to and which
/// states it is in are all answered from the record, so a surface cannot state a fact about a pool
/// that the record does not hold.
#[derive(Clone, Debug)]
pub struct PoolFacts {
    pub record: PoolRecord,
}

impl PoolFacts {
    pub fn name(&self) -> &str {
        &self.record.pool
    }

    /// The pool this row selects, which is how every act names its subject: from the row, never
    /// from something the operator typed.
    pub fn pool(&self) -> PoolName {
        PoolName::parse(&self.record.pool).unwrap_or_else(|_| PoolName::default_pool())
    }

    /// Whether this pool still follows the catalog.
    pub fn tracking(&self) -> bool {
        self.record.resolved.tracking
    }

    /// What the pool permits, in the words a row states it.
    pub fn models_text(&self) -> String {
        let entries = self.record.resolved.entries.len();
        match self.tracking() {
            true => format!("all admissible · {}", counted(entries)),
            false => format!("{} · the list left here", counted(entries)),
        }
    }

    /// The ceilings a run using this pool is held to. Both are ceilings and neither is a target:
    /// nothing creates a participant to reach either of them.
    pub fn capacity_text(&self) -> String {
        let capacity = self.record.declared.capacity;
        format!(
            "up to {} · {} at once",
            capacity.max_agents, capacity.max_concurrent_attempts
        )
    }

    /// The state the pool is in, out of the three a resolution can leave it in.
    ///
    /// `resolved`, `tracking` and `explicit` are held by every resolution and say what the pool is
    /// rather than how it stands, so the cell states the one that is about its entries.
    pub fn state(&self) -> PoolStateKind {
        for state in [
            PoolStateKind::Ready,
            PoolStateKind::Degraded,
            PoolStateKind::Empty,
        ] {
            if self.record.resolved.holds(state) {
                return state;
            }
        }
        PoolStateKind::Resolved
    }

    /// The sentence that state carries, in the words the record wrote it in.
    pub fn state_reason(&self) -> String {
        self.record
            .resolved
            .reason_for(self.state())
            .unwrap_or("this pool has not been resolved against a catalog reading")
            .to_owned()
    }

    /// How many of the entries this pool permits are offered now.
    pub fn admissible(&self) -> usize {
        self.record.resolved.admissible
    }
}

/// One reading of the pool level: the pools, and the catalog they resolve against.
///
/// The catalog is held beside the pools because the properties view offers every entry this host
/// could serve, not only the ones a pool already permits: an entry is added by permitting the row
/// that names it, so the row has to exist before it is permitted.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub pools: Vec<PoolFacts>,
    /// The catalog entries, in provider order then registry order.
    pub entries: Vec<CatalogEntry>,
    /// Why the pools could not be read, when they could not. A pool that exists and cannot be read
    /// is stated rather than left out: a shorter list reads exactly like a root holding fewer
    /// pools.
    pub error: Option<String>,
    /// Why the catalog could not be read, when it could not.
    pub catalog_error: Option<String>,
}

impl Report {
    pub fn pool_at(&self, index: usize) -> Option<&PoolFacts> {
        self.pools.get(index)
    }

    pub fn named(&self, name: &PoolName) -> Option<&PoolFacts> {
        self.pools
            .iter()
            .find(|pool| pool.record.pool == name.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.pools.is_empty()
    }

    /// How many pools are ready, which is what the table's own count states.
    pub fn ready_count(&self) -> usize {
        self.pools
            .iter()
            .filter(|pool| pool.state() == PoolStateKind::Ready)
            .count()
    }
}

/// Read the pool level of one root. Opens no process, reaches no network, writes nothing.
///
/// Resolving a pool is the controller's act and is taken where an observation is taken; this reads
/// what that controller last wrote, so opening either surface states the records as they stand
/// rather than resolving them again behind the operator.
pub fn read(address: &RegistryAddress) -> Report {
    let providers = address.providers();
    let registry = address.registry();
    let catalog = Catalog::read(&providers, &registry);
    let catalog_error = catalog.as_ref().err().map(ToString::to_string);
    let catalog = catalog.unwrap_or_default();

    let (pools, error) = match address.pools().read_all() {
        Ok(records) => (
            records
                .into_iter()
                .map(|record| PoolFacts { record })
                .collect(),
            None,
        ),
        Err(error) => (Vec::new(), Some(error.to_string())),
    };

    Report {
        pools,
        entries: catalog.entries().to_vec(),
        error,
        catalog_error,
    }
}

// ---------------------------------------------------------------------------
// The rows of a pool's properties view
// ---------------------------------------------------------------------------

/// Which ceiling a row states. Both are ceilings; neither is a target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ceiling {
    Participants,
    ConcurrentAttempts,
}

impl Ceiling {
    /// The value this ceiling holds in one declaration.
    pub fn of(self, capacity: PoolCapacity) -> u32 {
        match self {
            Self::Participants => capacity.max_agents,
            Self::ConcurrentAttempts => capacity.max_concurrent_attempts,
        }
    }

    /// The ceiling as its row states it.
    pub fn stated(self, capacity: PoolCapacity) -> String {
        match self {
            Self::Participants => format!("up to {} participants", self.of(capacity)),
            Self::ConcurrentAttempts => format!("up to {} attempts at once", self.of(capacity)),
        }
    }

    /// The declaration this ceiling raised or lowered by one leaves behind.
    ///
    /// A ceiling never goes below one: a pool bounded at zero would permit entries no run could
    /// use, which is a state nothing in the product asks for and nothing would restore.
    pub fn stepped(self, capacity: PoolCapacity, up: bool) -> PoolCapacity {
        let held = self.of(capacity);
        let moved = match up {
            true => held.saturating_add(1),
            false => held.saturating_sub(1).max(1),
        };
        match self {
            Self::Participants => PoolCapacity {
                max_agents: moved,
                ..capacity
            },
            Self::ConcurrentAttempts => PoolCapacity {
                max_concurrent_attempts: moved,
                ..capacity
            },
        }
    }
}

/// One entry as a row of the properties view states it: what it is, and where it stands with
/// respect to this pool and to the catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryRow {
    pub entry: PoolEntry,
    /// Whether this pool permits it.
    pub permitted: bool,
    /// Whether the catalog offers it.
    pub admissible: bool,
    /// Why the catalog does not offer it, in the words the record that holds it states.
    pub reason: Option<String>,
}

impl EntryRow {
    /// The triple, as one cell reads it.
    pub fn named(&self) -> String {
        format!(
            "{} · {} · {}",
            self.entry.provider, self.entry.engine, self.entry.model
        )
    }

    /// Where this entry stands, in the words the row states.
    pub fn state_text(&self) -> String {
        match (self.permitted, self.admissible) {
            (true, true) => "permitted · offered".to_owned(),
            (true, false) => "permitted · not offered".to_owned(),
            (false, true) => "not permitted".to_owned(),
            (false, false) => "not permitted · not offered".to_owned(),
        }
    }
}

/// One row of a pool's properties view: the thing a key press acts on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PoolRow {
    /// What the pool permits, stated as a whole.
    Models,
    /// One entry, permitted by this pool or not.
    Entry(EntryRow),
    /// One ceiling.
    Capacity(Ceiling),
    /// The per-participant bounds, each absent until an operator states one.
    Limits,
    /// The digest of the resolved ordered set, which is the value a run freezes.
    Digest,
    /// What the resolution was read from.
    Observation,
}

/// The rows of one pool's properties view, in the order S36 states them.
///
/// The entries come first in the pool's declared order, and every catalog entry the pool does not
/// permit follows them: an entry is permitted by pressing a key on its row, so a row has to exist
/// for every entry that could be permitted. An entry an explicit list names that the catalog no
/// longer holds keeps its row too — a pool that grew shorter would answer *why is this model not
/// offered* with silence.
pub fn rows(pool: &PoolFacts, entries: &[CatalogEntry]) -> Vec<PoolRow> {
    let mut rows = vec![PoolRow::Models];
    for resolved in &pool.record.resolved.entries {
        rows.push(PoolRow::Entry(EntryRow {
            entry: entry_of(resolved),
            permitted: true,
            admissible: resolved.admissible,
            reason: resolved.reason.clone(),
        }));
    }
    for entry in entries {
        let named = PoolEntry::of(entry);
        if pool
            .record
            .resolved
            .entries
            .iter()
            .any(|resolved| entry_of(resolved) == named)
        {
            continue;
        }
        rows.push(PoolRow::Entry(EntryRow {
            entry: named,
            permitted: false,
            admissible: entry.is_admissible(),
            reason: match &entry.availability {
                Availability::Admissible => None,
                Availability::Unavailable { reason } => Some(reason.clone()),
            },
        }));
    }
    rows.push(PoolRow::Capacity(Ceiling::Participants));
    rows.push(PoolRow::Capacity(Ceiling::ConcurrentAttempts));
    rows.push(PoolRow::Limits);
    rows.push(PoolRow::Digest);
    rows.push(PoolRow::Observation);
    rows
}

/// The triple one resolved entry names.
fn entry_of(resolved: &ResolvedEntry) -> PoolEntry {
    PoolEntry {
        provider: resolved.provider.clone(),
        engine: resolved.engine.clone(),
        model: resolved.model.clone(),
    }
}

/// The explicit ordered list one toggle leaves behind.
///
/// It is computed from the record rather than from the screen, and it keeps the declared order:
/// permitting an entry adds it at the end, because the order is what later decides which entry a
/// run ignites on ([decision D2](../../../../ymp-docs/design/COLLECTIVE-OWNER-DECISIONS.md)) and
/// an entry inserted into the middle of the list would move that decision without the operator
/// having asked for it.
pub fn permitted_after(record: &PoolRecord, entry: &PoolEntry, permitted: bool) -> Vec<PoolEntry> {
    let mut list: Vec<PoolEntry> = record
        .resolved
        .entries
        .iter()
        .map(entry_of)
        .filter(|held| held != entry)
        .collect();
    if permitted {
        list.push(entry.clone());
    }
    list
}

/// The pool one stated name selects, refused rather than resolved to the nearest one.
///
/// A command names the pool it acts on, exactly as it names the provider a properties view states;
/// the interface never asks for a name, because a pool is selected by standing on its row.
pub fn pool_named(report: &Report, stated: &str) -> Result<PoolName, String> {
    let name = PoolName::parse(stated).map_err(|error| error.to_string())?;
    match report.named(&name) {
        Some(_) => Ok(name),
        None => Err(format!(
            "nothing was changed — this root holds no pool named {name}{}",
            match report.pools.is_empty() {
                true => "; it holds no pool at all, and one is created when the first provider is \
                         measured with a model an admitted engine serves"
                    .to_owned(),
                false => format!(
                    ", and it holds {}",
                    report
                        .pools
                        .iter()
                        .map(PoolFacts::name)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        )),
    }
}

/// The entry one stated model selects among the rows of a pool's properties view.
///
/// A model named by more than one entry is refused with the entries named rather than resolved to
/// one of them: which account and which engine serve a model is the difference between two
/// capabilities, and choosing for the operator would permit a capability they did not name. The
/// interface reaches the same entry by standing on its row.
pub fn entry_named(
    report: &Report,
    pool: &PoolName,
    model: &str,
    provider: Option<&str>,
    engine: Option<&str>,
) -> Result<PoolEntry, String> {
    let Some(facts) = report.named(pool) else {
        return Err(format!("this root holds no pool named {pool}"));
    };
    let matched: Vec<PoolEntry> = rows(facts, &report.entries)
        .into_iter()
        .filter_map(|row| match row {
            PoolRow::Entry(entry) => Some(entry.entry),
            _ => None,
        })
        .filter(|entry| {
            entry.model == model
                && provider.is_none_or(|stated| entry.provider == stated)
                && engine.is_none_or(|stated| entry.engine == stated)
        })
        .collect();
    match matched.len() {
        1 => Ok(matched.into_iter().next().expect("one entry")),
        0 => Err(format!(
            "nothing was changed — no entry of the {pool} pool is the {model} model{}",
            match report.entries.is_empty() {
                true => "; this root's catalog holds no entry at all".to_owned(),
                false => format!(
                    "; the catalog holds {}",
                    report
                        .entries
                        .iter()
                        .map(|entry| entry.model.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        )),
        _ => Err(format!(
            "nothing was changed — {} entries are the {model} model, and which account and engine \
             serve it is the difference between them: {} · name one with --provider and --engine",
            matched.len(),
            matched
                .iter()
                .map(|entry| format!("{} · {}", entry.provider, entry.engine))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

// ---------------------------------------------------------------------------
// `/pools` — the pools this root holds
// ---------------------------------------------------------------------------

/// The `/pools` page: one row per pool, and what each one permits.
pub fn pools_page(report: &Report, status: String) -> Page {
    let columns = vec![
        Column {
            title: "NAME",
            width: 12,
        },
        Column {
            title: "MODELS",
            width: 28,
        },
        Column {
            title: "CAPACITY",
            width: 20,
        },
        Column {
            title: "TRACKING",
            width: 10,
        },
        Column {
            title: "STATE",
            width: 0,
        },
    ];

    let rows = report
        .pools
        .iter()
        .map(|pool| Row {
            cells: vec![
                Cell::new(pool.name().to_owned(), theme::bold()),
                Cell::new(pool.models_text(), theme::muted()),
                Cell::new(pool.capacity_text(), theme::muted()),
                Cell::new(
                    match pool.tracking() {
                        true => "yes",
                        false => "no",
                    },
                    theme::muted(),
                ),
                Cell::new(
                    pool.state().label(),
                    match pool.state() {
                        PoolStateKind::Ready => theme::green(),
                        _ => theme::red(),
                    },
                ),
            ],
            fix: (pool.state() != PoolStateKind::Ready)
                .then(|| style::spans(&format!("↳ {}", pool.state_reason()), theme::muted())),
            dim: false,
        })
        .collect::<Vec<_>>();

    let mut notes = vec![
        "a pool is what the collective may use, not who works · it assigns no role and creates no \
         participant"
            .to_owned(),
    ];
    if let Some(error) = &report.error {
        notes.push(format!(
            "the pools could not be read in full, so this list is not stated as complete — {error}"
        ));
    }
    if report.pools.is_empty() {
        notes.push(
            "no pool stands under this root · the default pool is created when a provider is \
             measured with a model an admitted engine serves — /providers is where one is enabled"
                .to_owned(),
        );
    } else {
        notes.push(
            "the default pool exists because a provider was measured · it is never created by \
             hand and is never removed while a provider is ready"
                .to_owned(),
        );
        notes.push(
            "TRACKING says whether a pool still follows the catalog: an edited pool holds the list \
             it was edited to, and a model measured later joins the catalog and not that pool"
                .to_owned(),
        );
        notes.push(
            "Enter opens a pool, where each key states above itself what it would do".to_owned(),
        );
    }

    Page {
        breadcrumb: vec![
            "transcript".into(),
            format!(
                "pools(all)[{}] · {} ready",
                report.pools.len(),
                report.ready_count()
            ),
        ],
        summary: Vec::new(),
        body: Body::Table { columns, rows },
        notes,
        footer: style::spans(&status, theme::muted()),
        keys: vec![("Enter", "properties"), ("Esc", "back")],
        selected: 0,
    }
}

// ---------------------------------------------------------------------------
// One pool's properties
// ---------------------------------------------------------------------------

/// The properties of one pool, with the acts that can be taken on its rows.
///
/// Every row is addressed by standing on it, and what each key would do to the row under the
/// cursor is stated in the notes above the keys. That is the whole reason the entries are rows
/// rather than a sentence: permitting one is a key press on the row that names it, so no model
/// name is ever typed and nothing is confirmed afterwards.
pub fn pool_page(pool: &PoolFacts, entries: &[CatalogEntry], status: String) -> Page {
    let columns = vec![
        Column {
            title: "FIELD",
            width: 12,
        },
        Column {
            title: "VALUE",
            width: 44,
        },
        Column {
            title: "STATE",
            width: 0,
        },
    ];

    let capacity = pool.record.declared.capacity;
    let rows = rows(pool, entries)
        .into_iter()
        .map(|row| match row {
            PoolRow::Models => Row {
                cells: vec![
                    Cell::new("models", theme::bold()),
                    Cell::new(models_field(pool), theme::text()),
                    Cell::new(
                        match pool.tracking() {
                            true => "tracking",
                            false => "explicit",
                        },
                        theme::muted(),
                    ),
                ],
                fix: None,
                dim: false,
            },
            PoolRow::Entry(entry) => Row {
                cells: vec![
                    Cell::new("entry", theme::muted()),
                    Cell::new(
                        entry.named(),
                        match entry.permitted {
                            true => theme::text(),
                            false => theme::muted(),
                        },
                    ),
                    Cell::new(
                        entry.state_text(),
                        match (entry.permitted, entry.admissible) {
                            (true, true) => theme::green(),
                            (true, false) => theme::red(),
                            _ => theme::muted(),
                        },
                    ),
                ],
                fix: entry
                    .reason
                    .map(|reason| style::spans(&format!("↳ {reason}"), theme::muted())),
                dim: false,
            },
            PoolRow::Capacity(ceiling) => Row {
                cells: vec![
                    Cell::new("capacity", theme::muted()),
                    Cell::new(ceiling.stated(capacity), theme::text()),
                    Cell::new("ceiling", theme::muted()),
                ],
                fix: None,
                dim: false,
            },
            PoolRow::Limits => Row {
                cells: vec![
                    Cell::new("limits", theme::muted()),
                    Cell::new(limits_field(pool), theme::text()),
                    Cell::new("per participant", theme::muted()),
                ],
                fix: None,
                dim: false,
            },
            PoolRow::Digest => Row {
                cells: vec![
                    Cell::new("digest", theme::muted()),
                    Cell::new(
                        crate::projection::short_digest(&pool.record.resolved.digest),
                        theme::text(),
                    ),
                    Cell::new("what a run freezes", theme::muted()),
                ],
                fix: None,
                dim: false,
            },
            PoolRow::Observation => Row {
                cells: vec![
                    Cell::new("read from", theme::muted()),
                    Cell::new(observation_field(pool), theme::text()),
                    Cell::new("observation", theme::muted()),
                ],
                fix: None,
                dim: false,
            },
        })
        .collect::<Vec<_>>();

    // The consequence of each key, immediately above the keys themselves. The first edit of a pool
    // that follows the catalog is a decision and not an addition, so what it replaces is stated
    // before the key is pressed — this is what stands in place of a confirmation.
    let mut notes = vec![match pool.tracking() {
        true => "Enter on an entry permits it or takes it out · this pool follows the catalog, so \
                 the first edit replaces `all admissible` with the list you leave and the pool \
                 stops following it: a model measured later joins the catalog and not this pool"
            .to_owned(),
        false => "Enter on an entry permits it or takes it out · this pool holds the list it was \
                  edited to and does not follow the catalog: a model measured later joins the \
                  catalog and not this pool"
            .to_owned(),
    }];
    notes.push(
        "+ and - raise and lower the ceiling on the selected row by one · a ceiling is never a \
         target and nothing creates a participant to reach it"
            .to_owned(),
    );
    notes.push(format!(
        "{} · {}",
        pool.state().label(),
        pool.state_reason()
    ));
    notes.push(
        "editing a pool disturbs no run that is already working: a run holds the snapshot it \
         froze, not this pool"
            .to_owned(),
    );

    Page {
        breadcrumb: vec![
            "transcript".into(),
            "pools(all)".into(),
            format!(
                "{} · {} of {} offered",
                pool.name(),
                pool.admissible(),
                pool.record.resolved.entries.len()
            ),
        ],
        summary: Vec::new(),
        body: Body::Table { columns, rows },
        notes,
        footer: style::spans(&status, theme::muted()),
        keys: vec![
            ("Enter", "permit / take out"),
            ("+", "raise"),
            ("-", "lower"),
            ("Esc", "back"),
        ],
        selected: 0,
    }
}

/// What the pool permits, stated as a whole.
fn models_field(pool: &PoolFacts) -> String {
    match pool.tracking() {
        true => "all admissible entries · tracking the catalog".to_owned(),
        false => format!(
            "{} · the explicit list this pool was edited to",
            counted(pool.record.resolved.entries.len())
        ),
    }
}

/// A count of entries, in a phrase that reads for one as well as for several.
fn counted(entries: usize) -> String {
    match entries {
        1 => "1 entry".to_owned(),
        _ => format!("{entries} entries"),
    }
}

/// The per-participant bounds. Every one of them is absent until an operator states one, so a pool
/// that carries none says that rather than showing a number nobody decided.
fn limits_field(pool: &PoolFacts) -> String {
    let limits = pool.record.declared.resource_limits;
    let mut stated = Vec::new();
    if let Some(money) = limits.max_money_micros {
        stated.push(format!("${:.2}", money as f64 / 1_000_000.0));
    }
    if let Some(wall) = limits.max_wall_time_ms {
        stated.push(format!("{} min", wall / 60_000));
    }
    if let Some(starts) = limits.max_attempt_starts {
        stated.push(format!("{starts} attempt starts"));
    }
    if let Some(queries) = limits.max_verification_queries {
        stated.push(format!("{queries} verification queries"));
    }
    match stated.is_empty() {
        true => "none stated".to_owned(),
        false => stated.join(" · "),
    }
}

/// What the resolution was read from.
///
/// It stands where a resolution time would, and it is what this level can actually name: nothing
/// that writes a pool record reads a clock, so the record states the provider records it was read
/// through and the digest of the catalog reading it resolved against.
fn observation_field(pool: &PoolFacts) -> String {
    let observation = &pool.record.resolved.observation;
    let providers = match observation.providers.is_empty() {
        true => "no provider record".to_owned(),
        false => observation
            .providers
            .iter()
            .map(|provider| format!("{} {}", provider.provider, provider.state.label()))
            .collect::<Vec<_>>()
            .join(" · "),
    };
    format!(
        "catalog {} · {providers}",
        crate::projection::short_digest(&observation.catalog_digest)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ymp_runtime_registry::{
        Engine, ModelCatalog, ModelSource, PoolModels, Pools, ProviderFamily, Registry,
    };

    fn root() -> (tempfile::TempDir, RegistryAddress) {
        let directory = tempfile::tempdir().expect("temporary root");
        let address = RegistryAddress::Root(directory.path().to_path_buf());
        (directory, address)
    }

    /// A root with one account enabled, one engine measured, and the pools resolved against it —
    /// which is the state the first observation of a provider leaves behind.
    fn measured(address: &RegistryAddress, names: &[&str]) {
        address
            .registry()
            .update(Engine::ClaudeCode, |record| {
                record.enabled = true;
                record.disabled_reason = None;
                record.properties.executable = Some("/usr/local/bin/claude".to_owned());
                record.properties.version = Some("2.1.227".to_owned());
                record.properties.executable_digest = Some("engine-digest".to_owned());
                record.properties.credential_origin =
                    Some("delegated_host_keychain_credential".to_owned());
                record.models = ModelCatalog {
                    source: ModelSource::Measured,
                    measured_for_version: Some("2.1.227".to_owned()),
                    measured_for_digest: Some("engine-digest".to_owned()),
                    note: None,
                    names: names.iter().map(|name| (*name).to_owned()).collect(),
                };
            })
            .expect("record the measurement");
        let providers = address.providers();
        providers
            .set_enabled(ProviderFamily::Anthropic, true, None)
            .expect("enable anthropic");
        providers
            .observe_family(
                ProviderFamily::Anthropic,
                &Registry::under(address.root()),
                std::time::UNIX_EPOCH,
            )
            .expect("observe anthropic");
        address
            .pools()
            .reconcile(&providers, &address.registry())
            .expect("resolve the pools");
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

    /// A root nothing has been measured on holds no pool, and the table says why rather than
    /// standing empty.
    #[test]
    fn a_root_that_has_measured_nothing_holds_no_pool_and_names_what_creates_one() {
        let (_directory, address) = root();
        let report = read(&address);
        assert!(report.is_empty());
        let page = pools_page(&report, "idle".into());
        assert!(
            page.breadcrumb
                .last()
                .is_some_and(|last| last.contains("pools(all)[0]")),
            "{:?}",
            page.breadcrumb
        );
        let shown = rendered(&page, 120, 40);
        assert!(shown.contains("no pool stands under this root"), "{shown}");
        assert!(shown.contains("/providers"), "{shown}");
    }

    /// A measured root holds the pool the controller created, tracking the catalog.
    #[test]
    fn a_measured_root_holds_the_default_pool_tracking_the_catalog() {
        let (_directory, address) = root();
        measured(&address, &["claude-opus-5", "claude-sonnet-5"]);
        let report = read(&address);
        let pool = report.pool_at(0).expect("the default pool");
        assert_eq!(pool.name(), "default");
        assert!(pool.tracking());
        assert_eq!(pool.admissible(), 2);
        assert_eq!(pool.state(), PoolStateKind::Ready);
        assert_eq!(pool.models_text(), "all admissible · 2 entries");
        assert_eq!(pool.capacity_text(), "up to 6 · 3 at once");

        let shown = rendered(&pools_page(&report, "idle".into()), 120, 40);
        assert!(shown.contains("default"), "{shown}");
        assert!(shown.contains("all admissible · 2 entries"), "{shown}");
        assert!(shown.contains("ready"), "{shown}");
        assert!(shown.contains("not who works"), "{shown}");
    }

    /// The properties view holds one row per entry, the ceilings and the digest, and states above
    /// its keys what the first edit of a tracking pool would replace.
    #[test]
    fn the_properties_view_holds_a_row_for_every_entry_and_states_what_an_edit_replaces() {
        let (_directory, address) = root();
        measured(&address, &["claude-opus-5", "claude-sonnet-5"]);
        let report = read(&address);
        let pool = report.pool_at(0).expect("the default pool");
        let rows = rows(pool, &report.entries);
        assert_eq!(
            rows.iter()
                .filter(|row| matches!(row, PoolRow::Entry(_)))
                .count(),
            2
        );
        assert!(rows.contains(&PoolRow::Capacity(Ceiling::Participants)));
        assert!(rows.contains(&PoolRow::Digest));

        let page = pool_page(pool, &report.entries, "idle".into());
        let shown = rendered(&page, 120, 40);
        assert!(shown.contains("claude-opus-5"), "{shown}");
        assert!(shown.contains("permitted · offered"), "{shown}");
        assert!(shown.contains("up to 6 participants"), "{shown}");
        assert!(shown.contains("what a run freezes"), "{shown}");
        assert!(
            page.notes[0].contains("stops following it"),
            "{:?}",
            page.notes
        );
    }

    /// Taking an entry out leaves the list the operator would be left with, in declared order, and
    /// permitting it again puts it back at the end.
    #[test]
    fn a_toggle_leaves_the_explicit_list_in_declared_order() {
        let (_directory, address) = root();
        measured(&address, &["claude-opus-5", "claude-sonnet-5"]);
        let report = read(&address);
        let pool = report.pool_at(0).expect("the default pool");
        let first = PoolEntry {
            provider: "anthropic".to_owned(),
            engine: "claude-code".to_owned(),
            model: "claude-opus-5".to_owned(),
        };

        let without = permitted_after(&pool.record, &first, false);
        assert_eq!(without.len(), 1);
        assert_eq!(without[0].model, "claude-sonnet-5");

        address
            .pools()
            .edit(
                &pool.pool(),
                &address.providers(),
                &address.registry(),
                |declared| declared.models = PoolModels::explicit(without),
            )
            .expect("edit the pool");
        let report = read(&address);
        let pool = report.pool_at(0).expect("the default pool");
        assert!(!pool.tracking());
        assert_eq!(pool.record.resolved.entries.len(), 1);

        let again = permitted_after(&pool.record, &first, true);
        assert_eq!(again.len(), 2);
        assert_eq!(
            again[1], first,
            "a permitted entry joined the list somewhere other than the end"
        );
    }

    /// A model no entry names is refused with what the catalog holds, and one two entries name is
    /// refused with both named.
    #[test]
    fn a_model_that_selects_no_entry_is_refused_rather_than_resolved() {
        let (_directory, address) = root();
        measured(&address, &["claude-opus-5"]);
        let report = read(&address);
        let pool = PoolName::default_pool();
        assert_eq!(
            entry_named(&report, &pool, "claude-opus-5", None, None)
                .expect("the entry")
                .model,
            "claude-opus-5"
        );
        let refused = entry_named(&report, &pool, "gpt-5.6-sol", None, None)
            .expect_err("a model no entry names");
        assert!(refused.contains("claude-opus-5"), "{refused}");

        let unknown = pool_named(&report, "cheap").expect_err("a pool this root does not hold");
        assert!(unknown.contains("default"), "{unknown}");
    }

    /// Pools this root cannot read are stated rather than left out, because a shorter list reads
    /// exactly like a root that holds fewer pools.
    #[test]
    fn an_unreadable_pool_is_stated_and_not_left_out() {
        let (_directory, address) = root();
        measured(&address, &["claude-opus-5"]);
        std::fs::write(
            Pools::under(address.root()).path_of(&PoolName::default_pool()),
            b"{ not a record",
        )
        .expect("damage the record");

        let report = read(&address);
        assert!(report.is_empty());
        let error = report.error.clone().expect("the read failure is stated");
        assert!(error.contains("not readable"), "{error}");
        let shown = rendered(&pools_page(&report, "idle".into()), 120, 40);
        assert!(shown.contains("could not be read in full"), "{shown}");
    }
}

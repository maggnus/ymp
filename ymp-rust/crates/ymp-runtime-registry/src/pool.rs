//! The agent pool level of the product root: which of the catalog's capabilities a run may create
//! participants from, and the mechanical ceilings it may do so within.
//!
//! An agent pool is a **capability boundary and never a team**
//! (`ymp-docs/design/COLLECTIVE-RESOURCES.md`, AgentPool). It names entries; it does not name
//! participants, counts or roles, and a pool of a hundred entries produces zero participants. One
//! record per pool stands beside the provider and engine records:
//!
//! ```text
//! .ymp/
//!   pools/default.json              what the operator declared, and what it resolves to
//!   providers/anthropic.json        the accounts the entries are attributed to
//!   runtimes/claude-code.json       the engine records the model names are measured into
//! ```
//!
//! **The record has two halves with two owners.** The **declaration** is the operator's: which
//! entries the pool permits and the ceilings a run using it is held to. The **resolution** is the
//! controller's: what that declaration resolves to against the catalog as it stands now, the digest
//! of the resolved ordered set, and the states the pool is in, each with the sentence that explains
//! it. Neither writer can reach the other's half — [`Pools::write_declaration`] carries the stored
//! resolution forward untouched, [`Pools::reconcile`] carries the stored declaration forward
//! untouched, and [`resolve`] takes the declaration by immutable borrow, so it cannot write one.
//!
//! The two halves are the product's own words for its own record, and they are not the Kubernetes
//! `spec` and `status` spelled in this crate (owner decision D12). The correspondence exists and is
//! stated once, in `ymp-rust/SCHEMA.md`, because mapping a record onto a CRD is a concern of a
//! future control plane rather than of this domain.
//!
//! **The `default` pool is created rather than seeded.** The controller creates one pool named
//! `default`, tracking the catalog, the first time the catalog holds an admissible entry — which is
//! the first time a provider is observed ready with a model an admitted engine can serve. A root
//! that has observed nothing therefore holds no pool at all, so the product's first-run state is
//! *enable a provider* rather than *no pool is configured*
//! (`ymp-docs/design/COLLECTIVE-DESIGN.md`, §7). Creating that one record is the only declaration
//! the controller ever writes.
//!
//! **Editing a pool stops it tracking, and the record says so.** An explicit list is the operator's
//! statement of which entries this pool permits, so the whole-catalog form is replaced rather than
//! added to, and a model discovered afterwards does not rejoin the pool. `resolved.tracking` and
//! the `explicit` state say so on the record, because a tracking pool that ignored an edit and an
//! edited pool that silently followed the catalog are both dishonest. An edit resolves before it
//! writes and writes both halves at once, so a record never stands with a list in one half and the
//! sentence *this pool follows the catalog* in the other.
//!
//! **The controller is mechanical in the strict sense.** Its inputs are a declaration and a catalog
//! reading. It reads no goal, no task and no run; it ranks nothing, because the resolved order is
//! the declared order — the catalog's for a tracking pool, the operator's for an edited one — and
//! it starts nothing. That is the whole of what the controllers table permits the pool reconciler
//! (`ymp-docs/design/COLLECTIVE-RESOURCES.md`, *Controllers, in full*), and the entry a run ignites
//! on is decided later, at the freeze, by position and measured readiness alone (decision D2).
//!
//! Four things are deliberately absent from the record, and each absence is a decision rather than
//! an omission.
//!
//! * **No floor, no desired count, no replica count.** `capacity` carries a ceiling on participants
//!   and a ceiling on concurrent attempts and nothing else. A floor would be the only field in the
//!   resource model with no controller behind it: either recorded and ignored, or satisfied by
//!   something creating participants nobody asked for (decision D11).
//! * **No role and no rank.** An entry is a capability, and a list of capabilities with a rank on it
//!   would be a team assignment under another name.
//! * **No disclosure class, assurance profile or external-action allowance.** They are ceilings of a
//!   run's policy, enforced where a run is created and a recruitment is admitted; nothing in this
//!   build reads them, and a field recorded here now would be recorded and ignored.
//! * **No resolution timestamp.** Nothing that writes this record reads a clock, so a time on it
//!   would be a claim nothing measured. What the resolution was taken from is stated instead, as
//!   the observation this level can actually name: the provider records it was read through and the
//!   digest of the catalog reading it resolved against. The question a timestamp would be consulted
//!   for — is this the resolution the catalog would produce now — is answered by comparing that
//!   digest with the catalog, which is a fact rather than an inference from an age.
//!
//! Nothing here instantiates a participant, spends anything or opens a process. Resolving a pool
//! reads records under one root and writes one record back.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    Availability, Catalog, CatalogEntry, ProviderError, ProviderState, Providers, Registry,
};

/// The directory the pool records stand in, under the product root.
pub const POOLS_DIRECTORY: &str = "pools";

/// The record layout this build writes and reads. A record is not migrated.
pub const POOL_SCHEMA_VERSION: u32 = 1;

/// The name of the pool the controller creates, and the only pool this build holds.
pub const DEFAULT_POOL: &str = "default";

/// The name of one pool, which is also the file the record stands in.
///
/// Spelling is exact and the character set is the one a file name can carry without a path in it,
/// because a name that reached the filesystem as a path would address a record outside the pools
/// directory.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PoolName(String);

impl PoolName {
    /// The pool the controller creates on the first admissible entry.
    pub fn default_pool() -> Self {
        Self(DEFAULT_POOL.to_owned())
    }

    /// The pool a stated name selects. A name carrying anything but lower-case letters, digits,
    /// `-` and `_` is refused rather than reduced to one that is.
    pub fn parse(value: &str) -> Result<Self, PoolError> {
        let name = value.trim();
        let acceptable = |character: char| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        };
        if name.is_empty() || !name.chars().all(acceptable) {
            return Err(PoolError::UnacceptableName {
                name: name.to_owned(),
            });
        }
        Ok(Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_default(&self) -> bool {
        self.0 == DEFAULT_POOL
    }
}

impl fmt::Display for PoolName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One catalog entry as a pool names it: the identifying triple and nothing else.
///
/// A pool holds no measurement of its own. What an entry is — where its models came from, which
/// build measured them, whether it is offered — is read from the catalog every time the pool is
/// resolved, so a pool cannot carry a stale copy of a fact the catalog states.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PoolEntry {
    pub provider: String,
    pub engine: String,
    pub model: String,
}

impl PoolEntry {
    /// The triple one catalog entry is named by.
    pub fn of(entry: &CatalogEntry) -> Self {
        Self {
            provider: entry.provider.clone(),
            engine: entry.engine.name().to_owned(),
            model: entry.model.clone(),
        }
    }

    fn names(&self, entry: &CatalogEntry) -> bool {
        self.provider == entry.provider
            && self.engine == entry.engine.name()
            && self.model == entry.model
    }
}

/// Which entries a pool permits.
///
/// The two forms are exclusive by construction, which is what makes an edit a decision rather than
/// an addition: writing a list replaces the whole-catalog form, and the pool stops following the
/// catalog.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "selection", rename_all = "snake_case")]
pub enum PoolModels {
    /// Every entry the catalog holds, admissible as the catalog measures it. This is the form the
    /// automatic `default` pool is created in.
    AllAdmissible,
    /// The entries the operator left, in the order they left them. Nothing is added to this list by
    /// a later observation.
    Explicit { entries: Vec<PoolEntry> },
}

impl PoolModels {
    /// Whether this pool still follows the catalog.
    pub fn is_tracking(&self) -> bool {
        matches!(self, Self::AllAdmissible)
    }

    /// An explicit ordered list, which is what an edit leaves behind.
    pub fn explicit(entries: impl IntoIterator<Item = PoolEntry>) -> Self {
        Self::Explicit {
            entries: entries.into_iter().collect(),
        }
    }
}

/// The mechanical ceilings a run using this pool is held to.
///
/// Two ceilings and no floor. `max_agents` bounds how many participants a run may create and
/// `max_concurrent_attempts` how many attempts may run at once; neither is a target, and nothing
/// creates a participant to reach either of them.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PoolCapacity {
    pub max_agents: u32,
    pub max_concurrent_attempts: u32,
}

impl PoolCapacity {
    /// The ceilings the automatic `default` pool is created with: six participants and three
    /// concurrent attempts, which is the *working* setting of decision D8. They are placeholders
    /// chosen for coherence rather than measurements, and the first evaluation of a
    /// multi-participant run is what should replace them.
    pub const DEFAULT: Self = Self {
        max_agents: 6,
        max_concurrent_attempts: 3,
    };
}

/// The per-participant bounds a recruit is endowed within.
///
/// Every bound is absent until an operator states one. A default here would be a number nobody
/// decided, standing where a decision belongs: what a run as a whole may spend is the workspace's
/// standing ceiling, and what one participant may be given out of it is a narrowing of that
/// ceiling rather than a product constant.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PoolResourceLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_money_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_wall_time_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_attempt_starts: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_verification_queries: Option<u32>,
}

/// What the operator declared. Every field here is theirs; nothing measured is written into it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PoolDeclaration {
    pub models: PoolModels,
    pub capacity: PoolCapacity,
    #[serde(default)]
    pub resource_limits: PoolResourceLimits,
}

impl PoolDeclaration {
    /// The declaration the automatic `default` pool is created with: the whole catalog, and the
    /// ceilings decision D8 set.
    pub fn tracking_the_catalog() -> Self {
        Self {
            models: PoolModels::AllAdmissible,
            capacity: PoolCapacity::DEFAULT,
            resource_limits: PoolResourceLimits::default(),
        }
    }
}

/// One entry as a resolution found it: the triple the pool names, and whether the catalog offers it
/// now.
///
/// An entry the catalog no longer offers is kept and marked rather than dropped. A pool that grew
/// shorter would answer *why is this model not offered* with silence, and the resolution would stop
/// being a statement about the entries the operator permitted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedEntry {
    pub provider: String,
    pub engine: String,
    pub model: String,
    pub admissible: bool,
    /// Why it is not admissible, in the words the catalog states it in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl ResolvedEntry {
    /// One catalog entry, resolved as it stands.
    fn of(entry: &CatalogEntry) -> Self {
        Self {
            provider: entry.provider.clone(),
            engine: entry.engine.name().to_owned(),
            model: entry.model.clone(),
            admissible: entry.is_admissible(),
            reason: match &entry.availability {
                Availability::Admissible => None,
                Availability::Unavailable { reason } => Some(reason.clone()),
            },
        }
    }

    /// One named entry the catalog does not hold.
    ///
    /// A pool references entries weakly, so an entry that left the catalog leaves the pool
    /// degraded and never shorter.
    fn absent(entry: &PoolEntry) -> Self {
        Self {
            provider: entry.provider.clone(),
            engine: entry.engine.clone(),
            model: entry.model.clone(),
            admissible: false,
            reason: Some(format!(
                "no catalog entry names {} through the {} engine of {}",
                entry.model, entry.engine, entry.provider
            )),
        }
    }
}

/// One provider record a resolution was read through.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObservedProvider {
    pub provider: String,
    pub state: ProviderState,
}

/// What a resolution was taken from.
///
/// This is the reference that stands where a resolution timestamp would: nothing in this crate
/// reads a clock, and a record that stated a time it had not measured would be worse than one that
/// names its source. A reader that holds the catalog can compare `catalog_digest` and see whether
/// the resolution is the one the catalog would produce now.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PoolObservation {
    /// The provider records the resolution was read through, in provider order, each with the state
    /// that record stated. A provider this root has observed nothing about contributes nothing
    /// here, exactly as it contributes no catalog entry.
    pub providers: Vec<ObservedProvider>,
    /// The digest of the whole catalog reading, computed over every entry it held. For a pool that
    /// tracks the catalog it equals the pool's own digest by construction, which is what tracking
    /// means.
    pub catalog_digest: String,
}

impl PoolObservation {
    fn taken(providers: &Providers, catalog: &Catalog) -> Result<Self, PoolError> {
        let mut observed = Vec::new();
        for (_, record) in providers.read_all() {
            if let Some(record) = record? {
                observed.push(ObservedProvider {
                    provider: record.provider,
                    state: record.state,
                });
            }
        }
        Ok(Self {
            providers: observed,
            catalog_digest: digest_of(&resolved_catalog(catalog)),
        })
    }
}

/// A state a resolution found the pool in.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolStateKind {
    /// The declaration was resolved against a catalog reading.
    Resolved,
    /// The pool follows the catalog: an entry discovered later joins it.
    Tracking,
    /// The pool holds the list the operator left: an entry discovered later does not join it.
    Explicit,
    /// Every resolved entry is offered.
    Ready,
    /// Some resolved entry is not offered. The pool is degraded, never shorter.
    Degraded,
    /// No resolved entry is offered.
    Empty,
}

impl PoolStateKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Tracking => "tracking",
            Self::Explicit => "explicit",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Empty => "empty",
        }
    }
}

/// One state, with the sentence a surface shows for it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PoolState {
    pub state: PoolStateKind,
    pub reason: String,
}

impl PoolState {
    fn new(state: PoolStateKind, reason: impl Into<String>) -> Self {
        Self {
            state,
            reason: reason.into(),
        }
    }
}

/// What the controller resolved and observed. Nothing an operator writes appears here.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PoolResolution {
    /// The resolved entries, in the pool's declared order.
    pub entries: Vec<ResolvedEntry>,
    /// How many of them are offered now.
    pub admissible: usize,
    /// The digest of the resolved ordered set, which is the value a run freezes.
    pub digest: String,
    /// Whether this pool still follows the catalog.
    pub tracking: bool,
    pub observation: PoolObservation,
    pub states: Vec<PoolState>,
}

impl PoolResolution {
    /// Whether a state of this kind was written by the last resolution.
    pub fn holds(&self, state: PoolStateKind) -> bool {
        self.states.iter().any(|held| held.state == state)
    }

    /// The sentence one state carries, when it is held.
    pub fn reason_for(&self, state: PoolStateKind) -> Option<&str> {
        self.states
            .iter()
            .find(|held| held.state == state)
            .map(|held| held.reason.as_str())
    }
}

/// One pool as the product root holds it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PoolRecord {
    pub schema_version: u32,
    /// The record's own name, which is the file it stands in. A record found under another pool's
    /// name is refused rather than read as that pool's.
    pub pool: String,
    /// What the operator declared.
    pub declared: PoolDeclaration,
    /// What the controller resolved it to.
    pub resolved: PoolResolution,
}

/// The catalog reading as a resolution sees it: every entry, in catalog order.
fn resolved_catalog(catalog: &Catalog) -> Vec<ResolvedEntry> {
    catalog.entries().iter().map(ResolvedEntry::of).collect()
}

/// Resolve one declaration against one catalog reading.
///
/// This is the whole of the controller's decision, and it is a pure function of a declaration and a
/// catalog: it takes the declaration by immutable borrow, so it cannot write one, and it holds
/// nothing else — no goal, no task, no run, no preference over entries. The resolved order is the
/// declared order in both forms, so an entry is never moved by any property of itself.
fn resolve(
    declared: &PoolDeclaration,
    catalog: &Catalog,
    observation: &PoolObservation,
) -> PoolResolution {
    let entries = match &declared.models {
        PoolModels::AllAdmissible => resolved_catalog(catalog),
        PoolModels::Explicit { entries } => entries
            .iter()
            .map(|named| {
                catalog
                    .entries()
                    .iter()
                    .find(|entry| named.names(entry))
                    .map_or_else(|| ResolvedEntry::absent(named), ResolvedEntry::of)
            })
            .collect(),
    };
    let admissible = entries.iter().filter(|entry| entry.admissible).count();
    let tracking = declared.models.is_tracking();
    let mut states = vec![
        PoolState::new(
            PoolStateKind::Resolved,
            format!(
                "{} of {} permitted {} offered by the catalog as it was read",
                admissible,
                entries.len(),
                match entries.len() {
                    1 => "entry is",
                    _ => "entries are",
                }
            ),
        ),
        match tracking {
            true => PoolState::new(
                PoolStateKind::Tracking,
                "this pool follows the catalog: an entry discovered later joins it",
            ),
            false => PoolState::new(
                PoolStateKind::Explicit,
                "this pool holds the list it was edited to and no longer follows the catalog: an \
                 entry discovered later does not join it",
            ),
        },
    ];
    states.push(match (admissible, entries.len()) {
        (0, _) => PoolState::new(PoolStateKind::Empty, empty_reason(&entries)),
        (offered, total) if offered == total => PoolState::new(
            PoolStateKind::Ready,
            "every entry this pool permits is offered",
        ),
        (offered, total) => PoolState::new(
            PoolStateKind::Degraded,
            format!(
                "{} of {total} entries are not offered — {}",
                total - offered,
                stated_reasons(&entries)
            ),
        ),
    });
    PoolResolution {
        digest: digest_of(&entries),
        entries,
        admissible,
        tracking,
        observation: observation.clone(),
        states,
    }
}

/// Why no entry is offered, naming what would restore one.
fn empty_reason(entries: &[ResolvedEntry]) -> String {
    match entries.is_empty() {
        true => "this pool permits no entry, because the catalog held none when it was resolved"
            .to_owned(),
        false => format!(
            "no entry this pool permits is offered — {}",
            stated_reasons(entries)
        ),
    }
}

/// The reasons the entries that are not offered carry, each stated once.
fn stated_reasons(entries: &[ResolvedEntry]) -> String {
    let stated: BTreeSet<&str> = entries
        .iter()
        .filter_map(|entry| entry.reason.as_deref())
        .collect();
    stated.into_iter().collect::<Vec<_>>().join("; ")
}

/// The digest of a resolved, ordered set of entries — the value a run freezes.
///
/// The input is stated here rather than left to a serializer: each entry contributes its provider,
/// engine and model, whether it is offered and the reason it is not, in the pool's declared order,
/// with every field terminated and every entry terminated after it. Both terminators carry meaning
/// and both are checked. Without the field terminator two different lists whose fields concatenate
/// to the same bytes would share a digest; without the order, a reordered list would — and the
/// order is what later decides which entry a run ignites on.
///
/// What the digest therefore follows is the catalog and the declared order, and nothing else about
/// a pool: a ceiling raised or a resource bound stated leaves it exactly as it was.
fn digest_of(entries: &[ResolvedEntry]) -> String {
    let mut hasher = Sha256::new();
    for entry in entries {
        for field in [
            entry.provider.as_str(),
            entry.engine.as_str(),
            entry.model.as_str(),
            match entry.admissible {
                true => "admissible",
                false => "unavailable",
            },
            entry.reason.as_deref().unwrap_or_default(),
        ] {
            hasher.update(field.as_bytes());
            hasher.update([0u8]);
        }
        hasher.update([0x1eu8]);
    }
    hex::encode(hasher.finalize())
}

#[derive(Debug, Error)]
pub enum PoolError {
    #[error(
        "no pool is named {name}; a pool name carries lower-case letters, digits, `-` and `_` and \
         is never a path"
    )]
    UnacceptableName { name: String },
    #[error("this root holds no pool named {name}; nothing but the controller creates one")]
    UnknownPool { name: String },
    /// The catalog this level resolves against could not be read. Resolution fails rather than
    /// answering with the part of the catalog that was readable, because a pool resolved against a
    /// shorter catalog looks exactly like a pool whose entries left it.
    #[error("the catalog this pool resolves against could not be read: {source}")]
    Catalog {
        #[from]
        source: ProviderError,
    },
    #[error("the pool record {} is not readable: {reason}", path.display())]
    Unreadable { path: PathBuf, reason: String },
    #[error(
        "the pool record {} states schema version {found}; this build reads version {expected} and \
         migrates no record",
        path.display()
    )]
    UnsupportedSchema {
        path: PathBuf,
        found: u32,
        expected: u32,
    },
    #[error("pool I/O error at {}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
}

/// The pool records under one product root.
///
/// Callers that hold a path an invocation addressed reach this through
/// [`crate::RegistryAddress::pools`], so the pools of a run are the ones its root holds, exactly as
/// its providers and its engine registry are.
#[derive(Clone, Debug)]
pub struct Pools {
    directory: PathBuf,
}

impl Pools {
    /// The pool records under a root, taken literally.
    pub fn under(root: impl AsRef<Path>) -> Self {
        Self {
            directory: root.as_ref().join(POOLS_DIRECTORY),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn path_of(&self, name: &PoolName) -> PathBuf {
        self.directory.join(format!("{name}.json"))
    }

    /// The record of one pool, or `None` where this root holds none of that name.
    ///
    /// A record that exists and cannot be read is an error rather than an absence: answering an
    /// unreadable record with `None` would read a broken pool as a pool nobody has created, and the
    /// next reconciliation would create `default` over it.
    pub fn read(&self, name: &PoolName) -> Result<Option<PoolRecord>, PoolError> {
        let path = self.path_of(name);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(PoolError::Io { path, source }),
        };
        let record: PoolRecord =
            serde_json::from_slice(&bytes).map_err(|error| PoolError::Unreadable {
                path: path.clone(),
                reason: error.to_string(),
            })?;
        if record.schema_version != POOL_SCHEMA_VERSION {
            return Err(PoolError::UnsupportedSchema {
                path,
                found: record.schema_version,
                expected: POOL_SCHEMA_VERSION,
            });
        }
        if record.pool != name.as_str() {
            return Err(PoolError::Unreadable {
                path,
                reason: format!(
                    "the record names the {} pool and stands in the place of {name}",
                    record.pool
                ),
            });
        }
        Ok(Some(record))
    }

    /// Every pool this root holds, in name order.
    pub fn read_all(&self) -> Result<Vec<PoolRecord>, PoolError> {
        let mut records = Vec::new();
        for name in self.names()? {
            if let Some(record) = self.read(&name)? {
                records.push(record);
            }
        }
        Ok(records)
    }

    /// The pools this root holds, in name order.
    ///
    /// A file standing in the pools directory whose name is not a pool name is refused rather than
    /// walked past, for the same reason a record standing under another pool's name is: a directory
    /// that answered with the records it happened to understand would read as complete.
    pub fn names(&self) -> Result<Vec<PoolName>, PoolError> {
        let listing = match fs::read_dir(&self.directory) {
            Ok(listing) => listing,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(PoolError::Io {
                    path: self.directory.clone(),
                    source,
                });
            }
        };
        let mut names = Vec::new();
        for entry in listing {
            let path = entry
                .map_err(|source| PoolError::Io {
                    path: self.directory.clone(),
                    source,
                })?
                .path();
            // A staging file of an interrupted write carries the `json.writing` extension and is
            // not a record; it is left where it stands rather than read or removed.
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default();
            names.push(PoolName::parse(stem)?);
        }
        names.sort();
        Ok(names)
    }

    /// Write the operator's half of a record and nothing else.
    ///
    /// The resolution stored is the one the controller last wrote, carried forward exactly: this
    /// path resolves nothing, computes no digest and states no state. A record whose declaration
    /// has moved ahead of its resolution is what a pending reconciliation looks like, and
    /// [`Pools::edit`] is the operator path that never leaves one behind.
    pub fn write_declaration(
        &self,
        name: &PoolName,
        declared: PoolDeclaration,
    ) -> Result<PoolRecord, PoolError> {
        let record = self.require(name)?;
        self.store(name, &declared, &record.resolved)
    }

    /// Amend one pool's declaration and record what it now resolves to.
    ///
    /// This is the operator's edit: the closure receives the declaration alone, so an edit cannot
    /// reach the resolution, and the resolution that follows is the controller's own. Writing an
    /// explicit list therefore leaves a record that both holds the list and says it has stopped
    /// tracking.
    ///
    /// **The catalog is read before anything is written.** The two halves reach the disk in one
    /// write, so a catalog that cannot be read leaves the record exactly as it stood rather than
    /// leaving the operator's list beside a resolution that still says the pool follows the
    /// catalog. An edit either lands whole or does not land.
    ///
    /// A pool this root does not hold is refused rather than created. Nothing but the controller
    /// creates a pool in this build, and it creates exactly one.
    pub fn edit(
        &self,
        name: &PoolName,
        providers: &Providers,
        registry: &Registry,
        amend: impl FnOnce(&mut PoolDeclaration),
    ) -> Result<PoolRecord, PoolError> {
        let mut declared = self.require(name)?.declared;
        amend(&mut declared);
        let catalog = Catalog::read(providers, registry)?;
        let observation = PoolObservation::taken(providers, &catalog)?;
        let resolved = resolve(&declared, &catalog, &observation);
        self.store(name, &declared, &resolved)
    }

    /// Resolve every pool this root holds against the catalog, and create the automatic `default`
    /// where the catalog offers an entry and no `default` exists.
    ///
    /// This is the pool controller. It writes the resolution and, once, the declaration of the
    /// record it creates. It reads no goal, ranks nothing and starts nothing: a pool of three
    /// entries after this call is three entries and zero participants.
    pub fn reconcile(
        &self,
        providers: &Providers,
        registry: &Registry,
    ) -> Result<Vec<PoolRecord>, PoolError> {
        let catalog = Catalog::read(providers, registry)?;
        let observation = PoolObservation::taken(providers, &catalog)?;
        self.create_default(&catalog, &observation)?;
        let mut reconciled = Vec::new();
        for name in self.names()? {
            reconciled.push(self.resolve_one(&name, &catalog, &observation)?);
        }
        Ok(reconciled)
    }

    /// Create the `default` pool, tracking the catalog, the first time the catalog offers an entry.
    ///
    /// This is the one declaration the controller writes, and it writes it once: a `default` that
    /// exists is left exactly as it stands, whether the operator has edited it or not. A root whose
    /// catalog offers nothing holds no pool at all, which is what makes *enable a provider* the
    /// first-run state instead of *no pool is configured*.
    fn create_default(
        &self,
        catalog: &Catalog,
        observation: &PoolObservation,
    ) -> Result<Option<PoolRecord>, PoolError> {
        let name = PoolName::default_pool();
        if self.read(&name)?.is_some() || catalog.admissible().next().is_none() {
            return Ok(None);
        }
        let declared = PoolDeclaration::tracking_the_catalog();
        let resolved = resolve(&declared, catalog, observation);
        self.store(&name, &declared, &resolved).map(Some)
    }

    /// Resolve one pool: the declaration is read from the record and carried forward untouched, and
    /// only the resolution this reading computed is written.
    fn resolve_one(
        &self,
        name: &PoolName,
        catalog: &Catalog,
        observation: &PoolObservation,
    ) -> Result<PoolRecord, PoolError> {
        let record = self.require(name)?;
        let resolved = resolve(&record.declared, catalog, observation);
        self.store(name, &record.declared, &resolved)
    }

    fn require(&self, name: &PoolName) -> Result<PoolRecord, PoolError> {
        self.read(name)?.ok_or_else(|| PoolError::UnknownPool {
            name: name.to_string(),
        })
    }

    /// Write one record, replacing it whole. The record is written beside its own path and renamed
    /// over it, so a reader sees the record before this write or the record after it.
    ///
    /// The two halves arrive as separate arguments and neither is derived from the other here, so
    /// which caller owns which half is decided by what it passes: the controller passes the stored
    /// declaration, the operator's write passes the stored resolution.
    fn store(
        &self,
        name: &PoolName,
        declared: &PoolDeclaration,
        resolved: &PoolResolution,
    ) -> Result<PoolRecord, PoolError> {
        let record = PoolRecord {
            schema_version: POOL_SCHEMA_VERSION,
            pool: name.as_str().to_owned(),
            declared: declared.clone(),
            resolved: resolved.clone(),
        };
        let path = self.path_of(name);
        fs::create_dir_all(&self.directory).map_err(|source| PoolError::Io {
            path: self.directory.clone(),
            source,
        })?;
        let mut bytes =
            serde_json::to_vec_pretty(&record).map_err(|error| PoolError::Unreadable {
                path: path.clone(),
                reason: error.to_string(),
            })?;
        bytes.push(b'\n');
        let staging = path.with_extension("json.writing");
        fs::write(&staging, &bytes).map_err(|source| PoolError::Io {
            path: staging.clone(),
            source,
        })?;
        fs::rename(&staging, &path).map_err(|source| PoolError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ModelCatalog, ModelSource, ProviderFamily};
    use serde_json::Value;

    /// A sandboxed root: a temporary directory, never the operator's own.
    fn root() -> (tempfile::TempDir, Registry, Providers, Pools) {
        let directory = tempfile::tempdir().expect("temporary root");
        let registry = Registry::under(directory.path());
        let providers = Providers::under(directory.path());
        let pools = Pools::under(directory.path());
        (directory, registry, providers, pools)
    }

    /// A measured, admitted engine with a credential and a model list, as a probe would have
    /// recorded it. The provider records are observed from it, which is what turns the names into
    /// catalog entries.
    fn measure(
        registry: &Registry,
        providers: &Providers,
        engine: Engine,
        credential: Option<&str>,
        names: &[&str],
    ) {
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
        providers.observe(registry).expect("observe the providers");
    }

    fn default_record(pools: &Pools) -> PoolRecord {
        pools
            .read(&PoolName::default_pool())
            .expect("read the default pool")
            .expect("the default pool exists")
    }

    /// The triples one pool resolved to, in order.
    fn triples(record: &PoolRecord) -> Vec<(String, String, String, bool)> {
        record
            .resolved
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.provider.clone(),
                    entry.engine.clone(),
                    entry.model.clone(),
                    entry.admissible,
                )
            })
            .collect()
    }

    /// The negative half of this level, and the state the product's first run is in: a root with no
    /// ready provider holds no pool at all.
    ///
    /// Three roots that could each look like the first are checked, because the pool must appear
    /// for the measured reason and not merely eventually: nothing observed, an engine measured
    /// whose account states no credential, and an engine held back by the operator. None of them
    /// offers an entry, so none of them holds a pool — and the directory itself is absent, so
    /// nothing reads as an empty list of pools either.
    #[test]
    fn a_root_with_no_ready_provider_holds_no_pool() {
        let (_directory, registry, providers, pools) = root();

        // Nothing observed at all.
        assert!(
            pools
                .reconcile(&providers, &registry)
                .expect("reconcile")
                .is_empty()
        );
        assert!(!pools.directory().exists(), "a pool directory was created");
        assert!(pools.read_all().expect("read the pools").is_empty());

        // Measured and installed, with nothing stating where a credential is read from: the
        // provider is not configured, so no entry is admissible.
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            None,
            &["claude-opus-5"],
        );
        assert!(
            pools
                .reconcile(&providers, &registry)
                .expect("reconcile")
                .is_empty()
        );
        assert!(!pools.directory().exists(), "a pool directory was created");

        // Measured, credentialed, and held back by the operator.
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        registry
            .set_enabled(
                Engine::ClaudeCode,
                false,
                Some("held back for this experiment"),
            )
            .expect("disable claude");
        providers.observe(&registry).expect("observe");
        assert!(
            pools
                .reconcile(&providers, &registry)
                .expect("reconcile")
                .is_empty()
        );
        assert!(!pools.directory().exists(), "a pool directory was created");

        // The first ready provider with an offered entry is what creates the pool.
        registry
            .set_enabled(Engine::ClaudeCode, true, None)
            .expect("enable claude");
        providers.observe(&registry).expect("observe");
        let reconciled = pools.reconcile(&providers, &registry).expect("reconcile");
        assert_eq!(reconciled.len(), 1);
        assert_eq!(reconciled[0].pool, DEFAULT_POOL);
        assert!(reconciled[0].resolved.tracking);
    }

    /// The `default` pool tracks the catalog: its resolved entries are the catalog's entries in
    /// catalog order, and its digest is the catalog reading's own.
    ///
    /// The ceilings it is created with are decision D8's, and it carries no floor and no count of
    /// participants — that half is asserted on the record's field set below.
    #[test]
    fn the_default_pool_resolves_to_the_catalog_it_tracks() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5", "claude-haiku-4-5"],
        );
        measure(
            &registry,
            &providers,
            Engine::Codex,
            Some("delegated_home_credential"),
            &["gpt-5-codex"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");

        let record = default_record(&pools);
        let catalog = Catalog::read(&providers, &registry).expect("catalog");
        assert_eq!(
            triples(&record),
            catalog
                .entries()
                .iter()
                .map(|entry| (
                    entry.provider.clone(),
                    entry.engine.name().to_owned(),
                    entry.model.clone(),
                    true
                ))
                .collect::<Vec<_>>()
        );
        assert_eq!(record.resolved.admissible, 3);
        assert_eq!(record.declared.models, PoolModels::AllAdmissible);
        assert_eq!(record.declared.capacity, PoolCapacity::DEFAULT);
        assert_eq!(
            record.declared.resource_limits,
            PoolResourceLimits::default()
        );
        assert!(record.resolved.holds(PoolStateKind::Ready));
        assert!(record.resolved.holds(PoolStateKind::Tracking));
        assert!(record.resolved.holds(PoolStateKind::Resolved));
        assert!(!record.resolved.holds(PoolStateKind::Explicit));
        // A pool that tracks the catalog resolves to the catalog, so the two digests are the same
        // value. That equality is what tracking means, stated where it can be read.
        assert_eq!(
            record.resolved.digest,
            record.resolved.observation.catalog_digest
        );
        assert_eq!(
            record
                .resolved
                .observation
                .providers
                .iter()
                .map(|observed| (observed.provider.as_str(), observed.state))
                .collect::<Vec<_>>(),
            vec![
                ("anthropic", ProviderState::Ready),
                ("openai", ProviderState::Ready)
            ]
        );
    }

    /// The digest follows the catalog and nothing else.
    ///
    /// Both halves are measured. A model added, a model withdrawn and an entry that stopped being
    /// offered each move it; a reconciliation over an unchanged catalog and an edit that raises a
    /// ceiling leave it exactly as it was.
    #[test]
    fn the_digest_changes_when_the_catalog_changes_and_only_then() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        let first = default_record(&pools).resolved.digest;

        // Reconciling again over an unchanged catalog reaches the same digest.
        pools.reconcile(&providers, &registry).expect("reconcile");
        assert_eq!(default_record(&pools).resolved.digest, first);

        // A ceiling is not a catalog fact, so raising one leaves the digest where it stands.
        pools
            .edit(
                &PoolName::default_pool(),
                &providers,
                &registry,
                |declared| declared.capacity.max_agents = 12,
            )
            .expect("raise the ceiling");
        let raised = default_record(&pools);
        assert_eq!(raised.declared.capacity.max_agents, 12);
        assert_eq!(raised.resolved.digest, first);

        // A model discovered is.
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5", "claude-sonnet-5"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        let widened = default_record(&pools).resolved.digest;
        assert_ne!(widened, first);
        assert_eq!(default_record(&pools).resolved.entries.len(), 2);

        // And so is an entry that stopped being offered, which leaves the pool degraded rather
        // than shorter.
        registry
            .set_enabled(Engine::ClaudeCode, false, Some("held back"))
            .expect("disable claude");
        pools.reconcile(&providers, &registry).expect("reconcile");
        let degraded = default_record(&pools);
        assert_ne!(degraded.resolved.digest, widened);
        assert_eq!(degraded.resolved.entries.len(), 2, "an entry was dropped");
        assert_eq!(degraded.resolved.admissible, 0);
        assert!(degraded.resolved.holds(PoolStateKind::Empty));
        assert!(
            degraded
                .resolved
                .reason_for(PoolStateKind::Empty)
                .is_some_and(|reason| reason.contains("held back")),
            "{:?}",
            degraded.resolved.states
        );
    }

    /// Editing `default` records the explicit list, stops tracking, and says so on the record.
    ///
    /// The second half is the one that matters: a provider enabled afterwards does not silently
    /// rejoin the pool, so the edit is not quietly overwritten by the next reconciliation.
    #[test]
    fn editing_the_default_pool_replaces_tracking_with_the_list_and_states_it() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5", "claude-haiku-4-5"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        assert_eq!(default_record(&pools).resolved.entries.len(), 2);

        let kept = PoolEntry {
            provider: "anthropic".to_owned(),
            engine: "claude-code".to_owned(),
            model: "claude-haiku-4-5".to_owned(),
        };
        let edited = pools
            .edit(
                &PoolName::default_pool(),
                &providers,
                &registry,
                |declared| {
                    declared.models = PoolModels::explicit([kept.clone()]);
                },
            )
            .expect("edit the default pool");
        assert!(!edited.resolved.tracking);
        assert!(edited.resolved.holds(PoolStateKind::Explicit));
        assert!(!edited.resolved.holds(PoolStateKind::Tracking));
        assert!(
            edited
                .resolved
                .reason_for(PoolStateKind::Explicit)
                .is_some_and(|reason| reason.contains("no longer follows the catalog")),
            "{:?}",
            edited.resolved.states
        );
        assert_eq!(
            triples(&edited),
            vec![(
                "anthropic".to_owned(),
                "claude-code".to_owned(),
                "claude-haiku-4-5".to_owned(),
                true
            )]
        );
        let after_edit = edited.resolved.digest.clone();

        // A second provider observed afterwards joins the catalog and not this pool.
        measure(
            &registry,
            &providers,
            Engine::Codex,
            Some("delegated_home_credential"),
            &["gpt-5-codex"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        let after_discovery = default_record(&pools);
        assert_eq!(triples(&after_discovery), triples(&edited));
        assert_eq!(after_discovery.resolved.digest, after_edit);
        assert!(!after_discovery.resolved.tracking);
        // The catalog moved even though the pool did not, and the observation says so.
        assert_ne!(
            after_discovery.resolved.observation.catalog_digest,
            after_discovery.resolved.digest
        );
        assert_eq!(
            Catalog::read(&providers, &registry)
                .expect("catalog")
                .entries()
                .len(),
            3
        );

        // An entry the operator kept that later leaves the catalog leaves the pool degraded and
        // named, never shorter.
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        let withdrawn = default_record(&pools);
        assert_eq!(withdrawn.resolved.entries.len(), 1);
        assert_eq!(withdrawn.resolved.admissible, 0);
        assert!(
            withdrawn.resolved.entries[0]
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("claude-haiku-4-5")),
            "{:?}",
            withdrawn.resolved.entries
        );
    }

    /// Ownership, first half: the controller writes no declaration but the one it creates.
    ///
    /// The stored declaration is captured as bytes and compared after every reconciliation across a
    /// catalog that widens, narrows and goes dark. A controller that adjusted a ceiling, pinned the
    /// resolved list into `declared.models` or dropped a withdrawn entry from it fails here.
    #[test]
    fn the_reconciler_writes_no_declaration_beyond_the_default_it_creates() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");

        let path = pools.path_of(&PoolName::default_pool());
        let stored_declaration = |label: &str| -> String {
            let value: Value =
                serde_json::from_slice(&fs::read(&path).expect("stored record")).expect("json");
            serde_json::to_string(&value["declared"])
                .unwrap_or_else(|_| panic!("declared at {label}"))
        };
        let created = stored_declaration("creation");
        assert!(created.contains("all_admissible"), "{created}");

        for names in [
            &["claude-opus-5", "claude-sonnet-5"][..],
            &["claude-sonnet-5"][..],
            &[][..],
        ] {
            measure(
                &registry,
                &providers,
                Engine::ClaudeCode,
                Some("delegated_host_keychain_credential"),
                names,
            );
            pools.reconcile(&providers, &registry).expect("reconcile");
            assert_eq!(
                stored_declaration("reconciliation"),
                created,
                "the controller wrote a declaration"
            );
        }

        // The operator's own edit moves the declaration, and the reconciliations that follow it
        // leave the moved declaration exactly as they found it.
        pools
            .edit(
                &PoolName::default_pool(),
                &providers,
                &registry,
                |declared| {
                    declared.capacity.max_concurrent_attempts = 1;
                    declared.resource_limits.max_money_micros = Some(5_000_000);
                },
            )
            .expect("edit");
        let narrowed = stored_declaration("edit");
        assert_ne!(narrowed, created);
        pools.reconcile(&providers, &registry).expect("reconcile");
        assert_eq!(stored_declaration("after the edit"), narrowed);
    }

    /// Ownership, second half: writing a declaration states no resolution of its own.
    ///
    /// The declaration written is the one that moves every field of the resolution a leak would
    /// reach: it replaces the whole-catalog form with a list, which decides `tracking`, and it is
    /// written against a catalog that has moved since the last resolution, which decides the
    /// entries and the digest. The stored resolution must nevertheless still be the one the last
    /// reconciliation wrote — stale, and owned by the controller that owns it. A declaration path
    /// that recomputed a resolution, cleared one or merely copied `tracking` out of the declaration
    /// fails here.
    #[test]
    fn a_declaration_write_states_no_resolution_of_its_own() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        let resolved = default_record(&pools).resolved;
        assert!(resolved.tracking);
        assert_eq!(resolved.entries.len(), 1);

        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5", "claude-sonnet-5"],
        );
        let written = pools
            .write_declaration(&PoolName::default_pool(), {
                let mut declared = PoolDeclaration::tracking_the_catalog();
                declared.models = PoolModels::explicit([
                    PoolEntry {
                        provider: "anthropic".to_owned(),
                        engine: "claude-code".to_owned(),
                        model: "claude-opus-5".to_owned(),
                    },
                    PoolEntry {
                        provider: "anthropic".to_owned(),
                        engine: "claude-code".to_owned(),
                        model: "claude-sonnet-5".to_owned(),
                    },
                ]);
                declared.capacity.max_agents = 2;
                declared
            })
            .expect("write the declaration");
        assert_eq!(written.declared.capacity.max_agents, 2);
        assert!(!written.declared.models.is_tracking());
        assert_eq!(
            written.resolved, resolved,
            "the declaration write path wrote a resolution of its own"
        );
        assert_eq!(default_record(&pools).resolved, resolved);

        // The controller is what moves it, and it moves nothing else.
        pools.reconcile(&providers, &registry).expect("reconcile");
        let reconciled = default_record(&pools);
        assert_ne!(reconciled.resolved, resolved);
        assert!(!reconciled.resolved.tracking);
        assert_eq!(reconciled.resolved.entries.len(), 2);
        assert_eq!(reconciled.declared.capacity.max_agents, 2);
    }

    /// Every field the record can carry, as it is actually serialized.
    ///
    /// A floor, a desired count, a replica count, a role, a rank or a participant cannot be added
    /// to this layout without this list changing, and changing it is the decision that has to be
    /// argued. It is asserted over a fully populated record — an explicit list, every resource
    /// bound stated, and an entry that is not offered — so no key is missing merely because its
    /// value was absent.
    ///
    /// The list is also where decision D12 is enforced: the halves of the record are `declared` and
    /// `resolved`, and neither `spec`, `status` nor `conditions` may reappear as a key without this
    /// assertion changing.
    #[test]
    fn the_pool_record_carries_only_mechanical_fields() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        pools
            .edit(
                &PoolName::default_pool(),
                &providers,
                &registry,
                |declared| {
                    declared.models = PoolModels::explicit([
                        PoolEntry {
                            provider: "anthropic".to_owned(),
                            engine: "claude-code".to_owned(),
                            model: "claude-opus-5".to_owned(),
                        },
                        PoolEntry {
                            provider: "anthropic".to_owned(),
                            engine: "claude-code".to_owned(),
                            model: "claude-sonnet-5".to_owned(),
                        },
                    ]);
                    declared.resource_limits = PoolResourceLimits {
                        max_money_micros: Some(5_000_000),
                        max_wall_time_ms: Some(7_200_000),
                        max_attempt_starts: Some(3),
                        max_verification_queries: Some(4),
                    };
                },
            )
            .expect("edit");

        let stored: Value = serde_json::from_slice(
            &fs::read(pools.path_of(&PoolName::default_pool())).expect("record"),
        )
        .expect("json");
        let mut keys = BTreeSet::new();
        collect_keys(&stored, &mut keys);
        assert_eq!(
            keys.into_iter().collect::<Vec<_>>(),
            vec![
                "admissible",
                "capacity",
                "catalog_digest",
                "declared",
                "digest",
                "engine",
                "entries",
                "max_agents",
                "max_attempt_starts",
                "max_concurrent_attempts",
                "max_money_micros",
                "max_verification_queries",
                "max_wall_time_ms",
                "model",
                "models",
                "observation",
                "pool",
                "provider",
                "providers",
                "reason",
                "resolved",
                "resource_limits",
                "schema_version",
                "selection",
                "state",
                "states",
                "tracking",
            ]
        );
    }

    fn collect_keys(value: &Value, keys: &mut BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    keys.insert(key.clone());
                    collect_keys(value, keys);
                }
            }
            Value::Array(items) => {
                for item in items {
                    collect_keys(item, keys);
                }
            }
            _ => {}
        }
    }

    /// Nothing about a pool instantiates a participant.
    ///
    /// A pool of three entries is three entries: no process is started, no directory but the pool's
    /// own appears under the root, and the record names nothing that runs. The ceiling it carries
    /// is a ceiling and not a count — six is what a run may create, and zero is what exists.
    #[test]
    fn resolving_a_pool_instantiates_nothing() {
        let (directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5", "claude-haiku-4-5"],
        );
        measure(
            &registry,
            &providers,
            Engine::Codex,
            Some("delegated_home_credential"),
            &["gpt-5-codex"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");

        let record = default_record(&pools);
        assert_eq!(record.resolved.entries.len(), 3);
        assert_eq!(record.declared.capacity.max_agents, 6);

        let mut held: Vec<String> = fs::read_dir(directory.path())
            .expect("the root")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        held.sort();
        assert_eq!(held, vec!["pools", "providers", "runtimes"]);
        let files: Vec<String> = fs::read_dir(pools.directory())
            .expect("the pools directory")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(files, vec!["default.json"]);

        let stored = fs::read_to_string(pools.path_of(&PoolName::default_pool())).expect("record");
        for word in [
            "participant",
            "agent_id",
            "invocation",
            "attempt_id",
            "run_id",
        ] {
            assert!(
                !stored.contains(word),
                "the record names a {word}: {stored}"
            );
        }
    }

    /// The resolution keeps the declared order and ranks nothing.
    ///
    /// The fixture is built so that a rank would be visible. The models of one provider are
    /// measured in an order that is not alphabetical, and the entries that stop being offered are
    /// the ones the catalog lists **first** — so a resolution that moved the offered entries to the
    /// front, or sorted by name, or preferred an entry for any other property of itself, produces a
    /// different list. The resolved order must be the catalog's own, with the entries that are not
    /// offered left exactly where they stand, because that position is what later decides which
    /// entry a run ignites on.
    #[test]
    fn the_resolution_keeps_the_declared_order_and_ranks_nothing() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5", "claude-haiku-4-5"],
        );
        measure(
            &registry,
            &providers,
            Engine::Codex,
            Some("delegated_home_credential"),
            &["gpt-5-codex"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        // The engine the catalog lists first is held back after the pool exists, so the entries
        // that are not offered stand ahead of the one that is.
        registry
            .set_enabled(Engine::ClaudeCode, false, Some("held back"))
            .expect("disable claude");
        providers.observe(&registry).expect("observe");
        pools.reconcile(&providers, &registry).expect("reconcile");

        let record = default_record(&pools);
        assert_eq!(
            record
                .resolved
                .entries
                .iter()
                .map(|entry| (entry.model.as_str(), entry.admissible))
                .collect::<Vec<_>>(),
            vec![
                ("claude-opus-5", false),
                ("claude-haiku-4-5", false),
                ("gpt-5-codex", true),
            ]
        );
        assert_eq!(record.resolved.admissible, 1);
        assert!(record.resolved.holds(PoolStateKind::Degraded));
        assert!(
            record
                .resolved
                .reason_for(PoolStateKind::Degraded)
                .is_some_and(|reason| reason.contains("held back")),
            "{:?}",
            record.resolved.states
        );
    }

    /// A record standing under another pool's name is refused, and so is one of another schema
    /// version. Neither is migrated, and neither is answered with a fresh `default`.
    #[test]
    fn a_record_of_another_pool_or_another_version_is_refused() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        let path = pools.path_of(&PoolName::default_pool());
        let mut record: PoolRecord =
            serde_json::from_slice(&fs::read(&path).expect("record")).expect("json");

        record.pool = "cheap".to_owned();
        fs::write(&path, serde_json::to_vec_pretty(&record).expect("bytes")).expect("write");
        let error = pools
            .read(&PoolName::default_pool())
            .expect_err("a misplaced record was read as the default pool");
        assert!(matches!(error, PoolError::Unreadable { .. }), "{error:?}");

        record.pool = DEFAULT_POOL.to_owned();
        record.schema_version = POOL_SCHEMA_VERSION + 1;
        fs::write(&path, serde_json::to_vec_pretty(&record).expect("bytes")).expect("write");
        let error = pools
            .read(&PoolName::default_pool())
            .expect_err("a later record was read");
        assert!(
            matches!(error, PoolError::UnsupportedSchema { .. }),
            "{error:?}"
        );
        // The reconciler does not answer an unreadable record by creating a new pool over it.
        let error = pools
            .reconcile(&providers, &registry)
            .expect_err("an unreadable record was reconciled");
        assert!(
            matches!(error, PoolError::UnsupportedSchema { .. }),
            "{error:?}"
        );
    }

    /// A pool this root does not hold is refused rather than created: nothing but the reconciler
    /// creates a pool, and a name that is a path is refused before it addresses anything.
    #[test]
    fn nothing_but_the_reconciler_creates_a_pool() {
        let (_directory, registry, providers, pools) = root();
        let error = pools
            .edit(
                &PoolName::default_pool(),
                &providers,
                &registry,
                |declared| {
                    declared.capacity.max_agents = 99;
                },
            )
            .expect_err("an edit created a pool");
        assert!(matches!(error, PoolError::UnknownPool { .. }), "{error:?}");
        assert!(!pools.directory().exists());

        for name in ["", "../escape", "Default", "with space"] {
            let error =
                PoolName::parse(name).expect_err("a name that is not a pool name was taken");
            assert!(
                matches!(error, PoolError::UnacceptableName { .. }),
                "{error:?}"
            );
        }
        assert_eq!(
            PoolName::parse(" default ").expect("default").as_str(),
            DEFAULT_POOL
        );
        assert!(PoolName::default_pool().is_default());
    }

    /// A catalog that cannot be read fails the resolution rather than shortening the pool.
    ///
    /// This is the reading that would otherwise look right and be wrong: a pool resolved against a
    /// catalog missing one engine's models is indistinguishable from a pool whose entries were
    /// withdrawn, and the digest it wrote would be a digest of the wrong set.
    #[test]
    fn an_unreadable_catalog_fails_the_resolution_rather_than_shortening_the_pool() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        measure(
            &registry,
            &providers,
            Engine::Codex,
            Some("delegated_home_credential"),
            &["gpt-5-codex"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        let before = default_record(&pools);

        fs::write(registry.path_of(Engine::Codex), b"{ not json").expect("corrupt the record");
        let error = pools
            .reconcile(&providers, &registry)
            .expect_err("a pool was resolved against a catalog that could not be read");
        assert!(matches!(error, PoolError::Catalog { .. }), "{error:?}");
        assert_eq!(
            default_record(&pools),
            before,
            "a failed resolution rewrote the record"
        );
    }

    /// Resolving a pool writes into no engine and no provider record.
    ///
    /// The catalog this level reads is measured elsewhere, and a level that rewrote a record while
    /// resolving against it would be taking a measurement under the name of reading one.
    #[test]
    fn resolving_a_pool_writes_into_no_engine_or_provider_record() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        let read_all = || -> Vec<Vec<u8>> {
            Engine::ALL
                .iter()
                .map(|engine| fs::read(registry.path_of(*engine)).unwrap_or_default())
                .chain(
                    ProviderFamily::ALL
                        .iter()
                        .map(|family| fs::read(providers.path_of(*family)).unwrap_or_default()),
                )
                .collect()
        };
        let before = read_all();
        pools.reconcile(&providers, &registry).expect("reconcile");
        pools
            .edit(
                &PoolName::default_pool(),
                &providers,
                &registry,
                |declared| {
                    declared.capacity.max_agents = 4;
                },
            )
            .expect("edit");
        assert_eq!(
            before,
            read_all(),
            "a record beneath the pool was rewritten"
        );
    }

    /// An edit that cannot be resolved leaves the record exactly as it stood.
    ///
    /// The catalog is broken between the operator's decision and the write, which is the one moment
    /// the two halves of a record could disagree: the list the operator left would stand beside a
    /// resolution still saying *this pool follows the catalog*, and the digest would belong to the
    /// entries of a pool that no longer exists. Reading the catalog before writing anything is what
    /// makes that impossible, so an edit either lands whole or does not land at all.
    #[test]
    fn an_edit_that_cannot_be_resolved_leaves_the_record_as_it_stood() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5"],
        );
        measure(
            &registry,
            &providers,
            Engine::Codex,
            Some("delegated_home_credential"),
            &["gpt-5-codex"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        let path = pools.path_of(&PoolName::default_pool());
        let before = fs::read(&path).expect("the stored record");

        fs::write(registry.path_of(Engine::Codex), b"{ not json").expect("corrupt the record");
        let error = pools
            .edit(
                &PoolName::default_pool(),
                &providers,
                &registry,
                |declared| {
                    declared.models = PoolModels::explicit([PoolEntry {
                        provider: "anthropic".to_owned(),
                        engine: "claude-code".to_owned(),
                        model: "claude-opus-5".to_owned(),
                    }]);
                },
            )
            .expect_err("an edit was written against a catalog that could not be read");
        assert!(matches!(error, PoolError::Catalog { .. }), "{error:?}");

        assert_eq!(
            fs::read(&path).expect("the stored record"),
            before,
            "a refused edit wrote a record"
        );
        // Stated as the property rather than only as bytes: whatever is on disk, the two halves
        // agree with each other.
        let record = default_record(&pools);
        assert!(record.declared.models.is_tracking());
        assert!(record.resolved.tracking);
        assert!(record.resolved.holds(PoolStateKind::Tracking));
        assert!(!record.resolved.holds(PoolStateKind::Explicit));
    }

    /// The digest follows the order the pool declares.
    ///
    /// The same two entries in the opposite order are a different permitted set, because the order
    /// is what later decides which entry a run ignites on. A digest computed over a sorted copy of
    /// the entries would answer with one value for both, and two runs frozen against it could not
    /// be told apart by the value they froze.
    #[test]
    fn the_digest_follows_the_order_the_pool_declares() {
        let (_directory, registry, providers, pools) = root();
        measure(
            &registry,
            &providers,
            Engine::ClaudeCode,
            Some("delegated_host_keychain_credential"),
            &["claude-opus-5", "claude-haiku-4-5"],
        );
        pools.reconcile(&providers, &registry).expect("reconcile");
        let entry = |model: &str| PoolEntry {
            provider: "anthropic".to_owned(),
            engine: "claude-code".to_owned(),
            model: model.to_owned(),
        };
        let ordered = |first: &str, second: &str| -> PoolRecord {
            pools
                .edit(
                    &PoolName::default_pool(),
                    &providers,
                    &registry,
                    |declared| {
                        declared.models = PoolModels::explicit([entry(first), entry(second)]);
                    },
                )
                .expect("edit")
        };

        let forward = ordered("claude-opus-5", "claude-haiku-4-5");
        let reversed = ordered("claude-haiku-4-5", "claude-opus-5");
        assert_ne!(
            forward.resolved.digest, reversed.resolved.digest,
            "a reordered pool froze under the same digest"
        );
        // The two hold the same entries, so nothing but the order can have moved the digest.
        let held = |record: &PoolRecord| -> BTreeSet<String> {
            record
                .resolved
                .entries
                .iter()
                .map(|entry| entry.model.clone())
                .collect()
        };
        assert_eq!(held(&forward), held(&reversed));
        assert_eq!(forward.resolved.admissible, 2);
    }

    /// The digest keeps the fields it hashes apart.
    ///
    /// The two lists below carry the same bytes in the same order and split them between the
    /// provider and the engine in different places. A digest that ran the fields together without
    /// terminating each one would answer with a single value for both, so two different permitted
    /// sets would freeze under one digest and evidence naming it could not say which was meant.
    #[test]
    fn the_digest_keeps_the_fields_it_hashes_apart() {
        let entry = |provider: &str, engine: &str| ResolvedEntry {
            provider: provider.to_owned(),
            engine: engine.to_owned(),
            model: "model".to_owned(),
            admissible: true,
            reason: None,
        };
        let one = [entry("ab", "c")];
        let other = [entry("a", "bc")];
        assert_eq!(
            format!("{}{}", one[0].provider, one[0].engine),
            format!("{}{}", other[0].provider, other[0].engine),
            "the two lists do not carry the same bytes, so they test nothing"
        );
        assert_ne!(
            digest_of(&one),
            digest_of(&other),
            "two different permitted sets share one digest"
        );
    }
}

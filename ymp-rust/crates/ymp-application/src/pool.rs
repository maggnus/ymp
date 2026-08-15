//! Taking the pool a run is created against out of the product root, once.
//!
//! This is the whole of the bridge between the level that resolves pools and the level that runs
//! them. The pool records under the root are the operator's boundary as it stands now; a run is
//! created against the value that boundary had at one moment, and reads it by value ever after
//! (`ymp-docs/design/COLLECTIVE-DESIGN.md` §7).
//!
//! **The freeze is read here and decided in the domain.** What this module does is read one pool
//! record and hand its resolved entries, in the pool's declared order, to
//! [`FrozenPool::freeze`]. Which entry the run ignites on is not decided here and is not passed in:
//! the rule reads position and measured readiness alone, and it lives with the value it produces
//! (owner decision D2).
//!
//! **A root that cannot offer a run anything refuses one, in plain words.** A root with no pool, or
//! with a pool nothing in is live, is not an error condition of the machinery: it is the ordinary
//! first state of the product, and what an operator reads names the one thing that is missing and
//! the command that supplies it (product brief v2, Part A §6). The technical reason is carried
//! beside those words rather than instead of them.

use std::path::Path;

use thiserror::Error;
use ymp_domain::pool::{FrozenEntry, FrozenPool, PoolFreezeError};
use ymp_runtime_registry::{
    DEFAULT_POOL, PoolError, PoolName, PoolRecord, PoolStateKind, Pools, ResolvedEntry,
};

/// Why no run was created, in the words an operator reads.
///
/// Every variant leads with the state and names the act that changes it. None of them reports a
/// failure of the product: a host with nothing enabled is a host that has not been set up yet.
#[derive(Debug, Error)]
pub enum PoolFreezeRefused {
    #[error(
        "your goal is held · running it needs at least one enabled AI provider — /providers is \
         where one is enabled · this root holds no pool to draw models from, and the pool is \
         created by the first measurement of an account rather than by you · nothing has started \
         and nothing has left this host"
    )]
    NoPool,
    #[error(
        "your goal is held · none of the models this root permits is available right now — \
         /providers is where a provider is enabled or measured again · {reason} · nothing has \
         started and nothing has left this host"
    )]
    NothingLive { pool: String, reason: String },
    #[error(
        "your goal is held · this root holds no pool named {named}, and only {DEFAULT_POOL} and \
         the pools this root already holds can be drawn from — /pools states them · nothing has \
         started and nothing has left this host"
    )]
    UnknownPool { named: String },
    #[error(
        "your goal is held · the pool this run would draw its models from could not be read, so \
         nothing was created against a boundary nobody can state — {source} · nothing has started \
         and nothing has left this host"
    )]
    Unreadable {
        #[source]
        source: PoolError,
    },
    /// A record that reached this level in a shape the freeze does not admit. It is stated rather
    /// than repaired: a snapshot silently corrected here would be a boundary the record does not
    /// hold.
    #[error(
        "your goal is held · the pool this run would draw its models from cannot be frozen as it \
         stands — {source} · /pools states what it holds · nothing has started and nothing has \
         left this host"
    )]
    Unfreezable {
        #[source]
        source: PoolFreezeError,
    },
}

/// Freeze the pool one run is created against, read from the pools of one product root.
///
/// `named` is the pool the work asks for. A root that holds a record under that name is frozen
/// from it; a name this root does not hold is refused rather than resolved to the nearest one,
/// because a run created against a pool nobody declared would carry a boundary the operator never
/// stated. Where no name is given, the pool is `default` — the one the pool controller creates on
/// the first admissible entry, and the only one this build creates.
pub fn freeze_under(root: &Path, named: Option<&str>) -> Result<FrozenPool, PoolFreezeRefused> {
    let pools = Pools::under(root);
    let name = match named {
        None => PoolName::default_pool(),
        Some(named) => PoolName::parse(named).map_err(|_| PoolFreezeRefused::UnknownPool {
            named: named.to_owned(),
        })?,
    };
    let record = pools
        .read(&name)
        .map_err(|source| PoolFreezeRefused::Unreadable { source })?
        .ok_or_else(|| match named {
            None => PoolFreezeRefused::NoPool,
            Some(named) => PoolFreezeRefused::UnknownPool {
                named: named.to_owned(),
            },
        })?;
    freeze_record(&record)
}

/// Freeze one pool record exactly as it stands.
///
/// The entries are carried across in the record's own order and the record's own digest is carried
/// with them: nothing is recomputed here, so the digest a run names is the digest the pool named,
/// and two runs frozen from one resolution name the same value.
pub fn freeze_record(record: &PoolRecord) -> Result<FrozenPool, PoolFreezeRefused> {
    let entries: Vec<FrozenEntry> = record.resolved.entries.iter().map(frozen).collect();
    FrozenPool::freeze(record.pool.clone(), entries, record.resolved.digest.clone()).map_err(
        |source| match source {
            PoolFreezeError::NoAdmissibleEntry { pool, .. } => PoolFreezeRefused::NothingLive {
                pool,
                reason: nothing_live_reason(record),
            },
            source => PoolFreezeRefused::Unfreezable { source },
        },
    )
}

/// One resolved entry as the run records it. The triple, whether admission found it live, and the
/// reason it did not — in the words the catalog stated them in, so a run's evidence and the pool
/// page cannot state different reasons for the same entry.
fn frozen(entry: &ResolvedEntry) -> FrozenEntry {
    FrozenEntry {
        provider: entry.provider.clone(),
        engine: entry.engine.clone(),
        model: entry.model.clone(),
        admissible: entry.admissible,
        reason: entry.reason.clone(),
    }
}

/// Why nothing in this pool is live, in the record's own words.
///
/// The resolution already wrote that sentence when it found the pool empty, and it names the
/// measured reasons rather than a count. Reaching for it here keeps one explanation for one state
/// instead of composing a second one beside it.
fn nothing_live_reason(record: &PoolRecord) -> String {
    record
        .resolved
        .reason_for(PoolStateKind::Empty)
        .unwrap_or("no entry this pool permits is offered")
        .to_owned()
}

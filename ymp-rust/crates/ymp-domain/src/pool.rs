//! The pool one run may create participants from, held by value.
//!
//! A run references its pool by name for provenance only. What every later decision about
//! recruitment reads is the value frozen here, and never the record under the product root
//! (`ymp-docs/design/COLLECTIVE-RESOURCES.md`, *Run*). That indirection is the whole of the
//! run-scoped freeze: a provider disabled, an entry taken out of a pool or a model discovered after
//! a run was created cannot change what that run may do, and the next run is created against the
//! pool as it stands then.
//!
//! Three things are recorded and each is load-bearing.
//!
//! * **Every permitted entry, in the pool's declared order**, with what admission measured about
//!   it. An entry the catalog no longer offered is kept and marked rather than dropped, exactly as
//!   the pool's own resolution keeps it: a snapshot that grew shorter would answer *why is this
//!   model not permitted* with silence.
//! * **The entry the run ignites on** — the first entry of the frozen pool, in declared order,
//!   that admission found live. It is decided here, from position and measured readiness alone, and
//!   committed with the snapshot, so which entry ignited a run is read from the record rather than
//!   reconstructed (owner decision D2).
//! * **The digest of the ordered set**, which is the pool's own digest. Two runs naming the same
//!   digest were created against the same capability boundary, which is what makes a matched-budget
//!   comparison reproducible.
//!
//! Nothing here reads a goal, a task or a prompt, and nothing ranks an entry by any property of
//! itself. The order is the pool's declared order — the catalog's for a pool that follows it, the
//! operator's for one they edited — and position is the only thing the ignition rule reads besides
//! readiness.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::MAX_IDENTIFIER_CHARS;

/// The triple one catalog entry is named by, which is all a containment check reads.
///
/// It carries no measurement: whether an entry may be recruited on is answered by
/// [`FrozenPool::permits`] against the snapshot, never by a field of the name.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EntryIdentity {
    pub provider: String,
    pub engine: String,
    pub model: String,
}

impl EntryIdentity {
    pub fn new(
        provider: impl Into<String>,
        engine: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            engine: engine.into(),
            model: model.into(),
        }
    }
}

impl std::fmt::Display for EntryIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} · {} · {}",
            self.provider, self.engine, self.model
        )
    }
}

/// One entry of a frozen pool: the triple it is named by, and what admission measured about it when
/// the run was created.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FrozenEntry {
    pub provider: String,
    pub engine: String,
    pub model: String,
    /// Whether admission found this entry live at the moment of the freeze.
    pub admissible: bool,
    /// Why it was not, in the words the catalog stated it in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl FrozenEntry {
    /// One entry admission found live.
    pub fn admissible(
        provider: impl Into<String>,
        engine: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            engine: engine.into(),
            model: model.into(),
            admissible: true,
            reason: None,
        }
    }

    /// One entry the pool permits and admission did not find live, with the reason it stated.
    pub fn unavailable(
        provider: impl Into<String>,
        engine: impl Into<String>,
        model: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            engine: engine.into(),
            model: model.into(),
            admissible: false,
            reason: Some(reason.into()),
        }
    }

    /// The triple this entry is named by.
    pub fn identity(&self) -> EntryIdentity {
        EntryIdentity::new(&self.provider, &self.engine, &self.model)
    }

    /// Whether this entry is the one a name addresses.
    pub fn names(&self, entry: &EntryIdentity) -> bool {
        self.provider == entry.provider && self.engine == entry.engine && self.model == entry.model
    }
}

/// The pool a run was created against, by value.
///
/// Every field is set once, when the run is created, and none of them ever changes: the freeze is a
/// value and not a reference (`ymp-docs/design/COLLECTIVE-DESIGN.md` §7).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FrozenPool {
    /// The pool this was taken from, by name. It is provenance and nothing else: no decision reads
    /// the record standing under that name, because the record may have moved on.
    pub pool: String,
    /// Every entry the pool permitted, in its declared order.
    pub entries: Vec<FrozenEntry>,
    /// The entry the run ignites on: the first entry above, in declared order, that admission found
    /// live (owner decision D2).
    pub origin: EntryIdentity,
    /// The digest of the ordered set, which is the pool's own digest.
    pub digest: String,
}

impl FrozenPool {
    /// Freeze one resolved pool for one run.
    ///
    /// The ignition entry is decided here rather than taken from a caller, and it is decided by the
    /// only two things the rule may read: position in the declared order, and whether admission
    /// found the entry live. A pool permitting no live entry is refused — a run created against it
    /// could recruit nobody, so it is not created at all.
    pub fn freeze(
        pool: impl Into<String>,
        entries: Vec<FrozenEntry>,
        digest: impl Into<String>,
    ) -> Result<Self, PoolFreezeError> {
        let pool = pool.into();
        let digest = digest.into();
        validate_pool_name(&pool)?;
        validate_pool_digest(&digest)?;
        let origin = entries
            .iter()
            .find(|entry| entry.admissible)
            .ok_or_else(|| PoolFreezeError::NoAdmissibleEntry {
                pool: pool.clone(),
                permitted: entries.len(),
            })?
            .identity();
        Ok(Self {
            pool,
            entries,
            origin,
            digest,
        })
    }

    /// Whether this run may create a participant on one entry.
    ///
    /// This is the containment check every later recruitment decision is held to. It reads the
    /// frozen value and nothing else: an entry that is not in the snapshot was never permitted, and
    /// an entry admission did not find live when the run was created is not made live by anything
    /// that happened to the pool afterwards.
    pub fn permits(&self, entry: &EntryIdentity) -> bool {
        self.entries
            .iter()
            .any(|frozen| frozen.admissible && frozen.names(entry))
    }

    /// How many of the permitted entries admission found live.
    pub fn admissible(&self) -> usize {
        self.entries.iter().filter(|entry| entry.admissible).count()
    }

    /// Whether this snapshot is one the ignition rule could have produced.
    ///
    /// A record is checked against the rule rather than trusted by it: the entry a snapshot names
    /// as its ignition must be the first live entry in its own declared order, so a record whose
    /// ignition was written by anything but the rule is refused where it is read.
    pub fn validate(&self) -> Result<(), PoolFreezeError> {
        validate_pool_name(&self.pool)?;
        validate_pool_digest(&self.digest)?;
        let expected =
            Self::freeze(self.pool.clone(), self.entries.clone(), self.digest.clone())?.origin;
        if expected != self.origin {
            return Err(PoolFreezeError::OriginIsNotTheFirstLiveEntry {
                recorded: Box::new(self.origin.clone()),
                first: Box::new(expected),
            });
        }
        Ok(())
    }
}

/// Why a pool could not be frozen for a run.
///
/// Every one of these is a statement about the pool and never about the goal: nothing here reads
/// what the run is for.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PoolFreezeError {
    #[error(
        "the {pool} pool permits {permitted} entr{} and admission found none of them live",
        if *permitted == 1 { "y" } else { "ies" }
    )]
    NoAdmissibleEntry { pool: String, permitted: usize },
    #[error("a pool name must contain between 1 and {MAX_IDENTIFIER_CHARS} characters")]
    UnacceptablePoolName,
    #[error("the pool digest is not a canonical lowercase SHA-256 digest")]
    InvalidDigest,
    #[error(
        "the snapshot ignites on {recorded}, which is not the first live entry of its own declared \
         order — that is {first}"
    )]
    OriginIsNotTheFirstLiveEntry {
        /// Boxed so that the refusal a run's transitions carry stays small: a variant holding two
        /// triples by value would widen every `Result` on the run's own decision path.
        recorded: Box<EntryIdentity>,
        first: Box<EntryIdentity>,
    },
}

fn validate_pool_name(name: &str) -> Result<(), PoolFreezeError> {
    let length = name.chars().count();
    if (1..=MAX_IDENTIFIER_CHARS).contains(&length) {
        Ok(())
    } else {
        Err(PoolFreezeError::UnacceptablePoolName)
    }
}

fn validate_pool_digest(digest: &str) -> Result<(), PoolFreezeError> {
    let canonical = digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if canonical {
        Ok(())
    } else {
        Err(PoolFreezeError::InvalidDigest)
    }
}

#[cfg(test)]
mod tests {
    use super::{EntryIdentity, FrozenEntry, FrozenPool, PoolFreezeError};

    fn digest() -> String {
        "a".repeat(64)
    }

    /// The declared order decides the ignition entry, and readiness is the only other thing read.
    /// The entry standing first is passed over exactly when admission did not find it live.
    #[test]
    fn the_run_ignites_on_the_first_live_entry_of_the_declared_order() {
        let frozen = FrozenPool::freeze(
            "default",
            vec![
                FrozenEntry::unavailable(
                    "anthropic",
                    "claude-code",
                    "claude-opus-5",
                    "the account states no credential",
                ),
                FrozenEntry::admissible("anthropic", "claude-code", "claude-sonnet-5"),
                FrozenEntry::admissible("openai", "codex", "gpt-5"),
            ],
            digest(),
        )
        .expect("the pool freezes");

        assert_eq!(
            frozen.origin,
            EntryIdentity::new("anthropic", "claude-code", "claude-sonnet-5")
        );
        // The entry that was passed over is still in the snapshot, with the reason it carried.
        assert_eq!(frozen.entries.len(), 3);
        assert_eq!(frozen.admissible(), 2);
        assert!(frozen.validate().is_ok());
    }

    /// The containment check reads the snapshot and nothing else: a permitted entry admission did
    /// not find live is not recruitable, and an entry outside the snapshot never was.
    #[test]
    fn the_snapshot_permits_only_the_entries_admission_found_live() {
        let frozen = FrozenPool::freeze(
            "default",
            vec![
                FrozenEntry::admissible("anthropic", "claude-code", "claude-opus-5"),
                FrozenEntry::unavailable("openai", "codex", "gpt-5", "the engine is held back"),
            ],
            digest(),
        )
        .expect("the pool freezes");

        assert!(frozen.permits(&EntryIdentity::new(
            "anthropic",
            "claude-code",
            "claude-opus-5"
        )));
        assert!(!frozen.permits(&EntryIdentity::new("openai", "codex", "gpt-5")));
        assert!(!frozen.permits(&EntryIdentity::new(
            "anthropic",
            "claude-code",
            "claude-sonnet-5"
        )));
        // The engine is part of the name, so the same model through another engine is another
        // entry and is not permitted by this one.
        assert!(!frozen.permits(&EntryIdentity::new("anthropic", "codex", "claude-opus-5")));
    }

    /// A pool nothing is live in freezes into nothing. The refusal names the pool and how much it
    /// permitted, so what the operator reads is the state of their providers.
    #[test]
    fn a_pool_with_no_live_entry_is_refused_rather_than_frozen_empty() {
        let refusal = FrozenPool::freeze(
            "default",
            vec![FrozenEntry::unavailable(
                "anthropic",
                "claude-code",
                "claude-opus-5",
                "the account is not enabled",
            )],
            digest(),
        )
        .expect_err("a pool with nothing live cannot be frozen");
        assert_eq!(
            refusal,
            PoolFreezeError::NoAdmissibleEntry {
                pool: "default".to_owned(),
                permitted: 1,
            }
        );

        // A pool that permits nothing at all is refused for the same reason and says so.
        assert!(matches!(
            FrozenPool::freeze("default", Vec::new(), digest()),
            Err(PoolFreezeError::NoAdmissibleEntry { permitted: 0, .. })
        ));
    }

    /// A snapshot whose ignition entry was not produced by the rule is refused where it is read.
    #[test]
    fn a_snapshot_ignites_where_the_rule_says_or_it_is_refused() {
        let mut frozen = FrozenPool::freeze(
            "default",
            vec![
                FrozenEntry::admissible("anthropic", "claude-code", "claude-opus-5"),
                FrozenEntry::admissible("openai", "codex", "gpt-5"),
            ],
            digest(),
        )
        .expect("the pool freezes");
        frozen.origin = EntryIdentity::new("openai", "codex", "gpt-5");

        assert!(matches!(
            frozen.validate(),
            Err(PoolFreezeError::OriginIsNotTheFirstLiveEntry { .. })
        ));
    }

    /// The digest is the pool's own and is carried, not recomputed: a value that is not a digest is
    /// refused rather than stored as one.
    #[test]
    fn the_digest_a_snapshot_carries_is_a_digest() {
        assert_eq!(
            FrozenPool::freeze(
                "default",
                vec![FrozenEntry::admissible(
                    "anthropic",
                    "claude-code",
                    "claude-opus-5"
                )],
                "not-a-digest",
            ),
            Err(PoolFreezeError::InvalidDigest)
        );
    }
}

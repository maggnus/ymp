//! Immutable results, the ancestry that produced them, and the rules that keep competing results
//! competing.
//!
//! A candidate here is a result together with its construction: the exact base it was built from,
//! the exact object standing at every path it changed, the bundle those changes were published as,
//! and every candidate, obligation and participant that contributed to it. Its identifier is the
//! digest of all of that and of nothing else, so it depends on no workspace directory, no process,
//! no branch name and no row in any store. The same construction stated on another host yields the
//! same identifier, and any difference in it yields another.
//!
//! Nothing here reads what a change means. A path is compared with a path and a digest with a
//! digest. Where two contributions put different bytes at one path, the kernel can state that they
//! disagree and cannot say which is right: settling that is work for a participant, and this module
//! gives it no way to be settled by anything else.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// The largest number of path changes one bundle may carry.
///
/// This bound and the two below it are what keep the facts of one submission inside a single
/// durable record: a bundle and the result formed from it are committed together, and a journal
/// record holds 64 KiB. They are a bound on how much one submission states at once and not on how
/// large a tree may be — a tree of any size is named by the digest of its base.
pub const MAX_BUNDLE_CHANGES: usize = 32;
/// The largest number of path changes the result of one bundle may carry once what its
/// contributions already agreed on is carried forward with it.
pub const MAX_CANDIDATE_CHANGES: usize = 48;
/// The largest number of candidates one bundle may carry forward, and the largest number one
/// recorded disagreement may name.
pub const MAX_BUNDLE_PARENTS: usize = 4;
/// The longest path a change may name, in bytes of its UTF-8 form. Bytes rather than characters,
/// because what the bound exists for is the size of the record the path is committed in.
pub const MAX_PATH_BYTES: usize = 256;

/// What a change puts at one path. `Delete` states that the path holds nothing, which is a stated
/// value like any other: a contribution that deletes a path and one that rewrites it disagree.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum PathChange {
    Upsert {
        /// The digest of the exact bytes that stand at the path. The kernel stores it, compares it
        /// and never opens it.
        object_digest: String,
        executable: bool,
    },
    Delete,
}

/// One path and what a bundle puts there.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleChange {
    pub path: String,
    pub change: PathChange,
}

/// The immutable content of one submission bundle.
///
/// The record carries no participant, no contract and no time: those belong to the result formed
/// from it, not to the bytes. Two participants that publish the same changes against the same base
/// publish one bundle, because a bundle is its content.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleRecord {
    pub bundle_digest: String,
    pub base_digest: String,
    /// The candidates this bundle carries forward: none for a first attempt, one for a rebase,
    /// several for a synthesis.
    pub parents: Vec<String>,
    pub changes: Vec<BundleChange>,
}

impl BundleRecord {
    /// The identity of a bundle: the digest of the base it applies to, the candidates it carries
    /// forward, and what it puts at each path.
    pub fn identify(base_digest: &str, parents: &[String], changes: &[BundleChange]) -> String {
        digest_of(&("bundle-v1", base_digest, parents, changes))
    }

    /// The identity these bytes would have, recomputed from the record itself.
    pub fn recomputed_digest(&self) -> String {
        Self::identify(&self.base_digest, &self.parents, &self.changes)
    }
}

/// What one contributing candidate brought into a result, stated so that the contribution can be
/// followed back to the work that produced it without reading anything else.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Contribution {
    pub candidate_digest: String,
    pub obligation_id: String,
    pub participant: String,
    pub bundle_digest: String,
}

/// One immutable result and the construction that produced it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CandidateRecord {
    pub candidate_digest: String,
    /// The digest of the result itself: the base and the exact objects standing at every path the
    /// result changed. Two participants that produce the same bytes from the same base agree here
    /// and remain two candidates, because a candidate is a construction and not only a tree.
    pub content_digest: String,
    pub contract_id: String,
    pub obligation_id: String,
    pub participant: String,
    pub generation: u64,
    pub base_digest: String,
    pub bundle_digest: String,
    pub contributions: Vec<Contribution>,
    /// The whole change set relative to `base_digest`: what this bundle stated, over what its
    /// contributions already agreed on.
    pub changes: Vec<BundleChange>,
}

impl CandidateRecord {
    /// The identity of a result: its base and the exact objects it puts at every path.
    pub fn identify_content(base_digest: &str, changes: &[BundleChange]) -> String {
        digest_of(&("candidate-content-v1", base_digest, changes))
    }

    /// The identity of a construction: the result it reached, the work it was performed under, and
    /// everything that contributed to it.
    pub fn identify(
        content_digest: &str,
        contract_id: &str,
        obligation_id: &str,
        participant: &str,
        generation: u64,
        bundle_digest: &str,
        contributions: &[Contribution],
    ) -> String {
        digest_of(&(
            "candidate-v1",
            content_digest,
            contract_id,
            obligation_id,
            participant,
            generation,
            bundle_digest,
            contributions,
        ))
    }

    /// The result digest these recorded facts reach, recomputed from the record itself.
    pub fn recomputed_content_digest(&self) -> String {
        Self::identify_content(&self.base_digest, &self.changes)
    }

    /// The identifier these recorded facts carry, recomputed from the record itself. A record whose
    /// stated identifier is not this one states a construction it did not perform.
    pub fn recomputed_digest(&self) -> String {
        Self::identify(
            &self.content_digest,
            &self.contract_id,
            &self.obligation_id,
            &self.participant,
            self.generation,
            &self.bundle_digest,
            &self.contributions,
        )
    }

    /// Whether the record reproduces both of its own digests.
    pub fn states_its_own_identity(&self) -> bool {
        self.recomputed_content_digest() == self.content_digest
            && self.recomputed_digest() == self.candidate_digest
    }
}

/// A recorded disagreement between candidates: the paths at which they put different bytes.
///
/// It is evidence and nothing else. Recording it takes no side, closes no candidate and creates no
/// work; what it gives the collective is an identifier a participant can fund an offer against.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConflictRecord {
    pub conflict_digest: String,
    pub base_digest: String,
    pub candidates: Vec<String>,
    pub paths: Vec<String>,
}

impl ConflictRecord {
    pub fn identify(base_digest: &str, candidates: &[String], paths: &[String]) -> String {
        digest_of(&("conflict-v1", base_digest, candidates, paths))
    }
}

/// The change set keyed by path, which is the form every comparison here is made in.
pub(crate) fn change_map(changes: &[BundleChange]) -> BTreeMap<String, PathChange> {
    changes
        .iter()
        .map(|change| (change.path.clone(), change.change.clone()))
        .collect()
}

/// The change set as a fact states it: one entry per path, in path order, so that two kernels
/// deciding the same command state the same bytes.
pub(crate) fn ordered_changes(map: BTreeMap<String, PathChange>) -> Vec<BundleChange> {
    map.into_iter()
        .map(|(path, change)| BundleChange { path, change })
        .collect()
}

/// What the contributions agree on, and where they do not.
///
/// A path only one contribution mentions is agreed: the others said nothing about it, and saying
/// nothing is not disagreeing. A path two contributions state differently is disputed, and it is
/// left out of the agreed set entirely rather than resolved in favour of either.
pub(crate) fn merge_contributions(
    contributions: &[&CandidateRecord],
) -> (BTreeMap<String, PathChange>, Vec<String>) {
    let mut agreed: BTreeMap<String, PathChange> = BTreeMap::new();
    let mut disputed: BTreeSet<String> = BTreeSet::new();
    for candidate in contributions {
        for change in &candidate.changes {
            match agreed.get(&change.path) {
                Some(stated) if *stated == change.change => {}
                Some(_) => {
                    disputed.insert(change.path.clone());
                }
                None => {
                    agreed.insert(change.path.clone(), change.change.clone());
                }
            }
        }
    }
    for path in &disputed {
        agreed.remove(path);
    }
    (agreed, disputed.into_iter().collect())
}

fn digest_of<T: Serialize>(value: &T) -> String {
    crate::digest_bytes(
        &serde_json::to_vec(value).expect("candidate ancestry is representable as committed data"),
    )
}

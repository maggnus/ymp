//! Knowledge is a projection of existing result/evidence records, never another truth store.
use crate::ConfirmationStatus;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

/// An exact stored lifecycle/content version, not a policy-supplied claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeRef {
    pub id: String,
    pub version: String,
}

/// The trusted client declares that a captured input replaces this old input.
/// The runtime resolves both snapshots; no policy supplies bytes or digests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeSourceReplacement {
    pub previous_input: PathBuf,
    pub replacement_input: PathBuf,
}

/// Part of the trusted acceptance contract and its immutable review context.
/// The named criteria must establish this correction, not merely an unrelated output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeCorrectionBinding {
    #[serde(default)]
    pub projection: KnowledgeProjection,
    pub target: KnowledgeRef,
    pub applicability: BTreeMap<String, String>,
    pub criterion_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_replacement: Option<KnowledgeSourceReplacement>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeProjection {
    #[default]
    ProjectOutcome,
    CheckProcedure,
}
impl KnowledgeProjection {
    pub fn proposal(self) -> KnowledgeProposal {
        match self {
            Self::ProjectOutcome => KnowledgeProposal::ProjectOutcome,
            Self::CheckProcedure => KnowledgeProposal::CheckProcedure,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeCorrectionProposal {
    pub target: KnowledgeRef,
    pub acceptance_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeCorrectionCommit {
    pub target: KnowledgeRef,
    pub replacement_id: String,
    pub acceptance_id: String,
    pub contract_id: String,
    pub policy: KnowledgePolicyIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum KnowledgeCorrectionOutcome {
    Applied {
        correction: KnowledgeCorrectionCommit,
    },
    AlreadyApplied {
        correction: KnowledgeCorrectionCommit,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeAvailability {
    Available,
    Superseded,
    Retired,
    Rejected,
    PendingCorrection,
    ScopeMismatch,
    Unconfirmed,
    SourceVersionChanged,
    SourceUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeInspection {
    /// Retained by bounded transport projections even if the entry is truncated.
    pub id: String,
    pub entry: crate::MemoryEntry,
    pub version: String,
    pub availability: KnowledgeAvailability,
    pub replaced_by: Option<String>,
    pub correction: Option<KnowledgeCorrectionCommit>,
}

pub fn validate_knowledge_scope(scope: &BTreeMap<String, String>) -> anyhow::Result<()> {
    anyhow::ensure!(
        scope.len() <= 16
            && scope.iter().all(|(k, v)| !k.trim().is_empty()
                && k.len() <= 128
                && !v.trim().is_empty()
                && v.len() <= 1024),
        "Knowledge scope requires at most 16 nonempty bounded key/value pairs"
    );
    Ok(())
}

/// Supported is the automatic-context default. Inspection is an explicit client
/// choice that includes candidates/legacy context, with its confirmation label.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeRetrievalMode {
    #[default]
    Supported,
    IncludeUnconfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgePolicyIdentity {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeSource {
    pub acceptance_id: String,
    pub result_id: String,
    pub result_version: usize,
    pub criteria_version: String,
    pub confirmation_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeProvenance {
    pub confirmation: ConfirmationStatus,
    /// Additional exact-match applicability constraints. Missing values do not match.
    pub applicability: BTreeMap<String, String>,
    pub source: Option<KnowledgeSource>,
    pub assignment_id: Option<String>,
    pub invocation_id: Option<String>,
    pub policy: KnowledgePolicyIdentity,
}

/// Free text cannot inherit confirmation from a linked result. Only the runtime
/// can render a confirmed projection from that result's captured evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KnowledgeProposal {
    ProjectOutcome,
    CheckProcedure,
    Candidate { title: String, content: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeArtifact {
    pub path: PathBuf,
    pub sha256: Option<String>,
}

/// Historical accepted outcome with current freshness, available without inference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredOutcome {
    pub source_session: String,
    pub acceptance_id: String,
    pub result_id: String,
    pub result_version: usize,
    pub directory: PathBuf,
    pub summary: String,
    pub artifacts: Vec<OutcomeArtifact>,
    pub confirmation: ConfirmationStatus,
    pub current: bool,
}

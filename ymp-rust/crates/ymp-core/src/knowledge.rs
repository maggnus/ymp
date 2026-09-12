//! Knowledge is a projection of existing result/evidence records, never another truth store.
use crate::ConfirmationStatus;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

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

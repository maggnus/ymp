//! Replaceable selection/proposal algorithms; storage and runtime retain authority.
use anyhow::Result;
use ymp_core::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeCandidateSource {
    Fts5,
    Inventory,
}

#[derive(Debug, Clone)]
pub struct MemorySelection {
    pub id: String,
    pub version: Option<String>,
}

pub struct KnowledgeRetrievalInput<'a> {
    pub query: &'a str,
    /// Read-only candidates. Runtime resolves selected IDs again before use.
    pub candidates: &'a [MemoryEntry],
}

pub trait KnowledgeRetrievalPolicy: Send + Sync {
    fn identity(&self) -> KnowledgePolicyIdentity;
    fn candidate_source(&self) -> KnowledgeCandidateSource;
    fn select(&self, input: KnowledgeRetrievalInput<'_>) -> Result<Vec<MemorySelection>>;
}

#[derive(Default)]
pub struct FtsKnowledgeRetrieval;
impl KnowledgeRetrievalPolicy for FtsKnowledgeRetrieval {
    fn identity(&self) -> KnowledgePolicyIdentity {
        KnowledgePolicyIdentity {
            id: "ymp.fts5".into(),
            version: "1".into(),
        }
    }
    fn candidate_source(&self) -> KnowledgeCandidateSource {
        KnowledgeCandidateSource::Fts5
    }
    fn select(&self, input: KnowledgeRetrievalInput<'_>) -> Result<Vec<MemorySelection>> {
        input
            .candidates
            .iter()
            .map(|entry| {
                Ok(MemorySelection {
                    id: entry.id.clone(),
                    version: Some(content_digest(&serde_json::to_string(entry)?)),
                })
            })
            .collect()
    }
}

pub struct KnowledgeProposalInput<'a> {
    pub acceptance: &'a DecisionRecord,
    pub reviewer_lesson: Option<&'a str>,
}

pub trait KnowledgeProposalPolicy: Send + Sync {
    fn identity(&self) -> KnowledgePolicyIdentity;
    fn propose(&self, input: KnowledgeProposalInput<'_>) -> Result<Vec<KnowledgeProposal>>;
}

pub struct KnowledgeCorrectionInput<'a> {
    pub acceptance: &'a DecisionRecord,
    pub binding: Option<&'a KnowledgeCorrectionBinding>,
}

pub trait KnowledgeCorrectionPolicy: Send + Sync {
    fn identity(&self) -> KnowledgePolicyIdentity;
    fn propose(
        &self,
        input: KnowledgeCorrectionInput<'_>,
    ) -> Result<Vec<KnowledgeCorrectionProposal>>;
}

#[derive(Default)]
pub struct BoundKnowledgeCorrections;
impl KnowledgeCorrectionPolicy for BoundKnowledgeCorrections {
    fn identity(&self) -> KnowledgePolicyIdentity {
        KnowledgePolicyIdentity {
            id: "ymp.bound-correction".into(),
            version: "1".into(),
        }
    }
    fn propose(
        &self,
        input: KnowledgeCorrectionInput<'_>,
    ) -> Result<Vec<KnowledgeCorrectionProposal>> {
        Ok(input
            .binding
            .into_iter()
            .map(|binding| KnowledgeCorrectionProposal {
                target: binding.target.clone(),
                acceptance_id: input.acceptance.id.clone(),
            })
            .collect())
    }
}

#[derive(Default)]
pub struct EvidenceKnowledgeProposals;
impl KnowledgeProposalPolicy for EvidenceKnowledgeProposals {
    fn identity(&self) -> KnowledgePolicyIdentity {
        KnowledgePolicyIdentity {
            id: "ymp.evidence-projection".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, input: KnowledgeProposalInput<'_>) -> Result<Vec<KnowledgeProposal>> {
        let mut proposals = vec![KnowledgeProposal::ProjectOutcome];
        if input
            .acceptance
            .links
            .result
            .as_ref()
            .is_some_and(|r| r.contract_id.is_some())
        {
            proposals.push(KnowledgeProposal::CheckProcedure);
        }
        if let Some(lesson) = input.reviewer_lesson.filter(|s| !s.trim().is_empty()) {
            proposals.push(KnowledgeProposal::Candidate {
                title: "Reviewer lesson (unconfirmed)".into(),
                content: lesson.into(),
            });
        }
        Ok(proposals)
    }
}

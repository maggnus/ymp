//! Typed event families. New families receive explicit replay rules and versions.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use ymp_domain::journal::{Decision, Method, PolicySelection};
use ymp_domain::{
    Digest, Ref, Result,
    task::{AcceptanceContract, Assumption, Clarification, Criterion, Task},
};

/// Explicit payload content, separate from domain Ref versions. Feature owners
/// add their referenced snapshot/output digests here; storage does not guess them.
#[derive(Default)]
pub struct EventContent {
    pub attached: BTreeMap<Digest, Vec<u8>>,
    pub required: BTreeSet<Digest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriteriaCommitted {
    pub task: Task,
    pub contract: AcceptanceContract,
    pub criteria: Vec<Criterion>,
    pub previous: Option<Ref>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Event {
    SessionChanged {
        version: u32,
        change: crate::session::SessionChange,
    },
    FinalizationRecorded {
        version: u32,
        data: Box<crate::finalization::FinalizationRecorded>,
    },
    ProgressRecorded {
        version: u32,
        data: Box<crate::progress::ProgressRecorded>,
    },
    PlanningRecorded {
        version: u32,
        data: Box<crate::plans::PlanningRecorded>,
    },
    LedgerUpdated {
        version: u32,
        data: Box<crate::ledger::LedgerRecorded>,
    },
    AcceptanceRecorded {
        version: u32,
        data: Box<crate::acceptance::AcceptanceRecorded>,
    },
    EvidenceRecorded {
        version: u32,
        data: Box<crate::acceptance::EvidenceRecorded>,
    },
    PaidReviewRecorded {
        version: u32,
        data: Box<crate::acceptance::PaidReviewRecorded>,
    },
    ReviewRecorded {
        version: u32,
        data: Box<crate::acceptance::ReviewRecorded>,
    },
    PlanCommitted {
        version: u32,
        data: Box<crate::results::PlanRecord>,
    },
    AttemptStarted {
        version: u32,
        data: Box<crate::results::AttemptRecord>,
    },
    ResultSubmitted {
        version: u32,
        attempt: ymp_domain::Id<ymp_domain::plan::Attempt>,
        result: Box<ymp_domain::result::ResultVersion>,
    },
    AttemptAbandoned {
        version: u32,
        attempt: ymp_domain::Id<ymp_domain::plan::Attempt>,
        reason: String,
    },
    InvocationStarted {
        version: u32,
        invocation: ymp_domain::assignment::Invocation,
    },
    InvocationObserved {
        version: u32,
        invocation: ymp_domain::Id<ymp_domain::assignment::Invocation>,
        observation: Box<crate::execution::InvocationObservation>,
    },
    InvocationEnded {
        version: u32,
        invocation: ymp_domain::Id<ymp_domain::assignment::Invocation>,
        terminal: ymp_domain::assignment::InvocationTerminal,
        confirmed: bool,
        ended: u64,
    },
    CheckRegistered {
        version: u32,
        data: Box<crate::acceptance::CheckRegistered>,
    },
    CheckRunRecorded {
        version: u32,
        data: Box<crate::acceptance::CheckRunRecorded>,
    },
    AssignmentRevoked {
        version: u32,
        assignment: ymp_domain::Id<ymp_domain::assignment::Assignment>,
        reason: String,
    },
    GrantIssued {
        version: u32,
        nonce: Digest,
        grant: ymp_domain::assignment::Grant,
    },
    AssignmentAdmitted {
        version: u32,
        nonce: Digest,
        assignment: ymp_domain::Id<ymp_domain::assignment::Assignment>,
    },
    ContributionProposed {
        version: u32,
        contribution: Box<ymp_domain::assignment::Contribution>,
    },
    SolicitationOpened {
        version: u32,
        solicitation: Box<ymp_domain::coordination::Solicitation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        predecessor: Option<crate::arbiter::CommitmentLink>,
    },
    OfferSubmitted {
        version: u32,
        offer: Box<ymp_domain::coordination::Offer>,
    },
    Awarded {
        version: u32,
        data: Box<crate::arbiter::Awarded>,
    },
    CommitmentChanged {
        version: u32,
        change: crate::arbiter::CommitmentChange,
    },
    WorkspaceBound {
        version: u32,
        workspace: ymp_domain::Id<ymp_domain::workspace::Workspace>,
        binding: Box<ymp_domain::workspace::WorkspaceBinding>,
    },
    LockChanged {
        version: u32,
        change: crate::workspace_locks::LockChange,
    },
    WorkspaceOpened {
        version: u32,
        workspace: Box<ymp_domain::workspace::Workspace>,
    },
    SnapshotTaken {
        version: u32,
        snapshot: Box<ymp_domain::workspace::Snapshot>,
    },
    BudgetOpened {
        version: u32,
        data: Box<crate::treasury::BudgetOpening>,
    },
    ReservationChanged {
        version: u32,
        change: crate::treasury::ReservationChange,
    },
    ReceiptSettled {
        version: u32,
        data: Box<crate::treasury::Settlement>,
    },
    ReportingStarted {
        version: u32,
        mode: ymp_domain::resources::ReportingMode,
    },
    PoolRecorded {
        version: u32,
        data: Box<crate::registry::PoolRecorded>,
    },
    SessionOpened {
        version: u32,
        selections: Vec<PolicySelection>,
    },
    MethodChosen {
        version: u32,
        decision: Box<Decision<Method>>,
    },
    CriteriaCommitted {
        version: u32,
        data: Box<CriteriaCommitted>,
    },
    ClarificationRecorded {
        version: u32,
        clarification: Clarification,
    },
    AssumptionRecorded {
        version: u32,
        assumption: Assumption,
    },
}

impl Event {
    pub fn contents(&self) -> Result<EventContent> {
        let selections: Vec<&PolicySelection> = match self {
            Self::SessionChanged { .. } => vec![],
            Self::FinalizationRecorded { data, .. } => data.selection().into_iter().collect(),
            Self::ProgressRecorded { data, .. } => data.selection().into_iter().collect(),
            Self::PlanningRecorded { data, .. } => data.selection().into_iter().collect(),
            Self::LedgerUpdated { data, .. } => {
                data.decisions.values().map(|d| &d.effective).collect()
            }
            Self::AcceptanceRecorded { data, .. } => vec![&data.credit.effective],
            Self::EvidenceRecorded { .. }
            | Self::ReviewRecorded { .. }
            | Self::PaidReviewRecorded { .. } => vec![],
            Self::PlanCommitted { .. }
            | Self::AttemptStarted { .. }
            | Self::ResultSubmitted { .. }
            | Self::AttemptAbandoned { .. } => vec![],
            Self::InvocationObserved { observation, .. } => match observation.as_ref() {
                crate::execution::InvocationObservation::Dispatch(data) => vec![&data.backend],
                crate::execution::InvocationObservation::Cost { decision, .. } => {
                    vec![&decision.effective]
                }
                _ => vec![],
            },
            Self::InvocationStarted { .. } | Self::InvocationEnded { .. } => vec![],
            Self::CheckRegistered { data, .. } => vec![&data.effective],
            Self::CheckRunRecorded { data, .. } => vec![&data.environment.runner],
            Self::AssignmentRevoked { .. }
            | Self::GrantIssued { .. }
            | Self::AssignmentAdmitted { .. }
            | Self::ContributionProposed { .. }
            | Self::SolicitationOpened { .. }
            | Self::OfferSubmitted { .. }
            | Self::CommitmentChanged { .. } => vec![],
            Self::Awarded { data, .. } => vec![&data.decision.effective],
            Self::LockChanged { .. } | Self::WorkspaceBound { .. } => vec![],
            Self::WorkspaceOpened { workspace, .. } => vec![&workspace.provider],
            Self::SnapshotTaken { .. } => vec![],
            Self::BudgetOpened { data, .. } => vec![&data.reporting.effective],
            Self::ReservationChanged {
                change: crate::treasury::ReservationChange::Reserved(data),
                ..
            } => vec![&data.estimate.effective, &data.allocation.effective],
            Self::ReceiptSettled { data, .. } => vec![&data.decision.effective],
            Self::ReportingStarted { .. } | Self::ReservationChanged { .. } => vec![],
            Self::SessionOpened { selections, .. } => selections.iter().collect(),
            Self::MethodChosen { decision, .. } => vec![&decision.effective],
            Self::PoolRecorded { data, .. } => vec![&data.effective],
            Self::CriteriaCommitted { .. }
            | Self::ClarificationRecorded { .. }
            | Self::AssumptionRecorded { .. } => vec![],
        };
        let mut content = EventContent::default();
        if let Self::FinalizationRecorded { data, .. } = self
            && let crate::finalization::FinalizationRecorded::Captured(aggregate) = data.as_ref()
        {
            for check in &aggregate.checks {
                let bytes = ymp_domain::journal::encode(&check.environment)?;
                let digest = Digest::of(&bytes);
                content.required.insert(digest.clone());
                content.attached.insert(digest, bytes);
            }
        }
        if let Self::EvidenceRecorded { data, .. } = self
            && let Some(environment) = &data.scope.environment
        {
            content.required.insert(environment.clone());
        }
        if let Self::ResultSubmitted { result, .. } = self {
            content
                .required
                .extend(result.artifacts.iter().map(|a| a.digest.clone()));
        }
        if let Self::CheckRunRecorded { data, .. } = self {
            let bytes = ymp_domain::journal::encode(&data.environment)?;
            content.required.extend([
                data.run.stdout.clone(),
                data.run.stderr.clone(),
                data.run.env.clone(),
            ]);
            content.attached.insert(data.run.env.clone(), bytes);
        }
        for selection in selections {
            selection.validate()?;
            let bytes = ymp_domain::journal::encode(&selection.parameters)?;
            content.required.insert(selection.policy.params.clone());
            content
                .attached
                .insert(selection.policy.params.clone(), bytes);
        }
        if let Self::SnapshotTaken { snapshot, .. } = self {
            let bytes = ymp_domain::journal::encode(&snapshot.tree)?;
            let digest = snapshot.tree.digest()?;
            content.required.insert(digest.clone());
            content.attached.insert(digest, bytes);
            content
                .required
                .extend(snapshot.tree.files.values().map(|file| file.digest.clone()));
        }
        Ok(content)
    }
    pub fn version(&self) -> u32 {
        match self {
            Self::SessionChanged { version, .. }
            | Self::FinalizationRecorded { version, .. }
            | Self::ProgressRecorded { version, .. }
            | Self::PlanningRecorded { version, .. }
            | Self::LedgerUpdated { version, .. }
            | Self::AcceptanceRecorded { version, .. } => *version,
            Self::EvidenceRecorded { version, .. }
            | Self::ReviewRecorded { version, .. }
            | Self::PaidReviewRecorded { version, .. } => *version,
            Self::PlanCommitted { version, .. }
            | Self::AttemptStarted { version, .. }
            | Self::ResultSubmitted { version, .. }
            | Self::AttemptAbandoned { version, .. } => *version,
            Self::InvocationStarted { version, .. }
            | Self::InvocationObserved { version, .. }
            | Self::InvocationEnded { version, .. } => *version,
            Self::CheckRegistered { version, .. } | Self::CheckRunRecorded { version, .. } => {
                *version
            }
            Self::AssignmentRevoked { version, .. }
            | Self::GrantIssued { version, .. }
            | Self::AssignmentAdmitted { version, .. }
            | Self::ContributionProposed { version, .. }
            | Self::SolicitationOpened { version, .. }
            | Self::OfferSubmitted { version, .. }
            | Self::Awarded { version, .. }
            | Self::CommitmentChanged { version, .. }
            | Self::WorkspaceBound { version, .. }
            | Self::LockChanged { version, .. }
            | Self::WorkspaceOpened { version, .. }
            | Self::SnapshotTaken { version, .. }
            | Self::BudgetOpened { version, .. }
            | Self::ReservationChanged { version, .. }
            | Self::ReceiptSettled { version, .. }
            | Self::ReportingStarted { version, .. }
            | Self::PoolRecorded { version, .. }
            | Self::SessionOpened { version, .. }
            | Self::MethodChosen { version, .. }
            | Self::CriteriaCommitted { version, .. }
            | Self::ClarificationRecorded { version, .. }
            | Self::AssumptionRecorded { version, .. } => *version,
        }
    }
}

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
    ContributionProposed {
        version: u32,
        contribution: Box<ymp_domain::assignment::Contribution>,
    },
    SolicitationOpened {
        version: u32,
        solicitation: Box<ymp_domain::coordination::Solicitation>,
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
            Self::ContributionProposed { .. }
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
            Self::ContributionProposed { version, .. }
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

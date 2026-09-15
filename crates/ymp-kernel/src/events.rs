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
        Ok(content)
    }
    pub fn version(&self) -> u32 {
        match self {
            Self::BudgetOpened { version, .. }
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

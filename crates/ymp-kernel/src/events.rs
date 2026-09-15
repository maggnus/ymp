//! Typed event families. New families receive explicit replay rules and versions.

use serde::{Deserialize, Serialize};
use ymp_domain::journal::{Decision, Method, PolicySelection};
use ymp_domain::{
    Ref,
    task::{AcceptanceContract, Assumption, Clarification, Criterion, Task},
};

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
    pub fn version(&self) -> u32 {
        match self {
            Self::SessionOpened { version, .. }
            | Self::MethodChosen { version, .. }
            | Self::CriteriaCommitted { version, .. }
            | Self::ClarificationRecorded { version, .. }
            | Self::AssumptionRecorded { version, .. } => *version,
        }
    }
}

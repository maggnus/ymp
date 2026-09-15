//! Typed event families. New families receive explicit replay rules and versions.

use serde::{Deserialize, Serialize};
use ymp_domain::journal::{Decision, Method, PolicySelection};

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
}

impl Event {
    pub fn version(&self) -> u32 {
        match self {
            Self::SessionOpened { version, .. } | Self::MethodChosen { version, .. } => *version,
        }
    }
}

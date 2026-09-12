use super::{AgentProfile, ProviderCapabilities};
use crate::Session;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPool {
    /// Includes excluded profiles so callers can explain local eligibility.
    pub agents: Vec<PoolAgent>,
    pub capabilities: BTreeMap<String, ProviderCapabilities>,
}

impl AgentPool {
    pub fn eligible(&self) -> impl Iterator<Item = &PoolAgent> {
        self.agents.iter().filter(|a| a.exclusions.is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolAgent {
    pub profile: AgentProfile,
    pub profile_version: String,
    pub exclusions: Vec<PoolExclusion>,
    pub model_status: PoolModelStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolExclusion {
    AgentDisabled,
    ProviderDisabled,
    ExecutableMissing,
    ModelUnlisted,
    NoModelsAvailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolModelStatus {
    /// The profile lets the native installation choose; no model is resolved here.
    InheritedDefault,
    Listed,
    Unlisted,
    Unknown,
}

/// References supplied by the live runtime, never inferred from saved contexts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentInvocationRef {
    pub invocation_id: String,
    pub session_id: String,
    pub agent_id: String,
}

/// Read-only identity projection. Membership changes remain runtime-owned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAgentView {
    pub session_id: String,
    pub captured_participants: Vec<AgentProfile>,
    /// None means current membership was not supplied; it is not an empty team.
    pub current_members: Option<Vec<String>>,
    /// None means live activity is unknown, including after a process restart.
    pub active_invocations: Option<Vec<AgentInvocationRef>>,
}

impl SessionAgentView {
    pub fn from_captured(
        session: &Session,
        current_members: Option<&[String]>,
        active_invocations: Option<&[AgentInvocationRef]>,
    ) -> Result<Self> {
        let mut participants = HashSet::new();
        if session.id.trim().is_empty()
            || session
                .team
                .iter()
                .any(|a| a.id.trim().is_empty() || !participants.insert(a.id.as_str()))
        {
            bail!("Captured session and participant identities must be nonempty and unique");
        }
        let mut current = HashSet::new();
        for id in current_members.into_iter().flatten() {
            if !participants.contains(id.as_str()) || !current.insert(id.as_str()) {
                bail!("Current member must be a unique captured participant");
            }
        }
        let mut invocations = HashSet::new();
        for invocation in active_invocations.into_iter().flatten() {
            if invocation.session_id != session.id
                || invocation.invocation_id.trim().is_empty()
                || !invocations.insert(&invocation.invocation_id)
                || !participants.contains(invocation.agent_id.as_str())
                || (current_members.is_some() && !current.contains(invocation.agent_id.as_str()))
            {
                bail!("Active invocation must identify this session and a current participant");
            }
        }
        Ok(Self {
            session_id: session.id.clone(),
            captured_participants: session.team.clone(),
            current_members: current_members.map(<[_]>::to_vec),
            active_invocations: active_invocations.map(<[_]>::to_vec),
        })
    }

    /// Resolve chat/task authors against their captured session, not today's pool.
    pub fn captured_name(&self, agent_id: &str) -> Option<&str> {
        self.captured_participants
            .iter()
            .find(|agent| agent.id == agent_id)
            .map(|agent| agent.name.as_str())
    }
}

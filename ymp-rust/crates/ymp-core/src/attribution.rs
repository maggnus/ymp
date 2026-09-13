//! Read-only attribution from exact persisted invocation links.
use crate::{
    AgentIdentity, ExecutionSettings, InvocationState, Message, ProvenanceEvent, SessionTrace,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageOrigin {
    pub message_seq: i64,
    pub agent_id: String,
    pub assignment_id: String,
    pub invocation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAttribution {
    pub agent_id: String,
    pub provider_id: String,
    pub assignment_id: String,
    pub invocation_id: String,
    pub identity: Option<AgentIdentity>,
    pub requested: ExecutionSettings,
    pub sent: ExecutionSettings,
    pub reported: ExecutionSettings,
}

impl SessionTrace {
    fn attributed_invocation(&self, origin: &MessageOrigin) -> Option<AgentAttribution> {
        let assignment = self.assignments.iter().find(|a| {
            a.id == origin.assignment_id
                && a.session_id == self.session.id
                && a.agent_id == origin.agent_id
        })?;
        let invocation = self.invocations.iter().find(|i| {
            i.id == origin.invocation_id
                && i.session_id == self.session.id
                && i.assignment_id == assignment.id
        })?;
        Some(AgentAttribution {
            agent_id: assignment.agent_id.clone(),
            provider_id: assignment.provider_id.clone(),
            assignment_id: assignment.id.clone(),
            invocation_id: invocation.id.clone(),
            identity: assignment.agent_identity.clone(),
            requested: invocation.requested.clone(),
            sent: invocation.sent.clone(),
            reported: invocation.reported.clone(),
        })
    }

    /// Never infer a historical model from the actor's most recent invocation.
    pub fn message_attribution(&self, message: &Message) -> Option<AgentAttribution> {
        if message.session_id != self.session.id || matches!(message.author.as_str(), "you" | "ymp")
        {
            return None;
        }
        let mut found: Option<AgentAttribution> = None;
        for event in self.history.iter().filter(|e| {
            e.session_id == message.session_id
                && e.data
                    .get("message_seq")
                    .and_then(serde_json::Value::as_i64)
                    == Some(message.seq)
        }) {
            let origin = if event.kind == "message_invocation" {
                serde_json::from_value::<MessageOrigin>(event.data.clone()).ok()
            } else if event.kind == "provenance" {
                match serde_json::from_value::<ProvenanceEvent>(event.data.clone()).ok() {
                    Some(ProvenanceEvent::TeamOperationCommitted {
                        message_seq: Some(message_seq),
                        agent_id,
                        assignment_id,
                        invocation_id,
                        ..
                    }) => Some(MessageOrigin {
                        message_seq,
                        agent_id,
                        assignment_id,
                        invocation_id,
                    }),
                    _ => None,
                }
            } else {
                None
            };
            let Some(origin) =
                origin.filter(|o| o.message_seq == message.seq && o.agent_id == message.author)
            else {
                continue;
            };
            let attribution = self.attributed_invocation(&origin)?;
            if found
                .as_ref()
                .is_some_and(|previous| previous.invocation_id != attribution.invocation_id)
            {
                return None;
            }
            found = Some(attribution);
        }
        found
    }

    /// Streaming presentation needs an unambiguous currently running invocation.
    pub fn active_agent_attribution(&self, agent_id: &str) -> Option<AgentAttribution> {
        let mut candidates = self
            .invocations
            .iter()
            .filter(|i| i.state == InvocationState::Running)
            .filter_map(|i| {
                self.attributed_invocation(&MessageOrigin {
                    message_seq: 0,
                    agent_id: agent_id.into(),
                    assignment_id: i.assignment_id.clone(),
                    invocation_id: i.id.clone(),
                })
            });
        let result = candidates.next()?;
        candidates.next().is_none().then_some(result)
    }
}

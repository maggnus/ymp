//! Assignment-scoped team API authority. Records contain audit identities only;
//! possession of a serialized record never creates a live capability.
use crate::{AssignmentRecord, InvocationRecord};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamOperation {
    TeamPost,
    TeamRead,
    TasksList,
    TaskPropose,
    BoardRead,
    MemorySearch,
    MemoryPropose,
}
impl TeamOperation {
    pub fn coordination() -> Vec<Self> {
        vec![
            Self::TeamPost,
            Self::TeamRead,
            Self::TasksList,
            Self::TaskPropose,
            Self::BoardRead,
            Self::MemorySearch,
            Self::MemoryPropose,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantRecord {
    pub schema_version: u32,
    pub id: String,
    pub session_id: String,
    pub agent_id: String,
    pub assignment_id: String,
    pub invocation_id: String,
    pub operations: Vec<TeamOperation>,
    pub issued_at: String,
    pub revoked_at: Option<String>,
    pub revocation_reason: Option<String>,
}
impl GrantRecord {
    pub fn for_assignment(
        assignment: &AssignmentRecord,
        invocation: &InvocationRecord,
        operations: Vec<TeamOperation>,
    ) -> Self {
        Self {
            schema_version: 1,
            id: crate::new_id(),
            session_id: assignment.session_id.clone(),
            agent_id: assignment.agent_id.clone(),
            assignment_id: assignment.id.clone(),
            invocation_id: invocation.id.clone(),
            operations,
            issued_at: crate::now(),
            revoked_at: None,
            revocation_reason: None,
        }
    }
}

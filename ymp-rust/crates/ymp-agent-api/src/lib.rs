#![forbid(unsafe_code)]

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use thiserror::Error;
use ymp_board::{Audience, MessageKind, Reference, Relation};
use ymp_domain::pool::EntryIdentity;

pub const READ_CONTROL_TOOL: &str = "read_control";
pub const READ_EVENTS_TOOL: &str = "read_events";
pub const READ_BOARD_TOOL: &str = "read_board";
pub const PUBLISH_TOOL: &str = "publish";
pub const REQUEST_PARTICIPANT_TOOL: &str = "request_participant";
pub const SUBMIT_TOOL: &str = "submit";
pub const YIELD_TOOL: &str = "yield";
pub const MAX_EVENT_PAGE: u16 = 128;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "tool", content = "arguments", rename_all = "snake_case")]
pub enum AgentToolCall {
    ReadControl,
    ReadEvents(ReadEventsArguments),
    ReadBoard(ReadBoardArguments),
    Publish(PublishArguments),
    RequestParticipant(RequestParticipantArguments),
    Submit(SubmitArguments),
    Yield(YieldArguments),
}

impl AgentToolCall {
    pub fn parse(name: &str, arguments: Value) -> Result<Self, ToolParseError> {
        match name {
            READ_CONTROL_TOOL => {
                serde_json::from_value::<EmptyArguments>(arguments)?;
                Ok(Self::ReadControl)
            }
            READ_EVENTS_TOOL => Ok(Self::ReadEvents(serde_json::from_value(arguments)?)),
            READ_BOARD_TOOL => Ok(Self::ReadBoard(serde_json::from_value(arguments)?)),
            PUBLISH_TOOL => Ok(Self::Publish(serde_json::from_value(arguments)?)),
            REQUEST_PARTICIPANT_TOOL => {
                Ok(Self::RequestParticipant(serde_json::from_value(arguments)?))
            }
            SUBMIT_TOOL => Ok(Self::Submit(serde_json::from_value(arguments)?)),
            YIELD_TOOL => Ok(Self::Yield(serde_json::from_value(arguments)?)),
            _ => Err(ToolParseError::UnknownTool(name.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct EmptyArguments {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadEventsArguments {
    #[serde(default)]
    pub cursor: u64,
    #[serde(default = "default_event_page")]
    pub limit: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadBoardArguments {
    pub limit_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublishArguments {
    pub command_id: String,
    #[serde(deserialize_with = "deserialize_audience")]
    pub audience: Audience,
    pub kind: MessageKind,
    pub content: String,
    pub salience_ms: u64,
    #[serde(deserialize_with = "deserialize_references")]
    pub references: Vec<Reference>,
    #[serde(deserialize_with = "deserialize_relation")]
    pub relation: Relation,
    pub claimed_decision_basis: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestParticipantArguments {
    pub request_id: String,
    #[serde(deserialize_with = "deserialize_entry_identity")]
    pub entry: EntryIdentity,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictEntryIdentity {
    provider: String,
    engine: String,
    model: String,
}

fn deserialize_entry_identity<'de, D>(deserializer: D) -> Result<EntryIdentity, D::Error>
where
    D: Deserializer<'de>,
{
    let entry = StrictEntryIdentity::deserialize(deserializer)?;
    Ok(EntryIdentity::new(
        entry.provider,
        entry.engine,
        entry.model,
    ))
}

#[derive(Deserialize)]
#[serde(tag = "audience", rename_all = "snake_case", deny_unknown_fields)]
enum StrictAudience {
    ProjectDiscovery {},
    Scope {
        scope_id: String,
    },
    Named {
        scope_id: String,
        recipients: Vec<String>,
    },
}

impl From<StrictAudience> for Audience {
    fn from(value: StrictAudience) -> Self {
        match value {
            StrictAudience::ProjectDiscovery {} => Self::ProjectDiscovery,
            StrictAudience::Scope { scope_id } => Self::Scope { scope_id },
            StrictAudience::Named {
                scope_id,
                recipients,
            } => Self::Named {
                scope_id,
                recipients,
            },
        }
    }
}

fn deserialize_audience<'de, D>(deserializer: D) -> Result<Audience, D::Error>
where
    D: Deserializer<'de>,
{
    StrictAudience::deserialize(deserializer).map(Into::into)
}

#[derive(Deserialize)]
#[serde(tag = "reference", rename_all = "snake_case", deny_unknown_fields)]
enum StrictReference {
    Message { message_id: String },
    Artifact { object_digest: String },
    Candidate { candidate_digest: String },
    ControlRecord { record_id: String },
}

impl From<StrictReference> for Reference {
    fn from(value: StrictReference) -> Self {
        match value {
            StrictReference::Message { message_id } => Self::Message { message_id },
            StrictReference::Artifact { object_digest } => Self::Artifact { object_digest },
            StrictReference::Candidate { candidate_digest } => Self::Candidate { candidate_digest },
            StrictReference::ControlRecord { record_id } => Self::ControlRecord { record_id },
        }
    }
}

fn deserialize_references<'de, D>(deserializer: D) -> Result<Vec<Reference>, D::Error>
where
    D: Deserializer<'de>,
{
    Vec::<StrictReference>::deserialize(deserializer)
        .map(|references| references.into_iter().map(Into::into).collect())
}

#[derive(Deserialize)]
#[serde(tag = "relation", rename_all = "snake_case", deny_unknown_fields)]
enum StrictRelation {
    Standalone {},
    ReplyTo { message_id: String },
    Challenges { message_id: String },
    Revises { message_id: String },
    Refreshes { message_id: String },
}

impl From<StrictRelation> for Relation {
    fn from(value: StrictRelation) -> Self {
        match value {
            StrictRelation::Standalone {} => Self::Standalone,
            StrictRelation::ReplyTo { message_id } => Self::ReplyTo { message_id },
            StrictRelation::Challenges { message_id } => Self::Challenges { message_id },
            StrictRelation::Revises { message_id } => Self::Revises { message_id },
            StrictRelation::Refreshes { message_id } => Self::Refreshes { message_id },
        }
    }
}

fn deserialize_relation<'de, D>(deserializer: D) -> Result<Relation, D::Error>
where
    D: Deserializer<'de>,
{
    StrictRelation::deserialize(deserializer).map(Into::into)
}

const fn default_event_page() -> u16 {
    64
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitArguments {
    pub command_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct YieldArguments {
    pub command_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentToolErrorCode {
    InvalidArguments,
    Rejected,
    Internal,
}

#[derive(Clone, Debug, Deserialize, Error, Eq, PartialEq, Serialize)]
#[error("{message}")]
pub struct AgentToolError {
    pub code: AgentToolErrorCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentToolCapabilities {
    pub request_participant: bool,
}

impl AgentToolCapabilities {
    pub const RECRUITMENT: Self = Self {
        request_participant: true,
    };
}

impl AgentToolError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: AgentToolErrorCode::InvalidArguments,
            message: message.into(),
        }
    }

    pub fn rejected(message: impl Into<String>) -> Self {
        Self {
            code: AgentToolErrorCode::Rejected,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: AgentToolErrorCode::Internal,
            message: message.into(),
        }
    }
}

pub trait AgentToolHandler {
    fn capabilities(&mut self) -> Result<AgentToolCapabilities, AgentToolError> {
        Ok(AgentToolCapabilities::default())
    }

    fn call(&mut self, call: AgentToolCall) -> Result<Value, AgentToolError>;
}

#[derive(Debug, Error)]
pub enum ToolParseError {
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    #[error("invalid tool arguments: {0}")]
    InvalidArguments(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::{
        AgentToolCall, MAX_EVENT_PAGE, PublishArguments, ReadBoardArguments, ReadEventsArguments,
        RequestParticipantArguments, SubmitArguments, ToolParseError, YieldArguments,
    };
    use serde_json::json;
    use ymp_board::{Audience, MessageKind, Relation};
    use ymp_domain::pool::EntryIdentity;

    #[test]
    fn tool_arguments_are_strict_and_defaults_are_stable() {
        assert_eq!(
            AgentToolCall::parse("read_events", json!({})).expect("parse defaults"),
            AgentToolCall::ReadEvents(ReadEventsArguments {
                cursor: 0,
                limit: 64
            })
        );
        assert_eq!(MAX_EVENT_PAGE, 128);
        assert_eq!(
            AgentToolCall::parse("submit", json!({ "command_id": "agent.submit" }))
                .expect("parse submission"),
            AgentToolCall::Submit(SubmitArguments {
                command_id: "agent.submit".to_owned()
            })
        );
        assert!(matches!(
            AgentToolCall::parse(
                "submit",
                json!({ "command_id": "agent.submit", "object_digest": "0".repeat(64) })
            ),
            Err(ToolParseError::InvalidArguments(_))
        ));
        assert_eq!(
            AgentToolCall::parse("yield", json!({ "command_id": "agent.yield" }))
                .expect("parse yield"),
            AgentToolCall::Yield(YieldArguments {
                command_id: "agent.yield".to_owned()
            })
        );
        assert!(matches!(
            AgentToolCall::parse("read_control", json!({"unexpected": true})),
            Err(ToolParseError::InvalidArguments(_))
        ));
        assert!(matches!(
            AgentToolCall::parse("unknown", json!({})),
            Err(ToolParseError::UnknownTool(_))
        ));
    }

    #[test]
    fn collaboration_arguments_are_exact_and_carry_no_caller_identity_or_payload_claims() {
        assert_eq!(
            AgentToolCall::parse("read_board", json!({ "limit_bytes": 32_768 }))
                .expect("parse board read"),
            AgentToolCall::ReadBoard(ReadBoardArguments {
                limit_bytes: 32_768
            })
        );
        let publication = json!({
            "command_id": "message-1",
            "audience": { "audience": "project_discovery" },
            "kind": "observation",
            "content": "exact UTF-8 content",
            "salience_ms": 5_000,
            "references": [],
            "relation": { "relation": "standalone" },
            "claimed_decision_basis": []
        });
        assert_eq!(
            AgentToolCall::parse("publish", publication.clone()).expect("parse publication"),
            AgentToolCall::Publish(PublishArguments {
                command_id: "message-1".to_owned(),
                audience: Audience::ProjectDiscovery,
                kind: MessageKind::Observation,
                content: "exact UTF-8 content".to_owned(),
                salience_ms: 5_000,
                references: Vec::new(),
                relation: Relation::Standalone,
                claimed_decision_basis: Vec::new(),
            })
        );

        for forbidden in [
            "author",
            "reader",
            "principal",
            "capability",
            "payload_digest",
            "payload_bytes",
            "workspace",
            "url",
            "protected_oracle",
            "command",
        ] {
            let mut arguments = publication.clone();
            arguments[forbidden] = json!("model-selected");
            assert!(
                matches!(
                    AgentToolCall::parse("publish", arguments),
                    Err(ToolParseError::InvalidArguments(_))
                ),
                "publish accepted forbidden field {forbidden}"
            );
        }
        for forbidden in ["reader", "author", "payload_digest", "payload_bytes"] {
            assert!(matches!(
                AgentToolCall::parse(
                    "read_board",
                    json!({ "limit_bytes": 1, (forbidden): "model-selected" })
                ),
                Err(ToolParseError::InvalidArguments(_))
            ));
        }
        let mut nested = publication.clone();
        nested["audience"]["reader"] = json!("model-selected");
        assert!(matches!(
            AgentToolCall::parse("publish", nested),
            Err(ToolParseError::InvalidArguments(_))
        ));
        let mut nested = publication;
        nested["relation"]["command"] = json!("generic");
        assert!(matches!(
            AgentToolCall::parse("publish", nested),
            Err(ToolParseError::InvalidArguments(_))
        ));
    }

    #[test]
    fn recruitment_arguments_name_only_the_request_and_frozen_entry() {
        let arguments = json!({
            "request_id": "request-1",
            "entry": {
                "provider": "anthropic",
                "engine": "claude-code",
                "model": "claude-opus-5"
            }
        });
        assert_eq!(
            AgentToolCall::parse("request_participant", arguments.clone())
                .expect("parse recruitment request"),
            AgentToolCall::RequestParticipant(RequestParticipantArguments {
                request_id: "request-1".to_owned(),
                entry: EntryIdentity::new("anthropic", "claude-code", "claude-opus-5"),
            })
        );

        for forbidden in [
            "proposer",
            "participant",
            "principal",
            "path",
            "profile",
            "route",
            "workspace",
            "capability",
            "score",
            "rank",
            "role",
        ] {
            let mut forged = arguments.clone();
            forged[forbidden] = json!("model-selected");
            assert!(
                matches!(
                    AgentToolCall::parse("request_participant", forged),
                    Err(ToolParseError::InvalidArguments(_))
                ),
                "recruitment accepted forbidden field {forbidden}"
            );
        }

        let mut nested = arguments;
        nested["entry"]["profile"] = json!("model-selected");
        assert!(matches!(
            AgentToolCall::parse("request_participant", nested),
            Err(ToolParseError::InvalidArguments(_))
        ));
    }
}

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const READ_CONTROL_TOOL: &str = "read_control";
pub const READ_EVENTS_TOOL: &str = "read_events";
pub const SUBMIT_TOOL: &str = "submit";
pub const YIELD_TOOL: &str = "yield";
pub const MAX_EVENT_PAGE: u16 = 128;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "tool", content = "arguments", rename_all = "snake_case")]
pub enum AgentToolCall {
    ReadControl,
    ReadEvents(ReadEventsArguments),
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
        AgentToolCall, MAX_EVENT_PAGE, ReadEventsArguments, SubmitArguments, ToolParseError,
        YieldArguments,
    };
    use serde_json::json;

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
}

//! Durable prototype entities. Frozen launch records do not reference mutable configuration bytes.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    ProviderRequired,
    Clarifying,
    Ready,
    Developing,
    Verifying,
    Review,
    Delivered,
    Stopped,
    Cancelled,
}
impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Self::ProviderRequired => "Provider required",
            Self::Clarifying => "Clarifying",
            Self::Ready => "Ready",
            Self::Developing => "Developing",
            Self::Verifying => "Verifying",
            Self::Review => "Review",
            Self::Delivered => "Delivered",
            Self::Stopped => "Stopped",
            Self::Cancelled => "Cancelled",
        }
    }
    pub fn busy(self) -> bool {
        matches!(self, Self::Developing | Self::Verifying)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Starting,
    Running,
    Waiting,
    Completed,
    Interrupted,
    Failed,
    Stopped,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Starting => "Starting",
            Self::Running => "Running",
            Self::Waiting => "Waiting",
            Self::Completed => "Completed",
            Self::Interrupted => "Interrupted",
            Self::Failed => "Failed",
            Self::Stopped => "Stopped",
        }
    }
    pub fn from_label(s: &str) -> Self {
        match s {
            "Starting" => Self::Starting,
            "Running" => Self::Running,
            "Waiting" => Self::Waiting,
            "Completed" => Self::Completed,
            "Interrupted" => Self::Interrupted,
            "Failed" => Self::Failed,
            _ => Self::Stopped,
        }
    }
}
impl PartialEq<&str> for Status {
    fn eq(&self, s: &&str) -> bool {
        self.label() == *s
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    OpenAi,
    Anthropic,
}
impl Family {
    pub const ALL: [Self; 2] = [Self::OpenAi, Self::Anthropic];
    pub fn name(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
        }
    }
    pub fn engine(self) -> &'static str {
        match self {
            Self::OpenAi => "Codex",
            Self::Anthropic => "Claude Code",
        }
    }
    pub fn model(self) -> &'static str {
        match self {
            Self::OpenAi => "gpt-5.6-terra",
            Self::Anthropic => "claude-opus-5",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub connection: String,
    pub connection_name: String,
    pub provider: String,
    pub engine: String,
    pub model: String,
    pub effort: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub family: Family,
    pub enabled: bool,
    pub checked: Option<u64>,
    pub created: u64,
}
impl Provider {
    pub fn profile(&self) -> Profile {
        Profile {
            connection: self.id.clone(),
            connection_name: self.name.clone(),
            provider: self.family.key().into(),
            engine: self.family.engine().into(),
            model: self.family.model().into(),
            effort: "low".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub author: Option<String>,
    pub text: String,
    pub user: bool,
    #[serde(default)]
    pub tool: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub goal: String,
    pub requirements: String,
    pub created: u64,
    pub stage: Stage,
    pub archived: bool,
    pub run: usize,
    pub run_started: Option<u64>,
    pub run_ended: Option<u64>,
    pub step: usize,
    pub next_step: u64,
    pub candidate: usize,
    pub entries: Vec<Entry>,
    pub profiles: Vec<Profile>,
    pub max_agents: usize,
    pub max_parallel: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Clarifier,
    Origin,
    Recruit,
    Manual,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    pub id: String,
    pub task: String,
    pub run: usize,
    pub source: String,
    pub assignment: String,
    pub profile: Profile,
    pub created: u64,
    pub ended: Option<u64>,
    pub state: Status,
    pub output: String,
    pub archived: bool,
    pub role: Role,
    pub due: Option<u64>,
    pub replaces: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Post {
    pub id: String,
    pub task: String,
    pub author: String,
    pub text: String,
    pub at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    pub id: String,
    pub task: String,
    pub run: usize,
    pub agent: String,
    pub command: String,
    pub started: u64,
    pub ended: Option<u64>,
    pub status: Status,
    pub output: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub id: String,
    pub task: String,
    pub run: usize,
    pub candidate: usize,
    pub name: String,
    pub passed: bool,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub task: String,
    pub number: usize,
    pub origin: String,
    pub started: u64,
    pub ended: Option<u64>,
    pub state: Stage,
    pub specification: String,
    pub profiles: Vec<Profile>,
    pub max_agents: usize,
    pub max_parallel: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub id: String,
    pub task: String,
    pub run: usize,
    pub number: usize,
    pub created: u64,
    pub files: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audit {
    pub sequence: usize,
    pub at: u64,
    pub actor: String,
    pub action: String,
    pub entity: String,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub id: String,
    pub task: String,
    pub kind: String,
    pub entity: String,
    pub relative_path: String,
    pub created: u64,
    pub removed: bool,
    pub digest: String,
}

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub team: Vec<crate::AgentProfile>,
    #[serde(default)]
    pub turns_used: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub seq: i64,
    pub session_id: String,
    pub author: String,
    pub recipient: Option<String>,
    pub kind: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Ready,
    Running,
    Review,
    Accepted,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub description: String,
    pub competence: String,
    pub difficulty: String,
    pub dependencies: Vec<String>,
    pub checks: Vec<String>,
    pub state: TaskState,
    pub assignee: Option<String>,
    pub reviewer: Option<String>,
    pub attempts: usize,
    pub result: Option<String>,
    pub workspace: Option<PathBuf>,
    #[serde(default)]
    pub base_commit: Option<String>,
    #[serde(default)]
    pub interrupted: bool,
}

impl Task {
    pub fn assign(&mut self, agent: &str, accepted: &HashSet<String>) -> Result<()> {
        if self.state != TaskState::Ready {
            bail!("Task is not ready");
        }
        if !self.dependencies.iter().all(|id| accepted.contains(id)) {
            bail!("Dependencies are not accepted");
        }
        self.state = TaskState::Running;
        self.assignee = Some(agent.into());
        self.attempts += 1;
        self.interrupted = false;
        Ok(())
    }
    pub fn submit(&mut self, actor: &str, result: String) -> Result<()> {
        if self.state != TaskState::Running || self.assignee.as_deref() != Some(actor) {
            bail!("Only the assigned running agent may submit");
        }
        self.result = Some(result);
        self.state = TaskState::Review;
        Ok(())
    }
    pub fn review(&mut self, actor: &str, approved: bool, limit: usize) -> Result<()> {
        if self.state != TaskState::Review {
            bail!("Task is not awaiting review");
        }
        if self.assignee.as_deref() == Some(actor) {
            bail!("Self-acceptance is forbidden");
        }
        self.reviewer = Some(actor.into());
        self.state = if approved {
            TaskState::Accepted
        } else if self.attempts >= limit {
            TaskState::Blocked
        } else {
            TaskState::Ready
        };
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTask {
    pub title: String,
    pub description: String,
    #[serde(default = "implementation")]
    pub competence: String,
    #[serde(default = "standard")]
    pub difficulty: String,
    /// Indexes in this plan, not database ids.
    #[serde(default)]
    pub dependencies: Vec<usize>,
    #[serde(default)]
    pub checks: Vec<String>,
}
fn implementation() -> String {
    "implementation".into()
}
fn standard() -> String {
    "standard".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub summary: String,
    pub tasks: Vec<PlanTask>,
}
impl Plan {
    pub fn validate(&self) -> Result<()> {
        if self.tasks.is_empty() || self.tasks.len() > 24 {
            bail!("Plan must contain 1..24 tasks");
        }
        for (i, task) in self.tasks.iter().enumerate() {
            if task.title.trim().is_empty() || task.description.trim().is_empty() {
                bail!("Task needs a title and description");
            }
            if ![
                "analysis",
                "planning",
                "implementation",
                "verification",
                "synthesis",
            ]
            .contains(&task.competence.as_str())
            {
                bail!("Unknown competence");
            }
            if !["simple", "standard", "complex"].contains(&task.difficulty.as_str()) {
                bail!("Unknown difficulty");
            }
            if task
                .dependencies
                .iter()
                .any(|&d| d >= self.tasks.len() || d == i)
            {
                bail!("Invalid dependency");
            }
        }
        let mut done = HashSet::new();
        loop {
            let before = done.len();
            for (i, t) in self.tasks.iter().enumerate() {
                if t.dependencies.iter().all(|d| done.contains(d)) {
                    done.insert(i);
                }
            }
            if done.len() == self.tasks.len() {
                return Ok(());
            }
            if done.len() == before {
                bail!("Cyclic task dependencies");
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub approved: bool,
    pub reason: String,
    #[serde(default)]
    pub lesson: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub project_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub source_session: String,
    pub author: String,
    pub reviewer: Option<String>,
    pub status: String,
    pub created_at: String,
    #[serde(default)]
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: String,
    pub agent_version: String,
    pub agent_name: String,
    pub competence: String,
    pub difficulty: String,
    pub success: bool,
    pub evidence: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    Message(Message),
    Delta { agent: String, text: String },
    AgentStatus { agent: String, status: String },
    Task(Task),
    Status(String),
    Finished { session_id: String, status: String },
}

/// Accept a complete JSON object or one enclosing fenced block, never silently pick
/// a substring from prose (which could make an invalid plan appear valid).
pub fn parse_response<T: serde::de::DeserializeOwned>(text: &str) -> Result<T> {
    let trimmed = text.trim();
    let payload = if trimmed.starts_with("```") && trimmed.ends_with("```") {
        trimmed
            .split_once('\n')
            .map(|(_, s)| s[..s.len() - 3].trim())
            .unwrap_or(trimmed)
    } else {
        trimmed
    };
    Ok(serde_json::from_str(payload)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_cycles() {
        let mut plan = Plan {
            summary: String::new(),
            tasks: (0..2)
                .map(|i| PlanTask {
                    title: "x".into(),
                    description: "x".into(),
                    competence: implementation(),
                    difficulty: standard(),
                    dependencies: vec![1 - i],
                    checks: vec![],
                })
                .collect(),
        };
        assert!(plan.validate().is_err());
        plan.tasks[1].dependencies.clear();
        assert!(plan.validate().is_ok());
    }
    #[test]
    fn rejects_self_acceptance_and_double_assignment() {
        let mut task = Task {
            id: new_id(),
            session_id: new_id(),
            title: "x".into(),
            description: "x".into(),
            competence: implementation(),
            difficulty: standard(),
            dependencies: vec![],
            checks: vec![],
            state: TaskState::Ready,
            assignee: None,
            reviewer: None,
            attempts: 0,
            result: None,
            workspace: None,
            base_commit: None,
            interrupted: false,
        };
        task.assign("a", &HashSet::new()).unwrap();
        assert!(task.assign("b", &HashSet::new()).is_err());
        task.submit("a", "done".into()).unwrap();
        assert!(task.review("a", true, 3).is_err());
        task.review("b", true, 3).unwrap();
        assert_eq!(task.state, TaskState::Accepted);
    }
}

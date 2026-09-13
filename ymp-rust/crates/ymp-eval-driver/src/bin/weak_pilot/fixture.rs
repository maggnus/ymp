//! Synthetic protocol work only; no measured-task solutions are available here.
use anyhow::{bail, ensure, Result};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use ymp_core::*;
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};

pub const VISIBLE_PROMPT: &str = "Read input.txt and return the visible protocol marker.";
pub const VISIBLE_INPUT: &str = "protocol-marker\n";

pub fn config(members: usize, turns: usize, total: u64) -> Config {
    let ids = (1..=members)
        .map(|n| format!("weak-{n}"))
        .collect::<Vec<_>>();
    Config {
        providers: vec![ProviderConfig {
            id: "offline".into(),
            kind: ProviderKind::Mock,
            command: "never-spawned-injected-backend".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        }],
        agents: ids
            .iter()
            .map(|id| AgentProfile {
                id: id.clone(),
                name: "protocol-weak".into(),
                provider: "offline".into(),
                model: Some("protocol-weak".into()),
                instructions: String::new(),
                enabled: true,
            })
            .collect(),
        team: ids.clone(),
        team_constraints: TeamConstraints {
            fixed_size: Some(members),
            fixed_roster: Some(ids.clone()),
            eligible_agents: Some(ids.clone()),
            max_members: members,
        },
        execution: ids
            .into_iter()
            .map(|id| {
                (
                    id,
                    AgentExecutionPolicy {
                        fixed: ModelEffort {
                            model: Some("protocol-weak".into()),
                            effort: Some("low".into()),
                        },
                        ..Default::default()
                    },
                )
            })
            .collect(),
        limits: Limits {
            parallel: 2,
            turns,
            // Native cleanup has its own two-second process shutdown interval.
            // Keep early protocol errors observable after that cleanup finishes.
            turn_timeout_secs: 5,
            attempts: 1,
            resources: Some(ResourceLimits {
                unknown_usage: UnknownUsagePolicy::Stop,
                startup_invocations: 2,
                required_review_invocations: 1,
                observed_tokens: Some(total),
                invocation_tokens: Some(20),
                review_reserve_tokens: Some(1),
                native_max_turns: 2,
                max_context_chars: 32_000,
                startup_context_chars: 16_000,
                max_output_chars: 4_000,
            }),
        },
        ..Default::default()
    }
}

#[derive(Clone, Copy)]
pub enum Behavior {
    Complete,
    Partial,
    Unknown,
    Failure,
    Pending,
    Cooperation,
}

pub struct ProtocolBackend {
    pub behavior: Behavior,
    pub tokens: u64,
    pub delay_ms: u64,
    pub cooperation_tasks: usize,
    journal: Arc<Mutex<Vec<Value>>>,
}
impl ProtocolBackend {
    pub fn new(behavior: Behavior) -> Self {
        Self {
            behavior,
            tokens: 10,
            delay_ms: 0,
            cooperation_tasks: 1,
            journal: Default::default(),
        }
    }
    pub fn journal(&self) -> Vec<Value> {
        self.journal.lock().unwrap().clone()
    }
    pub fn starts(&self) -> Vec<Value> {
        self.journal()
            .into_iter()
            .filter(|row| row["type"] == "started")
            .collect()
    }
}
struct Closed {
    id: String,
    journal: Arc<Mutex<Vec<Value>>>,
}
impl Drop for Closed {
    fn drop(&mut self) {
        self.journal
            .lock()
            .unwrap()
            .push(json!({"type":"closed","context_id":self.id}));
    }
}
impl ExecutionBackend for ProtocolBackend {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.evals.weak-pilot-protocol".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        if request.read_only {
            WorkspaceAccess::ReadAll
        } else {
            WorkspaceAccess::WriteAll
        }
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            ensure!(
                request.settings.effort.as_deref() == Some("low"),
                "Protocol received changed effort"
            );
            let id = new_id();
            let _closed = Closed {
                id: id.clone(),
                journal: self.journal.clone(),
            };
            self.journal.lock().unwrap().push(json!({
                "type":"started","context_id":id,"agent_id":request.profile.id,
                "purpose":request.purpose,"cwd":request.cwd,"resume":request.resume,
                "settings":request.settings,"prompt":request.prompt,
                "mcp_present":request.mcp.is_some()
            }));
            events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
                sent: Some(request.settings.clone()),
                reported: Some(request.settings.clone()),
                native_session_id: Some(id.clone()),
                native_turn_id: Some(id.clone()),
                native_version: Some("synthetic-protocol-only".into()),
                ..Default::default()
            })))?;
            events.send(ProviderEvent::Session(id.clone()))?;
            events.send(ProviderEvent::Tool("synthetic_read".into()))?;
            if !matches!(self.behavior, Behavior::Unknown) {
                let partial = matches!(self.behavior, Behavior::Partial | Behavior::Pending);
                let snapshot = UsageSnapshot {
                    counts: TokenCounts {
                        input: Some(self.tokens - 2),
                        output: Some(2),
                        cache_read: Some(3),
                        cache_write: Some(1),
                        reasoning: Some(1),
                    },
                    finalized: !partial,
                    partial,
                    note: Some("Synthetic protocol accounting; not model usage".into()),
                    native_total: None,
                };
                // Real replacement events: the observer must retain both and count once.
                events.send(ProviderEvent::Usage(snapshot.clone()))?;
                events.send(ProviderEvent::Usage(snapshot))?;
            }
            if self.delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
            }
            if matches!(self.behavior, Behavior::Pending) {
                return std::future::pending::<Result<TurnResult>>().await;
            }
            if matches!(self.behavior, Behavior::Failure) {
                bail!("synthetic_provider_failure");
            }
            let text = if matches!(self.behavior, Behavior::Cooperation) {
                let markers = if self.cooperation_tasks == 1 {
                    vec![(
                        "Protocol marker".to_owned(),
                        "protocol-marker.txt".to_owned(),
                    )]
                } else {
                    (1..=self.cooperation_tasks)
                        .map(|n| {
                            (
                                format!("Protocol marker {n}"),
                                format!("protocol-marker-{n}.txt"),
                            )
                        })
                        .collect::<Vec<_>>()
                };
                // Use the current task text, never the actor name or peer history,
                // to determine this synthetic task's output.
                let current = request
                    .prompt
                    .rsplit_once(&format!("Your current assignment ({}):\n", request.purpose))
                    .map(|(_, current)| current)
                    .unwrap_or(&request.prompt);
                match request.purpose.as_str() {
                    "plan" => json!({"summary":"Inspect explicit synthetic protocol markers","tasks":markers.iter().map(|(title, output)| json!({"title":title,"description":format!("Copy the visible input.txt marker into {output}"),"competence":"implementation","difficulty":"simple","access":"write","dependencies":[],"checks":[]})).collect::<Vec<_>>()} ).to_string(),
                    "review_plan" => json!({"approved":true,"reason":"Explicit protocol artifacts with independent review"}).to_string(),
                    "execute" => {
                        let selected = markers.iter().filter(|(title, _)| current.contains(&format!("\n{title}\n"))).collect::<Vec<_>>();
                        ensure!(selected.len() == 1, "Synthetic producer must have one actual task assignment");
                        std::fs::copy(request.cwd.join("input.txt"), request.cwd.join(&selected[0].1))?;
                        format!("Copied the visible protocol marker into {} for independent inspection.", selected[0].1)
                    },
                    "review" | "final_review" => {
                        let selected = markers.iter().filter(|(title, _)| request.purpose == "final_review" || current.contains(&format!("Task: {title}\n"))).collect::<Vec<_>>();
                        ensure!(!selected.is_empty(), "Synthetic review must inspect its actual assigned artifacts");
                        let original = std::fs::read(request.cwd.join("input.txt"))?;
                        let mut approved = true;
                        for (_, output) in selected { approved &= std::fs::read(request.cwd.join(output))? == original; }
                        json!({"approved":approved,"reason":"Compared each assigned actual protocol marker with visible input.txt"}).to_string()
                    },
                    "synthesis" => "The protocol marker was independently inspected; no external confirmation was supplied.".into(),
                    other => bail!("Unexpected Engine purpose {other}; no hidden fixture fallback"),
                }
            } else {
                ensure!(
                    request.resume.is_none() && request.mcp.is_none(),
                    "Independent request received shared history or team access"
                );
                ensure!(
                    request.prompt == VISIBLE_PROMPT,
                    "Independent request differs from visible requirements"
                );
                std::fs::read_to_string(request.cwd.join("input.txt"))?
            };
            events.send(ProviderEvent::Delta(text.clone()))?;
            Ok(TurnResult {
                text,
                session_id: id,
                usage: None,
            })
        })
    }
}

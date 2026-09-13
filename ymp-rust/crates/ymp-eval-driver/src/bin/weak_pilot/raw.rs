//! Narrow raw-turn observer. These records never assert production acceptance.
use super::{fixture, write_json};
use anyhow::{bail, ensure, Result};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};
use tokio::{sync::mpsc, time::Instant};
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{
    run_turn_with_backend, ExecutionBackend, ProviderEvent, TurnRequest, TurnResult,
};
use ymp_storage::{Store, StoreLock};

pub struct Group {
    pub store: Store,
    pub session: Session,
    pub config: Config,
    directory: PathBuf,
    work: PathBuf,
    max_calls: u64,
    deadline: Instant,
    admission: Mutex<()>,
    _lock: StoreLock,
}

struct OpenInvocation {
    store: Store,
    session: String,
    id: String,
    closed: bool,
}
impl Drop for OpenInvocation {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.store.finish_invocation(
                &self.session,
                &self.id,
                InvocationState::Interrupted,
                Some("Pilot observer ended before terminal accounting"),
            );
        }
    }
}

impl Group {
    pub fn new(
        directory: &Path,
        members: usize,
        max_calls: u64,
        total: u64,
        deadline_ms: u64,
    ) -> Result<Self> {
        ensure!(
            max_calls > 0 && deadline_ms > 0,
            "Pilot controls must be positive"
        );
        std::fs::create_dir(directory)?;
        let work = directory.join("work");
        std::fs::create_dir(&work)?;
        let store = Store::open(&directory.join("metadata"))?;
        // Keep the production review reserve. The exact raw-call cap is checked below.
        let config = fixture::config(members, usize::try_from(max_calls)? + 1, total);
        config.validate()?;
        let project = store.project(&work)?;
        let lock = store.lock_project(&project.id)?;
        let session = Session {
            id: new_id(),
            project_id: project.id,
            title: fixture::VISIBLE_PROMPT.into(),
            status: "running".into(),
            created_at: now(),
            team: config.agents.clone(),
            turns_used: 0,
        };
        store.create_session(
            &session,
            &SessionPolicy {
                session_id: session.id.clone(),
                goal: session.title.clone(),
                constraints: None,
                cwd: work.clone(),
                limits: config.limits.clone(),
                eligible_pool: config.agents.clone(),
                captured_team: config.agents.clone(),
                team_constraints: Some(config.team_constraints.clone()),
                execution: config.execution.clone(),
                assignment_settings: vec![],
                parent_session_id: None,
                evaluation: Some(EvaluationReference {
                    run_id: session.id.clone(),
                    scenario_id: "ymp-201-offline-protocol".into(),
                    revision: None,
                }),
                captured_at: now(),
            },
        )?;
        Ok(Self {
            store,
            session,
            config,
            directory: directory.into(),
            work,
            max_calls,
            deadline: Instant::now() + Duration::from_millis(deadline_ms),
            admission: Mutex::new(()),
            _lock: lock,
        })
    }

    pub async fn candidate(
        &self,
        backend: &dyn ExecutionBackend,
        agent: usize,
    ) -> Result<TurnResult> {
        let (request, invocation) = {
            let _admission = self
                .admission
                .lock()
                .map_err(|_| anyhow::anyhow!("Pilot admission lock poisoned"))?;
            ensure!(
                Instant::now() < self.deadline,
                "pilot_group_deadline: no time remains"
            );
            let budget = self.store.session_budget(&self.session.id)?.unwrap();
            ensure!(
                budget.admitted_invocations < self.max_calls,
                "pilot_call_limit: whole-group call cap reached"
            );
            let profile = self
                .config
                .agents
                .get(agent)
                .ok_or_else(|| anyhow::anyhow!("Unknown pilot actor"))?
                .clone();
            let provider = self.config.provider(&profile.provider)?.clone();
            let settings = self
                .config
                .execution_settings(&profile, &ModelEffort::default())?;
            let settings = ExecutionSettings {
                permission_mode: Some("read_only".into()),
                ..settings
            };
            let cwd = self
                .work
                .join(format!("candidate-{}", budget.admitted_invocations + 1));
            // A denied admission may leave its fresh, input-only directory behind.
            if !cwd.exists() {
                std::fs::create_dir(&cwd)?;
                std::fs::write(cwd.join("requirements.txt"), fixture::VISIBLE_PROMPT)?;
                std::fs::write(cwd.join("input.txt"), fixture::VISIBLE_INPUT)?;
            }
            let request = TurnRequest {
                profile: profile.clone(),
                settings: settings.clone(),
                provider: provider.clone(),
                cwd: cwd.clone(),
                prompt: fixture::VISIBLE_PROMPT.into(),
                purpose: "pilot_candidate".into(),
                read_only: true,
                resume: None,
                usage_baseline: None,
                mcp: None,
                resource_controls: NativeResourceControls {
                    max_turns: Some(2),
                    max_output_chars: Some(4_000),
                },
                timeout_secs: self.config.limits.turn_timeout_secs,
                bridge: PathBuf::new(),
            };
            let started_at = now();
            let assignment = AssignmentRecord {
                id: new_id(),
                session_id: self.session.id.clone(),
                task: None,
                agent_id: profile.id.clone(),
                agent_config_version: profile.version(&provider),
                provider_id: provider.id,
                purpose: request.purpose.clone(),
                reason: "Raw offline candidate; no production grant, review, or acceptance".into(),
                cwd,
                requested: settings.clone(),
                timeout_secs: request.timeout_secs,
                grant_ids: vec![],
                context: [
                    (ContextKind::Prompt, request.prompt.as_str()),
                    (
                        ContextKind::ProfileInstructions,
                        profile.instructions.as_str(),
                    ),
                ]
                .into_iter()
                .map(|(kind, text)| ContextReference {
                    kind,
                    id: content_digest(text),
                    session_id: Some(self.session.id.clone()),
                    digest: Some(content_digest(text)),
                    included_chars: Some(text.chars().count()),
                })
                .collect(),
                state: InvocationState::Running,
                started_at: started_at.clone(),
                ended_at: None,
                token_reservation: Some(20),
                agent_identity: None,
            };
            let invocation = self.store.admit_invocation(
                &assignment,
                InvocationRecord {
                    id: new_id(),
                    session_id: self.session.id.clone(),
                    assignment_id: assignment.id.clone(),
                    execution_backend: Some(backend.identity()),
                    turn: 1,
                    requested: settings,
                    sent: Default::default(),
                    reported: Default::default(),
                    resumed_from: None,
                    native_session_id: None,
                    native_turn_id: None,
                    native_version: None,
                    state: InvocationState::Running,
                    started_at,
                    ended_at: None,
                    usage: None,
                    terminal_reason: None,
                },
            )?;
            (request, invocation)
        };
        let mut open = OpenInvocation {
            store: self.store.clone(),
            session: self.session.id.clone(),
            id: invocation.id.clone(),
            closed: false,
        };
        let cancel = CancellationToken::new();
        let (events, mut received) = mpsc::unbounded_channel();
        let mut future = Box::pin(run_turn_with_backend(
            backend,
            request,
            cancel.clone(),
            events,
        ));
        let mut stop_reason = None;
        let mut result = loop {
            tokio::select! {
                outcome = &mut future => break outcome,
                _ = tokio::time::sleep_until(self.deadline) => {
                    stop_reason = Some("pilot_group_deadline");
                    cancel.cancel();
                    break future.as_mut().await;
                },
                Some(event) = received.recv() => {
                    if let Err(error) = self.observe(&invocation.id, event) { cancel.cancel(); break Err(error); }
                    let budget = self.store.session_budget(&self.session.id)?.unwrap();
                    if budget.observed_usage.known_total().is_some_and(|n| n >= budget.limits.resources.as_ref().unwrap().observed_tokens.unwrap()) {
                        stop_reason = Some("pilot_observed_token_threshold");
                        cancel.cancel();
                    }
                },
            }
        };
        drop(future);
        // Preserve already-received usage on success, failure, deadline and cancellation.
        while let Ok(event) = received.try_recv() {
            if let Err(error) = self.observe(&invocation.id, event) {
                // Preserve a primary provider failure and retain any secondary
                // accounting error independently in the event journal.
                self.store.event(
                    &self.session.id,
                    "pilot_observer_error",
                    &json!({
                        "invocation_id":invocation.id,"reason":format!("{error:#}")
                    }),
                )?;
                if result.is_ok() {
                    result = Err(error);
                }
            }
        }
        if let Some(reason) = stop_reason {
            result = Err(anyhow::anyhow!(reason));
        }
        let state = if result.is_ok() {
            InvocationState::Completed
        } else if cancel.is_cancelled() {
            InvocationState::Cancelled
        } else {
            InvocationState::Failed
        };
        let terminal_reason = result
            .as_ref()
            .err()
            .map(|_| stop_reason.unwrap_or("pilot_provider_error"));
        self.store
            .finish_invocation(&self.session.id, &invocation.id, state, terminal_reason)?;
        open.closed = true;
        // Results stay outside every candidate's solving directory.
        if let Ok(answer) = &result {
            write_json(
                &self
                    .directory
                    .join(format!("answer-{}.json", invocation.turn)),
                answer,
            )?;
        }
        result
    }

    fn observe(&self, id: &str, event: ProviderEvent) -> Result<()> {
        let (raw, observation) = match event {
            ProviderEvent::Execution(value) => {
                (json!({"kind":"execution","value":value}), Some(*value))
            }
            ProviderEvent::Usage(value) => (
                json!({"kind":"usage","value":value}),
                Some(InvocationObservation {
                    usage: Some(value),
                    ..Default::default()
                }),
            ),
            ProviderEvent::Session(value) => (
                json!({"kind":"session","value":value}),
                Some(InvocationObservation {
                    native_session_id: Some(value),
                    ..Default::default()
                }),
            ),
            ProviderEvent::Capabilities(value) => {
                (json!({"kind":"capabilities","value":value}), None)
            }
            ProviderEvent::Delta(value) => (json!({"kind":"delta","value":value}), None),
            ProviderEvent::Tool(value) => (json!({"kind":"tool","value":value}), None),
            ProviderEvent::Retry {
                session_id,
                turn_id,
                error_code,
            } => (
                json!({"kind":"retry","session_id":session_id,"turn_id":turn_id,"error_code":error_code}),
                None,
            ),
        };
        self.store.event(
            &self.session.id,
            "pilot_provider_event",
            &json!({"invocation_id":id,"event":raw}),
        )?;
        if let Some(observation) = observation {
            self.store
                .observe_invocation(&self.session.id, id, &observation)?;
        }
        Ok(())
    }

    pub fn save(&self, backend: &super::fixture::ProtocolBackend) -> Result<Value> {
        let trace = self.store.trace(&self.session.id)?;
        write_json(&self.directory.join("trace.json"), &trace)?;
        write_json(&self.directory.join("protocol.json"), &backend.journal())?;
        ensure!(
            trace.usage.total.open_calls == 0,
            "Invocation leaked past its boundary"
        );
        if trace.invocations.iter().any(|i| i.ended_at.is_none()) {
            bail!("Missing terminal accounting");
        }
        Ok(
            json!({"session_id":self.session.id,"usage":trace.usage,"budget":trace.budget,
            "external_call_cap":self.max_calls,"actual_native_model":null,"native_inference":false}),
        )
    }
}

//! Narrow raw-turn observer. These records never assert production acceptance.
use super::{fixture, write_json};
use anyhow::{ensure, Context, Result};
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
        let config = fixture::config(
            members,
            usize::try_from(max_calls)?
                .checked_add(1)
                .context("Pilot call limit overflow")?,
            total,
        );
        let duration = Duration::from_millis(deadline_ms);
        let mut group = Self::configured(
            directory,
            config,
            max_calls,
            Instant::now() + duration,
            fixture::VISIBLE_PROMPT,
            "ymp-201-offline-protocol",
        )?;
        // Preserve the original fixture deadline: metadata preparation is not
        // part of this synthetic timing control. Configured callers supply the
        // actual whole-condition deadline themselves.
        group.deadline = Instant::now() + duration;
        Ok(group)
    }

    pub fn configured(
        directory: &Path,
        config: Config,
        max_calls: u64,
        deadline: Instant,
        title: &str,
        evaluation_kind: &str,
    ) -> Result<Self> {
        // Retained for the original offline proofs, which deliberately test
        // data separation without claiming an OS read-isolation boundary.
        Self::configured_internal(
            directory,
            None,
            config,
            max_calls,
            deadline,
            title,
            evaluation_kind,
        )
    }

    pub fn configured_with_work(
        directory: &Path,
        work: &Path,
        config: Config,
        max_calls: u64,
        deadline: Instant,
        title: &str,
        evaluation_kind: &str,
    ) -> Result<Self> {
        Self::configured_internal(
            directory,
            Some(work),
            config,
            max_calls,
            deadline,
            title,
            evaluation_kind,
        )
    }

    fn configured_internal(
        directory: &Path,
        external_work: Option<&Path>,
        config: Config,
        max_calls: u64,
        deadline: Instant,
        title: &str,
        evaluation_kind: &str,
    ) -> Result<Self> {
        ensure!(
            max_calls > 0 && deadline > Instant::now(),
            "Pilot controls must be positive and leave time"
        );
        ensure!(
            !title.trim().is_empty() && !evaluation_kind.trim().is_empty(),
            "Pilot session and evaluation identity are required"
        );
        config.validate()?;
        let resources = config
            .limits
            .resources
            .as_ref()
            .context("Pilot requires captured resource limits")?;
        ensure!(
            resources.unknown_usage == UnknownUsagePolicy::Stop
                && resources.observed_tokens.is_some()
                && resources.invocation_tokens.is_some(),
            "Pilot requires observed token admission with unknown_usage=stop"
        );
        ensure!((config.limits.turns as u64) >= max_calls.checked_add(resources.required_review_invocations).context("Pilot reserve overflow")?,
            "Raw storage allowance must retain the production review reserve beyond the outer call cap");
        let team = config.members();
        ensure!(
            !team.is_empty(),
            "Pilot requires enabled captured participants"
        );
        std::fs::create_dir(directory)?;
        if external_work.is_some() {
            unlinked_directory(directory)?;
        }
        let directory = directory.canonicalize()?;
        let work = if let Some(work) = external_work {
            ensure!(
                work.is_absolute(),
                "External solving work must be an absolute path"
            );
            let parent =
                unlinked_directory(work.parent().context("External work needs a parent")?)?;
            let selected = parent.join(
                work.file_name()
                    .context("External work needs a directory name")?,
            );
            ensure!(
                work == selected.as_path(),
                "External work path must not contain aliases or parent traversal"
            );
            ensure!(
                !selected.starts_with(&directory) && !directory.starts_with(&selected),
                "Controller and solving directories must be disjoint"
            );
            std::fs::create_dir(&selected).context("External solving work must be fresh")?;
            disjoint_work_roots(&directory, &selected)?.1
        } else {
            let work = directory.join("work");
            std::fs::create_dir(&work)?;
            work
        };
        let store = Store::open(&directory.join("metadata"))?;
        let project = store.project(&work)?;
        let lock = store.lock_project(&project.id)?;
        let session = Session {
            id: new_id(),
            project_id: project.id,
            title: title.into(),
            status: "running".into(),
            created_at: now(),
            team: team.clone(),
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
                captured_team: team,
                team_constraints: Some(config.team_constraints.clone()),
                execution: config.execution.clone(),
                assignment_settings: vec![],
                parent_session_id: None,
                evaluation: Some(EvaluationReference {
                    run_id: session.id.clone(),
                    scenario_id: evaluation_kind.into(),
                    revision: None,
                }),
                captured_at: now(),
            },
        )?;
        Ok(Self {
            store,
            session,
            config,
            directory,
            work,
            max_calls,
            deadline,
            admission: Mutex::new(()),
            _lock: lock,
        })
    }

    pub fn workdir(&self) -> &Path {
        &self.work
    }

    pub async fn candidate(
        &self,
        backend: &dyn ExecutionBackend,
        agent: usize,
    ) -> Result<TurnResult> {
        let request = {
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
            TurnRequest {
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
            }
        };
        self.invoke(backend, request).await
    }

    pub async fn invoke(
        &self,
        backend: &dyn ExecutionBackend,
        request: TurnRequest,
    ) -> Result<TurnResult> {
        let invocation = {
            let _admission = self
                .admission
                .lock()
                .map_err(|_| anyhow::anyhow!("Pilot admission lock poisoned"))?;
            ensure!(
                Instant::now() < self.deadline,
                "pilot_group_deadline: no time remains"
            );
            let budget = self
                .store
                .session_budget(&self.session.id)?
                .context("Missing captured pilot budget")?;
            ensure!(
                budget.admitted_invocations < self.max_calls,
                "pilot_call_limit: whole-group call cap reached"
            );
            let policy = self
                .store
                .session_policy(&self.session.id)?
                .context("Missing captured pilot policy")?;
            let profile = policy
                .captured_team
                .iter()
                .find(|profile| profile.id == request.profile.id)
                .context("Requested actor is not in the captured pilot team")?;
            ensure!(
                &request.profile == profile,
                "Pilot request profile differs from the captured actor"
            );
            let provider = self.config.provider(&profile.provider)?;
            ensure!(
                serde_json::to_value(&request.provider)? == serde_json::to_value(provider)?,
                "Pilot request provider differs from captured configuration"
            );
            let settings = policy
                .execution
                .get(&profile.id)
                .cloned()
                .unwrap_or_default()
                .resolve(profile, &ModelEffort::default())?;
            ensure!(
                request.settings.model == settings.model
                    && request.settings.effort == settings.effort,
                "Pilot request model or effort differs from captured configuration"
            );
            ensure!(
                request.resume.is_none()
                    && request.usage_baseline.is_none()
                    && request.mcp.is_none(),
                "Independent pilot requests require fresh contexts without team capabilities"
            );
            ensure!(
                !request.purpose.trim().is_empty(),
                "Pilot request purpose is required"
            );
            ensure!(
                request.settings.permission_mode.as_deref()
                    == Some(if request.read_only {
                        "read_only"
                    } else {
                        "write"
                    }),
                "Pilot request permission mode differs from requested access"
            );
            let cwd = request.cwd.canonicalize()?;
            ensure!(
                cwd.is_dir() && cwd.starts_with(self.work.canonicalize()?),
                "Pilot request directory escapes the allowed solving root"
            );
            if !self.work.starts_with(&self.directory) {
                disjoint_work_roots(&self.directory, &self.work)?;
                unlinked_directory(&request.cwd)?;
            }
            let resources = policy
                .limits
                .resources
                .as_ref()
                .context("Missing captured pilot resources")?;
            ensure!(
                request
                    .resource_controls
                    .max_turns
                    .is_some_and(|n| n > 0 && n <= resources.native_max_turns)
                    && request
                        .resource_controls
                        .max_output_chars
                        .is_some_and(|n| n > 0 && n <= resources.max_output_chars),
                "Pilot request native controls exceed captured limits"
            );
            let started_at = now();
            let assignment = AssignmentRecord {
                id: new_id(),
                session_id: self.session.id.clone(),
                task: None,
                agent_id: profile.id.clone(),
                agent_config_version: profile.version(provider),
                provider_id: provider.id.clone(),
                purpose: request.purpose.clone(),
                reason: "Raw pilot invocation; no production grant, review, or acceptance".into(),
                cwd: request.cwd.clone(),
                requested: request.settings.clone(),
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
                token_reservation: resources.invocation_tokens,
                agent_identity: None,
            };
            self.store.admit_invocation(
                &assignment,
                InvocationRecord {
                    id: new_id(),
                    session_id: self.session.id.clone(),
                    assignment_id: assignment.id.clone(),
                    execution_backend: Some(backend.identity()),
                    turn: 1,
                    requested: request.settings.clone(),
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
            )?
        };
        let mut open = OpenInvocation {
            store: self.store.clone(),
            session: self.session.id.clone(),
            id: invocation.id.clone(),
            closed: false,
        };
        let request_scope = json!({
            "agent_id":request.profile.id,"purpose":request.purpose,"cwd":request.cwd,
            "requested":request.settings,"read_only":request.read_only,
            "prompt_sha256":content_digest(&request.prompt),
            "profile_instructions_sha256":content_digest(&request.profile.instructions),
            "resource_controls":request.resource_controls,"timeout_secs":request.timeout_secs
        });
        self.store.event(&self.session.id, "pilot_request_bound", &json!({
            "invocation_id":invocation.id,"assignment_id":invocation.assignment_id,"request":request_scope
        }))?;
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
        let observed = self.store.invocation(&self.session.id, &invocation.id)?;
        let receipt = json!({
            "invocation_id":invocation.id,"assignment_id":invocation.assignment_id,
            "turn":invocation.turn,"request":request_scope,"state":state,
            "result":result.as_ref().ok(),"error":result.as_ref().err().map(|error| format!("{error:#}")),
            "observation":observed
        });
        self.store
            .event(&self.session.id, "pilot_invocation_result", &receipt)?;
        write_json(
            &self
                .directory
                .join(format!("result-{}.json", invocation.turn)),
            &receipt,
        )?;
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
        let mut summary = self.save_trace()?;
        write_json(&self.directory.join("protocol.json"), &backend.journal())?;
        ensure!(
            summary["accounting_closed"] == true,
            "Invocation leaked past its boundary or lacks terminal accounting"
        );
        summary["actual_native_model"] = Value::Null;
        summary["native_inference"] = json!(false);
        Ok(summary)
    }

    pub fn save_trace(&self) -> Result<Value> {
        let trace = self.store.trace(&self.session.id)?;
        write_json(&self.directory.join("trace.json"), &trace)?;
        let summary = json!({
            "session_id":self.session.id,"usage":trace.usage,"budget":trace.budget,
            "external_call_cap":self.max_calls,
            "evaluation":trace.policy.as_ref().and_then(|policy| policy.evaluation.as_ref()),
            "accounting_closed":trace.usage.total.open_calls == 0 && trace.invocations.iter().all(|i| i.ended_at.is_some())
        });
        write_json(&self.directory.join("summary.json"), &summary)?;
        Ok(summary)
    }
}

/// Path checks support the independently verified native read policy. They do
/// not themselves contain a provider or replace its filesystem restrictions.
pub(super) fn disjoint_work_roots(directory: &Path, work: &Path) -> Result<(PathBuf, PathBuf)> {
    let directory = unlinked_directory(directory)?;
    let work = unlinked_directory(work)?;
    ensure!(
        !work.starts_with(&directory) && !directory.starts_with(&work),
        "Controller and solving directories must be disjoint"
    );
    for entry in walkdir::WalkDir::new(&work).follow_links(false) {
        ensure!(
            !entry?.file_type().is_symlink(),
            "Solving work must not contain symbolic links"
        );
    }
    Ok((directory, work))
}

fn unlinked_directory(path: &Path) -> Result<PathBuf> {
    ensure!(
        path.is_absolute(),
        "Isolated directories require absolute physical paths"
    );
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component.as_os_str());
        ensure!(
            !std::fs::symlink_metadata(&prefix)?.file_type().is_symlink(),
            "Isolated directory paths must not traverse symbolic links"
        );
    }
    let resolved = path.canonicalize()?;
    ensure!(
        resolved.is_dir() && resolved.as_path() == path,
        "Isolated directories require canonical paths without parent traversal"
    );
    Ok(resolved)
}

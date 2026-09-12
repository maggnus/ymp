//! Reviewed adverse setup helpers. Every producer/reviewer response comes from
//! an admitted scripted native turn; candidate, check and acceptance writes use
//! the same public storage validators as the Engine.
use crate::{script::NativeJournal, workflows};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};
use ymp_runtime::{
    mcp::TeamServer, BuiltinConfirmationChecker, ConfirmationChecker, Engine, WorkspaceAdmission,
};
use ymp_storage::Store;
#[derive(Clone, Serialize, Deserialize)]
pub struct NativeAction {
    pub writes: Vec<(PathBuf, Vec<u8>)>,
    pub reads: Vec<PathBuf>,
    pub response: String,
}
struct Backend {
    journal: Arc<NativeJournal>,
    actions: std::sync::Mutex<std::collections::BTreeMap<String, NativeAction>>,
}
impl ExecutionBackend for Backend {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.evals.result-boundaries".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        match serde_json::from_str::<Value>(&request.prompt)
            .ok()
            .and_then(|value| {
                value["command_id"]
                    .as_str()
                    .and_then(|id| self.actions.lock().unwrap().get(id).cloned())
            }) {
            Some(action) => WorkspaceAccess::Scoped {
                reads: action.reads,
                writes: action.writes.into_iter().map(|(path, _)| path).collect(),
            },
            None => WorkspaceAccess::WriteAll,
        }
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            let command: Value = serde_json::from_str(&request.prompt)?;
            let action = self
                .actions
                .lock()
                .unwrap()
                .get(
                    command["command_id"]
                        .as_str()
                        .context("Missing scripted command identity")?,
                )
                .cloned()
                .context("Unknown scripted command")?;
            let id = new_id();
            self.journal.push(json!({"type":"native_started","native_id":id,"agent_id":request.profile.id,"purpose":request.purpose,"action":action,"settings":request.settings}));
            events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
                sent: Some(request.settings.clone()),
                reported: Some(request.settings.clone()),
                native_session_id: Some(id.clone()),
                native_turn_id: Some(id.clone()),
                native_version: Some("scripted-result-v1".into()),
                ..Default::default()
            })))?;
            for path in &action.reads {
                let bytes = std::fs::read(request.cwd.join(path))?;
                self.journal.push(json!({"type":"native_read","native_id":id,"path":path,"sha256":bytes_digest(&bytes)}));
            }
            for (path, bytes) in &action.writes {
                let path = request.cwd.join(path);
                std::fs::create_dir_all(path.parent().unwrap())?;
                std::fs::write(&path, bytes)?;
                self.journal.push(json!({"type":"native_write","native_id":id,"path":path,"sha256":bytes_digest(bytes)}));
            }
            let usage = UsageSnapshot {
                counts: TokenCounts {
                    input: Some(1),
                    output: Some(1),
                    ..TokenCounts::zero()
                },
                finalized: true,
                partial: false,
                note: Some("Controlled scripted result/review request".into()),
                native_total: None,
            };
            events.send(ProviderEvent::Usage(usage.clone()))?;
            self.journal.push(json!({"type":"native_closed","native_id":id,"response":action.response,"usage":usage}));
            Ok(TurnResult {
                text: action.response,
                session_id: id,
                usage: None,
            })
        })
    }
}
pub struct TaskHarness {
    pub store: Store,
    pub engine: Engine,
    pub session: Session,
    pub directory: PathBuf,
    pub journal: Arc<NativeJournal>,
    server: Arc<TeamServer>,
    backend: Arc<Backend>,
    owner: Arc<ymp_runtime::WorkspaceOwner>,
}
pub struct NativeResult {
    pub assignment: AssignmentRecord,
    pub invocation: InvocationRecord,
    pub text: String,
}
impl TaskHarness {
    pub async fn create(
        directory: &Path,
        title: &str,
        contracts: Vec<AcceptanceContract>,
    ) -> Result<Self> {
        Self::create_with_config(directory, title, contracts, workflows::config()).await
    }
    pub async fn create_with_config(
        directory: &Path,
        title: &str,
        contracts: Vec<AcceptanceContract>,
        config: Config,
    ) -> Result<Self> {
        let work = directory.join("work");
        std::fs::create_dir_all(&work)?;
        let store = Store::open(&directory.join("metadata"))?;
        let session = Session {
            id: new_id(),
            project_id: store.project(&work)?.id,
            title: title.into(),
            status: "running".into(),
            created_at: now(),
            team: config
                .team
                .iter()
                .map(|id| config.agent(id).cloned())
                .collect::<Result<Vec<_>>>()?,
            turns_used: 0,
        };
        let captured = contracts
            .into_iter()
            .map(|contract| {
                CapturedAcceptanceContract::capture(
                    contract,
                    &work,
                    BuiltinConfirmationChecker.identity(),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        store.create_session_with_contracts(
            &session,
            &SessionPolicy {
                session_id: session.id.clone(),
                goal: session.title.clone(),
                constraints: None,
                cwd: work.canonicalize()?,
                limits: config.limits.clone(),
                eligible_pool: config.agents.clone(),
                captured_team: session.team.clone(),
                team_constraints: Some(config.team_constraints.clone()),
                execution: config.execution.clone(),
                assignment_settings: vec![],
                parent_session_id: None,
                evaluation: None,
                captured_at: now(),
            },
            &captured,
        )?;
        let (events, _) = mpsc::unbounded_channel();
        let journal = Arc::new(NativeJournal::default());
        let backend = Arc::new(Backend {
            journal: journal.clone(),
            actions: Default::default(),
        });
        let engine = Engine::new(
            store.clone(),
            config,
            events.clone(),
            CancellationToken::new(),
        )?
        .with_execution_backend(backend.clone())?;
        let owner = engine.acquire_workspace_owner(&session.id)?;
        let server = Arc::new(TeamServer::start(store.clone(), &session, events).await?);
        Ok(Self {
            store,
            engine,
            session,
            directory: work,
            journal,
            server,
            backend,
            owner,
        })
    }
    pub async fn invoke(
        &self,
        agent: &str,
        purpose: &str,
        task: Option<TaskAttemptRef>,
        action: NativeAction,
        result: Option<&DecisionRecord>,
    ) -> Result<NativeResult> {
        let command_id = new_id();
        self.backend
            .actions
            .lock()
            .unwrap()
            .insert(command_id.clone(), action.clone());
        let request = TurnRequest {
            profile: self.engine.config.agent(agent)?.clone(),
            provider: self.engine.config.providers[0].clone(),
            settings: ExecutionSettings {
                model: Some("scripted-small".into()),
                effort: Some("low".into()),
                permission_mode: Some(
                    if action.writes.is_empty() {
                        "read_only"
                    } else {
                        "write"
                    }
                    .into(),
                ),
            },
            cwd: self.directory.clone(),
            prompt: json!({"command_id":command_id,"purpose":purpose,"task":task,"candidate":result.map(|r| &r.links.result)}).to_string(),
            purpose: purpose.into(),
            read_only: action.writes.is_empty(),
            resume: None,
            usage_baseline: None,
            mcp: None,
            resource_controls: NativeResourceControls {
                max_turns: Some(2),
                max_output_chars: Some(4000),
            },
            timeout_secs: 10,
            bridge: PathBuf::new(),
        };
        let WorkspaceAdmission::Acquired(mut lease) = self.engine.try_reserve_workspace(
            &self.owner,
            &self.session.id,
            &request,
            task.clone(),
        )?
        else {
            anyhow::bail!("Unexpected task setup resource deferral")
        };
        let mut context = vec![ContextReference {
            kind: ContextKind::Prompt,
            id: content_digest(&request.prompt),
            session_id: Some(self.session.id.clone()),
            digest: Some(content_digest(&request.prompt)),
            included_chars: Some(request.prompt.chars().count()),
        }];
        context.push(ContextReference {
            kind: ContextKind::ProfileInstructions,
            id: request.profile.version(&request.provider),
            session_id: None,
            digest: Some(content_digest(&request.profile.instructions)),
            included_chars: Some(request.profile.instructions.chars().count()),
        });
        if let Some(result) = result {
            context.push(ContextReference {
                kind: ContextKind::Result,
                id: result.id.clone(),
                session_id: Some(self.session.id.clone()),
                digest: Some(content_digest(&serde_json::to_string(
                    &result.links.result,
                )?)),
                included_chars: None,
            });
        }
        let mut assignment = AssignmentRecord {
            token_reservation: None,
            agent_identity: None,
            id: lease.assignment_id().into(),
            session_id: self.session.id.clone(),
            task,
            agent_id: agent.into(),
            agent_config_version: content_digest(&serde_json::to_string(&request.settings)?),
            provider_id: request.provider.id.clone(),
            purpose: purpose.into(),
            reason: "Scripted adverse boundary assignment".into(),
            cwd: self.directory.clone(),
            requested: request.settings.clone(),
            timeout_secs: request.timeout_secs,
            grant_ids: vec![],
            context,
            state: InvocationState::Running,
            started_at: now(),
            ended_at: None,
        };
        let mut invocation = InvocationRecord {
            id: new_id(),
            session_id: self.session.id.clone(),
            assignment_id: assignment.id.clone(),
            execution_backend: Some(self.backend.identity()),
            turn: 1,
            requested: request.settings.clone(),
            sent: Default::default(),
            reported: Default::default(),
            resumed_from: None,
            native_session_id: None,
            native_turn_id: None,
            native_version: None,
            state: InvocationState::Running,
            started_at: now(),
            ended_at: None,
            usage: None,
            terminal_reason: None,
        };
        let _token = lease.admit_reserved(
            self.server.clone(),
            &mut assignment,
            &mut invocation,
            TeamOperation::coordination(),
        )?;
        let (events, mut received) = mpsc::unbounded_channel();
        let result = lease
            .run_turn(
                self.backend.as_ref(),
                request,
                CancellationToken::new(),
                events,
            )
            .await;
        while let Some(event) = received.recv().await {
            let observation = match event {
                ProviderEvent::Execution(observation) => Some(*observation),
                ProviderEvent::Usage(usage) => Some(InvocationObservation {
                    usage: Some(usage),
                    ..Default::default()
                }),
                _ => None,
            };
            if let Some(observation) = observation {
                self.store
                    .observe_invocation(&self.session.id, &invocation.id, &observation)?;
            }
        }
        self.server.finish(
            &invocation.id,
            if result.is_ok() {
                InvocationState::Completed
            } else {
                InvocationState::Failed
            },
            Some("Observed controlled native result"),
        )?;
        let output = result?;
        let invocation = self.store.invocation(&self.session.id, &invocation.id)?;
        drop(lease);
        Ok(NativeResult {
            assignment,
            invocation,
            text: output.text,
        })
    }
    pub fn new_task(&self, title: &str) -> Task {
        Task {
            id: new_id(),
            session_id: self.session.id.clone(),
            title: title.into(),
            description: format!("Controlled independently checkable {title}"),
            competence: "implementation".into(),
            difficulty: "simple".into(),
            access: TaskAccess::Write,
            dependencies: vec![],
            checks: vec![],
            state: TaskState::Ready,
            assignee: None,
            reviewer: None,
            attempts: 0,
            result: None,
            workspace: Some(self.directory.clone()),
            base_commit: None,
            interrupted: false,
        }
    }
    pub async fn produce(
        &self,
        task: &mut Task,
        agent: &str,
        action: NativeAction,
    ) -> Result<DecisionRecord> {
        task.assign(agent, &Default::default())?;
        self.store.save_task(task)?;
        let native = self
            .invoke(
                agent,
                "execute",
                Some(TaskAttemptRef::from(&*task)),
                action,
                None,
            )
            .await?;
        task.submit(agent, native.text)?;
        let contracts = self.store.decisions(&self.session.id)?;
        let contract = contracts.iter().find(|d| {
            d.links
                .acceptance_contract
                .as_ref()
                .is_some_and(|c| c.contract.task_title == task.title)
        });
        let criteria = contract
            .and_then(|d| d.links.acceptance_contract.as_ref())
            .map(|c| c.contract.criteria.clone())
            .unwrap_or_else(|| {
                vec![AcceptanceCriterion {
                    id: "controlled-quality".into(),
                    description: task.description.clone(),
                }]
            });
        let artifacts = contract
            .and_then(|d| d.links.acceptance_contract.as_ref())
            .map(|c| {
                c.contract
                    .artifacts
                    .iter()
                    .map(|p| FileSnapshot::capture(&self.directory, p))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        let result = ResultVersion {
            id: task.id.clone(),
            version: task.attempts,
            task: Some(TaskAttemptRef::from(&*task)),
            summary: task.result.clone().unwrap(),
            task_definition: Some(TaskDefinition::from(&*task)),
            criteria_version: content_digest(&serde_json::to_string(&criteria)?),
            criteria,
            contract_id: contract.map(|d| d.id.clone()),
            producer_assignment_ids: vec![native.assignment.id.clone()],
            artifacts,
            component_ids: vec![],
        };
        let decision = DecisionRecord {
            id: new_id(),
            session_id: self.session.id.clone(),
            kind: "result_submitted".into(),
            actor: Some(native.assignment.agent_id),
            reason: "Observed producer response submitted for independent review".into(),
            outcome: None,
            links: RecordLinks {
                task: result.task.clone(),
                result: Some(result),
                assignment_id: Some(native.assignment.id),
                invocation_id: Some(native.invocation.id),
                ..Default::default()
            },
            created_at: now(),
        };
        self.store.save_task_with_decision(task, &decision)?;
        Ok(decision)
    }
    pub async fn review(
        &self,
        task: &Task,
        submission: &DecisionRecord,
        agent: &str,
        approved: bool,
    ) -> Result<DecisionRecord> {
        let result = submission.links.result.as_ref().unwrap();
        let native=self.invoke(agent,"review",Some(TaskAttemptRef::from(task)),NativeAction{writes:vec![],reads:result.artifacts.iter().map(|a|a.path.clone()).collect(),response:json!({"approved":approved,"reason":"Controlled independent review of the observed candidate; agreement alone supplies no objective confirmation"}).to_string()},Some(submission)).await?;
        let review: Review = serde_json::from_str(&native.text)?;
        let decision = DecisionRecord {
            id: new_id(),
            session_id: self.session.id.clone(),
            kind: "candidate_review".into(),
            actor: Some(native.assignment.agent_id),
            reason: review.reason,
            outcome: Some(if review.approved {
                DecisionOutcome::Accepted {
                    confirmation: ConfirmationStatus::Unconfirmed,
                }
            } else {
                DecisionOutcome::Rejected
            }),
            links: RecordLinks {
                task: result.task.clone(),
                result: Some(result.clone()),
                assignment_id: Some(native.assignment.id),
                invocation_id: Some(native.invocation.id),
                ..Default::default()
            },
            created_at: now(),
        };
        self.store.record_decision(&decision)?;
        Ok(decision)
    }
    pub async fn check(&self, submission: &DecisionRecord) -> Result<Vec<DecisionRecord>> {
        let result = submission.links.result.as_ref().unwrap();
        let contract = self
            .store
            .decisions(&self.session.id)?
            .into_iter()
            .find(|d| Some(&d.id) == result.contract_id.as_ref())
            .context("Missing trusted check contract")?;
        let captured = contract.links.acceptance_contract.unwrap();
        let mut records = Vec::new();
        for check in &captured.contract.checks {
            let verifier_current = || {
                captured.verifier_digests.iter().all(|(path, digest)| {
                    std::fs::read(path).is_ok_and(|bytes| bytes_digest(&bytes) == *digest)
                })
            };
            ensure!(
                verifier_current(),
                "Captured verifier changed before checking"
            );
            let inputs = captured
                .contract
                .inputs
                .iter()
                .map(|p| FileSnapshot::capture(&self.directory, p))
                .collect::<Result<Vec<_>>>()?;
            let before = result
                .artifacts
                .iter()
                .map(|a| FileSnapshot::capture(&self.directory, &a.path))
                .collect::<Result<Vec<_>>>()?;
            ensure!(
                before == result.artifacts && inputs == captured.inputs,
                "Check requires the current captured result and source version"
            );
            let output = BuiltinConfirmationChecker
                .execute(
                    check,
                    &self.directory,
                    &before,
                    &inputs,
                    CancellationToken::new(),
                )
                .await?;
            let after = result
                .artifacts
                .iter()
                .map(|a| FileSnapshot::capture(&self.directory, &a.path))
                .collect::<Result<Vec<_>>>()?;
            ensure!(
                verifier_current(),
                "Captured verifier changed while checking"
            );
            let evidence = CheckEvidence {
                checker: captured.checker.clone(),
                check_id: check.id.clone(),
                contract_id: contract.id.clone(),
                criterion_ids: check.criterion_ids.clone(),
                outcome: match output.exit_code {
                    Some(0) => ConfirmationCheckOutcome::Passed,
                    Some(_) => ConfirmationCheckOutcome::Failed,
                    None => ConfirmationCheckOutcome::Inconclusive,
                },
                inputs,
                artifacts_after: after,
                stdout: output.stdout,
                stderr: output.stderr,
                exit_code: output.exit_code,
            };
            let decision = DecisionRecord {
                id: new_id(),
                session_id: self.session.id.clone(),
                kind: "check_observed".into(),
                actor: None,
                reason: "Executed the installed trusted check over the captured result and inputs"
                    .into(),
                outcome: None,
                links: RecordLinks {
                    task: result.task.clone(),
                    result: Some(result.clone()),
                    check: Some(evidence),
                    ..Default::default()
                },
                created_at: now(),
            };
            self.store.record_decision(&decision)?;
            records.push(decision);
        }
        Ok(records)
    }
    pub fn accept(
        &self,
        task: &mut Task,
        submission: &DecisionRecord,
        review: &DecisionRecord,
        approved: bool,
    ) -> Result<DecisionRecord> {
        let result = submission.links.result.as_ref().unwrap();
        let (grade, confirmation_ids, _) =
            self.store.confirmation_grade(&self.session.id, result)?;
        let mut next = task.clone();
        next.review(review.actor.as_deref().unwrap(), approved, 8)?;
        let decision = DecisionRecord {
            id: new_id(),
            session_id: self.session.id.clone(),
            kind: if approved {
                "task_accepted"
            } else {
                "task_rejected"
            }
            .into(),
            actor: review.actor.clone(),
            reason: if approved {
                "Runtime acceptance requested against the actual independent review"
            } else {
                "Runtime rejected a stale or failing candidate"
            }
            .into(),
            outcome: Some(if approved {
                DecisionOutcome::Accepted {
                    confirmation: grade,
                }
            } else {
                DecisionOutcome::Rejected
            }),
            links: RecordLinks {
                task: result.task.clone(),
                result: Some(result.clone()),
                review_ids: vec![review.id.clone()],
                assignment_id: review.links.assignment_id.clone(),
                invocation_id: review.links.invocation_id.clone(),
                confirmation_ids,
                ..Default::default()
            },
            created_at: now(),
        };
        self.store.save_task_with_decision(&next, &decision)?;
        *task = next;
        Ok(decision)
    }
    pub fn aggregate(&self, submissions: &[DecisionRecord]) -> Result<DecisionRecord> {
        let mut producers = Vec::new();
        let mut artifacts = Vec::new();
        let mut criteria = Vec::new();
        for submission in submissions {
            let result = submission.links.result.as_ref().unwrap();
            producers.extend(result.producer_assignment_ids.clone());
            artifacts.extend(result.artifacts.clone());
            criteria.extend(result.criteria.iter().cloned().map(|mut criterion| {
                criterion.id = format!("{}:{}", result.id, criterion.id);
                criterion
            }));
        }
        producers.sort();
        producers.dedup();
        let result = ResultVersion {
            id: self.session.id.clone(),
            version: 1,
            task: None,
            task_definition: None,
            summary: "Observed aggregate of independently accepted controlled contributions".into(),
            criteria_version: content_digest(&serde_json::to_string(&criteria)?),
            criteria,
            contract_id: None,
            producer_assignment_ids: producers,
            artifacts,
            component_ids: submissions.iter().map(|s| s.id.clone()).collect(),
        };
        let decision = DecisionRecord {
            id: new_id(),
            session_id: self.session.id.clone(),
            kind: "result_aggregated".into(),
            actor: None,
            reason: "Runtime assembled current accepted components without spreading confirmation"
                .into(),
            outcome: None,
            links: RecordLinks {
                result: Some(result),
                ..Default::default()
            },
            created_at: now(),
        };
        self.store.record_decision(&decision)?;
        Ok(decision)
    }
}
pub fn exact_contract(
    title: &str,
    criterion: &str,
    path: &str,
    expected: &[u8],
) -> AcceptanceContract {
    AcceptanceContract {
        knowledge_correction: None,
        task_title: title.into(),
        criteria: vec![AcceptanceCriterion {
            id: criterion.into(),
            description: format!("Exact controlled content for {title}"),
        }],
        artifacts: vec![path.into()],
        inputs: vec![],
        checks: vec![TrustedCheck {
            id: format!("check-{criterion}"),
            criterion_ids: vec![criterion.into()],
            assertion: CheckAssertion::ExactBytes {
                artifact: path.into(),
                expected: expected.into(),
            },
        }],
    }
}

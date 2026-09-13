use super::*;
use anyhow::ensure;

impl Engine {
    /// Explicit trusted local ingress. Sufficient backend evidence is required
    /// before an independent, normally admitted inspection can resolve effects.
    /// The caller then uses control_recovery(Continue) and the existing run API.
    pub async fn inspect_recovery(
        &self,
        command: &RecoveryInspectionCommand,
    ) -> Result<RecoveryInspectionReceipt> {
        ensure!(
            !command.command_id.trim().is_empty(),
            "owner_command: inspection command ID required"
        );
        if let Some(receipt) = self.store.recovery_inspection_receipt(command)? {
            return Ok(receipt);
        }
        let session = self.store.session(&command.session_id)?;
        let policy = self
            .store
            .session_policy(&session.id)?
            .context("Inspection requires captured session policy")?;
        let project = self.store.project(&policy.cwd)?;
        ensure!(
            project.id == session.project_id,
            "inspection_binding: foreign project"
        );
        let owner = Arc::new(crate::WorkspaceOwner::acquire(&self.store, &project)?);
        let _session_lock = self.store.lock_session(&session)?;
        // The second receipt check covers a command completed while locks were acquired.
        if let Some(receipt) = self.store.recovery_inspection_receipt(command)? {
            return Ok(receipt);
        }
        self.owner_boundary(&session.id)?;
        let stage = self
            .store
            .recovery_stages(&session.id)?
            .into_iter()
            .find(|s| s.id == command.stage_id)
            .context("Unknown recovery stage")?;
        self.store.recovery_inspection_effects(command)?;
        let trace = self.store.trace(&session.id)?;
        ensure!(
            !trace
                .assignments
                .iter()
                .any(|a| a.state == InvocationState::Running),
            "active_responsibility: native work must end before effect inspection"
        );
        let limits = trace
            .budget
            .as_ref()
            .context("Missing captured inspection budget")?
            .limits
            .clone();
        let workspace = Workspace::open(
            &project.path,
            &self.store.session_dir(&session).join("workspace"),
        )?;
        let server =
            Arc::new(TeamServer::start(self.store.clone(), &session, self.events.clone()).await?);
        let ctx = RunContext {
            workspace_owner: owner,
            session: session.clone(),
            server,
            workspace,
            turns: Arc::new(AtomicUsize::new(usize::try_from(trace.usage.total.calls)?)),
            permits: Arc::new(Semaphore::new(limits.parallel)),
            limits,
        };
        let id = new_id();
        let access = WorkspaceAccessDecision {
            reservation_id: id.clone(), policy: self.workspace_policy_identity.clone(),
            backend: self.backend_identity.clone(), directory: policy.cwd.canonicalize()?,
            backend_access: WorkspaceAccess::WriteAll, effective_access: WorkspaceAccess::WriteAll,
            local_effect_scope: None,
            rationale: "Protect observed local effects through independent read-only inspection and resolution commitment".into(),
        };
        let lease = self
            .acquire_workspace(&ctx, &id, None, stage.task.clone(), &access)
            .await?;
        let observed = self.store.recovery_inspection_effects(command)?;
        let producers = stage
            .plan
            .iter()
            .map(|p| p.producer_assignment_id.clone())
            .chain(
                stage
                    .result
                    .iter()
                    .flat_map(|r| r.producer_assignment_ids.clone()),
            )
            .collect::<HashSet<_>>();
        let excluded = trace
            .assignments
            .iter()
            .filter(|a| producers.contains(&a.id))
            .map(|a| a.agent_id.clone())
            .chain(stage.failures.iter().map(|f| f.agent_id.clone()))
            .collect::<HashSet<_>>();
        let peers = self
            .eligible_agents(&session.id)?
            .into_iter()
            .filter(|a| !excluded.contains(&a.id))
            .collect::<Vec<_>>();
        let mut engine = self.clone();
        engine.workspace_parent = Some(lease.id.clone());
        engine.require_recovery_read_only = true;
        engine.recovery_inspection_binding = Some(command.clone());
        let inspector = engine.choose(
            &ctx,
            &peers,
            "verification",
            "standard",
            "independent recovery effect inspection",
            "review",
            None,
        )?;
        let prompt = format!("Independently inspect this recovery obligation and the complete observed local effects. The trusted runtime has checked backend termination and its enforced local-only effect boundary. Assess whether the saved review can safely continue READ-ONLY using this exact observed state. Do not infer task completion or acceptance. Do not modify files. Unknown or inconsistent effects require rejection.\nStage: {}\nSaved task/plan/result: {}\nRecorded termination, access origins and observed files: {}\nReturn ONLY JSON {{\"approved\":true|false,\"reason\":\"concrete inspection findings\"}}.", stage.id, serde_json::to_string(&(&stage.task, &stage.plan, &stage.result))?, serde_json::to_string(&observed)?);
        let response = engine
            .ask_scoped_once(
                &ctx,
                &inspector,
                &ctx.workspace.directory,
                "review",
                &prompt,
                true,
                None,
                &mut None,
            )
            .await?;
        let review: Review = parse_response(&response.text)?;
        if !review.approved {
            self.store.record_decision(&DecisionRecord {
                id: new_id(),
                session_id: session.id,
                kind: "recovery_inspection_rejected".into(),
                actor: Some(inspector.id),
                reason: review.reason.clone(),
                outcome: Some(DecisionOutcome::Rejected),
                links: RecordLinks {
                    assignment_id: Some(response.assignment_id),
                    invocation_id: Some(response.invocation_id),
                    ..Default::default()
                },
                created_at: now(),
            })?;
            bail!("inspection_rejected: {}", review.reason);
        }
        self.store.commit_recovery_inspection(&DecisionRecord {
            id: new_id(),
            session_id: session.id,
            kind: "recovery_effects_inspected".into(),
            actor: Some(inspector.id.clone()),
            reason: review.reason,
            outcome: Some(DecisionOutcome::Accepted {
                confirmation: ConfirmationStatus::Confirmed,
            }),
            links: RecordLinks {
                assignment_id: Some(response.assignment_id.clone()),
                invocation_id: Some(response.invocation_id.clone()),
                recovery_inspection: Some(Box::new(RecoveryInspection {
                    command: command.clone(),
                    task: stage.task,
                    plan: stage.plan,
                    result: stage.result,
                    effects: observed,
                    reviewer: SavedResponse {
                        text: response.text,
                        assignment_id: response.assignment_id,
                        invocation_id: response.invocation_id,
                        agent_id: inspector.id,
                    },
                })),
                ..Default::default()
            },
            created_at: now(),
        })
    }
}

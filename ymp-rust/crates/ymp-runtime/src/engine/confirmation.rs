use super::*;

impl Engine {
    fn requested_contracts(&self) -> Result<Option<&[AcceptanceContract]>> {
        if self.acceptance_contracts.is_empty() {
            return Ok(self.config.acceptance_contracts.as_deref());
        }
        anyhow::ensure!(
            self.config
                .acceptance_contracts
                .as_ref()
                .is_none_or(|configured| configured == &self.acceptance_contracts),
            "Conflicting configuration and client acceptance contracts"
        );
        Ok(Some(&self.acceptance_contracts))
    }

    pub(super) fn prepare_contracts(
        &self,
        directory: &Path,
    ) -> Result<Vec<CapturedAcceptanceContract>> {
        let contracts = self.requested_contracts()?.unwrap_or_default();
        validate_contract_targets(contracts)?;
        contracts
            .iter()
            .map(|contract| {
                CapturedAcceptanceContract::capture(
                    contract.clone(),
                    directory,
                    self.confirmation_checker.identity(),
                )
            })
            .collect()
    }

    fn captured_contracts(&self, session: &str) -> Result<Vec<AcceptanceContract>> {
        let contracts = self
            .store
            .trace(session)?
            .decisions
            .into_iter()
            .filter_map(|d| d.links.acceptance_contract.map(|c| c.contract))
            .collect::<Vec<_>>();
        validate_contract_targets(&contracts)?;
        Ok(contracts)
    }

    pub(super) fn validate_resumed_contracts(&self, session: &str) -> Result<()> {
        if let Some(requested) = self.requested_contracts()? {
            validate_contract_targets(requested)?;
            let captured = self.captured_contracts(session)?;
            anyhow::ensure!(
                requested.len() == captured.len() && requested.iter().all(|c| captured.contains(c)),
                "Acceptance contracts are immutable on resume; restore the captured definitions or omit acceptance_contracts to use the capture"
            );
        }
        // Never recapture input bytes or verifier contents on resume.
        Ok(())
    }

    pub(super) fn acceptance_requirements(&self, session: &str) -> Result<String> {
        let requirements = self
            .captured_contracts(session)?
            .iter()
            .map(AcceptanceRequirements::from)
            .collect::<Vec<_>>();
        if requirements.is_empty() {
            return Ok(String::new());
        }
        let mut predecessors = String::new();
        for decision in self.store.decisions(session)? {
            if let Some(binding) = decision
                .links
                .acceptance_contract
                .as_ref()
                .and_then(|c| c.contract.knowledge_correction.as_ref())
            {
                let old = self.store.correction_predecessor(&decision.id)?;
                let excerpt = old.content.chars().take(4000).collect::<String>();
                predecessors.push_str(&format!("\nCorrection predecessor {}@{} (historical context, not current evidence): {}\n{}\nSource session: {}\n", old.id, binding.target.version, old.title, excerpt, old.source_session));
            }
        }
        Ok(format!("\nTrusted acceptance requirements (captured before planning):\n{}\nEvery plan and revision must contain exactly one task with each declared task_title, preserving its exact spelling. Implement and inspect the declared criteria, artifacts and inputs. These bindings remain mandatory throughout this session. Model-suggested shell commands do not replace trusted checks.\n{predecessors}", serde_json::to_string(&requirements)?))
    }

    pub(super) fn validate_contract_bindings<'a>(
        &self,
        session: &str,
        titles: impl Iterator<Item = &'a str>,
    ) -> Result<()> {
        let titles = titles.collect::<Vec<_>>();
        for contract in self.captured_contracts(session)? {
            let matches = titles
                .iter()
                .filter(|title| **title == contract.task_title)
                .count();
            anyhow::ensure!(matches == 1,
                "Acceptance contract target {:?} requires exactly one planned task; found {matches}", contract.task_title);
        }
        Ok(())
    }

    pub(super) fn candidate_result(
        &self,
        ctx: &RunContext,
        task: &Task,
        producer: &str,
    ) -> Result<ResultVersion> {
        let trace = self.store.trace(&ctx.session.id)?;
        let contract = trace.decisions.iter().find(|d| {
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
                    id: "task-quality".into(),
                    description: task.description.clone(),
                }]
            });
        let artifacts = contract
            .and_then(|d| d.links.acceptance_contract.as_ref())
            .map(|c| {
                c.contract
                    .artifacts
                    .iter()
                    .map(|p| FileSnapshot::capture(&ctx.workspace.directory, p))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(ResultVersion {
            id: task.id.clone(),
            version: task.attempts,
            task: Some(TaskAttemptRef::from(task)),
            summary: task.result.clone().unwrap_or_default(),
            task_definition: Some(TaskDefinition::from(task)),
            criteria_version: content_digest(&serde_json::to_string(&criteria)?),
            criteria,
            contract_id: contract.map(|d| d.id.clone()),
            producer_assignment_ids: vec![producer.into()],
            artifacts,
            component_ids: vec![],
        })
    }

    pub(super) fn submitted_result(&self, ctx: &RunContext, task: &Task) -> Result<ResultVersion> {
        let trace = self.store.trace(&ctx.session.id)?;
        if let Some(result) = trace
            .decisions
            .iter()
            .rev()
            .find(|d| {
                d.kind == "result_submitted"
                    && d.links.task.as_ref() == Some(&TaskAttemptRef::from(task))
            })
            .and_then(|d| d.links.result.clone())
        {
            return Ok(result);
        }
        // Recovery records actual files for inspection without claiming the
        // interrupted producer completed or repeating its side effects.
        let producer = trace
            .assignments
            .iter()
            .rev()
            .find(|a| {
                a.purpose == "execute" && a.task.as_ref() == Some(&TaskAttemptRef::from(task))
            })
            .context("Missing producing assignment for inspection")?;
        let invocation = trace
            .invocations
            .iter()
            .find(|i| i.assignment_id == producer.id)
            .context("Missing producing invocation")?;
        let result = self.candidate_result(ctx, task, &producer.id)?;
        self.store.record_decision(&DecisionRecord { id: new_id(), session_id: ctx.session.id.clone(), kind: "result_submitted".into(), actor: Some(producer.agent_id.clone()), reason: "Runtime captured interrupted artifacts for independent inspection; production completion is unknown".into(), outcome: None, links: RecordLinks { task: result.task.clone(), assignment_id: Some(producer.id.clone()), invocation_id: Some(invocation.id.clone()), result: Some(result.clone()), ..Default::default() }, created_at: now() })?;
        Ok(result)
    }

    pub(super) async fn confirm_result(
        &self,
        ctx: &RunContext,
        result: &ResultVersion,
    ) -> Result<String> {
        let Some(contract_id) = &result.contract_id else {
            return Ok(
                "No trusted objective acceptance contract; result quality remains unconfirmed."
                    .into(),
            );
        };
        let trace = self.store.trace(&ctx.session.id)?;
        let contract = trace
            .decisions
            .iter()
            .find(|d| &d.id == contract_id)
            .and_then(|d| d.links.acceptance_contract.as_ref())
            .context("Missing captured acceptance contract")?;
        let directory = &ctx.workspace.directory;
        let mut log = String::new();
        for check in &contract.contract.checks {
            self.owner_boundary(&ctx.session.id)?;
            let inputs = contract
                .contract
                .inputs
                .iter()
                .map(|p| FileSnapshot::capture(directory, p))
                .collect::<Result<Vec<_>>>()?;
            let before = result
                .artifacts
                .iter()
                .map(|a| FileSnapshot::capture(directory, &a.path))
                .collect::<Result<Vec<_>>>()?;
            let code_current = || {
                contract
                    .verifier_digests
                    .iter()
                    .all(|(p, d)| std::fs::read(p).is_ok_and(|b| bytes_digest(&b) == *d))
            };
            let mut outcome = ConfirmationCheckOutcome::Inconclusive;
            let mut stdout = Vec::new();
            let mut stderr;
            let mut exit_code = None;
            if before == result.artifacts && inputs == contract.inputs && code_current() {
                anyhow::ensure!(self.confirmation_checker.identity() == contract.checker, "Captured checker implementation is unavailable; restore its ID/version before resuming");
                let execution = tokio::select! { _=self.cancel.cancelled()=>bail!("Check cancelled"), output=tokio::time::timeout(std::time::Duration::from_secs(300), self.confirmation_checker.execute(check, directory, &before, &inputs, self.cancel.child_token()))=>output };
                match execution {
                    Ok(Ok(output)) => {
                        exit_code = output.exit_code;
                        stdout = output.stdout;
                        stderr = output.stderr;
                        outcome = match exit_code {
                            Some(0) => ConfirmationCheckOutcome::Passed,
                            Some(_) => ConfirmationCheckOutcome::Failed,
                            None => ConfirmationCheckOutcome::Inconclusive,
                        };
                    }
                    _ => {
                        stderr = b"Trusted check could not complete; no competence outcome observed"
                            .to_vec()
                    }
                }
            } else {
                stderr = b"Candidate, input or verifier version changed before checking".to_vec();
            }
            let artifacts_after = result
                .artifacts
                .iter()
                .map(|a| FileSnapshot::capture(directory, &a.path))
                .collect::<Result<Vec<_>>>()?;
            if artifacts_after != before
                || !contract.inputs.iter().all(|s| s.current(directory))
                || !code_current()
            {
                outcome = ConfirmationCheckOutcome::Inconclusive;
            }
            if stdout.len() + stderr.len() > 4 * 1024 * 1024 {
                outcome = ConfirmationCheckOutcome::Inconclusive;
                stdout.truncate(2 * 1024 * 1024);
                stderr.truncate(2 * 1024 * 1024);
            }
            log.push_str(&format!(
                "Trusted check {} ({:?}): {outcome:?}\n",
                check.id, check.criterion_ids
            ));
            let evidence = CheckEvidence {
                checker: contract.checker.clone(),
                check_id: check.id.clone(),
                contract_id: contract_id.clone(),
                criterion_ids: check.criterion_ids.clone(),
                outcome,
                inputs,
                artifacts_after,
                stdout,
                stderr,
                exit_code,
            };
            self.store.record_decision(&DecisionRecord {
                id: new_id(),
                session_id: ctx.session.id.clone(),
                kind: "check_observed".into(),
                actor: None,
                reason: format!("Runtime observed {}: {:?}", check.id, evidence.outcome),
                outcome: None,
                links: RecordLinks {
                    task: result.task.clone(),
                    result: Some(result.clone()),
                    check: Some(evidence),
                    ..Default::default()
                },
                created_at: now(),
            })?;
        }
        Ok(log)
    }

    pub(super) fn aggregate_result(
        &self,
        ctx: &RunContext,
        tasks: &[Task],
    ) -> Result<ResultVersion> {
        let trace = self.store.trace(&ctx.session.id)?;
        let mut components = Vec::new();
        let mut producers = Vec::new();
        let mut artifacts = Vec::new();
        let mut criteria = Vec::new();
        for task in tasks {
            let component = trace
                .decisions
                .iter()
                .rev()
                .find(|d| {
                    d.kind == "result_submitted"
                        && d.links.task.as_ref() == Some(&TaskAttemptRef::from(task))
                })
                .context("Accepted task has no result identity")?;
            let child = component
                .links
                .result
                .as_ref()
                .context("Missing candidate version")?;
            if !child
                .artifacts
                .iter()
                .all(|a| a.current(&ctx.workspace.directory))
            {
                self.store.record_decision(&DecisionRecord { id: new_id(), session_id: ctx.session.id.clone(), kind: "result_invalidated".into(), actor: None, reason: "Accepted artifact changed; fresh production version and independent review are required".into(), outcome: None, links: RecordLinks { task: child.task.clone(), result: Some(child.clone()), ..Default::default() }, created_at: now() })?;
                bail!("Accepted artifact version changed; result requires fresh inspection and review");
            }
            components.push(component.id.clone());
            producers.extend(child.producer_assignment_ids.clone());
            artifacts.extend(child.artifacts.clone());
            criteria.extend(child.criteria.iter().cloned().map(|mut c| {
                c.id = format!("{}:{}", child.id, c.id);
                c
            }));
        }
        producers.sort();
        producers.dedup();
        let result = ResultVersion {
            id: ctx.session.id.clone(),
            version: trace
                .decisions
                .iter()
                .filter(|d| d.kind == "result_aggregated")
                .count()
                + 1,
            task: None,
            task_definition: None,
            summary: tasks
                .iter()
                .filter_map(|t| t.result.clone())
                .collect::<Vec<_>>()
                .join("\n"),
            criteria_version: content_digest(&serde_json::to_string(&criteria)?),
            criteria,
            contract_id: None,
            producer_assignment_ids: producers,
            artifacts,
            component_ids: components,
        };
        if let Some(saved) = trace
            .decisions
            .iter()
            .rev()
            .find(|d| d.kind == "result_aggregated")
            .and_then(|d| d.links.result.as_ref())
        {
            let mut candidate = result.clone();
            candidate.version = saved.version;
            if &candidate == saved {
                return Ok(saved.clone());
            }
        }
        self.store.record_decision(&DecisionRecord {
            id: new_id(),
            session_id: ctx.session.id.clone(),
            kind: "result_aggregated".into(),
            actor: None,
            reason: "Runtime captured all accepted component versions for independent final review"
                .into(),
            outcome: None,
            links: RecordLinks {
                result: Some(result.clone()),
                ..Default::default()
            },
            created_at: now(),
        })?;
        Ok(result)
    }
}

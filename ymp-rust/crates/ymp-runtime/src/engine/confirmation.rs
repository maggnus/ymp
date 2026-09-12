use super::*;

impl Engine {
    pub(super) fn capture_contracts(&self, session: &Session, directory: &Path) -> Result<()> {
        for contract in &self.acceptance_contracts {
            let captured = CapturedAcceptanceContract::capture(
                contract.clone(),
                directory,
                self.confirmation_checker.identity(),
            )?;
            self.store.record_decision(&DecisionRecord { id: new_id(), session_id: session.id.clone(), kind: "acceptance_contract_captured".into(), actor: None, reason: "Trusted client supplied acceptance criteria and check bindings before execution".into(), outcome: None, links: RecordLinks { acceptance_contract: Some(captured), ..Default::default() }, created_at: now() })?;
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

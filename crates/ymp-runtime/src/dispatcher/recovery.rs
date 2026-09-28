use super::*;
use ymp_domain::{
    Ref,
    journal::{Decision, EscalationStep},
    task::SessionStatus,
};
use ymp_kernel::progress::{
    DiagnosisRequest, EscalationRequest, MonitorRequest, VerificationEstimate,
};
impl<J: Journal + 'static, C: ContentStore + 'static> Dispatcher<J, C> {
    pub(super) fn verification_capacity(
        &self,
        context: Option<&ApplicabilityContext>,
    ) -> Result<()> {
        let view = self.view()?;
        let ledger = self
            .app
            .acceptance(self.content.clone())
            .ledger(self.owner.session())?;
        if view
            .criteria()
            .iter()
            .filter(|c| c.required)
            .all(|c| ledger.entries[&c.id].status == LedgerStatus::Satisfied)
        {
            return Ok(());
        }
        let minimum = self
            .app
            .progress()
            .verification_inputs(self.owner.session(), context)?
            .iter()
            .map(|input| self.policies.cost.estimate(input).map(|p| p.value.expected))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .min_by(|a, b| a.get().total_cmp(&b.get()));
        if minimum.is_none_or(|cost| {
            !view
                .treasury()
                .unwrap()
                .remaining(Purpose::Verification)
                .is_ok_and(|remaining| remaining >= cost)
        }) {
            return Err(Denial::new(
                "verification_unavailable",
                "No affordable eligible minimum verification for unmet required criteria",
            ));
        }
        Ok(())
    }
    /// Polling the same meaningful boundary never manufactures a new progress step.
    pub(super) fn monitor(&self) -> Result<(Ref, bool)> {
        let service = self.app.progress();
        let view = self.view()?;
        let input = service.input(self.owner.session(), self.at()?)?;
        if let Some((r, previous, d)) = view.progress().monitor()
            && previous.boundary == input.boundary
        {
            return Ok((r.clone(), d.outcome.stall));
        }
        let proposal = self.policies.monitor.assess(&input)?;
        let stalled = proposal.value.stall;
        let reference = service.assess(
            &self.owner,
            MonitorRequest {
                expected_revision: view.revision(),
                input,
                proposal,
            },
        )?;
        Ok((reference, stalled))
    }
    pub(super) fn blocked(&mut self, reason: &str) -> Result<Tick> {
        let view = self.view()?;
        if view
            .treasury()
            .is_some_and(|book| book.reporting_mode.is_some())
        {
            self.draining = true;
            return self.report();
        }
        if !view.owner_stopped() {
            self.app.phase(
                &self.owner,
                self.at()?,
                SessionStatus::Blocked(reason.into()),
            )?;
            // A lost executor retains effects/holds. Diagnosis must not assert a work boundary.
            if view.method().is_some()
                && !view
                    .admission()
                    .assignments()
                    .values()
                    .any(|a| ymp_kernel::gatekeeper::unresolved(&view, a))
            {
                let (progress, _) = self.monitor()?;
                let service = self.app.progress();
                let view = self.view()?;
                let candidate = view
                    .results()
                    .items()
                    .values()
                    .next()
                    .and_then(|i| i.attempts.last())
                    .and_then(|a| view.results().attempts()[a].attempt.result.as_ref())
                    .and_then(|r| view.results().results().get(r));
                let context = candidate.map(|r| self.candidate_context(r)).transpose()?;
                let estimates = service
                    .verification_inputs(self.owner.session(), context.as_ref())?
                    .into_iter()
                    .map(|input| {
                        let proposal = self.policies.cost.estimate(&input)?;
                        Ok(VerificationEstimate {
                            decision: Decision {
                                input: Digest::of_value(&input)?,
                                outcome: proposal.value.clone(),
                                proposal,
                                effective: self.policies.cost.selection().clone(),
                                selection_change: None,
                            },
                            input,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let input = service.diagnosis_input(
                    self.owner.session(),
                    context.as_ref(),
                    &estimates,
                    self.at()?,
                )?;
                let diagnosis = service.diagnose(
                    &self.owner,
                    DiagnosisRequest {
                        expected_revision: view.revision(),
                        at: self.at()?,
                        progress,
                        context,
                        estimates,
                        proposal: self.policies.diagnosis.diagnose(&input)?,
                        input,
                    },
                )?;
                let input = service.escalation_input(self.owner.session(), &diagnosis)?;
                let proposal = self.policies.escalation.next(&input)?;
                let stop = proposal.value.step == EscalationStep::StopPreserving
                    && proposal.value.limitation.is_none();
                let view = self.view()?;
                let step = service.escalate(
                    &self.owner,
                    EscalationRequest {
                        expected_revision: view.revision(),
                        at: self.at()?,
                        input,
                        proposal,
                    },
                )?;
                if stop {
                    service.stop_preserving(
                        &self.owner,
                        &step,
                        self.app.treasury(),
                        &self.budget,
                        self.at()?,
                    )?;
                }
            }
        }
        match self.recovery_step() {
            Ok(Some(next)) => return Ok(next),
            Ok(None) => {}
            Err(error)
                if matches!(
                    error.code.as_str(),
                    "recovery_unresolved"
                        | "assignment_unavailable"
                        | "recovery_budget"
                        | "retry_limit"
                        | "escalation_stale"
                        | "recovery_context_hidden"
                ) =>
            {
                self.draining = true
            }
            Err(error) => return Err(error),
        }
        self.report()
    }
}

impl<J: Journal + 'static, C: ContentStore + 'static> Dispatcher<J, C> {
    pub(super) fn recovery_step(&mut self) -> Result<Option<Tick>> {
        use ymp_kernel::progress::{ProgressRecorded, RecoveryOutcome};
        let view = self.view()?;
        let Some((reference, input, plan)) =
            view.progress().history.iter().rev().find_map(|(r, e)| {
                if let ProgressRecorded::Escalation { input, decision } = e {
                    Some((r, input.as_ref(), &decision.outcome))
                } else {
                    None
                }
            })
        else {
            return Ok(None);
        };
        if plan.limitation.is_some() {
            return Ok(None);
        }
        if !matches!(
            plan.step,
            EscalationStep::Retry | EscalationStep::AddVerifier
        ) {
            return Ok(None);
        }
        let action = view.progress().history.iter().find_map(|(_, e)| {
            if let ProgressRecorded::Action { step, outcome } = e {
                (step == reference).then_some(outcome.as_ref())
            } else {
                None
            }
        });
        if let Some(RecoveryOutcome::Work(work)) = action {
            if view
                .admission()
                .assignments()
                .values()
                .any(|a| a.intent.assignment.contribution == work.contribution.id)
            {
                if self.completed(work.contribution.id.as_str())? {
                    let source = view.execution().invocations()
                        [&id(&format!("call-{}", work.contribution.id))?]
                        .end
                        .as_ref()
                        .unwrap();
                    if !view
                        .session_state()
                        .recovered
                        .iter()
                        .any(|(r, _)| r == source)
                    {
                        self.app
                            .recovery_consumed(&self.owner, self.at()?, source.clone())?;
                        return Ok(Some(Tick::Advanced));
                    }
                    return Ok(None);
                }
                return Err(Denial::new(
                    "recovery_unresolved",
                    "A recovery invocation cannot be duplicated",
                ));
            }
            let paths = if let Some(ContributionSubject::WorkItem(_)) = &work.contribution.subject {
                let item = &view.results().items()[input.item.as_ref().unwrap()];
                std::iter::once((WorkspacePath::root(), LockMode::Read))
                    .chain(item.writes.iter().cloned().map(|p| (p, LockMode::Write)))
                    .collect()
            } else {
                vec![(WorkspacePath::root(), LockMode::Read)]
            };
            return self
                .launch(
                    work.contribution.id.as_str(),
                    work.contribution.kind,
                    if work.retry_of.is_some() {
                        RoleKind::Producer
                    } else {
                        RoleKind::Researcher
                    },
                    work.contribution.subject.clone(),
                    work.contribution.needs.clone(),
                    paths,
                    None,
                )
                .map(Some);
        }
        if action.is_some() {
            return Ok(None);
        }
        let service = self.app.progress();
        let contribution = id(&format!("recovery-{}", reference.version))?;
        if plan.step == EscalationStep::Retry {
            let item = &view.results().items()[input
                .item
                .as_ref()
                .ok_or_else(|| Denial::new("retry_item", "No retry item"))?];
            let attempt = item
                .attempts
                .last()
                .ok_or_else(|| Denial::new("retry_attempt", "No prior attempt"))?;
            let prior = &view.results().attempts()[attempt];
            let assignment = &view.admission().assignments()[&prior.attempt.assignment]
                .intent
                .assignment;
            let expires = view.coordination().commitments()[&assignment.commitment]
                .lease
                .expires;
            if self.clock.now()? <= expires {
                return Ok(Some(Tick::Waiting {
                    until: expires.saturating_add(1),
                }));
            }
            let prepared = self
                .app
                .recover_attempt(&self.owner, &self.results, attempt)?;
            service.retry(
                &self.owner,
                reference,
                self.admission.gatekeeper().as_ref(),
                &self.results,
                &prepared,
                contribution,
                self.policies.cost.as_ref(),
                self.at()?,
            )?;
        } else {
            let diagnosis = view.progress().diagnosis(&input.diagnosis).unwrap();
            let producer = diagnosis
                .context
                .as_ref()
                .and_then(|c| {
                    view.results()
                        .results()
                        .values()
                        .find(|r| r.reference().as_ref() == Ok(&c.result))
                })
                .map(|r| &r.producer);
            let eligible = self.app.plans().eligible(
                self.owner.session(),
                &BTreeSet::from([Capability::ReadFiles]),
                producer,
            )?;
            let profile = view
                .registry()
                .unwrap()
                .decisions
                .iter()
                .find(|d| d.outcome == Readiness::Ready && eligible.contains(&d.profile.agent))
                .ok_or_else(|| {
                    Denial::new(
                        "assignment_unavailable",
                        "No independent existing member for diagnosis",
                    )
                })?
                .profile
                .clone();
            let provider = view
                .registry()
                .unwrap()
                .input
                .facts
                .agents
                .iter()
                .find(|a| a.id == profile.agent)
                .unwrap()
                .provider
                .clone();
            let demand = ResourceDemand {
                contribution: contribution.erased(),
                kind: ContributionKind::Research,
                difficulty: Difficulty::Simple,
                provider,
                profile: profile.clone(),
            };
            let estimate_input = ymp_kernel::treasury::estimate_view(&view, &demand)?;
            let proposal = self.policies.cost.estimate(&estimate_input)?;
            let estimate = VerificationEstimate {
                decision: Decision {
                    input: Digest::of_value(&estimate_input)?,
                    outcome: proposal.value.clone(),
                    proposal,
                    effective: self.policies.cost.selection().clone(),
                    selection_change: None,
                },
                input: estimate_input,
            };
            service.add_verifier(
                &self.owner,
                reference,
                profile,
                contribution,
                estimate,
                self.at()?,
            )?;
        }
        Ok(Some(Tick::Advanced))
    }
}

impl<J: Journal + 'static, C: ContentStore + 'static> Dispatcher<J, C> {
    pub(super) fn reconcile(
        journal: &Arc<J>,
        gate: &ymp_kernel::gatekeeper::Gatekeeper<J, C>,
        session: &Id,
        at: u64,
    ) -> Result<bool> {
        let view = journal.view(session, None)?;
        for call in view.execution().invocations().values() {
            let assignment = &call.dispatch.assignment;
            let contribution = &view.coordination().contributions()[&assignment.contribution].value;
            if call.terminal == Some(InvocationTerminal::Completed)
                && call.confirmed_terminal
                && call.ended_at.is_some_and(|at| {
                    at <= view.coordination().commitments()[&assignment.commitment]
                        .lease
                        .expires
                })
                && ymp_kernel::execution::closed(&view, &assignment.id)
                && !matches!(
                    contribution.kind,
                    ContributionKind::Produce
                        | ContributionKind::Alternative
                        | ContributionKind::Integrate
                )
                && view.coordination().commitments()[&assignment.commitment].state
                    == CommitmentState::Active
            {
                Arbiter::new(journal.clone()).discharge(
                    gate,
                    session,
                    view.revision(),
                    at,
                    &assignment.commitment,
                    call.end.as_ref().unwrap(),
                )?;
                return Ok(true);
            }
        }
        for accepted in view
            .acceptances()
            .values()
            .filter(|a| a.acceptance.decision == AcceptanceDecision::Accepted)
        {
            let attempt = &view.results().attempts()[&accepted.attempt];
            let assignment = &view.admission().assignments()[&attempt.attempt.assignment]
                .intent
                .assignment;
            let commitment = &view.coordination().commitments()[&assignment.commitment];
            if commitment.state == CommitmentState::Active
                && accepted.acceptance.at <= commitment.lease.expires
            {
                Arbiter::new(journal.clone()).discharge(
                    gate,
                    session,
                    view.revision(),
                    at,
                    &commitment.id,
                    &accepted.acceptance.reference()?,
                )?;
                return Ok(true);
            }
        }
        if view
            .coordination()
            .commitments()
            .values()
            .any(|c| c.state == CommitmentState::Active && at > c.lease.expires)
        {
            Arbiter::new(journal.clone()).tick(gate, session, at)?;
            return Ok(true);
        }
        Ok(false)
    }
}

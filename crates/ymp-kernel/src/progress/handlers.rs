//! Recovery preserves actual grants, leases, receipts and owner capabilities.
use super::*;
use crate::{
    acceptance::{AcceptanceAuthority, RunCheck},
    arbiter::Arbiter,
    gatekeeper::Gatekeeper,
    journal::ContentStore,
    ports::checks::CheckRunner,
    results::{PreparedAttempt, Results},
    treasury::{BudgetControl, Treasury},
};
use ymp_domain::{identity::ExecutionProfile, plan::AttemptOutcome};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryWork {
    pub estimate: VerificationEstimate,
    pub step: Ref,
    pub contribution: Contribution,
    pub profile: ExecutionProfile,
    pub prompt: Prompt,
    pub retry_of: Option<Id<ymp_domain::plan::Attempt>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryOutcome {
    Run {
        old: Ref,
        new: Ref,
        outcome: CheckOutcome,
    },
    Work(Box<RecoveryWork>),
    Stopped,
    Unavailable(String),
}
impl RecoveryOutcome {
    pub fn refs(&self) -> Vec<Ref> {
        match self {
            Self::Run { old, new, .. } => vec![old.clone(), new.clone()],
            Self::Work(w) => w.prompt.basis.clone(),
            _ => vec![],
        }
    }
}
pub(super) fn used_steps(view: &SessionView) -> Vec<EscalationStep> {
    view.progress()
        .history
        .iter()
        .filter_map(|(_, event)| {
            let reference = match event {
                ProgressRecorded::Action { step, outcome }
                    if !matches!(outcome.as_ref(), RecoveryOutcome::Unavailable(_)) =>
                {
                    Some(step)
                }
                ProgressRecorded::Replaced { proposal, .. } => {
                    replacement::proposal(view, proposal).ok().map(|p| &p.step)
                }
                _ => None,
            };
            reference
                .and_then(|r| view.progress().step(r))
                .map(|(_, d)| d.outcome.step.clone())
        })
        .collect()
}
pub(super) fn bound_step<'a>(
    view: &'a SessionView,
    reference: &Ref,
    expected: &EscalationStep,
) -> Result<(&'a EscalationInput, &'a EscalationPlan)> {
    let (input, decision) = view
        .progress()
        .step(reference)
        .ok_or_else(|| Denial::new("escalation_missing", "No recorded escalation step"))?;
    let used = used_steps(view);
    if decision.outcome.step != *expected
        || decision.outcome.limitation.is_some()
        || !input.method.ladder.contains(expected)
        || view.method() != Some(&input.method)
        || used.iter().filter(|s| *s == expected).count()
            >= decision.outcome.limit.max_uses as usize
        || view.progress().history.iter().any(|(_, e)| match e {
            ProgressRecorded::Action { step, outcome } => {
                step == reference && !matches!(outcome.as_ref(), RecoveryOutcome::Unavailable(_))
            }
            ProgressRecorded::Replaced { proposal, .. } => {
                replacement::proposal(view, proposal).is_ok_and(|p| p.step == *reference)
            }
            _ => false,
        })
    {
        return Err(Denial::new(
            "escalation_authority",
            "Step is unavailable, stale, exhausted or already consumed",
        ));
    }
    if view.progress().diagnosis(&input.diagnosis).is_none() {
        return Err(Denial::new(
            "diagnosis_required",
            "Recovery requires its recorded diagnosis",
        ));
    }
    Ok((input, &decision.outcome))
}
pub(super) fn fresh(
    view: &SessionView,
    input: &EscalationInput,
    allowed: &[Ref],
    completing_retry: bool,
) -> Result<()> {
    let diagnosis = view.progress().diagnosis(&input.diagnosis).unwrap();
    let original = view
        .progress()
        .history
        .iter()
        .find_map(|(r, e)| {
            if r == &diagnosis.progress {
                if let ProgressRecorded::Monitor { input, .. } = e {
                    Some(input)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    let mut ledger = crate::ledger::current(view, view.latest_at())?;
    if completing_retry {
        let item = input
            .item
            .as_ref()
            .and_then(|id| view.results().items().get(id))
            .ok_or_else(|| Denial::new("retry_item", "No retry work item"))?;
        let abandoned = item
            .attempts
            .last()
            .and_then(|id| view.results().attempts().get(id))
            .is_some_and(|a| a.attempt.outcome == AttemptOutcome::Abandoned);
        if !abandoned {
            return Err(Denial::new(
                "retry_state",
                "Only the exact abandoned source may complete retry preparation",
            ));
        }
        for target in &item.targets {
            ledger
                .entries
                .insert(target.clone(), original.ledger.entries[target].clone());
        }
    }
    if ledger
        .entries
        .iter()
        .map(|(id, e)| (id, e.belief, e.status, &e.evidence))
        .collect::<Vec<_>>()
        != original
            .ledger
            .entries
            .iter()
            .map(|(id, e)| (id, e.belief, e.status, &e.evidence))
            .collect::<Vec<_>>()
    {
        return Err(Denial::new(
            "escalation_stale",
            "New criterion evidence requires a fresh diagnosis",
        ));
    }
    let current: Vec<_> = view
        .progress()
        .facts
        .iter()
        .rev()
        .filter(|f| !allowed.contains(&f.source))
        .take(64)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|f| f.source.clone())
        .collect();
    if current != original.basis
        || original.criteria != view.criteria()
        || allowed.iter().any(|r| {
            view.progress()
                .facts
                .iter()
                .find(|f| f.source == *r)
                .is_none_or(|f| f.sequence <= input.at_revision + 1)
        })
    {
        return Err(Denial::new(
            "escalation_stale",
            "New work or changed criteria require reassessment before recovery",
        ));
    }
    Ok(())
}
pub(super) fn step<'a>(
    view: &'a SessionView,
    reference: &Ref,
    expected: &EscalationStep,
) -> Result<(&'a EscalationInput, &'a EscalationPlan)> {
    let (input, plan) = bound_step(view, reference, expected)?;
    fresh(view, input, &[], false)?;
    Ok((input, plan))
}
pub(super) fn validate_action(
    view: &SessionView,
    reference: &Ref,
    outcome: &RecoveryOutcome,
) -> Result<()> {
    let (input, decision) = view
        .progress()
        .step(reference)
        .ok_or_else(|| Denial::new("escalation_missing", "No step for this outcome"))?;
    if let RecoveryOutcome::Unavailable(reason) = outcome {
        ymp_domain::require_text(reason, 4096)?;
        return Ok(());
    }
    bound_step(view, reference, &decision.outcome.step)?;
    fresh(
        view,
        input,
        &match outcome {
            RecoveryOutcome::Run { new, .. } => vec![new.clone()],
            _ => vec![],
        },
        matches!(outcome,RecoveryOutcome::Work(work) if work.retry_of.is_some()),
    )?;
    match outcome {
        RecoveryOutcome::Run { old, new, outcome } => {
            if decision.outcome.step != EscalationStep::FixEnvironment {
                return Err(Denial::new(
                    "recovery_step",
                    "Only FixEnvironment reruns an exact check",
                ));
            }
            let before = view
                .check_runs()
                .values()
                .find(|r| r.reference().is_ok_and(|r| r == *old))
                .ok_or_else(|| Denial::new("recovery_run", "Missing failed run"))?;
            let after = view
                .check_runs()
                .values()
                .find(|r| r.reference().is_ok_and(|r| r == *new))
                .ok_or_else(|| Denial::new("recovery_run", "Missing rerun"))?;
            let diagnosis = view.progress().diagnosis(&input.diagnosis).unwrap();
            if !diagnosis.input.environment.contains(old)
                || !matches!(
                    before.outcome,
                    CheckOutcome::Error(ErrorClass::Environment | ErrorClass::Infrastructure)
                )
                || before.check != after.check
                || before.check_version != after.check_version
                || before.target != after.target
                || before.target_version != after.target_version
                || before.role != after.role
                || before.env == after.env
                || after.outcome != *outcome
                || view
                    .progress()
                    .environments
                    .get(&after.env)
                    .is_none_or(|env| env.limits.timeout_ms > decision.outcome.limit.timeout_ms)
            {
                return Err(Denial::new(
                    "recovery_run",
                    "FixEnvironment must preserve check/snapshot/role and record a corrected environment",
                ));
            }
        }
        RecoveryOutcome::Work(work) => {
            if work.step != *reference
                || work.contribution.session != *view.session()
                || work.contribution.cost.p90 > decision.outcome.limit.max_cost
                || work.contribution.forecast.source
                    != ForecastSource::Model(
                        view.progress()
                            .step(reference)
                            .unwrap()
                            .1
                            .effective
                            .policy
                            .clone(),
                    )
                || !input.basis.iter().all(|r| work.prompt.basis.contains(r))
            {
                return Err(Denial::new(
                    "recovery_work",
                    "Work differs from its diagnosis, bound or exact failing context",
                ));
            }
            work.contribution.validate()?;
            work.prompt.validate()?;
            if self::work(
                view,
                reference,
                input,
                &decision.outcome,
                work.contribution.id.clone(),
                work.profile.clone(),
                work.retry_of.clone(),
                &work.estimate,
            )? != **work
            {
                return Err(Denial::new(
                    "recovery_work",
                    "Recovery differs from its exact cost estimate and failure context",
                ));
            }
            if let Some(attempt) = &work.retry_of {
                if decision.outcome.step != EscalationStep::Retry {
                    return Err(Denial::new(
                        "retry_step",
                        "Only Retry may reopen production",
                    ));
                }
                let source = view
                    .results()
                    .attempts()
                    .get(attempt)
                    .ok_or_else(|| Denial::new("retry_attempt", "No previous attempt"))?;
                let assignment = &view.admission().assignments()[&source.attempt.assignment]
                    .intent
                    .assignment;
                let item = &view.results().items()[&source.attempt.item];
                let commitment = &view.coordination().commitments()[&assignment.commitment];
                let diagnosis = view.progress().diagnosis(&input.diagnosis).unwrap();
                if input.item.as_ref() != Some(&source.attempt.item)
                    || diagnosis
                        .context
                        .as_ref()
                        .and_then(|c| {
                            view.results()
                                .results()
                                .values()
                                .find(|r| r.reference().is_ok_and(|r| r == c.result))
                        })
                        .map(|r| &r.id)
                        != source.attempt.result.as_ref()
                    || source.attempt.outcome != AttemptOutcome::Abandoned
                    || commitment.state != ymp_domain::coordination::CommitmentState::Expired
                    || work.profile != assignment.profile
                    || item.accepted.is_some()
                    || work.contribution.kind != ContributionKind::Produce
                    || work.contribution.subject
                        != Some(ContributionSubject::WorkItem(item.reference()?))
                    || work.contribution.targets != item.targets
                    || work.contribution.needs != item.needs
                    || diagnosis.input.artifact_defect.is_empty()
                    || diagnosis.input.attempts >= diagnosis.input.attempt_limit as usize
                {
                    return Err(Denial::new(
                        "retry_authority",
                        "Retry requires expired responsibility, unchanged producer and remaining attempts",
                    ));
                }
            } else if decision.outcome.step != EscalationStep::AddVerifier
                || work.contribution.kind != ContributionKind::Research
                || work.contribution.subject.is_some()
            {
                return Err(Denial::new(
                    "diagnostic_work",
                    "AddVerifier creates bounded Researcher work",
                ));
            }
        }
        RecoveryOutcome::Stopped => {
            if decision.outcome.step != EscalationStep::StopPreserving
                || view.treasury().is_none_or(|b| b.reporting_mode.is_none())
            {
                return Err(Denial::new(
                    "stop_reporting",
                    "StopPreserving requires actual reporting mode",
                ));
            }
        }
        RecoveryOutcome::Unavailable(_) => {}
    }
    Ok(())
}
impl<J: Journal> Progress<J> {
    #[allow(clippy::too_many_arguments)]
    pub fn fix_environment<C: ContentStore>(
        &self,
        owner: &SessionControl,
        authority: &AcceptanceAuthority<J, C>,
        reference: &Ref,
        old: &Ref,
        run: Id<CheckRun>,
        at: u64,
        runner: &dyn CheckRunner,
    ) -> Result<CheckRun> {
        let session = self.owner.authorize(owner)?;
        authority.require_journal(&self.journal)?;
        let view = self.journal.view(&session, None)?;
        let existing = view.check_runs().get(&run);
        let (input, plan) = bound_step(&view, reference, &EscalationStep::FixEnvironment)?;
        fresh(
            &view,
            input,
            &existing
                .map(|r| r.reference())
                .transpose()?
                .into_iter()
                .collect::<Vec<_>>(),
            false,
        )?;
        let diagnosis = view.progress().diagnosis(&input.diagnosis).unwrap();
        let source = view
            .check_runs()
            .values()
            .find(|r| r.reference().is_ok_and(|r| r == *old))
            .ok_or_else(|| Denial::new("recovery_run", "No exact failing run"))?;
        let env = runner.environment()?;
        if !diagnosis.input.environment.contains(old)
            || Digest::of_value(&env)? == source.env
            || env.limits.timeout_ms > plan.limit.timeout_ms
        {
            return Err(Denial::new(
                "recovery_environment",
                "Use a bounded corrected environment for the recorded failure",
            ));
        }
        let result = if let Some(existing) = existing {
            existing.clone()
        } else {
            authority.run(
                owner,
                RunCheck {
                    expected_revision: view.revision(),
                    at,
                    id: run,
                    check: view.checks()[&source.check].reference(),
                    target: source.target.clone(),
                    role: source.role,
                },
                runner,
            )?
        };
        let current = self.journal.view(&session, None)?;
        self.append(
            owner,
            current.revision(),
            at,
            ProgressRecorded::Action {
                step: reference.clone(),
                outcome: Box::new(RecoveryOutcome::Run {
                    old: old.clone(),
                    new: result.reference()?,
                    outcome: result.outcome.clone(),
                }),
            },
        )?;
        Ok(result)
    }
    pub fn stop_preserving(
        &self,
        owner: &SessionControl,
        reference: &Ref,
        treasury: &Treasury<J>,
        budget: &BudgetControl,
        at: u64,
    ) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        treasury.require_control(&self.journal, budget, &session)?;
        let view = self.journal.view(&session, None)?;
        step(&view, reference, &EscalationStep::StopPreserving)?;
        crate::plans::work_boundary(&view)?;
        if view.treasury().unwrap().reporting_mode.is_none() {
            treasury.start_reporting(budget, view.revision(), at, ReportingMode::Deterministic)?;
        }
        let current = self.journal.view(&session, None)?;
        self.append(
            owner,
            current.revision(),
            at,
            ProgressRecorded::Action {
                step: reference.clone(),
                outcome: Box::new(RecoveryOutcome::Stopped),
            },
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn retry<C: ContentStore>(
        &self,
        owner: &SessionControl,
        reference: &Ref,
        gate: &Gatekeeper<J, C>,
        results: &Results<J, C>,
        attempt: &PreparedAttempt,
        contribution: Id<Contribution>,
        cost_model: &dyn CostModel,
        at: u64,
    ) -> Result<RecoveryWork> {
        let session = self.owner.authorize(owner)?;
        gate.require_journal(&self.journal)?;
        results.require_attempt(&self.journal, &session, attempt)?;
        let view = self.journal.view(&session, None)?;
        let (input, plan) = step(&view, reference, &EscalationStep::Retry)?;
        let diagnosis = view.progress().diagnosis(&input.diagnosis).unwrap();
        let source = view
            .results()
            .attempts()
            .get(attempt.id())
            .ok_or_else(|| Denial::new("retry_attempt", "No owned previous attempt"))?;
        let assignment = &view.admission().assignments()[&source.attempt.assignment]
            .intent
            .assignment;
        let item = &view.results().items()[&source.attempt.item];
        if input.item.as_ref() != Some(&item.id)
            || item.accepted.is_some()
            || !matches!(
                source.attempt.outcome,
                AttemptOutcome::Submitted | AttemptOutcome::Rejected(_)
            )
            || diagnosis.input.artifact_defect.is_empty()
            || diagnosis.input.attempts >= diagnosis.input.attempt_limit as usize
        {
            return Err(Denial::new(
                "retry_limit",
                "No current failing candidate or attempts remain",
            ));
        }
        if at
            <= view.coordination().commitments()[&assignment.commitment]
                .lease
                .expires
        {
            return Err(Denial::new(
                "retry_lease",
                "Retry waits for actual lease expiry; rejection does not discharge responsibility",
            ));
        }
        let profile = assignment.profile.clone();
        let estimate = cost_estimate(
            &view,
            ContributionKind::Produce,
            &profile,
            &contribution,
            cost_model,
        )?;
        work(
            &view,
            reference,
            input,
            plan,
            contribution.clone(),
            profile.clone(),
            Some(attempt.id().clone()),
            &estimate,
        )?;
        results.abandon(
            attempt,
            at,
            "Preserve the rejected candidate and retry with its failing evidence".into(),
        )?;
        Arbiter::new(self.journal.clone()).tick(gate, &session, at)?;
        let view = self.journal.view(&session, None)?;
        let (input, plan) = bound_step(&view, reference, &EscalationStep::Retry)?;
        fresh(&view, input, &[], true)?;
        let estimate = cost_estimate(
            &view,
            ContributionKind::Produce,
            &profile,
            &contribution,
            cost_model,
        )?;
        let work = work(
            &view,
            reference,
            input,
            plan,
            contribution,
            profile,
            Some(attempt.id().clone()),
            &estimate,
        )?;
        self.record_work(owner, at, work)
    }
    pub fn add_verifier(
        &self,
        owner: &SessionControl,
        reference: &Ref,
        profile: ExecutionProfile,
        contribution: Id<Contribution>,
        estimate: VerificationEstimate,
        at: u64,
    ) -> Result<RecoveryWork> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let (input, plan) = step(&view, reference, &EscalationStep::AddVerifier)?;
        let work = work(
            &view,
            reference,
            input,
            plan,
            contribution,
            profile,
            None,
            &estimate,
        )?;
        self.record_work(owner, at, work)
    }
    fn record_work(
        &self,
        owner: &SessionControl,
        at: u64,
        work: RecoveryWork,
    ) -> Result<RecoveryWork> {
        let session = self.owner.authorize(owner)?;
        let current = self.journal.read(&session)?;
        let mut view = current.view_with_schemas(&session, None, self.journal.schemas())?;
        let data = ProgressRecorded::Action {
            step: work.step.clone(),
            outcome: Box::new(RecoveryOutcome::Work(Box::new(work.clone()))),
        };
        let event = Envelope {
            seq: view.revision() + 1,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy: None,
            input: Some(view.digest()?),
            refs: references(&data),
            payload: Event::ProgressRecorded {
                version: 1,
                data: Box::new(data),
            },
        };
        view.apply(&event, self.journal.schemas())?;
        let payload = Event::ContributionProposed {
            version: 1,
            contribution: Box::new(work.contribution.clone()),
        };
        let (policy, input, refs) = crate::arbiter::attribution(&view, &payload)?;
        let proposed = Envelope {
            seq: view.revision() + 1,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy,
            input,
            refs,
            payload,
        };
        let events = [event, proposed];
        validate_append(
            &current,
            &session,
            current.revision,
            &events,
            self.journal.schemas(),
        )?;
        self.journal.append(&session, current.revision, &events)?;
        Ok(work)
    }
}
#[allow(clippy::too_many_arguments)]
fn work(
    view: &SessionView,
    reference: &Ref,
    input: &EscalationInput,
    plan: &EscalationPlan,
    id: Id<Contribution>,
    profile: ExecutionProfile,
    retry: Option<Id<ymp_domain::plan::Attempt>>,
    estimate: &VerificationEstimate,
) -> Result<RecoveryWork> {
    let kind = if retry.is_some() {
        ContributionKind::Produce
    } else {
        ContributionKind::Research
    };
    if view.planning().team.as_ref().is_some_and(|t| {
        !t.members
            .iter()
            .any(|m| m.agent == profile.agent && m.left.is_none())
    }) {
        return Err(Denial::new(
            "recovery_membership",
            "Use an existing team member; team growth needs its owning future handler",
        ));
    }
    let diagnosis = view.progress().diagnosis(&input.diagnosis).unwrap();
    let item = input
        .item
        .as_ref()
        .and_then(|id| view.results().items().get(id));
    if estimate.input.demand.profile != profile
        || estimate.input.demand.kind != kind
        || estimate.input.demand.contribution != id.erased()
        || estimate.input != crate::treasury::estimate_view(view, &estimate.input.demand)?
    {
        return Err(Denial::new(
            "recovery_cost",
            "Actual CostModel input must cover this recovery profile, purpose and work",
        ));
    }
    crate::treasury::verify_decision(view, "CostModel", &estimate.input, &estimate.decision)?;
    let cost = estimate.decision.outcome.clone();
    if cost.p90 > plan.limit.max_cost
        || cost.p90 > view.treasury().unwrap().remaining(kind.purpose())?
    {
        return Err(Denial::new(
            "recovery_budget",
            "Recovery exceeds its explicit bound or available purpose budget",
        ));
    }
    if retry.is_none()
        && diagnosis
            .context
            .as_ref()
            .and_then(|c| {
                view.results()
                    .results()
                    .values()
                    .find(|r| r.reference().is_ok_and(|r| r == c.result))
            })
            .is_some_and(|r| r.producer == profile.agent)
    {
        return Err(Denial::new(
            "diagnostic_independence",
            "Diagnostic Researcher must differ from the producer",
        ));
    }
    let mut basis = input.basis.clone();
    basis.push(reference.clone());
    basis.sort();
    basis.dedup();
    let runs: Vec<_> = view
        .check_runs()
        .values()
        .filter(|r| r.reference().is_ok_and(|r| basis.contains(&r)))
        .cloned()
        .collect();
    let checks: Vec<_> = runs.iter().map(|run| &view.checks()[&run.check]).collect();
    let prompt = Prompt {
        text: String::from_utf8(encode(&(
            "Bounded A9 recovery: inspect the recorded counterexample and failing observations",
            &plan.step,
            &diagnosis.input,
            &view.task().unwrap().goal,
            &item,
            &runs,
            &checks,
        ))?)
        .unwrap(),
        basis: basis.clone(),
    };
    prompt.validate()?;
    let contribution = Contribution {
        id,
        session: view.session().clone(),
        kind,
        targets: item
            .map(|i| i.targets.clone())
            .unwrap_or_else(|| view.criteria().iter().map(|c| c.id.clone()).collect()),
        subject: if retry.is_some() {
            Some(ContributionSubject::WorkItem(
                item.ok_or_else(|| Denial::new("retry_item", "Retry needs a work item"))?
                    .reference()?,
            ))
        } else {
            None
        },
        needs: if retry.is_some() {
            item.unwrap().needs.clone()
        } else {
            BTreeSet::from([ymp_domain::journal::Capability::ReadFiles])
        },
        forecast: Forecast {
            p_success: ymp_domain::Prob::new(0.5)?,
            delta_belief: BTreeMap::new(),
            source: ForecastSource::Model(
                view.progress()
                    .step(reference)
                    .unwrap()
                    .1
                    .effective
                    .policy
                    .clone(),
            ),
        },
        cost,
        difficulty: Difficulty::Simple,
        proposed_by: ContributionAuthor::Runtime,
        basis,
    };
    Ok(RecoveryWork {
        estimate: estimate.clone(),
        step: reference.clone(),
        contribution,
        profile,
        prompt,
        retry_of: retry,
    })
}
pub(crate) fn validate_admission(
    view: &SessionView,
    intent: &crate::gatekeeper::AdmissionIntent,
) -> Result<()> {
    if let Some(work) = view.progress().work(&intent.assignment.contribution) {
        let (input, decision) = view.progress().step(&work.step).unwrap();
        fresh(view, input, &[], work.retry_of.is_some())?;
        if view.method() != Some(&input.method) {
            return Err(Denial::new(
                "recovery_admission",
                "The recovery method changed before admission",
            ));
        }
        if let Some(previous) = &work.retry_of {
            let source = &view.results().attempts()[previous];
            if view.results().items()[&source.attempt.item].attempts.last() != Some(previous) {
                return Err(Denial::new(
                    "retry_stale",
                    "A new attempt superseded this retry context",
                ));
            }
        }
        if intent.assignment.profile != work.profile
            || intent.assignment.allowance.cost > decision.outcome.limit.max_cost
            || intent.assignment.allowance.timeout > decision.outcome.limit.timeout_ms
            || view
                .admission()
                .assignments()
                .values()
                .any(|a| a.intent.assignment.contribution == work.contribution.id)
        {
            return Err(Denial::new(
                "recovery_admission",
                "Recovery admission changes profile, reuses work or exceeds recorded limits",
            ));
        }
    }
    Ok(())
}
pub(crate) fn validate_dispatch(
    view: &SessionView,
    data: &crate::execution::InvocationDispatch,
) -> Result<()> {
    if let Some(work) = view.progress().work(&data.assignment.contribution)
        && data.prompt != work.prompt
    {
        return Err(Denial::new(
            "recovery_context",
            "Recovery must receive its exact recorded failure context",
        ));
    }
    Ok(())
}

fn cost_estimate(
    view: &SessionView,
    kind: ContributionKind,
    profile: &ExecutionProfile,
    id: &Id<Contribution>,
    model: &dyn CostModel,
) -> Result<VerificationEstimate> {
    if view.policies().get("CostModel") != Some(model.selection()) {
        return Err(Denial::new("recovery_cost", "Use the selected CostModel"));
    }
    let provider = view
        .registry()
        .and_then(|p| p.input.facts.agents.iter().find(|a| a.id == profile.agent))
        .ok_or_else(|| Denial::new("profile_excluded", "Unknown recovery agent"))?
        .provider
        .clone();
    let input = crate::treasury::estimate_view(
        view,
        &ResourceDemand {
            contribution: id.erased(),
            kind,
            difficulty: Difficulty::Simple,
            provider,
            profile: profile.clone(),
        },
    )?;
    let proposal = model.estimate(&input)?;
    let decision = Decision {
        input: Digest::of_value(&input)?,
        outcome: proposal.value.clone(),
        proposal,
        effective: model.selection().clone(),
        selection_change: None,
    };
    crate::treasury::verify_decision(view, "CostModel", &input, &decision)?;
    Ok(VerificationEstimate { input, decision })
}

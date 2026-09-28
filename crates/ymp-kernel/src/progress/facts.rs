//! Applicable observations and genuine CostModel estimates for A9.
use super::*;
use ymp_domain::identity::Readiness;
pub(super) fn verification_inputs(
    view: &SessionView,
    context: Option<&ApplicabilityContext>,
) -> Result<Vec<EstimateView>> {
    use ymp_domain::{
        identity::{DiscoverySource, ProviderKind},
        journal::Capability,
        task::EvidenceClass,
    };
    let task = view
        .task()
        .ok_or_else(|| Denial::new("task_missing", "Progress requires a task"))?;
    let Some(pool) = view.registry() else {
        return Ok(vec![]);
    };
    if pool.input.constraints != task.constraints {
        return Err(Denial::new(
            "registry_stale",
            "Refresh eligible profiles after constraints changed",
        ));
    }
    let ledger = crate::ledger::current(view, view.latest_at())?;
    let producer = context
        .and_then(|c| {
            view.results()
                .results()
                .values()
                .find(|r| r.reference().is_ok_and(|r| r == c.result))
        })
        .map(|r| &r.producer);
    let mut inputs = vec![];
    for criterion in view
        .criteria()
        .iter()
        .filter(|c| c.required && ledger.entries[&c.id].status != LedgerStatus::Satisfied)
    {
        let mut needs = BTreeSet::from([Capability::ReadFiles]);
        if criterion.needs_class.contains(&EvidenceClass::Browser) {
            needs.insert(Capability::Browser);
        }
        if criterion.needs_class.contains(&EvidenceClass::Executed) {
            needs.insert(Capability::RunProcess);
        }
        for d in &pool.decisions {
            if d.outcome != Readiness::Ready
                || producer == Some(&d.profile.agent)
                || view.planning().team.as_ref().is_some_and(|t| {
                    !t.members
                        .iter()
                        .any(|m| m.agent == d.profile.agent && m.left.is_none())
                })
                || view.admission().assignments().values().any(|a| {
                    a.intent.assignment.agent == d.profile.agent
                        && crate::gatekeeper::unresolved(view, a)
                })
            {
                continue;
            }
            let agent = pool
                .input
                .facts
                .agents
                .iter()
                .find(|a| a.id == d.profile.agent)
                .unwrap();
            let discovery = pool
                .input
                .facts
                .discoveries
                .iter()
                .find(|p| p.provider.id == agent.provider)
                .unwrap();
            if discovery.provider.kind != ProviderKind::Scripted
                || discovery.source != DiscoverySource::ScriptedFixture
                || !needs.is_subset(&task.constraints.allowed)
                || discovery.provider.capabilities.as_ref().is_none_or(|c| {
                    !needs.is_subset(c)
                        || c.iter().any(|cap| {
                            !matches!(cap, Capability::ReadFiles | Capability::WriteFiles)
                        })
                })
                || !view.workspaces().values().any(|w| {
                    w.provider.policy.implementation == "Direct"
                        && view.workspace_bindings().contains_key(&w.id)
                })
            {
                continue;
            }
            let demand = ResourceDemand {
                contribution: Id::new(format!("verify-{}", criterion.reference()?.version))?,
                kind: ContributionKind::Verify,
                difficulty: Difficulty::Simple,
                provider: agent.provider.clone(),
                profile: d.profile.clone(),
            };
            inputs.push(crate::treasury::estimate_view(view, &demand)?);
        }
    }
    Ok(inputs)
}
pub(super) fn diagnosis_input(
    view: &SessionView,
    context: Option<&ApplicabilityContext>,
    estimates: &[VerificationEstimate],
    at: u64,
) -> Result<DiagnosisInput> {
    let monitor = monitor_input(view, at)?;
    let prior = view.progress().monitor().ok_or_else(|| {
        Denial::new(
            "progress_missing",
            "Assess a work boundary before diagnosing it",
        )
    })?;
    if monitor.boundary != prior.1.boundary {
        return Err(Denial::new(
            "diagnosis_stale",
            "Assess the new work before diagnosis",
        ));
    }
    let expected = verification_inputs(view, context)?;
    if estimates.iter().map(|e| &e.input).collect::<Vec<_>>() != expected.iter().collect::<Vec<_>>()
    {
        return Err(Denial::new(
            "verification_cost",
            "Estimate every actually eligible verification profile",
        ));
    }
    for estimate in estimates {
        crate::treasury::verify_decision(view, "CostModel", &estimate.input, &estimate.decision)?;
        estimate.decision.outcome.validate()?;
    }
    if let Some(context) = context {
        crate::ledger::validate_context(view, context)?;
    }
    let result = context.and_then(|c| {
        view.results()
            .results()
            .values()
            .find(|r| r.reference().is_ok_and(|r| r == c.result))
    });
    let item = result.map(|r| &view.results().items()[&r.item]);
    let mut input=DiagnosisInput{journal:view.digest()?,boundary:monitor.boundary,item:item.map(|i|i.id.clone()),environment:vec![],check_defect:vec![],capability_mismatch:vec![],artifact_defect:vec![],artifact_profiles:0,attempts:0,attempt_limit:view.task().unwrap().constraints.attempt_limit,calibrated_success:None,ambiguity:vec![],plan_defect:vec![],verification_remaining:view.treasury().ok_or_else(||Denial::new("budget_missing","Progress requires a budget"))?.remaining(Purpose::Verification)?,minimum_verification:if view.criteria().iter().filter(|c|c.required).all(|c|prior.1.ledger.entries[&c.id].status==LedgerStatus::Satisfied){Some(ymp_domain::task::Real::new(0.0)?)}else{estimates.iter().map(|e|e.decision.outcome.expected).min_by(|a,b|a.get().total_cmp(&b.get()))},needs:BTreeSet::new(),basis:monitor.basis,limitations:vec!["Calibrated success, interpretation objections and StatusChange notices have no delivered source; they remain unknown".into()]};
    let repaired: BTreeSet<_> = view
        .progress()
        .history
        .iter()
        .filter_map(|(_, e)| {
            if let ProgressRecorded::Action { outcome, .. } = e {
                if let RecoveryOutcome::Run { old, outcome, .. } = outcome.as_ref() {
                    (!matches!(outcome, CheckOutcome::Error(_))).then_some(old.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    let runs: Vec<_> = view
        .check_runs()
        .values()
        .filter(|run| {
            !run.reference().is_ok_and(|r| repaired.contains(&r))
                && view.checks().get(&run.check).is_some_and(|check| {
                    view.contract()
                        .is_some_and(|c| c.checks.contains(&check.id.erased()))
                        && check.version == run.check_version
                        && view.criteria().iter().any(|c| {
                            c.id == check.criterion
                                && c.reference()
                                    .is_ok_and(|r| r.version == check.criterion_version)
                        })
                        && context.is_none_or(|scope| {
                            scope
                                .environments
                                .get(&check.reference())
                                .is_some_and(|env| env.contains(&run.env))
                        })
                        && result.is_none_or(|r| match run.role {
                            CheckRunRole::Baseline => run.target == r.before,
                            CheckRunRole::Candidate => run.target == r.after,
                            CheckRunRole::Control => {
                                view.snapshots().get(&run.target).is_some_and(|s| {
                                    view.snapshots()[&r.after].workspace == s.workspace
                                })
                            }
                        })
                })
        })
        .collect();
    let p: DiagnosisParameters = decode(&encode(
        &view
            .policies()
            .get("FailureDiagnoser")
            .ok_or_else(|| Denial::new("policy_selection", "No diagnoser selected"))?
            .parameters,
    )?)?;
    for run in &runs {
        let check = &view.checks()[&run.check];
        let criterion = view
            .criteria()
            .iter()
            .find(|c| c.id == check.criterion)
            .unwrap();
        if matches!(
            run.outcome,
            CheckOutcome::Error(ErrorClass::Environment | ErrorClass::Infrastructure)
        ) {
            input.environment.push(run.reference()?);
        }
        if (run.role == CheckRunRole::Control && run.outcome == CheckOutcome::Fail)
            || (criterion.kind == ymp_domain::task::CriterionKind::NewBehavior
                && run.role == CheckRunRole::Baseline
                && run.outcome == CheckOutcome::Pass)
        {
            input.check_defect.push(run.reference()?);
        }
        // A failing candidate cannot satisfy verification::discriminating (which requires a pass).
        // Establish the oracle on its baseline/control first, then assess the candidate separately.
        let discriminates = criterion.kind != ymp_domain::task::CriterionKind::NewBehavior
            || runs.iter().any(|base| {
                base.check == run.check
                    && base.check_version == run.check_version
                    && base.env == run.env
                    && base.role == CheckRunRole::Baseline
                    && base.outcome == CheckOutcome::Fail
            });
        let mutations_ok = view
            .evidence()
            .values()
            .filter(|e| e.evidence.runs.contains(&run.id))
            .all(|e| {
                e.evidence
                    .discrimination
                    .mutation_score
                    .is_none_or(|score| score.get() >= p.mutation_threshold.get())
            });
        if run.role == CheckRunRole::Candidate
            && run.outcome == CheckOutcome::Fail
            && discriminates
            && mutations_ok
        {
            input.artifact_defect.push(run.reference()?);
        }
    }
    for invocation in view.execution().invocations().values() {
        if result.is_some_and(|r| {
            !view.results().attempts().values().any(|a| {
                a.attempt.result.as_ref() == Some(&r.id)
                    && a.attempt.assignment == invocation.dispatch.assignment.id
            })
        }) {
            continue;
        }
        if matches!(
            invocation.terminal,
            Some(InvocationTerminal::Failed(
                ErrorClass::Environment | ErrorClass::Infrastructure
            ))
        ) && let Some(reference) = &invocation.end
        {
            input.environment.push(reference.clone());
        }
        for (event, reference) in invocation.observations.values() {
            if let crate::ports::execution::BackendObservation::ToolDenied(capability) =
                &event.observation
            {
                input.capability_mismatch.push(reference.clone());
                input.needs.insert(capability.clone());
            }
        }
        let contribution = &view.coordination().contributions()
            [&invocation.dispatch.assignment.contribution]
            .value;
        if !contribution
            .needs
            .is_subset(&invocation.dispatch.assignment.access)
        {
            input.capability_mismatch.push(invocation.ready.clone());
            input.needs.extend(
                contribution
                    .needs
                    .difference(&invocation.dispatch.assignment.access)
                    .cloned(),
            );
        }
    }
    if let Some(item) = item {
        input.attempts=view.admission().assignments().values().filter(|a|view.coordination().contributions()[&a.intent.assignment.contribution].value.subject.as_ref().is_some_and(|s|matches!(s,ContributionSubject::WorkItem(r) if item.reference().is_ok_and(|actual|actual==*r)))).count();
        let mut profiles = BTreeSet::new();
        for (_, event) in &view.progress().history {
            if let ProgressRecorded::Diagnosis(d) = event
                && d.input.item.as_ref() == Some(&item.id)
                && !d.input.artifact_defect.is_empty()
                && let Some(context) = &d.context
                && let Some(r) = view
                    .results()
                    .results()
                    .values()
                    .find(|r| r.reference().is_ok_and(|r| r == context.result))
            {
                profiles.insert(Digest::of_value(&r.profile)?);
            }
        }
        if !input.artifact_defect.is_empty()
            && let Some(r) = result
        {
            profiles.insert(Digest::of_value(&r.profile)?);
        }
        input.artifact_profiles = profiles.len();
        if !crate::results::plan_current(view, &item.plan)? {
            input.plan_defect.push(item.reference()?);
        }
    }
    // A user refinement can invalidate a committed plan before a candidate exists.
    // Retain that structural basis even when no result context can yet be supplied.
    if item.is_none() {
        for plan in view.results().plans().values() {
            if !crate::results::plan_current(view, &plan.plan.id)? {
                input.plan_defect.push(plan.plan.reference()?);
                input.item.get_or_insert_with(|| plan.item.id.clone());
            }
        }
    }
    for refs in [
        &input.environment,
        &input.check_defect,
        &input.capability_mismatch,
        &input.artifact_defect,
        &input.plan_defect,
    ] {
        input.basis.extend(refs.iter().cloned());
    }
    input.basis.sort();
    input.basis.dedup();
    Ok(input)
}
pub(crate) fn observe(view: &SessionView, event: &Envelope<Event>) -> Result<ProgressState> {
    let mut state = view.progress().clone();
    let mut fact = None;
    match &event.payload {
        Event::AssignmentAdmitted { assignment, .. } => {
            let actual = &view.admission().assignments()[assignment].intent.assignment;
            state
                .profiles
                .insert(actual.agent.clone(), actual.profile.clone());
        }
        Event::ContributionProposed { contribution, .. } => {
            if state.pending_work.as_ref() == Some(contribution.as_ref()) {
                state.pending_work = None;
            }
        }
        Event::InvocationEnded {
            invocation,
            terminal,
            ..
        } => {
            let call = &view.execution().invocations()[invocation];
            let c =
                &view.coordination().contributions()[&call.dispatch.assignment.contribution].value;
            fact = Some(WorkFact {
                source: event.reference()?,
                sequence: event.seq,
                kind: c.kind,
                targets: c.targets.clone(),
                outcome: String::from_utf8(encode(terminal)?).unwrap(),
                rejection: None,
                accepted: false,
            });
        }
        Event::CheckRunRecorded { data, .. } => {
            state
                .environments
                .insert(data.run.env.clone(), data.environment.clone());
            let check = &view.checks()[&data.run.check];
            fact = Some(WorkFact {
                source: data.run.reference()?,
                sequence: event.seq,
                kind: ContributionKind::Verify,
                targets: BTreeSet::from([check.criterion.clone()]),
                outcome: String::from_utf8(encode(&data.run.outcome)?).unwrap(),
                rejection: None,
                accepted: false,
            });
        }
        Event::AcceptanceRecorded { data, .. } => {
            let result = view
                .results()
                .results()
                .values()
                .find(|r| r.reference().is_ok_and(|r| r == data.result))
                .unwrap();
            fact = Some(WorkFact {
                source: data.acceptance.reference()?,
                sequence: event.seq,
                kind: ContributionKind::Review,
                targets: view.results().items()[&result.item].targets.clone(),
                outcome: String::from_utf8(encode(&data.acceptance.decision)?).unwrap(),
                rejection: match &data.acceptance.decision {
                    AcceptanceDecision::Rejected(reason) => Some(reason.clone()),
                    _ => None,
                },
                accepted: data.acceptance.decision == AcceptanceDecision::Accepted,
            });
        }
        _ => {}
    }
    if let Some(fact) = fact {
        if state.facts.len() >= 128 {
            state.facts.remove(0);
        }
        state.facts.push(fact);
    }
    Ok(state)
}

//! Paid final review provenance, separate from candidate Review values.
use super::*;
use ymp_domain::{
    assignment::Invocation,
    identity::{Agent, ExecutionProfile},
    verification::{Evidence, Review, ReviewVerdict},
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalVerdict {
    pub id: Id<Review>,
    pub aggregate: Ref,
    pub verdict: ReviewVerdict,
    pub basis: Vec<Id<Evidence>>,
    pub rationale: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalReview {
    pub verdict: FinalVerdict,
    pub criteria: Vec<Ref>,
    pub reviewer: Id<Agent>,
    pub profile: ExecutionProfile,
    pub invocation: Id<Invocation>,
    pub assignment: Ref,
    pub admission: Ref,
    pub completion: Ref,
    pub receipt: Ref,
    pub prompt: Ref,
}
impl FinalReview {
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.verdict.id.erased(),
            version: Digest::of_value(self)?,
        })
    }
}
pub(crate) fn applicable(
    view: &SessionView,
    review: &FinalReview,
    context: &crate::acceptance::ApplicabilityContext,
) -> Result<bool> {
    if view
        .finalization()
        .reviews
        .iter()
        .any(|record| record == review)
    {
        return crate::acceptance::evidence::final_review_applicable(
            view,
            &review.verdict.id,
            context,
        );
    }
    let Some(aggregate) = view.finalization().aggregate.as_ref() else {
        return Ok(false);
    };
    if aggregate_current(view, aggregate).is_err()
        || review.verdict.aggregate != aggregate.reference()?
        || context.result != review.verdict.aggregate
        || review.criteria.iter().cloned().collect::<BTreeSet<_>>() != context.criteria
        || aggregate.producers.contains(&review.reviewer)
    {
        return Ok(false);
    }
    for id in &review.verdict.basis {
        let Some(evidence) = view.evidence().get(id) else {
            return Ok(false);
        };
        if !crate::acceptance::evidence_applicable(view, evidence, context)? {
            return Ok(false);
        }
    }
    Ok(true)
}

use crate::{
    acceptance::{AcceptanceAuthority, ApplicabilityContext, EvidenceRequest, RunCheck},
    ports::{
        checks::CheckRunner,
        reporting::{ReviewerCandidate, ReviewerInput},
    },
};
use ymp_domain::{
    assignment::{ContributionKind, InvocationTerminal, RoleKind},
    identity::Readiness,
    journal::{Capability, Decision},
    resources::ReservationState,
    verification::*,
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewerRecorded {
    pub input: ReviewerInput,
    pub decision: Decision<Option<Id<Agent>>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalAcceptance {
    pub aggregate: Ref,
    pub context: ApplicabilityContext,
    pub rules: AssessmentRules,
    pub acceptance: Acceptance,
    pub credit: Decision<bool>,
}
pub(crate) fn final_phase(view: &SessionView) -> Result<&FinalAggregate> {
    if view.finalization().control != Some(Continuation::Continue)
        || view.finalization().stopped
        || view.treasury().is_some_and(|t| t.reporting_mode.is_some())
    {
        return Err(Denial::new(
            "finalization_phase",
            "Final verification requires continuation before reporting",
        ));
    }
    let aggregate =
        view.finalization().aggregate.as_ref().ok_or_else(|| {
            Denial::new("final_scope", "Capture a stable integrated result first")
        })?;
    aggregate_current(view, aggregate)?;
    validate_aggregate(view, aggregate)?;
    Ok(aggregate)
}
pub(crate) fn reviewer_input(view: &SessionView) -> Result<ReviewerInput> {
    let aggregate = final_phase(view)?;
    let pool = view
        .registry()
        .ok_or_else(|| Denial::new("registry_missing", "Final review needs ready profiles"))?;
    if pool.input.constraints != view.task().unwrap().constraints {
        return Err(Denial::new(
            "registry_stale",
            "Refresh profiles after owner constraints changed",
        ));
    }
    let mut candidates = vec![];
    for decision in &pool.decisions {
        if decision.outcome != Readiness::Ready
            || aggregate.producers.contains(&decision.profile.agent)
            || view.planning().team.as_ref().is_none_or(|team| {
                !team
                    .members
                    .iter()
                    .any(|m| m.agent == decision.profile.agent && m.left.is_none())
            })
            || view.admission().assignments().values().any(|a| {
                a.intent.assignment.agent == decision.profile.agent
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
            .find(|a| a.id == decision.profile.agent)
            .unwrap();
        let discovery = pool
            .input
            .facts
            .discoveries
            .iter()
            .find(|d| d.provider.id == agent.provider)
            .unwrap();
        if !crate::registry::mediated_backend(discovery, view.policies().get("ExecutionBackend"))
            || discovery.provider.capabilities.as_ref().is_none_or(|caps| {
                !caps.contains(&Capability::ReadFiles)
                    || caps
                        .iter()
                        .any(|cap| !matches!(cap, Capability::ReadFiles | Capability::WriteFiles))
            })
            || !view
                .task()
                .unwrap()
                .constraints
                .allowed
                .contains(&Capability::ReadFiles)
        {
            continue;
        }
        let prior_reviews = view
            .admission()
            .assignments()
            .values()
            .filter(|a| {
                a.intent.assignment.agent == agent.id
                    && matches!(
                        a.intent.assignment.role,
                        RoleKind::Reviewer | RoleKind::FinalReviewer
                    )
            })
            .count();
        candidates.push(ReviewerCandidate {
            profile: decision.profile.clone(),
            prior_reviews,
        });
    }
    Ok(ReviewerInput {
        journal: view.digest()?,
        aggregate: aggregate.reference()?,
        producers: aggregate.producers.clone(),
        candidates,
    })
}
pub(crate) fn validate_reviewer(view: &SessionView, data: &ReviewerRecorded) -> Result<()> {
    if data.input != reviewer_input(view)?
        || data.decision.input != Digest::of_value(&data.input)?
        || view.policies().get("ReviewerPolicy") != Some(&data.decision.effective)
        || data.decision.proposal.policy != data.decision.effective.policy
        || data.decision.proposal.value != data.decision.outcome
        || data.decision.selection_change.is_some()
    {
        return Err(Denial::new(
            "final_reviewer",
            "Reviewer choice differs from its eligible input or policy",
        ));
    }
    data.decision.proposal.validate()?;
    match &data.decision.outcome {
        Some(agent)
            if data
                .input
                .candidates
                .iter()
                .any(|c| &c.profile.agent == agent) =>
        {
            Ok(())
        }
        None if data.input.candidates.is_empty() => Ok(()),
        _ => Err(Denial::new(
            "final_reviewer",
            "Select a ready non-producer; absence must be real",
        )),
    }
}
pub(crate) fn selected(view: &SessionView) -> Result<(&Ref, &ReviewerRecorded)> {
    view.finalization()
        .history
        .iter()
        .rev()
        .find_map(|(r, e)| {
            if let FinalizationRecorded::Reviewer(d) = e {
                Some((r, d.as_ref()))
            } else {
                None
            }
        })
        .ok_or_else(|| Denial::new("final_review_pending", "No final reviewer decision"))
}
pub(crate) fn validate_admission(
    view: &SessionView,
    intent: &crate::gatekeeper::AdmissionIntent,
) -> Result<()> {
    if intent.assignment.role != RoleKind::FinalReviewer {
        return Ok(());
    }
    let aggregate = final_phase(view)?;
    let (reference, choice) = selected(view)?;
    let contribution = &view.coordination().contributions()[&intent.assignment.contribution].value;
    if intent.assignment.workspace != view.finalization().fence.as_ref().unwrap().1.workspace
        || choice.decision.outcome.as_ref() != Some(&intent.assignment.agent)
        || !choice
            .input
            .candidates
            .iter()
            .any(|c| c.profile == intent.assignment.profile)
        || aggregate.producers.contains(&intent.assignment.agent)
        || contribution.kind != ContributionKind::Review
        || contribution.subject.is_some()
        || !contribution.basis.contains(&aggregate.reference()?)
        || !contribution.basis.contains(reference)
        || contribution.targets != view.criteria().iter().map(|c| c.id.clone()).collect()
        || intent.assignment.access != std::collections::BTreeSet::from([Capability::ReadFiles])
    {
        return Err(Denial::new(
            "final_review_admission",
            "Final review requires the exact selected independent identity and read-only aggregate scope",
        ));
    }
    Ok(())
}
pub(crate) fn paid_review(view: &SessionView, invocation: &Id<Invocation>) -> Result<FinalReview> {
    let aggregate = final_phase(view)?;
    let call = view
        .execution()
        .invocations()
        .get(invocation)
        .ok_or_else(|| Denial::new("final_review_source", "No actual final review invocation"))?;
    let a = &call.dispatch.assignment;
    let record = &view.admission().assignments()[&a.id];
    let mut original = record.intent.assignment.clone();
    original.state = a.state;
    let snapshot = view
        .snapshots()
        .values()
        .find(|s| s.reference().is_ok_and(|r| r == aggregate.snapshot))
        .unwrap();
    let locks = view.path_locks().get(&a.id.erased()).ok_or_else(|| {
        Denial::new(
            "final_review_access",
            "No recorded readonly access to the final target",
        )
    })?;
    if a.workspace != snapshot.workspace
        || locks.acquired.workspace != snapshot.workspace
        || snapshot.tree.files.keys().any(|path| {
            !locks
                .acquired
                .requested
                .iter()
                .any(|lock| lock.mode == LockMode::Read && lock.path.contains(path))
        })
    {
        return Err(Denial::new(
            "final_review_access",
            "Final review must cover the integrated target with readonly access",
        ));
    }
    let account = &view.treasury().unwrap().accounts[&record.reservation];
    let context = view
        .finalization()
        .history
        .iter()
        .find_map(|(r, e)| {
            if let FinalizationRecorded::Context(c) = e {
                (c.input.assignment == *a).then_some((r, c.as_ref()))
            } else {
                None
            }
        })
        .ok_or_else(|| Denial::new("final_review_context", "No attributed final review prompt"))?;
    if a.role != RoleKind::FinalReviewer
        || original != *a
        || aggregate.producers.contains(&a.agent)
        || call.dispatch.prompt != context.1.decision.outcome
        || call.terminal != Some(InvocationTerminal::Completed)
        || !call.confirmed_terminal
        || !crate::execution::closed(view, &a.id)
        || crate::execution::limit_reason(view, call, call.ended_at.unwrap_or(0))?.is_some()
        || account.reservation.state != ReservationState::Settled
        || call.receipt.is_none()
        || account.settlement.as_ref().map(|s| &s.receipt) != call.receipt.as_ref()
    {
        return Err(Denial::new(
            "final_review_source",
            "Only exact paid completed independent review output can be committed",
        ));
    }
    let verdict: FinalVerdict = ymp_domain::journal::decode(call.output.as_bytes())?;
    ymp_domain::require_text(&verdict.rationale, 4096)?;
    if verdict.aggregate != aggregate.reference()?
        || view.reviews().contains_key(&verdict.id)
        || view
            .finalization()
            .reviews
            .iter()
            .any(|r| r.verdict.id == verdict.id)
    {
        return Err(Denial::new(
            "final_review_scope",
            "Final verdict names a stale aggregate or reused review identity",
        ));
    }
    let result = FinalReview {
        criteria: aggregate.criteria.iter().cloned().collect(),
        verdict,
        reviewer: a.agent.clone(),
        profile: a.profile.clone(),
        invocation: invocation.clone(),
        assignment: a.reference()?,
        admission: call.dispatch.admission.clone(),
        completion: call.end.clone().unwrap(),
        receipt: account.last.clone(),
        prompt: context.0.clone(),
    };
    if !applicable(view, &result, &super::context(aggregate)?)? {
        return Err(Denial::new(
            "final_review_basis",
            "Final review basis is not applicable",
        ));
    }
    Ok(result)
}
pub(crate) fn acceptance_value(
    view: &SessionView,
    id: Id<Acceptance>,
    rules: &AssessmentRules,
    at: u64,
) -> Result<Acceptance> {
    let aggregate = final_phase(view)?;
    if view
        .finalization()
        .acceptance
        .as_ref()
        .is_some_and(|a| a.acceptance.decision == AcceptanceDecision::Accepted)
    {
        return Err(Denial::new(
            "final_accepted",
            "Final acceptance is immutable",
        ));
    }
    let context = super::context(aggregate)?;
    let evidence = crate::acceptance::applicable_evidence(view, &context)?;
    for scope in &aggregate.checks {
        for run in view.check_runs().values().filter(|r| {
            r.check.erased() == scope.check.id
                && r.check_version == scope.check.version
                && r.target.erased() == aggregate.snapshot.id
                && r.target_version == aggregate.snapshot.version
                && r.role == CheckRunRole::Candidate
                && Digest::of_value(&scope.environment).is_ok_and(|env| env == r.env)
        }) {
            if !matches!(run.outcome, CheckOutcome::Error(_))
                && !evidence.iter().any(|e| e.evidence.runs.contains(&run.id))
            {
                return Err(Denial::new(
                    "final_evidence_pending",
                    "Every conclusive final rerun needs its canonical evidence before acceptance",
                ));
            }
        }
    }
    for scope in &aggregate.checks {
        if !view.check_runs().values().any(|r| {
            r.check.erased() == scope.check.id
                && r.check_version == scope.check.version
                && r.target.erased() == aggregate.snapshot.id
                && r.target_version == aggregate.snapshot.version
                && r.role == CheckRunRole::Candidate
                && Digest::of_value(&scope.environment).is_ok_and(|env| env == r.env)
        }) {
            return Err(Denial::new(
                "final_checks_pending",
                "Rerun every active check against the actual integrated snapshot",
            ));
        }
    }
    let reviews = view
        .finalization()
        .reviews
        .iter()
        .filter_map(|r| match applicable(view, r, &context) {
            Ok(true) => Some(Ok(r)),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<Vec<_>>>()?;
    let contradiction = evidence
        .iter()
        .any(|e| crate::ports::planning::hard_contradiction(&e.evidence));
    let approved = reviews
        .iter()
        .any(|r| r.verdict.verdict == ReviewVerdict::Approve);
    let decision = if contradiction {
        AcceptanceDecision::Rejected("failing applicable check".into())
    } else if !approved {
        AcceptanceDecision::Rejected("no independent final approval".into())
    } else {
        AcceptanceDecision::Accepted
    };
    let mut grades = std::collections::BTreeMap::new();
    let mut required = vec![];
    for criterion in view.criteria() {
        let applicable: Vec<_> = evidence
            .iter()
            .copied()
            .filter(|e| e.evidence.criterion == criterion.id)
            .collect();
        let grade = crate::acceptance::decisions::grade(criterion, &applicable, rules);
        grades.insert(criterion.id.clone(), grade);
        if criterion.required {
            required.push(grade);
        }
    }
    let grade = required
        .into_iter()
        .min_by_key(|g| g.rank())
        .unwrap_or(ConfirmationGrade::Unconfirmed);
    let mut basis = vec![
        aggregate.reference()?,
        aggregate.snapshot.clone(),
        aggregate.contract.clone(),
    ];
    basis.extend(aggregate.criteria.iter().cloned());
    for e in evidence {
        basis.push(e.reference()?);
    }
    for r in reviews {
        basis.push(r.reference()?);
    }
    basis.sort();
    basis.dedup();
    Ok(Acceptance {
        id,
        subject: AcceptanceSubject::FinalAggregate(view.session().clone()),
        decision,
        grades,
        grade,
        basis,
        at,
    })
}
impl<J: Journal, C: ContentStore> Finalization<J, C> {
    pub fn reviewer_input(&self, session: &Id) -> Result<ReviewerInput> {
        reviewer_input(&self.journal.view(session, None)?)
    }
    pub fn record_reviewer(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        input: ReviewerInput,
        proposal: ymp_domain::Proposal<Option<Id<Agent>>>,
    ) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let decision = Decision {
            input: Digest::of_value(&input)?,
            outcome: proposal.value.clone(),
            proposal,
            effective: view
                .policies()
                .get("ReviewerPolicy")
                .ok_or_else(|| Denial::new("policy_selection", "No ReviewerPolicy selected"))?
                .clone(),
            selection_change: None,
        };
        self.append(
            owner,
            expected,
            at,
            FinalizationRecorded::Reviewer(Box::new(ReviewerRecorded { input, decision })),
        )
    }
    pub fn record_review(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        invocation: &Id<Invocation>,
    ) -> Result<FinalReview> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let review = paid_review(&view, invocation)?;
        self.append(
            owner,
            expected,
            at,
            FinalizationRecorded::Reviewed(Box::new(review.clone())),
        )?;
        Ok(review)
    }
    pub fn final_acceptance_value(
        &self,
        session: &Id,
        id: Id<Acceptance>,
        rules: &AssessmentRules,
        at: u64,
    ) -> Result<Acceptance> {
        acceptance_value(&self.journal.view(session, None)?, id, rules, at)
    }
    pub fn accept(
        &self,
        owner: &SessionControl,
        request: crate::acceptance::AcceptanceRequest,
    ) -> Result<FinalAcceptance> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let aggregate = final_phase(&view)?.reference()?;
        let acceptance = acceptance_value(&view, request.id, &request.rules, request.at)?;
        let data = FinalAcceptance {
            aggregate,
            context: request.context,
            rules: request.rules,
            acceptance,
            credit: Decision {
                input: request.credit.input,
                outcome: request.credit.proposal.value,
                proposal: request.credit.proposal,
                effective: view
                    .policies()
                    .get("CreditPolicy")
                    .ok_or_else(|| Denial::new("policy_selection", "No CreditPolicy selected"))?
                    .clone(),
                selection_change: None,
            },
        };
        self.append(
            owner,
            request.expected_revision,
            request.at,
            FinalizationRecorded::Accepted(Box::new(data.clone())),
        )?;
        Ok(data)
    }
    pub fn recheck(
        &self,
        owner: &SessionControl,
        at: u64,
        runner: &dyn CheckRunner,
        baseline: bool,
    ) -> Result<Vec<CheckRun>> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let aggregate = final_phase(&view)?.clone();
        let environment = runner.environment()?;
        let authority = AcceptanceAuthority::new(
            self.journal.clone(),
            self.content.clone(),
            self.owner.shared(),
        );
        let mut runs = vec![];
        for scope in &aggregate.checks {
            if scope.environment != environment {
                continue;
            }
            let mut roles = vec![(CheckRunRole::Candidate, aggregate.snapshot.clone())];
            if baseline && let Some(before) = &aggregate.baseline {
                roles.insert(0, (CheckRunRole::Baseline, before.clone()));
            }
            let mut evidence_runs = vec![];
            for (role, target) in roles {
                let id = Id::new(format!(
                    "final-run-{}",
                    Digest::of_value(&(aggregate.reference()?, &scope.check, &target, role))?
                ))?;
                let view = self.journal.view(&session, None)?;
                let run = if let Some(run) = view.check_runs().get(&id) {
                    run.clone()
                } else {
                    authority.run(
                        owner,
                        RunCheck {
                            expected_revision: view.revision(),
                            at,
                            id,
                            check: scope.check.clone(),
                            target: Id::new(target.id.as_str())?,
                            role,
                        },
                        runner,
                    )?
                };
                evidence_runs.push(run.id.clone());
                runs.push(run);
            }
            let candidate = runs.last().unwrap();
            if !matches!(candidate.outcome, CheckOutcome::Error(_)) {
                let view = self.journal.view(&session, None)?;
                let id = Id::new(format!(
                    "final-evidence-{}",
                    Digest::of_value(&(aggregate.reference()?, &scope.check, &evidence_runs))?
                ))?;
                if !view.evidence().contains_key(&id) {
                    let check = view
                        .checks()
                        .values()
                        .find(|c| c.reference() == scope.check)
                        .unwrap();
                    authority.evidence(
                        owner,
                        EvidenceRequest {
                            expected_revision: view.revision(),
                            at,
                            id,
                            criterion: Ref {
                                id: check.criterion.erased(),
                                version: check.criterion_version.clone(),
                            },
                            result: aggregate.reference()?,
                            runs: evidence_runs,
                            reviews: vec![],
                        },
                    )?;
                }
            }
        }
        Ok(runs)
    }
}

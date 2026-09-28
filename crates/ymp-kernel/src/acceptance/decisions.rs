//! A7 acceptance and A8 ledger commits use existing owner and policy boundaries.
use super::*;
use crate::{
    ledger::{self, LedgerRecorded, LedgerRequest},
    ports::planning::BeliefResponse,
};
use std::collections::BTreeMap;
use ymp_domain::{
    journal::{Decision, SelectionChange},
    plan::{Attempt, AttemptOutcome},
    result::ResultVersion,
    task::EvidenceClass,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceRecorded {
    pub acceptance: Acceptance,
    pub result: Ref,
    pub attempt: Id<Attempt>,
    pub contract: Ref,
    pub context: ApplicabilityContext,
    pub rules: AssessmentRules,
    pub credit: Decision<bool>,
}
pub struct AcceptanceRequest {
    pub expected_revision: u64,
    pub at: u64,
    pub id: Id<Acceptance>,
    pub context: ApplicabilityContext,
    pub rules: AssessmentRules,
    pub credit: CreditResponse,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreditResponse {
    pub input: Digest,
    pub proposal: Proposal<bool>,
}
fn result<'a>(view: &'a SessionView, reference: &Ref) -> Result<&'a ResultVersion> {
    view.results()
        .results()
        .values()
        .find(|r| r.reference().is_ok_and(|r| r == *reference))
        .ok_or_else(|| Denial::new("acceptance_result", "No exact submitted result"))
}
fn attempt<'a>(
    view: &'a SessionView,
    result: &ResultVersion,
) -> Result<&'a crate::results::AttemptRecord> {
    let attempt = view
        .results()
        .attempts()
        .values()
        .find(|a| a.attempt.result.as_ref() == Some(&result.id))
        .ok_or_else(|| Denial::new("acceptance_attempt", "Result has no production attempt"))?;
    let item = &view.results().items()[&result.item];
    let assignment = &view.admission().assignments()[&attempt.attempt.assignment]
        .intent
        .assignment;
    if !matches!(
        attempt.attempt.outcome,
        AttemptOutcome::Submitted | AttemptOutcome::Rejected(_)
    ) || item.attempts.last() != Some(&attempt.attempt.id)
        || item.accepted.is_some()
        || attempt.attempt.item != result.item
        || assignment.agent != result.producer
        || assignment.profile != result.profile
    {
        return Err(Denial::new(
            "acceptance_attempt",
            "Only the current unaccepted submitted candidate can be decided",
        ));
    }
    Ok(attempt)
}
pub(crate) fn grade(
    criterion: &ymp_domain::task::Criterion,
    evidence: &[&EvidenceRecorded],
    rules: &AssessmentRules,
) -> ConfirmationGrade {
    if evidence
        .iter()
        .any(|e| crate::ports::planning::hard_contradiction(&e.evidence))
    {
        return ConfirmationGrade::Refuted;
    }
    let supporting: Vec<_> = evidence
        .iter()
        .filter(|e| e.evidence.polarity == Polarity::Supports)
        .collect();
    if supporting.iter().any(|e| {
        e.evidence.independence == Independence::Trusted
            && matches!(
                e.evidence.class,
                EvidenceClass::Executed | EvidenceClass::Browser
            )
            && e.evidence.discrimination.candidate_passes
    }) {
        return ConfirmationGrade::Confirmed(ConfirmationBasis::TrustedCheck);
    }
    if supporting
        .iter()
        .any(|e| e.evidence.class == EvidenceClass::ExternalData)
    {
        return ConfirmationGrade::Confirmed(ConfirmationBasis::ExternalData);
    }
    if supporting.iter().any(|e| {
        e.evidence.independence == Independence::IndependentHidden
            && matches!(
                e.evidence.class,
                EvidenceClass::Executed | EvidenceClass::Browser
            )
            && discriminating(&e.evidence, criterion, rules)
    }) {
        return ConfirmationGrade::Discriminated;
    }
    ConfirmationGrade::Unconfirmed
}
pub fn acceptance_value(
    view: &SessionView,
    id: Id<Acceptance>,
    context: &ApplicabilityContext,
    rules: &AssessmentRules,
    at: u64,
) -> Result<Acceptance> {
    ledger::validate_context(view, context)?;
    let result = result(view, &context.result)?;
    let snapshot = &view.snapshots()[&result.after];
    let evidence = applicable_evidence(view, context)?;
    for run in view.check_runs().values() {
        let Some(check) = view.checks().get(&run.check) else {
            continue;
        };
        let applies = run.role == CheckRunRole::Candidate
            && run.target == result.after
            && run.target_version == snapshot.reference()?.version
            && check.version == run.check_version
            && view.contract().unwrap().checks.contains(&check.id.erased())
            && context
                .criteria
                .iter()
                .any(|c| c.id == check.criterion.erased() && c.version == check.criterion_version)
            && context
                .environments
                .get(&check.reference())
                .is_some_and(|expected| expected.contains(&run.env));
        if applies
            && !matches!(run.outcome, CheckOutcome::Error(_))
            && !evidence.iter().any(|e| e.evidence.runs.contains(&run.id))
        {
            return Err(Denial::new("candidate_evidence_pending","Publish canonical evidence for the existing conclusive candidate check before acceptance").with_ref(run.reference()?));
        }
    }
    acceptance_value_v1(view, id, context, rules, at)
}
fn acceptance_value_v1(
    view: &SessionView,
    id: Id<Acceptance>,
    context: &ApplicabilityContext,
    rules: &AssessmentRules,
    at: u64,
) -> Result<Acceptance> {
    ledger::validate_context(view, context)?;
    let result = result(view, &context.result)?;
    attempt(view, result)?;
    let evidence = applicable_evidence(view, context)?;
    let reviews = view
        .reviews()
        .values()
        .filter_map(|r| match review_applicable(view, r, context) {
            Ok(true) => Some(Ok(r)),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<Vec<_>>>()?;
    let contradiction = evidence
        .iter()
        .any(|e| crate::ports::planning::hard_contradiction(&e.evidence));
    let approval = reviews.iter().any(|r| {
        r.review.reviewer != result.producer && r.review.verdict == ReviewVerdict::Approve
    });
    let decision = if contradiction {
        AcceptanceDecision::Rejected("failing applicable check".into())
    } else if !approval {
        AcceptanceDecision::Rejected("no independent approval".into())
    } else {
        AcceptanceDecision::Accepted
    };
    let mut grades = BTreeMap::new();
    let mut required = vec![];
    for reference in &context.criteria {
        let criterion = view
            .criteria()
            .iter()
            .find(|c| c.reference().is_ok_and(|r| r == *reference))
            .unwrap();
        let evidence: Vec<_> = evidence
            .iter()
            .copied()
            .filter(|e| e.evidence.criterion == criterion.id)
            .collect();
        let grade = grade(criterion, &evidence, rules);
        grades.insert(criterion.id.clone(), grade);
        if criterion.required {
            required.push(grade);
        }
    }
    let grade = required
        .into_iter()
        .min_by_key(|grade| grade.rank())
        .unwrap_or(ConfirmationGrade::Unconfirmed);
    let mut basis = vec![context.result.clone()];
    basis.extend(context.criteria.iter().cloned());
    for evidence in evidence {
        basis.push(evidence.reference()?);
    }
    for review in reviews {
        basis.push(review.reference()?);
    }
    basis.sort();
    basis.dedup();
    Ok(Acceptance {
        id,
        subject: AcceptanceSubject::ResultVersion(result.id.clone()),
        decision,
        grades,
        grade,
        basis,
        at,
    })
}
pub fn credit_input(acceptance: &Acceptance) -> Result<Digest> {
    Digest::of_value(&(acceptance.reference()?, acceptance.grade))
}
pub(crate) fn validate_credit(acceptance: &Acceptance, decision: &Decision<bool>) -> Result<()> {
    decision.proposal.validate()?;
    let allowed = match decision.effective.policy.implementation.as_str() {
        "ConfirmedOnly" => matches!(acceptance.grade, ConfirmationGrade::Confirmed(_)),
        "IncludeDiscriminated" => {
            acceptance.grade.rank() >= ConfirmationGrade::Discriminated.rank()
        }
        _ => {
            return Err(Denial::new(
                "credit_policy",
                "Credit eligibility requires an owner-approved policy variant",
            ));
        }
    };
    if decision.proposal.policy != decision.effective.policy
        || decision.proposal.value != allowed
        || decision.outcome != allowed
        || decision.input != credit_input(acceptance)?
    {
        return Err(Denial::new(
            "credit_policy",
            "Eligibility differs from the recorded grade and selected policy",
        ));
    }
    Ok(())
}
pub(crate) fn validate_acceptance(
    view: &SessionView,
    event: &Envelope<Event>,
    data: &AcceptanceRecorded,
    schemas: &crate::journal::ParameterSchemas,
) -> Result<()> {
    let assess = if event.payload.version() == 1 {
        acceptance_value_v1
    } else {
        acceptance_value
    };
    let expected = assess(
        view,
        data.acceptance.id.clone(),
        &data.context,
        &data.rules,
        event.at,
    )?;
    let result = result(view, &data.result)?;
    let attempt = attempt(view, result)?;
    schemas.validate(&data.credit.effective)?;
    ledger::selection(
        view,
        &data.credit.effective,
        data.credit.selection_change.as_ref(),
        "CreditPolicy",
    )?;
    validate_credit(&expected, &data.credit)?;
    let mut refs = expected.basis.clone();
    refs.push(data.contract.clone());
    refs.extend(data.credit.proposal.basis.iter().cloned());
    refs.sort();
    refs.dedup();
    if data.acceptance != expected
        || data.result != data.context.result
        || data.attempt != attempt.attempt.id
        || data.contract != view.contract().unwrap().reference()
        || view.acceptances().contains_key(&data.acceptance.id)
        || view.acceptances().len() >= 4096
        || event.at < view.latest_at()
        || event.policy != Some(data.credit.effective.policy.clone())
        || event.input != Some(view.digest()?)
        || event.refs != refs
    {
        return Err(Denial::new(
            "acceptance_attribution",
            "Acceptance differs from its actual candidate, independent review and applicable evidence",
        ));
    }
    Ok(())
}
impl<J: Journal, C: ContentStore> AcceptanceAuthority<J, C> {
    pub fn ledger(&self, session: &Id) -> Result<CriteriaLedger> {
        let view = self.journal.view(session, None)?;
        ledger::current(&view, view.latest_at())
    }
    pub fn ledger_inputs(
        &self,
        session: &Id,
        context: &ApplicabilityContext,
        rules: &AssessmentRules,
        at: u64,
    ) -> Result<BTreeMap<Id<ymp_domain::task::Criterion>, crate::ports::planning::BeliefView>> {
        ledger::inputs(&self.journal.view(session, None)?, context, rules, at)
    }
    pub fn record_ledger(
        &self,
        owner: &SessionControl,
        request: LedgerRequest,
        replacement: Option<PolicySelection>,
    ) -> Result<CriteriaLedger> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        boundary(&view, request.expected_revision, request.at)?;
        let current = view
            .policies()
            .get("BeliefModel")
            .ok_or_else(|| Denial::new("policy_selection", "No BeliefModel selected"))?;
        let (effective, selection_change) = match replacement {
            None => (current.clone(), None),
            Some(next) => {
                self.journal.schemas().validate(&next)?;
                (
                    next,
                    Some(SelectionChange {
                        previous: current.policy.clone(),
                        boundary: view.revision(),
                    }),
                )
            }
        };
        let decisions = request
            .responses
            .into_iter()
            .map(|(id, BeliefResponse { input, proposal })| {
                (
                    id,
                    Decision {
                        outcome: proposal.value.clone(),
                        proposal,
                        input,
                        effective: effective.clone(),
                        selection_change: selection_change.clone(),
                    },
                )
            })
            .collect();
        let data = LedgerRecorded {
            context: request.context,
            rules: request.rules,
            decisions,
        };
        let event = Envelope {
            seq: next(view.revision())?,
            session: session.clone(),
            at: request.at,
            actor: Actor::Runtime,
            policy: Some(effective.policy.clone()),
            input: Some(view.digest()?),
            refs: ledger::refs(&view, &data)?,
            payload: Event::LedgerUpdated {
                version: 1,
                data: Box::new(data),
            },
        };
        self.append(&session, view.revision(), event)?;
        self.ledger(&session)
    }
    pub fn acceptance_value(
        &self,
        session: &Id,
        id: Id<Acceptance>,
        context: &ApplicabilityContext,
        rules: &AssessmentRules,
        at: u64,
    ) -> Result<Acceptance> {
        acceptance_value(&self.journal.view(session, None)?, id, context, rules, at)
    }
    pub fn accept(
        &self,
        owner: &SessionControl,
        request: AcceptanceRequest,
        replacement: Option<PolicySelection>,
    ) -> Result<AcceptanceRecorded> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        boundary(&view, request.expected_revision, request.at)?;
        let acceptance = acceptance_value(
            &view,
            request.id,
            &request.context,
            &request.rules,
            request.at,
        )?;
        let current = view
            .policies()
            .get("CreditPolicy")
            .ok_or_else(|| Denial::new("policy_selection", "No CreditPolicy selected"))?;
        let (effective, selection_change) = match replacement {
            None => (current.clone(), None),
            Some(next) => {
                self.journal.schemas().validate(&next)?;
                (
                    next,
                    Some(SelectionChange {
                        previous: current.policy.clone(),
                        boundary: view.revision(),
                    }),
                )
            }
        };
        let data = AcceptanceRecorded {
            attempt: attempt(&view, result(&view, &request.context.result)?)?
                .attempt
                .id
                .clone(),
            acceptance,
            result: request.context.result.clone(),
            contract: view.contract().unwrap().reference(),
            context: request.context,
            rules: request.rules,
            credit: Decision {
                outcome: request.credit.proposal.value,
                proposal: request.credit.proposal,
                input: request.credit.input,
                effective,
                selection_change,
            },
        };
        let mut refs = data.acceptance.basis.clone();
        refs.push(data.contract.clone());
        refs.extend(data.credit.proposal.basis.iter().cloned());
        refs.sort();
        refs.dedup();
        let event = Envelope {
            seq: next(view.revision())?,
            session: session.clone(),
            at: request.at,
            actor: Actor::Runtime,
            policy: Some(data.credit.effective.policy.clone()),
            input: Some(view.digest()?),
            refs,
            payload: Event::AcceptanceRecorded {
                version: 2,
                data: Box::new(data.clone()),
            },
        };
        self.append(&session, view.revision(), event)?;
        Ok(data)
    }
}

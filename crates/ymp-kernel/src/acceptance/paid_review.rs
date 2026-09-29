//! Consume the exact completed paid ordinary review without reviving its Grant.
use super::*;
use std::collections::BTreeSet;
use ymp_domain::{
    assignment::{ContributionSubject, Invocation, InvocationTerminal},
    resources::ReservationState,
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateVerdict {
    pub id: Id<Review>,
    pub result: Ref,
    pub criteria: Vec<Ref>,
    pub verdict: ReviewVerdict,
    pub findings: Vec<Finding>,
    pub basis: Vec<Id<Evidence>>,
    pub rationale: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaidReviewRecorded {
    pub invocation: Id<Invocation>,
    pub assignment: Ref,
    pub admission: Ref,
    pub completion: Ref,
    pub receipt: Ref,
    pub prompt: Ref,
    pub verdict: CandidateVerdict,
    pub review: ReviewRecorded,
}
pub(crate) fn derive(
    view: &SessionView,
    invocation: &Id<Invocation>,
    at: u64,
) -> Result<PaidReviewRecorded> {
    let call = view
        .execution()
        .invocations()
        .get(invocation)
        .ok_or_else(|| Denial::new("paid_review_source", "No retained reviewer invocation"))?;
    let assignment = &call.dispatch.assignment;
    let admission = view
        .admission()
        .assignments()
        .get(&assignment.id)
        .ok_or_else(|| Denial::new("paid_review_source", "No exact admission"))?;
    let mut original = admission.intent.assignment.clone();
    original.state = assignment.state;
    let account = &view
        .treasury()
        .ok_or_else(|| Denial::new("budget_missing", "Paid review has no budget"))?
        .accounts[&admission.reservation];
    let context = view
        .finalization()
        .history
        .iter()
        .find_map(|(reference, record)| {
            if let crate::finalization::FinalizationRecorded::Context(context) = record {
                (context.input.assignment == *assignment).then_some((reference, context.as_ref()))
            } else {
                None
            }
        })
        .ok_or_else(|| {
            Denial::new(
                "paid_review_context",
                "Paid review requires its recorded attributed prompt",
            )
        })?;
    if original != *assignment
        || assignment.role != RoleKind::Reviewer
        || call.dispatch.prompt != context.1.decision.outcome
        || call.terminal != Some(InvocationTerminal::Completed)
        || !call.confirmed_terminal
        || !crate::execution::closed(view, &assignment.id)
        || crate::execution::limit_reason(view, call, call.ended_at.unwrap_or(0))?.is_some()
        || account.reservation.state != ReservationState::Settled
        || call.receipt.is_none()
        || account.settlement.as_ref().map(|s| &s.receipt) != call.receipt.as_ref()
    {
        return Err(Denial::new(
            "paid_review_source",
            "Review requires exact completed, settled, closed original authority",
        ));
    }
    let verdict: CandidateVerdict = ymp_domain::journal::decode(call.output.as_bytes())?;
    ymp_domain::require_text(&verdict.rationale, 4096)?;
    let criteria = context
        .1
        .input
        .criteria
        .iter()
        .map(|c| c.reference())
        .collect::<Result<BTreeSet<_>>>()?;
    if criteria != verdict.criteria.iter().cloned().collect()
        || criteria.len() != verdict.criteria.len()
        || context
            .1
            .input
            .purpose
            .get("operation")
            .and_then(serde_json::Value::as_str)
            != Some("candidate_review")
        || context.1.input.purpose["result"]
            != serde_json::to_value(&verdict.result)
                .map_err(|_| Denial::new("review_encoding", "Cannot encode review scope"))?
        || !call.dispatch.prompt.basis.contains(&verdict.result)
        || verdict.basis.iter().any(|id| {
            view.evidence().get(id).is_none_or(|e| {
                e.reference().is_err()
                    || !context.1.input.evidence.contains(&e.reference().unwrap())
            })
        })
    {
        return Err(Denial::new(
            "paid_review_scope",
            "Verdict differs from the exact result, criteria or evidence available to the reviewer",
        ));
    }
    let review = evidence::derive_review(
        view,
        assignment,
        &ReviewRequest {
            expected_revision: view.revision(),
            at,
            id: verdict.id.clone(),
            result: verdict.result.clone(),
            criteria: verdict.criteria.clone(),
            verdict: verdict.verdict,
            findings: verdict.findings.clone(),
            basis: verdict.basis.clone(),
        },
    )?;
    Ok(PaidReviewRecorded {
        invocation: invocation.clone(),
        assignment: assignment.reference()?,
        admission: call.dispatch.admission.clone(),
        completion: call.end.clone().unwrap(),
        receipt: account.last.clone(),
        prompt: context.0.clone(),
        verdict,
        review,
    })
}
pub(crate) fn refs(view: &SessionView, data: &PaidReviewRecorded) -> Result<Vec<Ref>> {
    let mut refs = evidence::review_refs(view, &data.review)?;
    refs.extend([
        data.admission.clone(),
        data.completion.clone(),
        data.receipt.clone(),
        data.prompt.clone(),
    ]);
    refs.sort();
    refs.dedup();
    Ok(refs)
}
pub(crate) fn validate(
    view: &SessionView,
    event: &Envelope<Event>,
    data: &PaidReviewRecorded,
) -> Result<()> {
    if view.reviews().contains_key(&data.review.review.id)
        || view
            .finalization()
            .reviews
            .iter()
            .any(|r| r.verdict.id == data.review.review.id)
        || view.reviews().len() >= 4096
        || event.at < view.latest_at()
        || event.policy.is_some()
        || event.input != Some(view.digest()?)
        || event.refs != refs(view, data)?
        || *data != derive(view, &data.invocation, event.at)?
    {
        return Err(Denial::new(
            "paid_review_attribution",
            "Paid review differs from its retained invocation and exact scope",
        ));
    }
    Ok(())
}
pub(crate) fn purpose(
    view: &SessionView,
    assignment: &ymp_domain::assignment::Assignment,
) -> Result<(Ref, Vec<Ref>, Vec<Ref>, serde_json::Value)> {
    let contribution = &view.coordination().contributions()[&assignment.contribution].value;
    let Some(ContributionSubject::ResultVersion(reference)) = &contribution.subject else {
        return Err(Denial::new(
            "review_subject",
            "Ordinary review requires an exact result",
        ));
    };
    let result = view
        .results()
        .results()
        .values()
        .find(|r| r.reference().as_ref() == Ok(reference))
        .ok_or_else(|| Denial::new("review_subject", "Result is not retained"))?;
    let context = evidence::recorded_context(view, reference)?;
    let basis = applicable_evidence(view, &context)?
        .iter()
        .map(|e| e.reference())
        .collect::<Result<Vec<_>>>()?;
    let snapshots = vec![
        view.snapshots()[&result.before].reference()?,
        view.snapshots()[&result.after].reference()?,
    ];
    let purpose = serde_json::json!({"operation":"candidate_review","result":reference,"source":{"author":result.producer,"untrusted":true,"summary":result.summary},"criteria":context.criteria,"response":"Return only CandidateVerdict JSON: {id:new review id,result:{id,version} copied from result,criteria:[{id,version}] copied exactly from criteria,verdict:\"Approve\" or \"Reject\" or \"NeedsEvidence\",findings:[{criterion:criterion id or null,text,severity:\"Blocking\" or \"Advisory\",proposed_check:null}],basis:[supplied Evidence ids],rationale:text}."});
    Ok((reference.clone(), snapshots, basis, purpose))
}
impl<J: Journal, C: ContentStore> AcceptanceAuthority<J, C> {
    pub fn consume_review(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        invocation: &Id<Invocation>,
    ) -> Result<ReviewRecorded> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        if view.revision() != expected {
            return Err(Denial::new(
                "stale_revision",
                "Review consumption boundary changed",
            ));
        }
        for event in self.journal.read(&session)?.events.iter().rev() {
            if let Event::PaidReviewRecorded { data, .. } = &event.payload
                && &data.invocation == invocation
            {
                return Ok(data.review.clone());
            }
        }
        let data = derive(&view, invocation, at)?;
        if view.reviews().contains_key(&data.review.review.id)
            || view
                .finalization()
                .reviews
                .iter()
                .any(|r| r.verdict.id == data.review.review.id)
        {
            return Err(Denial::new(
                "review_conflict",
                "Review identity already has another source",
            ));
        }
        let event = Envelope {
            seq: expected + 1,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy: None,
            input: Some(view.digest()?),
            refs: refs(&view, &data)?,
            payload: Event::PaidReviewRecorded {
                version: 1,
                data: Box::new(data.clone()),
            },
        };
        self.append(&session, expected, event)?;
        Ok(data.review)
    }
}

//! Criterion assessment proposals with kernel-owned applicability and A8 guards.
use crate::{
    acceptance::{ApplicabilityContext, applicable_evidence, review_applicable},
    events::Event,
    ports::planning::*,
    view::SessionView,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use ymp_domain::{
    Denial, Digest, Id, Ref, Result,
    journal::{Decision, Envelope, PolicySelection, SelectionChange},
    task::Criterion,
    verification::*,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedgerRecorded {
    pub context: ApplicabilityContext,
    pub rules: AssessmentRules,
    pub decisions: BTreeMap<Id<Criterion>, Decision<BeliefUpdate>>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct LedgerView {
    pub(crate) entries: BTreeMap<Id<Criterion>, LedgerEntry>,
    pub(crate) criteria: BTreeMap<Id<Criterion>, Ref>,
    pub(crate) subjects: BTreeMap<Id<Criterion>, Ref>,
    pub(crate) assessments: Vec<(Ref, LedgerRecorded)>,
}
impl LedgerView {
    pub fn assessments(&self) -> &[(Ref, LedgerRecorded)] {
        &self.assessments
    }
}
pub struct LedgerRequest {
    pub expected_revision: u64,
    pub at: u64,
    pub context: ApplicabilityContext,
    pub rules: AssessmentRules,
    pub responses: BTreeMap<Id<Criterion>, BeliefResponse>,
}
pub fn validate_context(view: &SessionView, context: &ApplicabilityContext) -> Result<()> {
    let result = view
        .results()
        .results()
        .values()
        .find(|r| r.reference().is_ok_and(|r| r == context.result))
        .ok_or_else(|| {
            Denial::new(
                "assessment_result",
                "No exact retained result for assessment",
            )
        })?;
    let targets = &view.results().items()[&result.item].targets;
    let criteria = targets
        .iter()
        .map(|id| {
            view.criteria()
                .iter()
                .find(|c| &c.id == id)
                .ok_or_else(|| {
                    Denial::new(
                        "assessment_criterion",
                        "A result criterion is no longer current",
                    )
                })?
                .reference()
        })
        .collect::<Result<BTreeSet<_>>>()?;
    if criteria != context.criteria || context.environments.len() > 4096 {
        return Err(Denial::new(
            "assessment_context",
            "Assessment must identify every current result criterion",
        ));
    }
    for (reference, environments) in &context.environments {
        let check = view
            .checks()
            .values()
            .find(|c| c.reference() == *reference)
            .ok_or_else(|| Denial::new("assessment_check", "Unknown expected check version"))?;
        if !targets.contains(&check.criterion)
            || !view.contract().unwrap().checks.contains(&check.id.erased())
            || !criteria.contains(&Ref {
                id: check.criterion.erased(),
                version: check.criterion_version.clone(),
            })
            || environments.is_empty()
            || environments.len() > 128
        {
            return Err(Denial::new(
                "assessment_context",
                "Expected environments must name active checks of this result",
            ));
        }
    }
    Ok(())
}
fn current_subject(view: &SessionView, subject: &Ref) -> bool {
    let Some(result) = view.results().results().values().find(|result| {
        result
            .reference()
            .is_ok_and(|reference| reference == *subject)
    }) else {
        return false;
    };
    let item = &view.results().items()[&result.item];
    if let Some(accepted) = &item.accepted {
        return *accepted == result.id;
    }
    item.attempts
        .last()
        .and_then(|id| view.results().attempts().get(id))
        .is_some_and(|attempt| {
            matches!(
                attempt.attempt.outcome,
                ymp_domain::plan::AttemptOutcome::Submitted
                    | ymp_domain::plan::AttemptOutcome::Rejected(_)
            ) && attempt.attempt.result.as_ref() == Some(&result.id)
        })
}
pub fn current(view: &SessionView, at: u64) -> Result<CriteriaLedger> {
    let mut entries = BTreeMap::new();
    for criterion in view.criteria() {
        let entry = if view.ledger_state().criteria.get(&criterion.id)
            == Some(&criterion.reference()?)
            && view
                .ledger_state()
                .subjects
                .get(&criterion.id)
                .is_some_and(|subject| current_subject(view, subject))
        {
            view.ledger_state().entries.get(&criterion.id).cloned()
        } else {
            None
        };
        entries.insert(
            criterion.id.clone(),
            entry.map_or_else(|| LedgerEntry::initial(at), Ok)?,
        );
    }
    Ok(CriteriaLedger {
        session: view.session().clone(),
        entries,
    })
}
pub fn inputs(
    view: &SessionView,
    context: &ApplicabilityContext,
    rules: &AssessmentRules,
    at: u64,
) -> Result<BTreeMap<Id<Criterion>, BeliefView>> {
    validate_context(view, context)?;
    if !current_subject(view, &context.result) {
        return Err(Denial::new(
            "assessment_stale",
            "Ledger updates require the current submitted or accepted candidate",
        ));
    }
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
    let mut inputs = BTreeMap::new();
    for reference in &context.criteria {
        let criterion = view
            .criteria()
            .iter()
            .find(|c| c.reference().is_ok_and(|r| r == *reference))
            .unwrap();
        let entry = if view.ledger_state().criteria.get(&criterion.id) == Some(reference)
            && view.ledger_state().subjects.get(&criterion.id) == Some(&context.result)
        {
            view.ledger_state().entries.get(&criterion.id).cloned()
        } else {
            None
        }
        .map_or_else(|| LedgerEntry::initial(at), Ok)?;
        let mut groups = BTreeSet::new();
        let mut selected = vec![];
        let mut prior_basis = vec![];
        for record in evidence
            .iter()
            .filter(|e| e.evidence.criterion == criterion.id)
        {
            prior_basis.push(PriorBasis {
                source: record.reference()?,
                polarity: record.evidence.polarity,
            });
            if record.evidence.polarity != Polarity::Supports
                || groups.insert(Digest::of_value(&(
                    record.evidence.independence,
                    &record.evidence.class,
                    &record.evidence.discrimination,
                ))?)
            {
                selected.push(record.evidence.clone());
            }
        }
        for review in &reviews {
            let polarity = match review.review.verdict {
                ReviewVerdict::Approve => Polarity::Supports,
                ReviewVerdict::Reject => Polarity::Contradicts,
                ReviewVerdict::NeedsEvidence => continue,
            };
            prior_basis.push(PriorBasis {
                source: review.reference()?,
                polarity,
            });
        }
        inputs.insert(
            criterion.id.clone(),
            BeliefView {
                journal: view.digest()?,
                entry,
                evidence: selected,
                criterion: criterion.clone(),
                subject: context.result.clone(),
                prior_basis,
                rules: rules.clone(),
                at,
            },
        );
    }
    Ok(inputs)
}
pub(crate) fn thresholds(selection: &PolicySelection) -> Result<BeliefThresholds> {
    let value = selection.parameters.get("thresholds").ok_or_else(|| {
        Denial::new(
            "belief_thresholds",
            "A BeliefModel must declare its satisfaction thresholds",
        )
    })?;
    let thresholds: BeliefThresholds =
        ymp_domain::journal::decode(&ymp_domain::journal::encode(value)?)?;
    thresholds.validate()?;
    Ok(thresholds)
}
fn validate_update(input: &BeliefView, decision: &Decision<BeliefUpdate>) -> Result<()> {
    decision.proposal.validate()?;
    let value = &decision.outcome;
    let prior = &value.prior;
    ymp_domain::require_text(&prior.rationale, 4096)?;
    if decision.proposal.policy != decision.effective.policy
        || decision.proposal.value != *value
        || decision.input != Digest::of_value(input)?
        || prior.subject != input.subject
        || prior.criterion != input.criterion.reference()?
        || prior.value.get() <= 0.0
        || prior.value.get() >= 1.0
        || prior.basis.len() > 128
        || prior.basis.iter().collect::<BTreeSet<_>>().len() != prior.basis.len()
    {
        return Err(Denial::new(
            "belief_attribution",
            "Belief proposal differs from its scoped input, policy or prior",
        ));
    }
    if decision.effective.policy.implementation == "LikelihoodRatioTable"
        && (prior.value.get() != 0.5 || !prior.basis.is_empty())
    {
        return Err(Denial::new(
            "belief_prior",
            "LikelihoodRatioTable starts from its neutral prior",
        ));
    }
    if decision.effective.policy.implementation == "StrongestSupport" {
        let parameters: StrongestSupportParameters = ymp_domain::journal::decode(
            &ymp_domain::journal::encode(&decision.effective.parameters)?,
        )?;
        if prior.value != parameters.prior {
            return Err(Denial::new(
                "belief_prior_parameters",
                "The experimental prior differs from its recorded effective parameter",
            ));
        }
    }
    if prior.value.get() != 0.5 {
        let polarity = if prior.value.get() > 0.5 {
            Polarity::Supports
        } else {
            Polarity::Contradicts
        };
        if prior.basis.is_empty()
            || prior.basis.iter().any(|reference| {
                !input
                    .prior_basis
                    .iter()
                    .any(|b| b.source == *reference && b.polarity == polarity)
            })
        {
            return Err(Denial::new(
                "belief_prior_basis",
                "A non-neutral prior needs applicable present-result evidence or review, never an assignment forecast",
            ));
        }
    } else if prior
        .basis
        .iter()
        .any(|reference| !input.prior_basis.iter().any(|b| b.source == *reference))
    {
        return Err(Denial::new(
            "belief_prior_basis",
            "Prior basis is unrelated to this criterion/result",
        ));
    }
    let expected = entry_for(input, value.entry.belief, &thresholds(&decision.effective)?);
    if value.entry != expected {
        return Err(Denial::new(
            "belief_guard",
            "Contradiction, positive class coverage or ledger metadata cannot be overridden by a strategy",
        ));
    }
    Ok(())
}
pub(crate) fn selection(
    view: &SessionView,
    effective: &PolicySelection,
    change: Option<&SelectionChange>,
    port: &str,
) -> Result<()> {
    let current = view.policies().get(port).ok_or_else(|| {
        Denial::new(
            "policy_selection",
            "No policy is selected for this consumer",
        )
    })?;
    if effective.policy.port != port
        || match change {
            None => effective != current,
            Some(change) => change.previous != current.policy || change.boundary != view.revision(),
        }
    {
        return Err(Denial::new(
            "policy_selection",
            "Decision does not preserve its selected implementation and change boundary",
        ));
    }
    Ok(())
}
pub(crate) fn refs(view: &SessionView, data: &LedgerRecorded) -> Result<Vec<Ref>> {
    let mut refs = vec![data.context.result.clone()];
    refs.extend(data.context.criteria.iter().cloned());
    for evidence in applicable_evidence(view, &data.context)? {
        refs.push(evidence.reference()?);
    }
    for decision in data.decisions.values() {
        refs.extend(decision.proposal.basis.iter().cloned());
        refs.extend(decision.outcome.prior.basis.iter().cloned());
    }
    refs.sort();
    refs.dedup();
    Ok(refs)
}
pub(crate) fn apply(
    view: &SessionView,
    event: &Envelope<Event>,
    data: &LedgerRecorded,
    schemas: &crate::journal::ParameterSchemas,
) -> Result<LedgerView> {
    let inputs = inputs(view, &data.context, &data.rules, event.at)?;
    if data.decisions.keys().collect::<Vec<_>>() != inputs.keys().collect::<Vec<_>>()
        || data.decisions.is_empty()
        || event.at < view.latest_at()
    {
        return Err(Denial::new(
            "ledger_scope",
            "An assessment must cover every current result criterion",
        ));
    }
    let first = data.decisions.values().next().unwrap();
    schemas.validate(&first.effective)?;
    selection(
        view,
        &first.effective,
        first.selection_change.as_ref(),
        "BeliefModel",
    )?;
    if event.policy != Some(first.effective.policy.clone())
        || event.input != Some(view.digest()?)
        || event.refs != refs(view, data)?
    {
        return Err(Denial::new(
            "ledger_attribution",
            "Ledger event differs from its complete input boundary",
        ));
    }
    let mut next = view.ledger_state().clone();
    for (criterion, decision) in &data.decisions {
        if decision.effective != first.effective
            || decision.selection_change != first.selection_change
        {
            return Err(Denial::new(
                "ledger_policy",
                "One assessment uses one explicitly selected model",
            ));
        }
        validate_update(&inputs[criterion], decision)?;
        next.entries
            .insert(criterion.clone(), decision.outcome.entry.clone());
        next.subjects
            .insert(criterion.clone(), data.context.result.clone());
        next.criteria
            .insert(criterion.clone(), inputs[criterion].criterion.reference()?);
    }
    if next.assessments.len() >= 4096 {
        return Err(Denial::new(
            "ledger_limit",
            "Assessment history capacity exhausted",
        ));
    }
    next.assessments.push((event.reference()?, data.clone()));
    Ok(next)
}

//! Reporting work is bound to one initial draft and at most one audited correction.
use super::*;
use crate::{ports::resources::*, treasury};
use ymp_domain::{
    assignment::{
        Assignment, Contribution, ContributionAuthor, ContributionKind, Forecast, ForecastSource,
        Invocation, InvocationTerminal, RoleKind,
    },
    identity::ExecutionProfile,
    journal::{Capability, Decision},
    resources::{
        CostEstimate, Difficulty, Purpose, ReportingMode, ReservationState, ResourceDemand,
    },
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrativeWork {
    pub policy: ymp_domain::journal::PolicySelection,
    pub preparation: Ref,
    pub correction: Option<Ref>,
    pub contribution: Contribution,
    pub profile: ExecutionProfile,
    pub estimate_input: EstimateView,
    pub estimate: Decision<CostEstimate>,
}
pub(crate) fn work<'a>(view: &'a SessionView, id: &Id<Contribution>) -> Option<&'a NarrativeWork> {
    view.finalization().history.iter().find_map(|(_, e)| {
        if let FinalizationRecorded::NarrationWork(w) = e {
            (w.contribution.id == *id).then_some(w.as_ref())
        } else {
            None
        }
    })
}
pub(crate) fn validate_work(view: &SessionView, data: &NarrativeWork) -> Result<()> {
    let (reference, prepared) = report::prepared(view)?;
    if view.policies().get("NarrativeComposer") != Some(&data.policy) {
        return Err(Denial::new(
            "narration_authority",
            "Narration work must retain the actual selected policy",
        ));
    }
    if view.owner_stopped()
        || view.finalization().control != Some(Continuation::Continue)
        || view.finalization().delivered.is_some()
        || !prepared.narrated
        || view
            .treasury()
            .is_none_or(|t| t.reporting_mode != Some(ReportingMode::Narrated))
        || view
            .policies()
            .get("NarrativeComposer")
            .is_none_or(|p| p.policy.implementation != "Narrator")
    {
        return Err(Denial::new(
            "narration_authority",
            "No continuing funded narration authority",
        ));
    }
    let prior: Vec<_> = view
        .finalization()
        .history
        .iter()
        .filter_map(|(_, e)| {
            if let FinalizationRecorded::NarrationWork(w) = e {
                Some(w)
            } else {
                None
            }
        })
        .collect();
    if *reference != data.preparation
        || prior.len() >= 2
        || (prior.is_empty()) != data.correction.is_none()
    {
        return Err(Denial::new(
            "narration_limit",
            "A report has one narration and at most one correction",
        ));
    }
    if let Some(correction) = &data.correction {
        let (audit_ref, audit) = report::last_audit(view)?;
        let (draft_ref, draft) = report::last_draft(view)?;
        if correction != audit_ref
            || audit.draft != *draft_ref
            || draft.invocation.is_none()
            || !audit
                .decisions
                .iter()
                .any(|d| matches!(d.outcome, ClaimAudit::Unsupported(_)))
        {
            return Err(Denial::new(
                "correction_basis",
                "Correction requires an unsupported paid draft and its exact audit",
            ));
        }
    }
    if data.estimate_input != treasury::estimate_view(view, &data.estimate_input.demand)?
        || data.estimate_input.demand.profile != data.profile
        || data.estimate_input.demand.contribution != data.contribution.id.erased()
        || data.estimate_input.demand.kind != ContributionKind::Narrate
        || data.contribution.kind != ContributionKind::Narrate
        || data.contribution.subject.is_some()
        || data.contribution.session != *view.session()
        || data.contribution.targets != view.criteria().iter().map(|c| c.id.clone()).collect()
        || data.contribution.needs != std::collections::BTreeSet::from([Capability::ReadFiles])
        || data.contribution.cost != data.estimate.outcome
        || !data.contribution.basis.contains(reference)
        || data
            .correction
            .as_ref()
            .is_some_and(|r| !data.contribution.basis.contains(r))
    {
        return Err(Denial::new(
            "narration_work",
            "Narration differs from its reporting scope and actual cost estimate",
        ));
    }
    treasury::verify_decision(view, "CostModel", &data.estimate_input, &data.estimate)?;
    data.contribution.validate()?;
    if data.contribution.cost.p90 > view.treasury().unwrap().remaining(Purpose::Reporting)? {
        return Err(Denial::new(
            "narration_funds",
            "Narration lacks defensible reporting funds",
        ));
    }
    Ok(())
}
pub(crate) fn validate_admission(
    view: &SessionView,
    intent: &crate::gatekeeper::AdmissionIntent,
) -> Result<()> {
    if intent.assignment.role != RoleKind::Narrator
        || (!view.policies().contains_key("NarrativeComposer")
            && view.finalization().history.is_empty())
    {
        return Ok(());
    }
    let data = work(view, &intent.assignment.contribution).ok_or_else(|| {
        Denial::new(
            "narration_work",
            "Narration requires its semantic report/correction authorization",
        )
    })?;
    continuing_work(view, data)?;
    if view.finalization().control != Some(Continuation::Continue)
        || view.owner_stopped()
        || view.finalization().delivered.is_some()
        || intent.assignment.profile != data.profile
        || intent.assignment.access != std::collections::BTreeSet::from([Capability::ReadFiles])
        || view
            .admission()
            .assignments()
            .values()
            .any(|a| a.intent.assignment.contribution == data.contribution.id)
    {
        return Err(Denial::new(
            "narration_admission",
            "Narration changed identity, lost continuation, or reused its slot",
        ));
    }
    Ok(())
}
pub(crate) fn purpose(view: &SessionView, assignment: &Assignment) -> Result<serde_json::Value> {
    let work = work(view, &assignment.contribution)
        .ok_or_else(|| Denial::new("narration_work", "No bounded narration work"))?;
    let input = report::narrative_input(view, None)?;
    let correction = work
        .correction
        .as_ref()
        .map(|_| report::last_audit(view).map(|(_, audit)| audit.clone()))
        .transpose()?;
    let runs = if let Some(aggregate) = &input.aggregate {
        crate::acceptance::applicable_evidence(view, &super::context(aggregate)?)?
            .iter()
            .flat_map(|e| e.evidence.runs.iter())
            .filter_map(|id| view.check_runs().get(id))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        vec![]
    };
    Ok(
        serde_json::json!({"operation":"narration","input":input,"runs":runs,"correction":correction,"allowed_claims":report::canonical_options(view)?,"response":"ReportDraft { claims: [{text, assertion}] }. Use exact scoped assertions; ExactBytes proves a byte comparison, not program execution. No authority, cost, unmet or acceptance fields can be supplied by narration."}),
    )
}
pub(crate) fn paid_output(view: &SessionView, invocation: &Id<Invocation>) -> Result<String> {
    if view.owner_stopped() || view.finalization().control != Some(Continuation::Continue) {
        return Err(Denial::new(
            "narrator_unavailable",
            "Stopped or unauthorized reporting uses deterministic facts",
        ));
    }
    let call = view
        .execution()
        .invocations()
        .get(invocation)
        .ok_or_else(|| Denial::new("narrator_unavailable", "No narrator invocation"))?;
    let a = &call.dispatch.assignment;
    let work = work(view, &a.contribution).ok_or_else(|| {
        Denial::new(
            "narrator_unavailable",
            "Invocation is unrelated to this report",
        )
    })?;
    let context = view
        .finalization()
        .history
        .iter()
        .find_map(|(_, e)| {
            if let FinalizationRecorded::Context(c) = e {
                (c.input.assignment == *a).then_some(c)
            } else {
                None
            }
        })
        .ok_or_else(|| Denial::new("narrator_context", "No exact attributed narrator context"))?;
    let account = &view.treasury().unwrap().accounts[&call.dispatch.reservation];
    let admitted = &view.admission().assignments()[&a.id];
    let mut original = admitted.intent.assignment.clone();
    original.state = a.state;
    if original != *a
        || a.role != RoleKind::Narrator
        || a.profile != work.profile
        || call.dispatch.prompt != context.decision.outcome
        || call.terminal != Some(InvocationTerminal::Completed)
        || !call.confirmed_terminal
        || !crate::execution::closed(view, &a.id)
        || crate::execution::limit_reason(view, call, call.ended_at.unwrap_or(0))?.is_some()
        || account.reservation.purpose != Purpose::Reporting
        || account.reservation.state != ReservationState::Settled
        || call.receipt.is_none()
        || account.settlement.as_ref().map(|s| &s.receipt) != call.receipt.as_ref()
    {
        return Err(Denial::new(
            "narrator_unavailable",
            "Only completed, settled, bounded Narrator output is usable",
        ));
    }
    Ok(call.output.clone())
}
impl<J: Journal, C: ContentStore> Finalization<J, C> {
    #[allow(clippy::too_many_arguments)]
    pub fn narration_work(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        id: Id<Contribution>,
        profile: ExecutionProfile,
        correction: bool,
        model: &dyn CostModel,
    ) -> Result<NarrativeWork> {
        let session = self.owner.authorize(owner)?;
        let current = self.journal.read(&session)?;
        let mut view = current.view_with_schemas(&session, None, self.journal.schemas())?;
        if view.policies().get("CostModel") != Some(model.selection()) {
            return Err(Denial::new(
                "policy_selection",
                "Use the selected CostModel",
            ));
        }
        let provider = view
            .registry()
            .and_then(|r| r.input.facts.agents.iter().find(|a| a.id == profile.agent))
            .ok_or_else(|| Denial::new("narrator_profile", "No registered narrator"))?
            .provider
            .clone();
        let demand = ResourceDemand {
            contribution: id.erased(),
            kind: ContributionKind::Narrate,
            difficulty: Difficulty::Simple,
            provider,
            profile: profile.clone(),
        };
        let input = treasury::estimate_view(&view, &demand)?;
        let proposal = model.estimate(&input)?;
        let (preparation, _) = report::prepared(&view)?;
        let correction = if correction {
            Some(report::last_audit(&view)?.0.clone())
        } else {
            None
        };
        let mut basis = vec![preparation.clone()];
        basis.extend(correction.iter().cloned());
        let contribution = Contribution {
            id,
            session: session.clone(),
            kind: ContributionKind::Narrate,
            targets: view.criteria().iter().map(|c| c.id.clone()).collect(),
            subject: None,
            needs: std::collections::BTreeSet::from([Capability::ReadFiles]),
            forecast: Forecast {
                p_success: ymp_domain::Prob::new(0.5)?,
                delta_belief: std::collections::BTreeMap::new(),
                source: ForecastSource::Model(view.policies()["NarrativeComposer"].policy.clone()),
            },
            cost: proposal.value.clone(),
            difficulty: Difficulty::Simple,
            proposed_by: ContributionAuthor::Runtime,
            basis,
        };
        let data = NarrativeWork {
            policy: view.policies()["NarrativeComposer"].clone(),
            preparation: preparation.clone(),
            correction,
            contribution,
            profile,
            estimate: Decision {
                input: Digest::of_value(&input)?,
                outcome: proposal.value.clone(),
                proposal,
                effective: model.selection().clone(),
                selection_change: None,
            },
            estimate_input: input,
        };
        let recorded = FinalizationRecorded::NarrationWork(Box::new(data.clone()));
        let event = Envelope {
            seq: expected + 1,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy: None,
            input: Some(view.digest()?),
            refs: refs(&recorded),
            payload: Event::FinalizationRecorded {
                version: 1,
                data: Box::new(recorded),
            },
        };
        view.apply(&event, self.journal.schemas())?;
        let payload = Event::ContributionProposed {
            version: 1,
            contribution: Box::new(data.contribution.clone()),
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
            expected,
            &events,
            self.journal.schemas(),
        )?;
        self.journal.append(&session, expected, &events)?;
        Ok(data)
    }
}

pub(crate) fn continuing_work(view: &SessionView, data: &NarrativeWork) -> Result<()> {
    let (reference, prepared) = report::prepared(view)?;
    if view.owner_stopped()
        || view.finalization().control != Some(Continuation::Continue)
        || view.finalization().delivered.is_some()
        || !prepared.narrated
        || *reference != data.preparation
        || view.policies().get("NarrativeComposer") != Some(&data.policy)
        || data.policy.policy.implementation != "Narrator"
        || view
            .treasury()
            .is_none_or(|t| t.reporting_mode != Some(ReportingMode::Narrated))
    {
        return Err(Denial::new(
            "narration_authority",
            "Pending narration lost its current semantic authorization",
        ));
    }
    match &data.correction {
        Some(reference) => {
            let (audit_ref, audit) = report::last_audit(view)?;
            if audit_ref != reference
                || report::last_draft(view)?.0 != &audit.draft
                || !audit
                    .decisions
                    .iter()
                    .any(|d| matches!(d.outcome, ClaimAudit::Unsupported(_)))
            {
                return Err(Denial::new(
                    "correction_stale",
                    "Correction no longer names the current unsupported draft",
                ));
            }
        }
        None => {
            if report::last_draft(view).is_ok() {
                return Err(Denial::new(
                    "narration_stale",
                    "A draft or fallback already replaced this pending narration",
                ));
            }
        }
    }
    Ok(())
}

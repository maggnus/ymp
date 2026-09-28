//! A9 work-boundary decisions and bounded recovery; strategies never own authority.
use crate::{
    acceptance::ApplicabilityContext,
    decision::{DecisionConsumer, SessionControl},
    events::Event,
    journal::{Journal, ParameterSchemas, validate_append},
    ports::{progress::*, resources::*},
    view::SessionView,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use ymp_domain::{
    Denial, Digest, Id, Proposal, Ref, Result,
    assignment::*,
    journal::{
        Actor, Decision, Envelope, EscalationStep, PolicySelection, SelectionChange, decode, encode,
    },
    resources::*,
    task::Criterion,
    verification::*,
};
mod facts;
mod handlers;
mod replacement;
pub(crate) use facts::observe;
pub(crate) use handlers::validate_dispatch;
pub use handlers::{RecoveryOutcome, RecoveryWork};
pub(crate) use replacement::replacement_value;
pub use replacement::{ReplacementApproval, ReplacementProposal, ReplacementVerdict};
pub(crate) fn validate_admission(
    view: &SessionView,
    intent: &crate::gatekeeper::AdmissionIntent,
) -> Result<()> {
    let enabled = view.policies().contains_key("ProgressMonitor");
    if enabled
        && view
            .progress()
            .profiles
            .get(&intent.assignment.agent)
            .is_some_and(|prior| prior != &intent.assignment.profile)
    {
        let diagnosed = view
            .progress()
            .history
            .iter()
            .any(|(_, record)| matches!(record, ProgressRecorded::Diagnosis(_)));
        return Err(Denial::new(
            if diagnosed {
                "profile_growth_unavailable"
            } else {
                "diagnosis_required"
            },
            "Changing an existing agent profile requires diagnosis and the future profile-growth handler",
        ));
    }
    handlers::validate_admission(view, intent)?;
    replacement::validate_admission(view, intent)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkFact {
    pub source: Ref,
    pub sequence: u64,
    pub kind: ContributionKind,
    pub targets: BTreeSet<Id<Criterion>>,
    pub outcome: String,
    pub rejection: Option<String>,
    pub accepted: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationEstimate {
    pub input: EstimateView,
    pub decision: Decision<CostEstimate>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisRecorded {
    pub progress: Ref,
    pub context: Option<ApplicabilityContext>,
    pub estimates: Vec<VerificationEstimate>,
    pub input: DiagnosisInput,
    pub decision: Decision<Diagnosis>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ProgressRecorded {
    Policy {
        effective: PolicySelection,
        change: SelectionChange,
    },
    Monitor {
        input: Box<ProgressInput>,
        decision: Box<Decision<ProgressAssessment>>,
    },
    Diagnosis(Box<DiagnosisRecorded>),
    Escalation {
        input: Box<EscalationInput>,
        decision: Box<Decision<EscalationPlan>>,
    },
    Action {
        step: Ref,
        outcome: Box<RecoveryOutcome>,
    },
    Replacement(Box<ReplacementProposal>),
    Replaced {
        proposal: Ref,
        approval: Box<ReplacementApproval>,
    },
}
impl ProgressRecorded {
    pub fn selection(&self) -> Option<&PolicySelection> {
        match self {
            Self::Policy { effective, .. } => Some(effective),
            Self::Monitor { decision, .. } => Some(&decision.effective),
            Self::Diagnosis(d) => Some(&d.decision.effective),
            Self::Escalation { decision, .. } => Some(&decision.effective),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ProgressState {
    pub(crate) environments: BTreeMap<Digest, CheckEnvironment>,
    pub(crate) profiles:
        BTreeMap<Id<ymp_domain::identity::Agent>, ymp_domain::identity::ExecutionProfile>,
    pub facts: Vec<WorkFact>,
    pub history: Vec<(Ref, ProgressRecorded)>,
    pub(crate) pending_work: Option<Contribution>,
}
impl ProgressState {
    pub(crate) fn complete(&self) -> Result<()> {
        if self.pending_work.is_some() {
            return Err(Denial::new(
                "recovery_incomplete",
                "Recovery work and its contribution must commit atomically",
            ));
        }
        Ok(())
    }
    pub(crate) fn check_next(&self, event: &Event) -> Result<()> {
        if let Some(expected) = &self.pending_work
            && !matches!(event,Event::ContributionProposed{contribution,..} if contribution.as_ref()==expected)
        {
            return Err(Denial::new(
                "recovery_incomplete",
                "Recovery work must be followed by its exact contribution",
            ));
        }
        Ok(())
    }

    pub fn monitor(&self) -> Option<(&Ref, &ProgressInput, &Decision<ProgressAssessment>)> {
        self.history.iter().rev().find_map(|(r, e)| {
            if let ProgressRecorded::Monitor { input, decision } = e {
                Some((r, input.as_ref(), decision.as_ref()))
            } else {
                None
            }
        })
    }
    pub fn diagnosis(&self, reference: &Ref) -> Option<&DiagnosisRecorded> {
        self.history.iter().find_map(|(r, e)| {
            if r == reference {
                if let ProgressRecorded::Diagnosis(d) = e {
                    Some(d.as_ref())
                } else {
                    None
                }
            } else {
                None
            }
        })
    }
    pub fn step(&self, reference: &Ref) -> Option<(&EscalationInput, &Decision<EscalationPlan>)> {
        self.history.iter().find_map(|(r, e)| {
            if r == reference {
                if let ProgressRecorded::Escalation { input, decision } = e {
                    Some((input.as_ref(), decision.as_ref()))
                } else {
                    None
                }
            } else {
                None
            }
        })
    }
    pub fn work(&self, contribution: &Id<Contribution>) -> Option<&RecoveryWork> {
        self.history.iter().find_map(|(_, e)| {
            if let ProgressRecorded::Action { outcome, .. } = e {
                if let RecoveryOutcome::Work(work) = outcome.as_ref() {
                    (work.contribution.id == *contribution).then_some(work.as_ref())
                } else {
                    None
                }
            } else {
                None
            }
        })
    }
}
pub struct Progress<J: Journal> {
    pub(crate) journal: Arc<J>,
    owner: DecisionConsumer<J>,
}
pub struct MonitorRequest {
    pub expected_revision: u64,
    pub input: ProgressInput,
    pub proposal: Proposal<ProgressAssessment>,
}
pub struct DiagnosisRequest {
    pub expected_revision: u64,
    pub at: u64,
    pub progress: Ref,
    pub context: Option<ApplicabilityContext>,
    pub estimates: Vec<VerificationEstimate>,
    pub input: DiagnosisInput,
    pub proposal: Proposal<Diagnosis>,
}
pub struct EscalationRequest {
    pub expected_revision: u64,
    pub at: u64,
    pub input: EscalationInput,
    pub proposal: Proposal<EscalationPlan>,
}
pub fn monitor_input(view: &SessionView, at: u64) -> Result<ProgressInput> {
    crate::plans::work_boundary(view)?;
    let ledger = crate::ledger::current(view, at)?;
    let criteria = view.criteria().to_vec();
    let previous = view.progress().monitor();
    let unchanged = previous.is_some_and(|(_, input, _)| input.criteria == criteria);
    let beliefs: BTreeMap<Id<Criterion>, ymp_domain::Prob> = previous
        .map(|(_, input, _)| {
            input
                .ledger
                .entries
                .iter()
                .filter(|(id, _)| {
                    input
                        .criteria
                        .iter()
                        .find(|c| c.id == **id)
                        .is_some_and(|old| criteria.iter().any(|current| current == old))
                })
                .map(|(id, entry)| (id.clone(), entry.belief))
                .collect()
        })
        .unwrap_or_default();
    let history_start = if unchanged {
        previous.unwrap().1.history_start
    } else {
        previous.map(|_| view.revision()).unwrap_or(0)
    };
    let mut rejection_since = if unchanged {
        previous.unwrap().1.rejection_since
    } else {
        history_start
    };
    let delta: f64 = criteria
        .iter()
        .map(|c| {
            c.weight.get()
                * (ledger.entries[&c.id].belief.get()
                    - beliefs
                        .get(&c.id)
                        .copied()
                        .unwrap_or(ymp_domain::Prob::new(0.5).unwrap())
                        .get())
        })
        .sum();
    if delta > 0.0 {
        rejection_since = view
            .progress()
            .facts
            .last()
            .map(|f| f.sequence)
            .unwrap_or(rejection_since);
    }
    if let Some(accepted) = view.progress().facts.iter().rev().find(|f| f.accepted) {
        rejection_since = rejection_since.max(accepted.sequence);
    }
    let looping = looping(&view.progress().facts, history_start, rejection_since);
    let basis: Vec<_> = view
        .progress()
        .facts
        .iter()
        .rev()
        .take(64)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|f| f.source.clone())
        .collect();
    let projected: Vec<_> = ledger
        .entries
        .iter()
        .map(|(id, e)| (id, e.belief, e.status, &e.evidence))
        .collect();
    let boundary = Digest::of_value(&(
        criteria
            .iter()
            .map(|c| c.reference())
            .collect::<Result<Vec<_>>>()?,
        &basis,
        projected,
    ))?;
    Ok(ProgressInput {
        journal: view.digest()?,
        boundary,
        at,
        criteria,
        ledger,
        previous: beliefs,
        stall_count: if unchanged {
            previous.map(|(_, _, d)| d.outcome.stall_count).unwrap_or(0)
        } else {
            0
        },
        history_start,
        rejection_since,
        work_sequence: view
            .progress()
            .facts
            .last()
            .map(|f| f.sequence)
            .unwrap_or(0),
        looping,
        new_acceptance: view.progress().facts.iter().any(|f| {
            f.accepted
                && f.sequence
                    > previous
                        .map(|(_, input, _)| input.work_sequence)
                        .unwrap_or(0)
        }),
        basis,
    })
}
fn decision<T: Clone + PartialEq + Serialize>(
    view: &SessionView,
    port: &str,
    input: &impl Serialize,
    proposal: Proposal<T>,
) -> Result<Decision<T>> {
    proposal.validate()?;
    let effective = view
        .policies()
        .get(port)
        .ok_or_else(|| Denial::new("policy_selection", "No selected progress strategy"))?
        .clone();
    if proposal.policy != effective.policy {
        return Err(Denial::new(
            "policy_selection",
            "Proposal differs from the selected progress policy",
        ));
    }
    for r in &proposal.basis {
        view.resolve(r)?;
    }
    Ok(Decision {
        input: Digest::of_value(input)?,
        outcome: proposal.value.clone(),
        proposal,
        effective,
        selection_change: None,
    })
}
fn verify<T: Clone + PartialEq + Serialize>(
    view: &SessionView,
    port: &str,
    input: &impl Serialize,
    d: &Decision<T>,
) -> Result<()> {
    if decision(view, port, input, d.proposal.clone())? != *d {
        return Err(Denial::new(
            "progress_attribution",
            "Decision differs from its exact input, parameters and proposal",
        ));
    }
    Ok(())
}
pub fn escalation_input(view: &SessionView, diagnosis: &Ref) -> Result<EscalationInput> {
    let source = view.progress().diagnosis(diagnosis).ok_or_else(|| {
        Denial::new(
            "diagnosis_required",
            "Escalation and growth require a recorded diagnosis",
        )
    })?;
    if monitor_input(view, view.latest_at())?.boundary != source.input.boundary {
        return Err(Denial::new(
            "diagnosis_stale",
            "New work requires a new diagnosis",
        ));
    }
    let used = handlers::used_steps(view);
    let mut environment_failed = false;
    for (_, event) in &view.progress().history {
        if let ProgressRecorded::Action { step, outcome } = event
            && let Some((_, decision)) = view.progress().step(step)
        {
            let _ = decision;
            if let RecoveryOutcome::Run {
                old,
                new,
                outcome: CheckOutcome::Error(_),
            } = outcome.as_ref()
            {
                environment_failed |= source.input.environment.contains(new)
                    || source.input.environment.contains(old);
            }
        }
    }
    let mut basis = source.input.basis.clone();
    basis.push(diagnosis.clone());
    basis.sort();
    basis.dedup();
    Ok(EscalationInput {
        at_revision: view.revision(),
        diagnosis: diagnosis.clone(),
        kind: source.decision.outcome,
        method: view.method().cloned().ok_or_else(|| {
            Denial::new(
                "method_missing",
                "Escalation requires the selected method ladder",
            )
        })?,
        needs: source.input.needs.clone(),
        item: source.input.item.clone(),
        used,
        environment_failed,
        basis,
    })
}
pub fn references(data: &ProgressRecorded) -> Vec<Ref> {
    let mut refs = match data {
        ProgressRecorded::Policy { .. } => vec![],
        ProgressRecorded::Monitor { decision, .. } => decision.proposal.basis.clone(),
        ProgressRecorded::Diagnosis(d) => {
            let mut r = d.decision.proposal.basis.clone();
            r.push(d.progress.clone());
            r
        }
        ProgressRecorded::Escalation { decision, .. } => decision.proposal.basis.clone(),
        ProgressRecorded::Action { step, outcome } => {
            let mut r = vec![step.clone()];
            r.extend(outcome.refs());
            r
        }
        ProgressRecorded::Replacement(d) => d.refs(),
        ProgressRecorded::Replaced { proposal, approval } => {
            let mut r = approval.refs();
            r.push(proposal.clone());
            r
        }
    };
    refs.sort();
    refs.dedup();
    refs
}
pub(crate) fn apply(
    view: &SessionView,
    event: &Envelope<Event>,
    data: &ProgressRecorded,
    schemas: &ParameterSchemas,
) -> Result<ProgressState> {
    if event.at < view.latest_at()
        || event.input != Some(view.digest()?)
        || event.policy != data.selection().map(|s| s.policy.clone())
        || event.refs != references(data)
    {
        return Err(Denial::new(
            "progress_event",
            "Progress event differs from its exact recorded boundary",
        ));
    }
    if let Some(p) = data.selection() {
        schemas.validate(p)?;
    }
    match data {
        ProgressRecorded::Policy { effective, change } => {
            crate::plans::work_boundary(view)?;
            if !matches!(
                effective.policy.port.as_str(),
                "ProgressMonitor" | "FailureDiagnoser" | "EscalationPolicy"
            ) {
                return Err(Denial::new("policy_port", "Not a progress port"));
            }
            crate::ledger::selection(view, effective, Some(change), &effective.policy.port)?;
        }
        ProgressRecorded::Monitor { input, decision } => {
            if **input != monitor_input(view, event.at)?
                || view
                    .progress()
                    .monitor()
                    .is_some_and(|(_, previous, _)| previous.boundary == input.boundary)
            {
                return Err(Denial::new(
                    "progress_no_new_work",
                    "This boundary is stale or has no new work or criterion evidence",
                ));
            }
            verify(view, "ProgressMonitor", input, decision)?;
            let p: MonitorParameters = decode(&encode(&decision.effective.parameters)?)?;
            if decision.outcome
                != assessment(
                    input,
                    &p,
                    decision.effective.policy.implementation == "AcceptedOnlyProgress",
                )?
            {
                return Err(Denial::new(
                    "progress_guard",
                    "Progress or stall differs from the recorded work",
                ));
            }
        }
        ProgressRecorded::Diagnosis(d) => {
            let latest = view.progress().monitor().ok_or_else(|| {
                Denial::new("progress_missing", "Assess progress before diagnosis")
            })?;
            if *latest.0 != d.progress
                || d.input
                    != facts::diagnosis_input(view, d.context.as_ref(), &d.estimates, event.at)?
            {
                return Err(Denial::new(
                    "diagnosis_input",
                    "Diagnosis differs from its applicable recorded facts",
                ));
            }
            verify(view, "FailureDiagnoser", &d.input, &d.decision)?;
            let p: DiagnosisParameters = decode(&encode(&d.decision.effective.parameters)?)?;
            if d.decision.outcome
                != diagnose(
                    &d.input,
                    &p,
                    d.decision.effective.policy.implementation == "DirectFailuresOnly",
                )
            {
                return Err(Denial::new(
                    "diagnosis_order",
                    "Diagnosis differs from the selected algorithm and exact A9 order",
                ));
            }
        }
        ProgressRecorded::Escalation { input, decision } => {
            if view.progress().history.iter().any(|(_,event)|matches!(event,ProgressRecorded::Escalation{input:prior,..} if prior.diagnosis==input.diagnosis)){return Err(Denial::new("escalation_duplicate","One diagnosis issues one escalation decision"));}
            if **input != escalation_input(view, &input.diagnosis)? {
                return Err(Denial::new(
                    "escalation_input",
                    "Escalation input is stale or unrelated",
                ));
            }
            verify(view, "EscalationPolicy", input, decision)?;
            let p: EscalationParameters = decode(&encode(&decision.effective.parameters)?)?;
            if decision.outcome
                != escalation(
                    input,
                    &p,
                    decision.effective.policy.implementation == "StopOnUncertainty",
                )?
            {
                return Err(Denial::new(
                    "escalation_guard",
                    "Step differs from the selected diagnosis, method or limits",
                ));
            }
        }
        ProgressRecorded::Action { step, outcome } => {
            handlers::validate_action(view, step, outcome)?
        }
        ProgressRecorded::Replacement(d) => {
            handlers::step(view, &d.step, &EscalationStep::ReplaceCheck)?;
            replacement::validate_proposal(view, d)?;
        }
        ProgressRecorded::Replaced { proposal, approval } => {
            replacement::validate_replaced(view, proposal, approval)?
        }
    }
    let mut state = view.progress().clone();
    if let ProgressRecorded::Action { outcome, .. } = data
        && let RecoveryOutcome::Work(work) = outcome.as_ref()
    {
        state.pending_work = Some(work.contribution.clone());
    }
    if state.history.len() >= 4096 {
        return Err(Denial::new("progress_limit", "Progress history exhausted"));
    }
    state.history.push((event.reference()?, data.clone()));
    Ok(state)
}
impl<J: Journal> Progress<J> {
    pub(crate) fn new(journal: Arc<J>, owner: DecisionConsumer<J>) -> Self {
        Self { journal, owner }
    }
    pub fn input(&self, session: &Id, at: u64) -> Result<ProgressInput> {
        monitor_input(&self.journal.view(session, None)?, at)
    }
    pub fn ledger(&self, session: &Id) -> Result<ProgressLedger> {
        let view = self.journal.view(session, None)?;
        Ok(ProgressLedger {
            session: session.clone(),
            records: view
                .progress()
                .history
                .iter()
                .filter_map(|(reference, e)| {
                    if let ProgressRecorded::Monitor { decision, .. } = e {
                        let mut record = decision.outcome.record.clone();
                        record.diagnosis =
                            view.progress().history.iter().rev().find_map(|(_, event)| {
                                if let ProgressRecorded::Diagnosis(d) = event {
                                    (d.progress == *reference).then_some(d.decision.outcome)
                                } else {
                                    None
                                }
                            });
                        Some(record)
                    } else {
                        None
                    }
                })
                .collect(),
            stall_count: view
                .progress()
                .monitor()
                .map(|(_, _, d)| d.outcome.stall_count)
                .unwrap_or(0),
        })
    }
    pub fn verification_inputs(
        &self,
        session: &Id,
        context: Option<&ApplicabilityContext>,
    ) -> Result<Vec<EstimateView>> {
        facts::verification_inputs(&self.journal.view(session, None)?, context)
    }
    pub fn diagnosis_input(
        &self,
        session: &Id,
        context: Option<&ApplicabilityContext>,
        estimates: &[VerificationEstimate],
        at: u64,
    ) -> Result<DiagnosisInput> {
        facts::diagnosis_input(&self.journal.view(session, None)?, context, estimates, at)
    }
    pub fn escalation_input(&self, session: &Id, diagnosis: &Ref) -> Result<EscalationInput> {
        escalation_input(&self.journal.view(session, None)?, diagnosis)
    }
    pub(crate) fn append(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        data: ProgressRecorded,
    ) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        let current = self.journal.read(&session)?;
        let view = current.view_with_schemas(&session, None, self.journal.schemas())?;
        let event = Envelope {
            seq: expected
                .checked_add(1)
                .ok_or_else(|| Denial::new("revision_overflow", "Journal exhausted"))?,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy: data.selection().map(|s| s.policy.clone()),
            input: Some(view.digest()?),
            refs: references(&data),
            payload: Event::ProgressRecorded {
                version: 1,
                data: Box::new(data),
            },
        };
        validate_append(
            &current,
            &session,
            expected,
            std::slice::from_ref(&event),
            self.journal.schemas(),
        )?;
        self.journal
            .append(&session, expected, std::slice::from_ref(&event))?;
        event.reference()
    }
    pub fn select(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        effective: PolicySelection,
    ) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let current = view
            .policies()
            .get(&effective.policy.port)
            .ok_or_else(|| Denial::new("policy_selection", "Choose the port at session opening"))?;
        self.append(
            owner,
            expected,
            at,
            ProgressRecorded::Policy {
                effective,
                change: SelectionChange {
                    previous: current.policy.clone(),
                    boundary: expected,
                },
            },
        )
    }
    pub fn assess(&self, owner: &SessionControl, r: MonitorRequest) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let d = decision(&view, "ProgressMonitor", &r.input, r.proposal)?;
        self.append(
            owner,
            r.expected_revision,
            r.input.at,
            ProgressRecorded::Monitor {
                input: Box::new(r.input),
                decision: Box::new(d),
            },
        )
    }
    pub fn diagnose(&self, owner: &SessionControl, r: DiagnosisRequest) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let decision = decision(&view, "FailureDiagnoser", &r.input, r.proposal)?;
        self.append(
            owner,
            r.expected_revision,
            r.at,
            ProgressRecorded::Diagnosis(Box::new(DiagnosisRecorded {
                progress: r.progress,
                context: r.context,
                estimates: r.estimates,
                input: r.input,
                decision,
            })),
        )
    }
    pub fn escalate(&self, owner: &SessionControl, r: EscalationRequest) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let d = decision(&view, "EscalationPolicy", &r.input, r.proposal)?;
        self.append(
            owner,
            r.expected_revision,
            r.at,
            ProgressRecorded::Escalation {
                input: Box::new(r.input),
                decision: Box::new(d),
            },
        )
    }
}

fn looping(facts: &[WorkFact], history_start: u64, rejection_since: u64) -> bool {
    let signatures: Vec<_> = facts
        .iter()
        .rev()
        .filter(|f| f.sequence > history_start)
        .take(2)
        .collect();
    let repeated = signatures.len() == 2
        && signatures[0].kind == signatures[1].kind
        && signatures[0].targets == signatures[1].targets
        && signatures[0].outcome == signatures[1].outcome;
    let mut rejections = BTreeMap::new();
    for fact in facts.iter().filter(|f| f.sequence > rejection_since) {
        if let Some(reason) = &fact.rejection {
            *rejections.entry(reason).or_insert(0) += 1;
        }
    }
    repeated || rejections.values().any(|count| *count >= 2)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolving_old_rejections_does_not_erase_consecutive_work() {
        let fact = |sequence| WorkFact {
            source: Ref {
                id: Id::new(format!("work-{sequence}")).unwrap(),
                version: Digest::of(b"test"),
            },
            sequence,
            kind: ContributionKind::Verify,
            targets: BTreeSet::from([Id::new("criterion").unwrap()]),
            outcome: "Pass".into(),
            rejection: None,
            accepted: false,
        };
        let facts = vec![fact(1), fact(2)];
        assert!(looping(&facts, 0, 2));
    }
}

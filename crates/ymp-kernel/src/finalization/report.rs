//! Evidence-audited reporting, with kernel-owned outcome and accounting projection.
use super::*;
use crate::{
    ports::reporting::*,
    treasury::{BudgetControl, Treasury},
};
use ymp_domain::{
    assignment::Invocation,
    journal::{Decision, PolicySelection, SelectionChange},
    resources::{ReportingMode, ReservationState},
    task::{Criterion, Real},
    verification::*,
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrativeParameters {
    pub max_claims: usize,
    pub max_text: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportPrepared {
    pub id: Id<Report>,
    pub narrated: bool,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrativeRecorded {
    pub input: NarrativeInput,
    pub invocation: Option<Id<Invocation>>,
    pub decision: Decision<ReportDraft>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditRecorded {
    pub draft: Ref,
    pub inputs: Vec<ClaimInput>,
    pub decisions: Vec<Decision<ClaimAudit>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountingSummary {
    pub spent: Real,
    pub held: Real,
    pub unknown: bool,
    pub receipts: Vec<Ref>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportDelivered {
    pub retained: Vec<Ref>,
    pub accepted_sources: Vec<FinalSource>,
    pub criteria: Vec<Criterion>,
    pub report: Report,
    pub claims: Vec<Claim>,
    pub aggregate: Option<Ref>,
    pub acceptance: Option<Ref>,
    pub outcome: SessionStatus,
    pub accounting: AccountingSummary,
    pub unresolved: Vec<Ref>,
    pub draft: Ref,
    pub audit: Ref,
}
impl ReportDelivered {
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.report.id.erased(),
            version: Digest::of_value(self)?,
        })
    }
}
pub(crate) fn prepared(view: &SessionView) -> Result<(&Ref, &ReportPrepared)> {
    view.finalization()
        .history
        .iter()
        .rev()
        .find_map(|(r, e)| {
            if let FinalizationRecorded::ReportPrepared(p) = e {
                Some((r, p))
            } else {
                None
            }
        })
        .ok_or_else(|| Denial::new("report_missing", "Prepare one report before composition"))
}
pub(crate) fn last_draft(view: &SessionView) -> Result<(&Ref, &NarrativeRecorded)> {
    view.finalization()
        .history
        .iter()
        .rev()
        .find_map(|(r, e)| {
            if let FinalizationRecorded::Narrative(d) = e {
                Some((r, d.as_ref()))
            } else {
                None
            }
        })
        .ok_or_else(|| Denial::new("report_draft", "No narrative has been committed"))
}
pub(crate) fn last_audit(view: &SessionView) -> Result<(&Ref, &AuditRecorded)> {
    view.finalization()
        .history
        .iter()
        .rev()
        .find_map(|(r, e)| {
            if let FinalizationRecorded::Audit(d) = e {
                Some((r, d.as_ref()))
            } else {
                None
            }
        })
        .ok_or_else(|| Denial::new("report_audit", "Audit the report before delivery"))
}
fn final_scope(view: &SessionView) -> Option<&FinalAggregate> {
    view.finalization()
        .aggregate
        .as_ref()
        .filter(|a| aggregate_current(view, a).is_ok())
}
pub fn narrative_input(view: &SessionView, source: Option<String>) -> Result<NarrativeInput> {
    let (_, prepared) = prepared(view)?;
    let aggregate = final_scope(view);
    let acceptance = view
        .finalization()
        .acceptance
        .as_ref()
        .filter(|a| {
            aggregate.is_some_and(|scope| scope.reference().is_ok_and(|r| r == a.aggregate))
        })
        .map(|a| a.acceptance.clone());
    let assessment = aggregate
        .and_then(|scope| {
            view.ledger_state()
                .assessments()
                .iter()
                .rev()
                .find(|(_, assessment)| {
                    scope
                        .reference()
                        .is_ok_and(|r| r == assessment.context.result)
                })
        })
        .map(|(_, assessment)| assessment);
    let unmet = view
        .criteria()
        .iter()
        .filter(|c| {
            c.required
                && assessment
                    .and_then(|a| a.decisions.get(&c.id))
                    .is_none_or(|d| d.outcome.entry.status != LedgerStatus::Satisfied)
        })
        .cloned()
        .collect();
    let mut basis = vec![
        view.contract()
            .ok_or_else(|| Denial::new("task_missing", "No report task"))?
            .reference(),
    ];
    if let Some(a) = aggregate {
        basis.push(a.reference()?);
    }
    if let Some(a) = &acceptance {
        basis.push(a.reference()?);
    }
    basis.sort();
    basis.dedup();
    Ok(NarrativeInput {
        retained: view
            .results()
            .results()
            .values()
            .map(|r| r.reference())
            .collect::<Result<_>>()?,
        journal: view.digest()?,
        report: prepared.id.clone(),
        final_acceptance: acceptance,
        aggregate: aggregate.cloned(),
        outcome: view
            .finalization()
            .outcome
            .clone()
            .unwrap_or(SessionStatus::Finalizing),
        unmet,
        source,
        basis,
    })
}
pub fn claim_input(view: &SessionView, claim: DraftClaim) -> Result<ClaimInput> {
    let input = narrative_input(view, None)?;
    let mut runs = vec![];
    let mut checks = vec![];
    if let Some(aggregate) = &input.aggregate {
        let context = super::context(aggregate)?;
        let evidence = crate::acceptance::applicable_evidence(view, &context)?;
        for record in evidence {
            for id in &record.evidence.runs {
                let run = &view.check_runs()[id];
                if !runs.contains(run) {
                    runs.push(run.clone());
                }
                let check = &view.checks()[&run.check];
                if !checks.contains(check) {
                    checks.push(check.clone());
                }
            }
        }
    }
    let mut recommendations: Vec<_> = input
        .unmet
        .iter()
        .map(|c| c.reference())
        .collect::<Result<_>>()?;
    recommendations.extend(view.progress().history.iter().filter_map(|(r, e)| {
        matches!(e, crate::progress::ProgressRecorded::Diagnosis(_)).then_some(r.clone())
    }));
    Ok(ClaimInput {
        retained: input.retained,
        journal: view.digest()?,
        claim,
        aggregate: input.aggregate,
        final_acceptance: input.final_acceptance,
        runs,
        checks,
        recommendations,
    })
}
pub fn canonical_claim(input: &ClaimInput) -> Result<Option<String>> {
    let text = match &input.claim.assertion {
        Assertion::Retained { result } => {
            if !input.retained.contains(result) {
                return Ok(None);
            }
            format!("Retained result version: {}.", result.id)
        }
        Assertion::Accepted { acceptance } => match &input.final_acceptance {
            Some(a)
                if a.reference()? == *acceptance && a.decision == AcceptanceDecision::Accepted =>
            {
                format!("Final result accepted; confirmation grade: {:?}.", a.grade)
            }
            _ => return Ok(None),
        },
        Assertion::Executed { run, program } => {
            let Some(r) = input
                .runs
                .iter()
                .find(|r| r.reference().is_ok_and(|reference| reference == *run))
            else {
                return Ok(None);
            };
            let Some(a) = &input.aggregate else {
                return Ok(None);
            };
            if r.role != CheckRunRole::Candidate
                || r.target.erased() != a.snapshot.id
                || r.target_version != a.snapshot.version
                || r.outcome != CheckOutcome::Pass
            {
                return Ok(None);
            }
            let Some(check) = input
                .checks
                .iter()
                .find(|c| c.id == r.check && c.version == r.check_version)
            else {
                return Ok(None);
            };
            match (&check.spec, program) {
                (CheckSpec::ExactBytes { .. }, false) => {
                    "The exact byte comparison passed on the integrated snapshot.".into()
                }
                (CheckSpec::Command { .. }, true) => {
                    "The recorded command check executed and passed on the integrated snapshot."
                        .into()
                }
                _ => return Ok(None),
            }
        }
        Assertion::Browser { .. } => return Ok(None),
        Assertion::Causal { before, after } => {
            let (Some(a), Some(b), Some(scope)) = (
                input
                    .runs
                    .iter()
                    .find(|r| r.reference().is_ok_and(|r| r == *before)),
                input
                    .runs
                    .iter()
                    .find(|r| r.reference().is_ok_and(|r| r == *after)),
                input.aggregate.as_ref(),
            ) else {
                return Ok(None);
            };
            if a.check != b.check
                || a.check_version != b.check_version
                || a.env != b.env
                || a.role != CheckRunRole::Baseline
                || b.role != CheckRunRole::Candidate
                || a.outcome != CheckOutcome::Fail
                || b.outcome != CheckOutcome::Pass
                || scope
                    .baseline
                    .as_ref()
                    .is_none_or(|r| r.id != a.target.erased() || r.version != a.target_version)
                || scope.snapshot.id != b.target.erased()
                || scope.snapshot.version != b.target_version
            {
                return Ok(None);
            }
            "The recorded change changed this check from failing on its baseline to passing on the integrated snapshot.".into()
        }
        Assertion::Scope { universal: false } if input.aggregate.is_some() => {
            "This report is limited to the recorded integrated snapshot.".into()
        }
        Assertion::Scope { .. } => return Ok(None),
        Assertion::Recommendation { basis } if input.recommendations.contains(basis) => format!(
            "Further work should address the recorded basis {}.",
            basis.id
        ),
        Assertion::Recommendation { .. } => return Ok(None),
        Assertion::Uncertainty => {
            if input.claim.text == "Uncertain: no broader claim is established by this statement."
                || (input.claim.text
                    == "Uncertain: independent final acceptance is unavailable; retained work is not presented as a completed artifact."
                    && input
                        .final_acceptance
                        .as_ref()
                        .is_none_or(|a| a.decision != AcceptanceDecision::Accepted))
            {
                return Ok(Some(input.claim.text.clone()));
            }
            return Ok(None);
        }
    };
    Ok(Some(text))
}
pub(crate) fn canonical_options(view: &SessionView) -> Result<Vec<DraftClaim>> {
    let facts = narrative_input(view, None)?;
    let mut assertions = vec![];
    if let Some(acceptance) = &facts.final_acceptance {
        assertions.push(Assertion::Accepted {
            acceptance: acceptance.reference()?,
        });
    }
    if facts.aggregate.is_some() {
        assertions.push(Assertion::Scope { universal: false });
    }
    for result in facts.retained.iter().take(8) {
        assertions.push(Assertion::Retained {
            result: result.clone(),
        });
    }
    for criterion in facts.unmet.iter().take(8) {
        assertions.push(Assertion::Recommendation {
            basis: criterion.reference()?,
        });
    }
    let sample = claim_input(
        view,
        DraftClaim {
            text: String::new(),
            assertion: Assertion::Uncertainty,
        },
    )?;
    for run in sample
        .runs
        .iter()
        .filter(|r| r.role == CheckRunRole::Candidate)
        .take(8)
    {
        let program = sample
            .checks
            .iter()
            .find(|c| c.id == run.check)
            .is_some_and(|c| matches!(c.spec, CheckSpec::Command { .. }));
        assertions.push(Assertion::Executed {
            run: run.reference()?,
            program,
        });
        for before in sample
            .runs
            .iter()
            .filter(|r| r.role == CheckRunRole::Baseline && r.check == run.check)
            .take(1)
        {
            assertions.push(Assertion::Causal {
                before: before.reference()?,
                after: run.reference()?,
            });
        }
    }
    let mut claims = vec![];
    for assertion in assertions {
        let mut input = sample.clone();
        input.claim.assertion = assertion.clone();
        if let Some(text) = canonical_claim(&input)? {
            claims.push(DraftClaim { text, assertion });
        }
    }
    Ok(claims)
}
pub fn audit_claim(input: &ClaimInput, conservative: bool) -> Result<ClaimAudit> {
    if conservative
        && matches!(
            input.claim.assertion,
            Assertion::Executed { .. } | Assertion::Causal { .. } | Assertion::Browser { .. }
        )
    {
        return Ok(ClaimAudit::Unsupported(
            "Conservative audit reports only explicit status and bounded scope".into(),
        ));
    }
    match canonical_claim(input)? {
        Some(text) if text == input.claim.text => Ok(ClaimAudit::Valid),
        _ => Ok(ClaimAudit::Unsupported(
            "The wording or assertion exceeds its applicable evidence and exact scope".into(),
        )),
    }
}
pub fn deterministic(input: &NarrativeInput) -> Result<ReportDraft> {
    let mut claims = vec![];
    if let Some(a) = &input.final_acceptance
        && a.decision == AcceptanceDecision::Accepted
    {
        claims.push(DraftClaim {
            text: format!("Final result accepted; confirmation grade: {:?}.", a.grade),
            assertion: Assertion::Accepted {
                acceptance: a.reference()?,
            },
        });
    }
    // Full retained versions, unmet criteria and assumptions are kernel facts below;
    // one bounded claim keeps the zero-call fallback valid at every configured cap.
    if claims.is_empty() {
        claims.push(DraftClaim{text:"Uncertain: independent final acceptance is unavailable; retained work is not presented as a completed artifact.".into(),assertion:Assertion::Uncertainty});
    }
    Ok(ReportDraft { claims })
}
pub(crate) fn validate_narrative(view: &SessionView, data: &NarrativeRecorded) -> Result<()> {
    let source = if let Some(id) = &data.invocation {
        Some(paid_narrative(view, id)?)
    } else {
        None
    };
    let input = narrative_input(view, source)?;
    let source_reused = data.invocation.as_ref().is_some_and(|id| {
        view.finalization().history.iter().any(|(_, event)| {
            matches!(event, FinalizationRecorded::Narrative(prior) if prior.invocation.as_ref() == Some(id))
        })
    });
    if view.finalization().delivered.is_some() || source_reused {
        return Err(Denial::new(
            "narrative_source_reused",
            "A paid invocation supplies one draft and delivered reports are immutable",
        ));
    }
    data.decision.proposal.validate()?;
    let parameters: NarrativeParameters = ymp_domain::journal::decode(
        &ymp_domain::journal::encode(&data.decision.effective.parameters)?,
    )?;
    if data.input != input
        || data.decision.input != Digest::of_value(&input)?
        || data.decision.proposal.policy != data.decision.effective.policy
        || data.decision.proposal.value != data.decision.outcome
        || data.decision.outcome.claims.is_empty()
        || data.decision.outcome.claims.len() > parameters.max_claims
        || data
            .decision
            .outcome
            .claims
            .iter()
            .any(|c| c.text.is_empty() || c.text.len() > parameters.max_text)
    {
        return Err(Denial::new(
            "narrative_attribution",
            "Narrative differs from its paid output or bounded selected policy",
        ));
    }
    crate::ledger::selection(
        view,
        &data.decision.effective,
        data.decision.selection_change.as_ref(),
        "NarrativeComposer",
    )?;
    match data.decision.effective.policy.implementation.as_str() {
        "Narrator" => {
            if view.finalization().stopped
                || view.finalization().control != Some(Continuation::Continue)
                || !prepared(view)?.1.narrated
            {
                return Err(Denial::new(
                    "narrator_unavailable",
                    "Stop or absent continuation requires deterministic fallback",
                ));
            }
            let draft: ReportDraft = ymp_domain::journal::decode(
                input
                    .source
                    .as_ref()
                    .ok_or_else(|| {
                        Denial::new(
                            "narrative_source",
                            "Narrator requires a paid completed source",
                        )
                    })?
                    .as_bytes(),
            )?;
            if draft != data.decision.outcome {
                return Err(Denial::new(
                    "narrative_source",
                    "Narrative differs from its actual output",
                ));
            }
        }
        "DeterministicReport" => {
            if data.invocation.is_some() || data.decision.outcome != deterministic(&input)? {
                return Err(Denial::new(
                    "deterministic_report",
                    "Fallback must derive only recorded facts",
                ));
            }
        }
        _ => {
            return Err(Denial::new(
                "narrative_policy",
                "Unsupported narrative implementation",
            ));
        }
    }
    Ok(())
}
pub(crate) fn validate_audit(view: &SessionView, data: &AuditRecorded) -> Result<()> {
    let (reference, draft) = last_draft(view)?;
    if *reference != data.draft
        || data.inputs.len() != draft.decision.outcome.claims.len()
        || data.decisions.len() != data.inputs.len()
    {
        return Err(Denial::new(
            "claim_audit",
            "Audit must cover every claim in the current draft",
        ));
    }
    for ((claim, input), decision) in draft
        .decision
        .outcome
        .claims
        .iter()
        .zip(&data.inputs)
        .zip(&data.decisions)
    {
        decision.proposal.validate()?;
        for reference in &decision.proposal.basis {
            view.resolve(reference)?;
        }
        if *input != claim_input(view, claim.clone())?
            || decision.input != Digest::of_value(input)?
            || view.policies().get("ClaimAuditor") != Some(&decision.effective)
            || decision.proposal.policy != decision.effective.policy
            || decision.proposal.value != decision.outcome
            || decision.selection_change.is_some()
            || decision.outcome
                != audit_claim(
                    input,
                    decision.effective.policy.implementation == "ConservativeAudit",
                )?
        {
            return Err(Denial::new(
                "claim_audit",
                "Claim audit differs from its applicable evidence or policy",
            ));
        }
    }
    Ok(())
}
fn assertion_refs(assertion: &Assertion) -> Vec<Ref> {
    match assertion {
        Assertion::Retained { result } => vec![result.clone()],
        Assertion::Accepted { acceptance } => vec![acceptance.clone()],
        Assertion::Executed { run, .. } | Assertion::Browser { run } => vec![run.clone()],
        Assertion::Causal { before, after } => vec![before.clone(), after.clone()],
        Assertion::Recommendation { basis } => vec![basis.clone()],
        _ => vec![],
    }
}
pub(crate) fn delivered(view: &SessionView) -> Result<ReportDelivered> {
    let (draft_ref, draft) = last_draft(view)?;
    let (audit_ref, audit) = last_audit(view)?;
    if audit.draft != *draft_ref || view.finalization().delivered.is_some() {
        return Err(Denial::new(
            "report_delivery",
            "Deliver the audited current draft exactly once",
        ));
    }
    let input = narrative_input(view, None)?;
    let mut claims = vec![];
    for (index, (claim, audit)) in draft
        .decision
        .outcome
        .claims
        .iter()
        .zip(&audit.decisions)
        .enumerate()
    {
        let (text, kind, evidence) = if audit.outcome == ClaimAudit::Valid
            && audit_claim(
                &claim_input(view, claim.clone())?,
                audit.effective.policy.implementation == "ConservativeAudit",
            )? == ClaimAudit::Valid
        {
            (
                claim.text.clone(),
                match claim.assertion {
                    Assertion::Causal { .. } => ClaimKind::Causal,
                    Assertion::Scope { .. } => ClaimKind::Scope,
                    Assertion::Recommendation { .. } => ClaimKind::Recommendation,
                    _ => ClaimKind::Status,
                },
                assertion_refs(&claim.assertion),
            )
        } else {
            ("Uncertain: an unsupported narrative statement was removed; no broader result is established.".into(),ClaimKind::Status,vec![])
        };
        claims.push(Claim {
            id: Id::new(format!(
                "claim-{}-{}",
                Digest::of(input.report.as_str()),
                index
            ))?,
            report: input.report.clone(),
            text,
            kind,
            evidence,
            audit: ClaimAudit::Valid,
        });
    }
    let book = view
        .treasury()
        .ok_or_else(|| Denial::new("budget_missing", "Reports retain actual accounting"))?;
    let mut receipts = vec![];
    let mut unknown = book.unbounded || book.unsettled_usage;
    for account in book.accounts.values() {
        if account.invocation.is_some() {
            receipts.push(account.last.clone());
            unknown |= !matches!(
                account.reservation.state,
                ReservationState::Settled | ReservationState::Released
            ) || account.settlement.as_ref().is_some_and(|s| {
                matches!(
                    s.decision.outcome,
                    ymp_domain::resources::ReceiptPrice::Estimated(_)
                        | ymp_domain::resources::ReceiptPrice::Unknown
                )
            });
        }
    }
    let accounting = AccountingSummary {
        spent: book.budget.spent,
        held: book.budget.held,
        unknown,
        receipts,
    };
    let unresolved = view
        .coordination()
        .commitments()
        .values()
        .filter(|c| matches!(c.state, ymp_domain::coordination::CommitmentState::Active))
        .filter_map(|c| c.history.last().cloned())
        .collect();
    let accepted_sources = view
        .acceptances()
        .values()
        .filter(|a| a.acceptance.decision == AcceptanceDecision::Accepted)
        .map(|a| {
            Ok(FinalSource {
                result: a.result.clone(),
                acceptance: a.acceptance.reference()?,
            })
        })
        .collect::<Result<_>>()?;
    Ok(ReportDelivered {
        retained: input.retained,
        accepted_sources,
        criteria: view.criteria().to_vec(),
        report: Report {
            id: input.report.clone(),
            session: view.session().clone(),
            claims: claims.iter().map(|c| c.id.clone()).collect(),
            unmet: input.unmet.iter().map(|c| c.id.clone()).collect(),
            assumptions: view.task().unwrap().goal.assumptions.clone(),
            grade: input
                .final_acceptance
                .as_ref()
                .map(|a| a.grade)
                .unwrap_or(ConfirmationGrade::Unconfirmed),
        },
        claims,
        aggregate: input
            .aggregate
            .as_ref()
            .map(|a| a.reference())
            .transpose()?,
        acceptance: input
            .final_acceptance
            .as_ref()
            .map(|a| a.reference())
            .transpose()?,
        outcome: if matches!(
            input.outcome,
            SessionStatus::Blocked(_) | SessionStatus::Cancelled
        ) {
            input.outcome
        } else if input
            .final_acceptance
            .as_ref()
            .is_some_and(|a| a.decision == AcceptanceDecision::Accepted)
        {
            SessionStatus::Delivered
        } else {
            SessionStatus::Blocked("final_acceptance_unavailable".into())
        },
        accounting,
        unresolved,
        draft: draft_ref.clone(),
        audit: audit_ref.clone(),
    })
}
// Native narration is connected by the same recorded work/context boundary below.
fn paid_narrative(view: &SessionView, id: &Id<Invocation>) -> Result<String> {
    super::narration::paid_output(view, id)
}
impl<J: Journal, C: ContentStore> Finalization<J, C> {
    pub fn prepare_report(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        id: Id<Report>,
        treasury: &Treasury<J>,
        budget: &BudgetControl,
    ) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        treasury.require_control(&self.journal, budget, &session)?;
        let view = self.journal.view(&session, None)?;
        if view.revision() != expected
            || view
                .finalization()
                .history
                .iter()
                .any(|(_, e)| matches!(e, FinalizationRecorded::ReportPrepared(_)))
        {
            return Err(Denial::new(
                "report_phase",
                "Prepare a report once at its current boundary",
            ));
        }
        let book = view
            .treasury()
            .ok_or_else(|| Denial::new("budget_missing", "No reporting reserve"))?;
        let selected = view
            .policies()
            .get("NarrativeComposer")
            .ok_or_else(|| Denial::new("policy_selection", "No NarrativeComposer selected"))?;
        let may_narrate = view.finalization().control == Some(Continuation::Continue)
            && !view.finalization().stopped
            && selected.policy.implementation == "Narrator"
            && book.reporting_plan.narration.is_some()
            && book
                .remaining(ymp_domain::resources::Purpose::Reporting)
                .is_ok_and(|r| r.get() > 0.0)
            && book.reporting_mode != Some(ReportingMode::Deterministic);
        let narrated = if book.reporting_mode.is_none() {
            if may_narrate
                && treasury
                    .start_reporting(budget, expected, at, ReportingMode::Narrated)
                    .is_ok()
            {
                true
            } else {
                let current = self.journal.view(&session, None)?;
                treasury.start_reporting(
                    budget,
                    current.revision(),
                    at,
                    ReportingMode::Deterministic,
                )?;
                false
            }
        } else {
            may_narrate && book.reporting_mode == Some(ReportingMode::Narrated)
        };
        let current = self.journal.view(&session, None)?;
        let reason = if narrated {
            "Bounded narration and at most one audited correction"
        } else {
            "Deterministic fallback: stop, missing authority, unavailable narration or indefensible funds"
        };
        self.append(
            owner,
            current.revision(),
            at,
            FinalizationRecorded::ReportPrepared(ReportPrepared {
                id,
                narrated,
                reason: reason.into(),
            }),
        )
    }
    pub fn narrative_input(
        &self,
        session: &Id,
        invocation: Option<&Id<Invocation>>,
    ) -> Result<NarrativeInput> {
        let view = self.journal.view(session, None)?;
        narrative_input(
            &view,
            invocation.map(|id| paid_narrative(&view, id)).transpose()?,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn record_narrative(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        input: NarrativeInput,
        invocation: Option<Id<Invocation>>,
        proposal: ymp_domain::Proposal<ReportDraft>,
        replacement: Option<PolicySelection>,
    ) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let current = view
            .policies()
            .get("NarrativeComposer")
            .ok_or_else(|| Denial::new("policy_selection", "No NarrativeComposer selected"))?;
        let (effective, change) = match replacement {
            Some(next) => (
                next,
                Some(SelectionChange {
                    previous: current.policy.clone(),
                    boundary: view.revision(),
                }),
            ),
            None => (current.clone(), None),
        };
        let data = NarrativeRecorded {
            decision: Decision {
                input: Digest::of_value(&input)?,
                outcome: proposal.value.clone(),
                proposal,
                effective,
                selection_change: change,
            },
            input,
            invocation,
        };
        self.append(
            owner,
            expected,
            at,
            FinalizationRecorded::Narrative(Box::new(data)),
        )
    }
    pub fn claim_inputs(&self, session: &Id) -> Result<Vec<ClaimInput>> {
        let view = self.journal.view(session, None)?;
        last_draft(&view)?
            .1
            .decision
            .outcome
            .claims
            .iter()
            .map(|c| claim_input(&view, c.clone()))
            .collect()
    }
    pub fn audit(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        inputs: Vec<ClaimInput>,
        proposals: Vec<ymp_domain::Proposal<ClaimAudit>>,
    ) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        if inputs.len() != proposals.len() {
            return Err(Denial::new("claim_audit", "Audit each claim"));
        }
        let effective = view
            .policies()
            .get("ClaimAuditor")
            .ok_or_else(|| Denial::new("policy_selection", "No ClaimAuditor selected"))?;
        let decisions = inputs
            .iter()
            .zip(proposals)
            .map(|(input, proposal)| {
                Ok(Decision {
                    input: Digest::of_value(input)?,
                    outcome: proposal.value.clone(),
                    proposal,
                    effective: effective.clone(),
                    selection_change: None,
                })
            })
            .collect::<Result<_>>()?;
        self.append(
            owner,
            expected,
            at,
            FinalizationRecorded::Audit(Box::new(AuditRecorded {
                draft: last_draft(&view)?.0.clone(),
                inputs,
                decisions,
            })),
        )
    }
    pub fn deliver(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
    ) -> Result<ReportDelivered> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let report = delivered(&view)?;
        self.append(
            owner,
            expected,
            at,
            FinalizationRecorded::Delivered(Box::new(report.clone())),
        )?;
        Ok(report)
    }
}

pub fn render_report(data: &ReportDelivered) -> String {
    let mut text = data
        .claims
        .iter()
        .map(|c| c.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    text.push_str(&format!(
        "\n\nOutcome: {:?}\nConfirmation grade: {:?}\n",
        data.outcome, data.report.grade
    ));
    for id in &data.report.unmet {
        if let Some(c) = data.criteria.iter().find(|c| c.id == *id) {
            text.push_str(&format!("Unmet criterion {}: {:?}\n", id, c.text));
        }
    }
    for assumption in &data.report.assumptions {
        text.push_str(&format!(
            "Assumption: {:?}; reason: {:?}\n",
            assumption.text, assumption.reason
        ));
    }
    for result in &data.retained {
        text.push_str(&format!(
            "Retained result: {} at {}\n",
            result.id, result.version
        ));
    }
    text.push_str(&format!(
        "Cost units settled: {}; held: {}; estimated or unresolved costs: {}\n",
        data.accounting.spent.get(),
        data.accounting.held.get(),
        data.accounting.unknown
    ));
    text.push_str(&format!(
        "Unresolved commitments: {}\n",
        data.unresolved.len()
    ));
    text
}

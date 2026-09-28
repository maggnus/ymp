//! Full check replacement proposals require a paid, independent exact approval.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplacementProposal {
    pub step: Ref,
    pub old: Ref,
    pub new: Check,
    pub criterion: Ref,
    pub result: Ref,
    pub contract: Ref,
    pub defect: Vec<Ref>,
    pub reason: String,
}
impl ReplacementProposal {
    pub fn refs(&self) -> Vec<Ref> {
        let mut refs = vec![
            self.step.clone(),
            self.old.clone(),
            self.criterion.clone(),
            self.result.clone(),
            self.contract.clone(),
        ];
        refs.extend(self.defect.clone());
        if let Some(v) = &self.new.verifier {
            refs.push(v.clone());
        }
        refs
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplacementVerdict {
    pub proposal: Ref,
    pub old: Ref,
    pub new: Ref,
    pub criterion: Ref,
    pub result: Ref,
    pub approve: bool,
    pub rationale: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplacementApproval {
    pub invocation: Id<Invocation>,
    pub assignment: Ref,
    pub admission: Ref,
    pub completion: Ref,
    pub receipt: Ref,
    pub verdict: ReplacementVerdict,
}
impl ReplacementApproval {
    pub fn refs(&self) -> Vec<Ref> {
        vec![
            self.admission.clone(),
            self.completion.clone(),
            self.receipt.clone(),
            self.verdict.old.clone(),
            self.verdict.new.clone(),
            self.verdict.criterion.clone(),
            self.verdict.result.clone(),
        ]
    }
}
pub(super) fn proposal<'a>(
    view: &'a SessionView,
    reference: &Ref,
) -> Result<&'a ReplacementProposal> {
    view.progress()
        .history
        .iter()
        .find_map(|(r, e)| {
            if r == reference {
                if let ProgressRecorded::Replacement(d) = e {
                    Some(d.as_ref())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .ok_or_else(|| Denial::new("replacement_missing", "No exact staged replacement"))
}
pub(super) fn validate_proposal(view: &SessionView, data: &ReplacementProposal) -> Result<()> {
    let (input, _) = handlers::bound_step(view, &data.step, &EscalationStep::ReplaceCheck)?;
    let diagnosis = view.progress().diagnosis(&input.diagnosis).unwrap();
    data.new.validate()?;
    ymp_domain::require_text(&data.reason, 4096)?;
    let old = view
        .checks()
        .values()
        .find(|c| c.reference() == data.old)
        .ok_or_else(|| Denial::new("replacement_check", "No exact old check"))?;
    let result = view
        .results()
        .results()
        .values()
        .find(|r| r.reference().is_ok_and(|r| r == data.result))
        .ok_or_else(|| {
            Denial::new(
                "replacement_result",
                "Replacement review requires an actual candidate",
            )
        })?;
    let item = &view.results().items()[&result.item];
    if diagnosis.context.as_ref().map(|c| &c.result) != Some(&data.result)
        || diagnosis.input.item.as_ref() != Some(&result.item)
        || data.defect.iter().any(|reference| {
            view.check_runs()
                .values()
                .find(|r| r.reference().is_ok_and(|r| r == *reference))
                .is_none_or(|run| run.check != old.id || run.check_version != old.version)
        })
    {
        return Err(Denial::new(
            "replacement_defect",
            "The exact old check and candidate must own the recorded defect",
        ));
    }

    if data.contract != view.contract().unwrap().reference()
        || !view.contract().unwrap().checks.contains(&old.id.erased())
        || view.checks().contains_key(&data.new.id)
        || data.new.id == old.id
        || data.new.criterion != old.criterion
        || data.criterion
            != (Ref {
                id: old.criterion.erased(),
                version: old.criterion_version.clone(),
            })
        || data.new.criterion_version != old.criterion_version
        || data.new.author != CheckAuthor::User
        || data.new.independence != Independence::Trusted
        || !data
            .new
            .needs
            .is_subset(&view.task().unwrap().constraints.allowed)
        || item.accepted.is_some()
        || !item.targets.contains(&old.criterion)
        || data.defect.is_empty()
        || data.defect.iter().any(|r| {
            !diagnosis.input.check_defect.contains(r) && !diagnosis.input.environment.contains(r)
        })
    {
        return Err(Denial::new(
            "replacement_scope",
            "Replacement must preserve the exact criterion and candidate, name a defect and retain user authorship",
        ));
    }
    if let Some(verifier) = &data.new.verifier
        && !view
            .snapshots()
            .values()
            .any(|s| s.reference().is_ok_and(|r| r == *verifier))
    {
        return Err(Denial::new(
            "replacement_verifier",
            "The verifier snapshot must be retained",
        ));
    }
    Ok(())
}
fn prompt(view: &SessionView, reference: &Ref) -> Result<Prompt> {
    let data = proposal(view, reference)?;
    let mut basis = data.refs();
    basis.push(reference.clone());
    basis.push(data.new.reference());
    basis.sort();
    basis.dedup();
    Ok(Prompt{text:String::from_utf8(encode(&("Independently review this complete check replacement. Return ReplacementVerdict with exact proposal, old, new, criterion, result refs, approve and rationale",reference,data))?).unwrap(),basis})
}
fn approval(
    view: &SessionView,
    reference: &Ref,
    invocation: &Id<Invocation>,
) -> Result<ReplacementApproval> {
    let data = proposal(view, reference)?;
    validate_proposal(view, data)?;
    let call = view
        .execution()
        .invocations()
        .get(invocation)
        .ok_or_else(|| Denial::new("replacement_review", "No paid review invocation"))?;
    let (input, _) = handlers::bound_step(view, &data.step, &EscalationStep::ReplaceCheck)?;
    handlers::fresh(
        view,
        input,
        &call.end.clone().into_iter().collect::<Vec<_>>(),
        false,
    )?;
    let assignment = &call.dispatch.assignment;
    let admitted = &view.admission().assignments()[&assignment.id];
    let mut original = admitted.intent.assignment.clone();
    original.state = assignment.state;
    let account = &view.treasury().unwrap().accounts[&call.dispatch.reservation];
    let result = view
        .results()
        .results()
        .values()
        .find(|r| r.reference().is_ok_and(|r| r == data.result))
        .unwrap();
    let contribution = &view.coordination().contributions()[&assignment.contribution];
    let old = view
        .checks()
        .values()
        .find(|c| c.reference() == data.old)
        .unwrap();
    let author = if let CheckAuthor::Agent(id) = &old.author {
        Some(&view.admission().assignments()[id].intent.assignment.agent)
    } else {
        None
    };
    if original != *assignment
        || assignment.role != RoleKind::Reviewer
        || assignment.agent == result.producer
        || author == Some(&assignment.agent)
        || contribution.value.kind != ContributionKind::Review
        || contribution.value.subject
            != Some(ContributionSubject::ResultVersion(data.result.clone()))
        || !contribution.value.basis.contains(reference)
        || contribution.contract != data.contract
        || call.dispatch.prompt != prompt(view, reference)?
        || call.terminal != Some(InvocationTerminal::Completed)
        || !call.confirmed_terminal
        || !crate::execution::closed(view, &assignment.id)
        || crate::execution::limit_reason(view, call, call.ended_at.unwrap_or(0))?.is_some()
        || account.reservation.state != ReservationState::Settled
        || account.settlement.as_ref().map(|s| &s.receipt) != call.receipt.as_ref()
        || call.receipt.is_none()
    {
        return Err(Denial::new(
            "replacement_review",
            "Only the exact paid independent Reviewer output approves this replacement",
        ));
    }
    let verdict: ReplacementVerdict = decode(call.output.as_bytes())?;
    ymp_domain::require_text(&verdict.rationale, 4096)?;
    if verdict.proposal != *reference
        || verdict.old != data.old
        || verdict.new != data.new.reference()
        || verdict.criterion != data.criterion
        || verdict.result != data.result
        || !verdict.approve
    {
        return Err(Denial::new(
            "replacement_approval",
            "Reviewer did not approve the exact full replacement",
        ));
    }
    Ok(ReplacementApproval {
        invocation: invocation.clone(),
        assignment: assignment.reference()?,
        admission: call.dispatch.admission.clone(),
        completion: call.end.clone().unwrap(),
        receipt: account.last.clone(),
        verdict,
    })
}
pub(super) fn validate_replaced(
    view: &SessionView,
    reference: &Ref,
    data: &ReplacementApproval,
) -> Result<()> {
    if *data != approval(view, reference, &data.invocation)? {
        return Err(Denial::new(
            "replacement_attribution",
            "Replacement approval differs from its retained paid source",
        ));
    }
    Ok(())
}
pub(crate) fn validate_admission(
    view: &SessionView,
    intent: &crate::gatekeeper::AdmissionIntent,
) -> Result<()> {
    let contribution = &view.coordination().contributions()[&intent.assignment.contribution].value;
    for reference in &contribution.basis {
        if let Ok(data) = proposal(view, reference) {
            let (_, plan) = handlers::step(view, &data.step, &EscalationStep::ReplaceCheck)?;
            if intent.assignment.role != RoleKind::Reviewer
                || contribution.subject
                    != Some(ContributionSubject::ResultVersion(data.result.clone()))
                || intent.assignment.allowance.cost > plan.limit.max_cost
                || intent.assignment.allowance.timeout > plan.limit.timeout_ms
                || view.admission().assignments().values().any(|a| {
                    view.coordination().contributions()[&a.intent.assignment.contribution]
                        .value
                        .basis
                        .contains(reference)
                })
            {
                return Err(Denial::new(
                    "replacement_review_limit",
                    "Exact replacement review is one bounded independent invocation",
                ));
            }
        }
    }
    Ok(())
}
impl<J: Journal> Progress<J> {
    pub fn propose_replacement(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        data: ReplacementProposal,
    ) -> Result<Ref> {
        self.append(
            owner,
            expected,
            at,
            ProgressRecorded::Replacement(Box::new(data)),
        )
    }
    pub fn replacement_prompt(&self, session: &Id, reference: &Ref) -> Result<Prompt> {
        prompt(&self.journal.view(session, None)?, reference)
    }
    pub fn replace_check(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        reference: &Ref,
        invocation: &Id<Invocation>,
    ) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        let view = self.journal.view(&session, None)?;
        let approval = approval(&view, reference, invocation)?;
        self.append(
            owner,
            expected,
            at,
            ProgressRecorded::Replaced {
                proposal: reference.clone(),
                approval: Box::new(approval),
            },
        )
    }
}
pub(crate) fn replacement_value(
    view: &SessionView,
    reference: &Ref,
) -> Result<(Check, ymp_domain::task::AcceptanceContract)> {
    let data = proposal(view, reference)?;
    let mut checks = view.contract().unwrap().checks.clone();
    checks.retain(|id| *id != data.old.id);
    checks.push(data.new.id.erased());
    Ok((
        data.new.clone(),
        ymp_domain::task::AcceptanceContract::new(view.task().unwrap(), view.criteria(), checks)?,
    ))
}

use super::*;
use ymp_domain::{
    Ref,
    result::{Artifact, ResultVersion},
};
use ymp_kernel::ledger::LedgerRequest;
impl<J: Journal + 'static, C: ContentStore + 'static> Dispatcher<J, C> {
    pub(super) fn work(&mut self) -> Result<Tick> {
        let view = self.view()?;
        let item = view
            .results()
            .items()
            .values()
            .next()
            .ok_or_else(|| Denial::new("work_item_missing", "No committed work"))?
            .clone();
        if !ymp_kernel::results::plan_current(&view, &item.plan)? {
            return Err(Denial::new(
                "plan_stale",
                "Recorded task or criterion scope changed; graph revision requires its owning future handler",
            ));
        }
        let result = item
            .attempts
            .last()
            .and_then(|a| view.results().attempts()[a].attempt.result.as_ref())
            .and_then(|r| view.results().results().get(r));
        if let Some(result) = result {
            return self.candidate(result.clone());
        }
        if let Some(attempt) = item
            .attempts
            .last()
            .filter(|a| view.results().attempts()[*a].attempt.outcome == AttemptOutcome::Pending)
        {
            let attempt = &view.results().attempts()[attempt];
            let name = attempt.attempt.assignment.as_str();
            if self.completed(name)? {
                let after = &view.snapshots()[&attempt.after];
                let artifacts = after
                    .tree
                    .files
                    .iter()
                    .filter(|(p, _)| item.writes.iter().any(|w| w.contains(p)))
                    .map(|(p, f)| Artifact {
                        path: p.clone(),
                        digest: f.digest.clone(),
                    })
                    .collect();
                let prepared =
                    self.app
                        .recover_attempt(&self.owner, &self.results, &attempt.attempt.id)?;
                let invocation = &view.execution().invocations()[&id(&format!("call-{name}"))?];
                self.results.submit(
                    &prepared,
                    self.at()?,
                    id(&format!("result-{name}"))?,
                    artifacts,
                    invocation.output.chars().take(4096).collect(),
                )?;
                return Ok(Tick::Advanced);
            }
        }
        self.verification_capacity(None)?;
        let (_, stalled) = self.monitor()?;
        if stalled {
            return Err(Denial::new(
                "no_useful_work",
                "Progress monitor reached the selected stall bound",
            ));
        }
        let view = self.view()?;
        let contribution = view.coordination().contributions().values().find(|c| {
            c.value.kind == ContributionKind::Produce
                && !view
                    .admission()
                    .assignments()
                    .values()
                    .any(|a| a.intent.assignment.contribution == c.value.id)
                && c.value.subject.as_ref()
                    == Some(&ContributionSubject::WorkItem(item.reference().unwrap()))
        });
        if let Some(c) = contribution {
            let paths = std::iter::once((WorkspacePath::root(), LockMode::Read))
                .chain(item.writes.iter().cloned().map(|p| (p, LockMode::Write)))
                .collect();
            return self.launch(
                c.value.id.as_str(),
                c.value.kind,
                RoleKind::Producer,
                c.value.subject.clone(),
                c.value.needs.clone(),
                paths,
                None,
            );
        }
        let plans = self.app.plans();
        let input = plans.contributions(self.owner.session())?;
        let proposal = self.policies.contributions.next(&input)?;
        if proposal.value.is_empty() {
            return Err(Denial::new(
                "no_useful_work",
                "No currently feasible work proposal",
            ));
        }
        plans.record_contributions(&self.owner, view.revision(), self.at()?, input, proposal)?;
        Ok(Tick::Advanced)
    }
    pub(super) fn candidate_context(&self, result: &ResultVersion) -> Result<ApplicabilityContext> {
        let view = self.view()?;
        let targets = &view.results().items()[&result.item].targets;
        let env = Digest::of_value(&self.runner.environment()?)?;
        Ok(ApplicabilityContext {
            result: result.reference()?,
            criteria: view
                .criteria()
                .iter()
                .filter(|k| targets.contains(&k.id))
                .map(|k| k.reference())
                .collect::<Result<_>>()?,
            environments: view
                .checks()
                .values()
                .filter(|c| {
                    view.contract().unwrap().checks.contains(&c.id.erased())
                        && targets.contains(&c.criterion)
                })
                .map(|c| (c.reference(), BTreeSet::from([env.clone()])))
                .collect(),
        })
    }
    pub(super) fn assess(&self, context: ApplicabilityContext) -> Result<()> {
        let authority = self.app.acceptance(self.content.clone());
        let view = self.view()?;
        let at = self.at()?;
        let rules = self.definition()?.rules;
        let responses = authority
            .ledger_inputs(self.owner.session(), &context, &rules, at)?
            .into_iter()
            .map(|(k, v)| {
                Ok((
                    k,
                    BeliefResponse {
                        input: Digest::of_value(&v)?,
                        proposal: self.policies.belief.update(&v)?,
                    },
                ))
            })
            .collect::<Result<_>>()?;
        authority.record_ledger(
            &self.owner,
            LedgerRequest {
                expected_revision: view.revision(),
                at,
                context,
                rules,
                responses,
            },
            None,
        )?;
        Ok(())
    }
    fn candidate(&mut self, result: ResultVersion) -> Result<Tick> {
        let view = self.view()?;
        let authority = self.app.acceptance(self.content.clone());
        let context = self.candidate_context(&result)?;
        let verification = view.coordination().contributions().values().find(|c| {
            c.value.kind == ContributionKind::Verify
                && c.value.subject.as_ref()
                    == Some(&ContributionSubject::ResultVersion(context.result.clone()))
        });
        if let Some(c) = verification {
            if !self.completed(c.value.id.as_str())? {
                self.verification_capacity(Some(&context))?;
                return self.launch(
                    c.value.id.as_str(),
                    ContributionKind::Verify,
                    RoleKind::Verifier,
                    c.value.subject.clone(),
                    c.value.needs.clone(),
                    vec![(WorkspacePath::root(), LockMode::Read)],
                    Some(&result.producer),
                );
            }
        } else {
            self.verification_capacity(Some(&context))?;
            let plans = self.app.plans();
            let input = plans.contributions(self.owner.session())?;
            let proposal = self.policies.contributions.next(&input)?;
            if proposal.value.is_empty() {
                return Err(Denial::new(
                    "verification_unavailable",
                    "No feasible verification work within the remaining resources",
                ));
            }
            plans.record_contributions(
                &self.owner,
                view.revision(),
                self.at()?,
                input,
                proposal,
            )?;
            return Ok(Tick::Advanced);
        }
        for check_ref in context.environments.keys() {
            let check = view
                .checks()
                .values()
                .find(|c| c.reference() == *check_ref)
                .unwrap();
            let mut runs = vec![];
            for (role, target, label) in [
                (CheckRunRole::Baseline, &result.before, "base"),
                (CheckRunRole::Candidate, &result.after, "candidate"),
            ] {
                let run_id = id(&format!("run-{}-{}-{label}", result.id, check.id))?;
                if !view.check_runs().contains_key(&run_id) {
                    authority.run(
                        &self.owner,
                        RunCheck {
                            expected_revision: view.revision(),
                            at: self.at()?,
                            id: run_id,
                            check: check_ref.clone(),
                            target: target.clone(),
                            role,
                        },
                        self.runner.as_ref(),
                    )?;
                    return Ok(Tick::Advanced);
                }
                runs.push(run_id);
            }
            let evidence_id = id(&format!("evidence-{}-{}", result.id, check.id))?;
            if !view.evidence().contains_key(&evidence_id) {
                if matches!(
                    view.check_runs()[runs.last().unwrap()].outcome,
                    CheckOutcome::Error(_)
                ) {
                    return Err(Denial::new(
                        "check_environment",
                        "Candidate check execution failed",
                    ));
                }
                authority.evidence(
                    &self.owner,
                    EvidenceRequest {
                        expected_revision: view.revision(),
                        at: self.at()?,
                        id: evidence_id,
                        criterion: Ref {
                            id: check.criterion.erased(),
                            version: check.criterion_version.clone(),
                        },
                        result: result.reference()?,
                        runs,
                        reviews: vec![],
                    },
                )?;
                return Ok(Tick::Advanced);
            }
        }
        let finalizer = self.app.finalization(self.content.clone());
        if !view.finalization().history.iter().any(|(_,e)|matches!(e,ymp_kernel::finalization::FinalizationRecorded::CandidateReviewer(c) if c.input.subject==context.result)) {
            let input=finalizer.candidate_reviewer_input(self.owner.session(),&context.result)?;
            let proposal=self.policies.reviewer.pick(&input)?;
            let missing=proposal.value.is_none();
            finalizer.record_candidate_reviewer(&self.owner,view.revision(),self.at()?,input,proposal)?;
            if missing{return Err(Denial::new("candidate_review_pending","No eligible independent reviewer"));}
            return Ok(Tick::Advanced);
        }
        if !view
            .reviews()
            .values()
            .any(|r| r.review.result == result.id)
        {
            let name = format!("review-{}", result.id);
            if self.completed(&name)? {
                authority.consume_review(
                    &self.owner,
                    view.revision(),
                    self.at()?,
                    &id(&format!("call-{name}"))?,
                )?;
                return Ok(Tick::Advanced);
            }
            return self.launch(
                &name,
                ContributionKind::Review,
                RoleKind::Reviewer,
                Some(ContributionSubject::ResultVersion(result.reference()?)),
                BTreeSet::from([Capability::ReadFiles]),
                vec![(WorkspacePath::root(), LockMode::Read)],
                Some(&result.producer),
            );
        }
        if !view
            .ledger_state()
            .assessments()
            .iter()
            .any(|(_, r)| r.context == context)
        {
            self.assess(context)?;
            return Ok(Tick::Advanced);
        }
        let accepted_id = id(&format!("accepted-{}", result.id))?;
        if !view.acceptances().contains_key(&accepted_id) {
            let rules = self.definition()?.rules;
            let at = self.at()?;
            let acceptance = authority.acceptance_value(
                self.owner.session(),
                accepted_id.clone(),
                &context,
                &rules,
                at,
            )?;
            authority.accept(
                &self.owner,
                AcceptanceRequest {
                    expected_revision: view.revision(),
                    at,
                    id: accepted_id,
                    context,
                    rules,
                    credit: CreditResponse {
                        input: credit_input(&acceptance)?,
                        proposal: self.policies.credit.creditable(acceptance.grade)?,
                    },
                },
                None,
            )?;
            return Ok(Tick::Advanced);
        }
        let accepted = &view.acceptances()[&accepted_id];
        let reference = accepted.acceptance.reference()?;
        if accepted.acceptance.decision != AcceptanceDecision::Accepted {
            return Err(Denial::new(
                "candidate_rejected",
                "Applicable evidence or review rejected the candidate",
            ));
        }
        let attempt = view
            .results()
            .attempts()
            .values()
            .find(|a| a.attempt.result.as_ref() == Some(&result.id))
            .unwrap();
        let admission = &view.admission().assignments()[&attempt.attempt.assignment];
        if view.coordination().commitments()[&admission.intent.assignment.commitment].state
            == CommitmentState::Expired
        {
            return Err(Denial::new(
                "commitment_expired",
                "Accepted work is retained but its production lease expired before discharge",
            ));
        }
        if view.coordination().commitments()[&admission.intent.assignment.commitment].state
            != CommitmentState::Discharged
        {
            Arbiter::new(self.journal.clone()).discharge(
                self.admission.gatekeeper().as_ref(),
                self.owner.session(),
                view.revision(),
                self.at()?,
                &admission.intent.assignment.commitment,
                &reference,
            )?;
            return Ok(Tick::Advanced);
        }
        let ledger = authority.ledger(self.owner.session())?;
        if !view
            .criteria()
            .iter()
            .filter(|c| c.required)
            .all(|c| ledger.entries[&c.id].status == LedgerStatus::Satisfied)
        {
            self.monitor()?;
            // One immutable accepted version has completed this fixed baseline. Do not
            // repeat identical paid inspection as an invented source of new evidence.
            return Err(Denial::new(
                "no_useful_work",
                "Accepted candidate retains unmet criteria; additional evidence classes or graph revisions are unavailable in this fixed workflow",
            ));
        }
        self.finish()
    }
}

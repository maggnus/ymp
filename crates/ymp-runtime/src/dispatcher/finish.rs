use super::*;
use crate::policies::narrative::DeterministicReport;
use ymp_domain::report::{ClaimAudit, FinalCheck};
use ymp_kernel::finalization::{Continuation, FinalizationRecorded};
impl<J: Journal + 'static, C: ContentStore + 'static> Dispatcher<J, C> {
    pub(super) fn finish(&mut self) -> Result<Tick> {
        let view = self.view()?;
        let finalizer = self.app.finalization(self.content.clone());
        if view.treasury().unwrap().reporting_mode.is_some() || view.owner_stopped() {
            return self.report();
        }
        if view.session_state().phase == Some(ymp_domain::task::SessionStatus::Running) {
            self.app.phase(
                &self.owner,
                self.at()?,
                ymp_domain::task::SessionStatus::Finalizing,
            )?;
            return Ok(Tick::Advanced);
        }
        if view.finalization().control != Some(Continuation::Continue) {
            finalizer.control(
                &self.owner,
                view.revision(),
                self.at()?,
                Continuation::Continue,
            )?;
            return Ok(Tick::Advanced);
        }
        if view.finalization().aggregate.is_none() {
            let checks = view
                .checks()
                .values()
                .filter(|c| view.contract().unwrap().checks.contains(&c.id.erased()))
                .map(|c| {
                    Ok(FinalCheck {
                        check: c.reference(),
                        environment: self.runner.environment()?,
                    })
                })
                .collect::<Result<_>>()?;
            if finalizer
                .capture(
                    &self.owner,
                    view.revision(),
                    self.at()?,
                    self.definition()?.workspace,
                    id("integrated")?,
                    checks,
                    self.provider.as_ref(),
                )?
                .is_none()
            {
                return self.report();
            }
            return Ok(Tick::Advanced);
        }
        let aggregate = view.finalization().aggregate.as_ref().unwrap();
        let context = ymp_kernel::finalization::context(aggregate)?;
        let checked = aggregate.checks.iter().all(|scope| {
            view.check_runs().values().any(|r| {
                r.check.erased() == scope.check.id
                    && r.check_version == scope.check.version
                    && r.target.erased() == aggregate.snapshot.id
                    && r.target_version == aggregate.snapshot.version
                    && r.role == CheckRunRole::Candidate
                    && Digest::of_value(&scope.environment).as_ref() == Ok(&r.env)
                    && view.evidence().values().any(|e| {
                        e.scope.result == context.result && e.evidence.runs.contains(&r.id)
                    })
            })
        });
        if !checked {
            let runs = finalizer.recheck(&self.owner, self.at()?, self.runner.as_ref(), true)?;
            if runs.iter().any(|r| {
                r.role == CheckRunRole::Candidate && matches!(r.outcome, CheckOutcome::Error(_))
            }) {
                return Err(Denial::new(
                    "check_environment",
                    "Final check execution failed",
                ));
            }
            return Ok(Tick::Advanced);
        }
        if !view
            .finalization()
            .history
            .iter()
            .any(|(_, r)| matches!(r, FinalizationRecorded::Reviewer(_)))
        {
            let input = finalizer.reviewer_input(self.owner.session())?;
            let proposal = self.policies.reviewer.pick(&input)?;
            let missing = proposal.value.is_none();
            finalizer.record_reviewer(&self.owner, view.revision(), self.at()?, input, proposal)?;
            return if missing {
                self.app.phase(
                    &self.owner,
                    self.at()?,
                    ymp_domain::task::SessionStatus::Blocked("final_review_pending".into()),
                )?;
                self.report()
            } else {
                Ok(Tick::Advanced)
            };
        }
        if view.finalization().reviews.is_empty() {
            if self.completed("final-review")? {
                finalizer.record_review(
                    &self.owner,
                    view.revision(),
                    self.at()?,
                    &id("call-final-review")?,
                )?;
                return Ok(Tick::Advanced);
            }
            return self.launch(
                "final-review",
                ContributionKind::Review,
                RoleKind::FinalReviewer,
                None,
                BTreeSet::from([Capability::ReadFiles]),
                vec![(WorkspacePath::root(), LockMode::Read)],
                None,
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
        if view.finalization().acceptance.is_none() {
            let rules = self.definition()?.rules;
            let at = self.at()?;
            let acceptance = finalizer.final_acceptance_value(
                self.owner.session(),
                id("final-accepted")?,
                &rules,
                at,
            )?;
            self.app.acceptance(self.content.clone()).finalize(
                &self.owner,
                AcceptanceRequest {
                    expected_revision: view.revision(),
                    at,
                    id: acceptance.id.clone(),
                    context,
                    rules,
                    credit: CreditResponse {
                        input: credit_input(&acceptance)?,
                        proposal: self.policies.credit.creditable(acceptance.grade)?,
                    },
                },
            )?;
            return Ok(Tick::Advanced);
        }
        self.report()
    }
    pub(super) fn report(&mut self) -> Result<Tick> {
        match self.compose_report() {
            Err(e)
                if matches!(
                    e.code.as_str(),
                    "assignment_unavailable"
                        | "recovery_unresolved"
                        | "narrator_unavailable"
                        | "narration_funds"
                        | "budget"
                        | "funds_unknown"
                        | "decoding"
                        | "narrative_attribution"
                        | "narrative_source"
                ) =>
            {
                self.draining = true;
                self.compose_report()
            }
            result => result,
        }
    }
    fn compose_report(&mut self) -> Result<Tick> {
        let view = self.view()?;
        let finalizer = self.app.finalization(self.content.clone());
        let prepared = view.finalization().history.iter().find_map(|(_, r)| {
            if let FinalizationRecorded::ReportPrepared(p) = r {
                Some(p)
            } else {
                None
            }
        });
        let Some(prepared) = prepared else {
            finalizer.prepare_report(
                &self.owner,
                view.revision(),
                self.at()?,
                id("report")?,
                self.app.treasury(),
                &self.budget,
            )?;
            return Ok(Tick::Advanced);
        };
        let draft = view.finalization().history.iter().rev().find_map(|(r, e)| {
            if let FinalizationRecorded::Narrative(n) = e {
                Some((r, n))
            } else {
                None
            }
        });
        let audit = view.finalization().history.iter().rev().find_map(|(_, e)| {
            if let FinalizationRecorded::Audit(a) = e {
                Some(a)
            } else {
                None
            }
        });
        let correction = draft.is_some()
            && audit.is_some_and(|a| {
                a.decisions
                    .iter()
                    .any(|d| matches!(d.outcome, ClaimAudit::Unsupported(_)))
            });
        let corrected=view.finalization().history.iter().any(|(_,r)|matches!(r,FinalizationRecorded::Narrative(n) if n.invocation.as_ref()==Some(&id("call-correction").unwrap())));
        let forced = self.owner.stopped() || view.owner_stopped() || self.draining;
        let needs_draft = (forced
            && draft.is_none_or(|(_, d)| {
                d.decision.effective.policy.implementation != "DeterministicReport"
            }))
            || draft.is_none()
            || (prepared.narrated
                && correction
                && !corrected
                && draft.is_some_and(|(_, n)| n.invocation.is_some()));
        if needs_draft {
            if prepared.narrated
                && !forced
                && view.policies()["NarrativeComposer"].policy.implementation == "Narrator"
            {
                let name = if correction {
                    "correction"
                } else {
                    "narration"
                };
                if self.completed(name)? {
                    let invocation = id(&format!("call-{name}"))?;
                    let input =
                        finalizer.narrative_input(self.owner.session(), Some(&invocation))?;
                    finalizer.record_narrative(
                        &self.owner,
                        view.revision(),
                        self.at()?,
                        input.clone(),
                        Some(invocation),
                        self.policies.narrative.compose(&input)?,
                        None,
                    )?;
                    return Ok(Tick::Advanced);
                }
                if !view.coordination().contributions().contains_key(&id(name)?) {
                    let eligible = self.app.plans().eligible(
                        self.owner.session(),
                        &BTreeSet::from([Capability::ReadFiles]),
                        None,
                    )?;
                    let profile = view
                        .registry()
                        .unwrap()
                        .decisions
                        .iter()
                        .find(|d| {
                            d.outcome == Readiness::Ready && eligible.contains(&d.profile.agent)
                        })
                        .ok_or_else(|| Denial::new("narrator_unavailable", "No eligible narrator"))?
                        .profile
                        .clone();
                    finalizer.narration_work(
                        &self.owner,
                        view.revision(),
                        self.at()?,
                        id(name)?,
                        profile,
                        correction,
                        self.policies.cost.as_ref(),
                    )?;
                    return Ok(Tick::Advanced);
                }
                return self.launch(
                    name,
                    ContributionKind::Narrate,
                    RoleKind::Narrator,
                    None,
                    BTreeSet::from([Capability::ReadFiles]),
                    vec![(WorkspacePath::root(), LockMode::Read)],
                    None,
                );
            }
            let deterministic = DeterministicReport::new()?;
            let input = finalizer.narrative_input(self.owner.session(), None)?;
            finalizer.record_narrative(
                &self.owner,
                view.revision(),
                self.at()?,
                input.clone(),
                None,
                deterministic.compose(&input)?,
                Some(deterministic.selection().clone()),
            )?;
            return Ok(Tick::Advanced);
        }
        if audit.is_none_or(|a| Some(&a.draft) != draft.map(|(r, _)| r)) {
            let inputs = finalizer.claim_inputs(self.owner.session())?;
            let proposals = inputs
                .iter()
                .map(|i| self.policies.audit.audit(i))
                .collect::<Result<_>>()?;
            finalizer.audit(&self.owner, view.revision(), self.at()?, inputs, proposals)?;
            return Ok(Tick::Advanced);
        }
        Ok(Tick::Delivered(Box::new(finalizer.deliver(
            &self.owner,
            view.revision(),
            self.at()?,
        )?)))
    }
}

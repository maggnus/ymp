//! Criterion-directed candidate generation and admission prefilters.
use super::*;
use ymp_domain::{
    identity::{DiscoverySource, ProviderKind, Readiness},
    journal::Capability,
    resources::{CostEstimate, Difficulty},
    verification::LedgerStatus,
};
fn available(
    view: &SessionView,
    needs: &BTreeSet<Capability>,
    producer: Option<&Id<ymp_domain::identity::Agent>>,
) -> BTreeSet<Id<ymp_domain::identity::Agent>> {
    let Some(pool) = view.registry() else {
        return BTreeSet::new();
    };
    if pool.input.constraints != view.task().unwrap().constraints {
        return BTreeSet::new();
    }
    let members = view
        .planning()
        .team
        .as_ref()
        .map(|t| {
            t.members
                .iter()
                .filter(|m| m.left.is_none())
                .map(|m| m.agent.clone())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let busy: BTreeSet<_> = view
        .admission()
        .assignments()
        .values()
        .filter(|a| crate::gatekeeper::unresolved(view, a))
        .map(|a| a.intent.assignment.agent.clone())
        .collect();
    pool.decisions
        .iter()
        .filter(|d| {
            d.outcome == Readiness::Ready
                && members.contains(&d.profile.agent)
                && !busy.contains(&d.profile.agent)
                && producer != Some(&d.profile.agent)
        })
        .filter_map(|d| {
            let agent = pool
                .input
                .facts
                .agents
                .iter()
                .find(|a| a.id == d.profile.agent)?;
            let provider = pool
                .input
                .facts
                .discoveries
                .iter()
                .find(|p| p.provider.id == agent.provider)?;
            (provider.provider.kind == ProviderKind::Scripted
                && provider.source == DiscoverySource::ScriptedFixture
                && provider
                    .provider
                    .capabilities
                    .as_ref()
                    .is_some_and(|caps| needs.is_subset(caps))
                && needs.is_subset(&view.task().unwrap().constraints.allowed)
                && needs
                    .iter()
                    .all(|c| matches!(c, Capability::ReadFiles | Capability::WriteFiles))
                && (needs.is_empty()
                    || view.workspaces().values().any(|w| {
                        w.provider.policy.implementation == "Direct"
                            && view.workspace_bindings().contains_key(&w.id)
                    })))
            .then(|| d.profile.agent.clone())
        })
        .collect()
}
fn path_conflict(
    view: &SessionView,
    writes: &BTreeSet<ymp_domain::workspace::WorkspacePath>,
) -> bool {
    view.path_locks()
        .values()
        .filter(|l| l.released.is_none())
        .any(|l| {
            l.acquired
                .effective
                .iter()
                .chain(l.file_holds.values())
                .any(|held| writes.iter().any(|p| p.overlaps(&held.lock.path)))
        })
        || view
            .capture_reads()
            .values()
            .filter(|c| c.ended.is_none())
            .any(|c| writes.iter().any(|p| p.overlaps(&c.observation.path)))
}
#[allow(clippy::too_many_arguments)]
fn candidate(
    view: &SessionView,
    kind: ContributionKind,
    targets: BTreeSet<Id<Criterion>>,
    subject: Option<ContributionSubject>,
    needs: BTreeSet<Capability>,
    producer: Option<&Id<ymp_domain::identity::Agent>>,
    status: LedgerStatus,
    ordinal: usize,
    parameters: &ContributionParameters,
    selection: &PolicySelection,
) -> Result<Option<ContributionCandidate>> {
    let eligible = available(view, &needs, producer);
    let Some(book) = view.treasury() else {
        return Ok(None);
    };
    if eligible.is_empty() || parameters.p90 > book.remaining(kind.purpose())? {
        return Ok(None);
    }
    let required = view
        .criteria()
        .iter()
        .any(|c| c.required && targets.contains(&c.id));
    let contribution = Contribution {
        id: Id::new(format!("next-{}-{ordinal}", view.revision()))?,
        session: view.session().clone(),
        kind,
        targets: targets.clone(),
        subject,
        needs,
        forecast: Forecast {
            p_success: parameters.p_success,
            delta_belief: targets
                .iter()
                .map(|id| (id.clone(), parameters.delta_belief))
                .collect(),
            source: ForecastSource::Model(selection.policy.clone()),
        },
        cost: CostEstimate {
            expected: parameters.expected,
            p90: parameters.p90,
        },
        difficulty: Difficulty::Simple,
        proposed_by: ContributionAuthor::Runtime,
        basis: vec![view.contract().unwrap().reference()],
    };
    contribution.validate()?;
    Ok(Some(ContributionCandidate {
        contribution,
        eligible,
        required,
        status,
    }))
}
pub fn contribution_input(view: &SessionView) -> Result<ContributionView> {
    let task = view
        .task()
        .ok_or_else(|| Denial::new("task_missing", "Next work requires an explicit task"))?;
    let treasury = view
        .treasury()
        .ok_or_else(|| Denial::new("budget_missing", "Next work requires a recorded budget"))?;
    let selection = view
        .policies()
        .get("ContributionPolicy")
        .ok_or_else(|| Denial::new("policy_selection", "No ContributionPolicy selected"))?;
    let parameters: ContributionParameters = decode(&encode(&selection.parameters)?)?;
    let ledger = crate::ledger::current(view, view.latest_at())?;
    let running = view
        .admission()
        .assignments()
        .values()
        .filter(|a| crate::gatekeeper::unresolved(view, a))
        .count();
    let slots = (task.constraints.parallel_limit as usize).saturating_sub(running);
    let mut input=ContributionView{journal:view.digest()?,candidates:vec![],slots,production_remaining:treasury.remaining(Purpose::Production)?,verification_remaining:treasury.remaining(Purpose::Verification)?,limitations:vec!["Forecast cost and criterion benefit are explicit uncalibrated experiment parameters".into(),"Path prefilters cover this session; Gatekeeper repeats physical and cross-session ownership checks at admission".into(),"Budget prefilters follow selected order; every admission rechecks actual remaining funds and holds".into()]};
    if view.planning().team.is_none() {
        input
            .limitations
            .push("Select an eligible initial team before proposing execution".into());
        return Ok(input);
    }
    for item in view.results().items().values() {
        if !crate::results::plan_current(view, &item.plan)?
            || item
                .targets
                .iter()
                .any(|id| !ledger.entries.contains_key(id))
        {
            input.limitations.push(
                "The plan has a stale criterion contract; paid replanning is unavailable until W3"
                    .into(),
            );
            continue;
        }
        if matches!(
            item.state,
            WorkState::Committed | WorkState::Running | WorkState::Superseded | WorkState::Blocked
        ) {
            continue;
        }
        if item.deps.iter().any(|id| {
            view.results().items().get(id).is_none_or(|d| {
                d.state != WorkState::Accepted
                    || d.accepted
                        .as_ref()
                        .is_none_or(|r| !view.results().results().contains_key(r))
            })
        }) {
            input
                .limitations
                .push(format!("{} awaits accepted prerequisites", item.id));
            continue;
        }
        let status = if item
            .targets
            .iter()
            .any(|k| ledger.entries[k].status == LedgerStatus::Contradicted)
        {
            LedgerStatus::Contradicted
        } else if item
            .targets
            .iter()
            .any(|k| ledger.entries[k].status == LedgerStatus::Unmet)
        {
            LedgerStatus::Unmet
        } else if item
            .targets
            .iter()
            .any(|k| ledger.entries[k].status == LedgerStatus::Supported)
        {
            LedgerStatus::Supported
        } else {
            continue;
        };
        let result = item
            .accepted
            .as_ref()
            .or_else(|| {
                item.attempts
                    .last()
                    .and_then(|id| view.results().attempts().get(id))
                    .filter(|a| a.attempt.outcome != AttemptOutcome::Abandoned)
                    .and_then(|a| a.attempt.result.as_ref())
            })
            .and_then(|id| view.results().results().get(id));
        let (kind, subject, mut needs, producer) = if let Some(result) = result {
            let kind = if status == LedgerStatus::Contradicted {
                ContributionKind::Diagnose
            } else if view.checks().values().any(|c| {
                item.targets.contains(&c.criterion)
                    && view.contract().unwrap().checks.contains(&c.id.erased())
                    && view.criteria().iter().any(|criterion| {
                        criterion.id == c.criterion
                            && criterion
                                .reference()
                                .is_ok_and(|r| r.version == c.criterion_version)
                    })
            }) {
                ContributionKind::Verify
            } else {
                ContributionKind::DesignChecks
            };
            if kind == ContributionKind::Diagnose {
                input.limitations.push("Diagnosis selection is available; the FailureDiagnoser consumer is a later task".into());
            }
            (
                kind,
                Some(ContributionSubject::ResultVersion(result.reference()?)),
                BTreeSet::from([Capability::ReadFiles]),
                (kind == ContributionKind::Verify).then_some(&result.producer),
            )
        } else {
            if view.results().plans()[&item.plan].contract != view.contract().unwrap().reference() {
                input.limitations.push("Production still requires its original plan contract; changing checks before production needs a new supported plan boundary".into());
                continue;
            }
            if path_conflict(view, &item.writes) {
                input.limitations.push(format!(
                    "{} conflicts with unresolved path ownership",
                    item.id
                ));
                continue;
            }
            (
                ContributionKind::Produce,
                Some(ContributionSubject::WorkItem(item.reference()?)),
                item.needs.clone(),
                None,
            )
        };
        if kind == ContributionKind::Verify {
            for criterion in view
                .criteria()
                .iter()
                .filter(|c| item.targets.contains(&c.id))
            {
                if criterion.needs_class.contains(&EvidenceClass::Executed) {
                    needs.insert(Capability::RunProcess);
                }
                if criterion.needs_class.contains(&EvidenceClass::Browser) {
                    needs.insert(Capability::Browser);
                }
            }
        }
        let candidate = candidate(
            view,
            kind,
            item.targets.clone(),
            subject,
            needs,
            producer,
            status,
            input.candidates.len(),
            &parameters,
            selection,
        )?;
        match candidate{Some(c)=>input.candidates.push(c),None=>input.limitations.push(format!("{:?} for {} has no independent capable member or purpose budget; unsupported native/runner capabilities remain unavailable",kind,item.id))}
    }
    let unanswered=view.planning().history.iter().filter_map(|(_,r)|if let PlanningRecorded::Intake(d)=r{Some(&d.decision.outcome.questions)}else{None}).flatten().any(|q|matches!(q,QuestionDecision::Ask(question) if !view.task().unwrap().goal.clarifications.iter().any(|c|c.question==question.question)));
    if unanswered {
        input.limitations.push("P6 questions are recorded; interactive ask/resume and Research output consumption arrive in W3".into());
        for kind in [ContributionKind::Clarify, ContributionKind::Research] {
            if let Some(c) = candidate(
                view,
                kind,
                view.criteria().iter().map(|c| c.id.clone()).collect(),
                None,
                BTreeSet::from([Capability::ReadFiles]),
                None,
                LedgerStatus::Supported,
                input.candidates.len(),
                &parameters,
                selection,
            )? {
                input.candidates.push(c);
            }
        }
    }
    if view.results().plans().is_empty() {
        input
            .limitations
            .push("A paid Planner must create the initial one-item plan".into());
    }
    Ok(input)
}
pub(super) fn validate(
    view: &SessionView,
    input: &ContributionView,
    decision: &Decision<Vec<Contribution>>,
) -> Result<()> {
    let expected = contribution_input(view)?;
    decision.proposal.validate()?;
    if *input != expected
        || decision.input != Digest::of_value(input)?
        || decision.effective != view.policies()["ContributionPolicy"]
        || decision.proposal.policy != decision.effective.policy
        || decision.proposal.value != decision.outcome
        || decision.selection_change.is_some()
        || decision.outcome.len() > input.slots
    {
        return Err(Denial::new(
            "contribution_decision",
            "Next work differs from the recorded candidate boundary",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut agents = BTreeSet::new();
    let mut total = 0.0;
    for contribution in &decision.outcome {
        let candidate = input
            .candidates
            .iter()
            .find(|c| c.contribution == *contribution)
            .ok_or_else(|| {
                Denial::new(
                    "contribution_filter",
                    "Strategy returned work outside kernel authority or resource filters",
                )
            })?;
        if !ids.insert(&contribution.id) {
            return Err(Denial::new(
                "contribution_duplicate",
                "A contribution is selected twice",
            ));
        }
        let agent = candidate
            .eligible
            .iter()
            .find(|a| !agents.contains(*a))
            .ok_or_else(|| {
                Denial::new(
                    "contribution_slots",
                    "Selected contributions need distinct available members",
                )
            })?;
        agents.insert(agent.clone());
        total += contribution.cost.p90.get();
        let remaining = if contribution.kind.purpose() == Purpose::Verification {
            input.verification_remaining
        } else {
            input.production_remaining
        };
        if total > remaining.get() {
            return Err(Denial::new(
                "contribution_budget",
                "Combined selected work exceeds remaining purpose funding",
            ));
        }
    }
    Ok(())
}
impl<J: Journal> Plans<J> {
    pub fn contributions(&self, session: &Id) -> Result<ContributionView> {
        contribution_input(&self.journal.view(session, None)?)
    }
    pub fn record_contributions(
        &self,
        control: &SessionControl,
        expected: u64,
        at: u64,
        input: ContributionView,
        proposal: Proposal<Vec<Contribution>>,
    ) -> Result<u64> {
        let session = self.owner.authorize(control)?;
        let current = self.journal.read(&session)?;
        let mut view = current.view_with_schemas(&session, None, self.journal.schemas())?;
        let selected = proposal.value.clone();
        let data = PlanningRecorded::Contributions {
            decision: Box::new(Decision {
                outcome: selected.clone(),
                proposal,
                effective: view
                    .policies()
                    .get("ContributionPolicy")
                    .ok_or_else(|| {
                        Denial::new("policy_selection", "No ContributionPolicy selected")
                    })?
                    .clone(),
                input: Digest::of_value(&input)?,
                selection_change: None,
            }),
            input: Box::new(input),
        };
        let event = Envelope {
            seq: expected
                .checked_add(1)
                .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy: data.selection().map(|s| s.policy.clone()),
            input: Some(view.digest()?),
            refs: references(&data),
            payload: Event::PlanningRecorded {
                version: 1,
                data: Box::new(data),
            },
        };
        view.apply(&event, self.journal.schemas())?;
        let mut events = vec![event];
        for contribution in selected {
            let payload = Event::ContributionProposed {
                version: 1,
                contribution: Box::new(contribution),
            };
            let (policy, input, refs) = crate::arbiter::attribution(&view, &payload)?;
            let event = Envelope {
                seq: view.revision().checked_add(1).ok_or_else(|| {
                    Denial::new("revision_overflow", "Journal sequence exhausted")
                })?,
                session: session.clone(),
                at,
                actor: Actor::Runtime,
                policy,
                input,
                refs,
                payload,
            };
            view.apply(&event, self.journal.schemas())?;
            events.push(event);
        }
        validate_append(
            &current,
            &session,
            expected,
            &events,
            self.journal.schemas(),
        )?;
        self.journal.append(&session, expected, &events)
    }
}

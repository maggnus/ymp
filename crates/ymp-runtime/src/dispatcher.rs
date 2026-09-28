//! A1 fixed-workflow composition. Services validate; strategies decide; this loop orders calls.
use crate::{
    admission::{AdmissionCommand, AdmissionPolicies, AdmissionRuntime},
    application::{Application, IntakeRequest, RecoveryIntent},
    clock::Clock,
    execution_host::{ExecutionHost, ExecutionStatus, LiveInvocation},
    policies::resources::{estimate_response, reporting_response},
    readiness::readiness_response,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use ymp_domain::{
    Denial, Digest, Id, Prob, Result,
    assignment::*,
    coordination::*,
    identity::*,
    journal::{Capability, PolicySelection},
    plan::*,
    resources::*,
    verification::*,
    workspace::*,
};
use ymp_kernel::{
    acceptance::*,
    arbiter::{Arbiter, award_view},
    decision::{MethodDecision, SessionControl},
    journal::{ContentStore, Journal},
    plans::{PaidRequest, PlanningRecorded},
    ports::{
        checks::{CheckRunner, ReadinessProbe},
        execution::{ExecutionBackend, WorkspaceProvider},
        experience::CreditPolicy,
        organization::AwardPolicy,
        planning::*,
        progress::*,
        reporting::*,
        resources::*,
    },
    registry::readiness_views,
    session::SessionDefinition,
    treasury::{BudgetControl, BudgetRequest, reporting_view},
    view::SessionView,
};

mod finish;
mod recovery;
mod work;
pub struct SessionPolicies {
    pub readiness: Box<dyn ReadinessProbe>,
    pub intake: Box<dyn IntakePolicy>,
    pub method: Box<dyn MethodRouter>,
    pub planner: Box<dyn Planner>,
    pub contributions: Box<dyn ContributionPolicy>,
    pub belief: Box<dyn BeliefModel>,
    pub credit: Box<dyn CreditPolicy>,
    pub monitor: Box<dyn ProgressMonitor>,
    pub diagnosis: Box<dyn FailureDiagnoser>,
    pub escalation: Box<dyn EscalationPolicy>,
    pub award: Arc<dyn AwardPolicy>,
    pub cost: Arc<dyn CostModel>,
    pub resources: Box<dyn ResourcePolicy>,
    pub context: Box<dyn ContextComposer>,
    pub reviewer: Box<dyn ReviewerPolicy>,
    pub narrative: Box<dyn NarrativeComposer>,
    pub audit: Box<dyn ClaimAuditor>,
}
impl SessionPolicies {
    pub fn selections(&self) -> Vec<PolicySelection> {
        vec![
            self.readiness.selection().clone(),
            self.intake.selection().clone(),
            self.method.selection().clone(),
            self.planner.selection().clone(),
            self.contributions.selection().clone(),
            self.belief.selection().clone(),
            self.credit.selection().clone(),
            self.monitor.selection().clone(),
            self.diagnosis.selection().clone(),
            self.escalation.selection().clone(),
            self.award.selection().clone(),
            self.cost.selection().clone(),
            self.resources.selection().clone(),
            self.context.selection().clone(),
            self.reviewer.selection().clone(),
            self.narrative.selection().clone(),
            self.audit.selection().clone(),
        ]
    }
}
pub struct SessionStart {
    pub session: Id,
    pub task: IntakeRequest,
    pub facts: RegistryFacts,
    pub definition: SessionDefinition,
    pub pricebook: PriceBook,
    pub unknown_usage: UnknownUsage,
}
#[derive(Clone, Debug)]
pub enum Tick {
    Advanced,
    Waiting { until: u64 },
    NeedsUser,
    Observed,
    Delivered(Box<ymp_kernel::finalization::ReportDelivered>),
}
pub struct Dispatcher<J: Journal, C: ContentStore> {
    app: Arc<Application<J>>,
    journal: Arc<J>,
    content: Arc<C>,
    admission: AdmissionRuntime<J, C>,
    results: ymp_kernel::results::Results<J, C>,
    owner: Arc<SessionControl>,
    budget: BudgetControl,
    host: ExecutionHost<J, C>,
    provider: Arc<dyn WorkspaceProvider>,
    runner: Arc<dyn CheckRunner>,
    clock: Arc<dyn Clock>,
    policies: SessionPolicies,
    live: Option<LiveInvocation<J>>,
    observed: bool,
    draining: bool,
}
fn id<T>(name: &str) -> Result<Id<T>> {
    if name.len() > 128 {
        Id::new(format!("derived-{}", Digest::of(name)))
    } else {
        Id::new(name)
    }
}
impl<J: Journal + 'static, C: ContentStore + 'static> Dispatcher<J, C> {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        journal: Arc<J>,
        content: Arc<C>,
        request: SessionStart,
        policies: SessionPolicies,
        backend: Arc<dyn ExecutionBackend>,
        provider: Arc<dyn WorkspaceProvider>,
        runner: Arc<dyn CheckRunner>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self> {
        let app = Arc::new(Application::new(journal.clone()));
        let at = clock.now()?;
        let mut selections = policies.selections();
        selections.extend([
            backend.selection().clone(),
            provider.selection().clone(),
            runner.environment()?.runner,
            PolicySelection::new(
                "VerificationDesigner",
                "ExplicitVisible",
                "1",
                serde_json::json!({}),
            )?,
        ]);
        let owner = app.open(request.session.clone(), at, request.task, selections)?;
        let input = app.registry_input(&request.session, request.facts, at)?;
        let responses = readiness_views(&input)
            .iter()
            .map(|v| readiness_response(policies.readiness.as_ref(), v))
            .collect::<Result<Vec<_>>>()?;
        let view = app.view(&request.session, None)?;
        app.record_pool(
            &request.session,
            view.revision(),
            at,
            input,
            policies.readiness.selection().clone(),
            responses,
        )?;
        let admission = app.admission(content.clone());
        let view = app.view(&request.session, None)?;
        admission.gatekeeper().workspace().open(
            &request.session,
            view.revision(),
            at,
            request.definition.workspace.clone(),
            provider.as_ref(),
        )?;
        let view = app.view(&request.session, None)?;
        admission.gatekeeper().workspace().bind_workspace(
            &request.session,
            view.revision(),
            at,
            &request.definition.workspace,
            provider.as_ref(),
        )?;
        let view = app.view(&request.session, None)?;
        app.treasury().open(
            &request.session,
            view.revision(),
            at,
            BudgetRequest {
                id: id("budget")?,
                pricebook: request.pricebook,
                unknown_usage: request.unknown_usage,
                reporting: reporting_response(
                    policies.resources.as_ref(),
                    &reporting_view(&view, at)?,
                )?,
            },
        )?;
        app.configure(&owner, at, request.definition)?;
        Self::assemble(
            app,
            journal,
            content,
            admission,
            RecoveryIntent::Continue,
            policies,
            backend,
            provider,
            runner,
            clock,
            request.session,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn recover(
        journal: Arc<J>,
        content: Arc<C>,
        session: Id,
        intent: RecoveryIntent,
        policies: SessionPolicies,
        backend: Arc<dyn ExecutionBackend>,
        provider: Arc<dyn WorkspaceProvider>,
        runner: Arc<dyn CheckRunner>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self> {
        let app = Arc::new(Application::new(journal.clone()));
        let admission = app.admission(content.clone());
        Self::assemble(
            app, journal, content, admission, intent, policies, backend, provider, runner, clock,
            session,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn assemble(
        app: Arc<Application<J>>,
        journal: Arc<J>,
        content: Arc<C>,
        admission: AdmissionRuntime<J, C>,
        intent: RecoveryIntent,
        policies: SessionPolicies,
        backend: Arc<dyn ExecutionBackend>,
        provider: Arc<dyn WorkspaceProvider>,
        runner: Arc<dyn CheckRunner>,
        clock: Arc<dyn Clock>,
        session: Id,
    ) -> Result<Self> {
        let observed = matches!(intent, RecoveryIntent::Observe);
        let report_only = matches!(intent, RecoveryIntent::Report);
        let recovered = app.recover(&session, clock.now()?, intent)?;
        let owner = Arc::new(recovered.control);
        if report_only {
            let view = app.view(&session, None)?;
            if view.finalization().delivered.is_none()
                && view.finalization().control
                    != Some(ymp_kernel::finalization::Continuation::NoAuthority)
            {
                app.finalization(content.clone()).control(
                    &owner,
                    view.revision(),
                    clock.now()?.max(view.latest_at()),
                    ymp_kernel::finalization::Continuation::NoAuthority,
                )?;
            }
        }
        if !observed && !report_only {
            let view = app.view(&session, None)?;
            if view.finalization().control
                == Some(ymp_kernel::finalization::Continuation::NoAuthority)
            {
                app.finalization(content.clone()).control(
                    &owner,
                    view.revision(),
                    clock.now()?.max(view.latest_at()),
                    ymp_kernel::finalization::Continuation::Continue,
                )?;
            }
        }
        let budget = recovered.budget.ok_or_else(|| {
            Denial::new(
                "budget_missing",
                "Session recovery never opens a replacement budget",
            )
        })?;
        let view = app.view(&session, None)?;
        let definition = view.session_state().definition.as_ref().ok_or_else(|| {
            Denial::new(
                "session_definition",
                "Session has no recorded fixed workflow inputs",
            )
        })?;
        if !observed && !report_only {
            provider.verify_binding(
                view.workspace_bindings()
                    .get(&definition.1.workspace)
                    .ok_or_else(|| {
                        Denial::new("workspace_binding", "Session workspace is not bound")
                    })?,
                journal.as_ref(),
            )?;
        }
        for selected in policies.selections() {
            if view.policies().get(&selected.policy.port) != Some(&selected)
                && selected.policy.port != "NarrativeComposer"
            {
                return Err(Denial::new(
                    "policy_selection",
                    "Recovered runtime must use the actual recorded strategies",
                ));
            }
        }
        let results = admission.results()?;
        let host = app.execution(
            &admission,
            owner.clone(),
            backend,
            policies.cost.clone(),
            clock.clone(),
        )?;
        Ok(Self {
            app,
            journal,
            content,
            admission,
            results,
            owner,
            budget,
            host,
            provider,
            runner,
            clock,
            policies,
            live: None,
            observed,
            draining: false,
        })
    }
    pub fn owner(&self) -> Arc<SessionControl> {
        self.owner.clone()
    }
    pub fn application(&self) -> &Arc<Application<J>> {
        &self.app
    }
    pub fn view(&self) -> Result<SessionView> {
        self.app.view(self.owner.session(), None)
    }
    fn at(&self) -> Result<u64> {
        Ok(self.clock.now()?.max(self.view()?.latest_at()))
    }
    fn definition(&self) -> Result<SessionDefinition> {
        Ok(self
            .view()?
            .session_state()
            .definition
            .as_ref()
            .ok_or_else(|| Denial::new("session_definition", "No fixed workflow inputs"))?
            .1
            .clone())
    }
    /// Run to the next user-visible boundary; no board/consequence source is fabricated.
    pub fn run(&mut self) -> Result<Tick> {
        loop {
            match self.tick()? {
                Tick::Advanced => {}
                Tick::Waiting { until } => {
                    let delay = until.saturating_sub(self.clock.now()?).clamp(1, 20);
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
                result => return Ok(result),
            }
        }
    }
    pub fn tick(&mut self) -> Result<Tick> {
        // Local stop and cleanup must run even when journal reads are unavailable.
        if let Some(live) = &mut self.live {
            let status = self.host.poll(live)?;
            Self::reconcile(
                &self.journal,
                self.admission.gatekeeper().as_ref(),
                self.owner.session(),
                self.clock.now()?,
            )?;
            if status == ExecutionStatus::Finished {
                let record = self.host.snapshot(live)?;
                self.live = None;
                if record.terminal != Some(InvocationTerminal::Completed) && !self.owner.stopped() {
                    return self.blocked("work_failed");
                }
                if !self.draining {
                    return Ok(Tick::Advanced);
                }
            } else if let ExecutionStatus::Blocked(reason) = status {
                if !self.owner.stopped()
                    && live.cleanup_pending()
                    && self.clock.now()? < self.host.snapshot(live)?.dispatch.deadline
                {
                    return Ok(Tick::Waiting {
                        until: self.clock.now()?.saturating_add(10),
                    });
                }
                self.host.cancel(live)?;
                self.draining = true;
                if !self.owner.stopped() {
                    return self.blocked(&reason);
                }
            } else if !self.owner.stopped() && !self.draining {
                return Ok(Tick::Waiting {
                    until: self.clock.now()?.saturating_add(10),
                });
            }
        }
        let view = self.view()?;
        if let Some(report) = &view.finalization().delivered {
            return Ok(Tick::Delivered(Box::new(report.clone())));
        }
        if self.observed {
            return Ok(Tick::Observed);
        }
        if Self::reconcile(
            &self.journal,
            self.admission.gatekeeper().as_ref(),
            self.owner.session(),
            self.at()?,
        )? {
            return Ok(Tick::Advanced);
        }
        if !self.owner.stopped()
            && !view.owner_stopped()
            && view.treasury().is_some_and(|b| b.reporting_mode.is_none())
        {
            match self.recovery_step() {
                Ok(Some(next)) => return Ok(next),
                Ok(None) => {}
                Err(error)
                    if matches!(
                        error.code.as_str(),
                        "recovery_unresolved"
                            | "assignment_unavailable"
                            | "recovery_budget"
                            | "retry_limit"
                            | "escalation_stale"
                            | "recovery_context_hidden"
                    ) =>
                {
                    self.draining = true;
                    return self.report();
                }
                Err(error) => return Err(error),
            }
        }
        if self.owner.stopped()
            || view.owner_stopped()
            || view.treasury().is_some_and(|b| b.reporting_mode.is_some())
            || matches!(
                view.session_state().phase,
                Some(ymp_domain::task::SessionStatus::Blocked(_))
            )
        {
            return self.report();
        }
        match self.advance() {
            Err(error)
                if matches!(
                    error.code.as_str(),
                    "decoding"
                        | "criteria_extraction"
                        | "plan_stale"
                        | "plan_definition"
                        | "plan_coverage"
                        | "plan_dependency"
                        | "plan_capability"
                        | "plan_paths"
                        | "artifact_missing"
                        | "recovery_unresolved"
                        | "visible_check_missing"
                        | "no_useful_work"
                        | "assignment_unavailable"
                        | "candidate_review_pending"
                        | "verification_unavailable"
                        | "candidate_rejected"
                        | "commitment_expired"
                        | "check_environment"
                        | "budget"
                        | "budget_exhausted"
                        | "funds_unknown"
                        | "narrator_unavailable"
                        | "narration_funds"
                        | "work_failed"
                ) =>
            {
                self.blocked(&error.code)
            }
            other => other,
        }
    }
    fn advance(&mut self) -> Result<Tick> {
        let view = self.view()?;
        let planning = self.app.plans();
        if !view
            .planning()
            .history
            .iter()
            .any(|(_, record)| matches!(record, PlanningRecorded::Intake(_)))
        {
            if self.completed("intake")? {
                let source = planning.paid_input(
                    self.owner.session(),
                    &id("call-intake")?,
                    "IntakePolicy",
                )?;
                let proposal = self.policies.intake.criteria(&source)?;
                planning.extract(
                    &self.owner,
                    PaidRequest {
                        expected_revision: view.revision(),
                        at: self.at()?,
                        source,
                        proposal,
                    },
                )?;
                return Ok(Tick::Advanced);
            }
            return self.launch(
                "intake",
                ContributionKind::Plan,
                RoleKind::Planner,
                None,
                BTreeSet::from([Capability::ReadFiles]),
                vec![(WorkspacePath::root(), LockMode::Read)],
                None,
            );
        }
        if view.planning().history.iter().any(|(_, record)| {
            if let PlanningRecorded::Intake(data) = record {
                data.decision
                    .outcome
                    .questions
                    .iter()
                    .any(|decision| match decision {
                        QuestionDecision::Ask(q) => !view
                            .task()
                            .unwrap()
                            .goal
                            .clarifications
                            .iter()
                            .any(|answer| answer.question == q.question),
                        _ => false,
                    })
            } else {
                false
            }
        }) {
            return Ok(Tick::NeedsUser);
        }
        let definition = self.definition()?;
        if !view.snapshots().contains_key(&id("session-base")?) {
            self.admission.gatekeeper().workspace().snapshot(
                self.owner.session(),
                view.revision(),
                self.at()?,
                &definition.workspace,
                id("session-base")?,
                self.provider.as_ref(),
            )?;
            return Ok(Tick::Advanced);
        }
        for criterion in view.criteria() {
            if !view.checks().values().any(|check| {
                check.criterion == criterion.id
                    && check.criterion_version == criterion.reference().unwrap().version
            }) {
                let fixed = definition
                    .checks
                    .iter()
                    .find(|c| criterion.reference().is_ok_and(|r| c.criterion == r))
                    .ok_or_else(|| {
                        Denial::new(
                            "visible_check_missing",
                            "No owner-supplied visible check for an extracted criterion",
                        )
                    })?;
                let mut check = Check {
                    id: id(&format!("check-{}", criterion.id))?,
                    criterion: criterion.id.clone(),
                    criterion_version: criterion.reference()?.version,
                    spec: fixed.spec.clone(),
                    author: CheckAuthor::User,
                    independence: Independence::Trusted,
                    visibility: CheckVisibility::Visible,
                    needs: BTreeSet::from([Capability::ReadFiles]),
                    verifier: None,
                    version: Digest::of(b"uncommitted"),
                };
                if matches!(check.spec, CheckSpec::Command { .. }) {
                    check.needs.insert(Capability::RunProcess);
                    check.verifier = Some(view.snapshots()[&id("session-base")?].reference()?);
                }
                check.version = check.content_version()?;
                let selection = view.policies()["VerificationDesigner"].clone();
                self.app.acceptance(self.content.clone()).register_check(
                    &self.owner,
                    RegisterCheck {
                        expected_revision: view.revision(),
                        at: self.at()?,
                        proposal: ymp_domain::Proposal {
                            value: check,
                            rationale: "Explicit fixed visible specification supplied by the owner"
                                .into(),
                            basis: vec![criterion.reference()?],
                            policy: selection.policy.clone(),
                        },
                        effective: selection,
                    },
                )?;
                return Ok(Tick::Advanced);
            }
        }
        if view.method().is_none() {
            planning.method(
                &self.owner,
                MethodDecision {
                    expected_revision: view.revision(),
                    at: self.at()?,
                    input: view.digest()?,
                    proposal: self.policies.method.choose(&view)?,
                },
                None,
            )?;
            return Ok(Tick::Advanced);
        }
        if view.planning().team.is_none() {
            let at = self.at()?;
            let mut members = view
                .registry()
                .unwrap()
                .outcome
                .eligible
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            let pins = &view.task().unwrap().constraints.pins;
            if let Some(roster) = &pins.roster {
                members.retain(|id| roster.contains(&id.erased()));
            }
            let count = pins.team_size.unwrap_or(match view.method().unwrap().kind {
                ymp_domain::journal::MethodKind::Solo => 1,
                _ => 2,
            }) as usize;
            members.truncate(count);
            planning.team(
                &self.owner,
                view.revision(),
                at,
                Team {
                    id: id("team")?,
                    session: self.owner.session().clone(),
                    revision: 1,
                    members: members
                        .into_iter()
                        .map(|agent| Membership {
                            agent,
                            joined: at,
                            left: None,
                            reason: "Initial fixed method with explicit membership".into(),
                        })
                        .collect(),
                },
            )?;
            return Ok(Tick::Advanced);
        }
        if view.results().plans().is_empty() {
            if self.completed("plan")? {
                let source =
                    planning.paid_input(self.owner.session(), &id("call-plan")?, "Planner")?;
                let proposal = self.policies.planner.plan(&source)?;
                planning.plan(
                    &self.owner,
                    PaidRequest {
                        expected_revision: view.revision(),
                        at: self.at()?,
                        source,
                        proposal,
                    },
                )?;
                return Ok(Tick::Advanced);
            }
            return self.launch(
                "plan",
                ContributionKind::Plan,
                RoleKind::Planner,
                None,
                BTreeSet::from([Capability::ReadFiles]),
                vec![(WorkspacePath::root(), LockMode::Read)],
                None,
            );
        }
        if view.session_state().phase == Some(ymp_domain::task::SessionStatus::Intake) {
            self.app.phase(
                &self.owner,
                self.at()?,
                ymp_domain::task::SessionStatus::Running,
            )?;
            return Ok(Tick::Advanced);
        }
        self.work()
    }
    fn completed(&self, name: &str) -> Result<bool> {
        let view = self.view()?;
        match view
            .execution()
            .invocations()
            .get(&id(&format!("call-{name}"))?)
        {
            Some(record)
                if record.terminal == Some(InvocationTerminal::Completed)
                    && record.confirmed_terminal
                    && ymp_kernel::execution::closed(&view, &record.dispatch.assignment.id) =>
            {
                Ok(true)
            }
            Some(_) => Err(Denial::new(
                "recovery_unresolved",
                "An existing invocation cannot be restarted from journal identity",
            )),
            None => Ok(false),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn launch(
        &mut self,
        name: &str,
        kind: ContributionKind,
        role: RoleKind,
        subject: Option<ContributionSubject>,
        needs: BTreeSet<Capability>,
        paths: Vec<(WorkspacePath, LockMode)>,
        producer: Option<&Id<Agent>>,
    ) -> Result<Tick> {
        let arbiter = Arbiter::new(self.journal.clone());
        let planning = self.app.plans();
        let mut view = self.view()?;
        let at = self.at()?;
        let contribution_id = id(name)?;
        if view.admission().assignments().contains_key(&id(name)?) {
            return Err(Denial::new(
                "recovery_unresolved",
                "Original live admission capability is unavailable; no replacement start is inferred",
            ));
        }
        let eligible = planning.eligible(self.owner.session(), &needs, producer)?;
        let mut profiles = view
            .registry()
            .unwrap()
            .decisions
            .iter()
            .filter(|d| d.outcome == Readiness::Ready && eligible.contains(&d.profile.agent))
            .map(|d| d.profile.clone())
            .fold(BTreeMap::new(), |mut map, p| {
                map.entry(p.agent.clone()).or_insert(p);
                map
            });
        let mut basis = vec![];
        if role == RoleKind::Reviewer {
            let result = subject.as_ref().map(|s| s.reference());
            let (reference, choice) = view
                .finalization()
                .history
                .iter()
                .rev()
                .find_map(|(r, e)| {
                    if let ymp_kernel::finalization::FinalizationRecorded::CandidateReviewer(c) = e
                    {
                        (Some(&c.input.subject) == result).then_some((r, c))
                    } else {
                        None
                    }
                })
                .ok_or_else(|| {
                    Denial::new("candidate_review_pending", "No recorded candidate reviewer")
                })?;
            profiles.retain(|agent, _| Some(agent) == choice.decision.outcome.as_ref());
            basis.push(reference.clone());
        }
        if role == RoleKind::FinalReviewer {
            let (reference, choice) = view
                .finalization()
                .history
                .iter()
                .rev()
                .find_map(|(r, e)| {
                    if let ymp_kernel::finalization::FinalizationRecorded::Reviewer(c) = e {
                        Some((r, c))
                    } else {
                        None
                    }
                })
                .ok_or_else(|| {
                    Denial::new("final_review_pending", "No recorded reviewer choice")
                })?;
            profiles.retain(|agent, _| Some(agent) == choice.decision.outcome.as_ref());
            basis = vec![
                view.finalization()
                    .aggregate
                    .as_ref()
                    .unwrap()
                    .reference()?,
                reference.clone(),
            ];
        }
        if role == RoleKind::Narrator
            && let Some(work) = view.finalization().history.iter().find_map(|(_, e)| {
                if let ymp_kernel::finalization::FinalizationRecorded::NarrationWork(w) = e {
                    (w.contribution.id == contribution_id).then_some(w)
                } else {
                    None
                }
            })
        {
            profiles.retain(|_, p| *p == work.profile);
        }
        if let Some(work) = view.progress().work(&contribution_id) {
            profiles.retain(|_, p| *p == work.profile);
        }
        let first = profiles.values().next().ok_or_else(|| {
            Denial::new(
                "assignment_unavailable",
                "No currently eligible member for this independent bounded work",
            )
        })?;
        if !view
            .coordination()
            .contributions()
            .contains_key(&contribution_id)
        {
            let provider = &view
                .registry()
                .unwrap()
                .input
                .facts
                .agents
                .iter()
                .find(|a| a.id == first.agent)
                .unwrap()
                .provider;
            let demand = ResourceDemand {
                contribution: contribution_id.erased(),
                kind,
                difficulty: Difficulty::Simple,
                provider: provider.clone(),
                profile: first.clone(),
            };
            let estimate = estimate_response(
                self.policies.cost.as_ref(),
                &ymp_kernel::treasury::estimate_view(&view, &demand)?,
            )?;
            arbiter.propose(
                self.owner.session(),
                view.revision(),
                at,
                Contribution {
                    id: contribution_id.clone(),
                    session: self.owner.session().clone(),
                    kind,
                    targets: view.criteria().iter().map(|c| c.id.clone()).collect(),
                    subject,
                    needs: needs.clone(),
                    forecast: Forecast {
                        p_success: Prob::new(0.5)?,
                        delta_belief: BTreeMap::new(),
                        source: ForecastSource::Model(
                            self.policies.contributions.selection().policy.clone(),
                        ),
                    },
                    cost: estimate.proposal.value,
                    difficulty: Difficulty::Simple,
                    proposed_by: ContributionAuthor::Runtime,
                    basis,
                },
            )?;
            return Ok(Tick::Advanced);
        }
        if !view.coordination().solicitations().contains_key(&id(name)?) {
            arbiter.open(
                self.owner.session(),
                view.revision(),
                at,
                Solicitation {
                    id: id(name)?,
                    contribution: contribution_id.clone(),
                    stimulus: ymp_domain::task::Real::new(1.0)?,
                    deadline: at.saturating_add(self.definition()?.offer_window_ms),
                    eligible: profiles.keys().cloned().collect(),
                    visibility: SolicitationVisibility::Open,
                    reopened: 0,
                    state: SolicitationState::Open,
                },
            )?;
            return Ok(Tick::Advanced);
        }
        for (agent, profile) in &profiles {
            let offer_id = id(&format!("offer-{name}-{}", Digest::of(agent.as_str())))?;
            if !view.coordination().offers().contains_key(&offer_id) {
                let c = &view.coordination().contributions()[&contribution_id].value;
                arbiter.submit(
                    self.owner.session(),
                    view.revision(),
                    at,
                    Offer {
                        id: offer_id,
                        solicitation: id(name)?,
                        agent: agent.clone(),
                        profile: profile.clone(),
                        forecast: c.forecast.clone(),
                        cost: c.cost.clone(),
                        approach: "RuntimeProxy for the recorded fixed-workflow baseline".into(),
                        source: OfferSource::RuntimeProxy,
                        at,
                    },
                )?;
                return Ok(Tick::Advanced);
            }
        }
        let deadline = view.coordination().solicitations()[&id(name)?]
            .value
            .deadline;
        if at < deadline {
            return Ok(Tick::Waiting { until: deadline });
        }
        if !view.coordination().awards().contains_key(&id(name)?) {
            let input = award_view(&view, &id(name)?, at)?;
            let terms = self.policies.award.commitment_terms(&input)?;
            let proposal = self.policies.award.award(&input)?;
            let offer = &view.coordination().offers()[&proposal.value.offer].value;
            arbiter.award_with_terms(
                self.owner.session(),
                view.revision(),
                at,
                proposal,
                Digest::of_value(&input)?,
                Commitment {
                    id: id(name)?,
                    debtor: offer.agent.clone(),
                    creditor: Creditor::Runtime,
                    subject: contribution_id,
                    condition: None,
                    lease: terms.value.initial_lease(at, terms.value.lease_duration)?,
                    state: CommitmentState::Proposed,
                    history: vec![],
                },
                Some(terms),
            )?;
            return Ok(Tick::Advanced);
        }
        let award = &view.coordination().awards()[&id(name)?];
        let terms = &award
            .value
            .terms
            .as_ref()
            .ok_or_else(|| {
                Denial::new("commitment_terms", "Recorded award has no lifecycle terms")
            })?
            .outcome;
        let offer = &view.coordination().offers()[&award.value.decision.outcome.offer].value;
        let contribution =
            &view.coordination().contributions()[&award.value.commitment.subject].value;
        let agent = view
            .registry()
            .unwrap()
            .input
            .facts
            .agents
            .iter()
            .find(|a| a.id == offer.agent)
            .unwrap();
        let demand = ResourceDemand {
            contribution: contribution.id.erased(),
            kind: contribution.kind,
            difficulty: contribution.difficulty,
            provider: agent.provider.clone(),
            profile: offer.profile.clone(),
        };
        let estimate = self
            .policies
            .cost
            .estimate(&ymp_kernel::treasury::estimate_view(&view, &demand)?)?;
        let allowance =
            self.policies
                .resources
                .allowance(&ymp_kernel::treasury::allowance_view(
                    &view,
                    &demand,
                    estimate.value,
                    at,
                )?)?;
        let mut prepared = self.admission.prepare(
            self.owner.session(),
            view.revision(),
            at,
            AdmissionCommand {
                assignment: id(name)?,
                award: award.reference.clone(),
                role,
                workspace: self.definition()?.workspace,
                access: needs,
                paths,
                reservation: id(name)?,
                grant: id(name)?,
                operations: BTreeSet::from([TeamOperation::BoardRead]),
                lease: terms.initial_lease(at, allowance.value.timeout)?,
            },
            AdmissionPolicies {
                cost: self.policies.cost.as_ref(),
                resources: self.policies.resources.as_ref(),
            },
            Some(self.provider.clone()),
        )?;
        let admitted = self.admission.admit(&mut prepared)?;
        if role == RoleKind::Producer {
            let before = id(&format!("before-{name}"))?;
            self.admission.gatekeeper().workspace().snapshot_unstarted(
                admitted.files.as_ref().ok_or_else(|| {
                    Denial::new(
                        "production_files",
                        "Production requires protected file access",
                    )
                })?,
                before.clone(),
                self.at()?,
            )?;
            let current = self.view()?;
            let attempt = self.results.prepare_attempt(
                &admitted.grant,
                current.revision(),
                self.at()?,
                id(&format!("attempt-{name}"))?,
                before,
                id(&format!("after-{name}"))?,
            )?;
            self.results.begin(&attempt)?;
        }
        view = self.view()?;
        let composer = self.app.finalization(self.content.clone());
        let input = composer.context_input(self.owner.session(), &admitted.assignment.id)?;
        let prompt = composer.record_context(
            &self.owner,
            view.revision(),
            self.at()?,
            input.clone(),
            self.policies.context.prompt(&input)?,
        )?;
        let mut live = self.host.attach(
            admitted,
            id(&format!("call-{name}"))?,
            id(&format!("receipt-{name}"))?,
            prompt,
        )?;
        let started = self.host.start(&mut live);
        self.live = Some(live);
        started?;
        Ok(Tick::Advanced)
    }
}

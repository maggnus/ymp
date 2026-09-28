//! Paid planning consumers. Historical output never restores execution authority.
use crate::{
    decision::{DecisionConsumer, SessionControl},
    events::Event,
    journal::{Journal, ParameterSchemas, validate_append},
    ports::planning::*,
    view::SessionView,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};
use ymp_domain::{
    Denial, Digest, Id, Proposal, Ref, Result,
    assignment::*,
    journal::{Actor, Decision, Envelope, PolicySelection, SelectionChange, decode, encode},
    plan::*,
    resources::{Purpose, ReservationState},
    task::*,
};

pub(crate) mod contributions;
pub use contributions::contribution_input;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaidDecision<T> {
    pub source: PaidPlanningView,
    pub decision: Decision<T>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum PlanningRecorded {
    Policy {
        effective: PolicySelection,
        change: SelectionChange,
    },
    Intake(Box<PaidDecision<IntakeOutcome>>),
    Plan(Box<PaidDecision<PlanDefinition>>),
    Team(Team),
    Contributions {
        input: Box<ContributionView>,
        decision: Box<Decision<Vec<Contribution>>>,
    },
}
impl PlanningRecorded {
    pub fn selection(&self) -> Option<&PolicySelection> {
        match self {
            Self::Policy { effective, .. } => Some(effective),
            Self::Intake(data) => Some(&data.decision.effective),
            Self::Plan(data) => Some(&data.decision.effective),
            Self::Contributions { decision, .. } => Some(&decision.effective),
            Self::Team(_) => None,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct PlanningState {
    pub team: Option<Team>,
    pub history: Vec<(Ref, PlanningRecorded)>,
    pub consumed: BTreeSet<Id<Invocation>>,
}
pub struct Plans<J: Journal> {
    journal: Arc<J>,
    owner: DecisionConsumer<J>,
}
pub struct PaidRequest<T> {
    pub expected_revision: u64,
    pub at: u64,
    pub source: PaidPlanningView,
    pub proposal: Proposal<T>,
}
pub fn work_boundary(view: &SessionView) -> Result<()> {
    if view
        .admission()
        .assignments()
        .values()
        .any(|a| crate::gatekeeper::unresolved(view, a))
    {
        return Err(Denial::new(
            "planning_boundary",
            "Unfinished execution or unresolved ownership prevents a planning policy change",
        ));
    }
    Ok(())
}
pub fn prompt(view: &SessionView, port: &str) -> Result<Prompt> {
    if !matches!(port, "IntakePolicy" | "Planner") {
        return Err(Denial::new(
            "planning_port",
            "This port does not consume paid Planner output",
        ));
    }
    let input = PlanningPrompt {
        contract: view
            .contract()
            .ok_or_else(|| Denial::new("contract_missing", "Planning requires a task"))?
            .reference(),
        goal: view.task().unwrap().goal.clone(),
        criteria: view.criteria().to_vec(),
        method: view.method().cloned(),
        effective: view
            .policies()
            .get(port)
            .ok_or_else(|| Denial::new("policy_selection", "No planning policy selected"))?
            .clone(),
    };
    Ok(Prompt {
        text: String::from_utf8(encode(&input)?)
            .map_err(|_| Denial::new("planning_prompt", "Planning input is not UTF-8"))?,
        basis: vec![input.contract],
    })
}
pub fn paid_input(
    view: &SessionView,
    invocation: &Id<Invocation>,
    port: &str,
) -> Result<PaidPlanningView> {
    work_boundary(view)?;
    let record = view
        .execution()
        .invocations()
        .get(invocation)
        .ok_or_else(|| Denial::new("planner_invocation", "No invocation in this session"))?;
    let original = &record.dispatch.assignment;
    let admission = &view.admission().assignments()[&original.id];
    let mut restored = admission.intent.assignment.clone();
    restored.state = original.state;
    let account = &view.treasury().unwrap().accounts[&record.dispatch.reservation];
    let contribution = &view.coordination().contributions()[&original.contribution];
    let receipt = record
        .receipt
        .as_ref()
        .ok_or_else(|| Denial::new("planner_receipt", "No settled Planner receipt"))?;
    let (input, legacy): (PlanningPrompt, bool) = if let Ok(input) =
        decode(record.dispatch.prompt.text.as_bytes())
    {
        (input, true)
    } else {
        let context = view
            .finalization()
            .history
            .iter()
            .find_map(|(_, event)| match event {
                crate::finalization::FinalizationRecorded::Context(context)
                    if context.input.assignment == *original
                        && context.decision.outcome == record.dispatch.prompt =>
                {
                    Some(context.as_ref())
                }
                _ => None,
            })
            .ok_or_else(|| {
                Denial::new(
                    "planning_context",
                    "Planner requires its exact recorded role context",
                )
            })?;
        if context.input.purpose["operation"] != "planning" || context.input.purpose["port"] != port
        {
            return Err(Denial::new(
                "planning_context",
                "Planner context names another operation",
            ));
        }
        (decode(&encode(&context.input.purpose["planning"])?)?, false)
    };
    let expected: PlanningPrompt = decode(prompt(view, port)?.text.as_bytes())?;
    if restored != *original
        || original.session != *view.session()
        || original.role != RoleKind::Planner
        || contribution.value.kind != ContributionKind::Plan
        || contribution.contract != expected.contract
        || contribution.value.targets != expected.criteria.iter().map(|c| c.id.clone()).collect()
        || record.terminal != Some(InvocationTerminal::Completed)
        || !record.confirmed_terminal
        || !crate::execution::closed(view, &original.id)
        || crate::execution::limit_reason(view, record, record.ended_at.unwrap_or(0))?.is_some()
        || account.reservation.state != ReservationState::Settled
        || account.reservation.purpose != Purpose::Coordination
        || account.invocation.as_ref() != Some(&invocation.erased())
        || account.settlement.as_ref().map(|s| &s.receipt) != Some(receipt)
        || input != expected
        || (legacy && record.dispatch.prompt.basis != vec![input.contract.clone()])
        || view.planning().consumed.contains(invocation)
    {
        return Err(Denial::new(
            "planner_provenance",
            "Planner output is unpaid, stale, consumed, foreign or outside its admitted scope",
        ));
    }
    let assignment = original.reference()?;
    Ok(PaidPlanningView {
        journal: view.digest()?,
        invocation: invocation.clone(),
        assignment,
        admission: record.dispatch.admission.clone(),
        completion: record.end.clone().unwrap(),
        receipt: account.last.clone(),
        prompt: input,
        output: record.output.clone(),
    })
}
fn paid_validate<T: Clone + PartialEq + Serialize>(
    view: &SessionView,
    data: &PaidDecision<T>,
    port: &str,
) -> Result<()> {
    if data.source != paid_input(view, &data.source.invocation, port)?
        || data.decision.input != Digest::of_value(&data.source)?
        || data.decision.effective != data.source.prompt.effective
        || data.decision.selection_change.is_some()
        || data.decision.proposal.policy != data.decision.effective.policy
        || data.decision.outcome != data.decision.proposal.value
        || data.decision.proposal.basis
            != vec![
                data.source.admission.clone(),
                data.source.completion.clone(),
                data.source.receipt.clone(),
                data.source.prompt.contract.clone(),
            ]
    {
        return Err(Denial::new(
            "planner_attribution",
            "Decision differs from its paid recorded input and selected policy",
        ));
    }
    data.decision.proposal.validate()
}
pub fn extracted(input: &PaidPlanningView) -> Result<IntakeOutput> {
    let mut output: IntakeOutput = decode(input.output.as_bytes())?;
    if (output.criteria.is_empty() && input.prompt.criteria.is_empty())
        || output.criteria.len() > 128
        || output.questions.len() > 32
    {
        return Err(Denial::new(
            "criteria_extraction",
            "Extraction must contain bounded atomic criteria and questions",
        ));
    }
    for criterion in &mut output.criteria {
        criterion.origin = CriterionOrigin::Derived(input.assignment.clone());
        criterion.validate()?;
    }
    for q in &output.questions {
        ymp_domain::require_text(&q.question, 4096)?;
        ymp_domain::require_text(&q.assumption, 4096)?;
        ymp_domain::require_text(&q.reason, 4096)?;
        if q.rework_cost.get() < 0.0 {
            return Err(Denial::new(
                "question_voi",
                "Question VOI cannot be negative",
            ));
        }
    }
    Ok(output)
}
pub fn validate_definition(
    view: &SessionView,
    definition: &PlanDefinition,
    author: &Id<Assignment>,
) -> Result<()> {
    definition.plan.validate()?;
    if definition.plan.session != *view.session()
        || definition.plan.author != *author
        || definition.plan.version != 1
        || !view.results().plans().is_empty()
        || definition.plan.items != definition.items.iter().map(|i| i.id.clone()).collect()
        || definition.items.len() != definition.plan.items.len()
    {
        return Err(Denial::new(
            "plan_definition",
            "Initial plan identity or item graph differs",
        ));
    }
    let targets: BTreeSet<_> = view.criteria().iter().map(|c| c.id.clone()).collect();
    let covered: BTreeSet<_> = definition
        .items
        .iter()
        .flat_map(|i| i.targets.iter().cloned())
        .collect();
    if covered != targets {
        return Err(Denial::new(
            "plan_coverage",
            "The plan must cover every current criterion exactly within its scope",
        ));
    }
    let mut ready = BTreeSet::new();
    for item in &definition.items {
        item.validate()?;
        if item.plan != definition.plan.id
            || item.state != WorkState::Open
            || !item.attempts.is_empty()
            || item.accepted.is_some()
            || item.parent.is_some()
            || !item
                .needs
                .is_subset(&view.task().unwrap().constraints.allowed)
            || item.writes.is_empty()
            || !item
                .needs
                .contains(&ymp_domain::journal::Capability::WriteFiles)
            || item.writes.iter().any(|path| path.as_str() == ".")
        {
            return Err(Denial::new(
                "plan_scope",
                "Work requires supported capabilities, exact write paths and fresh lifecycle",
            ));
        }
        for dependency in &item.deps {
            if !definition.plan.items.contains(dependency) {
                let prior = view
                    .results()
                    .items()
                    .get(dependency)
                    .ok_or_else(|| Denial::new("plan_dependency", "Unknown prerequisite"))?;
                let accepted = prior
                    .accepted
                    .as_ref()
                    .and_then(|r| view.results().results().get(r));
                if prior.state != WorkState::Accepted || accepted.is_none() {
                    return Err(Denial::new(
                        "plan_dependency",
                        "Prerequisite has no accepted result",
                    ));
                }
                ready.insert(dependency.clone());
            }
        }
    }
    loop {
        let before = ready.len();
        for item in &definition.items {
            if item.deps.is_subset(&ready) {
                ready.insert(item.id.clone());
            }
        }
        if ready.len() == before {
            break;
        }
    }
    if !definition.plan.items.is_subset(&ready) {
        return Err(Denial::new(
            "plan_cycle",
            "Work dependencies contain a cycle",
        ));
    }
    if definition.items.len() != 1 {
        return Err(Denial::new(
            "plan_decomposition_unavailable",
            "Multiple work items require diagnosed decomposition in W3",
        ));
    }
    Ok(())
}
pub fn validate(
    view: &SessionView,
    event: &Envelope<Event>,
    data: &PlanningRecorded,
    schemas: &ParameterSchemas,
) -> Result<PlanningState> {
    if event.at < view.latest_at()
        || event.input != Some(view.digest()?)
        || event.policy != data.selection().map(|s| s.policy.clone())
        || event.refs != references(data)
    {
        return Err(Denial::new(
            "planning_attribution",
            "Planning event differs from its work boundary",
        ));
    }
    if let Some(selection) = data.selection() {
        schemas.validate(selection)?;
    }
    let mut state = view.planning().clone();
    match data {
        PlanningRecorded::Policy { effective, change } => {
            work_boundary(view)?;
            if !matches!(
                effective.policy.port.as_str(),
                "IntakePolicy" | "Planner" | "ContributionPolicy"
            ) {
                return Err(Denial::new("policy_port", "Unsupported planning consumer"));
            }
            crate::ledger::selection(view, effective, Some(change), &effective.policy.port)?;
        }
        PlanningRecorded::Intake(data) => {
            paid_validate(view, data, "IntakePolicy")?;
            if !view.results().plans().is_empty() {
                return Err(Denial::new(
                    "intake_planned",
                    "Criteria extraction precedes the initial plan",
                ));
            }
            let output = extracted(&data.source)?;
            let mut criteria = view.criteria().to_vec();
            criteria.extend(output.criteria);
            if data.decision.outcome.criteria != criteria
                || data.decision.outcome.questions.len() != output.questions.len()
            {
                return Err(Denial::new(
                    "criteria_provenance",
                    "Preserve existing criteria and exact Derived assignment provenance",
                ));
            }
            let threshold: Real = decode(&encode(
                data.decision
                    .effective
                    .parameters
                    .get("cost_interrupt")
                    .ok_or_else(|| Denial::new("intake_parameters", "Missing interruption cost"))?,
            )?)?;
            for (actual, q) in data.decision.outcome.questions.iter().zip(output.questions) {
                let expected = if data.decision.effective.policy.implementation
                    == "VoiClarification"
                    && q.p_misinterpretation.get() * q.rework_cost.get() > threshold.get()
                {
                    QuestionDecision::Ask(q)
                } else {
                    QuestionDecision::Assume(q)
                };
                if *actual != expected {
                    return Err(Denial::new(
                        "question_decision",
                        "Question decision differs from its VOI and interruption cost",
                    ));
                }
            }
            AcceptanceContract::new(
                view.task().unwrap(),
                &criteria,
                view.contract().unwrap().checks.clone(),
            )?;
            state.consumed.insert(data.source.invocation.clone());
        }
        PlanningRecorded::Plan(data) => {
            paid_validate(view, data, "Planner")?;
            if view.method().is_none() || view.planning().team.is_none() {
                return Err(Denial::new(
                    "planning_team",
                    "Choose method and initial team before committing a plan",
                ));
            }
            let author = Id::new(data.source.assignment.id.as_str())?;
            validate_definition(view, &data.decision.outcome, &author)?;
            let mut expected: PlanDefinition =
                if data.decision.effective.policy.implementation == "ExplicitPlan" {
                    decode(&encode(&data.decision.effective.parameters["definition"])?)?
                } else {
                    decode(data.source.output.as_bytes())?
                };
            expected.plan.author = author;
            if expected != data.decision.outcome {
                return Err(Denial::new(
                    "plan_output",
                    "Plan differs from its paid output or explicit experiment control",
                ));
            }
            state.consumed.insert(data.source.invocation.clone());
        }
        PlanningRecorded::Team(team) => {
            work_boundary(view)?;
            if state.team.is_some() || team.session != *view.session() || team.revision != 1 {
                return Err(Denial::new(
                    "team_initial",
                    "Only the initial team is available",
                ));
            }
            let pool = view
                .registry()
                .ok_or_else(|| Denial::new("registry_missing", "Team requires eligible agents"))?;
            if pool.input.constraints != view.task().unwrap().constraints {
                return Err(Denial::new(
                    "team_registry",
                    "Refresh Registry after constraints or pins changed",
                ));
            }
            let members: BTreeSet<_> = team.members.iter().map(|m| m.agent.clone()).collect();
            let constraints = &view.task().unwrap().constraints;
            let minimum = match view.method().map(|m| &m.kind) {
                Some(ymp_domain::journal::MethodKind::Solo) => 1,
                Some(ymp_domain::journal::MethodKind::SoloWithVerifier) => 2,
                _ => {
                    return Err(Denial::new(
                        "team_method",
                        "Initial team needs an implemented fixed method",
                    ));
                }
            };
            if members.len() != team.members.len()
                || members.len() < minimum
                || members.len() > constraints.max_members as usize
                || !members.is_subset(&pool.outcome.eligible)
                || constraints
                    .pins
                    .team_size
                    .is_some_and(|n| members.len() != n as usize)
                || constraints
                    .pins
                    .roster
                    .as_ref()
                    .is_some_and(|roster| *roster != members.iter().map(Id::erased).collect())
                || team.members.iter().any(|m| {
                    m.joined != event.at
                        || m.left.is_some()
                        || ymp_domain::require_text(&m.reason, 1024).is_err()
                })
            {
                return Err(Denial::new(
                    "team_membership",
                    "Initial membership violates eligibility, method independence or user pins",
                ));
            }
            state.team = Some(team.clone());
        }
        PlanningRecorded::Contributions { input, decision } => {
            contributions::validate(view, input, decision)?
        }
    }
    if state.history.len() >= 4096 {
        return Err(Denial::new("planning_limit", "Planning history exhausted"));
    }
    state.history.push((event.reference()?, data.clone()));
    Ok(state)
}
pub fn references(data: &PlanningRecorded) -> Vec<Ref> {
    let mut refs = match data {
        PlanningRecorded::Intake(d) => d.decision.proposal.basis.clone(),
        PlanningRecorded::Plan(d) => d.decision.proposal.basis.clone(),
        PlanningRecorded::Contributions { decision, .. } => decision.proposal.basis.clone(),
        _ => vec![],
    };
    refs.sort();
    refs.dedup();
    refs
}
impl<J: Journal> Plans<J> {
    pub(crate) fn new(journal: Arc<J>, owner: DecisionConsumer<J>) -> Self {
        Self { journal, owner }
    }
    pub fn eligible(
        &self,
        session: &Id,
        needs: &BTreeSet<ymp_domain::journal::Capability>,
        producer: Option<&Id<ymp_domain::identity::Agent>>,
    ) -> Result<BTreeSet<Id<ymp_domain::identity::Agent>>> {
        let view = self.journal.view(session, None)?;
        if view.task().is_none() {
            return Err(Denial::new(
                "task_missing",
                "No task for assignment selection",
            ));
        }
        Ok(contributions::available(&view, needs, producer))
    }
    pub fn prompt(&self, session: &Id, port: &str) -> Result<Prompt> {
        prompt(&self.journal.view(session, None)?, port)
    }
    pub fn paid_input(
        &self,
        session: &Id,
        invocation: &Id<Invocation>,
        port: &str,
    ) -> Result<PaidPlanningView> {
        paid_input(&self.journal.view(session, None)?, invocation, port)
    }
    fn append(
        &self,
        control: &SessionControl,
        expected: u64,
        at: u64,
        data: PlanningRecorded,
    ) -> Result<u64> {
        let session = self.owner.authorize(control)?;
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
            payload: Event::PlanningRecorded {
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
        self.journal.append(&session, expected, &[event])
    }
    pub fn select(
        &self,
        control: &SessionControl,
        expected: u64,
        at: u64,
        effective: PolicySelection,
    ) -> Result<u64> {
        let session = self.owner.authorize(control)?;
        let view = self.journal.view(&session, None)?;
        let current = view.policies().get(&effective.policy.port).ok_or_else(|| {
            Denial::new(
                "policy_selection",
                "Select this port when opening the session",
            )
        })?;
        self.append(
            control,
            expected,
            at,
            PlanningRecorded::Policy {
                effective,
                change: SelectionChange {
                    previous: current.policy.clone(),
                    boundary: expected,
                },
            },
        )
    }
    pub fn extract(
        &self,
        control: &SessionControl,
        request: PaidRequest<IntakeOutcome>,
    ) -> Result<u64> {
        let data = paid_decision(&request.source, request.proposal)?;
        self.append(
            control,
            request.expected_revision,
            request.at,
            PlanningRecorded::Intake(Box::new(data)),
        )
    }
    pub fn plan(
        &self,
        control: &SessionControl,
        request: PaidRequest<PlanDefinition>,
    ) -> Result<u64> {
        let data = paid_decision(&request.source, request.proposal)?;
        self.append(
            control,
            request.expected_revision,
            request.at,
            PlanningRecorded::Plan(Box::new(data)),
        )
    }
    pub fn team(
        &self,
        control: &SessionControl,
        expected: u64,
        at: u64,
        team: Team,
    ) -> Result<u64> {
        self.append(control, expected, at, PlanningRecorded::Team(team))
    }
    pub fn method(
        &self,
        control: &SessionControl,
        request: crate::decision::MethodDecision,
        replacement: Option<PolicySelection>,
    ) -> Result<u64> {
        let session = self.owner.authorize(control)?;
        work_boundary(&self.journal.view(&session, None)?)?;
        self.owner
            .record_method(&session, request, replacement.map(|s| (control, s)))
    }
}
fn paid_decision<T: Clone>(
    source: &PaidPlanningView,
    proposal: Proposal<T>,
) -> Result<PaidDecision<T>> {
    Ok(PaidDecision {
        source: source.clone(),
        decision: Decision {
            outcome: proposal.value.clone(),
            proposal,
            effective: source.prompt.effective.clone(),
            input: Digest::of_value(source)?,
            selection_change: None,
        },
    })
}

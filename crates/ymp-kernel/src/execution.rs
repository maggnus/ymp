//! One durable dispatch boundary, attributed observations, and conservative closure.
use crate::{
    events::Event,
    gatekeeper::{Gatekeeper, GrantToken},
    journal::{AppendResolution, ContentStore, Journal},
    ports::execution::{BackendEvent, BackendObservation, BackendStart},
    treasury::{ReservationChange, Treasury},
    view::SessionView,
    workspace_guard::MediatedAccess,
    workspace_locks::LockChange,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use ymp_domain::{
    Denial, Digest, Id, Ref, Result,
    assignment::{Assignment, AssignmentState, ErrorClass, Invocation, InvocationTerminal, Prompt},
    identity::{InvocationSettings, ProfileSettings, Provider},
    journal::{Decision, Envelope, PolicySelection},
    resources::{Allowance, Coverage, Receipt, ReceiptPrice, Reservation, ReservationState, Usage},
};
pub const MAX_INVOCATION_EVENTS: usize = 4096;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationDispatch {
    pub invocation: Id<Invocation>,
    pub assignment: Assignment,
    pub provider: Id<Provider>,
    pub reservation: Id<Reservation>,
    pub receipt: Id<Receipt>,
    pub admission: Ref,
    pub prompt: Prompt,
    pub settings: ProfileSettings,
    pub allowance: Allowance,
    pub backend: PolicySelection,
    pub nonce: Digest,
    pub at: u64,
    pub deadline: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationObservation {
    Dispatch(Box<InvocationDispatch>),
    Ready {
        nonce: Digest,
    },
    Backend(BackendEvent),
    Receipt(Receipt),
    Cost {
        receipt: Digest,
        decision: Box<Decision<ReceiptPrice>>,
    },
    Diagnostic {
        class: ErrorClass,
        code: String,
        message: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InvocationRecord {
    pub dispatch: InvocationDispatch,
    pub ready: Ref,
    pub invocation: Option<Invocation>,
    pub start: Option<Ref>,
    pub observations: BTreeMap<u64, (BackendEvent, Ref)>,
    pub output: String,
    pub usage: Usage,
    pub turns: u32,
    pub receipt: Option<Receipt>,
    pub cost: Option<(Digest, Decision<ReceiptPrice>, Ref)>,
    pub terminal: Option<InvocationTerminal>,
    pub confirmed_terminal: bool,
    pub backend_terminal: Option<(InvocationTerminal, Ref, u64)>,
    pub ended_at: Option<u64>,
    pub end: Option<Ref>,
    pub diagnostics: Vec<(ErrorClass, String, String, Ref)>,
    pub last: Ref,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct PendingDispatch {
    dispatch: InvocationDispatch,
    remaining: Vec<Event>,
    last: Ref,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ExecutionView {
    invocations: BTreeMap<Id<Invocation>, InvocationRecord>,
    pending: Option<PendingDispatch>,
}
impl ExecutionView {
    pub fn invocations(&self) -> &BTreeMap<Id<Invocation>, InvocationRecord> {
        &self.invocations
    }
    pub(crate) fn complete(&self) -> Result<()> {
        if self.pending.is_some() {
            return Err(Denial::new(
                "invocation_incomplete",
                "Dispatch and both authorizations must commit together",
            ));
        }
        Ok(())
    }
    pub(crate) fn check_next(&self, event: &Envelope<Event>) -> Result<()> {
        if let Some(pending) = &self.pending
            && (pending.remaining.first() != Some(&event.payload)
                || pending.dispatch.at != event.at)
        {
            return Err(Denial::new(
                "invocation_incomplete",
                "An event cannot interrupt this exact dispatch packet",
            ));
        }
        Ok(())
    }
}
fn zero_usage() -> Usage {
    Usage {
        input: 0,
        cache_read: 0,
        cache_write: 0,
        output: 0,
        reasoning: None,
    }
}
fn settings(assignment: &Assignment) -> ProfileSettings {
    ProfileSettings {
        model: Some(assignment.profile.model.clone()),
        effort: assignment.profile.effort.clone(),
    }
}
fn record<'a>(view: &'a SessionView, id: &Id<Invocation>) -> Result<&'a InvocationRecord> {
    view.execution().invocations.get(id).ok_or_else(|| {
        Denial::new(
            "invocation_missing",
            "No committed dispatch for this invocation",
        )
    })
}
fn dispatch_tail(data: &InvocationDispatch) -> Vec<Event> {
    let mut events = vec![Event::ReservationChanged {
        version: 1,
        change: ReservationChange::Authorized {
            reservation: data.reservation.clone(),
            invocation: data.invocation.erased(),
        },
    }];
    if !data.assignment.access.is_empty() {
        events.push(Event::LockChanged {
            version: 1,
            change: LockChange::Authorized {
                assignment: data.assignment.id.erased(),
                invocation: data.invocation.erased(),
            },
        });
    }
    events.push(Event::InvocationObserved {
        version: 1,
        invocation: data.invocation.clone(),
        observation: Box::new(InvocationObservation::Ready {
            nonce: data.nonce.clone(),
        }),
    });
    events
}
fn validate_dispatch(view: &SessionView, data: &InvocationDispatch, at: u64) -> Result<()> {
    crate::progress::validate_dispatch(view, data)?;
    crate::finalization::validate_dispatch(view, data)?;
    crate::results::validate_dispatch(view, &data.assignment)?;
    data.prompt.validate()?;
    data.settings.validate()?;
    data.allowance.validate()?;
    let source = view
        .admission()
        .assignments()
        .get(&data.assignment.id)
        .ok_or_else(|| {
            Denial::new(
                "assignment_missing",
                "Dispatch requires an admitted assignment",
            )
        })?;
    let account = view
        .treasury()
        .and_then(|book| book.accounts.get(&data.reservation))
        .ok_or_else(|| {
            Denial::new(
                "reservation_missing",
                "Dispatch has no financial reservation",
            )
        })?;
    let lease = &view.coordination().commitments()[&data.assignment.commitment];
    let provider = view
        .registry()
        .and_then(|registry| {
            registry
                .input
                .facts
                .agents
                .iter()
                .find(|agent| agent.id == data.assignment.agent)
        })
        .map(|agent| &agent.provider);
    if at < view.latest_at()
        || data.at != at
        || data.assignment != source.intent.assignment
        || data.assignment.state != AssignmentState::Admitted
        || source.reservation != data.reservation
        || source.references.last() != Some(&data.admission)
        || provider != Some(&data.provider)
        || data.settings != settings(&data.assignment)
        || view.policies().get("ExecutionBackend") != Some(&data.backend)
        || lease.state != ymp_domain::coordination::CommitmentState::Active
        || at > lease.lease.expires
        || data.deadline
            != source.intent.grant.expires.min(
                view.task()
                    .and_then(|task| task.constraints.deadline)
                    .unwrap_or(u64::MAX),
            )
        || data.deadline <= at
        || data.allowance.timeout != data.deadline - at
        || data.allowance.cost != data.assignment.allowance.cost
        || data.allowance.native_turns != data.assignment.allowance.native_turns
        || data.allowance.output_chars != data.assignment.allowance.output_chars
        || data.allowance.output_chars > 1_000_000
        || data.allowance.native_turns > 4096
        || data.allowance.timeout > 86_400_000
        || account.reservation.state != ReservationState::Held
        || account.revoked
        || account.invocation.is_some()
        || view.execution().invocations.len() >= 4096
        || view.execution().invocations.contains_key(&data.invocation)
        || view
            .execution()
            .invocations
            .values()
            .any(|prior| prior.dispatch.assignment.id == data.assignment.id)
        || data.invocation.erased() == data.assignment.id.erased()
    {
        return Err(Denial::new(
            "invocation_dispatch",
            "Dispatch differs from current authority, limits, identity or selected backend",
        ));
    }
    for reference in &data.prompt.basis {
        view.resolve(reference)?;
    }
    match (
        source.intent.access_owner.as_ref(),
        view.path_locks().get(&data.assignment.id.erased()),
    ) {
        (None, None) if data.assignment.access.is_empty() => {}
        (Some(owner), Some(lock))
            if lock.acquired.mediated_owner.as_ref() == Some(owner)
                && lock.invocation.is_none()
                && !lock.revoked
                && lock.released.is_none() => {}
        _ => {
            return Err(Denial::new(
                "invocation_scope",
                "Dispatch has no matching unused workspace capability",
            ));
        }
    }
    Ok(())
}
pub(crate) fn attribution(
    view: &SessionView,
    event: &Event,
) -> Result<(Option<ymp_domain::PolicyRef>, Option<Digest>, Vec<Ref>)> {
    let mut refs = match event {
        Event::InvocationObserved { observation, .. }
            if matches!(observation.as_ref(), InvocationObservation::Dispatch(_)) =>
        {
            let InvocationObservation::Dispatch(data) = observation.as_ref() else {
                unreachable!()
            };
            let mut refs = data.prompt.basis.clone();
            refs.push(data.admission.clone());
            refs
        }
        Event::InvocationObserved { observation, .. }
            if matches!(observation.as_ref(), InvocationObservation::Ready { .. }) =>
        {
            vec![
                view.execution()
                    .pending
                    .as_ref()
                    .ok_or_else(|| Denial::new("invocation_incomplete", "No dispatch packet"))?
                    .last
                    .clone(),
            ]
        }
        Event::InvocationStarted { invocation, .. } => {
            vec![record(view, &invocation.id)?.last.clone()]
        }
        Event::InvocationObserved {
            invocation,
            observation,
            ..
        } => {
            let mut refs = vec![record(view, invocation)?.last.clone()];
            if let InvocationObservation::Backend(BackendEvent {
                observation:
                    BackendObservation::Progress {
                        basis: Some(reference),
                        ..
                    },
                ..
            }) = observation.as_ref()
                && view.resolve(reference).is_ok()
            {
                refs.push(reference.clone());
            }
            if matches!(
                observation.as_ref(),
                InvocationObservation::Receipt(_) | InvocationObservation::Cost { .. }
            ) {
                refs.push(
                    view.treasury().unwrap().accounts
                        [&record(view, invocation)?.dispatch.reservation]
                        .last
                        .clone(),
                );
            }
            if let InvocationObservation::Cost { decision, .. } = observation.as_ref() {
                refs.extend(decision.proposal.basis.clone());
            }
            refs
        }
        Event::InvocationEnded { invocation, .. } => {
            let source = record(view, invocation)?;
            vec![
                source.last.clone(),
                view.admission().assignments()[&source.dispatch.assignment.id]
                    .references
                    .last()
                    .unwrap()
                    .clone(),
            ]
        }
        _ => return Err(Denial::new("invocation_event", "Not an invocation event")),
    };
    refs.sort();
    refs.dedup();
    if let Event::InvocationObserved { observation, .. } = event
        && let InvocationObservation::Cost { decision, .. } = observation.as_ref()
    {
        return Ok((
            Some(decision.effective.policy.clone()),
            Some(decision.input.clone()),
            refs,
        ));
    }
    Ok((None, None, refs))
}
pub(crate) fn apply(view: &SessionView, event: &Envelope<Event>) -> Result<Option<ExecutionView>> {
    view.execution().check_next(event)?;
    if view.execution().pending.is_some() {
        let mut next = view.execution().clone();
        let complete = {
            let pending = next.pending.as_mut().unwrap();
            pending.remaining.remove(0);
            pending.last = event.reference()?;
            pending.remaining.is_empty()
        };
        if complete {
            let data = next.pending.take().unwrap().dispatch;
            next.invocations.insert(
                data.invocation.clone(),
                InvocationRecord {
                    dispatch: data,
                    ready: event.reference()?,
                    invocation: None,
                    start: None,
                    observations: BTreeMap::new(),
                    output: String::new(),
                    usage: zero_usage(),
                    turns: 0,
                    receipt: None,
                    cost: None,
                    terminal: None,
                    confirmed_terminal: false,
                    backend_terminal: None,
                    ended_at: None,
                    end: None,
                    diagnostics: vec![],
                    last: event.reference()?,
                },
            );
        }
        return Ok(Some(next));
    }
    let (id, observation) = match &event.payload {
        Event::InvocationObserved {
            invocation,
            observation,
            ..
        } => (invocation, Some(observation.as_ref())),
        Event::InvocationStarted { invocation, .. } => (&invocation.id, None),
        Event::InvocationEnded { invocation, .. } => (invocation, None),
        _ => return Ok(None),
    };
    if event.at < view.latest_at() {
        return Err(Denial::new(
            "invocation_time",
            "Invocation observation predates recorded inputs",
        ));
    }
    if let Some(InvocationObservation::Dispatch(data)) = observation {
        validate_dispatch(view, data, event.at)?;
        if data.invocation != *id {
            return Err(Denial::new(
                "invocation_identity",
                "Dispatch identity differs",
            ));
        }
        let mut next = view.execution().clone();
        next.pending = Some(PendingDispatch {
            dispatch: (**data).clone(),
            remaining: dispatch_tail(data),
            last: event.reference()?,
        });
        return Ok(Some(next));
    }
    let mut next = view.execution().clone();
    let current = next
        .invocations
        .get_mut(id)
        .ok_or_else(|| Denial::new("invocation_missing", "Observation has no dispatch"))?;
    match (&event.payload, observation) {
        (Event::InvocationStarted { invocation, .. }, _) => {
            invocation.validate()?;
            if current.invocation.is_some()
                || invocation.assignment != current.dispatch.assignment.id
                || invocation.provider != current.dispatch.provider
                || invocation.settings.requested != current.dispatch.settings
                || invocation.started < current.dispatch.at
                || invocation.started > event.at
                || invocation.started >= current.dispatch.deadline
                || invocation.terminal != current.terminal
                || invocation.ended != current.ended_at
                || invocation.receipt.is_some()
            {
                return Err(Denial::new(
                    "invocation_start",
                    "Confirmation differs from the committed dispatch",
                ));
            }
            current.invocation = Some(invocation.clone());
            current.start = Some(event.reference()?);
        }
        (_, Some(InvocationObservation::Backend(observed))) => {
            if current.invocation.is_none()
                || observed.invocation != *id
                || observed.sequence != current.observations.len() as u64 + 1
                || current.observations.len() >= MAX_INVOCATION_EVENTS
            {
                return Err(Denial::new(
                    "invocation_identity",
                    "Event is foreign, repeated, out of order or after termination",
                ));
            }
            match &observed.observation {
                BackendObservation::Output(text) => {
                    if text.len() > 65_536
                        || current.output.chars().count() as u64 + text.chars().count() as u64
                            > current.dispatch.allowance.output_chars
                    {
                        return Err(Denial::new(
                            "output_limit",
                            "Whole-invocation output allowance exhausted",
                        ));
                    }
                    current.output.push_str(text);
                }
                BackendObservation::Usage { usage, turns } => {
                    usage.validate()?;
                    if !usage.includes(&current.usage) || *turns < current.turns {
                        return Err(Denial::new(
                            "usage_baseline",
                            "Invocation counters cannot reset between observations",
                        ));
                    }
                    current.usage = usage.clone();
                    current.turns = *turns;
                }
                BackendObservation::Terminal(terminal) => {
                    if current.backend_terminal.is_some() {
                        return Err(Denial::new(
                            "invocation_terminal",
                            "Backend terminal outcome is immutable",
                        ));
                    }
                    current.backend_terminal =
                        Some((terminal.clone(), event.reference()?, event.at));
                }
                BackendObservation::OperationRequest {
                    args, correlation, ..
                } => {
                    ymp_domain::require_text(correlation, 128)?;
                    if args.len() > 16_384 || args.contains('\0') {
                        return Err(Denial::new(
                            "operation_request",
                            "Operation arguments exceed the bounded host contract",
                        ));
                    }
                }
                BackendObservation::Progress { .. } | BackendObservation::ToolDenied(_) => {}
            }
            current
                .observations
                .insert(observed.sequence, (observed.clone(), event.reference()?));
        }
        (_, Some(InvocationObservation::Receipt(receipt))) => {
            receipt.validate()?;
            let account = &view.treasury().unwrap().accounts[&current.dispatch.reservation];
            if receipt.id != current.dispatch.receipt
                || receipt.invocation != id.erased()
                || !receipt.usage.includes(&current.usage)
                || account.receipt.as_ref() != Some(receipt)
            {
                return Err(Denial::new(
                    "receipt_binding",
                    "Receipt differs from this invocation and its Treasury observation",
                ));
            }
            current.receipt = Some(receipt.clone());
            if let Some(invocation) = &mut current.invocation {
                invocation.receipt = Some(receipt.id.clone());
            }
        }
        (
            _,
            Some(InvocationObservation::Diagnostic {
                class,
                code,
                message,
            }),
        ) => {
            ymp_domain::require_text(code, 128)?;
            ymp_domain::require_text(message, 1024)?;
            if current.diagnostics.len() >= 64 {
                return Err(Denial::new(
                    "invocation_limit",
                    "Diagnostic capacity exhausted",
                ));
            }
            current
                .diagnostics
                .push((*class, code.clone(), message.clone(), event.reference()?));
        }
        (_, Some(InvocationObservation::Cost { receipt, decision })) => {
            decision.proposal.validate()?;
            let actual = current
                .receipt
                .as_ref()
                .ok_or_else(|| Denial::new("receipt_missing", "Price needs observed usage"))?;
            let input = crate::treasury::cost_view(view, &current.dispatch.reservation)?;
            if *receipt != Digest::of_value(actual)?
                || view.policies().get("CostModel") != Some(&decision.effective)
                || decision.proposal.policy != decision.effective.policy
                || decision.outcome != decision.proposal.value
                || decision.selection_change.is_some()
                || decision.input != Digest::of_value(&input)?
                || (matches!(decision.outcome, ReceiptPrice::Known(_))
                    && actual.coverage != Coverage::Complete
                    && input.known_complete_cost.is_none())
            {
                return Err(Denial::new(
                    "invocation_cost",
                    "Observation price differs from its selected policy, receipt or coverage",
                ));
            }
            if let ReceiptPrice::Known(cost) | ReceiptPrice::Estimated(cost) = decision.outcome {
                ymp_domain::resources::nonnegative(cost)?;
            }
            current.cost = Some((receipt.clone(), (**decision).clone(), event.reference()?));
        }
        (
            Event::InvocationEnded {
                terminal,
                confirmed,
                ended,
                ..
            },
            _,
        ) => {
            if current.terminal.is_some()
                || *ended > event.at
                || *ended < current.dispatch.at
                || view.admission().assignments()[&current.dispatch.assignment.id]
                    .intent
                    .assignment
                    .state
                    != AssignmentState::Revoked
                || (*confirmed
                    && current
                        .observations
                        .last_key_value()
                        .is_none_or(|(_, (event, _))| {
                            event.observation != BackendObservation::Terminal(terminal.clone())
                        }))
                || (!*confirmed && *terminal == InvocationTerminal::Completed)
            {
                return Err(Denial::new(
                    "invocation_end",
                    "Terminal outcome lacks its matching observation or authority revocation",
                ));
            }
            if *terminal == InvocationTerminal::Completed {
                let account = &view.treasury().unwrap().accounts[&current.dispatch.reservation];
                if limit_reason(view, current, *ended)?.is_some()
                    || account.reservation.state != ReservationState::Settled
                    || !closed(view, &current.dispatch.assignment.id)
                {
                    return Err(Denial::new(
                        "invocation_limit",
                        "Completion lacks bounded accounting and scoped cessation",
                    ));
                }
            }
            current.terminal = Some(terminal.clone());
            current.confirmed_terminal = *confirmed;
            current.ended_at = Some(*ended);
            current.end = Some(event.reference()?);
            if let Some(invocation) = &mut current.invocation {
                invocation.ended = Some(*ended);
                invocation.terminal = Some(terminal.clone());
                invocation.validate()?;
            }
        }
        _ => {
            return Err(Denial::new(
                "invocation_event",
                "Unsupported invocation transition",
            ));
        }
    }
    current.last = event.reference()?;
    Ok(Some(next))
}
pub fn limit_reason(
    view: &SessionView,
    current: &InvocationRecord,
    at: u64,
) -> Result<Option<&'static str>> {
    if at >= current.dispatch.deadline {
        return Ok(Some("timeout"));
    }
    if current.turns > current.dispatch.allowance.native_turns {
        return Ok(Some("native_turn_limit"));
    }
    let mut floor =
        view.treasury().unwrap().accounts[&current.dispatch.reservation].observed_cost_floor;
    if let Some((receipt, decision, _)) = &current.cost
        && current
            .receipt
            .as_ref()
            .map(Digest::of_value)
            .transpose()?
            .as_ref()
            == Some(receipt)
        && let ReceiptPrice::Known(cost) | ReceiptPrice::Estimated(cost) = decision.outcome
        && cost > floor
    {
        floor = cost;
    }
    if floor > current.dispatch.allowance.cost {
        return Ok(Some("cost_limit"));
    }
    Ok(None)
}
pub(crate) fn closed(view: &SessionView, assignment: &Id<Assignment>) -> bool {
    view.execution().invocations.values().any(|record| {
        record.dispatch.assignment.id == *assignment
            && record.backend_terminal.is_some()
            && record.invocation.is_some()
            && view
                .path_locks()
                .get(&assignment.erased())
                .is_none_or(|lock| lock.released.is_some())
    })
}
pub(crate) fn control_reserve(view: &SessionView) -> (usize, usize) {
    let count: usize = view
        .execution()
        .invocations
        .values()
        .map(|record| {
            let held = view
                .treasury()
                .and_then(|book| book.accounts.get(&record.dispatch.reservation))
                .is_some_and(|account| account.reservation.state == ReservationState::Held);
            let final_price = record.receipt.as_ref().is_some_and(|receipt| {
                receipt.coverage == Coverage::Complete
                    && record.cost.as_ref().is_some_and(|(digest, _, _)| {
                        Digest::of_value(receipt).is_ok_and(|actual| actual == *digest)
                    })
            });
            usize::from(record.invocation.is_none())
                + 2 * usize::from(record.terminal.is_none())
                + usize::from(record.backend_terminal.is_none())
                + usize::from(
                    held && record
                        .receipt
                        .as_ref()
                        .is_none_or(|receipt| receipt.coverage != Coverage::Complete),
                )
                + usize::from(held && !final_price)
        })
        .sum();
    (count, count * 65_536)
}
pub struct PreparedInvocation {
    session: Id,
    expected: u64,
    events: Vec<Envelope<Event>>,
    dispatch: InvocationDispatch,
    issuer: Arc<()>,
}
pub struct Execution<J: Journal, C: ContentStore> {
    journal: Arc<J>,
    gate: Arc<Gatekeeper<J, C>>,
    treasury: Treasury<J>,
    issuer: Arc<()>,
}
impl<J: Journal, C: ContentStore> Execution<J, C> {
    pub fn new(journal: Arc<J>, gate: Arc<Gatekeeper<J, C>>) -> Result<Self> {
        gate.require_journal(&journal)?;
        Ok(Self {
            treasury: Treasury::new(journal.clone()),
            journal,
            gate,
            issuer: Arc::new(()),
        })
    }
    pub fn gatekeeper(&self) -> &Arc<Gatekeeper<J, C>> {
        &self.gate
    }
    pub fn treasury(&self) -> &Treasury<J> {
        &self.treasury
    }
    pub fn view(&self, session: &Id) -> Result<SessionView> {
        self.journal.view(session, None)
    }
    pub fn validate_admitted(
        &self,
        admitted: &crate::gatekeeper::AdmittedAssignment<J>,
        invocation: &Id<Invocation>,
        at: u64,
    ) -> Result<Assignment> {
        let view = self.view(admitted.grant.session())?;
        let canonical = self
            .gate
            .authorize_revision(&admitted.grant, None, at, view.revision())?;
        if canonical != admitted.assignment {
            return Err(Denial::new(
                "invocation_credentials",
                "Public assignment metadata differs from its grant",
            ));
        }
        match (&admitted.files, canonical.access.is_empty()) {
            (None, true) => {}
            (Some(files), false) => self.gate.workspace().prepare_invocation_access(
                files,
                &canonical.id.erased(),
                &invocation.erased(),
            )?,
            _ => {
                return Err(Denial::new(
                    "invocation_scope",
                    "Live file capability differs from the admitted scope",
                ));
            }
        }
        Ok(canonical)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &self,
        token: &GrantToken,
        id: Id<Invocation>,
        receipt: Id<Receipt>,
        prompt: Prompt,
        backend: PolicySelection,
        files: Option<&MediatedAccess<J>>,
        at: u64,
    ) -> Result<PreparedInvocation> {
        let current = self.journal.read(token.session())?;
        let view = current.view_with_schemas(token.session(), None, self.journal.schemas())?;
        let assignment = self
            .gate
            .authorize_revision(token, None, at, current.revision)?;
        let source = &view.admission().assignments()[&assignment.id];
        self.journal.schemas().validate(&backend)?;
        let mut random = [0; 32];
        getrandom::fill(&mut random)
            .map_err(|_| Denial::new("invocation_entropy", "Cannot create dispatch identity"))?;
        let deadline = source.intent.grant.expires.min(
            view.task()
                .and_then(|task| task.constraints.deadline)
                .unwrap_or(u64::MAX),
        );
        let mut allowance = assignment.allowance.clone();
        allowance.timeout = deadline
            .checked_sub(at)
            .filter(|left| *left > 0)
            .ok_or_else(|| {
                Denial::new("deadline", "Assignment lifetime exhausted before dispatch")
            })?;
        let provider = view.treasury().unwrap().accounts[&source.reservation]
            .demand
            .provider
            .clone();
        let dispatch = InvocationDispatch {
            invocation: id.clone(),
            settings: settings(&assignment),
            assignment,
            provider,
            reservation: source.reservation.clone(),
            receipt,
            admission: source.references.last().unwrap().clone(),
            prompt,
            allowance,
            backend,
            nonce: Digest::of(random),
            at,
            deadline,
        };
        validate_dispatch(&view, &dispatch, at)?;
        if let Some(access) = files {
            self.gate.workspace().prepare_invocation_access(
                access,
                &dispatch.assignment.id.erased(),
                &id.erased(),
            )?;
        } else if !dispatch.assignment.access.is_empty() {
            return Err(Denial::new(
                "invocation_scope",
                "The original live file capability is required",
            ));
        }
        let mut payloads = vec![Event::InvocationObserved {
            version: 1,
            invocation: id,
            observation: Box::new(InvocationObservation::Dispatch(Box::new(dispatch.clone()))),
        }];
        payloads.extend(dispatch_tail(&dispatch));
        let events = self.gate.events(&current, token.session(), at, payloads)?;
        Ok(PreparedInvocation {
            session: token.session().clone(),
            expected: current.revision,
            events,
            dispatch,
            issuer: self.issuer.clone(),
        })
    }
    pub fn dispatch(&self, plan: &PreparedInvocation) -> Result<InvocationDispatch> {
        if !Arc::ptr_eq(&plan.issuer, &self.issuer) {
            return Err(Denial::new(
                "invocation_plan",
                "Foreign dispatch preparation",
            ));
        }
        match self
            .journal
            .resolve_append(&plan.session, plan.expected, &plan.events)?
        {
            AppendResolution::Committed(_) => return Ok(plan.dispatch.clone()),
            AppendResolution::Absent => {}
            AppendResolution::Conflict { .. } => {
                return Err(Denial::new("stale_revision", "Dispatch source changed"));
            }
        }
        let result = self
            .journal
            .append(&plan.session, plan.expected, &plan.events);
        match self
            .journal
            .resolve_append(&plan.session, plan.expected, &plan.events)
        {
            Ok(AppendResolution::Committed(_)) => Ok(plan.dispatch.clone()),
            _ => Err(result.err().unwrap_or_else(|| {
                Denial::new("invocation_uncertain", "Dispatch commit is not confirmed")
            })),
        }
    }
    pub fn resolve_dispatch(
        &self,
        plan: &PreparedInvocation,
    ) -> Result<Option<InvocationDispatch>> {
        if !Arc::ptr_eq(&plan.issuer, &self.issuer) {
            return Err(Denial::new(
                "invocation_plan",
                "Foreign dispatch preparation",
            ));
        }
        match self
            .journal
            .resolve_append(&plan.session, plan.expected, &plan.events)?
        {
            AppendResolution::Committed(_) => Ok(Some(plan.dispatch.clone())),
            AppendResolution::Absent => Ok(None),
            AppendResolution::Conflict { .. } => Err(Denial::new(
                "invocation_uncertain",
                "Cannot resolve the prepared dispatch at a changed boundary",
            )),
        }
    }
    pub fn allowed(&self, token: &GrantToken, invocation: &Id<Invocation>, at: u64) -> Result<()> {
        let view = self.view(token.session())?;
        let source = record(&view, invocation)?;
        crate::finalization::execution_allowed(&view, &source.dispatch.assignment)?;
        let assignment = self.gate.authorize_snapshot(token, None, at, &view)?;
        let task = view
            .task()
            .ok_or_else(|| Denial::new("task_missing", "No invocation task"))?;
        let account = &view.treasury().unwrap().accounts[&source.dispatch.reservation];
        crate::treasury::validate_demand(&view, &account.demand)?;
        if !assignment.access.is_subset(&task.constraints.allowed)
            || task
                .constraints
                .deadline
                .is_some_and(|deadline| at >= deadline)
            || view.coordination().contributions()[&assignment.contribution].contract
                != view.contract().unwrap().reference()
            || view.policies().get("ExecutionBackend") != Some(&source.dispatch.backend)
        {
            return Err(Denial::new(
                "invocation_constraints",
                "Current owner constraints no longer permit this invocation",
            ));
        }
        if source.dispatch.assignment.id != assignment.id
            || source.terminal.is_some()
            || at >= source.dispatch.deadline
        {
            return Err(Denial::new(
                "invocation_stopped",
                "Invocation authority or whole-invocation lifetime ended",
            ));
        }
        if limit_reason(&view, source, at)?.is_some() {
            return Err(Denial::new(
                "invocation_limit",
                "Whole-invocation allowance is exhausted",
            ));
        }
        Ok(())
    }
    pub fn operation_denial(
        &self,
        token: &GrantToken,
        invocation: &Id<Invocation>,
        operation: ymp_domain::assignment::TeamOperation,
        at: u64,
    ) -> Denial {
        if let Err(error) = self.allowed(token, invocation, at) {
            return error;
        }
        match self.gate.authorize(token, operation, at) {
            Err(error) => error,
            Ok(_) => Denial::new(
                "operation_transport_unavailable",
                "Team operation dispatch is not connected yet",
            ),
        }
    }
    fn append(&self, session: &Id, at: u64, payloads: Vec<Event>) -> Result<Ref> {
        let current = self.journal.read(session)?;
        let events = self.gate.events(&current, session, at, payloads)?;
        let end = events.last().unwrap().reference()?;
        let result = self.journal.append(session, current.revision, &events);
        match self
            .journal
            .resolve_append(session, current.revision, &events)
        {
            Ok(AppendResolution::Committed(_)) => Ok(end),
            _ => Err(result.err().unwrap_or_else(|| {
                Denial::new(
                    "invocation_uncertain",
                    "Observation commit is not confirmed",
                )
            })),
        }
    }
    pub fn started(
        &self,
        session: &Id,
        id: &Id<Invocation>,
        called_at: u64,
        at: u64,
        response: &BackendStart,
    ) -> Result<Ref> {
        if response.handle.invocation != *id {
            return Err(Denial::new(
                "invocation_identity",
                "Start returned a foreign invocation",
            ));
        }
        let view = self.view(session)?;
        let source = record(&view, id)?;
        let invocation = Invocation {
            id: id.clone(),
            assignment: source.dispatch.assignment.id.clone(),
            provider: source.dispatch.provider.clone(),
            settings: InvocationSettings {
                requested: source.dispatch.settings.clone(),
                sent: response.sent.clone(),
                reported: response.reported.clone(),
            },
            native_session: response.native_session.clone(),
            started: called_at,
            ended: source.ended_at,
            receipt: None,
            terminal: source.terminal.clone(),
        };
        if let Some(prior) = &source.invocation {
            if prior == &invocation {
                return Ok(source.start.clone().unwrap());
            }
            return Err(Denial::new(
                "invocation_start",
                "Start confirmation changed",
            ));
        }
        self.append(
            session,
            at,
            vec![Event::InvocationStarted {
                version: 1,
                invocation,
            }],
        )
    }
    pub fn observe(&self, session: &Id, at: u64, observed: BackendEvent) -> Result<Ref> {
        let view = self.view(session)?;
        if let Some((prior, reference)) = record(&view, &observed.invocation)?
            .observations
            .get(&observed.sequence)
        {
            return if prior == &observed {
                Ok(reference.clone())
            } else {
                Err(Denial::new(
                    "invocation_identity",
                    "A recorded event sequence changed",
                ))
            };
        }
        self.append(
            session,
            at,
            vec![Event::InvocationObserved {
                version: 1,
                invocation: observed.invocation.clone(),
                observation: Box::new(InvocationObservation::Backend(observed)),
            }],
        )
    }
    pub fn diagnostic(
        &self,
        session: &Id,
        id: &Id<Invocation>,
        at: u64,
        class: ErrorClass,
        code: &str,
        message: &str,
    ) -> Result<Ref> {
        let view = self.view(session)?;
        if let Some((_, _, _, reference)) = record(&view, id)?
            .diagnostics
            .iter()
            .find(|(c, key, text, _)| *c == class && key == code && text == message)
        {
            return Ok(reference.clone());
        }
        self.append(
            session,
            at,
            vec![Event::InvocationObserved {
                version: 1,
                invocation: id.clone(),
                observation: Box::new(InvocationObservation::Diagnostic {
                    class,
                    code: code.into(),
                    message: message.into(),
                }),
            }],
        )
    }
    pub fn revoke(&self, session: &Id, id: &Id<Invocation>, at: u64) -> Result<()> {
        let view = self.view(session)?;
        let source = record(&view, id)?;
        self.gate.revoke(
            session,
            view.revision(),
            at,
            &source.dispatch.assignment.id,
            "Backend terminal observed; completion checks pending".into(),
        )?;
        Ok(())
    }
    pub fn end(
        &self,
        session: &Id,
        id: &Id<Invocation>,
        at: u64,
        terminal: InvocationTerminal,
        confirmed: bool,
    ) -> Result<Ref> {
        let view = self.view(session)?;
        let source = record(&view, id)?;
        if let Some(reference) = &source.end {
            return Ok(reference.clone());
        }
        let mut payloads = crate::gatekeeper::revocation_payloads(
            &view,
            &source.dispatch.assignment.id,
            "Invocation authority ended".into(),
        )?;
        let ended = if confirmed {
            source
                .backend_terminal
                .as_ref()
                .map_or(at, |(_, _, at)| *at)
        } else {
            at
        };
        payloads.push(Event::InvocationEnded {
            version: 1,
            invocation: id.clone(),
            terminal,
            confirmed,
            ended,
        });
        self.append(session, at, payloads)
    }
    pub fn receipt(
        &self,
        session: &Id,
        id: &Id<Invocation>,
        at: u64,
        receipt: Receipt,
    ) -> Result<Ref> {
        let view = self.view(session)?;
        let source = record(&view, id)?;
        if source.receipt.as_ref() == Some(&receipt) {
            return Ok(source.last.clone());
        }
        if receipt.id != source.dispatch.receipt
            || receipt.invocation != id.erased()
            || !receipt.usage.includes(&source.usage)
        {
            return Err(Denial::new(
                "receipt_binding",
                "Receipt is foreign or erases observed usage",
            ));
        }
        let mut payloads = vec![Event::ReservationChanged {
            version: 1,
            change: ReservationChange::Observed {
                reservation: source.dispatch.reservation.clone(),
                receipt: receipt.clone(),
                evidence: vec![],
            },
        }];
        payloads.push(Event::InvocationObserved {
            version: 1,
            invocation: id.clone(),
            observation: Box::new(InvocationObservation::Receipt(receipt)),
        });
        self.append(session, at, payloads)
    }
    pub fn price(
        &self,
        session: &Id,
        id: &Id<Invocation>,
        at: u64,
        response: crate::ports::resources::ResourceResponse<ReceiptPrice>,
    ) -> Result<Ref> {
        let view = self.view(session)?;
        let source = record(&view, id)?;
        let receipt = Digest::of_value(
            source
                .receipt
                .as_ref()
                .ok_or_else(|| Denial::new("receipt_missing", "No usage observation to price"))?,
        )?;
        if let Some((prior, _, reference)) = &source.cost
            && *prior == receipt
        {
            return Ok(reference.clone());
        }
        let effective = view
            .policies()
            .get("CostModel")
            .ok_or_else(|| Denial::new("policy_selection", "No CostModel selected"))?
            .clone();
        self.append(
            session,
            at,
            vec![Event::InvocationObserved {
                version: 1,
                invocation: id.clone(),
                observation: Box::new(InvocationObservation::Cost {
                    receipt,
                    decision: Box::new(Decision {
                        outcome: response.proposal.value,
                        proposal: response.proposal,
                        effective,
                        input: response.input,
                        selection_change: None,
                    }),
                }),
            }],
        )
    }
}

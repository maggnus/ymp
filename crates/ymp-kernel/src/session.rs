//! Trusted session controls. User stop is ordered atomically with authority revocation.
use crate::{
    events::Event,
    journal::{JournalRead, ParameterSchemas},
    view::SessionView,
};
use serde::{Deserialize, Serialize};
use ymp_domain::{
    Denial, Id, Ref, Result,
    assignment::AssignmentState,
    journal::{Actor, Envelope},
    task::SessionStatus,
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionChange {
    Configured(Box<SessionDefinition>),
    Stop,
    Continue,
    Phase(SessionStatus),
    RecoveryConsumed(Ref),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisibleCheck {
    pub criterion: Ref,
    pub spec: ymp_domain::verification::CheckSpec,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionDefinition {
    pub workspace: Id<ymp_domain::workspace::Workspace>,
    pub offer_window_ms: u64,
    pub checks: Vec<VisibleCheck>,
    pub rules: ymp_domain::verification::AssessmentRules,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SessionState {
    pub definition: Option<(Ref, SessionDefinition)>,
    pub stopped: bool,
    pub phase: Option<SessionStatus>,
    pub control: Option<(SessionChange, Ref)>,
    pub recovered: Vec<(Ref, Ref)>,
}
pub(crate) fn apply(
    view: &SessionView,
    event: &Envelope<Event>,
    change: &SessionChange,
) -> Result<SessionState> {
    if view.task().is_none()
        || event.at < view.latest_at()
        || event.input != Some(view.digest()?)
        || event.policy.is_some()
        || event.refs
            != match change {
                SessionChange::RecoveryConsumed(source) => vec![source.clone()],
                _ => vec![],
            }
    {
        return Err(Denial::new(
            "session_control",
            "Session control requires its current recorded task and boundary",
        ));
    }
    let mut next = view.session_state().clone();
    match change {
        SessionChange::Configured(definition) => {
            if next.definition.is_some()
                || view.owner_stopped()
                || !view.workspaces().contains_key(&definition.workspace)
                || definition.offer_window_ms == 0
                || definition.offer_window_ms > 60000
                || definition.checks.is_empty()
                || definition.checks.len() > 128
                || definition
                    .checks
                    .iter()
                    .map(|c| &c.criterion)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != definition.checks.len()
            {
                return Err(Denial::new(
                    "session_definition",
                    "Configure one explicit workspace and distinct fixed visible checks",
                ));
            }
            for check in &definition.checks {
                if !view
                    .criteria()
                    .iter()
                    .any(|criterion| criterion.reference().is_ok_and(|r| r == check.criterion))
                {
                    return Err(Denial::new(
                        "session_check_scope",
                        "A fixed visible check must name an exact current owner criterion",
                    ));
                }
                if ymp_domain::journal::encode(&check.spec)?.len() > 65536 {
                    return Err(Denial::new(
                        "check_spec",
                        "Visible specification exceeds its bound",
                    ));
                }
            }
            next.definition = Some((event.reference()?, (**definition).clone()));
        }
        SessionChange::Stop => {
            if view
                .admission()
                .assignments()
                .values()
                .any(|a| a.intent.assignment.state != AssignmentState::Revoked)
            {
                return Err(Denial::new(
                    "session_stop",
                    "Stop must revoke all old assignment authority in its atomic packet",
                ));
            }
            next.stopped = true;
            next.phase = Some(SessionStatus::Cancelled);
            next.control = Some((change.clone(), event.reference()?));
        }
        SessionChange::Continue => {
            continue_allowed(view)?;
            next.stopped = false;
            next.phase = Some(if view.results().plans().is_empty() {
                SessionStatus::Intake
            } else {
                SessionStatus::Running
            });
            next.control = Some((change.clone(), event.reference()?));
        }
        SessionChange::RecoveryConsumed(source) => {
            let call = view
                .execution()
                .invocations()
                .values()
                .find(|c| c.end.as_ref() == Some(source))
                .ok_or_else(|| {
                    Denial::new(
                        "recovery_completion",
                        "No exact completed recovery invocation",
                    )
                })?;
            if next.recovered.iter().any(|(r, _)| r == source)
                || view.owner_stopped()
                || view.treasury().is_some_and(|b| b.reporting_mode.is_some())
                || view.finalization().delivered.is_some()
                || view
                    .progress()
                    .work(&call.dispatch.assignment.contribution)
                    .is_none()
                || call.terminal != Some(ymp_domain::assignment::InvocationTerminal::Completed)
                || !call.confirmed_terminal
                || !crate::execution::closed(view, &call.dispatch.assignment.id)
            {
                return Err(Denial::new(
                    "recovery_completion",
                    "Only one settled, closed recovery completion may advance active work",
                ));
            }
            next.recovered.push((source.clone(), event.reference()?));
            next.phase = Some(SessionStatus::Running);
        }
        SessionChange::Phase(phase) => {
            if view.owner_stopped()
                || view.finalization().delivered.is_some()
                || !matches!(
                    phase,
                    SessionStatus::Intake
                        | SessionStatus::Running
                        | SessionStatus::Finalizing
                        | SessionStatus::Blocked(_)
                )
            {
                return Err(Denial::new(
                    "session_phase",
                    "Only active session work phases can be recorded here",
                ));
            }
            if let SessionStatus::Blocked(reason) = phase {
                ymp_domain::require_text(reason, 4096)?;
            }
            next.phase = Some(phase.clone());
        }
    }
    Ok(next)
}
fn continue_allowed(view: &SessionView) -> Result<()> {
    if view.finalization().stopped
        || view.finalization().delivered.is_some()
        || view.treasury().is_some_and(|b| b.reporting_mode.is_some())
    {
        return Err(Denial::new(
            "continuation_unavailable",
            "Reporting or terminal finalization cannot be reopened for model work",
        ));
    }
    Ok(())
}
/// Adapter calls this under its aggregate write lock/transaction. No optimistic
/// revision supplied by an owner can make stop lose to unrelated journal writes.
pub fn control_events(
    current: &JournalRead,
    session: &Id,
    at: u64,
    change: SessionChange,
    schemas: &ParameterSchemas,
) -> Result<(Vec<Envelope<Event>>, Ref)> {
    let mut view = current.view_with_schemas(session, None, schemas)?;
    if let SessionChange::RecoveryConsumed(source) = &change
        && let Some((_, prior)) = view
            .session_state()
            .recovered
            .iter()
            .find(|(r, _)| r == source)
    {
        return Ok((vec![], prior.clone()));
    }
    if change == SessionChange::Continue {
        continue_allowed(&view)?;
    }
    if let Some((prior, reference)) = &view.session_state().control
        && *prior == change
        && matches!(change, SessionChange::Stop | SessionChange::Continue)
    {
        return Ok((vec![], reference.clone()));
    }
    let at = at.max(view.latest_at());
    let mut events = vec![];
    if change == SessionChange::Stop {
        let assignments = view
            .admission()
            .assignments()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for assignment in assignments {
            for payload in crate::gatekeeper::revocation_payloads(
                &view,
                &assignment,
                "User stopped autonomous work".into(),
            )? {
                let (policy, input, refs) = match &payload {
                    Event::ReservationChanged { .. } => crate::treasury::attribution(&payload)?,
                    Event::LockChanged { change, .. } => (
                        None,
                        None,
                        crate::workspace_locks::attribution(&view, change)?,
                    ),
                    Event::AssignmentRevoked { assignment, .. } => (
                        None,
                        None,
                        crate::gatekeeper::revocation_refs(&view, assignment)?,
                    ),
                    _ => return Err(Denial::new("session_stop", "Unexpected revocation packet")),
                };
                let event = Envelope {
                    seq: view.revision() + 1,
                    session: session.clone(),
                    at,
                    actor: Actor::Runtime,
                    policy,
                    input,
                    refs,
                    payload,
                };
                view.apply(&event, schemas)?;
                events.push(event);
            }
        }
    }
    let event = Envelope {
        seq: view.revision() + 1,
        session: session.clone(),
        at,
        actor: Actor::Runtime,
        policy: None,
        input: Some(view.digest()?),
        refs: match &change {
            SessionChange::RecoveryConsumed(source) => vec![source.clone()],
            _ => vec![],
        },
        payload: Event::SessionChanged { version: 1, change },
    };
    view.apply(&event, schemas)?;
    view.validate_complete()?;
    let reference = event.reference()?;
    events.push(event);
    Ok((events, reference))
}

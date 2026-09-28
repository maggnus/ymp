//! Trusted opening and method-decision operations for the first journal consumer.

use crate::{
    events::Event,
    journal::{Journal, validate_append},
};
use std::sync::Arc;
use ymp_domain::{
    Denial, Digest, Id, Proposal, Result,
    journal::{Actor, Decision, Envelope, Method, PolicySelection, SelectionChange},
};

/// An in-process owner capability issued at opening, never part of a proposal or journal.
/// Recovery through an authenticated application boundary is added with the session runtime.
pub struct SessionControl {
    session: Id,
    consumer: Arc<()>,
    stopped: std::sync::atomic::AtomicBool,
}

impl SessionControl {
    pub fn session(&self) -> &Id {
        &self.session
    }
    pub fn stopped(&self) -> bool {
        self.stopped.load(std::sync::atomic::Ordering::SeqCst)
    }
    /// Irreversible for this local handle; an explicit Continue issues a new handle.
    pub fn stop_local(&self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
pub struct MethodDecision {
    pub expected_revision: u64,
    pub at: u64,
    pub input: Digest,
    pub proposal: Proposal<Method>,
}

/// Records Method choices; it neither runs strategies nor schedules execution.
pub struct DecisionConsumer<J: Journal> {
    journal: Arc<J>,
    control: Arc<()>,
}

impl<J: Journal> DecisionConsumer<J> {
    pub(crate) fn shared(&self) -> Self {
        Self {
            journal: self.journal.clone(),
            control: self.control.clone(),
        }
    }
    pub fn new(journal: Arc<J>) -> Self {
        Self {
            journal,
            control: Arc::new(()),
        }
    }

    pub fn open(
        &self,
        session: Id,
        at: u64,
        selections: Vec<PolicySelection>,
    ) -> Result<SessionControl> {
        for selection in &selections {
            self.journal.schemas().validate(selection)?;
        }
        let event = Envelope {
            seq: 1,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy: None,
            input: None,
            refs: Vec::new(),
            payload: Event::SessionOpened {
                version: 1,
                selections,
            },
        };
        self.open_events(session, &[event])
    }

    /// Trusted local owner recovery; this API is not exposed through participant ports.
    pub fn recover(&self, session: &Id) -> Result<SessionControl> {
        let view = self.journal.view(session, None)?;
        if !view.opened() || view.task().is_none() {
            return Err(Denial::new(
                "session_missing",
                "Recovery needs an existing owner task",
            ));
        }
        Ok(SessionControl {
            session: session.clone(),
            consumer: self.control.clone(),
            stopped: std::sync::atomic::AtomicBool::new(view.owner_stopped()),
        })
    }
    pub fn change_session(
        &self,
        control: &SessionControl,
        at: u64,
        change: crate::session::SessionChange,
    ) -> Result<ymp_domain::Ref> {
        let session = self.authorize(control)?;
        if change == crate::session::SessionChange::Stop {
            control.stop_local();
        }
        self.journal.session_control(&session, at, change)
    }
    pub(crate) fn authorize(&self, control: &SessionControl) -> Result<Id> {
        if !Arc::ptr_eq(&control.consumer, &self.control) {
            return Err(Denial::new(
                "owner_authority",
                "This control belongs to another application instance",
            ));
        }
        Ok(control.session.clone())
    }

    pub(crate) fn open_events(
        &self,
        session: Id,
        events: &[Envelope<Event>],
    ) -> Result<SessionControl> {
        let current = self.journal.read(&session)?;
        let validated = validate_append(&current, &session, 0, events, self.journal.schemas())?;
        let committed = self.journal.append(&session, 0, events)?;
        if committed != validated.revision() {
            return Err(Denial::new(
                "journal_append",
                "Journal returned an unexpected committed revision",
            ));
        }
        Ok(SessionControl {
            session,
            consumer: self.control.clone(),
            stopped: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn record_method(
        &self,
        session: &Id,
        request: MethodDecision,
        selection: Option<(&SessionControl, PolicySelection)>,
    ) -> Result<u64> {
        let current_read = self.journal.read(session)?;
        let view = current_read.view_with_schemas(session, None, self.journal.schemas())?;
        if view.revision() != request.expected_revision {
            return Err(Denial::new(
                "stale_revision",
                "The decision's work boundary is no longer current",
            ));
        }
        if request.input != view.digest()? {
            return Err(Denial::new(
                "input_view",
                "The proposal's input digest differs from the recorded view",
            ));
        }
        let current = view
            .policies()
            .get("MethodRouter")
            .ok_or_else(|| Denial::new("policy_selection", "No MethodRouter is selected"))?;
        let (effective, selection_change) = match selection {
            None => (current.clone(), None),
            Some((control, next)) => {
                if control.session != *session || !Arc::ptr_eq(&control.consumer, &self.control) {
                    return Err(Denial::new(
                        "owner_authority",
                        "This control does not authorize changes to this session",
                    ));
                }
                self.journal.schemas().validate(&next)?;
                (
                    next,
                    Some(SelectionChange {
                        previous: current.policy.clone(),
                        boundary: view.revision(),
                    }),
                )
            }
        };
        if effective.policy.port != "MethodRouter" {
            return Err(Denial::new(
                "policy_port",
                "MethodChosen requires a MethodRouter policy",
            ));
        }
        let event = Envelope {
            seq: request
                .expected_revision
                .checked_add(1)
                .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?,
            session: session.clone(),
            at: request.at,
            actor: Actor::Runtime,
            policy: Some(effective.policy.clone()),
            input: Some(request.input.clone()),
            refs: request.proposal.basis.clone(),
            payload: Event::MethodChosen {
                version: 2,
                decision: Box::new(Decision {
                    outcome: request.proposal.value.clone(),
                    proposal: request.proposal,
                    effective,
                    input: request.input,
                    selection_change,
                }),
            },
        };
        // Validate in the consumer before calling the adapter. The adapter repeats
        // validation under its lock/transaction to close the concurrent-write race.
        let events = [event];
        let validated = validate_append(
            &current_read,
            session,
            request.expected_revision,
            &events,
            self.journal.schemas(),
        )?;
        let committed = self
            .journal
            .append(session, request.expected_revision, &events)?;
        if committed != validated.revision() {
            return Err(Denial::new(
                "journal_append",
                "Journal returned an unexpected committed revision",
            ));
        }
        Ok(committed)
    }
}

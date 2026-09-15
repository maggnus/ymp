//! Replay projection of execution state and accounting.
//!
//! [`replay_execution`] rebuilds the per-invocation views and the session
//! accounting from journal entries that
//! [`replay_session`](crate::replay_session) has already validated. The
//! journal is the only durable record: nothing here derives from any other
//! source.

use std::collections::BTreeMap;

use super::accounting::{AccountingFold, SessionAccounting};
use super::types::{
    Assignment, ExecutionProfile, InvocationStatus, ObservedUsage, Settings, Termination,
    UncertaintyCause,
};
use crate::{HistoryError, JournalEntry, Revision, SessionEvent, SessionId, replay_session};

/// The replayed state of one invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationView {
    assignment: Assignment,
    start_attempted: bool,
    started: bool,
    cancel_requested: bool,
    uncertain: bool,
    cause: UncertaintyCause,
    failed_at_start: Option<super::types::ErrorClass>,
    effect_evidence: bool,
    termination: Option<Termination>,
    reported_settings: Option<Settings>,
    observed_usage: Option<ObservedUsage>,
    settled_usage: Option<ObservedUsage>,
}

impl InvocationView {
    pub fn assignment(&self) -> &Assignment {
        &self.assignment
    }

    /// Whether a start attempt is journaled whose outcome has not resolved
    /// yet. The invocation is still in its admitted phase; a retry resolves
    /// the attempt through the backend's idempotent start.
    pub fn start_attempted(&self) -> bool {
        self.start_attempted && !self.started
    }

    /// The derived lifecycle status: `terminated` on the termination
    /// observation, `failed` on a confirmed never-started start failure
    /// (terminal without a termination observation), `uncertain` once the
    /// kernel recorded it, `cancelling` after a cancellation request, then
    /// `started` and `admitted`.
    pub fn status(&self) -> InvocationStatus {
        if self.termination.is_some() {
            InvocationStatus::Terminated
        } else if self.failed_at_start.is_some() {
            InvocationStatus::Failed
        } else if self.uncertain {
            InvocationStatus::Uncertain
        } else if self.cancel_requested {
            InvocationStatus::Cancelling
        } else if self.started {
            InvocationStatus::Started
        } else {
            InvocationStatus::Admitted
        }
    }

    /// Whether this assignment is live: not yet terminated or failed.
    /// Live assignments make a new assignment for the same agent or role
    /// not independent.
    pub fn is_live(&self) -> bool {
        self.termination.is_none() && self.failed_at_start.is_none()
    }

    /// The error class of a confirmed never-started start failure, if any.
    pub fn failure_class(&self) -> Option<&super::types::ErrorClass> {
        self.failed_at_start.as_ref()
    }

    /// Whether effect evidence was recorded: the writes-ended observation
    /// released the workspace hold while the invocation itself remains
    /// `uncertain` with its reservation held.
    pub fn effect_evidence_recorded(&self) -> bool {
        self.effect_evidence
    }

    /// The cause recorded with the uncertainty, if the kernel recorded one.
    pub fn uncertainty_cause(&self) -> Option<UncertaintyCause> {
        self.uncertain.then_some(self.cause)
    }

    pub fn termination(&self) -> Option<&Termination> {
        self.termination.as_ref()
    }

    /// The execution profile: requested and sent settings are recorded in
    /// the admission batch, and reported settings join them at the
    /// termination observation. All three stay separate; `reported` is
    /// unknown until the backend reports.
    pub fn profile(&self) -> ExecutionProfile {
        ExecutionProfile::new(
            self.assignment.requested_settings().clone(),
            self.assignment.sent_settings().clone(),
            self.reported_settings.clone(),
        )
    }

    pub fn cancel_requested(&self) -> bool {
        self.cancel_requested
    }

    pub fn observed_usage(&self) -> Option<&ObservedUsage> {
        self.observed_usage.as_ref()
    }

    /// The usage recorded by the settlement; present once accounted.
    pub fn settled_usage(&self) -> Option<&ObservedUsage> {
        self.settled_usage.as_ref()
    }

    /// Whether the invocation still holds its reservation and workspace
    /// hold: everything admitted but not yet accounted.
    pub fn holds_reservation(&self) -> bool {
        self.settled_usage.is_none()
    }
}

/// The replayed execution state of one session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionExecutionView {
    session_id: SessionId,
    revision: Revision,
    invocations: BTreeMap<super::types::InvocationId, InvocationView>,
    accounting: SessionAccounting,
}

impl SessionExecutionView {
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn invocations(
        &self,
    ) -> impl Iterator<Item = (&super::types::InvocationId, &InvocationView)> {
        self.invocations.iter()
    }

    pub fn invocation(&self, invocation: &super::types::InvocationId) -> Option<&InvocationView> {
        self.invocations.get(invocation)
    }

    pub fn invocation_count(&self) -> usize {
        self.invocations.len()
    }

    pub fn accounting(&self) -> &SessionAccounting {
        &self.accounting
    }
}

/// Rebuilds the execution view from journal entries.
///
/// The entries are validated first by
/// [`replay_session`](crate::replay_session), whose rules cover the
/// invocation lifecycle; the projection then simply follows the events.
/// An empty history yields an empty view at the initial revision.
pub fn replay_execution(
    stream_id: &SessionId,
    entries: &[JournalEntry],
) -> Result<SessionExecutionView, HistoryError> {
    let session = replay_session(stream_id, entries)?;
    let revision = session
        .as_ref()
        .map_or(Revision::INITIAL, |view| view.revision());

    let mut invocations: BTreeMap<super::types::InvocationId, InvocationView> = BTreeMap::new();
    let mut fold = AccountingFold::new();

    for entry in entries {
        let event = entry.event();
        fold.apply(event);
        match event {
            SessionEvent::AssignmentAdmitted {
                invocation,
                assignment,
                ..
            } => {
                invocations.insert(
                    invocation.clone(),
                    InvocationView {
                        assignment: assignment.clone(),
                        start_attempted: false,
                        started: false,
                        cancel_requested: false,
                        uncertain: false,
                        cause: UncertaintyCause::BoundedWaitExpired,
                        failed_at_start: None,
                        effect_evidence: false,
                        termination: None,
                        reported_settings: None,
                        observed_usage: None,
                        settled_usage: None,
                    },
                );
            }
            SessionEvent::InvocationStartAttempted { invocation, .. } => {
                if let Some(view) = invocations.get_mut(invocation) {
                    view.start_attempted = true;
                }
            }
            SessionEvent::InvocationStarted { invocation, .. } => {
                if let Some(view) = invocations.get_mut(invocation) {
                    view.started = true;
                }
            }
            SessionEvent::InvocationUncertain {
                invocation, cause, ..
            } => {
                if let Some(view) = invocations.get_mut(invocation) {
                    view.uncertain = true;
                    view.cause = *cause;
                }
            }
            SessionEvent::InvocationCancellationRequested { invocation, .. } => {
                if let Some(view) = invocations.get_mut(invocation) {
                    view.cancel_requested = true;
                }
            }
            SessionEvent::InvocationFailedAtStart {
                invocation, class, ..
            } => {
                if let Some(view) = invocations.get_mut(invocation) {
                    view.failed_at_start = Some(class.clone());
                }
            }
            SessionEvent::EffectEvidenceRecorded { invocation, .. } => {
                if let Some(view) = invocations.get_mut(invocation) {
                    view.effect_evidence = true;
                }
            }
            SessionEvent::InvocationObserved {
                invocation,
                termination,
                reported_settings,
                usage,
                ..
            } => {
                if let Some(view) = invocations.get_mut(invocation) {
                    view.termination = Some(termination.clone());
                    view.reported_settings = reported_settings.clone();
                    view.observed_usage = Some(*usage);
                }
            }
            SessionEvent::InvocationAccounted {
                invocation, usage, ..
            } => {
                if let Some(view) = invocations.get_mut(invocation) {
                    view.settled_usage = Some(*usage);
                }
            }
            SessionEvent::SessionOpened { .. } | SessionEvent::SessionCancelled { .. } => {}
        }
    }

    Ok(SessionExecutionView {
        session_id: stream_id.clone(),
        revision,
        invocations,
        accounting: fold.accounting().clone(),
    })
}

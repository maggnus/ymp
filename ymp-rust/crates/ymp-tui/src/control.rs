//! Owner control of a loaded session: its current team and the recovery of stopped work.
//!
//! Nothing here decides what the owner may do. The runtime's typed read model names the actions
//! a stage supports, and every command goes to the trusted local owner API, which validates it
//! again. [`Control::take_read`] and [`Control::take_request`] hand the event loop one read and
//! one command at a time. Each reply carries the exact request it answers, so a reply for a
//! session that is no longer displayed never replaces what is shown, and a command whose outcome
//! did not arrive can be sent again unchanged.

use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use ymp_core::{
    Config, ContinueWithCurrentFilesCommand, CurrentFilesAuthorization, CurrentFilesContext,
    CurrentFilesReceipt, FreshPlanReviewCommand, FreshPlanReviewReceipt, OwnerTeamAction,
    OwnerTeamCommand, OwnerTeamReceipt, RecoveryControlCommand, RecoveryControlReceipt,
    RecoveryInspectionCommand, RecoveryInspectionReceipt, RecoveryStage, TeamControlView,
};
use ymp_runtime::Engine;
use ymp_storage::Store;

/// The shortest gap between two reads of the session team while a run keeps reporting.
const READ_GAP: Duration = Duration::from_millis(500);

/// What the controller last read about the displayed session's team.
#[derive(Debug, Default)]
pub enum Read {
    #[default]
    Unread,
    Loading,
    /// The session recorded no team. Its membership cannot be changed, and starting
    /// preferences are not a substitute for it.
    NotInitialized,
    Ready(Box<Snapshot>),
    Failed(String),
}

/// One read of the backend model, with the records it needs beside it.
#[derive(Debug)]
pub struct Snapshot {
    pub view: TeamControlView,
    /// Agents the owner removed, which stay out of automatic selection until chosen again.
    pub excluded: Vec<String>,
    pub authorizations: Vec<CurrentFilesAuthorization>,
    pub read_at: String,
}

impl Snapshot {
    pub fn stage(&self, id: &str) -> Option<&RecoveryStage> {
        self.view
            .recovery_stages
            .iter()
            .find(|stage| stage.id == id)
    }

    /// The recorded current-files decision that already acknowledges every failure of `stage`.
    pub fn authorization(&self, stage: &RecoveryStage) -> Option<&CurrentFilesAuthorization> {
        self.authorizations.iter().find(|authorization| {
            let context = &authorization.command.context;
            context.session_id == stage.session_id
                && context.stage_id == stage.id
                && stage
                    .failures
                    .iter()
                    .all(|failure| context.failures.contains(failure))
        })
    }
}

/// One piece of work for the owner API.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    Read { generation: u64, session: String },
    Team(OwnerTeamCommand),
    Stage(RecoveryControlCommand),
    Review(Box<FreshPlanReviewCommand>),
    Inspect(RecoveryInspectionCommand),
    Context { session: String, stage: String },
    Authorize(Box<ContinueWithCurrentFilesCommand>),
}

impl Request {
    pub fn session(&self) -> &str {
        match self {
            Request::Read { session, .. } | Request::Context { session, .. } => session,
            Request::Team(command) => &command.session_id,
            Request::Stage(command) => &command.session_id,
            Request::Review(command) => &command.session_id,
            Request::Inspect(command) => &command.session_id,
            Request::Authorize(command) => &command.context.session_id,
        }
    }

    /// The backend takes the session's working directory for this work, so no run and no branch
    /// switch may start beside it.
    pub fn holds_workspace(&self) -> bool {
        matches!(
            self,
            Request::Review(_) | Request::Inspect(_) | Request::Authorize(_)
        )
    }

    /// Whether the work can call a model, and so can be stopped with /stop.
    pub fn cancellable(&self) -> bool {
        matches!(self, Request::Review(_) | Request::Inspect(_))
    }

    /// What the status row says while this is running.
    pub fn activity(&self) -> &'static str {
        match self {
            Request::Read { .. } => "Reading the session team",
            Request::Team(_) => "Changing the session team",
            Request::Stage(_) => "Recording the stage decision",
            Request::Review(_) => "Reviewing the saved plan",
            Request::Inspect(_) => "Inspecting recorded effects",
            Request::Context { .. } => "Reading current files",
            Request::Authorize(_) => "Authorizing continuation with current files",
        }
    }
}

/// The owner API's answer to a [`Request`].
#[derive(Debug)]
pub enum Outcome {
    /// `None` when the session has no initialized team.
    Read(Result<Option<Box<Snapshot>>, String>),
    Team(Result<OwnerTeamReceipt, String>),
    Stage(Result<RecoveryControlReceipt, String>),
    Review(Result<FreshPlanReviewReceipt, String>),
    Inspect(Result<RecoveryInspectionReceipt, String>),
    Context(Result<CurrentFilesContext, String>),
    Authorize(Result<CurrentFilesReceipt, String>),
}

impl Outcome {
    fn failed(request: &Request, error: String) -> Self {
        match request {
            Request::Read { .. } => Outcome::Read(Err(error)),
            Request::Team(_) => Outcome::Team(Err(error)),
            Request::Stage(_) => Outcome::Stage(Err(error)),
            Request::Review(_) => Outcome::Review(Err(error)),
            Request::Inspect(_) => Outcome::Inspect(Err(error)),
            Request::Context { .. } => Outcome::Context(Err(error)),
            Request::Authorize(_) => Outcome::Authorize(Err(error)),
        }
    }
}

#[derive(Debug)]
pub struct Reply {
    pub request: Request,
    pub outcome: Outcome,
}

/// Perform one request against the trusted local owner API.
///
/// Synchronous reads, hashing and storage transactions run on a blocking thread; a review or an
/// inspection is awaited under `cancel`, which stops its native call. The engine gets an event
/// channel of its own, so work for one session never retargets the conversation on display.
pub async fn perform(
    request: Request,
    store: Store,
    config: Config,
    cancel: CancellationToken,
) -> Reply {
    let (events, _unread) = tokio::sync::mpsc::unbounded_channel();
    match Engine::new(store.clone(), config, events, cancel) {
        Ok(engine) => perform_with(request, store, engine).await,
        Err(error) => {
            let outcome = Outcome::failed(&request, format!("{error:#}"));
            Reply { request, outcome }
        }
    }
}

/// [`perform`] with an engine the caller built, such as one with a compiled test backend.
pub async fn perform_with(request: Request, store: Store, engine: Engine) -> Reply {
    let outcome = match request.clone() {
        Request::Read { session, .. } => {
            Outcome::Read(blocking(move || read(&store, &engine, &session)).await)
        }
        Request::Team(command) => {
            Outcome::Team(blocking(move || engine.owner_team_command(&command)).await)
        }
        Request::Stage(command) => {
            Outcome::Stage(blocking(move || engine.control_recovery(&command)).await)
        }
        // Boxed: a native review is a deep future, and this one stays small wherever it is polled.
        Request::Review(command) => Outcome::Review(
            Box::pin(engine.review_saved_plan_fresh(&command))
                .await
                .map_err(|error| format!("{error:#}")),
        ),
        Request::Inspect(command) => Outcome::Inspect(
            Box::pin(engine.inspect_recovery(&command))
                .await
                .map_err(|error| format!("{error:#}")),
        ),
        Request::Context { session, stage } => {
            Outcome::Context(blocking(move || engine.current_files_context(&session, &stage)).await)
        }
        Request::Authorize(command) => {
            Outcome::Authorize(blocking(move || engine.continue_with_current_files(&command)).await)
        }
    };
    Reply { request, outcome }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> Result<T, String> {
    match tokio::task::spawn_blocking(work).await {
        Ok(result) => result.map_err(|error| format!("{error:#}")),
        Err(error) => Err(format!("The owner task failed: {error}")),
    }
}

/// Read the backend model. Whether a team exists is read from its own record rather than inferred
/// from the text of an error.
fn read(store: &Store, engine: &Engine, session: &str) -> anyhow::Result<Option<Box<Snapshot>>> {
    if store.team_state(session)?.is_none() {
        return Ok(None);
    }
    let view = engine.team_control(session)?;
    let excluded = store
        .owner_team_state(session)?
        .map(|owner| owner.excluded_members)
        .unwrap_or_default();
    let authorizations = engine.current_files_authorizations(session)?;
    Ok(Some(Box::new(Snapshot {
        view,
        excluded,
        authorizations,
        read_at: ymp_core::now(),
    })))
}

/// The displayed session's control state, and the owner work admitted for it.
#[derive(Debug, Default)]
pub struct Control {
    session: Option<String>,
    generation: u64,
    pub read: Read,
    read_wanted: bool,
    reading: bool,
    last_read: Option<Instant>,
    outbox: Option<Request>,
    pending: Option<Request>,
    /// The exact command whose outcome did not arrive as a success. Sending it again is
    /// idempotent: a command the backend already handled returns its recorded outcome.
    pub retry: Option<Request>,
    /// A recorded current-files authorization whose run has not started.
    pub continuation: Option<CurrentFilesReceipt>,
    /// A membership change typed before the session team was read, sent once it has been.
    pub deferred: Option<(String, OwnerTeamAction)>,
}

impl Control {
    /// Follow the displayed session. A different session starts a new generation, so replies
    /// for the previous one no longer change what is shown.
    pub fn bind(&mut self, session: Option<&str>) {
        if self.session.as_deref() == session {
            return;
        }
        self.session = session.map(str::to_owned);
        self.generation += 1;
        self.read = Read::Unread;
        self.read_wanted = false;
        self.deferred = None;
    }

    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    pub fn want_read(&mut self) {
        if self.session.is_some() {
            self.read_wanted = true;
        }
    }

    /// The read the event loop should start now, if any.
    pub fn take_read(&mut self, now: Instant) -> Option<Request> {
        let session = self.session.clone()?;
        if !self.read_wanted
            || self.reading
            || self
                .last_read
                .is_some_and(|last| now.saturating_duration_since(last) < READ_GAP)
        {
            return None;
        }
        self.read_wanted = false;
        self.reading = true;
        self.last_read = Some(now);
        if !matches!(self.read, Read::Ready(_)) {
            self.read = Read::Loading;
        }
        Some(Request::Read {
            generation: self.generation,
            session,
        })
    }

    /// Apply a read. Returns whether it describes the displayed session.
    pub fn receive_read(
        &mut self,
        generation: u64,
        session: &str,
        result: Result<Option<Box<Snapshot>>, String>,
    ) -> bool {
        self.reading = false;
        if generation != self.generation || self.session.as_deref() != Some(session) {
            return false;
        }
        self.read = match result {
            Ok(Some(snapshot)) => Read::Ready(snapshot),
            Ok(None) => Read::NotInitialized,
            Err(error) => Read::Failed(error),
        };
        true
    }

    /// The read ended without a reply.
    pub fn read_abandoned(&mut self, reason: String) {
        self.reading = false;
        if !matches!(self.read, Read::Ready(_)) {
            self.read = Read::Failed(reason);
        }
    }

    /// The snapshot of the displayed session, when one has been read.
    pub fn ready(&self) -> Option<&Snapshot> {
        match &self.read {
            Read::Ready(snapshot) if Some(snapshot.view.session_id.as_str()) == self.session() => {
                Some(snapshot)
            }
            _ => None,
        }
    }

    /// Queue one command. Only one runs at a time.
    pub fn submit(&mut self, request: Request) -> Result<(), String> {
        if let Some(busy) = self.pending.as_ref().or(self.outbox.as_ref()) {
            return Err(format!("{} is still in progress.", busy.activity()));
        }
        self.outbox = Some(request);
        Ok(())
    }

    /// The command the event loop should start now, if any.
    pub fn take_request(&mut self) -> Option<Request> {
        let request = self.outbox.take()?;
        self.pending = Some(request.clone());
        Some(request)
    }

    /// The command in progress, or queued to start.
    pub fn pending(&self) -> Option<&Request> {
        self.pending.as_ref().or(self.outbox.as_ref())
    }

    pub fn finish(&mut self) {
        self.pending = None;
    }

    /// Owner work holds the session's working directory now or is about to.
    pub fn holds_workspace(&self) -> bool {
        self.pending().is_some_and(Request::holds_workspace)
    }

    /// The queued command is withdrawn before it started, as when the window closes.
    pub fn withdraw(&mut self) -> Option<Request> {
        self.outbox.take()
    }
}

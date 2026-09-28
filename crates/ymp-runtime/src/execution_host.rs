//! Bounded supervision of one admitted invocation. Live capabilities never come from replay.
use crate::{clock::Clock, policies::resources::cost_response};
use std::{
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, TryRecvError, sync_channel},
    },
};
use ymp_domain::{
    Denial, Digest, Id, Result,
    assignment::{
        Assignment, ContributionKind, ErrorClass, Invocation, InvocationTerminal, Prompt,
    },
    resources::{Coverage, Receipt, ReservationState},
    workspace::WorkspacePath,
};
use ymp_kernel::{
    arbiter::Arbiter,
    execution::{
        Execution, InvocationDispatch, InvocationRecord, PreparedInvocation, limit_reason,
    },
    gatekeeper::{AdmittedAssignment, Gatekeeper, GrantToken},
    journal::{ContentStore, Journal},
    ports::{execution::*, resources::CostModel},
    treasury::cost_view,
    workspace_guard::{CessationEvidence, MediatedAccess},
};
struct Credentials<J: Journal> {
    assignment: Assignment,
    grant: GrantToken,
    files: Option<Arc<MediatedAccess<J>>>,
    owner: Option<Arc<ymp_kernel::decision::SessionControl>>,
}
struct Files<J: Journal, C: ContentStore> {
    execution: Arc<Execution<J, C>>,
    credentials: Arc<Credentials<J>>,
    invocation: Id<Invocation>,
    clock: Arc<dyn Clock>,
}
impl<J: Journal, C: ContentStore> InvocationFiles for Files<J, C> {
    fn read(&self, path: &WorkspacePath, limit: usize) -> Result<Vec<u8>> {
        if self
            .credentials
            .owner
            .as_ref()
            .is_some_and(|owner| owner.stopped())
        {
            return Err(Denial::new("session_stopped", "Owner stopped local work"));
        }
        let at = self.clock.now()?;
        self.execution
            .allowed(&self.credentials.grant, &self.invocation, at)?;
        self.credentials
            .files
            .as_ref()
            .unwrap()
            .read(path, limit, at)
    }
    fn write(&self, path: &WorkspacePath, bytes: &[u8]) -> Result<()> {
        if self
            .credentials
            .owner
            .as_ref()
            .is_some_and(|owner| owner.stopped())
        {
            return Err(Denial::new("session_stopped", "Owner stopped local work"));
        }
        let at = self.clock.now()?;
        self.execution
            .allowed(&self.credentials.grant, &self.invocation, at)?;
        self.credentials
            .files
            .as_ref()
            .unwrap()
            .write(path, bytes, at)
    }
}
struct Control<J: Journal, C: ContentStore> {
    execution: Arc<Execution<J, C>>,
    credentials: Arc<Credentials<J>>,
    invocation: Id<Invocation>,
    clock: Arc<dyn Clock>,
    stopped: Arc<AtomicBool>,
}
impl<J: Journal, C: ContentStore> InvocationControl for Control<J, C> {
    fn before_inference(&self) -> Result<()> {
        if self.stopped.load(Ordering::SeqCst)
            || self
                .credentials
                .owner
                .as_ref()
                .is_some_and(|owner| owner.stopped())
        {
            return Err(Denial::new(
                "invocation_stopped",
                "Host stopped before native inference",
            ));
        }
        self.execution
            .allowed(&self.credentials.grant, &self.invocation, self.clock.now()?)
    }
}
enum Reply {
    Started {
        called_at: u64,
        response: Result<BackendStart>,
    },
    Events(Result<Vec<BackendEvent>>),
}
type WithdrawalReply = (CessationEvidence, Option<Denial>);
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionStatus {
    Prepared,
    Running,
    WaitingForReceipt,
    WaitingForObservation,
    Blocked(String),
    Finished,
}
/// Caller-owned, non-serializable state. Keep this object when a journal reply is
/// lost: it retains the exact preparation and any uncommitted backend response.
pub struct LiveInvocation<J: Journal> {
    issuer: Arc<()>,
    credentials: Arc<Credentials<J>>,
    id: Id<Invocation>,
    receipt: Id<Receipt>,
    prompt: Prompt,
    previous: Option<InvocationContinuation>,
    preparation: Option<PreparedInvocation>,
    dispatch: Option<InvocationDispatch>,
    attempted: bool,
    stopped: Arc<AtomicBool>,
    handle: Option<ExecutionHandle>,
    call: Option<Receiver<Reply>>,
    pending: Option<Reply>,
    events: VecDeque<BackendEvent>,
    cancel: Option<Receiver<Result<()>>>,
    cancel_sent: bool,
    withdrawal: Option<Receiver<Result<WithdrawalReply>>>,
    withdrawal_proof: Option<CessationEvidence>,
    withdrawal_started: bool,
    capture_retry: bool,
    files_closed: bool,
    receipt_call: Option<Receiver<Result<Receipt>>>,
    pending_receipt: Option<Receipt>,
    receipt_observed: bool,
    blocked: Option<String>,
    pending_stop: Option<(InvocationTerminal, bool)>,
    pending_diagnostic: Option<(ErrorClass, String, String)>,
    replies: VecDeque<(String, String)>,
    reply_call: Option<Receiver<Result<()>>>,
}
impl<J: Journal> LiveInvocation<J> {
    pub fn id(&self) -> &Id<Invocation> {
        &self.id
    }
    pub fn session(&self) -> &Id {
        self.credentials.grant.session()
    }
    pub(crate) fn cleanup_pending(&self) -> bool {
        self.withdrawal.is_some()
            || self.capture_retry
            || self.withdrawal_proof.is_some()
            || self.receipt_call.is_some()
            || self.pending_receipt.is_some()
    }
    pub fn dispatched(&self) -> bool {
        self.attempted
    }
}
impl<J: Journal> Drop for LiveInvocation<J> {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(files) = &self.credentials.files {
            files.request_withdrawal();
        }
        // Dropping the controller records no termination, no receipt and no release.
    }
}
pub struct ExecutionHost<J: Journal, C: ContentStore> {
    execution: Arc<Execution<J, C>>,
    arbiter: Arbiter<J>,
    backend: Arc<dyn ExecutionBackend>,
    cost: Arc<dyn CostModel>,
    clock: Arc<dyn Clock>,
    issuer: Arc<()>,
    owner: Option<Arc<ymp_kernel::decision::SessionControl>>,
}
fn call<T: Send + 'static>(operation: impl FnOnce() -> T + Send + 'static) -> Result<Receiver<T>> {
    let (sender, receiver) = sync_channel(1);
    std::thread::Builder::new()
        .name("ymp-invocation".into())
        .spawn(move || {
            let result = operation();
            let _ = sender.send(result);
        })
        .map_err(|_| {
            Denial::new(
                "execution_worker",
                "Cannot start a bounded execution worker",
            )
        })?;
    Ok(receiver)
}
fn protected<T>(operation: impl FnOnce() -> Result<T>) -> Result<T> {
    catch_unwind(AssertUnwindSafe(operation))
        .map_err(|_| Denial::new("backend_panic", "Backend call panicked"))?
}
impl<J: Journal + 'static, C: ContentStore + 'static> ExecutionHost<J, C> {
    pub fn new(
        journal: Arc<J>,
        gate: Arc<Gatekeeper<J, C>>,
        backend: Arc<dyn ExecutionBackend>,
        cost: Arc<dyn CostModel>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self> {
        Ok(Self {
            execution: Arc::new(Execution::new(journal.clone(), gate)?),
            arbiter: Arbiter::new(journal),
            backend,
            cost,
            clock,
            issuer: Arc::new(()),
            owner: None,
        })
    }
    pub(crate) fn controlled(mut self, owner: Arc<ymp_kernel::decision::SessionControl>) -> Self {
        self.owner = Some(owner);
        self
    }
    pub fn attach(
        &self,
        admitted: AdmittedAssignment<J>,
        id: Id<Invocation>,
        receipt: Id<Receipt>,
        prompt: Prompt,
    ) -> Result<LiveInvocation<J>> {
        if self
            .owner
            .as_ref()
            .is_some_and(|owner| owner.session() != admitted.grant.session() || owner.stopped())
        {
            return Err(Denial::new(
                "owner_authority",
                "Controlled host belongs to another or stopped session",
            ));
        }
        let canonical = self
            .execution
            .validate_admitted(&admitted, &id, self.clock.now()?)?;
        Ok(LiveInvocation {
            issuer: self.issuer.clone(),
            credentials: Arc::new(Credentials {
                assignment: canonical,
                grant: admitted.grant,
                files: admitted.files.map(Arc::new),
                owner: self.owner.clone(),
            }),
            id,
            receipt,
            prompt,
            previous: None,
            preparation: None,
            dispatch: None,
            attempted: false,
            stopped: Arc::new(AtomicBool::new(false)),
            handle: None,
            call: None,
            pending: None,
            events: VecDeque::new(),
            cancel: None,
            cancel_sent: false,
            withdrawal: None,
            withdrawal_proof: None,
            withdrawal_started: false,
            capture_retry: false,
            files_closed: false,
            receipt_call: None,
            pending_receipt: None,
            receipt_observed: false,
            blocked: None,
            pending_stop: None,
            pending_diagnostic: None,
            replies: VecDeque::new(),
            reply_call: None,
        })
    }
    /// Continue a completed native thread through a fresh admitted invocation.
    pub fn attach_continuing(
        &self,
        admitted: AdmittedAssignment<J>,
        id: Id<Invocation>,
        receipt: Id<Receipt>,
        prompt: Prompt,
        previous: InvocationContinuation,
    ) -> Result<LiveInvocation<J>> {
        let mut live = self.attach(admitted, id, receipt, prompt)?;
        live.previous = Some(previous);
        Ok(live)
    }
    fn own(&self, live: &LiveInvocation<J>) -> Result<()> {
        if !Arc::ptr_eq(&self.issuer, &live.issuer) {
            return Err(Denial::new(
                "invocation_owner",
                "Live invocation belongs to another host",
            ));
        }
        Ok(())
    }
    pub fn snapshot(&self, live: &LiveInvocation<J>) -> Result<InvocationRecord> {
        self.own(live)?;
        self.execution
            .view(live.session())?
            .execution()
            .invocations()
            .get(&live.id)
            .cloned()
            .ok_or_else(|| Denial::new("invocation_missing", "No committed dispatch"))
    }
    pub fn start(&self, live: &mut LiveInvocation<J>) -> Result<()> {
        self.own(live)?;
        if self.owner.as_ref().is_some_and(|owner| owner.stopped()) {
            return Err(Denial::new(
                "session_stopped",
                "Owner stopped before dispatch",
            ));
        }
        if live.attempted || live.stopped.load(Ordering::SeqCst) {
            return Err(Denial::new(
                "invocation_duplicate",
                "This live invocation cannot start again",
            ));
        }
        if live.preparation.is_none() {
            live.preparation = Some(self.execution.prepare_continuing(
                &live.credentials.grant,
                live.id.clone(),
                live.receipt.clone(),
                live.prompt.clone(),
                self.backend.selection().clone(),
                live.credentials.files.as_deref(),
                self.clock.now()?,
                live.previous.clone(),
            )?);
        }
        let dispatch = self
            .execution
            .dispatch(live.preparation.as_ref().unwrap())?;
        live.dispatch = Some(dispatch.clone());
        // Set before creating the worker. Failure here remains conservative: the
        // recorded dispatch is never used as permission for another backend call.
        live.attempted = true;
        let credentials = live.credentials.clone();
        let backend = self.backend.clone();
        let clock = self.clock.clone();
        let execution = self.execution.clone();
        let stopped = live.stopped.clone();
        let files = live.credentials.files.as_ref().map(|_| {
            Arc::new(Files {
                execution: self.execution.clone(),
                credentials: live.credentials.clone(),
                invocation: live.id.clone(),
                clock: self.clock.clone(),
            }) as Arc<dyn InvocationFiles>
        });
        let control = Arc::new(Control {
            execution: self.execution.clone(),
            credentials: live.credentials.clone(),
            invocation: live.id.clone(),
            clock: self.clock.clone(),
            stopped: live.stopped.clone(),
        });
        live.call = Some(call(move || {
            let observed_time = clock.now();
            let called_at = observed_time.as_ref().copied().unwrap_or(0);
            let response = protected(|| {
                let called_at = observed_time?;
                if stopped.load(Ordering::SeqCst) {
                    return Err(Denial::new(
                        "invocation_stopped",
                        "Host stopped before backend call",
                    ));
                }
                control.before_inference()?;
                execution.allowed(&credentials.grant, &dispatch.invocation, called_at)?;
                backend.start(&ExecutionRequest {
                    control,
                    previous: dispatch.previous,
                    invocation: dispatch.invocation,
                    receipt: dispatch.receipt,
                    assignment: dispatch.assignment,
                    prompt: dispatch.prompt,
                    settings: dispatch.settings,
                    grant: &credentials.grant,
                    allowance: dispatch.allowance,
                    files,
                })
            });
            Reply::Started {
                called_at,
                response,
            }
        })?);
        Ok(())
    }
    fn stop(
        &self,
        live: &mut LiveInvocation<J>,
        terminal: InvocationTerminal,
        confirmed: bool,
    ) -> Result<()> {
        live.stopped.store(true, Ordering::SeqCst);
        if let Some(files) = &live.credentials.files {
            files.request_withdrawal();
        }
        live.pending_stop.get_or_insert((terminal, confirmed));
        self.request_cleanup(live)?;
        let (terminal, confirmed) = live.pending_stop.as_ref().unwrap();
        self.execution.end(
            live.session(),
            &live.id,
            self.clock.now()?,
            terminal.clone(),
            *confirmed,
        )?;
        live.pending_stop = None;
        Ok(())
    }
    pub fn cancel(&self, live: &mut LiveInvocation<J>) -> Result<()> {
        self.own(live)?;
        live.stopped.store(true, Ordering::SeqCst);
        if let Some(files) = &live.credentials.files {
            files.request_withdrawal();
        }
        if live.dispatch.is_none()
            && let Some(plan) = &live.preparation
        {
            live.dispatch = self.execution.resolve_dispatch(plan)?;
        }
        if live.dispatch.is_none() {
            // There was no dispatch: existing kernel evidence can still establish
            // that neither the financial nor path capability was authorized.
            let view = self.execution.view(live.session())?;
            let source = &view.admission().assignments()[&live.credentials.assignment.id];
            let reservation = source.reservation.clone();
            self.execution.gatekeeper().revoke(
                live.session(),
                view.revision(),
                self.clock.now()?,
                &live.credentials.assignment.id,
                "Cancelled before invocation dispatch".into(),
            )?;
            let proof = self
                .execution
                .treasury()
                .never_started(live.session(), &reservation)?;
            let view = self.execution.view(live.session())?;
            self.execution.treasury().release_unstarted(
                live.session(),
                view.revision(),
                self.clock.now()?,
                &proof,
            )?;
            live.stopped.store(true, Ordering::SeqCst);
            return self.request_cleanup(live);
        }
        self.stop(live, InvocationTerminal::Cancelled, false)
    }
    fn request_cleanup(&self, live: &mut LiveInvocation<J>) -> Result<()> {
        if let Some(files) = &live.credentials.files {
            files.request_withdrawal();
            if !live.files_closed && !live.withdrawal_started {
                live.withdrawal_started = true;
                let files = files.clone();
                let execution = self.execution.clone();
                let session = live.session().clone();
                let assignment = live.credentials.assignment.id.clone();
                let clock = self.clock.clone();
                live.withdrawal = Some(call(move || {
                    protected(|| {
                        let guard = execution.gatekeeper().workspace();
                        let proof = guard.withdraw_mediated(&files)?;
                        let error = execution
                            .view(&session)
                            .and_then(|view| {
                                if let Some(attempt) = view.results().pending_for(&assignment) {
                                    guard.snapshot_withdrawn(
                                        &files,
                                        &proof,
                                        attempt.after.clone(),
                                        clock.now()?,
                                    )?;
                                }
                                Ok(())
                            })
                            .err();
                        Ok((proof, error))
                    })
                })?);
            }
        } else {
            live.files_closed = true;
        }
        if !live.cancel_sent
            && let Some(handle) = live.handle.clone()
        {
            let backend = self.backend.clone();
            live.cancel_sent = true;
            live.cancel = Some(call(move || protected(|| backend.cancel(&handle)))?);
        }
        Ok(())
    }
    fn diagnostic(
        &self,
        live: &LiveInvocation<J>,
        class: ErrorClass,
        code: &str,
        message: &str,
    ) -> Result<()> {
        self.execution.diagnostic(
            live.session(),
            &live.id,
            self.clock.now()?,
            class,
            code,
            message,
        )?;
        Ok(())
    }
    fn reject_reply(&self, live: &mut LiveInvocation<J>, error: &Denial) -> Result<()> {
        let class = if matches!(
            error.code.as_str(),
            "backend_panic" | "execution_worker" | "stream_lost"
        ) {
            ErrorClass::Infrastructure
        } else if matches!(
            error.code.as_str(),
            "output_limit" | "native_turn_limit" | "cost_limit"
        ) {
            ErrorClass::Content
        } else {
            ErrorClass::Protocol
        };
        live.pending_diagnostic = Some((
            class,
            error.code.chars().take(128).collect(),
            "Backend observation could not be validated".into(),
        ));
        live.blocked = Some(error.code.clone());
        self.stop(live, InvocationTerminal::Failed(class), false)?;
        self.flush_diagnostic(live)
    }
    fn flush_diagnostic(&self, live: &mut LiveInvocation<J>) -> Result<()> {
        if let Some((class, code, message)) = &live.pending_diagnostic {
            self.diagnostic(live, *class, code, message)?;
            live.pending_diagnostic = None;
        }
        Ok(())
    }
    fn process_reply(&self, live: &mut LiveInvocation<J>) -> Result<()> {
        let Some(reply) = live.pending.take() else {
            return Ok(());
        };
        match reply {
            Reply::Started {
                called_at,
                response,
            } => match response {
                Ok(response) => {
                    if response.handle.invocation == live.id {
                        live.handle = Some(response.handle.clone());
                    }
                    let result = self.clock.now().and_then(|at| {
                        self.execution
                            .started(live.session(), &live.id, called_at, at, &response)
                    });
                    if let Err(error) = result {
                        if matches!(
                            error.code.as_str(),
                            "invocation_identity"
                                | "invocation_start"
                                | "invocation"
                                | "invalid_text"
                        ) {
                            return self.reject_reply(live, &error);
                        }
                        live.pending = Some(Reply::Started {
                            called_at,
                            response: Ok(response),
                        });
                        return Err(error);
                    }
                    live.handle = Some(response.handle.clone());
                    let requested = &live.dispatch.as_ref().unwrap().settings;
                    if response
                        .sent
                        .model
                        .as_ref()
                        .is_some_and(|value| requested.model.as_ref() != Some(value))
                        || response
                            .sent
                            .effort
                            .as_ref()
                            .is_some_and(|value| requested.effort.as_ref() != Some(value))
                        || response
                            .reported
                            .model
                            .as_ref()
                            .is_some_and(|value| requested.model.as_ref() != Some(value))
                        || response
                            .reported
                            .effort
                            .as_ref()
                            .is_some_and(|value| requested.effort.as_ref() != Some(value))
                    {
                        return self.reject_reply(
                            live,
                            &Denial::new("invocation_settings", "Backend changed pinned settings"),
                        );
                    }
                    if live.stopped.load(Ordering::SeqCst) {
                        self.request_cleanup(live)?;
                    }
                }
                Err(error) => self.reject_reply(live, &error)?,
            },
            Reply::Events(result) => match result {
                Ok(events) if events.len() <= 128 => live.events.extend(events),
                Ok(_) => self.reject_reply(
                    live,
                    &Denial::new("event_limit", "Backend exceeded bounded event response"),
                )?,
                Err(error) => self.reject_reply(live, &error)?,
            },
        }
        Ok(())
    }
    fn process_events(&self, live: &mut LiveInvocation<J>) -> Result<()> {
        while let Some(observed) = live.events.front().cloned() {
            if observed.invocation != live.id {
                live.events.clear();
                return self.reject_reply(
                    live,
                    &Denial::new("invocation_identity", "Foreign event identity"),
                );
            }
            let reference =
                match self
                    .execution
                    .observe(live.session(), self.clock.now()?, observed.clone())
                {
                    Ok(reference) => reference,
                    Err(error)
                        if matches!(
                            error.code.as_str(),
                            "output_limit"
                                | "invocation_identity"
                                | "invocation_limit"
                                | "usage_baseline"
                                | "invocation_terminal"
                                | "operation_request"
                                | "invalid_text"
                        ) =>
                    {
                        live.events.clear();
                        return self.reject_reply(live, &error);
                    }
                    Err(error) => return Err(error),
                };
            match &observed.observation {
                BackendObservation::Usage { usage, .. } => {
                    self.execution.receipt(
                        live.session(),
                        &live.id,
                        self.clock.now()?,
                        Receipt {
                            id: live.receipt.clone(),
                            invocation: live.id.erased(),
                            usage: usage.clone(),
                            coverage: Coverage::Partial,
                            cost: None,
                        },
                    )?;
                    self.price_observation(live)?;
                }
                BackendObservation::Progress { signal, .. } => {
                    let view = self.execution.view(live.session())?;
                    if view.execution().invocations()[&live.id].terminal.is_none() {
                        match self.arbiter.renew_with_basis(
                            self.execution.gatekeeper(),
                            &live.credentials.grant,
                            view.revision(),
                            self.clock.now()?,
                            *signal,
                            Some(reference),
                        ) {
                            Ok(_) => {}
                            Err(error)
                                if matches!(
                                    error.code.as_str(),
                                    "lease_renewal"
                                        | "lease_limit"
                                        | "progress_basis_unsupported"
                                        | "progress_basis"
                                        | "grant_denied"
                                ) =>
                            {
                                self.diagnostic(
                                    live,
                                    ErrorClass::Content,
                                    &error.code,
                                    "Recorded progress did not renew the commitment",
                                )?;
                            }
                            Err(error) => return Err(error),
                        }
                    }
                }
                BackendObservation::Terminal(terminal) => {
                    let view = self.execution.view(live.session())?;
                    if view.execution().invocations()[&live.id].terminal.is_none() {
                        if let Some(reason) = limit_reason(
                            &view,
                            &view.execution().invocations()[&live.id],
                            self.clock.now()?,
                        )? {
                            self.diagnostic(
                                live,
                                ErrorClass::Content,
                                reason,
                                "Whole-invocation resource bound reached",
                            )?;
                            self.stop(
                                live,
                                if reason == "timeout" {
                                    InvocationTerminal::TimedOut
                                } else {
                                    InvocationTerminal::Failed(ErrorClass::Content)
                                },
                                false,
                            )?;
                        } else if *terminal == InvocationTerminal::Completed {
                            live.stopped.store(true, Ordering::SeqCst);
                            self.request_cleanup(live)?;
                            self.execution
                                .revoke(live.session(), &live.id, self.clock.now()?)?;
                        } else {
                            self.stop(live, terminal.clone(), true)?;
                        }
                    }
                }
                BackendObservation::OperationRequest {
                    operation,
                    correlation,
                    ..
                } => {
                    let denial = self.execution.operation_denial(
                        &live.credentials.grant,
                        &live.id,
                        *operation,
                        self.clock.now()?,
                    );
                    let reply = serde_json::to_string(&denial)
                        .map_err(|_| Denial::new("operation_reply", "Cannot encode denial"))?;
                    live.replies.push_back((correlation.clone(), reply));
                }
                _ => {}
            }
            live.events.pop_front();
            let view = self.execution.view(live.session())?;
            let record = &view.execution().invocations()[&live.id];
            if record.terminal.is_none()
                && let Some(reason) = limit_reason(&view, record, self.clock.now()?)?
            {
                self.diagnostic(
                    live,
                    ErrorClass::Content,
                    reason,
                    "Whole-invocation resource bound reached",
                )?;
                self.stop(
                    live,
                    if reason == "timeout" {
                        InvocationTerminal::TimedOut
                    } else {
                        InvocationTerminal::Failed(ErrorClass::Content)
                    },
                    false,
                )?;
            }
        }
        Ok(())
    }
    fn cleanup(&self, live: &mut LiveInvocation<J>) -> Result<()> {
        if let Some(receiver) = &live.reply_call {
            match receiver.try_recv() {
                Ok(result) => {
                    live.reply_call = None;
                    if let Err(error) = result {
                        self.diagnostic(
                            live,
                            ErrorClass::Protocol,
                            &error.code,
                            "Operation reply was not delivered",
                        )?;
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    live.reply_call = None;
                    live.blocked = Some("operation_reply_unobserved".into());
                }
            }
        }
        if live.reply_call.is_none()
            && let Some(handle) = live.handle.clone()
            && let Some((correlation, result)) = live.replies.pop_front()
        {
            let backend = self.backend.clone();
            live.reply_call = Some(call(move || {
                protected(|| backend.reply(&handle, &correlation, &result))
            })?);
        }
        if let Some(receiver) = &live.withdrawal {
            match receiver.try_recv() {
                Ok(Ok((proof, capture_error))) => {
                    live.withdrawal = None;
                    live.capture_retry = capture_error.as_ref().is_some_and(|error| {
                        matches!(error.code.as_str(), "stale_revision" | "cessation_evidence")
                    });
                    live.withdrawal_proof = Some(proof);
                    if capture_error.is_some() && !live.capture_retry {
                        live.pending_diagnostic = Some((ErrorClass::Environment, "result_capture_unavailable".into(),
                            "Result capture must be resolved or the attempt abandoned; cessation evidence is retained".into()));
                    }
                }
                Ok(Err(error)) => {
                    live.withdrawal = None;
                    if matches!(error.code.as_str(), "stale_revision" | "cessation_evidence") {
                        live.withdrawal_started = false;
                    } else {
                        live.blocked = Some(error.code);
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    live.withdrawal = None;
                    live.blocked = Some("cessation_unobserved".into());
                }
            }
        }
        if let Some(proof) = &live.withdrawal_proof {
            let view = self.execution.view(live.session())?;
            if let Some(attempt) = view.results().pending_for(&live.credentials.assignment.id)
                && !view.snapshots().contains_key(&attempt.after)
            {
                if view
                    .capture_reads()
                    .get(&attempt.after)
                    .is_some_and(|capture| capture.ended.is_none())
                {
                    self.execution.gatekeeper().workspace().resolve_capture(
                        live.session(),
                        view.revision(),
                        self.clock.now()?,
                        &attempt.after,
                    )?;
                    return Ok(());
                }
                if view.path_locks()[&live.credentials.assignment.id.erased()]
                    .released
                    .is_none()
                {
                    if live.capture_retry && !view.capture_reads().contains_key(&attempt.after) {
                        // Resolve a retained unstarted preparation before retrying.
                        // Any recorded capture takes the resolution path above;
                        // a completed or aborted capture never repeats its I/O.
                        match self.execution.gatekeeper().workspace().resolve_capture(
                            live.session(),
                            view.revision(),
                            self.clock.now()?,
                            &attempt.after,
                        ) {
                            Err(error)
                                if matches!(
                                    error.code.as_str(),
                                    "capture_missing" | "capture_evidence"
                                ) => {}
                            other => {
                                other?;
                            }
                        }
                        live.withdrawal_proof = None;
                        live.withdrawal_started = false;
                        live.capture_retry = false;
                        return Ok(());
                    }
                    live.blocked = Some("result_capture_unavailable".into());
                    return Ok(());
                }
            }
            if view.path_locks()[&live.credentials.assignment.id.erased()]
                .released
                .is_none()
                && let Err(error) = self.execution.gatekeeper().workspace().release(
                    live.session(),
                    view.revision(),
                    self.clock.now()?,
                    proof,
                )
            {
                if error.code == "cessation_evidence" {
                    live.withdrawal_proof = None;
                    live.withdrawal_started = false;
                    return Ok(());
                }
                return Err(error);
            }
            live.withdrawal_proof = None;
            live.files_closed = true;
        }
        if let Some(receiver) = &live.cancel {
            match receiver.try_recv() {
                Ok(result) => {
                    live.cancel = None;
                    if let Err(error) = result {
                        self.diagnostic(
                            live,
                            ErrorClass::Infrastructure,
                            &error.code,
                            "Cancellation was requested but not confirmed by the backend",
                        )?;
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    live.cancel = None;
                    live.blocked = Some("cancel_unobserved".into());
                }
            }
        }
        Ok(())
    }
    fn price_observation(&self, live: &LiveInvocation<J>) -> Result<()> {
        let view = self.execution.view(live.session())?;
        let source = &view.execution().invocations()[&live.id];
        if let Some(receipt) = &source.receipt {
            let digest = Digest::of_value(receipt)?;
            if source
                .cost
                .as_ref()
                .is_some_and(|(prior, _, _)| *prior == digest)
            {
                return Ok(());
            }
            let input = cost_view(&view, &source.dispatch.reservation)?;
            let response = ymp_kernel::ports::resources::ResourceResponse {
                input: Digest::of_value(&input)?,
                proposal: self.cost.observed_cost(&input)?,
            };
            self.execution
                .price(live.session(), &live.id, self.clock.now()?, response)?;
        }
        Ok(())
    }
    fn accounting(&self, live: &mut LiveInvocation<J>) -> Result<()> {
        if let Some(receiver) = &live.receipt_call {
            match receiver.try_recv() {
                Ok(Ok(receipt)) => {
                    live.receipt_call = None;
                    live.pending_receipt = Some(receipt);
                }
                Ok(Err(error)) => {
                    live.receipt_call = None;
                    if error.code != "receipt_pending" {
                        self.diagnostic(
                            live,
                            ErrorClass::Infrastructure,
                            &error.code,
                            "Receipt is unavailable; financial hold remains",
                        )?;
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    live.receipt_call = None;
                    live.blocked = Some("receipt_unobserved".into());
                }
            }
        }
        if let Some(receipt) = live.pending_receipt.clone() {
            match self
                .execution
                .receipt(live.session(), &live.id, self.clock.now()?, receipt)
            {
                Ok(_) => {
                    live.pending_receipt = None;
                    live.receipt_observed = true;
                }
                Err(error)
                    if matches!(
                        error.code.as_str(),
                        "receipt_binding" | "receipt_conflict" | "usage"
                    ) =>
                {
                    live.pending_receipt = None;
                    live.receipt_observed = true;
                    self.diagnostic(
                        live,
                        ErrorClass::Protocol,
                        &error.code,
                        "Receipt identity or cumulative usage was rejected",
                    )?;
                    live.blocked = Some(error.code);
                }
                Err(error) => return Err(error),
            }
        }
        self.price_observation(live)?;
        let view = self.execution.view(live.session())?;
        let source = &view.execution().invocations()[&live.id];
        let reservation = source.dispatch.reservation.clone();
        let account = &view.treasury().unwrap().accounts[&reservation];
        if account.reservation.state == ReservationState::Held
            && account.receipt.is_some()
            && live.receipt_observed
        {
            let response = cost_response(self.cost.as_ref(), &cost_view(&view, &reservation)?)?;
            if !matches!(
                response.proposal.value,
                ymp_domain::resources::ReceiptPrice::Unknown
            ) {
                self.execution.treasury().settle(
                    live.session(),
                    view.revision(),
                    self.clock.now()?,
                    reservation,
                    response,
                )?;
            }
        }
        Ok(())
    }
    pub fn poll(&self, live: &mut LiveInvocation<J>) -> Result<ExecutionStatus> {
        match self.poll_inner(live) {
            Err(error) if error.code == "stale_revision" => {
                Ok(ExecutionStatus::WaitingForObservation)
            }
            result => result,
        }
    }
    fn poll_inner(&self, live: &mut LiveInvocation<J>) -> Result<ExecutionStatus> {
        self.own(live)?;
        if self.owner.as_ref().is_some_and(|owner| owner.stopped())
            && !live.stopped.load(Ordering::SeqCst)
        {
            self.cancel(live)?;
        }

        if let Err(error) = self.clock.now() {
            live.stopped.store(true, Ordering::SeqCst);
            live.pending_stop
                .get_or_insert((InvocationTerminal::Failed(ErrorClass::Environment), false));
            self.request_cleanup(live)?;
            return Err(error);
        }
        if live.dispatch.is_none() {
            if live.stopped.load(Ordering::SeqCst) {
                self.cleanup(live)?;
            }
            return Ok(ExecutionStatus::Prepared);
        }
        if let Some((terminal, confirmed)) = live.pending_stop.clone() {
            self.stop(live, terminal, confirmed)?;
        }
        self.flush_diagnostic(live)?;
        let source = self.snapshot(live)?;
        if source.terminal.is_none() && source.backend_terminal.is_none() {
            let now = self.clock.now()?;
            if let Err(error) = self
                .execution
                .allowed(&live.credentials.grant, &live.id, now)
            {
                if error.code == "stale_revision" {
                    return Err(error);
                }
                let view = self.execution.view(live.session())?;
                let lease = &view.coordination().commitments()
                    [&source.dispatch.assignment.commitment]
                    .lease;
                let timed_out = now >= source.dispatch.deadline || now > lease.expires;
                let revoked = view.admission().assignments()[&source.dispatch.assignment.id]
                    .intent
                    .assignment
                    .state
                    == ymp_domain::assignment::AssignmentState::Revoked;
                let terminal = if timed_out {
                    InvocationTerminal::TimedOut
                } else if revoked {
                    InvocationTerminal::Cancelled
                } else {
                    InvocationTerminal::Failed(ErrorClass::Environment)
                };
                live.pending_diagnostic = Some((
                    ErrorClass::Environment,
                    error.code.clone(),
                    "Current invocation authority could not be established".into(),
                ));
                self.stop(live, terminal, false)?;
                self.flush_diagnostic(live)?;
            }
        }
        if live.pending.is_none()
            && let Some(receiver) = &live.call
        {
            match receiver.try_recv() {
                Ok(reply) => {
                    live.call = None;
                    live.pending = Some(reply);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    live.call = None;
                    self.reject_reply(
                        live,
                        &Denial::new("backend_panic", "Backend worker ended without a reply"),
                    )?;
                }
            }
        }
        self.process_reply(live)?;
        self.process_events(live)?;
        if live.stopped.load(Ordering::SeqCst) {
            self.request_cleanup(live)?;
        }
        self.cleanup(live)?;
        let source = self.snapshot(live)?;
        if source.terminal.is_some() || source.backend_terminal.is_some() {
            self.accounting(live)?;
        }
        let view = self.execution.view(live.session())?;
        let source = &view.execution().invocations()[&live.id];
        let financial = &view.treasury().unwrap().accounts[&source.dispatch.reservation];
        if source.terminal.is_none()
            && let Some((InvocationTerminal::Completed, _, ended)) = &source.backend_terminal
        {
            if let Some(reason) = limit_reason(&view, source, *ended)? {
                live.pending_diagnostic = Some((
                    ErrorClass::Content,
                    reason.into(),
                    "Final receipt exceeds the whole-invocation allowance".into(),
                ));
                self.stop(live, InvocationTerminal::Failed(ErrorClass::Content), false)?;
                self.flush_diagnostic(live)?;
                return Ok(ExecutionStatus::WaitingForObservation);
            }
            if live.receipt_observed
                && financial.reservation.state == ReservationState::Settled
                && live.files_closed
            {
                self.stop(live, InvocationTerminal::Completed, true)?;
                return Ok(ExecutionStatus::WaitingForObservation);
            }
        }

        if source.terminal == Some(InvocationTerminal::Completed)
            && source.confirmed_terminal
            && live.files_closed
            && limit_reason(&view, source, source.ended_at.unwrap())?.is_none()
        {
            let contribution = &view.coordination().contributions()
                [&source.dispatch.assignment.contribution]
                .value;
            if !matches!(
                contribution.kind,
                ContributionKind::Produce
                    | ContributionKind::Alternative
                    | ContributionKind::Integrate
            ) && view.coordination().commitments()[&source.dispatch.assignment.commitment].state
                == ymp_domain::coordination::CommitmentState::Active
            {
                self.arbiter.discharge(
                    self.execution.gatekeeper(),
                    live.session(),
                    view.revision(),
                    self.clock.now()?,
                    &source.dispatch.assignment.commitment,
                    source.end.as_ref().unwrap(),
                )?;
            }
        }
        if source.terminal.is_some()
            && live.files_closed
            && financial.reservation.state != ReservationState::Held
            && source.backend_terminal.is_some()
            && live.replies.is_empty()
            && live.reply_call.is_none()
        {
            return Ok(ExecutionStatus::Finished);
        }
        if let Some(handle) = live.handle.clone() {
            if (source.terminal.is_some() || source.backend_terminal.is_some())
                && live.receipt_call.is_none()
                && (!live.receipt_observed
                    || (financial.reservation.state == ReservationState::Held
                        && source
                            .receipt
                            .as_ref()
                            .is_some_and(|receipt| receipt.coverage != Coverage::Complete)))
            {
                let backend = self.backend.clone();
                let handle = handle.clone();
                live.receipt_call = Some(call(move || protected(|| backend.receipt(&handle)))?);
            }
            if live.call.is_none()
                && live.pending.is_none()
                && live.events.is_empty()
                && source.backend_terminal.is_none()
                && live.blocked.is_none()
            {
                let backend = self.backend.clone();
                let after = source.observations.len() as u64;
                live.call = Some(call(move || {
                    Reply::Events(protected(|| backend.events(&handle, after, 128)))
                })?);
            }
        }
        if let Some(reason) = &live.blocked {
            return Ok(ExecutionStatus::Blocked(reason.clone()));
        }
        if live.reply_call.is_some() || !live.replies.is_empty() {
            return Ok(ExecutionStatus::WaitingForObservation);
        }
        if source.terminal.is_some() || source.backend_terminal.is_some() {
            return Ok(if live.receipt_observed {
                ExecutionStatus::Blocked("unsettled_usage_or_effects".into())
            } else {
                ExecutionStatus::WaitingForReceipt
            });
        }
        Ok(ExecutionStatus::Running)
    }
    /// Replay is observation-only. It never constructs a live handle or calls start.
    pub fn recover(&self, session: &Id, invocation: &Id<Invocation>) -> Result<InvocationRecord> {
        let view = self.execution.view(session)?;
        let source = view
            .execution()
            .invocations()
            .get(invocation)
            .ok_or_else(|| Denial::new("invocation_missing", "No recorded invocation"))?;
        let account = &view.treasury().unwrap().accounts[&source.dispatch.reservation];
        let paths = view
            .path_locks()
            .get(&source.dispatch.assignment.id.erased())
            .is_none_or(|lock| lock.released.is_some());
        if source.backend_terminal.is_some()
            && paths
            && account.reservation.state != ReservationState::Held
        {
            return Ok(source.clone());
        }
        Err(Denial::new("invocation_recovery_blocked", "Unresolved execution has no recoverable local capability; retain its holds and observe it without restarting").with_ref(source.ready.clone()))
    }
}

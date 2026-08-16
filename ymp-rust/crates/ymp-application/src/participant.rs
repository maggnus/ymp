//! Starting the participant a run ignites on, out of the record and nothing else.
//!
//! When a run is authorized its pool is already frozen, and that snapshot names the entry the run
//! ignites on (owner decision D2). This is where that entry becomes a running participant: one
//! managed attempt, in a private workspace of its own, under the provider, engine and model the
//! snapshot recorded — never under whatever the product root's pools happen to say now.
//!
//! Four things decide the shape of this path.
//!
//! * **The route is read, never resolved.** The frozen record is taken from the run's own
//!   projection, which is a reading of its journal, and the entry is its `origin`. Nothing here
//!   opens a pool record, a provider record or a catalog. A pool edited, an entry taken out of it
//!   or an account held back after the run was created therefore cannot move a participant onto
//!   another model, and the next run — created against the pool as it stands then — is where such a
//!   change lands.
//! * **This host still has a say, and it says it before anything is written.** Whether the engine
//!   that route names may be started at all is the operator's decision on the engine record, and it
//!   is read at start time: an engine held back refuses the start in plain words naming
//!   `/runtimes`, with the journal exactly as it was. What the frozen record decides is what the
//!   run may do; what admission decides is what this host will do, and neither answers for the
//!   other.
//! * **A start is a charged, journalled fact.** One unit of the participant-starts dimension is
//!   consumed from the run's own authority before the participant exists, and the start is
//!   committed to the journal with the route and the workspace it ran in. A participant nothing
//!   paid for and nothing recorded would be a run doing work no account states.
//! * **One authorization is one attempt.** A second start on a run that already ignited is refused
//!   here rather than served from the recorded result of the first, because a repeated delivery
//!   answered from the record would still have started a second process. Yielding and resuming
//!   moves the participant that exists; it never begins another.

use std::path::{Path, PathBuf};

use thiserror::Error;
use ymp_domain::commitment::{BudgetVector, CommitmentCommand, Dimension, RegisterParticipant};
use ymp_domain::contract::ContractDocument;
use ymp_domain::participant::{
    OriginParticipant, ParticipantOutcome, ParticipantStart, ParticipantState,
};
use ymp_domain::pool::EntryIdentity;
use ymp_domain::{Command, RunStatus};
use ymp_runtime_api::{
    CancellationToken, InvocationRequest, McpBinding, Readiness, RuntimeDriver, RuntimeError,
    RuntimeEvent, RuntimeEventKind, RuntimeSession,
};
use ymp_runtime_registry::{Engine, Registry, RegistryError};

use crate::{Application, ApplicationError};

/// The participant every run ignites on. One run has exactly one, so it is named rather than
/// numbered, and a repeated authorization addresses the participant that already exists.
pub const ORIGIN_PARTICIPANT: &str = "origin";
/// The attempt that participant runs as.
pub const ORIGIN_ATTEMPT: &str = "attempt-origin";
/// The process slice its first invocation is recorded under.
pub const ORIGIN_INVOCATION: &str = "invocation-origin";

/// The authority the run itself holds, and the account a participant start is charged to.
const RUN_AUTHORITY: &str = "run-authority";
/// The principal that authority acts as. It is the operator's authorization of the run, supplied by
/// the product and never chosen by a participant.
const RUN_PRINCIPAL: &str = "principal-operator";
const RUN_OBLIGATION: &str = "run-origin";
/// The principal the started participant acts as.
const ORIGIN_PRINCIPAL: &str = "principal-origin-participant";

const KERNEL_COMMAND: &str = "ymp.participant.origin.kernel";
const REGISTER_COMMAND: &str = "ymp.participant.origin.register";
const START_COMMAND: &str = "ymp.participant.origin.start";
const FINISH_COMMAND: &str = "ymp.participant.origin.finish";

/// The directory a run's private workspaces stand in, inside its own store.
const WORKSPACES_DIRECTORY: &str = "workspaces";

/// What the run's own authority holds when it ignites.
///
/// It holds exactly one permission to start a participant, so the accounting itself states that
/// this build ignites one participant and recruits none: a second registration is refused by the
/// account rather than by a rule beside it. Recruitment (P10) is what widens this, and widening it
/// is a decision recorded where the budget is stated.
fn run_authority_budget() -> BudgetVector {
    BudgetVector::ZERO.with(Dimension::ParticipantStarts, 1)
}

/// What the started participant holds in its own name. It requests nothing and offers nothing in
/// this build, so it is endowed with nothing: the work it does is funded by the run.
fn origin_endowment() -> BudgetVector {
    BudgetVector::ZERO
}

/// The drivers a host can serve a frozen route with.
///
/// The route is handed over by value — the provider, engine and model the run's own record names —
/// and the implementation answers with the driver that serves it or with the reason this host
/// serves none. It is a parameter rather than a lookup so that the runtime crates stay above this
/// level: what a route means as a program is the business of the level that owns those programs,
/// and what a route *is* has already been decided by the record.
pub trait ParticipantRuntimes {
    fn driver_for(&self, route: &EntryIdentity)
    -> Result<Box<dyn RuntimeDriver>, RouteUnavailable>;
}

/// Why this host serves no driver for a frozen route, in the words the host states it in.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{0}")]
pub struct RouteUnavailable(pub String);

impl RouteUnavailable {
    pub fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }
}

/// What the caller supplies to start the origin, which is deliberately almost nothing.
///
/// The prompt, the source and the exclusions come from the approved contract, and the route comes
/// from the frozen record. What is left is where the engine records that govern this store stand,
/// and the two handles a supervising caller holds: the bridge the participant reaches the run
/// through, and the token that stops it.
#[derive(Clone, Debug, Default)]
pub struct OriginStartRequest {
    /// The product root whose registry answers whether the frozen engine may be started here.
    pub root: PathBuf,
    /// The bridge the participant reaches its run through, where the caller runs one.
    pub mcp: Option<McpBinding>,
    pub cancellation: CancellationToken,
}

impl OriginStartRequest {
    /// A request against one product root, which is the part that is never inferable: the engine
    /// records that govern a store are the operator's, and a start that guessed where they stand
    /// could admit an engine under a root nobody addressed.
    pub fn under(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            ..Self::default()
        }
    }
}

/// Why no participant was started, in the words an operator reads.
///
/// Every variant that can be reached before anything is written leaves the run exactly as it was:
/// no journal record, no charge, no workspace. The refusals name the state and the surface that
/// changes it rather than reporting a failure of the machinery.
#[derive(Debug, Error)]
pub enum OriginStartRefused {
    #[error(
        "your goal is held · this run has frozen no pool, so it names no entry to ignite on and no \
         participant was started · nothing has started and nothing has left this host"
    )]
    PoolNotFrozen,
    #[error(
        "your goal is held · this run already ignited on participant {participant_id}, and one \
         authorization starts one participant · nothing further has started"
    )]
    AlreadyStarted { participant_id: String },
    #[error(
        "your goal is held · this run has already ended, so nothing more is started for it · \
         nothing has started and nothing has left this host"
    )]
    RunEnded { status: RunStatus },
    #[error(
        "your goal is held · this run ignites on {entry}, which its own frozen pool does not \
         permit · nothing has started and nothing has left this host"
    )]
    OriginNotPermitted { entry: EntryIdentity },
    #[error(
        "your goal is held · this run ignites on {entry}, and this build manages no engine named \
         {engine} — /runtimes states the engines it manages · nothing has started and nothing has \
         left this host"
    )]
    UnknownEngine {
        entry: EntryIdentity,
        engine: String,
    },
    #[error(
        "your goal is held · this run ignites on {entry}, and the {engine} engine is not admitted \
         on this host — {reason} · /runtimes is where it is admitted again · the run's frozen pool \
         is untouched and nothing has left this host"
    )]
    RuntimeNotAdmitted {
        entry: EntryIdentity,
        engine: String,
        reason: String,
    },
    #[error(
        "your goal is held · this run ignites on {entry}, and this host serves no runtime for that \
         route — {reason} · /runtimes states what it serves · nothing has started and nothing has \
         left this host"
    )]
    RouteUnavailable {
        entry: EntryIdentity,
        reason: String,
    },
    #[error(
        "your goal is held · this run ignites on {entry}, and the runtime that serves it is not \
         ready — {detail} · /runtimes is where it is measured again · nothing has started and \
         nothing has left this host"
    )]
    RuntimeNotReady {
        entry: EntryIdentity,
        detail: String,
    },
    #[error(
        "your goal is held · this run is judged against no approved contract, so there is no work \
         to start a participant on · nothing has started and nothing has left this host"
    )]
    NoApprovedContract,
    #[error("no participant started — the approved contract could not be read: {0}")]
    ContractUnreadable(String),
    #[error("no participant started — the private workspace could not be prepared: {0}")]
    Workspace(#[source] ApplicationError),
    #[error(transparent)]
    Application(#[from] ApplicationError),
    #[error("the participant was started and its runtime refused the invocation: {0}")]
    Runtime(#[source] RuntimeError),
}

/// What went wrong while a started participant was being driven.
#[derive(Debug, Error)]
pub enum OriginAttemptError {
    #[error(transparent)]
    Application(#[from] ApplicationError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

impl Application {
    /// Start the participant this run ignites on, from the frozen record and the approved contract.
    ///
    /// The order is the whole of the honesty guarantee. Everything that can refuse a start — the
    /// snapshot, the containment check, the engine this host admits, the driver, its readiness — is
    /// answered first, so a refused start leaves no journal record, no charge and no workspace. Only
    /// then is the workspace materialized, the participant registered against the run's own
    /// authority — which is what consumes one participant start — and the start committed. The
    /// runtime is started last, and a runtime that refuses to start is recorded as the ending of the
    /// attempt that was authorized rather than quietly undone.
    pub fn start_origin_participant<'a>(
        &'a mut self,
        request: &OriginStartRequest,
        runtimes: &dyn ParticipantRuntimes,
    ) -> Result<OriginAttempt<'a>, OriginStartRefused> {
        self.refresh()?;
        if let Some(started) = &self.state.origin_participant {
            return Err(OriginStartRefused::AlreadyStarted {
                participant_id: started.participant_id().to_owned(),
            });
        }
        if self.state.status.is_terminal() {
            return Err(OriginStartRefused::RunEnded {
                status: self.state.status,
            });
        }
        let frozen = self
            .state
            .frozen_pool
            .as_ref()
            .ok_or(OriginStartRefused::PoolNotFrozen)?;
        let entry = frozen.origin.clone();
        if !frozen.permits(&entry) {
            return Err(OriginStartRefused::OriginNotPermitted { entry });
        }
        admit_route(&request.root, &entry)?;

        // The words the participant is started on come from the contract the run is judged against,
        // read back out of its own immutable store rather than from whatever a caller holds.
        let document = match self.approved_contract() {
            Ok((_, document)) => document,
            Err(ApplicationError::NoApprovedContract) => {
                return Err(OriginStartRefused::NoApprovedContract);
            }
            Err(error) => return Err(OriginStartRefused::ContractUnreadable(error.to_string())),
        };
        let driver =
            runtimes
                .driver_for(&entry)
                .map_err(|reason| OriginStartRefused::RouteUnavailable {
                    entry: entry.clone(),
                    reason: reason.0,
                })?;
        // A runtime that cannot even be measured is one that is not ready, and it is stated as that
        // rather than as a failure of the start: nothing has been written at this point.
        let detail = match driver.probe() {
            Ok(probe) if probe.readiness == Readiness::Ready => None,
            Ok(probe) => Some(probe.detail),
            Err(error) => Some(error.to_string()),
        };
        if let Some(detail) = detail {
            return Err(OriginStartRefused::RuntimeNotReady { entry, detail });
        }

        // From here the run is committed to starting: nothing below refuses, and what fails is
        // recorded rather than hidden.
        let workspace = self
            .materialize_origin_workspace(&document)
            .map_err(OriginStartRefused::Workspace)?;
        self.charge_participant_start()?;
        let outcome = self.execute(
            START_COMMAND,
            Command::StartOriginParticipant {
                participant_id: ORIGIN_PARTICIPANT.to_owned(),
                attempt_id: ORIGIN_ATTEMPT.to_owned(),
                workspace: workspace.to_string_lossy().into_owned(),
            },
        )?;
        let start = match &outcome.event.event {
            ymp_domain::EventKind::ParticipantStarted(start) => start.clone(),
            // The attempt budget was spent before the participant could be started. The run states
            // that ending itself; nothing is started for it.
            _ => {
                return Err(OriginStartRefused::RunEnded {
                    status: outcome.status,
                });
            }
        };

        match driver.start(InvocationRequest {
            invocation_id: ORIGIN_INVOCATION.to_owned(),
            attempt_id: start.attempt_id.clone(),
            workspace,
            mcp: request.mcp.clone(),
            prompt: document.prompt.clone(),
            cancellation: request.cancellation.clone(),
        }) {
            Ok(session) => Ok(OriginAttempt {
                application: self,
                session,
                start,
                slices: 0,
            }),
            Err(error) => {
                // The start is a fact, and a runtime that never came up is how that fact ended.
                let _ = self.finish_origin(
                    &start.participant_id,
                    ParticipantOutcome::Failed {
                        reason: error.to_string(),
                    },
                );
                Err(OriginStartRefused::Runtime(error))
            }
        }
    }

    /// The participant this run ignited on, as its own record states it.
    pub fn origin_participant(&self) -> Option<&OriginParticipant> {
        self.state.origin_participant.as_ref()
    }

    /// Bring the projection back to the journal before anything is decided against it.
    fn refresh(&mut self) -> Result<(), ApplicationError> {
        self.recovered_commitments().map(|_| ())
    }

    /// The private workspace the origin's attempt runs in, materialized from the contract's source.
    ///
    /// It is the same private copy the single-participant path makes: the source is captured into
    /// the run's own artifact store and the attempt is given a directory materialized from that
    /// capture, so the work happens beside the operator's project and never in it.
    fn materialize_origin_workspace(
        &self,
        document: &ContractDocument,
    ) -> Result<PathBuf, ApplicationError> {
        let artifacts = self.artifact_store();
        let base = artifacts.capture_source(&document.source)?;
        let workspace = self
            .data_root()
            .join(WORKSPACES_DIRECTORY)
            .join(ORIGIN_ATTEMPT);
        artifacts.materialize(&base.manifest_digest, &workspace)?;
        Ok(workspace)
    }

    /// Consume the one participant start this run's authority holds.
    ///
    /// The ledger is opened where the run does not carry one yet, and the participant is registered
    /// against whoever holds the run's root obligation. Registration is what charges the dimension:
    /// the kernel takes one unit of participant-starts from that account and refuses a registration
    /// the account cannot cover, so a run cannot ignite twice by asking twice.
    fn charge_participant_start(&mut self) -> Result<(), ApplicationError> {
        let sponsor = match self.recovered_commitments()? {
            Some(ledger) => ledger
                .obligations()
                .get(ledger.root_obligation())
                .map(|obligation| obligation.owner.clone())
                .ok_or(ApplicationError::NoCommitmentKernel)?,
            None => {
                self.open_commitment_kernel(
                    KERNEL_COMMAND,
                    RUN_AUTHORITY,
                    RUN_PRINCIPAL,
                    RUN_OBLIGATION,
                    run_authority_budget(),
                )?;
                RUN_AUTHORITY.to_owned()
            }
        };
        self.execute_commitment(
            REGISTER_COMMAND,
            &CommitmentCommand::RegisterParticipant(RegisterParticipant {
                participant_id: ORIGIN_PARTICIPANT.to_owned(),
                principal_id: ORIGIN_PRINCIPAL.to_owned(),
                sponsor,
                endowment: origin_endowment(),
            }),
        )?;
        Ok(())
    }

    fn finish_origin(
        &mut self,
        participant_id: &str,
        outcome: ParticipantOutcome,
    ) -> Result<(), ApplicationError> {
        self.execute(
            FINISH_COMMAND,
            Command::FinishParticipant {
                participant_id: participant_id.to_owned(),
                outcome,
            },
        )?;
        Ok(())
    }
}

/// Whether this host will start the engine a frozen route names.
///
/// Two questions, both answered from the engine records under the product root and neither from a
/// pool: whether this build manages an engine of that name at all, and whether the operator admits
/// it. A route the freeze recorded stays recorded either way — this decides what happens now, not
/// what the run was created under.
fn admit_route(root: &Path, entry: &EntryIdentity) -> Result<(), OriginStartRefused> {
    let engine = Engine::parse(&entry.engine).map_err(|_| OriginStartRefused::UnknownEngine {
        entry: entry.clone(),
        engine: entry.engine.clone(),
    })?;
    match Registry::under(root).admit(engine) {
        Ok(_) => Ok(()),
        Err(RegistryError::Disabled { engine, reason }) => {
            Err(OriginStartRefused::RuntimeNotAdmitted {
                entry: entry.clone(),
                engine,
                reason,
            })
        }
        Err(error) => Err(OriginStartRefused::RuntimeNotAdmitted {
            entry: entry.clone(),
            engine: entry.engine.clone(),
            reason: error.to_string(),
        }),
    }
}

/// One started participant, and the run its life is recorded in.
///
/// Every event the runtime produces passes through here, so what the record states about a
/// participant is what its runtime actually did: a yield is journalled when the runtime yields, an
/// ending when it ends, and a resumption is committed before the instruction reaches the runtime.
/// A caller that reads events elsewhere would leave the record behind the process it describes.
pub struct OriginAttempt<'a> {
    application: &'a mut Application,
    session: Box<dyn RuntimeSession>,
    start: ParticipantStart,
    /// How many process slices this attempt has run: the first, plus one for each resumption. It
    /// numbers the lifecycle commands so a second yield is not read as a repeat of the first.
    slices: u32,
}

/// The session a running attempt holds is a live process and describes nothing, so what is stated
/// here is the authorization it runs under and how many slices it has taken.
impl std::fmt::Debug for OriginAttempt<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OriginAttempt")
            .field("start", &self.start)
            .field("slices", &self.slices)
            .finish()
    }
}

impl OriginAttempt<'_> {
    /// The start this attempt was authorized by, with the route it runs under.
    pub const fn start(&self) -> &ParticipantStart {
        &self.start
    }

    /// The route the participant runs under, as its start recorded it.
    pub const fn route(&self) -> &EntryIdentity {
        &self.start.entry
    }

    /// Where the participant stands, as the run's own record states it.
    pub fn state(&self) -> Option<ParticipantState> {
        self.application
            .origin_participant()
            .map(|participant| participant.state)
    }

    /// The run this attempt is recorded in.
    pub fn application(&mut self) -> &mut Application {
        self.application
    }

    /// The next event of the participant's process slice, with what it means for the run recorded
    /// before it is returned.
    ///
    /// A caller is therefore never holding an event the record does not already state. A runtime
    /// that fails is the ending of this attempt and is recorded as one, and the failure is then
    /// reported rather than swallowed.
    pub fn next_event(&mut self) -> Result<Option<RuntimeEvent>, OriginAttemptError> {
        let event = match self.session.next_event() {
            Ok(event) => event,
            Err(error) => {
                self.record_finish(ParticipantOutcome::Failed {
                    reason: error.to_string(),
                })?;
                return Err(error.into());
            }
        };
        let Some(event) = event else {
            return Ok(None);
        };
        match &event.event {
            RuntimeEventKind::Yielded { cursor } => {
                let cursor = cursor.clone();
                self.record_yield(&cursor)?;
            }
            RuntimeEventKind::Completed { .. } => {
                self.record_finish(ParticipantOutcome::Completed)?;
            }
            RuntimeEventKind::Interrupted => {
                self.record_finish(ParticipantOutcome::Interrupted)?;
            }
            RuntimeEventKind::Failed { kind, .. } => {
                self.record_finish(ParticipantOutcome::Failed {
                    reason: format!("the runtime reported {kind:?}"),
                })?;
            }
            RuntimeEventKind::TimedOut { limit_ms, .. } => {
                self.record_finish(ParticipantOutcome::Failed {
                    reason: format!("the runtime exceeded its {limit_ms}-millisecond limit"),
                })?;
            }
            _ => {}
        }
        Ok(Some(event))
    }

    /// Put the yielded participant back to work on the instruction given.
    ///
    /// The resumption is committed first and reaches the runtime second, which is the order the
    /// kernel already holds a resumption to: a runtime is put back to work only for something the
    /// record states. It resumes the attempt that exists and starts no second one — the command
    /// carries no attempt identifier and the domain refuses a resumption of anything but a yielded
    /// participant.
    pub fn resume(&mut self, instruction: impl Into<String>) -> Result<(), OriginAttemptError> {
        let instruction = instruction.into();
        self.slices = self.slices.saturating_add(1);
        let slice = self.slices;
        self.application.execute(
            format!("ymp.participant.origin.resume-{slice}"),
            Command::ResumeParticipant {
                participant_id: self.start.participant_id.clone(),
                cursor: instruction.clone(),
            },
        )?;
        if let Err(error) = self.session.resume(instruction) {
            self.record_finish(ParticipantOutcome::Failed {
                reason: error.to_string(),
            })?;
            return Err(error.into());
        }
        Ok(())
    }

    /// Stop the participant's process slice.
    ///
    /// The ending is recorded where every other ending is: the interruption arrives as an event and
    /// is journalled by [`Self::next_event`]. Recording it here as well would put an ending in the
    /// record before the process it describes had reached one.
    pub fn interrupt(&mut self) -> Result<(), OriginAttemptError> {
        self.session.interrupt()?;
        Ok(())
    }

    fn record_yield(&mut self, cursor: &str) -> Result<(), ApplicationError> {
        let slice = self.slices;
        self.application.execute(
            format!("ymp.participant.origin.yield-{slice}"),
            Command::YieldParticipant {
                participant_id: self.start.participant_id.clone(),
                cursor: cursor.to_owned(),
            },
        )?;
        Ok(())
    }

    /// Record how the attempt ended, once. An ending reached from more than one place — the loop
    /// that saw the last event, and the failure on the way out of it — states that ending once.
    fn record_finish(&mut self, outcome: ParticipantOutcome) -> Result<(), ApplicationError> {
        if self.state() == Some(ParticipantState::Finished) {
            return Ok(());
        }
        let participant_id = self.start.participant_id.clone();
        self.application.finish_origin(&participant_id, outcome)
    }
}

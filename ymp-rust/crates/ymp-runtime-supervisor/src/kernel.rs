//! The kernel record of one managed run, and the only place the live controller decides anything
//! about the life of its process slice.
//!
//! The controller used to keep its own answer to "may this slice resume": a flag saying it had
//! yielded and a map of wake identifiers it had already served. Those answers were reached without
//! the committed facts, so a live run and a modelled run could disagree about what resumes and
//! what terminates. Here there is one answer. A slice starts, yields, is admitted and closes
//! through [`StartInvocation`], [`YieldInvocation`], [`ResumeInvocation`] and [`CloseInvocation`],
//! and the resumption a live operator asks for reaches the runtime only when the kernel has
//! committed it.
//!
//! One binding decision is stated here rather than implied. The kernel admits a resumption only
//! for a committed fact a yield named, and an operator's instruction is not a fact: the protocol's
//! direct-delivery wake condition does not exist in the accepted model. So the instruction enters
//! the kernel the only way inert content may — as a digest. A yielding slice advertises an offer
//! whose intent digest is the yield it registered, and it registers exactly one wake condition:
//! consent recorded against that offer. The operator's wake records that consent, carrying the
//! digest of the instruction as its proposal, and the fact it commits is what authorizes the
//! resumption. Nothing here reads the instruction, and no transition outside the accepted model is
//! used to admit it.
//!
//! Where the ledger lives is the second decision stated here. It is the run's own journal: a
//! command is decided against a copy of the ledger the application folded out of that journal, the
//! facts it commits are appended as one record, and only then is the caller answered. A controller
//! told that a slice resumed is therefore holding something the record already states, and the
//! ledger a restart rebuilds is the ledger the run held. The kernel kept in memory beside the
//! journal answered the same questions and left nothing behind: a run that ended took its
//! commitments with it, and no operator could read them afterwards.
//!
//! Two participants issue commands at once — the operator's thread admitting a wake while the
//! worker records a yield or the terminal of its slice — and the durable path decides through
//! `&mut Application`. The order they are decided in is therefore the order of the one lock the
//! run's journal is written under, and each command is decided, journalled and answered without
//! that lock changing hands. A lock of this kernel's own would order these commands against each
//! other but not against the run's other writers, and the order has to hold in the journal.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use anyhow::{Context, bail};
use ymp_application::{Application, ApplicationError, CandidateConstruction};
use ymp_domain::commitment::{
    AdvanceClock, Advertise, Award, BudgetVector, CloseInvocation, CommitmentCommand,
    CommitmentError, CommitmentLedger, ContractState, Dimension, FundingSource, InvocationClosure,
    InvocationState, MAX_LEASE_MS, OfferPolicy, OfferState, OpenAuthority, Outcome, PathChange,
    RecordBid, RecordObject, RecordVerification, RegisterParticipant, ResumeInvocation,
    ReturnObligation, RootTerminal, SettleOffer, StartAttempt, StartInvocation, StopReason,
    StopRun, SubmitBundle, Verdict, WakeCondition, WithdrawOffer, YieldInvocation,
};
use ymp_domain::{TransitionError, digest_bytes};

/// The principal the controller acts as. It is supplied by the trusted controller from the
/// authenticated connection, never chosen by a participant.
const SPONSOR_PRINCIPAL: &str = "principal-controller";
const AGENT_PRINCIPAL: &str = "principal-managed-runtime";
/// The opaque class token the managed run's consent must match. Equality is the only operation
/// performed on it.
const ARTIFACT_CLASS: &str = "workspace-candidate";

/// How the managed run ended, in the vocabulary the run is accountable in rather than in the
/// runtime's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedTermination {
    /// The runtime ended after an accepted explicit task action. It is not candidate acceptance:
    /// whether the candidate is accepted is decided by a protected query this controller does not
    /// perform, so the work obligation stays open and the run is not quiescent.
    Completed,
    /// An authorized command stopped the run.
    Cancelled,
    /// An authorized command stopped a run whose slice had already run a bounded dimension out.
    ///
    /// The run is stopped as cancelled, which is what an operator did and what both records state.
    /// The slice closes on the exhausted limit, because what the run consumed is not something the
    /// cancellation caused and not something its arrival undoes.
    CancelledPastLimit,
    /// The runtime, the model route or the supervision around them failed.
    Failed(InvocationClosure),
}

/// What the kernel recorded of one submission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Submission {
    /// The construction is in the record: every object the result puts at a path, the bundle those
    /// changes were published as, and the result they form, which is what the work is sealed with.
    Recorded,
    /// The kernel cannot state this construction, so nothing about the result was recorded and the
    /// work carries none. A run that reaches this has produced something its own record cannot
    /// describe, and the reason says which bound it exceeded.
    ProvenanceUnrecorded(UnrecordedProvenance),
}

/// Why the construction of a result could not be recorded.
///
/// Both cases are bounds the protocol sets on what one submission states at once, so that a bundle
/// and the result formed from it are committed together in a single durable record. Neither is a
/// judgement about the work: what the runtime produced is in the run's store either way, and what
/// is missing is the kernel's account of how it was built.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnrecordedProvenance {
    /// The result changes more paths than one bundle may carry.
    TooManyChanges {
        changes: usize,
        /// The kernel's own words for the refusal, so what an operator reads is the bound the
        /// protocol states and not a paraphrase of it.
        refusal: String,
    },
    /// One path is in a shape the protocol does not admit.
    UnacceptablePath { path: String, refusal: String },
}

impl std::fmt::Display for UnrecordedProvenance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyChanges { changes, refusal } => {
                write!(formatter, "too_many_changes({changes}): {refusal}")
            }
            Self::UnacceptablePath { path, refusal } => {
                write!(formatter, "unacceptable_path({path}): {refusal}")
            }
        }
    }
}

/// The budget one managed run is funded with. Every dimension is enforced on its own, so what is
/// stated here is what the run may spend and not a total it may reallocate.
fn execution_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 1_000_000)
        .with(Dimension::ModelTokens, 10_000_000)
        // The lease this controller buys, plus the slack a renewal would draw on.
        .with(Dimension::WallTimeMs, MAX_LEASE_MS.saturating_mul(2))
        .with(Dimension::VerificationQueries, 4)
        .with(Dimension::AttemptStarts, 2)
        // One slice for the start, and one for each wake the attempt is allowed.
        .with(Dimension::InvocationStarts, 16)
}

/// The authority a supervised participant holds in its own name: one offer for each yield it may
/// register a wake against. It is not part of the work's escrow, because a request the slice makes
/// is the slice's own act and is not drawn on the account its work is funded from.
fn agent_endowment() -> BudgetVector {
    BudgetVector::ZERO.with(Dimension::OfferCreations, 16)
}

fn root_budget() -> BudgetVector {
    execution_escrow()
        .checked_scale(2)
        .unwrap_or_else(|_| execution_escrow())
        .with(Dimension::ParticipantStarts, 2)
        // What the controller endows the participant with, plus the offer it advertises the work
        // under itself.
        .with(Dimension::OfferCreations, 32)
        .with(Dimension::ObligationCreations, 8)
}

/// What one wake offer holds. It funds nothing: it exists to carry the digest of the yield and to
/// be the object consent is recorded against.
fn wake_escrow() -> BudgetVector {
    BudgetVector::ZERO
}

/// The identifiers of one managed run inside the kernel, and the ledger those identifiers name.
pub struct ManagedKernel {
    /// The run's own record, which is where this kernel's ledger is. Every command is decided
    /// against the ledger folded out of this journal and is in that journal before it answers.
    application: Arc<Mutex<Application>>,
    sponsor: String,
    participant: String,
    offer_id: String,
    contract_id: String,
    obligation_id: String,
    attempt_id: String,
    invocation_id: String,
    generation: u64,
    /// The moment the ledger's clock reads zero at. Every later reading of the kernel clock is the
    /// wall time elapsed since it, so a lease and a wake deadline are the wall time somebody paid
    /// for and not a number the controller chose.
    epoch: Instant,
    lease_expires_at: u64,
    /// The command identifiers this controller issues on its own behalf are numbered, so a
    /// controller-issued command is never mistaken for a repeated delivery of an earlier one.
    commands: AtomicU64,
    /// The offer the current yield registered its condition against, while it is still open.
    open_wake_offer: Mutex<Option<String>>,
}

impl ManagedKernel {
    /// Prepare the ledger of one managed run up to the point where its first process slice may
    /// begin: the participant is registered, the work is offered, consented to and awarded, and the
    /// attempt has started under a live lease.
    ///
    /// The genesis is committed first, as a record of its own: the root participant, the principal
    /// it acts as, the obligation the run is accountable for and the budget it opens with are not
    /// derivable from the facts that follow, so a journal without that record could not rebuild
    /// this ledger at all. One run opens one kernel, and a second opening is refused rather than
    /// passed over — a run whose store already carries a kernel stops the start here, naming what
    /// the store holds, instead of running under a ledger the record does not state.
    pub fn prepare(
        application: Arc<Mutex<Application>>,
        attempt_id: &str,
        invocation_id: &str,
        task_scope: &str,
        base_digest: &str,
        intent_digest: &str,
    ) -> anyhow::Result<Self> {
        let sponsor = format!("controller-{attempt_id}");
        let participant = format!("runtime-{attempt_id}");
        let root_obligation = format!("run-{attempt_id}");
        let offer_id = format!("offer-{attempt_id}");
        let contract_id = format!("work-{attempt_id}");
        let obligation_id = format!("obligation-{attempt_id}");
        let kernel = Self {
            application,
            sponsor: sponsor.clone(),
            participant: participant.clone(),
            offer_id: offer_id.clone(),
            contract_id: contract_id.clone(),
            obligation_id: obligation_id.clone(),
            attempt_id: attempt_id.to_owned(),
            invocation_id: invocation_id.to_owned(),
            generation: 1,
            epoch: Instant::now(),
            lease_expires_at: MAX_LEASE_MS,
            commands: AtomicU64::new(0),
            open_wake_offer: Mutex::new(None),
        };
        kernel
            .application()
            .open_commitment_kernel(
                format!("{attempt_id}.kernel"),
                sponsor.as_str(),
                SPONSOR_PRINCIPAL,
                root_obligation.as_str(),
                root_budget(),
            )
            .context("the managed run ledger")?;
        // The whole formation is recorded while the ledger's clock still reads zero, which is what
        // lets the offer be advertised, consented to and awarded in one instant and settled the
        // moment the clock moves.
        kernel.commit(
            "participant",
            &CommitmentCommand::RegisterParticipant(RegisterParticipant {
                participant_id: participant.clone(),
                principal_id: AGENT_PRINCIPAL.to_owned(),
                sponsor: sponsor.clone(),
                endowment: agent_endowment(),
            }),
        )?;
        kernel.commit(
            "offer",
            &CommitmentCommand::Advertise(Advertise {
                offer_id: offer_id.clone(),
                sponsor: sponsor.clone(),
                parent_obligation: root_obligation,
                funding_source: FundingSource::Participant,
                task_scope: task_scope.to_owned(),
                base_digest: base_digest.to_owned(),
                intent_digest: intent_digest.to_owned(),
                artifact_class: ARTIFACT_CLASS.to_owned(),
                dependencies: Vec::new(),
                capability_scope: Vec::new(),
                execution_escrow: execution_escrow(),
                policy: OfferPolicy::Targeted {
                    participant_id: participant.clone(),
                },
                bid_deadline: 0,
                offer_deadline: 0,
                max_awards: 1,
            }),
        )?;
        kernel.commit(
            "consent",
            &CommitmentCommand::RecordBid(RecordBid {
                bid_id: format!("consent-{attempt_id}"),
                offer_id: offer_id.clone(),
                bidder: participant.clone(),
                requested_escrow: execution_escrow(),
                artifact_class: ARTIFACT_CLASS.to_owned(),
                proposal_digest: None,
                expires_at: 0,
            }),
        )?;
        kernel.commit(
            "award",
            &CommitmentCommand::Award(Award {
                contract_id: contract_id.clone(),
                obligation_id,
                lease_id: format!("lease-{attempt_id}"),
                offer_id,
                bid_id: format!("consent-{attempt_id}"),
                sponsor,
                lease_ms: MAX_LEASE_MS,
            }),
        )?;
        kernel.commit(
            "attempt",
            &CommitmentCommand::StartAttempt(StartAttempt {
                attempt_id: attempt_id.to_owned(),
                contract_id,
                participant,
                generation: 1,
            }),
        )?;
        Ok(kernel)
    }

    /// Begin the first process slice. It costs one unit of the authority to run a slice, so a run
    /// that keeps starting processes runs out of the authority to do so.
    pub fn start_invocation(&self) -> anyhow::Result<()> {
        let cursor = self.cursor()?;
        self.commit(
            "slice",
            &CommitmentCommand::StartInvocation(StartInvocation {
                invocation_id: self.invocation_id.clone(),
                attempt_id: self.attempt_id.clone(),
                contract_id: self.contract_id.clone(),
                participant: self.participant.clone(),
                generation: self.generation,
                cursor,
            }),
        )?;
        Ok(())
    }

    /// Record that the slice stopped running without returning its task contract, and register the
    /// one fact that would be worth resuming it for.
    ///
    /// A run whose journal has ended registers nothing. The offer this yield would advertise exists
    /// to be woken against, and there is no wake coming for a run an operator has stopped: what
    /// follows a yield on such a run is the slice being closed and the accounting wound down. The
    /// record states that ending rather than one more request nobody will answer, and the slice
    /// stays where the facts left it until the terminal closes it.
    pub fn yielded(&self, yield_command_id: &str) -> anyhow::Result<()> {
        self.advance_clock()?;
        let sequence = self.commands.fetch_add(1, Ordering::Relaxed);
        let wake_offer = format!("wake-{sequence}-{}", self.attempt_id);
        match self.execute(
            &format!("{}.wake-offer-{sequence}", self.attempt_id),
            &CommitmentCommand::Advertise(Advertise {
                offer_id: wake_offer.clone(),
                // The slice asks; the controller answers. The parent obligation is the work this
                // slice holds, so the request is accounted under the work that raised it.
                sponsor: self.participant.clone(),
                parent_obligation: self.obligation_id.clone(),
                funding_source: FundingSource::Participant,
                task_scope: self.contract_id.clone(),
                base_digest: digest_bytes(self.contract_id.as_bytes()),
                // The yield itself, as inert content the kernel stores and never opens.
                intent_digest: digest_bytes(yield_command_id.as_bytes()),
                artifact_class: ARTIFACT_CLASS.to_owned(),
                dependencies: Vec::new(),
                capability_scope: Vec::new(),
                execution_escrow: wake_escrow(),
                policy: OfferPolicy::Targeted {
                    participant_id: self.sponsor.clone(),
                },
                bid_deadline: self.lease_expires_at,
                offer_deadline: self.lease_expires_at,
                max_awards: 1,
            }),
        ) {
            Ok(()) => {}
            Err(error) if run_has_ended(&error) => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        let cursor = self.cursor()?;
        self.commit(
            &format!("yield-{sequence}"),
            &CommitmentCommand::YieldInvocation(YieldInvocation {
                invocation_id: self.invocation_id.clone(),
                participant: self.participant.clone(),
                generation: self.generation,
                cursor,
                conditions: vec![WakeCondition::BidRecorded {
                    offer_id: wake_offer.clone(),
                }],
                wake_deadline: self.lease_expires_at,
            }),
        )?;
        *self
            .open_wake_offer
            .lock()
            .map_err(|_| anyhow::anyhow!("the managed kernel lock was poisoned"))? =
            Some(wake_offer);
        Ok(())
    }

    /// Admit one resumption of the yielded slice.
    ///
    /// The instruction is recorded as consent against the offer the yield named, and the
    /// resumption is then decided by the kernel against the committed facts: whether the slice is
    /// yielded, whether its wake is still funded and undeadlined, whether the attempt has wakes
    /// left, and whether a fact the yield named has actually been committed. A repeated delivery of
    /// the same wake returns the recorded result without a second effect, and the caller is told so
    /// rather than resuming the runtime twice.
    pub fn admit_wake(&self, wake_id: &str, instruction: &str) -> anyhow::Result<bool> {
        self.advance_clock()?;
        let consent_id = format!("{wake_id}.consent");
        let instruction_digest = digest_bytes(instruction.as_bytes());
        // Whether this wake has already been served is read out of the committed consent, not out
        // of a list the controller keeps: a repeated delivery is one whose instruction is the
        // content the recorded consent already carries.
        let recorded = self.snapshot()?.bids().get(&consent_id).cloned();
        if let Some(recorded) = &recorded
            && recorded.proposal_digest.as_deref() != Some(instruction_digest.as_str())
        {
            bail!("wake command identifier was reused with different input");
        }
        let open = self
            .open_wake_offer
            .lock()
            .map_err(|_| anyhow::anyhow!("the managed kernel lock was poisoned"))?
            .clone();
        let Some(offer_id) = open else {
            // Nothing is registered for a wake to answer. A delivery whose consent is already
            // committed is a repeat of a wake that was served, and it resumes nothing a second
            // time; anything else is a wake for a run that is not yielded at all.
            if recorded.is_some() {
                return Ok(false);
            }
            bail!("managed runtime is not yielded");
        };
        // Consent recorded against an offer some earlier yield registered is likewise a repeat: the
        // resumption it authorized is committed, and the yield now open registered a different
        // offer of its own.
        if recorded
            .as_ref()
            .is_some_and(|recorded| recorded.offer_id != offer_id)
        {
            return Ok(false);
        }
        if recorded.is_none() {
            self.execute(
                &consent_id,
                &CommitmentCommand::RecordBid(RecordBid {
                    bid_id: consent_id.clone(),
                    offer_id: offer_id.clone(),
                    bidder: self.sponsor.clone(),
                    requested_escrow: wake_escrow(),
                    artifact_class: ARTIFACT_CLASS.to_owned(),
                    proposal_digest: Some(instruction_digest),
                    expires_at: self.lease_expires_at,
                }),
            )?;
        }
        // Admission order is the whole of the fairness rule, so a controller that resumed a slice
        // the kernel did not put first would be scheduling on its own authority. The order is read
        // here rather than before the consent, because a yielded slice enters the queue only once a
        // committed fact answers the condition it registered: read any earlier, the queue is empty
        // whatever else is waiting, and the comparison can refuse nothing. The consent stays
        // committed when this refuses, so the same wake may be delivered again once the slices
        // ahead of it have been admitted.
        let admissible = self.admission_order()?;
        if let Some(first) = admissible.first()
            && first != &self.invocation_id
        {
            bail!("the kernel admits invocation {first} before this one");
        }
        self.execute(
            &format!("{wake_id}.resume"),
            &CommitmentCommand::ResumeInvocation(ResumeInvocation {
                invocation_id: self.invocation_id.clone(),
                participant: self.participant.clone(),
                generation: self.generation,
            }),
        )?;
        self.settle_wake_offer(&offer_id)?;
        Ok(true)
    }

    /// Record the construction of the result this attempt produced, so the work the run is
    /// accountable for carries the ancestry of the exact result a protected query would be spent
    /// on, and not only a digest.
    ///
    /// Each object the result puts at a path is stated first, as an object stored whole. The bytes
    /// were written into the run's store and are named by digest before any of this is committed,
    /// and a result naming an object nothing recorded is refused rather than formed over bytes that
    /// may never arrive. The bundle is then published against the exact base the work was awarded
    /// from, and the kernel computes the identity of the result from those facts alone. What the
    /// ledger seals is therefore a construction a reader can follow back to the base and the
    /// objects it was built from, on any host and after this process is gone.
    ///
    /// Whether the bundle may be published is settled before the first object is recorded, and it
    /// is settled by the kernel: the command is put to the ledger for a decision that commits
    /// nothing. An object recorded for a bundle that is then refused for its shape would stay in
    /// the record with nothing naming it, so no such fact is written. The one refusal that is
    /// expected here is that the objects are not yet stored whole, which is exactly what the next
    /// step records.
    ///
    /// One construction the kernel cannot state is the one that exceeds what a single record holds:
    /// a bundle carries at most [`ymp_domain::commitment::MAX_BUNDLE_CHANGES`] path changes, and a
    /// path at most [`ymp_domain::commitment::MAX_PATH_BYTES`] bytes in a shape the protocol
    /// admits. Nothing about such a result is
    /// recorded — not the objects, not a seal — and the caller is told which bound was exceeded, so
    /// that the run states it where an operator reads it rather than sealing a digest that reads
    /// like a run from before a construction was ever journalled.
    pub fn submitted(&self, construction: &CandidateConstruction) -> anyhow::Result<Submission> {
        self.advance_clock()?;
        let bundle = CommitmentCommand::SubmitBundle(SubmitBundle {
            contract_id: self.contract_id.clone(),
            attempt_id: self.attempt_id.clone(),
            participant: self.participant.clone(),
            generation: self.generation,
            base_digest: construction.base_digest.clone(),
            // A first attempt carries no result forward: this run has one participant and one
            // attempt, and a rebase or a synthesis is somebody else's submission.
            parents: Vec::new(),
            changes: construction.changes.clone(),
        });
        match self.snapshot()?.decide(&bundle) {
            // Admitted as it stands, or admitted once the objects it names are recorded.
            Ok(_) | Err(CommitmentError::ObjectIncomplete { .. }) => {}
            Err(error) => {
                return match unrecorded_provenance(&error, construction) {
                    Some(reason) => Ok(Submission::ProvenanceUnrecorded(reason)),
                    None => Err(ApplicationError::Commitment(error).into()),
                };
            }
        }
        let objects: BTreeSet<&str> = construction
            .changes
            .iter()
            .filter_map(|change| match &change.change {
                PathChange::Upsert { object_digest, .. } => Some(object_digest.as_str()),
                PathChange::Delete => None,
            })
            .collect();
        for (index, object_digest) in objects.into_iter().enumerate() {
            self.commit(
                &format!("object-{index}"),
                &CommitmentCommand::RecordObject(RecordObject {
                    contract_id: self.contract_id.clone(),
                    participant: self.participant.clone(),
                    generation: self.generation,
                    object_digest: object_digest.to_owned(),
                }),
            )?;
        }
        self.commit("bundle", &bundle)?;
        Ok(Submission::Recorded)
    }

    /// Record what a protected query decided about the candidate this run committed, and close the
    /// accounting that decision settles.
    ///
    /// The decision enters the kernel as a committed fact rather than staying a report the
    /// controller holds, because the terminal state of the run is derived from it and from nothing
    /// else: a run whose candidate passed the approved oracle at root scope reaches `Accepted`, and
    /// one whose candidate was rejected reaches `Exhausted` — quiet, a spent budget and a
    /// contractor's own result reach neither. The verdict is attached to the exact bundle the work
    /// recorded, since the kernel compares the digest and refuses a verdict aimed at any other.
    ///
    /// The work obligation is then returned and the offer settled, so a run that has been answered
    /// stops holding itself open on work nothing is doing. It is called after the process slice has
    /// ended: a slice still running is itself what the run waits for, and no verdict shortens that.
    ///
    /// One result is named twice in this store, and a verdict has to be recorded against the name
    /// the kernel sealed. The run's journal names the result by the snapshot a candidate is
    /// materialized from, which is what a protected query is run against and what its verdict is
    /// therefore issued for; the kernel names it by the digest of the construction that produced it,
    /// which is what its own facts formed. Where the run's record says the two are one result, the
    /// verdict is recorded against the sealed construction. A digest neither record states is
    /// carried to the kernel as it stands and refused there, so what a verdict may be attached to is
    /// still decided by the kernel and not here.
    pub fn verified(&self, candidate_digest: &str, verdict: Verdict) -> anyhow::Result<()> {
        self.advance_clock()?;
        let candidate_digest = &self.sealed_result(candidate_digest)?;
        self.commit(
            "verification",
            &CommitmentCommand::RecordVerification(RecordVerification {
                contract_id: self.contract_id.clone(),
                participant: self.participant.clone(),
                generation: self.generation,
                candidate_digest: candidate_digest.to_owned(),
                verdict,
            }),
        )?;
        let ledger = self.snapshot()?;
        if ledger
            .contracts()
            .get(&self.contract_id)
            .is_some_and(|contract| contract.state == ContractState::Active)
        {
            self.commit(
                "verified-return",
                &CommitmentCommand::ReturnObligation(ReturnObligation {
                    contract_id: self.contract_id.clone(),
                    participant: self.participant.clone(),
                    generation: self.generation,
                    outcome: Outcome::Result {
                        candidate_digest: candidate_digest.to_owned(),
                    },
                }),
            )?;
        }
        if ledger
            .offers()
            .get(&self.offer_id)
            .is_some_and(|offer| offer.state != OfferState::Settled)
        {
            self.commit(
                "verified-settle",
                &CommitmentCommand::SettleOffer(SettleOffer {
                    offer_id: self.offer_id.clone(),
                    sponsor: self.sponsor.clone(),
                }),
            )?;
        }
        Ok(())
    }

    /// End the slice for good and wind the run down as far as what happened allows.
    ///
    /// A completed slice leaves its work obligation open: whether its candidate is accepted is
    /// decided by a protected query this controller does not perform, and a run that closed its
    /// own accounting on quiet would be claiming an answer nobody produced. A stopped or failed run
    /// has no such answer coming, so it is stopped, returned and settled here, and the kernel
    /// states the terminal it reached.
    /// Every step here is skipped when the facts already record it, so a run that reaches its
    /// terminal from more than one place — the loop that saw the last event, and the supervision
    /// that failed on its way out — records that terminal once.
    pub fn terminated(&self, termination: ManagedTermination) -> anyhow::Result<()> {
        self.advance_clock()?;
        if self.invocation_state()? != InvocationState::Closed {
            let reason = match termination {
                ManagedTermination::Completed => InvocationClosure::Completed,
                ManagedTermination::Cancelled => InvocationClosure::Cancelled,
                ManagedTermination::CancelledPastLimit => InvocationClosure::LimitExceeded,
                ManagedTermination::Failed(reason) => reason,
            };
            self.commit(
                "close",
                &CommitmentCommand::CloseInvocation(CloseInvocation {
                    invocation_id: self.invocation_id.clone(),
                    closer: self.participant.clone(),
                    reason,
                }),
            )?;
        }
        let open_offer = self
            .open_wake_offer
            .lock()
            .map_err(|_| anyhow::anyhow!("the managed kernel lock was poisoned"))?
            .clone();
        if let Some(offer_id) = open_offer {
            self.settle_wake_offer(&offer_id)?;
        }
        let (stop, outcome) = match termination {
            ManagedTermination::Completed => return Ok(()),
            ManagedTermination::Cancelled | ManagedTermination::CancelledPastLimit => {
                (StopReason::Cancelled, Outcome::Cancelled)
            }
            ManagedTermination::Failed(_) => (
                StopReason::InfrastructureError,
                Outcome::InfrastructureError,
            ),
        };
        let ledger = self.snapshot()?;
        if ledger.stopped().is_none() {
            self.commit(
                "stop",
                &CommitmentCommand::StopRun(StopRun {
                    authority: self.sponsor.clone(),
                    reason: stop,
                }),
            )?;
        }
        if ledger
            .contracts()
            .get(&self.contract_id)
            .is_some_and(|contract| contract.state == ContractState::Active)
        {
            self.commit(
                "return",
                &CommitmentCommand::ReturnObligation(ReturnObligation {
                    contract_id: self.contract_id.clone(),
                    participant: self.participant.clone(),
                    generation: self.generation,
                    outcome,
                }),
            )?;
        }
        if self
            .snapshot()?
            .offers()
            .get(&self.offer_id)
            .is_some_and(|offer| offer.state != OfferState::Settled)
        {
            self.commit(
                "settle",
                &CommitmentCommand::SettleOffer(SettleOffer {
                    offer_id: self.offer_id.clone(),
                    sponsor: self.sponsor.clone(),
                }),
            )?;
        }
        Ok(())
    }

    /// Where the slice is in its life, as the committed facts state it.
    pub fn invocation_state(&self) -> anyhow::Result<InvocationState> {
        self.read(|ledger| {
            ledger
                .invocations()
                .get(&self.invocation_id)
                .map(|invocation| invocation.state)
        })?
        .context("the managed run has no recorded process slice")
    }

    /// The yielded slices the controller may admit now, in the order it must admit them.
    pub fn admission_order(&self) -> anyhow::Result<Vec<String>> {
        self.read(|ledger| {
            ledger
                .admission_order()
                .into_iter()
                .map(|invocation| invocation.invocation_id.clone())
                .collect()
        })
    }

    /// The first funded control object that can still advance the run, or `None` when the run is
    /// quiescent.
    pub fn open_authority(&self) -> anyhow::Result<Option<OpenAuthority>> {
        self.read(CommitmentLedger::open_authority)
    }

    /// How the run ended, or `None` while a funded control object can still advance it.
    pub fn root_terminal(&self) -> anyhow::Result<Option<RootTerminal>> {
        self.read(CommitmentLedger::root_terminal)
    }

    pub fn contract_id(&self) -> &str {
        &self.contract_id
    }

    pub fn participant(&self) -> &str {
        &self.participant
    }

    /// A consistent read of the whole ledger, for a caller that has to state what the kernel holds
    /// rather than ask it one question.
    pub fn snapshot(&self) -> anyhow::Result<CommitmentLedger> {
        self.read(Clone::clone)
    }

    /// The identifier the ledger sealed for the result the caller names, where the run's own record
    /// states that the two name one result, and the caller's identifier otherwise.
    ///
    /// Both readings come out of the one store: the journal's candidate is what the run committed,
    /// and the ledger's is what the work sealed for it. Nothing is remembered between them.
    fn sealed_result(&self, candidate_digest: &str) -> anyhow::Result<String> {
        let mut application = self.application();
        if application.state().candidate_digest.as_deref() != Some(candidate_digest) {
            return Ok(candidate_digest.to_owned());
        }
        let sealed = application
            .recovered_commitments()?
            .and_then(|ledger| ledger.contracts().get(&self.contract_id))
            .and_then(|contract| contract.candidate_digest.clone());
        Ok(sealed.unwrap_or_else(|| candidate_digest.to_owned()))
    }

    /// Close one wake offer and return what it held. An offer nobody settles keeps the run open,
    /// so the request a resumption answered is wound down where it was answered.
    fn settle_wake_offer(&self, offer_id: &str) -> anyhow::Result<()> {
        let sequence = self.commands.fetch_add(1, Ordering::Relaxed);
        self.commit(
            &format!("wake-withdraw-{sequence}"),
            &CommitmentCommand::WithdrawOffer(WithdrawOffer {
                offer_id: offer_id.to_owned(),
                sponsor: self.participant.clone(),
            }),
        )?;
        self.commit(
            &format!("wake-settle-{sequence}"),
            &CommitmentCommand::SettleOffer(SettleOffer {
                offer_id: offer_id.to_owned(),
                sponsor: self.participant.clone(),
            }),
        )?;
        *self
            .open_wake_offer
            .lock()
            .map_err(|_| anyhow::anyhow!("the managed kernel lock was poisoned"))? = None;
        Ok(())
    }

    /// Move the kernel clock to the wall time that has actually elapsed. Two controller threads may
    /// reach this in either order, and the one that arrives with the earlier reading changes
    /// nothing rather than moving time backwards.
    fn advance_clock(&self) -> anyhow::Result<()> {
        let elapsed = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        let sequence = self.commands.fetch_add(1, Ordering::Relaxed);
        match self.execute(
            &format!("{}.clock-{sequence}", self.attempt_id),
            &CommitmentCommand::AdvanceClock(AdvanceClock { to: elapsed }),
        ) {
            Ok(()) => Ok(()),
            Err(ApplicationError::Commitment(CommitmentError::ClockRegression { .. })) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// How many facts the ledger holds, which is the cursor a slice records as its position.
    fn cursor(&self) -> anyhow::Result<u64> {
        self.read(CommitmentLedger::sequence)
    }

    fn commit(&self, step: &str, command: &CommitmentCommand) -> anyhow::Result<()> {
        self.execute(&format!("{}.{step}", self.attempt_id), command)?;
        Ok(())
    }

    /// Decide one command against the run's ledger and record what it commits before answering.
    fn execute(
        &self,
        command_id: &str,
        command: &CommitmentCommand,
    ) -> Result<(), ApplicationError> {
        self.application().execute_commitment(command_id, command)?;
        Ok(())
    }

    /// Read one value out of the run's ledger, with the journal caught up with first.
    fn read<T>(&self, read: impl FnOnce(&CommitmentLedger) -> T) -> anyhow::Result<T> {
        let mut application = self.application();
        let ledger = application
            .recovered_commitments()?
            .context("the managed run carries no commitment kernel")?;
        Ok(read(ledger))
    }

    /// Take the lock the run's journal is written under.
    ///
    /// A poisoned lock is taken all the same, for the reason every other ending of this run takes
    /// it poisoned: each fact is appended to the journal whole before it is applied in memory, and
    /// the projection is brought back to the journal at the start of every command, so a panic
    /// under this lock leaves nothing half-written behind it. Refusing here would leave the journal
    /// still able to record how a run ended while the kernel could no longer record it at all —
    /// the two records disagreeing about one run, which is what this path exists to prevent.
    fn application(&self) -> MutexGuard<'_, Application> {
        self.application
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Whether a refusal is the run's own record stating that the run has ended.
///
/// It is the answer to "may anything more be created here", and it is the journal's answer rather
/// than the ledger's: the two records reach their endings at different moments, and what a
/// controller must not do between them is create work for a run an operator has already stopped.
fn run_has_ended(error: &ApplicationError) -> bool {
    matches!(
        error,
        ApplicationError::Transition(TransitionError::Terminal(_))
    )
}

/// Which bound a refused bundle exceeded, where the refusal is about what one submission may state
/// at once rather than about whether this participant may submit at all.
///
/// The bounds are the protocol's own, and its words for them are carried through: an operator reads
/// the rule the kernel applied, together with the number or the path it was applied to.
fn unrecorded_provenance(
    error: &CommitmentError,
    construction: &CandidateConstruction,
) -> Option<UnrecordedProvenance> {
    match error {
        CommitmentError::InvalidBundleSize | CommitmentError::TooManyEntries { .. } => {
            Some(UnrecordedProvenance::TooManyChanges {
                changes: construction.changes.len(),
                refusal: error.to_string(),
            })
        }
        CommitmentError::InvalidPath { path } => Some(UnrecordedProvenance::UnacceptablePath {
            path: path.clone(),
            refusal: error.to_string(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tempfile::TempDir;
    use ymp_application::{Application, CandidateConstruction};
    use ymp_domain::commitment::{BundleChange, CommitmentEvent, InvocationState, PathChange};
    use ymp_domain::{Budget, Command, EventKind};

    use super::{
        CloseInvocation, CommitmentCommand, InvocationClosure, ManagedKernel, ManagedTermination,
        RootTerminal, StartInvocation, Submission, UnrecordedProvenance, WakeCondition,
        YieldInvocation, digest_bytes,
    };
    use ymp_domain::commitment::{MAX_ATTEMPT_WAKES, MAX_BUNDLE_CHANGES};

    /// A second yielded slice of the same attempt, waiting on the submission this run commits.
    /// Whether it is admitted before or after the controller's own slice is decided by which of the
    /// two yields the facts recorded first, which is what these tests vary.
    const OTHER_SLICE: &str = "invocation-other";

    /// One store holding one run, which is where the kernel of that run records everything.
    ///
    /// The directory is returned with the kernel: the journal is a file in it, so a store dropped
    /// while the kernel is still in use would take the record with it.
    fn store() -> (TempDir, Arc<Mutex<Application>>) {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let application =
            Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
                .expect("create the run");
        (temporary, Arc::new(Mutex::new(application)))
    }

    /// The construction of one result, stated as a run's own record states it: the base the work
    /// was awarded from, and the exact object standing at the one path the result changed.
    fn construction(tag: &[u8]) -> CandidateConstruction {
        CandidateConstruction {
            base_digest: digest_bytes(b"base"),
            candidate_digest: digest_bytes(tag),
            changes: vec![BundleChange {
                path: "result.txt".to_owned(),
                change: PathChange::Upsert {
                    object_digest: digest_bytes(tag),
                    executable: false,
                },
            }],
        }
    }

    fn prepared() -> (TempDir, ManagedKernel) {
        let (temporary, application) = store();
        let kernel = ManagedKernel::prepare(
            application,
            "attempt-order",
            "invocation-order",
            "scope-order",
            &digest_bytes(b"base"),
            &digest_bytes(b"intent"),
        )
        .expect("the managed run ledger");
        (temporary, kernel)
    }

    /// Begin and yield a second slice of this attempt, waiting on the submission of its work.
    fn yield_other_slice(kernel: &ManagedKernel, step: &str) {
        kernel
            .commit(
                &format!("{step}-start"),
                &CommitmentCommand::StartInvocation(StartInvocation {
                    invocation_id: OTHER_SLICE.to_owned(),
                    attempt_id: kernel.attempt_id.clone(),
                    contract_id: kernel.contract_id.clone(),
                    participant: kernel.participant.clone(),
                    generation: kernel.generation,
                    cursor: 0,
                }),
            )
            .expect("a slice of this attempt begins");
        kernel
            .commit(
                &format!("{step}-yield"),
                &CommitmentCommand::YieldInvocation(YieldInvocation {
                    invocation_id: OTHER_SLICE.to_owned(),
                    participant: kernel.participant.clone(),
                    generation: kernel.generation,
                    cursor: 0,
                    conditions: vec![WakeCondition::SubmissionRecorded {
                        contract_id: kernel.contract_id.clone(),
                    }],
                    wake_deadline: kernel.lease_expires_at,
                }),
            )
            .expect("that slice registers what would be worth resuming it for");
    }

    /// The controller refuses to resume its own slice while the kernel puts another one first.
    ///
    /// Two slices of this run wait on the same committed fact, and the queue is ordered by the
    /// record: the one whose yield was recorded first is admitted first. Here the other slice
    /// yielded first, so the controller asks for its own and is told which slice the kernel admits
    /// instead — the instruction never reaches the runtime and the slice stays where it was.
    #[test]
    fn a_wake_out_of_the_kernel_order_is_refused_and_resumes_nothing() {
        let (_store, kernel) = prepared();
        yield_other_slice(&kernel, "other");
        kernel.start_invocation().expect("the controller's slice");
        // The fact the other slice waits on. From here the kernel has something to admit it for.
        kernel
            .submitted(&construction(b"candidate"))
            .expect("the candidate");
        kernel.yielded("cursor-0").expect("the controller's yield");
        assert_eq!(
            kernel.admission_order().expect("the admission order"),
            vec![OTHER_SLICE.to_owned()],
            "the slice that yielded first is not the one the kernel admits first"
        );

        let refusal = kernel
            .admit_wake("wake-1", "continue")
            .expect_err("the controller admitted a slice out of the kernel order");
        assert!(
            refusal
                .to_string()
                .contains(&format!("admits invocation {OTHER_SLICE} before this one")),
            "unexpected refusal: {refusal}"
        );
        let ledger = kernel.snapshot().expect("the ledger");
        assert_eq!(
            ledger.invocations()[&kernel.invocation_id].state,
            InvocationState::Yielded,
            "a refused wake left the slice running"
        );
        assert!(
            !ledger
                .facts()
                .iter()
                .any(|fact| matches!(fact, CommitmentEvent::InvocationResumed { .. })),
            "a refused wake resumed a slice anyway"
        );
    }

    /// The same two slices in the other order, which is what the placement of the check decides.
    ///
    /// The controller's own slice yielded first, so the kernel admits it first — but only once the
    /// agreement its wake carries is a committed fact, because until then that slice is not in the
    /// queue at all and the only identifier the order offers is the other slice's. An order read
    /// before the agreement therefore refuses the very wake the kernel would put first, which is
    /// the half this placement is measured against.
    #[test]
    fn a_wake_the_kernel_puts_first_is_admitted_although_another_slice_is_waiting() {
        let (_store, kernel) = prepared();
        kernel.start_invocation().expect("the controller's slice");
        kernel.yielded("cursor-0").expect("the controller's yield");
        yield_other_slice(&kernel, "other");
        kernel
            .submitted(&construction(b"candidate"))
            .expect("the candidate");
        assert_eq!(
            kernel.admission_order().expect("the admission order"),
            vec![OTHER_SLICE.to_owned()],
            "a slice whose wake nothing has answered is already in the queue"
        );

        assert!(
            kernel
                .admit_wake("wake-1", "continue")
                .expect("the wake the kernel admits first was refused"),
            "the wake resumed nothing"
        );
        assert_eq!(
            kernel.invocation_state().expect("the slice record"),
            InvocationState::Running
        );
    }

    /// The agreement a refused wake carried stays committed, so the same instruction is delivered
    /// again — under the same identifier and without being taken for a repeat — once the slice
    /// ahead of it has left the queue.
    #[test]
    fn a_wake_refused_for_its_turn_is_admitted_once_the_queue_clears() {
        let (_store, kernel) = prepared();
        yield_other_slice(&kernel, "other");
        kernel.start_invocation().expect("the controller's slice");
        kernel
            .submitted(&construction(b"candidate"))
            .expect("the candidate");
        kernel.yielded("cursor-0").expect("the controller's yield");
        assert!(kernel.admit_wake("wake-1", "continue").is_err());

        // The slice ahead is closed for good, which is what takes it out of the queue.
        kernel
            .commit(
                "other-close",
                &CommitmentCommand::CloseInvocation(CloseInvocation {
                    invocation_id: OTHER_SLICE.to_owned(),
                    closer: kernel.participant.clone(),
                    reason: InvocationClosure::Completed,
                }),
            )
            .expect("the other slice ends");
        assert!(
            kernel
                .admit_wake("wake-1", "continue")
                .expect("the wake is now the one the kernel admits"),
            "the re-delivered wake was taken for a repeat and resumed nothing"
        );
        assert_eq!(
            kernel.invocation_state().expect("the slice record"),
            InvocationState::Running
        );
    }

    /// Stop the run the way an operator does: in the run's own journal, which is the record that
    /// states whether anything may still be created and is moved well before the kernel is stopped.
    fn cancel(kernel: &ManagedKernel) {
        kernel
            .application()
            .execute(
                format!("{}.cancel", kernel.attempt_id),
                Command::Cancel {
                    reason: "stopped by the operator".to_owned(),
                },
            )
            .expect("the journal records the cancellation");
    }

    /// A run whose journal has ended offers no more work, and its ending is still recorded.
    ///
    /// This is the sequence an operator produces: a slice is running, the operator stops the run,
    /// and the slice then yields. The yield would advertise the offer its wake is recorded against
    /// — authority for work on a run that has ended, funded out of the run's own accounts — and it
    /// is refused before the ledger is asked. The offers the run holds are therefore the ones it
    /// held when the operator stopped it, and the slice is left where the facts put it rather than
    /// registered for a wake nothing can answer. Everything the ending itself needs still lands:
    /// the slice closes, the run stops, the work returns and the offer settles.
    #[test]
    fn a_cancelled_run_advertises_no_wake_offer_and_still_records_its_ending() {
        let (_store, kernel) = prepared();
        kernel.start_invocation().expect("the controller's slice");
        let before = kernel.snapshot().expect("the ledger").offers().len();

        cancel(&kernel);
        kernel
            .yielded("cursor-0")
            .expect("a yield on a run that has ended registers nothing and reports no failure");

        let ledger = kernel.snapshot().expect("the ledger");
        assert_eq!(
            ledger.offers().len(),
            before,
            "a run that had already ended advertised work anyway"
        );
        assert_eq!(
            kernel.invocation_state().expect("the slice record"),
            InvocationState::Running,
            "the slice was registered for a wake that can never be answered"
        );

        kernel
            .terminated(ManagedTermination::Cancelled)
            .expect("the ending of a run is recorded after the journal states it");
        let ended = kernel.snapshot().expect("the ledger");
        assert_eq!(
            kernel.root_terminal().expect("the terminal"),
            Some(RootTerminal::Cancelled)
        );
        assert_eq!(
            ended.invocations()[&kernel.invocation_id].closure,
            Some(InvocationClosure::Cancelled)
        );
        assert_eq!(
            kernel.open_authority().expect("the open authority"),
            None,
            "a stopped run is still held open"
        );
    }

    /// A wake delivered after the run's journal has ended resumes nothing, and it is refused before
    /// the ledger has been stopped.
    ///
    /// The window between the two records is what this closes. The operator's cancellation moves
    /// the journal at once; the kernel is stopped only when the slice that was running has been
    /// wound down, and a wake arriving in between used to record consent, admit the resumption and
    /// put the runtime back to work on a run that had ended.
    #[test]
    fn a_wake_after_the_run_has_ended_resumes_nothing() {
        let (_store, kernel) = prepared();
        kernel.start_invocation().expect("the controller's slice");
        kernel.yielded("cursor-0").expect("the controller's yield");
        cancel(&kernel);
        assert!(
            kernel.snapshot().expect("the ledger").stopped().is_none(),
            "the kernel was stopped before the wake, which is not the window under test"
        );

        let refusal = kernel
            .admit_wake("wake-1", "continue")
            .expect_err("a run that has ended admitted a wake");
        assert!(
            refusal.to_string().contains("run is already terminal"),
            "unexpected refusal: {refusal}"
        );
        assert_eq!(
            kernel.invocation_state().expect("the slice record"),
            InvocationState::Yielded,
            "a refused wake resumed the slice"
        );
        assert_eq!(resumptions(&kernel), 0);
    }

    /// A submission records the objects the result names and the construction they form, and the
    /// work is sealed with the identity the kernel computed from those facts.
    #[test]
    fn a_submission_records_its_objects_its_bundle_and_the_result_they_form() {
        let (_store, kernel) = prepared();
        kernel.start_invocation().expect("the controller's slice");
        let construction = construction(b"candidate");
        assert_eq!(
            kernel.submitted(&construction).expect("the submission"),
            Submission::Recorded
        );

        let ledger = kernel.snapshot().expect("the ledger");
        let object = match &construction.changes[0].change {
            PathChange::Upsert { object_digest, .. } => object_digest.clone(),
            PathChange::Delete => panic!("the fixture states an object"),
        };
        assert!(
            ledger.facts().iter().any(|fact| matches!(
                fact,
                CommitmentEvent::ObjectRecorded { object_digest } if *object_digest == object
            )),
            "the object the result puts at its path was never recorded whole"
        );
        let bundle = ledger
            .facts()
            .iter()
            .find_map(|fact| match fact {
                CommitmentEvent::BundleRecorded {
                    bundle_digest,
                    base_digest,
                    changes,
                    ..
                } => Some((bundle_digest.clone(), base_digest.clone(), changes.clone())),
                _ => None,
            })
            .expect("the bundle the result was published as");
        assert_eq!(bundle.1, construction.base_digest, "another base");
        assert_eq!(bundle.2, construction.changes, "other changes");
        let candidate = ledger
            .facts()
            .iter()
            .find_map(|fact| match fact {
                CommitmentEvent::CandidateFormed {
                    candidate_digest,
                    bundle_digest,
                    base_digest,
                    ..
                } => Some((
                    candidate_digest.clone(),
                    bundle_digest.clone(),
                    base_digest.clone(),
                )),
                _ => None,
            })
            .expect("the result the bundle constructs");
        assert_eq!(candidate.1, bundle.0, "the result names another bundle");
        assert_eq!(candidate.2, construction.base_digest);
        assert_eq!(
            ledger.contracts()[&kernel.contract_id].candidate_digest,
            Some(candidate.0),
            "the work was sealed with something other than the result it formed"
        );
    }

    /// How many facts of the record are about the result of this work: the objects it names, the
    /// bundle it was published as, the result formed from it, and the seal of the work.
    fn provenance_facts(kernel: &ManagedKernel) -> usize {
        kernel
            .snapshot()
            .expect("the ledger")
            .facts()
            .iter()
            .filter(|fact| {
                matches!(
                    fact,
                    CommitmentEvent::ObjectRecorded { .. }
                        | CommitmentEvent::BundleRecorded { .. }
                        | CommitmentEvent::CandidateFormed { .. }
                        | CommitmentEvent::SubmissionRecorded { .. }
                )
            })
            .count()
    }

    /// A result the kernel cannot state leaves nothing in the record, and the reason names the
    /// bound it exceeded.
    ///
    /// The objects are recorded before the bundle that names them, so a bundle refused for its
    /// shape after they had been written would leave facts behind that nothing refers to. Whether
    /// the bundle may be published is therefore settled first, and a construction that may not be
    /// leaves the work unsealed rather than sealed with a digest whose construction the record
    /// does not hold.
    #[test]
    fn a_result_of_more_changes_than_one_bundle_carries_records_nothing() {
        let (_store, kernel) = prepared();
        kernel.start_invocation().expect("the controller's slice");
        let mut construction = construction(b"candidate");
        construction.changes = (0..=MAX_BUNDLE_CHANGES)
            .map(|index| BundleChange {
                path: format!("file-{index}.txt"),
                change: PathChange::Upsert {
                    object_digest: digest_bytes(format!("object-{index}").as_bytes()),
                    executable: false,
                },
            })
            .collect();

        match kernel.submitted(&construction).expect("the submission") {
            Submission::ProvenanceUnrecorded(UnrecordedProvenance::TooManyChanges {
                changes,
                refusal,
            }) => {
                assert_eq!(changes, MAX_BUNDLE_CHANGES + 1);
                assert!(
                    refusal.contains(&MAX_BUNDLE_CHANGES.to_string()),
                    "the reason does not state the bound it exceeded: {refusal}"
                );
            }
            other => {
                panic!("a construction of {MAX_BUNDLE_CHANGES} + 1 changes was taken: {other:?}")
            }
        }
        assert_eq!(
            provenance_facts(&kernel),
            0,
            "a construction the kernel cannot state left facts about the result behind"
        );
        assert_eq!(
            kernel.snapshot().expect("the ledger").contracts()[&kernel.contract_id]
                .candidate_digest,
            None,
            "the work was sealed with a result whose construction the record does not hold"
        );
    }

    /// The same, for a path the protocol does not admit. Here the count is within the bound, so
    /// nothing but the shape of one path stops the bundle — and the objects would already have been
    /// recorded if the bundle were put to the kernel only after them.
    #[test]
    fn a_result_naming_a_path_the_protocol_refuses_records_nothing() {
        let (_store, kernel) = prepared();
        kernel.start_invocation().expect("the controller's slice");
        let mut construction = construction(b"candidate");
        construction.changes[0].path = "dir\\result.txt".to_owned();

        match kernel.submitted(&construction).expect("the submission") {
            Submission::ProvenanceUnrecorded(UnrecordedProvenance::UnacceptablePath {
                path,
                ..
            }) => assert_eq!(path, "dir\\result.txt"),
            other => panic!("a path the protocol refuses was taken: {other:?}"),
        }
        assert_eq!(
            provenance_facts(&kernel),
            0,
            "the objects of a bundle that was never published stayed in the record"
        );
    }

    /// Every commitment record of this store, in journal order.
    fn journalled(application: &Arc<Mutex<Application>>) -> Vec<(String, Vec<CommitmentEvent>)> {
        application
            .lock()
            .expect("the application")
            .events_after(0)
            .expect("read the committed records")
            .into_iter()
            .filter_map(|envelope| match envelope.event {
                EventKind::CommitmentFactsRecorded { facts } => Some((envelope.command_id, facts)),
                _ => None,
            })
            .collect()
    }

    /// How many participants ask for the same slice at once.
    const CONTENDERS: usize = 8;
    /// How many times the run is brought back to the state where they can. It is the number of
    /// resumptions one attempt is funded for, which is the most contention this run can be put
    /// through without inventing authority it was never given.
    const ROUNDS: u32 = MAX_ATTEMPT_WAKES;

    /// How many resumptions the record holds.
    fn resumptions(kernel: &ManagedKernel) -> usize {
        kernel
            .snapshot()
            .expect("the ledger")
            .facts()
            .iter()
            .filter(|fact| matches!(fact, CommitmentEvent::InvocationResumed { .. }))
            .count()
    }

    /// Participants contending for one slice commit through one order, and the record holds each
    /// command once and whole.
    ///
    /// They contend on one aggregate on purpose. Commands that touch different things can be
    /// decided against each other's stale readings and still commit facts that contradict nothing,
    /// which is why two participants doing unrelated work prove little about the order they were
    /// serialized into. Here every participant asks for the same process slice, and the slice is
    /// what each of them reads before deciding: whether it may be resumed is true for exactly one
    /// of them, and it stops being true the moment that one commits.
    ///
    /// Four things are required of the record. Exactly one participant per round is told the slice
    /// resumed, and the record holds exactly that many resumptions, so no reading that had been
    /// overtaken was acted on. Each command wrote exactly one record, so no fact was committed
    /// twice and none was lost. The ledger the run holds is that record read in order, fact for
    /// fact. And a restart rebuilds the same ledger from the record alone.
    ///
    /// The check that must fail: decide a command outside the lock the journal is written under —
    /// read the ledger, release the lock, and append afterwards — and two participants decide the
    /// same resumption against the same reading. Both are told the slice resumed and the record
    /// holds two resumptions of one slice, which the counts here state directly.
    ///
    /// What that failure depends on is measured rather than assumed, and the measurements do not
    /// agree with each other. The review of this build measured 26 runs in 30 with a scheduling
    /// point between the decision and the append, and 0 in 30 with the two back to back. Measured
    /// here against a mutation whose decision goes through the journal-backed read the durable path
    /// itself uses — the projection is caught up with, the ledger is cloned, the lock is released —
    /// it is 30 in 30 either way, because that reading is itself wide enough for another
    /// participant to be scheduled inside it.
    ///
    /// What follows is the bound of the claim rather than its strength: how often an unlocked
    /// decision is caught depends on how much work sits between the decision and the append, and a
    /// window narrow enough can hide from a check of any shape. What this check establishes is that
    /// the failure is a difference in the record — two resumptions of one slice — and not something
    /// a caller has to be watching for at the right moment.
    #[test]
    fn participants_contending_for_one_slice_leave_every_fact_once_and_in_one_order() {
        let (temporary, application) = store();
        let root = temporary.path().join("data");
        let kernel = Arc::new(
            ManagedKernel::prepare(
                Arc::clone(&application),
                "attempt-race",
                "invocation-race",
                "scope-race",
                &digest_bytes(b"base"),
                &digest_bytes(b"intent"),
            )
            .expect("the managed run ledger"),
        );
        kernel.start_invocation().expect("the controller's slice");
        let before = journalled(&application).len();

        for round in 0..ROUNDS {
            kernel
                .yielded(&format!("cursor-{round}"))
                .expect("the slice yields");
            let start = Arc::new(std::sync::Barrier::new(CONTENDERS));
            let asking: Vec<_> = (0..CONTENDERS)
                .map(|contender| {
                    let kernel = Arc::clone(&kernel);
                    let start = Arc::clone(&start);
                    std::thread::Builder::new()
                        .spawn(move || {
                            start.wait();
                            kernel.admit_wake(&format!("wake-{round}-{contender}"), "continue")
                        })
                        .expect("a participant asking for the slice")
                })
                .collect();
            let admitted = asking
                .into_iter()
                .map(|handle| handle.join().expect("a participant asking for the slice"))
                .filter(|answer| matches!(answer, Ok(true)))
                .count();
            assert_eq!(
                admitted, 1,
                "round {round}: {CONTENDERS} participants asked for one slice and it resumed \
                 {admitted} times"
            );
            assert_eq!(
                resumptions(&kernel),
                round as usize + 1,
                "round {round}: the record holds a resumption for a participant that was refused"
            );
            assert_eq!(
                kernel.invocation_state().expect("the slice record"),
                InvocationState::Running
            );
        }

        let records = journalled(&application);
        assert!(records.len() > before, "no participant reached the record");
        let identifiers: std::collections::BTreeSet<&String> =
            records.iter().map(|(command_id, _)| command_id).collect();
        assert_eq!(
            identifiers.len(),
            records.len(),
            "a command was written to the record more than once"
        );
        let written: Vec<CommitmentEvent> = records
            .iter()
            .flat_map(|(_, facts)| facts.iter().cloned())
            .collect();
        let held = kernel.snapshot().expect("the ledger");
        assert_eq!(
            held.facts(),
            written.as_slice(),
            "the ledger the run holds is not the record it wrote, fact for fact and in order"
        );

        // The record on its own, read by a process that carried nothing over from this one.
        drop(kernel);
        drop(application);
        let reopened = Application::open(&root).expect("reopen the store");
        assert_eq!(
            reopened.commitments().expect("the kernel is recovered"),
            &held,
            "a restart rebuilt a different ledger from the same record"
        );
    }

    /// Two wakes arrive at once and the slice resumes once.
    ///
    /// Both participants find the slice yielded and both ask for it, so what decides the answer is
    /// the order the two commands are committed in and nothing either caller read beforehand. One
    /// of them is told the slice resumed; the other is refused, because by the time its command is
    /// decided the slice is running. The record holds one resumption, which is what a resumption
    /// committed twice — or committed by a command whose caller was told it was refused — would
    /// break.
    #[test]
    fn two_wakes_arriving_at_once_resume_the_slice_once() {
        let (_store, kernel) = prepared();
        let kernel = Arc::new(kernel);
        kernel.start_invocation().expect("the controller's slice");
        kernel.yielded("cursor-0").expect("the controller's yield");

        let start = Arc::new(std::sync::Barrier::new(2));
        let waking: Vec<_> = ["wake-a", "wake-b"]
            .into_iter()
            .map(|wake_id| {
                let kernel = Arc::clone(&kernel);
                let start = Arc::clone(&start);
                std::thread::Builder::new()
                    .spawn(move || {
                        start.wait();
                        kernel.admit_wake(wake_id, "continue")
                    })
                    .expect("a participant asking for the slice")
            })
            .collect();
        let admitted = waking
            .into_iter()
            .map(|handle| handle.join().expect("a participant asking for the slice"))
            .filter(|answer| matches!(answer, Ok(true)))
            .count();

        assert_eq!(
            admitted, 1,
            "two wakes arriving at once did not resume the slice exactly once"
        );
        assert_eq!(
            kernel.invocation_state().expect("the slice record"),
            InvocationState::Running
        );
        let resumptions = kernel
            .snapshot()
            .expect("the ledger")
            .facts()
            .iter()
            .filter(|fact| matches!(fact, CommitmentEvent::InvocationResumed { .. }))
            .count();
        assert_eq!(
            resumptions, 1,
            "the record holds a resumption for each participant that asked"
        );
    }
}

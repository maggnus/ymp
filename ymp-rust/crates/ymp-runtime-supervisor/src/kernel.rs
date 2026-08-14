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

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use anyhow::{Context, bail};
use ymp_application::{CommitmentService, CommitmentServiceError};
use ymp_domain::commitment::{
    AdvanceClock, Advertise, Award, BudgetVector, CloseInvocation, CommitmentCommand,
    CommitmentError, CommitmentLedger, ContractState, Dimension, FundingSource, InvocationClosure,
    InvocationState, MAX_LEASE_MS, OfferPolicy, OfferState, OpenAuthority, Outcome, RecordBid,
    RecordVerification, RegisterParticipant, ResumeInvocation, ReturnObligation, RootTerminal,
    SettleOffer, StartAttempt, StartInvocation, StopReason, StopRun, SubmitResult, Verdict,
    WakeCondition, WithdrawOffer, YieldInvocation,
};
use ymp_domain::digest_bytes;

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
    service: CommitmentService,
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
    pub fn prepare(
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
        let ledger =
            CommitmentLedger::new(&sponsor, SPONSOR_PRINCIPAL, &root_obligation, root_budget())
                .context("the managed run ledger")?;
        let kernel = Self {
            service: CommitmentService::new(ledger),
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
    pub fn yielded(&self, yield_command_id: &str) -> anyhow::Result<()> {
        self.advance_clock()?;
        let sequence = self.commands.fetch_add(1, Ordering::Relaxed);
        let wake_offer = format!("wake-{sequence}-{}", self.attempt_id);
        self.commit(
            &format!("wake-offer-{sequence}"),
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
        )?;
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
            self.service.execute(
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
        self.service.execute(
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

    /// Record the candidate the controller committed for this attempt, so the work the run is
    /// accountable for carries the exact result a protected query would be spent on.
    pub fn submitted(&self, candidate_digest: &str) -> anyhow::Result<()> {
        self.advance_clock()?;
        self.commit(
            "submission",
            &CommitmentCommand::SubmitResult(SubmitResult {
                contract_id: self.contract_id.clone(),
                attempt_id: self.attempt_id.clone(),
                participant: self.participant.clone(),
                generation: self.generation,
                candidate_digest: candidate_digest.to_owned(),
            }),
        )?;
        Ok(())
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
    pub fn verified(&self, candidate_digest: &str, verdict: Verdict) -> anyhow::Result<()> {
        self.advance_clock()?;
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
        let ledger = self.service.snapshot()?;
        let invocation = ledger
            .invocations()
            .get(&self.invocation_id)
            .context("the managed run has no recorded process slice")?;
        Ok(invocation.state)
    }

    /// The yielded slices the controller may admit now, in the order it must admit them.
    pub fn admission_order(&self) -> anyhow::Result<Vec<String>> {
        Ok(self.service.admission_order()?)
    }

    /// The first funded control object that can still advance the run, or `None` when the run is
    /// quiescent.
    pub fn open_authority(&self) -> anyhow::Result<Option<OpenAuthority>> {
        Ok(self.service.open_authority()?)
    }

    /// How the run ended, or `None` while a funded control object can still advance it.
    pub fn root_terminal(&self) -> anyhow::Result<Option<RootTerminal>> {
        Ok(self.service.root_terminal()?)
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
        Ok(self.service.snapshot()?)
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
        match self.service.execute(
            &format!("clock-{sequence}"),
            &CommitmentCommand::AdvanceClock(AdvanceClock { to: elapsed }),
        ) {
            Ok(_) => Ok(()),
            Err(CommitmentServiceError::Refused(CommitmentError::ClockRegression { .. })) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn cursor(&self) -> anyhow::Result<u64> {
        Ok(self.service.committed_facts()? as u64)
    }

    fn commit(&self, step: &str, command: &CommitmentCommand) -> anyhow::Result<()> {
        self.service
            .execute(&format!("{}.{step}", self.attempt_id), command)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ymp_domain::commitment::{CommitmentEvent, InvocationState};

    use super::{
        CloseInvocation, CommitmentCommand, InvocationClosure, ManagedKernel, StartInvocation,
        WakeCondition, YieldInvocation, digest_bytes,
    };

    /// A second yielded slice of the same attempt, waiting on the submission this run commits.
    /// Whether it is admitted before or after the controller's own slice is decided by which of the
    /// two yields the facts recorded first, which is what these tests vary.
    const OTHER_SLICE: &str = "invocation-other";

    fn prepared() -> ManagedKernel {
        ManagedKernel::prepare(
            "attempt-order",
            "invocation-order",
            "scope-order",
            &digest_bytes(b"base"),
            &digest_bytes(b"intent"),
        )
        .expect("the managed run ledger")
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
        let kernel = prepared();
        yield_other_slice(&kernel, "other");
        kernel.start_invocation().expect("the controller's slice");
        // The fact the other slice waits on. From here the kernel has something to admit it for.
        kernel
            .submitted(&digest_bytes(b"candidate"))
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
        let kernel = prepared();
        kernel.start_invocation().expect("the controller's slice");
        kernel.yielded("cursor-0").expect("the controller's yield");
        yield_other_slice(&kernel, "other");
        kernel
            .submitted(&digest_bytes(b"candidate"))
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
        let kernel = prepared();
        yield_other_slice(&kernel, "other");
        kernel.start_invocation().expect("the controller's slice");
        kernel
            .submitted(&digest_bytes(b"candidate"))
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
}

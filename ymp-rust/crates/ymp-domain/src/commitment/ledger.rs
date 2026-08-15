//! The local-commitment kernel: it decides whether a typed command preserves the mechanical
//! invariants, and commits every fact of an accepted command together or none of them.
//!
//! Deciding is a pure function of the current ledger and the command. A refused command therefore
//! leaves the ledger byte for byte as it was, which is what makes contract formation, escrow
//! transfer, the first lease and the child obligation one indivisible step.
//!
//! What the kernel decides is accounting, ownership and order: whether capacity exists, whether
//! consent is live, whether a fencing token is current, whether causal work is closed. What it
//! never decides is who should do the work. An award names the consent the sponsor chose; an open
//! acceptance is whichever mechanically valid acceptance was serialized first. No transition
//! ranks, scores, compares or interprets two participants.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::budget::{BudgetVector, Dimension};
use super::candidates::{
    BundleChange, BundleRecord, CandidateRecord, ConflictRecord, Contribution, MAX_BUNDLE_CHANGES,
    MAX_BUNDLE_PARENTS, MAX_CANDIDATE_CHANGES, MAX_PATH_BYTES, PathChange, change_map,
    merge_contributions, ordered_changes,
};
use super::invocations::{
    InvocationClosure, InvocationRecord, InvocationState, OpenAuthority, RootTerminal, StopReason,
    Verdict, VerificationRecord, WakeCondition, WakeRegistration,
};
use super::protocol::{
    AcceptOpen, AdvanceClock, Advertise, Award, CancelContract, CloseInvocation, CommitmentCommand,
    CommitmentError, CommitmentEvent, MAX_ATTEMPT_WAKES, MAX_AWARDS, MAX_LEASE_MS,
    MAX_SCOPE_ENTRIES, MAX_WAKE_CONDITIONS, Reassign, RecordBid, RecordConflict, RecordObject,
    RecordVerification, RegisterParticipant, RenewLease, ResumeInvocation, ReturnObligation,
    SettleOffer, StartAttempt, StartInvocation, StopRun, SubmitBundle, SubmitResult, WithdrawBid,
    WithdrawOffer, YieldInvocation,
};
use super::records::{
    AccountRef, AttemptRecord, AttemptState, BidOrigin, BidRecord, BidState, ContractState,
    FundingSource, LeaseRecord, ObligationRecord, ObligationState, OfferPolicy, OfferRecord,
    OfferState, Outcome, ParticipantRecord, TaskContractRecord,
};
use crate::MAX_IDENTIFIER_CHARS;

/// The identifiers a caller supplies for the objects one award brings into existence, and the
/// wall time it buys the first lease with. They are caller-supplied so that forming a contract
/// stays a deterministic function of the command and can be retried by identity.
#[derive(Clone, Copy, Debug)]
struct Formation<'a> {
    contract_id: &'a str,
    obligation_id: &'a str,
    lease_id: &'a str,
    lease_ms: u64,
}

/// A guarded invariant check, named so that a deliberate removal can be exercised by the property
/// suite instead of being asserted in prose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Check {
    /// One award per funded slot, and one contract per recorded consent.
    Reservation,
    /// Only the current fencing generation may advance a task contract.
    Fencing,
    /// Causal work closes from the leaves inward.
    ChildReturn,
    /// Only a live task contract is an account that capacity may be drawn from or settled into.
    LiveAccount,
    /// A task contract stays open while a reservation is still due back to it.
    ReturningReservation,
    /// An account holds what a command is about to move out of it before that command is decided.
    Covering,
    /// Only the participant holding the lease may close the obligation of a task contract.
    ReturnHolder,
    /// Every process slice, first or resumed, is bought with one unit of creation authority.
    InvocationStart,
    /// A yielded slice resumes only after a committed fact its own registration named.
    WakeMatch,
    /// The slices of one attempt may be resumed only a bounded number of times.
    WakeCount,
    /// A stopped run counts no yielded slice as a funded wake.
    StoppedWake,
    /// One attempt runs one process slice at a time.
    SingleRunningSlice,
    /// The result a task contract recorded is never replaced by another.
    CandidateSeal,
    /// A result is formed only on the exact base its task contract was awarded from.
    CandidateBase,
    /// Where the contributions a bundle carries forward disagree, the bundle itself states what
    /// stands there. The kernel does not settle a disagreement in favour of either side.
    ConflictResolution,
}

/// Checks a test build may switch off to prove that each one is load-bearing. The field does not
/// exist outside `cfg(test)`, so a release build has no way to reach a weakened kernel.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DisabledChecks {
    pub reservation: bool,
    pub fencing: bool,
    pub child_return: bool,
    pub live_account: bool,
    pub returning_reservation: bool,
    pub covering: bool,
    pub return_holder: bool,
    pub invocation_start: bool,
    pub wake_match: bool,
    pub wake_count: bool,
    pub stopped_wake: bool,
    pub single_running_slice: bool,
    pub candidate_seal: bool,
    pub candidate_base: bool,
    pub conflict_resolution: bool,
}

/// Deliberate corruptions of the facts an accepted command emits, which a test build may switch on
/// one at a time. A corrupted fact is committed like any other, so every record the kernel keeps is
/// built from it and agrees with it: what such a fact can be caught by is the command it came from,
/// and nothing else. The field does not exist outside `cfg(test)`, so a release build has no way to
/// reach a kernel that states anything but what it decided.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AlteredFacts {
    /// Move one unit of money less into a task contract than the consent the award named asked for.
    pub understated_escrow: bool,
    /// Take the escrow an award transfers out of the sponsor's own balance instead of out of the
    /// reservation the offer holds. The quantity and the destination stay exactly as commanded, so
    /// nothing but the source account of the fact is wrong.
    pub redirected_escrow: bool,
    /// Return what a closing task contract still holds to the participant that was executing it,
    /// instead of to the account that funded the offer it was awarded from. The contract is emptied
    /// either way, so nothing but the destination account of the fact is wrong.
    pub misdirected_settlement: bool,
    /// State that the task contract a verdict was recorded against hangs directly under the root
    /// obligation, wherever it actually hangs. The verdict, the candidate and the reservation it
    /// spends stay exactly as decided, so nothing but the scope the fact claims is wrong — and the
    /// terminal state a run reports is derived from that claim.
    pub overstated_verification_scope: bool,
    /// State a formed result as carrying no change at all, wherever it in fact puts objects. The
    /// identifiers the fact carries stay exactly as they were computed, so what is wrong is that
    /// the construction the record states no longer reaches the identity it claims — and every
    /// record built from that fact agrees with it.
    pub unstated_changes: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommitmentLedger {
    now: u64,
    initial_total: BudgetVector,
    consumed: BudgetVector,
    /// The obligation the whole run is accountable for. Work hanging directly under it is what a
    /// root-scope result and a root-scope verdict are about.
    root_obligation: String,
    participants: BTreeMap<String, ParticipantRecord>,
    offers: BTreeMap<String, OfferRecord>,
    bids: BTreeMap<String, BidRecord>,
    contracts: BTreeMap<String, TaskContractRecord>,
    obligations: BTreeMap<String, ObligationRecord>,
    attempts: BTreeMap<String, AttemptRecord>,
    invocations: BTreeMap<String, InvocationRecord>,
    verifications: Vec<VerificationRecord>,
    /// The objects this run has been told are stored whole. A bundle may name no other.
    objects: BTreeSet<String>,
    /// Every bundle published, by its content digest.
    bundles: BTreeMap<String, BundleRecord>,
    /// Every result formed, by its identity. Nothing removes an entry and nothing rewrites one:
    /// competing results stand beside each other here, and the kernel prefers none of them.
    candidates: BTreeMap<String, CandidateRecord>,
    /// Every recorded disagreement between candidates, by its digest.
    conflicts: BTreeMap<String, ConflictRecord>,
    stopped: Option<StopReason>,
    /// Every committed fact in the order it was committed. Its position, counted from one, is the
    /// run sequence a cursor names.
    ///
    /// This is the authority a wake is decided against. A notification channel may coalesce,
    /// duplicate, delay or drop what it carries without any of that being recoverable, so nothing
    /// here is derived from one: what a participant missed is whatever lies after the cursor it
    /// last recorded, and that question is answered from this list.
    facts: Vec<CommitmentEvent>,
    #[cfg(test)]
    #[serde(skip)]
    disabled: DisabledChecks,
    #[cfg(test)]
    #[serde(skip)]
    altered: AlteredFacts,
}

impl CommitmentLedger {
    /// A ledger holding one root participant, its whole budget, and the root work obligation the
    /// run is accountable for.
    pub fn new(
        root_participant: &str,
        root_principal: &str,
        root_obligation: &str,
        budget: BudgetVector,
    ) -> Result<Self, CommitmentError> {
        validate_identifier("participant_id", root_participant)?;
        validate_identifier("principal_id", root_principal)?;
        validate_identifier("obligation_id", root_obligation)?;
        let participants = BTreeMap::from([(
            root_participant.to_owned(),
            ParticipantRecord {
                participant_id: root_participant.to_owned(),
                principal_id: root_principal.to_owned(),
                balance: budget,
            },
        )]);
        let obligations = BTreeMap::from([(
            root_obligation.to_owned(),
            ObligationRecord {
                obligation_id: root_obligation.to_owned(),
                parent: None,
                owner: root_participant.to_owned(),
                contract_id: None,
                children: Vec::new(),
                state: ObligationState::Active,
                outcome: None,
            },
        )]);
        Ok(Self {
            now: 0,
            initial_total: budget,
            consumed: BudgetVector::ZERO,
            root_obligation: root_obligation.to_owned(),
            participants,
            offers: BTreeMap::new(),
            bids: BTreeMap::new(),
            contracts: BTreeMap::new(),
            obligations,
            attempts: BTreeMap::new(),
            invocations: BTreeMap::new(),
            verifications: Vec::new(),
            objects: BTreeSet::new(),
            bundles: BTreeMap::new(),
            candidates: BTreeMap::new(),
            conflicts: BTreeMap::new(),
            stopped: None,
            facts: Vec::new(),
            #[cfg(test)]
            disabled: DisabledChecks::default(),
            #[cfg(test)]
            altered: AlteredFacts::default(),
        })
    }

    pub const fn now(&self) -> u64 {
        self.now
    }

    pub const fn initial_total(&self) -> &BudgetVector {
        &self.initial_total
    }

    /// Capacity that has left the accounts for good.
    pub const fn consumed(&self) -> &BudgetVector {
        &self.consumed
    }

    pub const fn participants(&self) -> &BTreeMap<String, ParticipantRecord> {
        &self.participants
    }

    pub const fn offers(&self) -> &BTreeMap<String, OfferRecord> {
        &self.offers
    }

    pub const fn bids(&self) -> &BTreeMap<String, BidRecord> {
        &self.bids
    }

    pub const fn contracts(&self) -> &BTreeMap<String, TaskContractRecord> {
        &self.contracts
    }

    pub const fn obligations(&self) -> &BTreeMap<String, ObligationRecord> {
        &self.obligations
    }

    pub const fn attempts(&self) -> &BTreeMap<String, AttemptRecord> {
        &self.attempts
    }

    pub const fn invocations(&self) -> &BTreeMap<String, InvocationRecord> {
        &self.invocations
    }

    pub fn verifications(&self) -> &[VerificationRecord] {
        &self.verifications
    }

    pub const fn objects(&self) -> &BTreeSet<String> {
        &self.objects
    }

    pub const fn bundles(&self) -> &BTreeMap<String, BundleRecord> {
        &self.bundles
    }

    /// Every result the run has formed, keyed by its identity.
    ///
    /// There is deliberately no accessor beside this one that answers which of them is best,
    /// current, or preferred. Which result a task contract recorded is a fact about that contract;
    /// which result a run should stand on is a question this kernel does not answer.
    pub const fn candidates(&self) -> &BTreeMap<String, CandidateRecord> {
        &self.candidates
    }

    pub const fn conflicts(&self) -> &BTreeMap<String, ConflictRecord> {
        &self.conflicts
    }

    pub const fn stopped(&self) -> Option<StopReason> {
        self.stopped
    }

    pub fn root_obligation(&self) -> &str {
        &self.root_obligation
    }

    /// Every fact committed so far, in the order it was committed.
    pub fn facts(&self) -> &[CommitmentEvent] {
        &self.facts
    }

    /// The run sequence of the last committed fact, which is where a reader that has seen
    /// everything stands.
    pub fn sequence(&self) -> u64 {
        self.facts.len() as u64
    }

    /// The facts a reader holding this cursor has not seen. Recovery is this and nothing else: a
    /// reader that missed every notification asks the same question and gets the same answer as one
    /// that received all of them.
    pub fn facts_after(&self, cursor: u64) -> &[CommitmentEvent] {
        let from = usize::try_from(cursor)
            .unwrap_or(usize::MAX)
            .min(self.facts.len());
        &self.facts[from..]
    }

    /// The first committed fact after an invocation's cursor that one of its registered conditions
    /// names, by run sequence.
    ///
    /// Coalescing lives here: several matching facts are one answer, because what is asked is
    /// whether there is anything worth resuming for and not how many times there was.
    pub fn matching_fact(&self, invocation_id: &str) -> Option<u64> {
        let invocation = self.invocations.get(invocation_id)?;
        let wake = invocation.wake.as_ref()?;
        let from = usize::try_from(invocation.cursor)
            .unwrap_or(usize::MAX)
            .min(self.facts.len());
        self.facts[from..]
            .iter()
            .position(|fact| {
                wake.conditions
                    .iter()
                    .any(|condition| condition.matches(fact))
            })
            .map(|offset| (from + offset + 1) as u64)
    }

    /// The yielded slices a controller may admit now, in the order it must admit them.
    ///
    /// The order is mechanical and is the whole of the fairness rule: principals take turns, and a
    /// principal's own slices are taken in the order the facts recorded their yields. Nothing here
    /// reads what a participant is for, how long it has waited in wall time, how much it holds, or
    /// what the fact that woke it was about — one principal cannot buy a turn by yielding more
    /// often, because extra slices of its own only lengthen its own queue.
    pub fn admission_order(&self) -> Vec<&InvocationRecord> {
        let mut queues: BTreeMap<&str, Vec<&InvocationRecord>> = BTreeMap::new();
        for invocation in self.invocations.values() {
            if !self.wake_is_admissible(invocation) {
                continue;
            }
            let Some(participant) = self.participants.get(&invocation.participant) else {
                continue;
            };
            queues
                .entry(participant.principal_id.as_str())
                .or_default()
                .push(invocation);
        }
        let mut principals: Vec<(&str, Vec<&InvocationRecord>)> = queues.into_iter().collect();
        for (_, queue) in &mut principals {
            queue.sort_by_key(|invocation| invocation.yielded_at);
        }
        // Whose turn comes first is decided by durable record order too: the principal whose
        // earliest waiting slice was recorded first.
        principals.sort_by_key(|(principal, queue)| {
            (
                queue.first().map_or(u64::MAX, |first| first.yielded_at),
                *principal,
            )
        });
        let rounds = principals
            .iter()
            .map(|(_, queue)| queue.len())
            .max()
            .unwrap_or(0);
        let mut order = Vec::new();
        for round in 0..rounds {
            for (_, queue) in &principals {
                if let Some(invocation) = queue.get(round) {
                    order.push(*invocation);
                }
            }
        }
        order
    }

    /// The first funded control object that can still advance the run, or `None` when the run is
    /// quiescent.
    ///
    /// Only funded control objects appear here. A wake that nothing pays for, one whose deadline has
    /// passed, and one held by a slice whose lease is gone or whose contract has changed hands are
    /// all absent, which is what stops a participant nobody will ever resume from keeping a run
    /// open. Nothing that is merely unread, unanswered or unrefreshed is a control object at all.
    pub fn open_authority(&self) -> Option<OpenAuthority> {
        if let Some(invocation) = self
            .invocations
            .values()
            .find(|invocation| invocation.state == InvocationState::Running)
        {
            return Some(OpenAuthority::Invocation {
                invocation_id: invocation.invocation_id.clone(),
            });
        }
        if let Some(invocation) = self
            .invocations
            .values()
            .find(|invocation| self.wake_is_live(invocation))
        {
            return Some(OpenAuthority::FundedWake {
                invocation_id: invocation.invocation_id.clone(),
            });
        }
        if let Some(obligation) = self.obligations.values().find(|obligation| {
            obligation.parent.is_some() && obligation.state != ObligationState::Terminal
        }) {
            return Some(OpenAuthority::Obligation {
                obligation_id: obligation.obligation_id.clone(),
            });
        }
        if let Some(offer) = self
            .offers
            .values()
            .find(|offer| offer.state != OfferState::Settled)
        {
            return Some(OpenAuthority::Offer {
                offer_id: offer.offer_id.clone(),
            });
        }
        None
    }

    /// How the run ended, or `None` while a funded control object can still advance it.
    ///
    /// Quiescence is not one of the answers. Running out of funded work is `Exhausted`, and the
    /// only route to `Accepted` is a protected query that passed against the exact candidate of a
    /// task contract hanging directly under the root obligation. A run that went quiet, spent its
    /// last unit, or was handed a result nobody verified reaches a non-success state and says so.
    pub fn root_terminal(&self) -> Option<RootTerminal> {
        self.open_authority().is_none().then(|| {
            if self.stopped == Some(StopReason::InfrastructureError)
                || self.root_scope_verdict(Verdict::InfrastructureError)
                || self.root_scope_outcome(|outcome| *outcome == Outcome::InfrastructureError)
            {
                return RootTerminal::InfrastructureError;
            }
            if self.stopped == Some(StopReason::Cancelled) {
                return RootTerminal::Cancelled;
            }
            if self.root_scope_verdict(Verdict::Passed) {
                return RootTerminal::Accepted;
            }
            // An allowed stopping policy: every piece of root-scope work that returned said it had
            // nothing further to offer, rather than offering something nothing verified.
            let returns = self.root_scope_returns();
            if !returns.is_empty()
                && returns
                    .iter()
                    .all(|outcome| matches!(outcome, Outcome::Declined | Outcome::DeadEnd))
            {
                return RootTerminal::Abstained;
            }
            RootTerminal::Exhausted
        })
    }

    /// Whether a yielded slice still holds a wake somebody paid for and time to use it in.
    fn wake_is_live(&self, invocation: &InvocationRecord) -> bool {
        if invocation.state != InvocationState::Yielded {
            return false;
        }
        // A resumption is a process slice being begun, and a stopped run begins nothing: whatever
        // the wake was registered for, no command can honour it any more. A run that kept counting
        // such a wake would report itself held open by the one thing it has already refused, so a
        // stop releases every yielded slice at once instead of waiting for each to be closed.
        if self.enforces(Check::StoppedWake) && self.stopped.is_some() {
            return false;
        }
        let Some(wake) = &invocation.wake else {
            return false;
        };
        if wake.conditions.is_empty() || self.now > wake.wake_deadline {
            return false;
        }
        if self.attempt_wakes(&invocation.attempt_id) >= MAX_ATTEMPT_WAKES {
            return false;
        }
        let Some(contract) = self.contracts.get(&invocation.contract_id) else {
            return false;
        };
        contract.state == ContractState::Active
            && contract.lease.generation == invocation.generation
            && self.now <= contract.lease.expires_at
            && contract
                .escrow
                .covers(&BudgetVector::unit(Dimension::InvocationStarts))
    }

    /// Whether a yielded slice is not merely alive but has something committed to resume for.
    fn wake_is_admissible(&self, invocation: &InvocationRecord) -> bool {
        self.wake_is_live(invocation) && self.matching_fact(&invocation.invocation_id).is_some()
    }

    /// How many times the slices of one attempt have been resumed.
    fn attempt_wakes(&self, attempt_id: &str) -> u32 {
        self.invocations
            .values()
            .filter(|invocation| invocation.attempt_id == attempt_id)
            .map(|invocation| invocation.wakes_used)
            .sum()
    }

    /// Whether a task contract hangs directly under the obligation the whole run is accountable
    /// for. A pass there can end the run; a pass one level down is evidence for one parent.
    fn is_root_scope(&self, contract_id: &str) -> bool {
        self.contracts
            .get(contract_id)
            .and_then(|contract| self.obligations.get(&contract.obligation_id))
            .and_then(|obligation| obligation.parent.as_deref())
            .is_some_and(|parent| parent == self.root_obligation)
    }

    fn root_scope_verdict(&self, verdict: Verdict) -> bool {
        self.verifications
            .iter()
            .any(|record| record.root_scope && record.verdict == verdict)
    }

    fn root_scope_returns(&self) -> Vec<&Outcome> {
        self.obligations
            .values()
            .filter(|obligation| {
                obligation.parent.as_deref() == Some(self.root_obligation.as_str())
            })
            .filter_map(|obligation| obligation.outcome.as_ref())
            .collect()
    }

    fn root_scope_outcome(&self, predicate: impl Fn(&Outcome) -> bool) -> bool {
        self.root_scope_returns().into_iter().any(predicate)
    }

    #[cfg(test)]
    pub(crate) fn disable_checks(&mut self, disabled: DisabledChecks) {
        self.disabled = disabled;
    }

    /// What an exhaustive traversal compares two states by: everything the ledger holds except the
    /// history that produced it.
    ///
    /// The fields are taken apart by name rather than read back through the accessors, so a field
    /// added to the ledger later cannot quietly drop out of the comparison. Two are deliberately
    /// left out. The list of committed facts grows with every command, so keying on it would make
    /// every path a state of its own and no traversal would ever converge; everything a transition
    /// reads out of that list — the clock, the balances, the candidate a contract recorded — is in
    /// the registry beside it. And the sequence a yield was recorded at only orders the admission
    /// queue, which no transition reads at all.
    #[cfg(test)]
    pub(crate) fn registry_snapshot(&self) -> String {
        let Self {
            now,
            initial_total,
            consumed,
            root_obligation,
            participants,
            offers,
            bids,
            contracts,
            obligations,
            attempts,
            invocations,
            verifications,
            objects,
            bundles,
            candidates,
            conflicts,
            stopped,
            facts: _,
            disabled: _,
            altered: _,
        } = self;
        let invocations: Vec<InvocationRecord> = invocations
            .values()
            .map(|invocation| InvocationRecord {
                yielded_at: 0,
                ..invocation.clone()
            })
            .collect();
        serde_json::to_string(&(
            now,
            initial_total,
            consumed,
            root_obligation,
            participants,
            offers,
            bids,
            contracts,
            obligations,
            attempts,
            invocations,
            verifications,
            (objects, bundles, candidates, conflicts),
            stopped,
        ))
        .expect("a registry is representable as committed data")
    }

    #[cfg(test)]
    fn enforces(&self, check: Check) -> bool {
        match check {
            Check::Reservation => !self.disabled.reservation,
            Check::Fencing => !self.disabled.fencing,
            Check::ChildReturn => !self.disabled.child_return,
            Check::LiveAccount => !self.disabled.live_account,
            Check::ReturningReservation => !self.disabled.returning_reservation,
            Check::Covering => !self.disabled.covering,
            Check::ReturnHolder => !self.disabled.return_holder,
            Check::InvocationStart => !self.disabled.invocation_start,
            Check::WakeMatch => !self.disabled.wake_match,
            Check::WakeCount => !self.disabled.wake_count,
            Check::StoppedWake => !self.disabled.stopped_wake,
            Check::SingleRunningSlice => !self.disabled.single_running_slice,
            Check::CandidateSeal => !self.disabled.candidate_seal,
            Check::CandidateBase => !self.disabled.candidate_base,
            Check::ConflictResolution => !self.disabled.conflict_resolution,
        }
    }

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    const fn enforces(&self, _check: Check) -> bool {
        true
    }

    #[cfg(test)]
    pub(crate) fn alter_facts(&mut self, altered: AlteredFacts) {
        self.altered = altered;
    }

    /// The amount an escrow transfer states. It is the amount that was decided, unless a test build
    /// asked for it to be understated.
    #[cfg(test)]
    fn stated(&self, amount: BudgetVector) -> BudgetVector {
        if self.altered.understated_escrow {
            amount
                .checked_sub(&BudgetVector::unit(Dimension::MoneyMicros))
                .unwrap_or(amount)
        } else {
            amount
        }
    }

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    const fn stated(&self, amount: BudgetVector) -> BudgetVector {
        amount
    }

    /// The account an escrow transfer states it came out of. It is the account that was decided,
    /// unless a test build asked for the money to be taken from somewhere else.
    #[cfg(test)]
    fn debited(&self, decided: AccountRef, sponsor: &str) -> AccountRef {
        if self.altered.redirected_escrow {
            AccountRef::Participant {
                participant_id: sponsor.to_owned(),
            }
        } else {
            decided
        }
    }

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    fn debited(&self, decided: AccountRef, _sponsor: &str) -> AccountRef {
        decided
    }

    /// The account a closing contract's remaining escrow states it went to. It is the account that
    /// was decided, unless a test build asked for it to be misdirected.
    #[cfg(test)]
    fn returned_to(&self, decided: AccountRef, holder: &str) -> AccountRef {
        if self.altered.misdirected_settlement {
            AccountRef::Participant {
                participant_id: holder.to_owned(),
            }
        } else {
            decided
        }
    }

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    fn returned_to(&self, decided: AccountRef, _holder: &str) -> AccountRef {
        decided
    }

    /// The scope a verification fact states its task contract hangs at. It is the scope that was
    /// decided, unless a test build asked for a contract below the root to claim the root.
    #[cfg(test)]
    const fn scoped(&self, decided: bool) -> bool {
        self.altered.overstated_verification_scope || decided
    }

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    const fn scoped(&self, decided: bool) -> bool {
        decided
    }

    /// The change set a formed result states. It is the set that was decided, unless a test build
    /// asked the fact to state none of it while keeping the identifiers it was given.
    #[cfg(test)]
    fn stated_changes(&self, decided: Vec<BundleChange>) -> Vec<BundleChange> {
        if self.altered.unstated_changes {
            Vec::new()
        } else {
            decided
        }
    }

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    fn stated_changes(&self, decided: Vec<BundleChange>) -> Vec<BundleChange> {
        decided
    }

    /// Decide a command without changing anything, then commit every fact it produced.
    ///
    /// The facts are committed to a copy that replaces the ledger only once every one of them has
    /// been honoured. A debit the accounts cannot pay for therefore refuses the whole command and
    /// says which account and which dimension refused it, instead of being dropped while the facts
    /// still claim the capacity moved. Deciding already establishes that the capacity exists, so
    /// reaching that refusal means a check that was supposed to establish it is missing.
    pub fn execute(
        &mut self,
        command: &CommitmentCommand,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        let events = self.decide(command)?;
        let mut committed = self.clone();
        for event in &events {
            committed.apply(event)?;
        }
        *self = committed;
        Ok(events)
    }

    /// Commit one fact that was decided and recorded earlier, which is how a ledger is rebuilt
    /// from a durable record.
    ///
    /// This is the recovery path and the only way a fact enters a ledger without being decided
    /// here first. What it takes must therefore come from a record the kernel itself committed:
    /// nothing in this method re-checks the authority, the deadlines or the funding that
    /// [`Self::decide`] established, because a committed fact is not a request. A fact the
    /// accounts cannot honour is reported rather than dropped, so a record that does not rebuild
    /// refuses the whole recovery instead of producing a ledger that quietly differs from it.
    pub fn replay(&mut self, fact: &CommitmentEvent) -> Result<(), CommitmentError> {
        self.apply(fact)
    }

    /// The facts a command would commit, or the reason it may not.
    pub fn decide(
        &self,
        command: &CommitmentCommand,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        match command {
            CommitmentCommand::RegisterParticipant(command) => self.decide_register(command),
            CommitmentCommand::Advertise(command) => self.decide_advertise(command),
            CommitmentCommand::RecordBid(command) => self.decide_bid(command),
            CommitmentCommand::WithdrawBid(command) => self.decide_withdraw_bid(command),
            CommitmentCommand::WithdrawOffer(command) => self.decide_withdraw_offer(command),
            CommitmentCommand::SettleOffer(command) => self.decide_settle_offer(command),
            CommitmentCommand::Award(command) => self.decide_award(command),
            CommitmentCommand::AcceptOpen(command) => self.decide_accept_open(command),
            CommitmentCommand::StartAttempt(command) => self.decide_start_attempt(command),
            CommitmentCommand::RenewLease(command) => self.decide_renew(command),
            CommitmentCommand::SubmitResult(command) => self.decide_submit(command),
            CommitmentCommand::RecordObject(command) => self.decide_record_object(command),
            CommitmentCommand::SubmitBundle(command) => self.decide_submit_bundle(command),
            CommitmentCommand::RecordConflict(command) => self.decide_record_conflict(command),
            CommitmentCommand::Reassign(command) => self.decide_reassign(command),
            CommitmentCommand::ReturnObligation(command) => self.decide_return(command),
            CommitmentCommand::CancelContract(command) => self.decide_cancel(command),
            CommitmentCommand::AdvanceClock(command) => self.decide_advance_clock(command),
            CommitmentCommand::StartInvocation(command) => self.decide_start_invocation(command),
            CommitmentCommand::YieldInvocation(command) => self.decide_yield(command),
            CommitmentCommand::ResumeInvocation(command) => self.decide_resume(command),
            CommitmentCommand::CloseInvocation(command) => self.decide_close_invocation(command),
            CommitmentCommand::RecordVerification(command) => self.decide_verification(command),
            CommitmentCommand::StopRun(command) => self.decide_stop_run(command),
        }
    }

    /// Whether the run still creates anything. A stopped run winds down: what exists is returned,
    /// settled and closed, and nothing new is begun, extended or paid for.
    fn ensure_run_creates(&self) -> Result<(), CommitmentError> {
        if self.stopped.is_some() {
            return Err(CommitmentError::RunStopped);
        }
        Ok(())
    }

    fn decide_register(
        &self,
        command: &RegisterParticipant,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        self.ensure_run_creates()?;
        validate_identifier("participant_id", &command.participant_id)?;
        validate_identifier("principal_id", &command.principal_id)?;
        let sponsor = self.participant(&command.sponsor)?;
        if self.participants.contains_key(&command.participant_id) {
            return Err(CommitmentError::DuplicateIdentifier {
                kind: "participant",
                id: command.participant_id.clone(),
            });
        }
        let authority = BudgetVector::unit(Dimension::ParticipantStarts);
        let total = add(&command.endowment, &authority)?;
        let account = AccountRef::Participant {
            participant_id: sponsor.participant_id.clone(),
        };
        self.ensure_covers(&account, &total)?;
        Ok(vec![
            CommitmentEvent::ParticipantRegistered {
                participant_id: command.participant_id.clone(),
                principal_id: command.principal_id.clone(),
            },
            CommitmentEvent::BudgetConsumed {
                account: account.clone(),
                amount: authority,
            },
            CommitmentEvent::BudgetTransferred {
                from: account,
                to: AccountRef::Participant {
                    participant_id: command.participant_id.clone(),
                },
                amount: command.endowment,
            },
        ])
    }

    fn decide_advertise(
        &self,
        command: &Advertise,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        self.ensure_run_creates()?;
        validate_identifier("offer_id", &command.offer_id)?;
        validate_identifier("task_scope", &command.task_scope)?;
        validate_identifier("artifact_class", &command.artifact_class)?;
        validate_digest("base_digest", &command.base_digest)?;
        validate_digest("intent_digest", &command.intent_digest)?;
        if command.dependencies.len() > MAX_SCOPE_ENTRIES {
            return Err(CommitmentError::TooManyEntries {
                kind: "dependencies",
            });
        }
        if command.capability_scope.len() > MAX_SCOPE_ENTRIES {
            return Err(CommitmentError::TooManyEntries {
                kind: "capability_scope",
            });
        }
        for entry in command.dependencies.iter().chain(&command.capability_scope) {
            validate_identifier("scope entry", entry)?;
        }
        if !(1..=MAX_AWARDS).contains(&command.max_awards) {
            return Err(CommitmentError::InvalidAwardCount);
        }
        if command.bid_deadline > command.offer_deadline || command.offer_deadline < self.now {
            return Err(CommitmentError::InvalidDeadlines);
        }
        if let OfferPolicy::Targeted { participant_id } = &command.policy {
            self.participant(participant_id)?;
        }
        self.participant(&command.sponsor)?;
        if self.offers.contains_key(&command.offer_id) {
            return Err(CommitmentError::DuplicateIdentifier {
                kind: "offer",
                id: command.offer_id.clone(),
            });
        }
        let parent = self.obligation(&command.parent_obligation)?;
        if parent.owner != command.sponsor {
            return Err(CommitmentError::NotAuthorized {
                principal: command.sponsor.clone(),
            });
        }
        if parent.state != ObligationState::Active {
            return Err(CommitmentError::ObligationTerminal {
                obligation_id: parent.obligation_id.clone(),
            });
        }
        let funding = self.funding_account(&command.funding_source, &command.sponsor)?;
        let pool = command
            .execution_escrow
            .checked_scale(u64::from(command.max_awards))
            .map_err(|dimension| CommitmentError::BudgetOverflow { dimension })?;
        let authority = BudgetVector::unit(Dimension::OfferCreations);
        self.ensure_covers(&funding, &add(&pool, &authority)?)?;
        Ok(vec![
            CommitmentEvent::OfferAdvertised {
                offer_id: command.offer_id.clone(),
                sponsor: command.sponsor.clone(),
                parent_obligation: command.parent_obligation.clone(),
                funding_source: command.funding_source.clone(),
                task_scope: command.task_scope.clone(),
                base_digest: command.base_digest.clone(),
                intent_digest: command.intent_digest.clone(),
                artifact_class: command.artifact_class.clone(),
                dependencies: command.dependencies.clone(),
                capability_scope: command.capability_scope.clone(),
                execution_escrow: command.execution_escrow,
                policy: command.policy.clone(),
                bid_deadline: command.bid_deadline,
                offer_deadline: command.offer_deadline,
                max_awards: command.max_awards,
            },
            CommitmentEvent::BudgetConsumed {
                account: funding.clone(),
                amount: authority,
            },
            CommitmentEvent::BudgetTransferred {
                from: funding,
                to: AccountRef::Offer {
                    offer_id: command.offer_id.clone(),
                },
                amount: pool,
            },
        ])
    }

    fn decide_bid(&self, command: &RecordBid) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        self.ensure_run_creates()?;
        validate_identifier("bid_id", &command.bid_id)?;
        validate_identifier("artifact_class", &command.artifact_class)?;
        if let Some(digest) = &command.proposal_digest {
            validate_digest("proposal_digest", digest)?;
        }
        self.participant(&command.bidder)?;
        if self.bids.contains_key(&command.bid_id) {
            return Err(CommitmentError::DuplicateIdentifier {
                kind: "bid",
                id: command.bid_id.clone(),
            });
        }
        let offer = self.offer(&command.offer_id)?;
        if offer.state != OfferState::Advertised {
            return Err(CommitmentError::OfferNotOpen {
                offer_id: offer.offer_id.clone(),
            });
        }
        if self.now > offer.bid_deadline {
            return Err(CommitmentError::OfferExpired {
                offer_id: offer.offer_id.clone(),
                deadline: offer.bid_deadline,
                now: self.now,
            });
        }
        if let OfferPolicy::Targeted { participant_id } = &offer.policy
            && participant_id != &command.bidder
        {
            return Err(CommitmentError::TargetMismatch {
                offer_id: offer.offer_id.clone(),
                participant_id: participant_id.clone(),
            });
        }
        Ok(vec![CommitmentEvent::BidRecorded {
            bid_id: command.bid_id.clone(),
            offer_id: command.offer_id.clone(),
            bidder: command.bidder.clone(),
            requested_escrow: command.requested_escrow,
            artifact_class: command.artifact_class.clone(),
            proposal_digest: command.proposal_digest.clone(),
            expires_at: command.expires_at,
            origin: BidOrigin::Bid,
        }])
    }

    fn decide_withdraw_bid(
        &self,
        command: &WithdrawBid,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        let bid = self.bid(&command.bid_id)?;
        if bid.bidder != command.bidder {
            return Err(CommitmentError::NotAuthorized {
                principal: command.bidder.clone(),
            });
        }
        if bid.state != BidState::Live {
            return Err(CommitmentError::BidNotLive {
                bid_id: bid.bid_id.clone(),
            });
        }
        Ok(vec![CommitmentEvent::BidWithdrawn {
            bid_id: command.bid_id.clone(),
        }])
    }

    fn decide_withdraw_offer(
        &self,
        command: &WithdrawOffer,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        let offer = self.offer(&command.offer_id)?;
        if offer.sponsor != command.sponsor {
            return Err(CommitmentError::NotAuthorized {
                principal: command.sponsor.clone(),
            });
        }
        if offer.state != OfferState::Advertised {
            return Err(CommitmentError::OfferNotOpen {
                offer_id: offer.offer_id.clone(),
            });
        }
        Ok(vec![CommitmentEvent::OfferWithdrawn {
            offer_id: command.offer_id.clone(),
        }])
    }

    fn decide_settle_offer(
        &self,
        command: &SettleOffer,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        let offer = self.offer(&command.offer_id)?;
        if offer.sponsor != command.sponsor {
            return Err(CommitmentError::NotAuthorized {
                principal: command.sponsor.clone(),
            });
        }
        if offer.state == OfferState::Settled {
            return Err(CommitmentError::OfferNotOpen {
                offer_id: offer.offer_id.clone(),
            });
        }
        if offer.state == OfferState::Advertised && self.now <= offer.offer_deadline {
            return Err(CommitmentError::OfferStillOpen {
                offer_id: offer.offer_id.clone(),
                deadline: offer.offer_deadline,
            });
        }
        let destination = self.settlement_destination(&offer.funding_source, &offer.sponsor)?;
        let mut events = vec![CommitmentEvent::OfferSettled {
            offer_id: command.offer_id.clone(),
        }];
        if !offer.escrow.is_zero() {
            events.push(CommitmentEvent::BudgetTransferred {
                from: AccountRef::Offer {
                    offer_id: offer.offer_id.clone(),
                },
                to: destination,
                amount: offer.escrow,
            });
        }
        Ok(events)
    }

    fn decide_award(&self, command: &Award) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        let offer = self.offer(&command.offer_id)?;
        if offer.sponsor != command.sponsor {
            return Err(CommitmentError::NotAuthorized {
                principal: command.sponsor.clone(),
            });
        }
        if !matches!(
            offer.policy,
            OfferPolicy::Negotiated | OfferPolicy::Targeted { .. }
        ) {
            return Err(CommitmentError::PolicyMismatch {
                offer_id: offer.offer_id.clone(),
            });
        }
        let bid = self.bid(&command.bid_id)?;
        if bid.offer_id != offer.offer_id {
            return Err(CommitmentError::BidOfferMismatch {
                bid_id: bid.bid_id.clone(),
                named: bid.offer_id.clone(),
                offer_id: offer.offer_id.clone(),
            });
        }
        if let OfferPolicy::Targeted { participant_id } = &offer.policy
            && participant_id != &bid.bidder
        {
            return Err(CommitmentError::TargetMismatch {
                offer_id: offer.offer_id.clone(),
                participant_id: participant_id.clone(),
            });
        }
        self.form_contract(
            offer,
            bid.clone(),
            None,
            &Formation {
                contract_id: &command.contract_id,
                obligation_id: &command.obligation_id,
                lease_id: &command.lease_id,
                lease_ms: command.lease_ms,
            },
        )
    }

    fn decide_accept_open(
        &self,
        command: &AcceptOpen,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        validate_identifier("bid_id", &command.bid_id)?;
        validate_identifier("artifact_class", &command.artifact_class)?;
        if let Some(digest) = &command.proposal_digest {
            validate_digest("proposal_digest", digest)?;
        }
        self.participant(&command.participant)?;
        if self.bids.contains_key(&command.bid_id) {
            return Err(CommitmentError::DuplicateIdentifier {
                kind: "bid",
                id: command.bid_id.clone(),
            });
        }
        let offer = self.offer(&command.offer_id)?;
        if offer.policy != OfferPolicy::OpenAccept {
            return Err(CommitmentError::PolicyMismatch {
                offer_id: offer.offer_id.clone(),
            });
        }
        if self.now > offer.bid_deadline {
            return Err(CommitmentError::OfferExpired {
                offer_id: offer.offer_id.clone(),
                deadline: offer.bid_deadline,
                now: self.now,
            });
        }
        let consent = BidRecord {
            bid_id: command.bid_id.clone(),
            offer_id: command.offer_id.clone(),
            bidder: command.participant.clone(),
            requested_escrow: command.requested_escrow,
            artifact_class: command.artifact_class.clone(),
            proposal_digest: command.proposal_digest.clone(),
            expires_at: offer.bid_deadline,
            origin: BidOrigin::OpenAccept,
            state: BidState::Live,
        };
        let recorded = CommitmentEvent::BidRecorded {
            bid_id: consent.bid_id.clone(),
            offer_id: consent.offer_id.clone(),
            bidder: consent.bidder.clone(),
            requested_escrow: consent.requested_escrow,
            artifact_class: consent.artifact_class.clone(),
            proposal_digest: consent.proposal_digest.clone(),
            expires_at: consent.expires_at,
            origin: BidOrigin::OpenAccept,
        };
        self.form_contract(
            offer,
            consent,
            Some(recorded),
            &Formation {
                contract_id: &command.contract_id,
                obligation_id: &command.obligation_id,
                lease_id: &command.lease_id,
                lease_ms: command.lease_ms,
            },
        )
    }

    /// The one path that forms a task contract, whichever consent route reached it. Everything it
    /// checks is mechanical: liveness, class equality, funding, the funded award count, creation
    /// authority and a first lease that the transferred escrow actually pays for.
    fn form_contract(
        &self,
        offer: &OfferRecord,
        bid: BidRecord,
        consent_event: Option<CommitmentEvent>,
        formation: &Formation<'_>,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        let Formation {
            contract_id,
            obligation_id,
            lease_id,
            lease_ms,
        } = *formation;
        self.ensure_run_creates()?;
        validate_identifier("contract_id", contract_id)?;
        validate_identifier("obligation_id", obligation_id)?;
        validate_identifier("lease_id", lease_id)?;
        if !(1..=MAX_LEASE_MS).contains(&lease_ms) {
            return Err(CommitmentError::InvalidLeaseDuration);
        }
        if self.contracts.contains_key(contract_id) {
            return Err(CommitmentError::DuplicateIdentifier {
                kind: "task contract",
                id: contract_id.to_owned(),
            });
        }
        if self.obligations.contains_key(obligation_id) {
            return Err(CommitmentError::DuplicateIdentifier {
                kind: "obligation",
                id: obligation_id.to_owned(),
            });
        }
        if self
            .contracts
            .values()
            .any(|contract| contract.lease.lease_id == lease_id)
        {
            return Err(CommitmentError::DuplicateIdentifier {
                kind: "lease",
                id: lease_id.to_owned(),
            });
        }
        if offer.state != OfferState::Advertised {
            return Err(CommitmentError::OfferNotOpen {
                offer_id: offer.offer_id.clone(),
            });
        }
        if self.now > offer.offer_deadline {
            return Err(CommitmentError::OfferExpired {
                offer_id: offer.offer_id.clone(),
                deadline: offer.offer_deadline,
                now: self.now,
            });
        }
        if bid.state != BidState::Live {
            return Err(CommitmentError::BidNotLive {
                bid_id: bid.bid_id.clone(),
            });
        }
        if self.now > bid.expires_at {
            return Err(CommitmentError::BidExpired {
                bid_id: bid.bid_id.clone(),
                deadline: bid.expires_at,
                now: self.now,
            });
        }
        if bid.artifact_class != offer.artifact_class {
            return Err(CommitmentError::ArtifactClassMismatch {
                bid_id: bid.bid_id.clone(),
                offer_id: offer.offer_id.clone(),
            });
        }
        if let Some(dimension) = offer.execution_escrow.shortfall(&bid.requested_escrow) {
            return Err(CommitmentError::EscrowExceedsOffer {
                offer_id: offer.offer_id.clone(),
                bid_id: bid.bid_id.clone(),
                dimension,
            });
        }
        if bid.requested_escrow.get(Dimension::WallTimeMs) < lease_ms {
            return Err(CommitmentError::LeaseNotFunded {
                bid_id: bid.bid_id.clone(),
                lease_ms,
            });
        }
        if self.enforces(Check::Reservation) && offer.awards_made >= offer.max_awards {
            return Err(CommitmentError::AwardsExhausted {
                offer_id: offer.offer_id.clone(),
                max_awards: offer.max_awards,
            });
        }
        let parent = self.obligation(&offer.parent_obligation)?;
        if parent.state != ObligationState::Active {
            return Err(CommitmentError::ObligationTerminal {
                obligation_id: parent.obligation_id.clone(),
            });
        }
        let contractor = self.participant(&bid.bidder)?;
        let offer_account = AccountRef::Offer {
            offer_id: offer.offer_id.clone(),
        };
        self.ensure_covers(&offer_account, &bid.requested_escrow)?;
        let funding = self.funding_account(&offer.funding_source, &offer.sponsor)?;
        let authority = BudgetVector::unit(Dimension::ObligationCreations);
        self.ensure_covers(&funding, &authority)?;

        let contract_account = AccountRef::TaskContract {
            contract_id: contract_id.to_owned(),
        };
        let mut events = Vec::new();
        events.extend(consent_event);
        events.push(CommitmentEvent::BudgetConsumed {
            account: funding,
            amount: authority,
        });
        events.push(CommitmentEvent::TaskContractFormed {
            contract_id: contract_id.to_owned(),
            offer_id: offer.offer_id.clone(),
            bid_id: bid.bid_id.clone(),
            sponsor: offer.sponsor.clone(),
            contractor: contractor.participant_id.clone(),
            obligation_id: obligation_id.to_owned(),
            task_scope: offer.task_scope.clone(),
            base_digest: offer.base_digest.clone(),
        });
        events.push(CommitmentEvent::BudgetTransferred {
            from: self.debited(offer_account, &offer.sponsor),
            to: contract_account.clone(),
            amount: self.stated(bid.requested_escrow),
        });
        events.push(CommitmentEvent::ObligationCreated {
            obligation_id: obligation_id.to_owned(),
            parent: offer.parent_obligation.clone(),
            owner: contractor.participant_id.clone(),
            contract_id: contract_id.to_owned(),
        });
        events.push(CommitmentEvent::LeaseIssued {
            contract_id: contract_id.to_owned(),
            lease_id: lease_id.to_owned(),
            holder: contractor.participant_id.clone(),
            generation: 1,
            expires_at: self.now.saturating_add(lease_ms),
        });
        events.push(CommitmentEvent::BudgetConsumed {
            account: contract_account,
            amount: BudgetVector::units(Dimension::WallTimeMs, lease_ms),
        });
        Ok(events)
    }

    fn decide_start_attempt(
        &self,
        command: &StartAttempt,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        self.ensure_run_creates()?;
        validate_identifier("attempt_id", &command.attempt_id)?;
        if self.attempts.contains_key(&command.attempt_id) {
            return Err(CommitmentError::DuplicateIdentifier {
                kind: "attempt",
                id: command.attempt_id.clone(),
            });
        }
        let contract = self.active_contract(&command.contract_id)?;
        self.ensure_holder(contract, &command.participant)?;
        self.ensure_generation(contract, command.generation)?;
        self.ensure_lease_live(contract)?;
        let account = AccountRef::TaskContract {
            contract_id: contract.contract_id.clone(),
        };
        let authority = BudgetVector::unit(Dimension::AttemptStarts);
        self.ensure_covers(&account, &authority)?;
        Ok(vec![
            // The generation the caller presented, not the one the ledger holds: a recorded fact
            // states what was actually claimed, so a token that should have been refused stays
            // visible in the event stream instead of being quietly normalized.
            CommitmentEvent::AttemptStarted {
                attempt_id: command.attempt_id.clone(),
                contract_id: contract.contract_id.clone(),
                participant: command.participant.clone(),
                generation: command.generation,
            },
            CommitmentEvent::BudgetConsumed {
                account,
                amount: authority,
            },
        ])
    }

    fn decide_renew(&self, command: &RenewLease) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        self.ensure_run_creates()?;
        if !(1..=MAX_LEASE_MS).contains(&command.lease_ms) {
            return Err(CommitmentError::InvalidLeaseDuration);
        }
        let contract = self.active_contract(&command.contract_id)?;
        self.ensure_holder(contract, &command.holder)?;
        self.ensure_generation(contract, command.generation)?;
        self.ensure_lease_live(contract)?;
        let account = AccountRef::TaskContract {
            contract_id: contract.contract_id.clone(),
        };
        let charge = BudgetVector::units(Dimension::WallTimeMs, command.lease_ms);
        self.ensure_covers(&account, &charge)?;
        Ok(vec![
            CommitmentEvent::LeaseRenewed {
                contract_id: contract.contract_id.clone(),
                generation: command.generation,
                expires_at: self.now.saturating_add(command.lease_ms),
            },
            CommitmentEvent::BudgetConsumed {
                account,
                amount: charge,
            },
        ])
    }

    /// Record a result whose construction the caller's own object store holds, as the exact digest
    /// of that result and nothing more.
    ///
    /// It seals the task contract exactly as a bundle does. What it cannot do is state where the
    /// result came from, which is why a submission that carries its ancestry goes through
    /// [`Self::decide_submit_bundle`].
    fn decide_submit(
        &self,
        command: &SubmitResult,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        validate_digest("candidate_digest", &command.candidate_digest)?;
        let contract = self.submission_target(
            &command.contract_id,
            &command.participant,
            command.generation,
            &command.attempt_id,
        )?;
        self.ensure_unsealed(contract, &command.candidate_digest)?;
        Ok(vec![CommitmentEvent::SubmissionRecorded {
            contract_id: contract.contract_id.clone(),
            attempt_id: command.attempt_id.clone(),
            generation: command.generation,
            candidate_digest: command.candidate_digest.clone(),
        }])
    }

    /// State that an object is stored whole, so that a bundle may put it at a path.
    ///
    /// Recording the same object twice states the same fact twice and changes nothing, because the
    /// fact is about bytes that already exist and not about a claim on them. Two participants that
    /// produce the same bytes record the same object.
    fn decide_record_object(
        &self,
        command: &RecordObject,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        validate_digest("object_digest", &command.object_digest)?;
        let contract = self.active_contract(&command.contract_id)?;
        self.ensure_holder(contract, &command.participant)?;
        self.ensure_generation(contract, command.generation)?;
        self.ensure_lease_live(contract)?;
        Ok(vec![CommitmentEvent::ObjectRecorded {
            object_digest: command.object_digest.clone(),
        }])
    }

    /// Publish a bundle and form the immutable result it constructs.
    ///
    /// Everything decided here is mechanical: whether the bundle names the base its task contract
    /// was awarded from, whether its fencing token is current, whether every object it names is
    /// stored whole, and whether the results it carries forward disagree anywhere it says nothing
    /// about. The identity of the result is computed from those facts rather than taken from the
    /// command, so a caller cannot claim an identifier for a construction it did not state.
    fn decide_submit_bundle(
        &self,
        command: &SubmitBundle,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        validate_digest("base_digest", &command.base_digest)?;
        if command.changes.len() > MAX_BUNDLE_CHANGES {
            return Err(CommitmentError::InvalidBundleSize);
        }
        if command.parents.len() > MAX_BUNDLE_PARENTS {
            return Err(CommitmentError::TooManyEntries {
                kind: "bundle parents",
            });
        }
        let mut stated = BTreeSet::new();
        for change in &command.changes {
            validate_path(&change.path)?;
            if !stated.insert(change.path.as_str()) {
                return Err(CommitmentError::DuplicateIdentifier {
                    kind: "path",
                    id: change.path.clone(),
                });
            }
            if let PathChange::Upsert { object_digest, .. } = &change.change {
                validate_digest("object_digest", object_digest)?;
            }
        }
        let contract = self.submission_target(
            &command.contract_id,
            &command.participant,
            command.generation,
            &command.attempt_id,
        )?;
        if self.enforces(Check::CandidateBase) && command.base_digest != contract.base_digest {
            return Err(CommitmentError::StaleBase {
                contract_id: contract.contract_id.clone(),
                expected: contract.base_digest.clone(),
                actual: command.base_digest.clone(),
            });
        }
        // A half-written object is one nothing ever recorded, so a result that names it is refused
        // rather than formed over bytes that may never arrive.
        for change in &command.changes {
            if let PathChange::Upsert { object_digest, .. } = &change.change
                && !self.objects.contains(object_digest)
            {
                return Err(CommitmentError::ObjectIncomplete {
                    object_digest: object_digest.clone(),
                });
            }
        }

        let mut parents = command.parents.clone();
        parents.sort();
        parents.dedup();
        let carried = parents
            .iter()
            .map(|digest| self.candidate(digest))
            .collect::<Result<Vec<_>, _>>()?;
        for candidate in &carried {
            if candidate.base_digest != command.base_digest {
                return Err(CommitmentError::DivergentBase {
                    candidate_digest: candidate.candidate_digest.clone(),
                    base_digest: candidate.base_digest.clone(),
                });
            }
        }

        let own = change_map(&command.changes);
        let (agreed, disputed) = merge_contributions(&carried);
        let unresolved: Vec<String> = disputed
            .into_iter()
            .filter(|path| !own.contains_key(path))
            .collect();
        if self.enforces(Check::ConflictResolution) && !unresolved.is_empty() {
            return Err(CommitmentError::IntegrationConflict { paths: unresolved });
        }
        let mut effective = agreed;
        effective.extend(own.clone());
        if effective.len() > MAX_CANDIDATE_CHANGES {
            return Err(CommitmentError::TooManyEntries {
                kind: "candidate changes",
            });
        }

        let changes = ordered_changes(own);
        let effective = ordered_changes(effective);
        let bundle_digest = BundleRecord::identify(&command.base_digest, &parents, &changes);
        let contributions: Vec<Contribution> = carried
            .iter()
            .map(|candidate| Contribution {
                candidate_digest: candidate.candidate_digest.clone(),
                obligation_id: candidate.obligation_id.clone(),
                participant: candidate.participant.clone(),
                bundle_digest: candidate.bundle_digest.clone(),
            })
            .collect();
        let content_digest = CandidateRecord::identify_content(&command.base_digest, &effective);
        let candidate_digest = CandidateRecord::identify(
            &content_digest,
            &contract.contract_id,
            &contract.obligation_id,
            &command.participant,
            command.generation,
            &bundle_digest,
            &contributions,
        );
        self.ensure_unsealed(contract, &candidate_digest)?;

        Ok(vec![
            CommitmentEvent::BundleRecorded {
                bundle_digest: bundle_digest.clone(),
                base_digest: command.base_digest.clone(),
                parents,
                changes,
            },
            CommitmentEvent::CandidateFormed {
                candidate_digest: candidate_digest.clone(),
                content_digest,
                contract_id: contract.contract_id.clone(),
                obligation_id: contract.obligation_id.clone(),
                participant: command.participant.clone(),
                generation: command.generation,
                base_digest: command.base_digest.clone(),
                bundle_digest,
                contributions,
                changes: self.stated_changes(effective),
            },
            CommitmentEvent::SubmissionRecorded {
                contract_id: contract.contract_id.clone(),
                attempt_id: command.attempt_id.clone(),
                generation: command.generation,
                candidate_digest,
            },
        ])
    }

    /// Record where named results put different bytes at the same path.
    ///
    /// The paths are derived from the candidates themselves, so the evidence states what is true of
    /// them rather than what the caller says about them. Nothing follows from recording it: no
    /// candidate is closed, no work is created, and no side is taken. It gives the collective an
    /// identifier a participant may fund an offer against, and that offer is consent like any other.
    fn decide_record_conflict(
        &self,
        command: &RecordConflict,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        self.participant(&command.participant)?;
        let mut named = command.candidates.clone();
        named.sort();
        named.dedup();
        if !(2..=MAX_BUNDLE_PARENTS).contains(&named.len()) {
            return Err(CommitmentError::InvalidConflictScope);
        }
        let candidates = named
            .iter()
            .map(|digest| self.candidate(digest))
            .collect::<Result<Vec<_>, _>>()?;
        let base_digest = candidates[0].base_digest.clone();
        for candidate in &candidates {
            if candidate.base_digest != base_digest {
                return Err(CommitmentError::DivergentBase {
                    candidate_digest: candidate.candidate_digest.clone(),
                    base_digest: candidate.base_digest.clone(),
                });
            }
        }
        let (_, paths) = merge_contributions(&candidates);
        if paths.is_empty() {
            return Err(CommitmentError::NoConflict);
        }
        Ok(vec![CommitmentEvent::ConflictRecorded {
            conflict_digest: ConflictRecord::identify(&base_digest, &named, &paths),
            base_digest,
            candidates: named,
            paths,
        }])
    }

    /// The task contract a result may be recorded against: active, held by this participant, under
    /// the current fencing token, with a live lease and an attempt of its own.
    fn submission_target(
        &self,
        contract_id: &str,
        participant: &str,
        generation: u64,
        attempt_id: &str,
    ) -> Result<&TaskContractRecord, CommitmentError> {
        let contract = self.active_contract(contract_id)?;
        self.ensure_holder(contract, participant)?;
        self.ensure_generation(contract, generation)?;
        self.ensure_lease_live(contract)?;
        let attempt = self.attempt(attempt_id)?;
        if attempt.contract_id != contract.contract_id {
            return Err(CommitmentError::AttemptMismatch {
                attempt_id: attempt.attempt_id.clone(),
                contract_id: contract.contract_id.clone(),
            });
        }
        if self.enforces(Check::Fencing) && attempt.generation != contract.lease.generation {
            return Err(CommitmentError::StaleGeneration {
                contract_id: contract.contract_id.clone(),
                seen: attempt.generation,
                current: contract.lease.generation,
            });
        }
        Ok(contract)
    }

    /// Whether a task contract may record this result.
    ///
    /// A contract records one result. Submitting the same one again states the same fact and is
    /// idempotent; submitting another is refused with both identifiers named, whether it arrives
    /// from a retry, a second attempt or a participant that took the contract over.
    fn ensure_unsealed(
        &self,
        contract: &TaskContractRecord,
        proposed: &str,
    ) -> Result<(), CommitmentError> {
        if let Some(current) = &contract.candidate_digest
            && self.enforces(Check::CandidateSeal)
            && current != proposed
        {
            return Err(CommitmentError::CandidateSealed {
                contract_id: contract.contract_id.clone(),
                current: current.clone(),
                proposed: proposed.to_owned(),
            });
        }
        Ok(())
    }

    /// Issue the next fencing generation for a contract whose lease has run out. The sponsor names
    /// the consent that replaces the old one; the previous generation can no longer advance
    /// anything.
    fn decide_reassign(&self, command: &Reassign) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        self.ensure_run_creates()?;
        validate_identifier("lease_id", &command.lease_id)?;
        if !(1..=MAX_LEASE_MS).contains(&command.lease_ms) {
            return Err(CommitmentError::InvalidLeaseDuration);
        }
        let contract = self.active_contract(&command.contract_id)?;
        if contract.sponsor != command.sponsor {
            return Err(CommitmentError::NotAuthorized {
                principal: command.sponsor.clone(),
            });
        }
        if self.now <= contract.lease.expires_at {
            return Err(CommitmentError::LeaseStillLive {
                contract_id: contract.contract_id.clone(),
                expires_at: contract.lease.expires_at,
            });
        }
        let bid = self.bid(&command.bid_id)?;
        if bid.offer_id != contract.offer_id {
            return Err(CommitmentError::BidOfferMismatch {
                bid_id: bid.bid_id.clone(),
                named: bid.offer_id.clone(),
                offer_id: contract.offer_id.clone(),
            });
        }
        if bid.state != BidState::Live {
            return Err(CommitmentError::BidNotLive {
                bid_id: bid.bid_id.clone(),
            });
        }
        if self.now > bid.expires_at {
            return Err(CommitmentError::BidExpired {
                bid_id: bid.bid_id.clone(),
                deadline: bid.expires_at,
                now: self.now,
            });
        }
        let offer = self.offer(&contract.offer_id)?;
        if bid.artifact_class != offer.artifact_class {
            return Err(CommitmentError::ArtifactClassMismatch {
                bid_id: bid.bid_id.clone(),
                offer_id: offer.offer_id.clone(),
            });
        }
        self.participant(&bid.bidder)?;
        let account = AccountRef::TaskContract {
            contract_id: contract.contract_id.clone(),
        };
        let charge = BudgetVector::units(Dimension::WallTimeMs, command.lease_ms);
        self.ensure_covers(&account, &bid.requested_escrow)?;
        self.ensure_covers(&account, &charge)?;
        Ok(vec![
            CommitmentEvent::ContractReassigned {
                contract_id: contract.contract_id.clone(),
                bid_id: bid.bid_id.clone(),
                previous_holder: contract.lease.holder.clone(),
                holder: bid.bidder.clone(),
                generation: contract.lease.generation.saturating_add(1),
            },
            CommitmentEvent::LeaseIssued {
                contract_id: contract.contract_id.clone(),
                lease_id: command.lease_id.clone(),
                holder: bid.bidder.clone(),
                generation: contract.lease.generation.saturating_add(1),
                expires_at: self.now.saturating_add(command.lease_ms),
            },
            CommitmentEvent::BudgetConsumed {
                account,
                amount: charge,
            },
        ])
    }

    fn decide_return(
        &self,
        command: &ReturnObligation,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        if let Outcome::Result { candidate_digest } = &command.outcome {
            validate_digest("candidate_digest", candidate_digest)?;
        }
        let contract = self.active_contract(&command.contract_id)?;
        // A current fencing token is not by itself a claim on the work: it is the token of whoever
        // holds the lease, and a participant that does not hold it closes nothing here, whether it
        // held the lease before a reassignment or never held it at all.
        if self.enforces(Check::ReturnHolder) {
            self.ensure_holder(contract, &command.participant)?;
        }
        self.ensure_generation(contract, command.generation)?;
        self.ensure_lease_live(contract)?;
        self.ensure_causal_work_closed(&contract.obligation_id)?;
        self.ensure_reservations_returned(contract)?;
        let mut events = vec![CommitmentEvent::ObligationReturned {
            obligation_id: contract.obligation_id.clone(),
            contract_id: contract.contract_id.clone(),
            // Who closed the work is part of the fact. Without it the event stream records that an
            // obligation closed under some generation but not by whom, and no audit built from the
            // facts alone could tell an owner's return from anyone else's.
            participant: command.participant.clone(),
            generation: command.generation,
            outcome: command.outcome.clone(),
        }];
        events.extend(self.settlement(contract)?);
        Ok(events)
    }

    fn decide_cancel(
        &self,
        command: &CancelContract,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        let contract = self.active_contract(&command.contract_id)?;
        if contract.sponsor != command.sponsor {
            return Err(CommitmentError::NotAuthorized {
                principal: command.sponsor.clone(),
            });
        }
        self.ensure_causal_work_closed(&contract.obligation_id)?;
        self.ensure_reservations_returned(contract)?;
        let mut events = vec![CommitmentEvent::ContractCancelled {
            contract_id: contract.contract_id.clone(),
            obligation_id: contract.obligation_id.clone(),
        }];
        events.extend(self.settlement(contract)?);
        Ok(events)
    }

    fn decide_advance_clock(
        &self,
        command: &AdvanceClock,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        if command.to < self.now {
            return Err(CommitmentError::ClockRegression {
                now: self.now,
                to: command.to,
            });
        }
        Ok(vec![CommitmentEvent::ClockAdvanced { to: command.to }])
    }

    /// Begin one supervised process slice. It costs one unit of creation authority whether it is
    /// the first slice of an attempt or a later one, so a participant that keeps starting processes
    /// runs out of the authority to do so.
    fn decide_start_invocation(
        &self,
        command: &StartInvocation,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        self.ensure_run_creates()?;
        validate_identifier("invocation_id", &command.invocation_id)?;
        if self.invocations.contains_key(&command.invocation_id) {
            return Err(CommitmentError::DuplicateIdentifier {
                kind: "invocation",
                id: command.invocation_id.clone(),
            });
        }
        let contract = self.active_contract(&command.contract_id)?;
        self.ensure_holder(contract, &command.participant)?;
        self.ensure_generation(contract, command.generation)?;
        self.ensure_lease_live(contract)?;
        let attempt = self.attempt(&command.attempt_id)?;
        if attempt.contract_id != contract.contract_id {
            return Err(CommitmentError::AttemptMismatch {
                attempt_id: attempt.attempt_id.clone(),
                contract_id: contract.contract_id.clone(),
            });
        }
        // An attempt from before a reassignment cannot be run again: what closed it was the
        // contract changing hands, and its generation says so.
        if self.enforces(Check::Fencing) && attempt.generation != contract.lease.generation {
            return Err(CommitmentError::StaleGeneration {
                contract_id: contract.contract_id.clone(),
                seen: attempt.generation,
                current: contract.lease.generation,
            });
        }
        self.ensure_attempt_idle(&attempt.attempt_id)?;
        self.ensure_cursor_committed(command.cursor)?;
        let account = AccountRef::TaskContract {
            contract_id: contract.contract_id.clone(),
        };
        let started = CommitmentEvent::InvocationStarted {
            invocation_id: command.invocation_id.clone(),
            attempt_id: command.attempt_id.clone(),
            contract_id: contract.contract_id.clone(),
            participant: command.participant.clone(),
            generation: command.generation,
            cursor: command.cursor,
        };
        self.with_invocation_start(account, started)
    }

    /// End a process slice without returning the task contract, registering what would be worth
    /// resuming for.
    ///
    /// The conditions name committed facts and the cursor says which of them are still ahead. The
    /// deadline may not outlive the lease, because a wake that outlived the wall time somebody paid
    /// for would keep the run open on capacity nobody holds.
    fn decide_yield(
        &self,
        command: &YieldInvocation,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        let invocation = self.invocation(&command.invocation_id)?;
        if invocation.state != InvocationState::Running {
            return Err(CommitmentError::InvocationNotRunning {
                invocation_id: invocation.invocation_id.clone(),
            });
        }
        if invocation.participant != command.participant {
            return Err(CommitmentError::NotAuthorized {
                principal: command.participant.clone(),
            });
        }
        let contract = self.active_contract(&invocation.contract_id)?;
        self.ensure_holder(contract, &command.participant)?;
        self.ensure_generation(contract, command.generation)?;
        self.ensure_lease_live(contract)?;
        if !(1..=MAX_WAKE_CONDITIONS).contains(&command.conditions.len()) {
            return Err(CommitmentError::InvalidWakeConditions);
        }
        for condition in &command.conditions {
            validate_condition(condition)?;
        }
        if command.cursor < invocation.cursor {
            return Err(CommitmentError::CursorRegression {
                current: invocation.cursor,
                seen: command.cursor,
            });
        }
        self.ensure_cursor_committed(command.cursor)?;
        if command.wake_deadline > contract.lease.expires_at {
            return Err(CommitmentError::WakeDeadlineUnfunded {
                contract_id: contract.contract_id.clone(),
                wake_deadline: command.wake_deadline,
                expires_at: contract.lease.expires_at,
            });
        }
        Ok(vec![CommitmentEvent::InvocationYielded {
            invocation_id: command.invocation_id.clone(),
            cursor: command.cursor,
            conditions: command.conditions.clone(),
            wake_deadline: command.wake_deadline,
        }])
    }

    /// Admit a yielded slice back into a running process.
    ///
    /// This is the only transition whose precondition is a fact rather than a record: without a
    /// committed fact after the cursor that one of the registered conditions names, there is
    /// nothing to resume for and the command is refused. The sequence of that fact is committed
    /// with the resumption, so what authorized every wake stays readable in the stream.
    fn decide_resume(
        &self,
        command: &ResumeInvocation,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        self.ensure_run_creates()?;
        let invocation = self.invocation(&command.invocation_id)?;
        if invocation.state != InvocationState::Yielded {
            return Err(CommitmentError::InvocationNotYielded {
                invocation_id: invocation.invocation_id.clone(),
            });
        }
        if invocation.participant != command.participant {
            return Err(CommitmentError::NotAuthorized {
                principal: command.participant.clone(),
            });
        }
        let wake =
            invocation
                .wake
                .as_ref()
                .ok_or_else(|| CommitmentError::InvocationNotYielded {
                    invocation_id: invocation.invocation_id.clone(),
                })?;
        if self.now > wake.wake_deadline {
            return Err(CommitmentError::WakeDeadlinePassed {
                invocation_id: invocation.invocation_id.clone(),
                wake_deadline: wake.wake_deadline,
                now: self.now,
            });
        }
        let contract = self.active_contract(&invocation.contract_id)?;
        self.ensure_holder(contract, &command.participant)?;
        self.ensure_generation(contract, command.generation)?;
        self.ensure_generation(contract, invocation.generation)?;
        self.ensure_lease_live(contract)?;
        if self.enforces(Check::WakeCount)
            && self.attempt_wakes(&invocation.attempt_id) >= MAX_ATTEMPT_WAKES
        {
            return Err(CommitmentError::WakeBudgetExhausted {
                attempt_id: invocation.attempt_id.clone(),
            });
        }
        self.ensure_attempt_idle(&invocation.attempt_id)?;
        let matched = self.matching_fact(&command.invocation_id);
        if self.enforces(Check::WakeMatch) && matched.is_none() {
            return Err(CommitmentError::NoMatchingEvent {
                invocation_id: invocation.invocation_id.clone(),
                cursor: invocation.cursor,
            });
        }
        let account = AccountRef::TaskContract {
            contract_id: contract.contract_id.clone(),
        };
        let resumed = CommitmentEvent::InvocationResumed {
            invocation_id: command.invocation_id.clone(),
            contract_id: contract.contract_id.clone(),
            participant: command.participant.clone(),
            generation: command.generation,
            matched_sequence: matched.unwrap_or(0),
            wakes_used: invocation.wakes_used.saturating_add(1),
        };
        self.with_invocation_start(account, resumed)
    }

    /// One attempt runs one process slice at a time, whether the second slice would be a new one or
    /// a resumed one.
    ///
    /// An attempt is the unit the run counts starts and wakes against, and both bounds are stated
    /// per attempt rather than per slice. Two slices of one attempt running together would spend
    /// those bounds twice over on work the run counts once, and the facts they commit carry the
    /// attempt rather than the slice, so nothing downstream could tell which of the two a
    /// submission or a return came from.
    fn ensure_attempt_idle(&self, attempt_id: &str) -> Result<(), CommitmentError> {
        if !self.enforces(Check::SingleRunningSlice) {
            return Ok(());
        }
        let running = self.invocations.values().find(|invocation| {
            invocation.attempt_id == attempt_id && invocation.state == InvocationState::Running
        });
        if let Some(running) = running {
            return Err(CommitmentError::AttemptAlreadyRunning {
                attempt_id: attempt_id.to_owned(),
                invocation_id: running.invocation_id.clone(),
            });
        }
        Ok(())
    }

    /// One process slice and the creation authority that buys it, as one indivisible pair.
    fn with_invocation_start(
        &self,
        account: AccountRef,
        slice: CommitmentEvent,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        if !self.enforces(Check::InvocationStart) {
            return Ok(vec![slice]);
        }
        let authority = BudgetVector::unit(Dimension::InvocationStarts);
        self.ensure_covers(&account, &authority)?;
        Ok(vec![
            slice,
            CommitmentEvent::BudgetConsumed {
                account,
                amount: authority,
            },
        ])
    }

    /// End a process slice for good. Its own participant may close it, and so may the sponsor of
    /// its task contract, which is how a participant that stopped answering is recorded as lost.
    fn decide_close_invocation(
        &self,
        command: &CloseInvocation,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        let invocation = self.invocation(&command.invocation_id)?;
        if invocation.state == InvocationState::Closed {
            return Err(CommitmentError::InvocationNotRunning {
                invocation_id: invocation.invocation_id.clone(),
            });
        }
        let sponsor = self
            .contracts
            .get(&invocation.contract_id)
            .map(|contract| contract.sponsor.as_str());
        if invocation.participant != command.closer && sponsor != Some(command.closer.as_str()) {
            return Err(CommitmentError::NotAuthorized {
                principal: command.closer.clone(),
            });
        }
        // A deadline is a fact about the clock, not a claim: it may be recorded only once it has
        // actually passed.
        if command.reason == InvocationClosure::WakeDeadlineExpired {
            let deadline = invocation
                .wake
                .as_ref()
                .map_or(0, |wake| wake.wake_deadline);
            if invocation.state != InvocationState::Yielded || self.now <= deadline {
                return Err(CommitmentError::WakeDeadlinePassed {
                    invocation_id: invocation.invocation_id.clone(),
                    wake_deadline: deadline,
                    now: self.now,
                });
            }
        }
        Ok(vec![CommitmentEvent::InvocationClosed {
            invocation_id: command.invocation_id.clone(),
            reason: command.reason,
        }])
    }

    /// Spend one protected-query reservation on the exact candidate a task contract recorded.
    ///
    /// The digest is compared rather than trusted: a query names the candidate that was actually
    /// submitted, so a verdict cannot be attached to a different bundle afterwards.
    fn decide_verification(
        &self,
        command: &RecordVerification,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        self.ensure_run_creates()?;
        validate_digest("candidate_digest", &command.candidate_digest)?;
        let contract = self.active_contract(&command.contract_id)?;
        self.ensure_holder(contract, &command.participant)?;
        self.ensure_generation(contract, command.generation)?;
        self.ensure_lease_live(contract)?;
        let Some(recorded) = &contract.candidate_digest else {
            return Err(CommitmentError::NoCandidate {
                contract_id: contract.contract_id.clone(),
            });
        };
        if recorded != &command.candidate_digest {
            return Err(CommitmentError::CandidateMismatch {
                contract_id: contract.contract_id.clone(),
                candidate_digest: command.candidate_digest.clone(),
            });
        }
        let account = AccountRef::TaskContract {
            contract_id: contract.contract_id.clone(),
        };
        let reservation = BudgetVector::unit(Dimension::VerificationQueries);
        self.ensure_covers(&account, &reservation)?;
        Ok(vec![
            CommitmentEvent::VerificationRecorded {
                contract_id: contract.contract_id.clone(),
                candidate_digest: command.candidate_digest.clone(),
                verdict: command.verdict,
                root_scope: self.scoped(self.is_root_scope(&contract.contract_id)),
            },
            // An infrastructure error consumes the reservation like any other verdict. Nothing was
            // learned about the candidate, and the query was spent all the same.
            CommitmentEvent::BudgetConsumed {
                account,
                amount: reservation,
            },
        ])
    }

    /// Stop the run. Only the participant the root obligation belongs to may do it.
    fn decide_stop_run(&self, command: &StopRun) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        if self.stopped.is_some() {
            return Err(CommitmentError::RunAlreadyStopped);
        }
        let root = self.obligation(&self.root_obligation)?;
        if root.owner != command.authority {
            return Err(CommitmentError::NotAuthorized {
                principal: command.authority.clone(),
            });
        }
        Ok(vec![CommitmentEvent::RunStopped {
            authority: command.authority.clone(),
            reason: command.reason,
        }])
    }

    fn ensure_cursor_committed(&self, cursor: u64) -> Result<(), CommitmentError> {
        if cursor > self.sequence() {
            return Err(CommitmentError::CursorAhead {
                seen: cursor,
                committed: self.sequence(),
            });
        }
        Ok(())
    }

    fn invocation(&self, invocation_id: &str) -> Result<&InvocationRecord, CommitmentError> {
        self.invocations
            .get(invocation_id)
            .ok_or_else(|| CommitmentError::Unknown {
                kind: "invocation",
                id: invocation_id.to_owned(),
            })
    }

    /// Unspent escrow returns to the account that funded the offer this contract came from.
    fn settlement(
        &self,
        contract: &TaskContractRecord,
    ) -> Result<Option<CommitmentEvent>, CommitmentError> {
        if contract.escrow.is_zero() {
            return Ok(None);
        }
        let offer = self.offer(&contract.offer_id)?;
        let destination = self.settlement_destination(&offer.funding_source, &offer.sponsor)?;
        Ok(Some(CommitmentEvent::BudgetTransferred {
            from: AccountRef::TaskContract {
                contract_id: contract.contract_id.clone(),
            },
            to: self.returned_to(destination, &contract.lease.holder),
            amount: contract.escrow,
        }))
    }

    /// A task contract stays open while a reservation is still due back to it: an offer its escrow
    /// funded that has not settled, or a contract formed from such an offer that has not returned.
    ///
    /// This is the other half of the same rule as [`Self::live_account`]. That one refuses to
    /// settle into an account that has closed; without this one the refusal would strand the
    /// reservation instead, because the contract could close first and the escrow would then have
    /// nowhere left to go.
    ///
    /// An offer hanging under this contract's own obligation is left to the causal check: there the
    /// money follows the work, and closing the work already clears the account. What is checked
    /// here is the remainder, where a contractor funded an offer from one of its contracts and hung
    /// it under the obligation of another, so that nothing causal connects the two.
    fn ensure_reservations_returned(
        &self,
        contract: &TaskContractRecord,
    ) -> Result<(), CommitmentError> {
        if !self.enforces(Check::ReturningReservation) {
            return Ok(());
        }
        for offer in self.offers.values() {
            if !funded_by(&offer.funding_source, &contract.contract_id)
                || offer.parent_obligation == contract.obligation_id
            {
                continue;
            }
            if offer.state != OfferState::Settled {
                return Err(CommitmentError::ReservationOutstanding {
                    contract_id: contract.contract_id.clone(),
                    outstanding: offer.offer_id.clone(),
                });
            }
            if let Some(open) = self.contracts.values().find(|awarded| {
                awarded.offer_id == offer.offer_id && awarded.state == ContractState::Active
            }) {
                return Err(CommitmentError::ReservationOutstanding {
                    contract_id: contract.contract_id.clone(),
                    outstanding: open.contract_id.clone(),
                });
            }
        }
        Ok(())
    }

    /// Causal work closes from the leaves inward: no descendant obligation may still be open, and
    /// no offer this obligation parents may still hold escrow.
    fn ensure_causal_work_closed(&self, obligation_id: &str) -> Result<(), CommitmentError> {
        if !self.enforces(Check::ChildReturn) {
            return Ok(());
        }
        let obligation = self.obligation(obligation_id)?;
        if let Some(descendant) = obligation.children.iter().find(|child| {
            self.obligations
                .get(*child)
                .is_none_or(|child| child.state != ObligationState::Terminal)
        }) {
            return Err(CommitmentError::DescendantOutstanding {
                obligation_id: obligation_id.to_owned(),
                descendant: descendant.clone(),
            });
        }
        if let Some(offer) = self.offers.values().find(|offer| {
            offer.parent_obligation == obligation_id && offer.state != OfferState::Settled
        }) {
            return Err(CommitmentError::OfferUnsettled {
                obligation_id: obligation_id.to_owned(),
                offer_id: offer.offer_id.clone(),
            });
        }
        Ok(())
    }

    fn ensure_holder(
        &self,
        contract: &TaskContractRecord,
        participant: &str,
    ) -> Result<(), CommitmentError> {
        if contract.lease.holder == participant {
            Ok(())
        } else {
            Err(CommitmentError::NotAuthorized {
                principal: participant.to_owned(),
            })
        }
    }

    fn ensure_generation(
        &self,
        contract: &TaskContractRecord,
        seen: u64,
    ) -> Result<(), CommitmentError> {
        if !self.enforces(Check::Fencing) || seen == contract.lease.generation {
            Ok(())
        } else {
            Err(CommitmentError::StaleGeneration {
                contract_id: contract.contract_id.clone(),
                seen,
                current: contract.lease.generation,
            })
        }
    }

    fn ensure_lease_live(&self, contract: &TaskContractRecord) -> Result<(), CommitmentError> {
        if self.now <= contract.lease.expires_at {
            Ok(())
        } else {
            Err(CommitmentError::LeaseExpired {
                contract_id: contract.contract_id.clone(),
                expires_at: contract.lease.expires_at,
                now: self.now,
            })
        }
    }

    fn ensure_covers(
        &self,
        account: &AccountRef,
        amount: &BudgetVector,
    ) -> Result<(), CommitmentError> {
        let balance = self.balance_of(account)?;
        match balance.shortfall(amount) {
            None => Ok(()),
            Some(_) if !self.enforces(Check::Covering) => Ok(()),
            Some(dimension) => Err(CommitmentError::InsufficientBudget {
                account: account_label(account),
                dimension,
            }),
        }
    }

    fn balance_of(&self, account: &AccountRef) -> Result<BudgetVector, CommitmentError> {
        match account {
            AccountRef::Participant { participant_id } => {
                Ok(self.participant(participant_id)?.balance)
            }
            AccountRef::Offer { offer_id } => Ok(self.offer(offer_id)?.escrow),
            AccountRef::TaskContract { contract_id } => Ok(self.contract(contract_id)?.escrow),
        }
    }

    /// The account a command may draw on: it must be live, and a task contract's escrow may be
    /// spent only by the participant currently contracted to do the work.
    fn funding_account(
        &self,
        source: &FundingSource,
        sponsor: &str,
    ) -> Result<AccountRef, CommitmentError> {
        if let FundingSource::TaskContract { contract_id } = source {
            let contract = self.contract(contract_id)?;
            if contract.contractor != sponsor {
                return Err(CommitmentError::FundingContractMismatch {
                    contract_id: contract_id.clone(),
                });
            }
        }
        self.live_account(source, sponsor)
    }

    /// The account a reservation returns to. Escrow belongs to the contract rather than to whoever
    /// currently holds it, so a change of contractor does not strand it; what the account must
    /// still be is one that can spend.
    fn settlement_destination(
        &self,
        source: &FundingSource,
        sponsor: &str,
    ) -> Result<AccountRef, CommitmentError> {
        self.live_account(source, sponsor)
    }

    /// A task contract stops being an account the moment it reaches a terminal state: nothing may
    /// be drawn from it and nothing may be settled into it, because whatever landed there could
    /// then be spent a second time by a participant whose obligation is already closed.
    fn live_account(
        &self,
        source: &FundingSource,
        sponsor: &str,
    ) -> Result<AccountRef, CommitmentError> {
        match source {
            FundingSource::Participant => {
                self.participant(sponsor)?;
                Ok(AccountRef::Participant {
                    participant_id: sponsor.to_owned(),
                })
            }
            FundingSource::TaskContract { contract_id } => {
                let contract = self.contract(contract_id)?;
                if self.enforces(Check::LiveAccount) && contract.state != ContractState::Active {
                    return Err(CommitmentError::AccountClosed {
                        contract_id: contract_id.clone(),
                    });
                }
                Ok(AccountRef::TaskContract {
                    contract_id: contract_id.clone(),
                })
            }
        }
    }

    fn participant(&self, participant_id: &str) -> Result<&ParticipantRecord, CommitmentError> {
        self.participants
            .get(participant_id)
            .ok_or_else(|| CommitmentError::Unknown {
                kind: "participant",
                id: participant_id.to_owned(),
            })
    }

    fn offer(&self, offer_id: &str) -> Result<&OfferRecord, CommitmentError> {
        self.offers
            .get(offer_id)
            .ok_or_else(|| CommitmentError::Unknown {
                kind: "offer",
                id: offer_id.to_owned(),
            })
    }

    fn bid(&self, bid_id: &str) -> Result<&BidRecord, CommitmentError> {
        self.bids
            .get(bid_id)
            .ok_or_else(|| CommitmentError::Unknown {
                kind: "bid",
                id: bid_id.to_owned(),
            })
    }

    fn contract(&self, contract_id: &str) -> Result<&TaskContractRecord, CommitmentError> {
        self.contracts
            .get(contract_id)
            .ok_or_else(|| CommitmentError::Unknown {
                kind: "task contract",
                id: contract_id.to_owned(),
            })
    }

    fn active_contract(&self, contract_id: &str) -> Result<&TaskContractRecord, CommitmentError> {
        let contract = self.contract(contract_id)?;
        if contract.state == ContractState::Active {
            Ok(contract)
        } else {
            Err(CommitmentError::ContractNotActive {
                contract_id: contract_id.to_owned(),
            })
        }
    }

    fn obligation(&self, obligation_id: &str) -> Result<&ObligationRecord, CommitmentError> {
        self.obligations
            .get(obligation_id)
            .ok_or_else(|| CommitmentError::Unknown {
                kind: "obligation",
                id: obligation_id.to_owned(),
            })
    }

    fn attempt(&self, attempt_id: &str) -> Result<&AttemptRecord, CommitmentError> {
        self.attempts
            .get(attempt_id)
            .ok_or_else(|| CommitmentError::Unknown {
                kind: "attempt",
                id: attempt_id.to_owned(),
            })
    }

    fn candidate(&self, candidate_digest: &str) -> Result<&CandidateRecord, CommitmentError> {
        self.candidates
            .get(candidate_digest)
            .ok_or_else(|| CommitmentError::Unknown {
                kind: "candidate",
                id: candidate_digest.to_owned(),
            })
    }

    /// Commit one decided fact. Every precondition was checked while deciding, so a fact that
    /// cannot be recorded is a defect in that deciding rather than an ordinary refusal: it is
    /// reported instead of dropped, and the caller discards the copy it was being written to.
    fn apply(&mut self, event: &CommitmentEvent) -> Result<(), CommitmentError> {
        self.facts.push(event.clone());
        let sequence = self.sequence();
        match event {
            CommitmentEvent::ClockAdvanced { to } => self.now = *to,
            CommitmentEvent::ParticipantRegistered {
                participant_id,
                principal_id,
            } => {
                self.participants.insert(
                    participant_id.clone(),
                    ParticipantRecord {
                        participant_id: participant_id.clone(),
                        principal_id: principal_id.clone(),
                        balance: BudgetVector::ZERO,
                    },
                );
            }
            CommitmentEvent::BudgetTransferred { from, to, amount } => {
                self.debit(from, amount)?;
                self.credit(to, amount)?;
            }
            CommitmentEvent::BudgetConsumed { account, amount } => {
                self.debit(account, amount)?;
                self.consumed = self
                    .consumed
                    .checked_add(amount)
                    .map_err(|dimension| CommitmentError::BudgetOverflow { dimension })?;
            }
            CommitmentEvent::OfferAdvertised {
                offer_id,
                sponsor,
                parent_obligation,
                funding_source,
                task_scope,
                base_digest,
                intent_digest,
                artifact_class,
                dependencies,
                capability_scope,
                execution_escrow,
                policy,
                bid_deadline,
                offer_deadline,
                max_awards,
            } => {
                self.offers.insert(
                    offer_id.clone(),
                    OfferRecord {
                        offer_id: offer_id.clone(),
                        sponsor: sponsor.clone(),
                        parent_obligation: parent_obligation.clone(),
                        funding_source: funding_source.clone(),
                        task_scope: task_scope.clone(),
                        base_digest: base_digest.clone(),
                        intent_digest: intent_digest.clone(),
                        artifact_class: artifact_class.clone(),
                        dependencies: dependencies.clone(),
                        capability_scope: capability_scope.clone(),
                        execution_escrow: *execution_escrow,
                        escrow: BudgetVector::ZERO,
                        policy: policy.clone(),
                        bid_deadline: *bid_deadline,
                        offer_deadline: *offer_deadline,
                        max_awards: *max_awards,
                        awards_made: 0,
                        state: OfferState::Advertised,
                    },
                );
            }
            CommitmentEvent::OfferWithdrawn { offer_id } => {
                if let Some(offer) = self.offers.get_mut(offer_id) {
                    offer.state = OfferState::Withdrawn;
                }
            }
            CommitmentEvent::OfferSettled { offer_id } => {
                if let Some(offer) = self.offers.get_mut(offer_id) {
                    offer.state = OfferState::Settled;
                }
            }
            CommitmentEvent::BidRecorded {
                bid_id,
                offer_id,
                bidder,
                requested_escrow,
                artifact_class,
                proposal_digest,
                expires_at,
                origin,
            } => {
                self.bids.insert(
                    bid_id.clone(),
                    BidRecord {
                        bid_id: bid_id.clone(),
                        offer_id: offer_id.clone(),
                        bidder: bidder.clone(),
                        requested_escrow: *requested_escrow,
                        artifact_class: artifact_class.clone(),
                        proposal_digest: proposal_digest.clone(),
                        expires_at: *expires_at,
                        origin: *origin,
                        state: BidState::Live,
                    },
                );
            }
            CommitmentEvent::BidWithdrawn { bid_id } => {
                if let Some(bid) = self.bids.get_mut(bid_id) {
                    bid.state = BidState::Withdrawn;
                }
            }
            CommitmentEvent::TaskContractFormed {
                contract_id,
                offer_id,
                bid_id,
                sponsor,
                contractor,
                obligation_id,
                task_scope,
                base_digest,
            } => {
                self.contracts.insert(
                    contract_id.clone(),
                    TaskContractRecord {
                        contract_id: contract_id.clone(),
                        offer_id: offer_id.clone(),
                        bid_id: bid_id.clone(),
                        sponsor: sponsor.clone(),
                        contractor: contractor.clone(),
                        obligation_id: obligation_id.clone(),
                        task_scope: task_scope.clone(),
                        base_digest: base_digest.clone(),
                        escrow: BudgetVector::ZERO,
                        lease: LeaseRecord {
                            lease_id: String::new(),
                            holder: contractor.clone(),
                            generation: 0,
                            expires_at: 0,
                        },
                        candidate_digest: None,
                        state: ContractState::Active,
                    },
                );
                if let Some(bid) = self.bids.get_mut(bid_id) {
                    bid.state = BidState::Awarded;
                }
                if let Some(offer) = self.offers.get_mut(offer_id) {
                    offer.awards_made = offer.awards_made.saturating_add(1);
                }
            }
            CommitmentEvent::ObligationCreated {
                obligation_id,
                parent,
                owner,
                contract_id,
            } => {
                self.obligations.insert(
                    obligation_id.clone(),
                    ObligationRecord {
                        obligation_id: obligation_id.clone(),
                        parent: Some(parent.clone()),
                        owner: owner.clone(),
                        contract_id: Some(contract_id.clone()),
                        children: Vec::new(),
                        state: ObligationState::Active,
                        outcome: None,
                    },
                );
                if let Some(parent) = self.obligations.get_mut(parent) {
                    parent.children.push(obligation_id.clone());
                }
            }
            CommitmentEvent::LeaseIssued {
                contract_id,
                lease_id,
                holder,
                generation,
                expires_at,
            } => {
                if let Some(contract) = self.contracts.get_mut(contract_id) {
                    contract.lease = LeaseRecord {
                        lease_id: lease_id.clone(),
                        holder: holder.clone(),
                        generation: *generation,
                        expires_at: *expires_at,
                    };
                }
            }
            CommitmentEvent::LeaseRenewed {
                contract_id,
                expires_at,
                ..
            } => {
                if let Some(contract) = self.contracts.get_mut(contract_id) {
                    contract.lease.expires_at = *expires_at;
                }
            }
            CommitmentEvent::ContractReassigned {
                contract_id,
                bid_id,
                holder,
                ..
            } => {
                let obligation_id = self
                    .contracts
                    .get(contract_id)
                    .map(|contract| contract.obligation_id.clone());
                if let Some(contract) = self.contracts.get_mut(contract_id) {
                    contract.contractor = holder.clone();
                }
                if let Some(obligation_id) = obligation_id
                    && let Some(obligation) = self.obligations.get_mut(&obligation_id)
                {
                    obligation.owner = holder.clone();
                }
                if let Some(bid) = self.bids.get_mut(bid_id) {
                    bid.state = BidState::Awarded;
                }
                for attempt in self.attempts.values_mut() {
                    if attempt.contract_id == *contract_id {
                        attempt.state = AttemptState::Closed;
                    }
                }
                Self::supersede_invocations(&mut self.invocations, contract_id);
            }
            CommitmentEvent::AttemptStarted {
                attempt_id,
                contract_id,
                participant,
                generation,
            } => {
                self.attempts.insert(
                    attempt_id.clone(),
                    AttemptRecord {
                        attempt_id: attempt_id.clone(),
                        contract_id: contract_id.clone(),
                        participant: participant.clone(),
                        generation: *generation,
                        state: AttemptState::Running,
                    },
                );
            }
            CommitmentEvent::SubmissionRecorded {
                contract_id,
                candidate_digest,
                ..
            } => {
                if let Some(contract) = self.contracts.get_mut(contract_id) {
                    contract.candidate_digest = Some(candidate_digest.clone());
                }
            }
            CommitmentEvent::ObjectRecorded { object_digest } => {
                self.objects.insert(object_digest.clone());
            }
            // Content is written once. An entry that is already here holds the same bytes, because
            // the key is the digest of those bytes.
            CommitmentEvent::BundleRecorded {
                bundle_digest,
                base_digest,
                parents,
                changes,
            } => {
                self.bundles
                    .entry(bundle_digest.clone())
                    .or_insert_with(|| BundleRecord {
                        bundle_digest: bundle_digest.clone(),
                        base_digest: base_digest.clone(),
                        parents: parents.clone(),
                        changes: changes.clone(),
                    });
            }
            CommitmentEvent::CandidateFormed {
                candidate_digest,
                content_digest,
                contract_id,
                obligation_id,
                participant,
                generation,
                base_digest,
                bundle_digest,
                contributions,
                changes,
            } => {
                self.candidates
                    .entry(candidate_digest.clone())
                    .or_insert_with(|| CandidateRecord {
                        candidate_digest: candidate_digest.clone(),
                        content_digest: content_digest.clone(),
                        contract_id: contract_id.clone(),
                        obligation_id: obligation_id.clone(),
                        participant: participant.clone(),
                        generation: *generation,
                        base_digest: base_digest.clone(),
                        bundle_digest: bundle_digest.clone(),
                        contributions: contributions.clone(),
                        changes: changes.clone(),
                    });
            }
            CommitmentEvent::ConflictRecorded {
                conflict_digest,
                base_digest,
                candidates,
                paths,
            } => {
                self.conflicts
                    .entry(conflict_digest.clone())
                    .or_insert_with(|| ConflictRecord {
                        conflict_digest: conflict_digest.clone(),
                        base_digest: base_digest.clone(),
                        candidates: candidates.clone(),
                        paths: paths.clone(),
                    });
            }
            CommitmentEvent::ObligationReturned {
                obligation_id,
                contract_id,
                outcome,
                ..
            } => {
                if let Some(obligation) = self.obligations.get_mut(obligation_id) {
                    obligation.state = ObligationState::Terminal;
                    obligation.outcome = Some(outcome.clone());
                }
                self.close_contract(contract_id, ContractState::Returned);
            }
            CommitmentEvent::ContractCancelled {
                contract_id,
                obligation_id,
            } => {
                if let Some(obligation) = self.obligations.get_mut(obligation_id) {
                    obligation.state = ObligationState::Terminal;
                    obligation.outcome = Some(Outcome::Cancelled);
                }
                self.close_contract(contract_id, ContractState::Cancelled);
            }
            CommitmentEvent::InvocationStarted {
                invocation_id,
                attempt_id,
                contract_id,
                participant,
                generation,
                cursor,
            } => {
                self.invocations.insert(
                    invocation_id.clone(),
                    InvocationRecord {
                        invocation_id: invocation_id.clone(),
                        attempt_id: attempt_id.clone(),
                        contract_id: contract_id.clone(),
                        participant: participant.clone(),
                        generation: *generation,
                        cursor: *cursor,
                        state: InvocationState::Running,
                        wake: None,
                        yielded_at: 0,
                        wakes_used: 0,
                        closure: None,
                    },
                );
            }
            CommitmentEvent::InvocationYielded {
                invocation_id,
                cursor,
                conditions,
                wake_deadline,
            } => {
                if let Some(invocation) = self.invocations.get_mut(invocation_id) {
                    invocation.state = InvocationState::Yielded;
                    invocation.cursor = *cursor;
                    invocation.wake = Some(WakeRegistration {
                        conditions: conditions.clone(),
                        wake_deadline: *wake_deadline,
                    });
                    invocation.yielded_at = sequence;
                }
            }
            CommitmentEvent::InvocationResumed {
                invocation_id,
                wakes_used,
                ..
            } => {
                if let Some(invocation) = self.invocations.get_mut(invocation_id) {
                    invocation.state = InvocationState::Running;
                    invocation.wake = None;
                    invocation.wakes_used = *wakes_used;
                }
            }
            CommitmentEvent::InvocationClosed {
                invocation_id,
                reason,
            } => {
                if let Some(invocation) = self.invocations.get_mut(invocation_id) {
                    invocation.state = InvocationState::Closed;
                    invocation.wake = None;
                    invocation.closure = Some(*reason);
                }
            }
            CommitmentEvent::VerificationRecorded {
                contract_id,
                candidate_digest,
                verdict,
                root_scope,
            } => {
                self.verifications.push(VerificationRecord {
                    contract_id: contract_id.clone(),
                    candidate_digest: candidate_digest.clone(),
                    verdict: *verdict,
                    root_scope: *root_scope,
                });
            }
            CommitmentEvent::RunStopped { reason, .. } => {
                self.stopped.get_or_insert(*reason);
            }
        }
        Ok(())
    }

    fn close_contract(&mut self, contract_id: &str, state: ContractState) {
        if let Some(contract) = self.contracts.get_mut(contract_id) {
            contract.state = state;
        }
        for attempt in self.attempts.values_mut() {
            if attempt.contract_id == contract_id {
                attempt.state = AttemptState::Closed;
            }
        }
        // A process slice of a contract that has closed or changed hands has nothing left to
        // advance. Leaving it open would let a wake nobody can honour hold the run alive.
        Self::supersede_invocations(&mut self.invocations, contract_id);
    }

    fn supersede_invocations(
        invocations: &mut BTreeMap<String, InvocationRecord>,
        contract_id: &str,
    ) {
        for invocation in invocations.values_mut() {
            if invocation.contract_id == contract_id && invocation.state != InvocationState::Closed
            {
                invocation.state = InvocationState::Closed;
                invocation.wake = None;
                invocation.closure = Some(InvocationClosure::Superseded);
            }
        }
    }

    fn debit(
        &mut self,
        account: &AccountRef,
        amount: &BudgetVector,
    ) -> Result<(), CommitmentError> {
        let label = account_label(account);
        let Some(balance) = self.balance_mut(account) else {
            return Err(CommitmentError::Unknown {
                kind: "account",
                id: label,
            });
        };
        match balance.checked_sub(amount) {
            Ok(remaining) => {
                *balance = remaining;
                Ok(())
            }
            Err(dimension) => Err(CommitmentError::UncoveredDebit {
                account: label,
                dimension,
            }),
        }
    }

    fn credit(
        &mut self,
        account: &AccountRef,
        amount: &BudgetVector,
    ) -> Result<(), CommitmentError> {
        let label = account_label(account);
        let Some(balance) = self.balance_mut(account) else {
            return Err(CommitmentError::Unknown {
                kind: "account",
                id: label,
            });
        };
        *balance = balance
            .checked_add(amount)
            .map_err(|dimension| CommitmentError::BudgetOverflow { dimension })?;
        Ok(())
    }

    fn balance_mut(&mut self, account: &AccountRef) -> Option<&mut BudgetVector> {
        match account {
            AccountRef::Participant { participant_id } => self
                .participants
                .get_mut(participant_id)
                .map(|participant| &mut participant.balance),
            AccountRef::Offer { offer_id } => {
                self.offers.get_mut(offer_id).map(|offer| &mut offer.escrow)
            }
            AccountRef::TaskContract { contract_id } => self
                .contracts
                .get_mut(contract_id)
                .map(|contract| &mut contract.escrow),
        }
    }
}

fn funded_by(source: &FundingSource, contract_id: &str) -> bool {
    match source {
        FundingSource::Participant => false,
        FundingSource::TaskContract {
            contract_id: funder,
        } => funder == contract_id,
    }
}

fn add(left: &BudgetVector, right: &BudgetVector) -> Result<BudgetVector, CommitmentError> {
    left.checked_add(right)
        .map_err(|dimension| CommitmentError::BudgetOverflow { dimension })
}

fn account_label(account: &AccountRef) -> String {
    match account {
        AccountRef::Participant { participant_id } => format!("participant {participant_id}"),
        AccountRef::Offer { offer_id } => format!("offer {offer_id}"),
        AccountRef::TaskContract { contract_id } => format!("task contract {contract_id}"),
    }
}

fn validate_identifier(kind: &'static str, value: &str) -> Result<(), CommitmentError> {
    let length = value.chars().count();
    if (1..=MAX_IDENTIFIER_CHARS).contains(&length) {
        Ok(())
    } else {
        Err(CommitmentError::InvalidIdentifier { kind })
    }
}

/// A wake condition carries one identifier and nothing else, so validating it is validating that
/// identifier. There is no payload here to bound, because there is no payload.
fn validate_condition(condition: &WakeCondition) -> Result<(), CommitmentError> {
    match condition {
        WakeCondition::ObligationReturned { obligation_id } => {
            validate_identifier("obligation_id", obligation_id)
        }
        WakeCondition::BidRecorded { offer_id } | WakeCondition::OfferClosed { offer_id } => {
            validate_identifier("offer_id", offer_id)
        }
        WakeCondition::LeaseIssued { contract_id }
        | WakeCondition::SubmissionRecorded { contract_id }
        | WakeCondition::ContractCancelled { contract_id }
        | WakeCondition::VerificationRecorded { contract_id } => {
            validate_identifier("contract_id", contract_id)
        }
        WakeCondition::RunStopped => Ok(()),
    }
}

fn validate_digest(kind: &'static str, value: &str) -> Result<(), CommitmentError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(CommitmentError::InvalidDigest { kind })
    }
}

/// A path names a place inside the result and nowhere else.
///
/// The kernel compares paths and never opens one, but a path that leaves the tree it belongs to is
/// not content: it is an instruction to whatever materializes the result. Such a path is refused
/// here rather than carried as an opaque token nobody checked. Control characters are refused with
/// it, both because no tree path holds one and because their escaped form is six times their size
/// in the record this path is committed in.
fn validate_path(path: &str) -> Result<(), CommitmentError> {
    let refused = || CommitmentError::InvalidPath {
        path: path.to_owned(),
    };
    if path.is_empty() || path.len() > MAX_PATH_BYTES {
        return Err(refused());
    }
    if path.contains('\\') || path.chars().any(char::is_control) {
        return Err(refused());
    }
    for component in path.split('/') {
        if matches!(component, "" | "." | "..") {
            return Err(refused());
        }
    }
    Ok(())
}

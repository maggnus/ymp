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

use std::collections::BTreeMap;

use serde::Serialize;

use super::budget::{BudgetVector, Dimension};
use super::protocol::{
    AcceptOpen, AdvanceClock, Advertise, Award, CancelContract, CommitmentCommand, CommitmentError,
    CommitmentEvent, MAX_AWARDS, MAX_LEASE_MS, MAX_SCOPE_ENTRIES, Reassign, RecordBid,
    RegisterParticipant, RenewLease, ReturnObligation, SettleOffer, StartAttempt, SubmitResult,
    WithdrawBid, WithdrawOffer,
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
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommitmentLedger {
    now: u64,
    initial_total: BudgetVector,
    consumed: BudgetVector,
    participants: BTreeMap<String, ParticipantRecord>,
    offers: BTreeMap<String, OfferRecord>,
    bids: BTreeMap<String, BidRecord>,
    contracts: BTreeMap<String, TaskContractRecord>,
    obligations: BTreeMap<String, ObligationRecord>,
    attempts: BTreeMap<String, AttemptRecord>,
    #[cfg(test)]
    #[serde(skip)]
    disabled: DisabledChecks,
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
            participants,
            offers: BTreeMap::new(),
            bids: BTreeMap::new(),
            contracts: BTreeMap::new(),
            obligations,
            attempts: BTreeMap::new(),
            #[cfg(test)]
            disabled: DisabledChecks::default(),
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

    #[cfg(test)]
    pub(crate) fn disable_checks(&mut self, disabled: DisabledChecks) {
        self.disabled = disabled;
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
        }
    }

    #[cfg(not(test))]
    #[allow(clippy::unused_self)]
    const fn enforces(&self, _check: Check) -> bool {
        true
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
            CommitmentCommand::Reassign(command) => self.decide_reassign(command),
            CommitmentCommand::ReturnObligation(command) => self.decide_return(command),
            CommitmentCommand::CancelContract(command) => self.decide_cancel(command),
            CommitmentCommand::AdvanceClock(command) => self.decide_advance_clock(command),
        }
    }

    fn decide_register(
        &self,
        command: &RegisterParticipant,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
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
            from: offer_account,
            to: contract_account.clone(),
            amount: bid.requested_escrow,
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

    fn decide_submit(
        &self,
        command: &SubmitResult,
    ) -> Result<Vec<CommitmentEvent>, CommitmentError> {
        validate_digest("candidate_digest", &command.candidate_digest)?;
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
        if self.enforces(Check::Fencing) && attempt.generation != contract.lease.generation {
            return Err(CommitmentError::StaleGeneration {
                contract_id: contract.contract_id.clone(),
                seen: attempt.generation,
                current: contract.lease.generation,
            });
        }
        Ok(vec![CommitmentEvent::SubmissionRecorded {
            contract_id: contract.contract_id.clone(),
            attempt_id: command.attempt_id.clone(),
            generation: command.generation,
            candidate_digest: command.candidate_digest.clone(),
        }])
    }

    /// Issue the next fencing generation for a contract whose lease has run out. The sponsor names
    /// the consent that replaces the old one; the previous generation can no longer advance
    /// anything.
    fn decide_reassign(&self, command: &Reassign) -> Result<Vec<CommitmentEvent>, CommitmentError> {
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
            to: destination,
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

    /// Commit one decided fact. Every precondition was checked while deciding, so a fact that
    /// cannot be recorded is a defect in that deciding rather than an ordinary refusal: it is
    /// reported instead of dropped, and the caller discards the copy it was being written to.
    fn apply(&mut self, event: &CommitmentEvent) -> Result<(), CommitmentError> {
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

//! Local commitments: budgets, offers, consent, awards, escrow, leases and work obligations.
//!
//! A task relationship here is created by a sponsor's offer and a participant's consent, and by
//! nothing else. The kernel in this module conserves resources, serializes contention, fences
//! stale holders and keeps causal work accountable. It has no way to prefer one participant over
//! another: the only participant-shaped inputs any transition reads are identifiers it compares
//! for equality, integer quantities it subtracts, deadlines it compares against the clock, and
//! digests of inert content it stores without opening.
//!
//! What a command may not do is as much of the design as what it may. There is no transition that
//! selects a bid, no field carrying a skill, grade, model or rationale, and no ordering of
//! participants other than the order in which their records were committed.
//!
//! One boundary assumption is load-bearing and is stated rather than implied: the acting
//! participant named by a command — its sponsor, bidder, holder or participant field — is supplied
//! by the trusted controller from the authenticated connection, not chosen by the caller. This
//! module checks that the named participant holds the authority the command needs; it cannot check
//! that the name was honestly obtained, and nothing here should be read as if it could.

mod budget;
mod ledger;
mod protocol;
mod records;

#[cfg(test)]
mod schedules;
#[cfg(test)]
mod tests;

pub use budget::{BudgetVector, DIMENSION_COUNT, DIMENSIONS, Dimension, DimensionKind};
pub use ledger::CommitmentLedger;
pub use protocol::{
    AcceptOpen, AdvanceClock, Advertise, Award, CancelContract, CommitmentCommand, CommitmentError,
    CommitmentEvent, MAX_AWARDS, MAX_LEASE_MS, MAX_SCOPE_ENTRIES, Reassign, RecordBid,
    RegisterParticipant, RenewLease, ReturnObligation, SettleOffer, StartAttempt, SubmitResult,
    WithdrawBid, WithdrawOffer,
};
pub use records::{
    AccountRef, AttemptRecord, AttemptState, BidOrigin, BidRecord, BidState, ContractState,
    FundingSource, LeaseRecord, ObligationRecord, ObligationState, OfferPolicy, OfferRecord,
    OfferState, Outcome, ParticipantRecord, TaskContractRecord,
};

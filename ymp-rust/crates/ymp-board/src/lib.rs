//! The scoped collaboration board: attributed, bounded, inert publication between participants.
//!
//! This is the untrusted plane. What it is trusted for is who published what, in which order, to
//! which audience, and at what cost; what it is not trusted for is whether any of it is true,
//! important, or meant honestly. The two are kept apart by the shape of the module rather than by a
//! rule somebody has to remember: attribution and audience are typed records the kernel here
//! decides on, and the payload is a digest and a byte count that no transition reads.
//!
//! A payload never enters this crate. A caller hashes the bytes it is about to publish into a
//! [`Payload`], and the command carries that digest and that length. There is consequently no
//! transition whose input contains a capability token, a command, an address, a consent sentence, a
//! protected reference or an instruction to a tool, because there is no transition whose input
//! contains bytes. What a participant does after reading a message is that participant's own act
//! under its own authority, issued to another plane entirely.
//!
//! The plane boundary is also the dependency list. This crate depends on no other ymp package, so
//! nothing here can form a task contract, move escrow, issue a capability, request a protected
//! query or record a verdict: those types are not in scope, and adding them would be a visible
//! change to the manifest rather than a quiet one to a function body.
//!
//! What the board does own is finite. Publishing, refreshing salience, opening an audience,
//! requesting one, committing an assessment and receiving delivered bytes each spend a dimension of
//! a [communication allowance](CommunicationAllowance) that no other dimension pays for. The
//! allowance namespace is the board's own and is deliberately not the control plane's budget
//! vector: capacity granted for talking cannot be spent on starting a participant, and the two
//! cannot be confused by a caller that holds one and needs the other.
//!
//! The board keeps its records in a section of its own, described in [`store`], and not in the
//! control journal: this plane is not authoritative, so its records are versioned, written and
//! read apart from the ones that are. A board is restored by replaying its own recorded facts, so
//! there is no second on-disk representation that could disagree with them, and a lost record file
//! is refused by name rather than read as a board on which nothing was ever said.
//!
//! Audit history and active salience are separate. Every message record is permanent, and the
//! [audit projection](BoardLedger::audit) keeps returning it forever; the active projection a
//! participant is delivered stops carrying it once its salience expires, and refreshing it costs
//! another publication. Stale coordination cues therefore evaporate while the evidence of what was
//! said does not.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod budget;
pub mod ledger;
pub mod observatory;
pub mod protocol;
pub mod records;
pub mod store;

#[cfg(test)]
mod tests;

pub use budget::{ALLOWANCES, Allowance, AllowanceKind, CommunicationAllowance};
pub use ledger::{BoardLedger, DeliveredMessage, Delivery};
pub use observatory::{
    Basis, Edge, EdgeKind, MIN_CAUSAL_REPLICATIONS, NodeRef, ViewDefect, ViewState,
};
pub use protocol::{
    AdvanceClock, BoardCommand, BoardError, BoardEvent, CommitAssessment, GrantAudience,
    InitialMember, LeaveAudience, NoteAncestry, NoteVerdict, OpenScope, Publish, ReadBoard,
    RecordIntervention, RefreshSalience, RegisterParticipant, RequestAudience, RevealContext,
};
pub use records::{
    AccountRecord, AccountRef, Admissibility, AncestryRecord, AssessmentRecord, Audience,
    AudienceRequestRecord, DeliveryReceipt, GrantRecord, GrantState, InterventionKind,
    InterventionRecord, MessageKind, MessageRecord, ObservedVerdict, ReaderRecord, Reference,
    Relation, ReviewPolicy, ReviewerRecord, ReviewerState, Rights, RunKeepingAuthority, ScopeKind,
    ScopeRecord, VerdictRecord,
};
pub use store::{
    BOARD_EVIDENCE_KIND, BOARD_RECORD_KIND, BOARD_SECTION, BoardEvidence, BoardOpening,
    BoardRecordError, BoardStore, FACT_RECORD, MAX_FACT_BYTES, MAX_RECORD_BYTES, OPENING_RECORD,
    RecordedFact,
};

/// The version of the collaboration records this crate decides and projects.
///
/// It is the board's own version and is deliberately not the control journal's: the two planes have
/// separate records, separate readers and separate export rules, so one may change without the
/// other being reinterpreted.
pub const BOARD_SCHEMA_VERSION: u32 = 1;

/// The longest identifier any board record carries.
pub const MAX_IDENTIFIER_CHARS: usize = 128;
/// The largest payload one detailed message may state, in bytes.
pub const MAX_PAYLOAD_BYTES: u64 = 8 * 1024;
/// The largest payload a project-discovery notice may state, in bytes.
///
/// Discovery announces that work or help exists. A notice that could carry a full finding would
/// make every detailed audience decorative, so the bound is what keeps discovery a summary.
pub const MAX_DISCOVERY_PAYLOAD_BYTES: u64 = 512;
/// The largest number of references one detailed message may carry.
pub const MAX_REFERENCES: usize = 16;
/// The largest number of references one project-discovery notice may carry.
pub const MAX_DISCOVERY_REFERENCES: usize = 4;
/// The largest number of earlier messages a decision may claim to have used.
pub const MAX_DECISION_BASIS: usize = 8;
/// The largest number of participants one message may name inside a scope it is published to.
pub const MAX_RECIPIENTS: usize = 16;
/// The largest number of members a scope may be opened with.
pub const MAX_INITIAL_MEMBERS: usize = 8;
/// The largest number of scopes a review policy may withhold until an assessment is committed.
pub const MAX_WITHHELD_SCOPES: usize = 8;
/// The longest salience a single publication may buy, in milliseconds.
pub const MAX_SALIENCE_MS: u64 = 24 * 60 * 60 * 1000;
/// The longest audience grant a single command may issue, in milliseconds from now.
pub const MAX_GRANT_MS: u64 = 24 * 60 * 60 * 1000;
/// The largest read one delivery may return, in bytes. A reader cannot ask for the whole board.
pub const MAX_DELIVERY_BYTES: u64 = 32 * 1024;

/// The identity and size of an inert payload, which is all of a payload this plane ever holds.
///
/// The bytes are hashed by the caller and are not carried into any record, command or fact. A
/// transition therefore decides on a digest it compares and a length it subtracts, and there is no
/// point at which the content could be matched, parsed or obeyed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Payload {
    digest: String,
    bytes: u64,
}

impl Payload {
    /// The identity and length of exactly these bytes. The bytes are read once, to be hashed, and
    /// are not retained.
    pub fn of(bytes: &[u8]) -> Self {
        Self {
            digest: digest_bytes(bytes),
            bytes: bytes.len() as u64,
        }
    }

    /// A payload whose identity and length a caller states, for content already stored elsewhere.
    pub fn stated(digest: impl Into<String>, bytes: u64) -> Self {
        Self {
            digest: digest.into(),
            bytes,
        }
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

pub fn digest_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(crate) fn validate_identifier(kind: &'static str, value: &str) -> Result<(), BoardError> {
    let length = value.chars().count();
    if (1..=MAX_IDENTIFIER_CHARS).contains(&length) {
        Ok(())
    } else {
        Err(BoardError::InvalidIdentifier { kind })
    }
}

pub(crate) fn validate_digest(kind: &'static str, value: &str) -> Result<(), BoardError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(BoardError::InvalidDigest { kind })
    }
}

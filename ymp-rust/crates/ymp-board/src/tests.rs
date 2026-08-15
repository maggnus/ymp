//! The suite for the collaboration plane, and the fixtures every part of it is built from.
//!
//! Six claims are made here, one per module, and each is made with its negative half beside it.
//! `audiences` states who a detailed message reaches and shows that removing the admission check
//! reaches everyone. `inertness` states that a payload changes nothing but the record it is the
//! payload of, and switches on three boards that read the payload to show that the comparison would
//! catch one. `accounting` states what each act costs and what runs out, and shows that removing
//! the charge makes talking free. `observatory` states what a reading distinguishes and refuses a
//! reading that calls an association causal. `refusals` states that a malformed or cross-scope
//! command leaves the board byte for byte as it was. `blinding` states that nothing reaches a
//! reviewer before its own assessment is durable, and that an assessment committed after disclosure
//! never enters the primary comparison.
//!
//! `containment` makes a seventh claim of a different kind: the plane boundary is checked against
//! this crate's own manifest and sources rather than described in prose.

use crate::budget::{Allowance, CommunicationAllowance};
use crate::ledger::BoardLedger;
use crate::protocol::{
    BoardCommand, GrantAudience, InitialMember, OpenScope, Publish, RegisterParticipant,
};
use crate::records::{Audience, MessageKind, Relation, ReviewPolicy, Rights, ScopeKind};
use crate::{Payload, digest_bytes};

mod accounting;
mod audiences;
mod blinding;
mod containment;
mod inertness;
mod observatory;
mod refusals;

pub(crate) const CONTROLLER: &str = "controller";
/// The participant that sponsors every scope in the fixture and funds the others.
pub(crate) const ROOT: &str = "participant-root";
pub(crate) const ALPHA: &str = "participant-alpha";
pub(crate) const BETA: &str = "participant-beta";
pub(crate) const GAMMA: &str = "participant-gamma";
pub(crate) const DELTA: &str = "participant-delta";

pub(crate) const TASK_SCOPE: &str = "task-scope-1";
pub(crate) const OTHER_SCOPE: &str = "task-scope-2";
pub(crate) const REVIEW_SCOPE: &str = "review-scope-1";

pub(crate) const GRANT_EXPIRY: u64 = 10_000;
pub(crate) const SALIENCE_MS: u64 = 5_000;
pub(crate) const READ_LIMIT: u64 = 16 * 1024;

pub(crate) fn digest(tag: &str) -> String {
    digest_bytes(tag.as_bytes())
}

/// What one participant is funded with. Every dimension is finite, and every test that is about
/// running out narrows one of them deliberately.
pub(crate) fn endowment() -> CommunicationAllowance {
    CommunicationAllowance::ZERO
        .with(Allowance::Publications, 64)
        .with(Allowance::SalienceRefreshes, 16)
        .with(Allowance::MembershipGrants, 32)
        .with(Allowance::ActiveMemberships, 32)
        .with(Allowance::PublishedBytes, 1_000_000)
        .with(Allowance::DeliveredBytes, 1_000_000)
}

pub(crate) fn root_total() -> CommunicationAllowance {
    CommunicationAllowance::ZERO
        .with(Allowance::Publications, 1_000)
        .with(Allowance::SalienceRefreshes, 200)
        .with(Allowance::MembershipGrants, 200)
        .with(Allowance::ActiveMemberships, 200)
        .with(Allowance::PublishedBytes, 10_000_000)
        .with(Allowance::DeliveredBytes, 10_000_000)
}

/// A board with a funded root and four funded participants, and no scope yet.
pub(crate) fn board() -> BoardLedger {
    board_with(endowment())
}

pub(crate) fn board_with(endowment: CommunicationAllowance) -> BoardLedger {
    let mut board = BoardLedger::new(CONTROLLER, ROOT, root_total()).expect("a board opens");
    for participant in [ALPHA, BETA, GAMMA, DELTA] {
        board
            .execute(&BoardCommand::RegisterParticipant(RegisterParticipant {
                participant_id: participant.to_owned(),
                sponsor: ROOT.to_owned(),
                endowment,
            }))
            .expect("a participant is funded to talk");
    }
    board
}

/// The fixture every audience claim is made against: one task scope carrying two publishers and one
/// reader, and a second task scope carrying a participant that is a stranger to the first.
pub(crate) fn scoped() -> BoardLedger {
    let mut board = board();
    open_scope(
        &mut board,
        TASK_SCOPE,
        ScopeKind::Task,
        None,
        &[
            (ALPHA, Rights::READ_AND_PUBLISH),
            (BETA, Rights::READ_AND_PUBLISH),
            (DELTA, Rights::READ),
        ],
    );
    open_scope(
        &mut board,
        OTHER_SCOPE,
        ScopeKind::Task,
        None,
        &[(GAMMA, Rights::READ_AND_PUBLISH)],
    );
    board
}

pub(crate) fn grant_id(scope_id: &str, participant: &str) -> String {
    format!("grant-{scope_id}-{participant}")
}

pub(crate) fn open_scope(
    board: &mut BoardLedger,
    scope_id: &str,
    kind: ScopeKind,
    review_policy: Option<ReviewPolicy>,
    members: &[(&str, Rights)],
) {
    board
        .execute(&BoardCommand::OpenScope(open_scope_command(
            scope_id,
            kind,
            review_policy,
            members,
        )))
        .expect("a scope opens");
}

pub(crate) fn open_scope_command(
    scope_id: &str,
    kind: ScopeKind,
    review_policy: Option<ReviewPolicy>,
    members: &[(&str, Rights)],
) -> OpenScope {
    OpenScope {
        controller: CONTROLLER.to_owned(),
        scope_id: scope_id.to_owned(),
        kind,
        sponsor: ROOT.to_owned(),
        review_policy,
        initial_members: members
            .iter()
            .map(|(participant, rights)| InitialMember {
                grant_id: grant_id(scope_id, participant),
                participant: (*participant).to_owned(),
                rights: *rights,
            })
            .collect(),
        member_expires_at: GRANT_EXPIRY,
    }
}

pub(crate) fn grant(
    board: &mut BoardLedger,
    scope_id: &str,
    participant: &str,
    rights: Rights,
    expires_at: u64,
) {
    board
        .execute(&BoardCommand::GrantAudience(GrantAudience {
            grant_id: format!("late-{}", grant_id(scope_id, participant)),
            scope_id: scope_id.to_owned(),
            sponsor: ROOT.to_owned(),
            participant: participant.to_owned(),
            rights,
            expires_at,
        }))
        .expect("a sponsor admits a participant");
}

pub(crate) fn task_audience() -> Audience {
    Audience::Scope {
        scope_id: TASK_SCOPE.to_owned(),
    }
}

/// One ordinary publication: an observation of a stated size, standing on its own.
pub(crate) fn publication(
    message_id: &str,
    author: &str,
    audience: Audience,
    bytes: u64,
) -> Publish {
    Publish {
        message_id: message_id.to_owned(),
        author: author.to_owned(),
        audience,
        kind: MessageKind::Observation,
        payload: Payload::stated(digest(message_id), bytes),
        salience_ms: SALIENCE_MS,
        references: Vec::new(),
        relation: Relation::Standalone,
        claimed_decision_basis: Vec::new(),
    }
}

pub(crate) fn publish(board: &mut BoardLedger, command: Publish) {
    board
        .execute(&BoardCommand::Publish(command))
        .expect("a message is appended");
}

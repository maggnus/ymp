//! What talking costs, what runs out, and what survives expiry.
//!
//! Publishing, refreshing salience, opening an audience and receiving delivered bytes each spend a
//! dimension nothing else pays for, and each of the four runs out on its own. What does not expire
//! is the record: once salience has gone the message is no longer projected to anyone and is still
//! exactly where it was published, which is the difference between a stale coordination cue and
//! evidence of what was said.

use crate::budget::{Allowance, CommunicationAllowance};
use crate::ledger::DisabledChecks;
use crate::protocol::{
    AdvanceClock, BoardCommand, BoardError, GrantAudience, InitialMember, OpenScope, ReadBoard,
    RefreshSalience, RequestAudience,
};
use crate::records::{Rights, ScopeKind};
use crate::tests::{
    ALPHA, BETA, CONTROLLER, GRANT_EXPIRY, READ_LIMIT, ROOT, SALIENCE_MS, TASK_SCOPE, board_with,
    endowment, grant_id, publication, publish, scoped, task_audience,
};

fn held(board: &crate::BoardLedger, participant: &str, allowance: Allowance) -> u64 {
    board
        .allowance(participant)
        .expect("the participant holds an account")
        .get(allowance)
}

/// Every act is charged, and each one is charged in its own dimension.
///
/// The negative half switches off charging. Without it the same sequence leaves every balance where
/// it started, which is a board on which nothing anyone says costs anything.
#[test]
fn publishing_refreshing_admitting_and_receiving_each_spend_their_own_dimension() {
    let mut board = scoped();
    let publications = held(&board, ALPHA, Allowance::Publications);
    let published_bytes = held(&board, ALPHA, Allowance::PublishedBytes);
    publish(&mut board, publication("m-1", ALPHA, task_audience(), 64));
    assert_eq!(
        held(&board, ALPHA, Allowance::Publications),
        publications - 1,
        "a publication did not spend a publication"
    );
    assert_eq!(
        held(&board, ALPHA, Allowance::PublishedBytes),
        published_bytes - 64,
        "a publication did not spend the bytes it published"
    );

    let refreshes = held(&board, ALPHA, Allowance::SalienceRefreshes);
    board
        .execute(&BoardCommand::RefreshSalience(RefreshSalience {
            message_id: "m-1-again".to_owned(),
            refreshes: "m-1".to_owned(),
            author: ALPHA.to_owned(),
            salience_ms: SALIENCE_MS,
        }))
        .expect("salience is bought again");
    assert_eq!(
        held(&board, ALPHA, Allowance::SalienceRefreshes),
        refreshes - 1,
        "keeping a finding salient did not spend a refresh"
    );
    assert_eq!(
        held(&board, ALPHA, Allowance::Publications),
        publications - 2,
        "a refresh is an attributed message and did not spend a publication"
    );

    let grants = held(&board, ROOT, Allowance::MembershipGrants);
    let memberships = held(&board, ROOT, Allowance::ActiveMemberships);
    crate::tests::grant(
        &mut board,
        TASK_SCOPE,
        crate::tests::GAMMA,
        Rights::READ,
        GRANT_EXPIRY,
    );
    assert_eq!(
        held(&board, ROOT, Allowance::MembershipGrants),
        grants - 1,
        "admitting a participant did not spend membership authority"
    );
    assert_eq!(
        held(&board, ROOT, Allowance::ActiveMemberships),
        memberships - 1,
        "admitting a participant did not reserve a concurrent membership"
    );

    let delivered = held(&board, BETA, Allowance::DeliveredBytes);
    let delivery = board
        .deliver(&ReadBoard {
            reader: BETA.to_owned(),
            limit_bytes: READ_LIMIT,
        })
        .expect("a read is answered");
    assert!(delivery.bytes > 0, "the read carried nothing to charge for");
    assert_eq!(
        held(&board, BETA, Allowance::DeliveredBytes),
        delivered - delivery.bytes,
        "a reader was not charged for the bytes made available to it"
    );

    let mut weakened = scoped();
    weakened.disable_checks(DisabledChecks {
        charge: true,
        ..DisabledChecks::default()
    });
    let free = held(&weakened, ALPHA, Allowance::Publications);
    publish(
        &mut weakened,
        publication("m-1", ALPHA, task_audience(), 64),
    );
    assert_eq!(
        held(&weakened, ALPHA, Allowance::Publications),
        free,
        "with charging switched off a publication was still charged, so the charge under test is \
         not what makes talking finite"
    );
}

/// Each of the four runs out, and the refusal names the dimension that refused rather than a
/// general shortage.
#[test]
fn publication_refresh_membership_and_delivery_each_run_out() {
    let mut board = board_with(endowment().with(Allowance::Publications, 0));
    crate::tests::open_scope(
        &mut board,
        TASK_SCOPE,
        ScopeKind::Task,
        None,
        &[(ALPHA, Rights::READ_AND_PUBLISH)],
    );
    assert_eq!(
        board.decide(&BoardCommand::Publish(publication(
            "m-1",
            ALPHA,
            task_audience(),
            8
        ))),
        Err(BoardError::InsufficientAllowance {
            account: ALPHA.to_owned(),
            allowance: Allowance::Publications,
        }),
        "a participant with no publications left was allowed to publish"
    );
    assert_eq!(
        board.decide(&BoardCommand::RequestAudience(RequestAudience {
            request_id: "request-1".to_owned(),
            scope_id: TASK_SCOPE.to_owned(),
            participant: ALPHA.to_owned(),
        })),
        Err(BoardError::InsufficientAllowance {
            account: ALPHA.to_owned(),
            allowance: Allowance::Publications,
        }),
        "asking for admission is a communication act and was not charged as one"
    );

    let mut board = board_with(endowment().with(Allowance::SalienceRefreshes, 0));
    crate::tests::open_scope(
        &mut board,
        TASK_SCOPE,
        ScopeKind::Task,
        None,
        &[(ALPHA, Rights::READ_AND_PUBLISH)],
    );
    publish(&mut board, publication("m-1", ALPHA, task_audience(), 8));
    assert_eq!(
        board.decide(&BoardCommand::RefreshSalience(RefreshSalience {
            message_id: "m-1-again".to_owned(),
            refreshes: "m-1".to_owned(),
            author: ALPHA.to_owned(),
            salience_ms: SALIENCE_MS,
        })),
        Err(BoardError::InsufficientAllowance {
            account: ALPHA.to_owned(),
            allowance: Allowance::SalienceRefreshes,
        }),
        "a participant with no refreshes left kept a finding salient for nothing"
    );

    let board = board_with(endowment().with(Allowance::MembershipGrants, 0));
    let sponsored_by_alpha = OpenScope {
        controller: CONTROLLER.to_owned(),
        scope_id: "alpha-scope".to_owned(),
        kind: ScopeKind::Task,
        sponsor: ALPHA.to_owned(),
        review_policy: None,
        initial_members: vec![InitialMember {
            grant_id: "grant-alpha-scope-beta".to_owned(),
            participant: BETA.to_owned(),
            rights: Rights::READ,
        }],
        member_expires_at: GRANT_EXPIRY,
    };
    assert_eq!(
        board.decide(&BoardCommand::OpenScope(sponsored_by_alpha)),
        Err(BoardError::InsufficientAllowance {
            account: ALPHA.to_owned(),
            allowance: Allowance::MembershipGrants,
        }),
        "a sponsor with no membership authority left still admitted somebody"
    );

    let mut board = board_with(endowment().with(Allowance::DeliveredBytes, 0));
    crate::tests::open_scope(
        &mut board,
        TASK_SCOPE,
        ScopeKind::Task,
        None,
        &[
            (ALPHA, Rights::READ_AND_PUBLISH),
            (BETA, Rights::READ_AND_PUBLISH),
        ],
    );
    publish(&mut board, publication("m-1", ALPHA, task_audience(), 64));
    let delivery = board
        .deliver(&ReadBoard {
            reader: BETA.to_owned(),
            limit_bytes: READ_LIMIT,
        })
        .expect("a read is answered");
    assert!(
        delivery.messages.is_empty() && delivery.bytes == 0,
        "a reader with no delivered bytes left was still handed a payload"
    );
    assert!(
        board.reader(BETA).is_none(),
        "a delivery that carried nothing still moved a cursor"
    );
}

/// Salience expires; the record does not.
#[test]
fn the_audit_record_outlives_the_salience_it_was_published_with() {
    let mut board = scoped();
    publish(&mut board, publication("m-1", ALPHA, task_audience(), 64));
    let digest = board
        .message("m-1")
        .expect("the record stands")
        .payload_digest
        .clone();

    board
        .execute(&BoardCommand::AdvanceClock(AdvanceClock {
            to: SALIENCE_MS,
        }))
        .expect("the clock moves");

    assert!(
        board.active_projection(BETA).is_empty(),
        "a message whose salience expired is still projected to a participant"
    );
    let delivery = board
        .deliver(&ReadBoard {
            reader: BETA.to_owned(),
            limit_bytes: READ_LIMIT,
        })
        .expect("a read is answered");
    assert!(
        delivery.messages.is_empty(),
        "a message whose salience expired was still delivered"
    );

    let recorded = board.message("m-1").expect("the audit record stands");
    assert_eq!(
        recorded.payload_digest, digest,
        "the audit record changed when its salience expired"
    );
    assert_eq!(
        board.audit().len(),
        1,
        "the audit record was dropped when its salience expired"
    );
}

/// A refresh buys salience again as a new attributed message. It rewrites nothing: the earlier
/// record keeps its own author, its own moment and its own expiry.
#[test]
fn a_refresh_buys_salience_again_and_rewrites_nothing() {
    let mut board = scoped();
    publish(&mut board, publication("m-1", ALPHA, task_audience(), 64));
    let original = board.message("m-1").expect("the record stands").clone();

    board
        .execute(&BoardCommand::AdvanceClock(AdvanceClock {
            to: SALIENCE_MS,
        }))
        .expect("the clock moves");
    board
        .execute(&BoardCommand::RefreshSalience(RefreshSalience {
            message_id: "m-1-again".to_owned(),
            refreshes: "m-1".to_owned(),
            author: BETA.to_owned(),
            salience_ms: SALIENCE_MS,
        }))
        .expect("salience is bought again");

    assert_eq!(
        board.message("m-1").expect("the record stands"),
        &original,
        "a refresh rewrote the message it refreshed"
    );
    let refreshed = board.message("m-1-again").expect("the refresh stands");
    assert_eq!(
        refreshed.payload_digest, original.payload_digest,
        "a refresh republished something other than what it refreshed"
    );
    assert_eq!(
        refreshed.author, BETA,
        "a refresh is attributed to whoever paid for it"
    );
    assert_eq!(
        board.active_projection(BETA).len(),
        1,
        "the refreshed message is not the one thing standing in the projection"
    );
}

/// Communication capacity is conserved: every unit is held by an account, held by a live grant, or
/// spent. A grant that ends returns the concurrent membership it held to the account that funded
/// it, and the membership authority it spent never returns.
#[test]
fn capacity_is_conserved_across_admission_release_and_expiry() {
    let mut board = scoped();
    assert!(
        board.conserves_allowance(),
        "the opened board does not add up"
    );

    publish(&mut board, publication("m-1", ALPHA, task_audience(), 64));
    board
        .deliver(&ReadBoard {
            reader: BETA.to_owned(),
            limit_bytes: READ_LIMIT,
        })
        .expect("a read is answered");
    assert!(
        board.conserves_allowance(),
        "talking lost track of capacity"
    );

    let memberships = held(&board, ROOT, Allowance::ActiveMemberships);
    let authority = held(&board, ROOT, Allowance::MembershipGrants);
    board
        .execute(&BoardCommand::LeaveAudience(crate::LeaveAudience {
            grant_id: grant_id(TASK_SCOPE, BETA),
            participant: BETA.to_owned(),
        }))
        .expect("a participant leaves");
    assert_eq!(
        held(&board, ROOT, Allowance::ActiveMemberships),
        memberships + 1,
        "leaving an audience did not return the concurrent membership to the account that funded it"
    );
    assert_eq!(
        held(&board, ROOT, Allowance::MembershipGrants),
        authority,
        "leaving an audience returned the membership authority the admission spent, so a scope \
         could be reopened to the same participant forever"
    );
    assert!(
        board.conserves_allowance(),
        "leaving lost track of capacity"
    );

    board
        .execute(&BoardCommand::AdvanceClock(AdvanceClock {
            to: GRANT_EXPIRY,
        }))
        .expect("the clock moves");
    assert!(
        board
            .grants()
            .all(|grant| !matches!(grant.state, crate::records::GrantState::Live)),
        "a grant survived its own expiry"
    );
    assert!(board.conserves_allowance(), "expiry lost track of capacity");
}

/// An admission that a sponsor cannot fund is refused whole. The scope is not opened half-way, and
/// no member is admitted on credit.
#[test]
fn an_admission_a_sponsor_cannot_fund_opens_nothing() {
    let mut board = board_with(endowment().with(Allowance::ActiveMemberships, 0));
    let command = OpenScope {
        controller: CONTROLLER.to_owned(),
        scope_id: "alpha-scope".to_owned(),
        kind: ScopeKind::Task,
        sponsor: ALPHA.to_owned(),
        review_policy: None,
        initial_members: vec![InitialMember {
            grant_id: "grant-alpha-scope-beta".to_owned(),
            participant: BETA.to_owned(),
            rights: Rights::READ,
        }],
        member_expires_at: GRANT_EXPIRY,
    };
    let before = board.snapshot();
    assert_eq!(
        board.execute(&BoardCommand::OpenScope(command)),
        Err(BoardError::InsufficientAllowance {
            account: ALPHA.to_owned(),
            allowance: Allowance::ActiveMemberships,
        }),
    );
    assert_eq!(
        board.snapshot(),
        before,
        "a refused scope opening changed the board"
    );
}

/// A sponsor admits only to the scope it sponsors.
#[test]
fn only_the_scope_sponsor_admits_to_it() {
    let mut board = scoped();
    assert_eq!(
        board.execute(&BoardCommand::GrantAudience(GrantAudience {
            grant_id: "grant-forged".to_owned(),
            scope_id: TASK_SCOPE.to_owned(),
            sponsor: ALPHA.to_owned(),
            participant: crate::tests::GAMMA.to_owned(),
            rights: Rights::READ,
            expires_at: GRANT_EXPIRY,
        })),
        Err(BoardError::NotAuthorized {
            participant: ALPHA.to_owned()
        }),
        "a member of a scope admitted somebody else to it"
    );
}

/// Nothing mints communication capacity. What a board holds at the end is what it was opened with.
#[test]
fn the_board_holds_no_more_than_it_was_opened_with() {
    let mut board = scoped();
    for index in 0..8 {
        publish(
            &mut board,
            publication(&format!("m-{index}"), ALPHA, task_audience(), 32),
        );
    }
    board
        .deliver(&ReadBoard {
            reader: BETA.to_owned(),
            limit_bytes: READ_LIMIT,
        })
        .expect("a read is answered");
    let total = board
        .allowance(ALPHA)
        .expect("an account")
        .checked_add(&board.allowance(BETA).expect("an account"))
        .expect("no overflow");
    assert!(
        total.get(Allowance::Publications) < 2 * endowment().get(Allowance::Publications),
        "capacity appeared from somewhere"
    );
    assert!(
        board.conserves_allowance(),
        "the board no longer accounts for what it was opened with"
    );
    assert_eq!(
        board.allowance("nobody"),
        None::<CommunicationAllowance>,
        "an account exists for a participant nobody funded"
    );
}

//! The transition suite and the property suite for local commitments.
//!
//! Three kinds of claim are made here. Transition tests state what one command does and what it
//! refuses. Property tests replay generated schedules and assert that no ordering makes the facts
//! and the records disagree, or breaks the funded award count, consent, fencing, ownership or
//! causal accounting. Mutation tests
//! switch off one guarded check at a time and require the property suite to produce a
//! counterexample, so that each check is shown to be load-bearing rather than asserted to be.
//! Three mutations are of another kind: they leave every check in place and corrupt a fact where
//! that fact is generated, which the records then agree with, and require the comparison with the
//! command that issued it to be the check that reports it. One corrupts how much an award moved,
//! one the account it came out of, and one the account a closing contract's escrow went back to.
//!
//! A fourth kind of claim is made about how a run stops. Lifecycle schedules replay one scenario
//! each — a passing query, a rejected candidate, a query that could not be carried out, a
//! contractor with nothing to offer, an allowed stopping policy, a participant that stopped
//! answering, a lease that ran out, an authorized stop, and a participant that keeps asking to be
//! woken — in seeded interleavings, and require every one of them to reach the state its scenario
//! describes with no state repaired by hand. What none of them may reach is acceptance without a
//! passing protected query against the exact root candidate.

use super::budget::{BudgetVector, DIMENSIONS, Dimension, DimensionKind};
use super::invocations::{
    InvocationClosure, InvocationState, OpenAuthority, RootTerminal, StopReason, Verdict,
    WakeCondition,
};
use super::ledger::{AlteredFacts, CommitmentLedger, DisabledChecks};
use super::protocol::{
    AcceptOpen, AdvanceClock, Advertise, Award, CancelContract, CloseInvocation, CommitmentCommand,
    CommitmentError, CommitmentEvent, MAX_ATTEMPT_WAKES, MAX_WAKE_CONDITIONS, Reassign, RecordBid,
    RecordVerification, RenewLease, ResumeInvocation, ReturnObligation, SettleOffer, StartAttempt,
    StartInvocation, StopRun, SubmitResult, WithdrawOffer, YieldInvocation,
};
use super::records::{
    AccountRef, BidState, ContractState, FundingSource, ObligationState, OfferPolicy, OfferState,
    Outcome,
};
use super::schedules::{
    self, ALPHA, BETA, CROSS_OFFER, DEADLINE, FactAccounts, GAMMA, LEASE_MS, LIFE_ATTEMPT,
    LIFE_CONTRACT, LIFE_INVOCATION, LIFE_LEASE_MS, LIFE_OBLIGATION, LIFE_WAKE_DEADLINE,
    MAIN_MAX_AWARDS, MAIN_OFFER, ROOT_OBLIGATION, ROOT_PARTICIPANT, SECOND_OFFER,
    SOLO_FUNDING_CONTRACT, Scenario, Tokens, Violation, advertise_main, award_main, digest,
    life_setup, new_ledger, requested_escrow, run_altered_schedule, run_lifecycle, run_schedule,
    setup, state_violations,
};

/// How many generated schedules the property suite replays. Every seed is a different total order
/// over the same contended pool.
const SCHEDULE_SEEDS: u64 = 192;

fn prepared() -> (CommitmentLedger, Tokens) {
    prepared_with(DisabledChecks::default())
}

fn prepared_with(disabled: DisabledChecks) -> (CommitmentLedger, Tokens) {
    let tokens = Tokens::variant("a");
    let mut ledger = new_ledger();
    ledger.disable_checks(disabled);
    for command in setup(&tokens) {
        ledger.execute(&command).expect("prefix command");
    }
    (ledger, tokens)
}

fn expect_refusal(ledger: &mut CommitmentLedger, command: &CommitmentCommand) -> CommitmentError {
    let before = ledger.clone();
    let error = ledger
        .execute(command)
        .expect_err("the command must be refused");
    assert_eq!(
        *ledger, before,
        "a refused command left the ledger changed: {error}"
    );
    error
}

// ---------------------------------------------------------------------------------------------
// Transition tests
// ---------------------------------------------------------------------------------------------

#[test]
fn a_compatible_offer_and_bid_form_one_contract_escrow_lease_and_obligation_together() {
    let (mut ledger, _tokens) = prepared();
    let sponsor_before = ledger.participants()[ROOT_PARTICIPANT].balance;
    let offer_escrow_before = ledger.offers()[MAIN_OFFER].escrow;

    let events = ledger
        .execute(&award_main("bid-alpha", "alpha"))
        .expect("a compatible award");

    let contract = &ledger.contracts()["contract-alpha"];
    assert_eq!(contract.sponsor, ROOT_PARTICIPANT);
    assert_eq!(contract.contractor, ALPHA);
    assert_eq!(contract.state, ContractState::Active);
    assert_eq!(contract.lease.generation, 1);
    assert_eq!(contract.lease.holder, ALPHA);
    assert_eq!(contract.lease.expires_at, LEASE_MS);
    assert_eq!(ledger.bids()["bid-alpha"].state, BidState::Awarded);
    assert_eq!(ledger.offers()[MAIN_OFFER].awards_made, 1);

    let obligation = &ledger.obligations()["obligation-alpha"];
    assert_eq!(obligation.parent.as_deref(), Some(ROOT_OBLIGATION));
    assert_eq!(obligation.owner, ALPHA);
    assert_eq!(obligation.state, ObligationState::Active);
    assert!(
        ledger.obligations()[ROOT_OBLIGATION]
            .children
            .contains(&"obligation-alpha".to_owned())
    );

    // The escrow moved; it was not created. The offer pool lost exactly what the contract gained,
    // less the wall time the first lease consumed.
    let escrow = requested_escrow();
    assert_eq!(
        ledger.offers()[MAIN_OFFER].escrow,
        offer_escrow_before.checked_sub(&escrow).expect("pool")
    );
    assert_eq!(
        contract.escrow,
        escrow
            .checked_sub(&BudgetVector::units(Dimension::WallTimeMs, LEASE_MS))
            .expect("first lease is funded from the transferred escrow")
    );
    // Creating the obligation cost the sponsor one unit of creation authority, and cost it
    // nothing else.
    assert_eq!(
        ledger.participants()[ROOT_PARTICIPANT].balance,
        sponsor_before
            .checked_sub(&BudgetVector::unit(Dimension::ObligationCreations))
            .expect("authority")
    );
    assert_eq!(
        ledger.consumed().get(Dimension::ObligationCreations),
        1,
        "spent creation authority is gone, not parked"
    );

    // One command, one set of facts.
    assert!(
        events
            .iter()
            .any(|event| matches!(event, CommitmentEvent::TaskContractFormed { .. }))
    );
    assert!(events.iter().any(|event| matches!(
        event,
        CommitmentEvent::BudgetTransferred {
            to: AccountRef::TaskContract { .. },
            ..
        }
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, CommitmentEvent::LeaseIssued { generation: 1, .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, CommitmentEvent::ObligationCreated { .. }))
    );
    assert!(state_violations(&ledger).is_empty());
}

#[test]
fn an_incompatible_or_expired_record_forms_nothing_at_all() {
    let (mut ledger, tokens) = prepared();

    // Consent for another artifact class.
    ledger
        .execute(&CommitmentCommand::RecordBid(RecordBid {
            bid_id: "bid-other-class".to_owned(),
            offer_id: MAIN_OFFER.to_owned(),
            bidder: BETA.to_owned(),
            requested_escrow: requested_escrow(),
            artifact_class: "class-elsewhere".to_owned(),
            proposal_digest: None,
            expires_at: DEADLINE,
        }))
        .expect("a bid may name any class; only forming a contract requires equality");
    assert!(matches!(
        expect_refusal(&mut ledger, &award_main("bid-other-class", "other")),
        CommitmentError::ArtifactClassMismatch { .. }
    ));

    // A counter-offer above what the offer funds per award.
    ledger
        .execute(&CommitmentCommand::RecordBid(RecordBid {
            bid_id: "bid-too-large".to_owned(),
            offer_id: MAIN_OFFER.to_owned(),
            bidder: BETA.to_owned(),
            requested_escrow: requested_escrow().with(Dimension::MoneyMicros, 900_000),
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: None,
            expires_at: DEADLINE,
        }))
        .expect("recorded consent");
    assert!(matches!(
        expect_refusal(&mut ledger, &award_main("bid-too-large", "large")),
        CommitmentError::EscrowExceedsOffer {
            dimension: Dimension::MoneyMicros,
            ..
        }
    ));

    // Consent that has run out.
    ledger
        .execute(&CommitmentCommand::RecordBid(RecordBid {
            bid_id: "bid-short".to_owned(),
            offer_id: MAIN_OFFER.to_owned(),
            bidder: BETA.to_owned(),
            requested_escrow: requested_escrow(),
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: None,
            expires_at: 10,
        }))
        .expect("recorded consent");
    ledger
        .execute(&CommitmentCommand::AdvanceClock(AdvanceClock { to: 11 }))
        .expect("the clock moves forward");
    assert!(matches!(
        expect_refusal(&mut ledger, &award_main("bid-short", "short")),
        CommitmentError::BidExpired { .. }
    ));

    // An offer that has run out.
    ledger
        .execute(&CommitmentCommand::AdvanceClock(AdvanceClock {
            to: DEADLINE + 1,
        }))
        .expect("the clock moves forward");
    assert!(matches!(
        expect_refusal(&mut ledger, &award_main("bid-alpha", "alpha")),
        CommitmentError::OfferExpired { .. }
    ));

    assert!(ledger.contracts().is_empty(), "no contract was formed");
    assert!(
        ledger.obligations().len() == 1,
        "no child obligation was created"
    );
    assert!(state_violations(&ledger).is_empty());
}

#[test]
fn an_award_beyond_the_funded_count_is_refused_while_escrow_still_has_slack() {
    let (mut ledger, _tokens) = prepared();
    ledger
        .execute(&award_main("bid-alpha", "alpha"))
        .expect("first funded slot");
    ledger
        .execute(&award_main("bid-beta", "beta"))
        .expect("second funded slot");
    // The pool still holds money, tokens and authority for another award. Only the funded count
    // stops it.
    assert!(
        ledger.offers()[MAIN_OFFER]
            .escrow
            .covers(&requested_escrow()),
        "the refusal must not rest on arithmetic"
    );
    assert!(matches!(
        expect_refusal(&mut ledger, &award_main("bid-gamma", "gamma")),
        CommitmentError::AwardsExhausted {
            max_awards: MAIN_MAX_AWARDS,
            ..
        }
    ));
}

#[test]
fn an_open_offer_serializes_concurrent_acceptance_to_the_count_it_funded() {
    let (mut ledger, tokens) = prepared();
    let accept = |participant: &str, suffix: &str| {
        CommitmentCommand::AcceptOpen(AcceptOpen {
            contract_id: format!("contract-open-{suffix}"),
            obligation_id: format!("obligation-open-{suffix}"),
            lease_id: format!("lease-open-{suffix}"),
            bid_id: format!("bid-open-{suffix}"),
            offer_id: "offer-open".to_owned(),
            participant: participant.to_owned(),
            requested_escrow: requested_escrow(),
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: None,
            lease_ms: LEASE_MS,
        })
    };
    ledger
        .execute(&accept(ALPHA, "a"))
        .expect("first acceptance");
    assert!(matches!(
        expect_refusal(&mut ledger, &accept(BETA, "b")),
        CommitmentError::AwardsExhausted { .. }
    ));
    assert_eq!(ledger.offers()["offer-open"].awards_made, 1);
    assert_eq!(
        ledger.contracts()["contract-open-a"].contractor,
        ALPHA,
        "the acceptance that was serialized first is the one that formed a contract"
    );
}

#[test]
fn a_stale_fencing_generation_cannot_submit_renew_or_close_after_reassignment() {
    let (mut ledger, _tokens) = prepared();
    ledger
        .execute(&award_main("bid-alpha", "alpha"))
        .expect("award");
    ledger
        .execute(&CommitmentCommand::StartAttempt(StartAttempt {
            attempt_id: "attempt-1".to_owned(),
            contract_id: "contract-alpha".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
        }))
        .expect("attempt under the current generation");
    ledger
        .execute(&CommitmentCommand::AdvanceClock(AdvanceClock { to: 400 }))
        .expect("the lease runs out");
    ledger
        .execute(&CommitmentCommand::Reassign(Reassign {
            contract_id: "contract-alpha".to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            bid_id: "bid-alpha-spare".to_owned(),
            lease_id: "lease-alpha-2".to_owned(),
            lease_ms: LEASE_MS,
        }))
        .expect("the next generation is issued against live consent");
    assert_eq!(ledger.contracts()["contract-alpha"].lease.generation, 2);

    // The holder is unchanged here on purpose: the refusals below rest on the fencing token and on
    // nothing else.
    for (label, command) in [
        (
            "submit",
            CommitmentCommand::SubmitResult(SubmitResult {
                contract_id: "contract-alpha".to_owned(),
                attempt_id: "attempt-1".to_owned(),
                participant: ALPHA.to_owned(),
                generation: 1,
                candidate_digest: digest("stale-candidate"),
            }),
        ),
        (
            "renew",
            CommitmentCommand::RenewLease(RenewLease {
                contract_id: "contract-alpha".to_owned(),
                holder: ALPHA.to_owned(),
                generation: 1,
                lease_ms: LEASE_MS,
            }),
        ),
        (
            "return",
            CommitmentCommand::ReturnObligation(ReturnObligation {
                contract_id: "contract-alpha".to_owned(),
                participant: ALPHA.to_owned(),
                generation: 1,
                outcome: Outcome::Result {
                    candidate_digest: digest("stale-candidate"),
                },
            }),
        ),
    ] {
        assert!(
            matches!(
                expect_refusal(&mut ledger, &command),
                CommitmentError::StaleGeneration {
                    seen: 1,
                    current: 2,
                    ..
                }
            ),
            "a stale generation was accepted for {label}"
        );
    }

    // An attempt opened under the old generation cannot carry a result into the new one either.
    ledger
        .execute(&CommitmentCommand::SubmitResult(SubmitResult {
            contract_id: "contract-alpha".to_owned(),
            attempt_id: "attempt-1".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 2,
            candidate_digest: digest("stale-candidate"),
        }))
        .expect_err("an attempt from the previous generation may not submit");
    assert!(
        ledger.contracts()["contract-alpha"]
            .candidate_digest
            .is_none()
    );
    assert_eq!(
        ledger.obligations()["obligation-alpha"].state,
        ObligationState::Active
    );
}

#[test]
fn a_holder_at_the_current_generation_submits_and_closes_its_obligation() {
    let (mut ledger, _tokens) = prepared();
    ledger
        .execute(&award_main("bid-alpha", "alpha"))
        .expect("award");
    ledger
        .execute(&CommitmentCommand::StartAttempt(StartAttempt {
            attempt_id: "attempt-1".to_owned(),
            contract_id: "contract-alpha".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
        }))
        .expect("attempt");
    ledger
        .execute(&CommitmentCommand::SubmitResult(SubmitResult {
            contract_id: "contract-alpha".to_owned(),
            attempt_id: "attempt-1".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            candidate_digest: digest("candidate"),
        }))
        .expect("submission under the current generation");
    ledger
        .execute(&CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: "contract-alpha".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            outcome: Outcome::Result {
                candidate_digest: digest("candidate"),
            },
        }))
        .expect("a closing return");

    let obligation = &ledger.obligations()["obligation-alpha"];
    assert_eq!(obligation.state, ObligationState::Terminal);
    assert_eq!(
        obligation.outcome,
        Some(Outcome::Result {
            candidate_digest: digest("candidate")
        })
    );
    assert_eq!(
        ledger.contracts()["contract-alpha"].state,
        ContractState::Returned
    );
    assert!(
        ledger.contracts()["contract-alpha"].escrow.is_zero(),
        "unspent escrow settled back to the account that funded the offer"
    );
    assert!(state_violations(&ledger).is_empty());
}

#[test]
fn an_obligation_cannot_close_while_the_work_below_it_is_outstanding() {
    let (mut ledger, tokens) = prepared();
    ledger
        .execute(&award_main("bid-alpha", "alpha"))
        .expect("award");
    ledger
        .execute(&CommitmentCommand::Advertise(Advertise {
            offer_id: "offer-child".to_owned(),
            sponsor: ALPHA.to_owned(),
            parent_obligation: "obligation-alpha".to_owned(),
            funding_source: FundingSource::TaskContract {
                contract_id: "contract-alpha".to_owned(),
            },
            task_scope: tokens.task_scope.clone(),
            base_digest: tokens.base_digest.clone(),
            intent_digest: tokens.intent_digest.clone(),
            artifact_class: tokens.artifact_class.clone(),
            dependencies: Vec::new(),
            capability_scope: Vec::new(),
            execution_escrow: BudgetVector::ZERO
                .with(Dimension::MoneyMicros, 2_000)
                .with(Dimension::WallTimeMs, 2_000),
            policy: OfferPolicy::Negotiated,
            bid_deadline: DEADLINE,
            offer_deadline: DEADLINE,
            max_awards: 1,
        }))
        .expect("a contractor may delegate from the escrow it holds");

    // An unsettled offer is outstanding work: it can still spend.
    let closing = CommitmentCommand::ReturnObligation(ReturnObligation {
        contract_id: "contract-alpha".to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        outcome: Outcome::DeadEnd,
    });
    assert!(matches!(
        expect_refusal(&mut ledger, &closing),
        CommitmentError::OfferUnsettled { .. }
    ));

    ledger
        .execute(&CommitmentCommand::RecordBid(RecordBid {
            bid_id: "bid-child".to_owned(),
            offer_id: "offer-child".to_owned(),
            bidder: BETA.to_owned(),
            requested_escrow: BudgetVector::ZERO
                .with(Dimension::MoneyMicros, 1_000)
                .with(Dimension::WallTimeMs, 1_000),
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: None,
            expires_at: DEADLINE,
        }))
        .expect("consent");
    ledger
        .execute(&CommitmentCommand::Award(Award {
            contract_id: "contract-child".to_owned(),
            obligation_id: "obligation-child".to_owned(),
            lease_id: "lease-child".to_owned(),
            offer_id: "offer-child".to_owned(),
            bid_id: "bid-child".to_owned(),
            sponsor: ALPHA.to_owned(),
            lease_ms: LEASE_MS,
        }))
        .expect("a child obligation with a causal parent");
    assert_eq!(
        ledger.obligations()["obligation-child"].parent.as_deref(),
        Some("obligation-alpha")
    );

    assert!(matches!(
        expect_refusal(&mut ledger, &closing),
        CommitmentError::DescendantOutstanding { .. }
    ));

    ledger
        .execute(&CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: "contract-child".to_owned(),
            participant: BETA.to_owned(),
            generation: 1,
            outcome: Outcome::Declined,
        }))
        .expect("the leaf closes first");
    ledger
        .execute(&CommitmentCommand::WithdrawOffer(
            super::protocol::WithdrawOffer {
                offer_id: "offer-child".to_owned(),
                sponsor: ALPHA.to_owned(),
            },
        ))
        .expect("withdraw");
    ledger
        .execute(&CommitmentCommand::SettleOffer(
            super::protocol::SettleOffer {
                offer_id: "offer-child".to_owned(),
                sponsor: ALPHA.to_owned(),
            },
        ))
        .expect("settle");
    ledger.execute(&closing).expect("now the parent may close");
    assert_eq!(
        ledger.obligations()["obligation-alpha"].state,
        ObligationState::Terminal
    );
    assert!(state_violations(&ledger).is_empty());
}

#[test]
fn a_dimension_is_never_paid_for_out_of_another() {
    let (mut ledger, tokens) = prepared();
    // Consent asking for one protected verification query more than the offer funds per award,
    // while asking for far less of everything else.
    let escrow = requested_escrow()
        .with(Dimension::MoneyMicros, 1)
        .with(Dimension::VerificationQueries, 3);
    ledger
        .execute(&CommitmentCommand::RecordBid(RecordBid {
            bid_id: "bid-queries".to_owned(),
            offer_id: MAIN_OFFER.to_owned(),
            bidder: BETA.to_owned(),
            requested_escrow: escrow,
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: None,
            expires_at: DEADLINE,
        }))
        .expect("consent");
    assert!(matches!(
        expect_refusal(&mut ledger, &award_main("bid-queries", "queries")),
        CommitmentError::EscrowExceedsOffer {
            dimension: Dimension::VerificationQueries,
            ..
        }
    ));

    let spare = BudgetVector::ZERO.with(Dimension::MoneyMicros, u64::MAX);
    assert!(
        spare
            .checked_sub(&BudgetVector::unit(Dimension::AttemptStarts))
            .is_err(),
        "money does not buy an attempt start"
    );
    assert_eq!(
        DIMENSIONS
            .into_iter()
            .filter(|dimension| dimension.kind() == DimensionKind::CreationAuthority)
            .count(),
        5,
        "offer, obligation, participant, attempt and process-slice creation are the authority \
         dimensions"
    );
}

#[test]
fn the_kernel_never_chooses_between_two_live_bids() {
    let (mut ledger, tokens) = prepared();
    // The same offer, two live consents, one asking for strictly more than the other. A kernel
    // that preferred the cheaper, the earlier or the larger would show it here.
    ledger
        .execute(&CommitmentCommand::RecordBid(RecordBid {
            bid_id: "bid-expensive".to_owned(),
            offer_id: MAIN_OFFER.to_owned(),
            bidder: GAMMA.to_owned(),
            requested_escrow: requested_escrow().with(Dimension::MoneyMicros, 19_000),
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: None,
            expires_at: DEADLINE,
        }))
        .expect("consent");
    ledger
        .execute(&award_main("bid-expensive", "expensive"))
        .expect("the sponsor's own choice is the only input");
    assert_eq!(
        ledger.contracts()["contract-expensive"].bid_id,
        "bid-expensive"
    );
    assert_eq!(ledger.bids()["bid-alpha"].state, BidState::Live);

    // There is no command that forms a contract without naming one exact consent record: every
    // formation path carries a bid identifier supplied by its issuer.
    let award = CommitmentCommand::Award(Award {
        contract_id: "contract-nameless".to_owned(),
        obligation_id: "obligation-nameless".to_owned(),
        lease_id: "lease-nameless".to_owned(),
        offer_id: MAIN_OFFER.to_owned(),
        bid_id: "bid-that-does-not-exist".to_owned(),
        sponsor: ROOT_PARTICIPANT.to_owned(),
        lease_ms: LEASE_MS,
    });
    assert!(matches!(
        expect_refusal(&mut ledger, &award),
        CommitmentError::Unknown { kind: "bid", .. }
    ));
}

#[test]
fn only_the_sponsor_awards_and_only_the_holder_advances() {
    let (mut ledger, _tokens) = prepared();
    let mut stolen = award_main("bid-alpha", "alpha");
    if let CommitmentCommand::Award(command) = &mut stolen {
        command.sponsor = BETA.to_owned();
    }
    assert!(matches!(
        expect_refusal(&mut ledger, &stolen),
        CommitmentError::NotAuthorized { .. }
    ));

    ledger
        .execute(&award_main("bid-alpha", "alpha"))
        .expect("award");
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &CommitmentCommand::RenewLease(RenewLease {
                contract_id: "contract-alpha".to_owned(),
                holder: BETA.to_owned(),
                generation: 1,
                lease_ms: LEASE_MS,
            })
        ),
        CommitmentError::NotAuthorized { .. }
    ));
}

#[test]
fn a_targeted_offer_still_requires_consent_and_admits_only_its_target() {
    let tokens = Tokens::variant("a");
    let mut ledger = new_ledger();
    for command in setup(&tokens) {
        // Re-run the prefix but replace the main offer with a targeted one.
        let command = match &command {
            CommitmentCommand::Advertise(advertise) if advertise.offer_id == MAIN_OFFER => {
                advertise_main(
                    &tokens,
                    OfferPolicy::Targeted {
                        participant_id: ALPHA.to_owned(),
                    },
                )
            }
            other => other.clone(),
        };
        match &command {
            CommitmentCommand::RecordBid(bid) if bid.bidder != ALPHA => {
                assert!(matches!(
                    expect_refusal(&mut ledger, &command),
                    CommitmentError::TargetMismatch { .. }
                ));
            }
            _ => {
                ledger.execute(&command).expect("prefix command");
            }
        }
    }
    // Naming a participant does not create an obligation for it: without consent there is no
    // contract at all.
    assert!(ledger.contracts().is_empty());
    ledger
        .execute(&award_main("bid-alpha", "alpha"))
        .expect("the target consented, so a contract may form");
    assert_eq!(ledger.contracts()["contract-alpha"].contractor, ALPHA);
}

#[test]
fn withdrawal_and_expiry_settle_reservations_and_create_no_obligation() {
    let (mut ledger, _tokens) = prepared();
    let sponsor_before = ledger.participants()[ROOT_PARTICIPANT].balance;
    ledger
        .execute(&CommitmentCommand::WithdrawOffer(
            super::protocol::WithdrawOffer {
                offer_id: MAIN_OFFER.to_owned(),
                sponsor: ROOT_PARTICIPANT.to_owned(),
            },
        ))
        .expect("withdraw");
    assert!(matches!(
        expect_refusal(&mut ledger, &award_main("bid-alpha", "alpha")),
        CommitmentError::OfferNotOpen { .. }
    ));
    ledger
        .execute(&CommitmentCommand::SettleOffer(
            super::protocol::SettleOffer {
                offer_id: MAIN_OFFER.to_owned(),
                sponsor: ROOT_PARTICIPANT.to_owned(),
            },
        ))
        .expect("settle");
    assert_eq!(ledger.offers()[MAIN_OFFER].state, OfferState::Settled);
    assert!(ledger.offers()[MAIN_OFFER].escrow.is_zero());
    assert!(ledger.contracts().is_empty());
    // Capacity came back; the offer-creation unit did not.
    let after = ledger.participants()[ROOT_PARTICIPANT].balance;
    for dimension in DIMENSIONS {
        let expected = sponsor_before.get(dimension)
            + ledger.offers()[MAIN_OFFER].execution_escrow.get(dimension)
                * u64::from(MAIN_MAX_AWARDS);
        assert_eq!(
            after.get(dimension),
            expected,
            "{dimension} settled wrongly"
        );
    }
    assert_eq!(ledger.consumed().get(Dimension::OfferCreations), 2);
    assert!(state_violations(&ledger).is_empty());
}

/// An advertisement reserving a thousand times what its sponsor holds, first refused because the
/// capacity is not there, and then — with that refusal switched off — refused again when the facts
/// it produced reach the accounts.
fn oversized_offer(tokens: &Tokens) -> CommitmentCommand {
    CommitmentCommand::Advertise(Advertise {
        offer_id: "offer-oversized".to_owned(),
        sponsor: ROOT_PARTICIPANT.to_owned(),
        parent_obligation: ROOT_OBLIGATION.to_owned(),
        funding_source: FundingSource::Participant,
        task_scope: tokens.task_scope.clone(),
        base_digest: tokens.base_digest.clone(),
        intent_digest: tokens.intent_digest.clone(),
        artifact_class: tokens.artifact_class.clone(),
        dependencies: Vec::new(),
        capability_scope: Vec::new(),
        execution_escrow: BudgetVector::ZERO.with(Dimension::MoneyMicros, 1_000_000_000),
        policy: OfferPolicy::Negotiated,
        bid_deadline: DEADLINE,
        offer_deadline: DEADLINE,
        max_awards: 1,
    })
}

/// A debit that cannot be covered is refused and says so, rather than being dropped while the fact
/// that produced it goes on claiming the capacity moved.
///
/// Under every check the kernel enforces this state is unreachable, which is the point of the
/// covering check. It is reached here by switching that check off, because a fact the accounts
/// cannot pay for must fail loudly wherever it comes from — including from a check that is one day
/// weakened or missed.
#[test]
fn a_debit_the_accounts_cannot_cover_is_refused_and_named_in_the_result() {
    let (mut ledger, tokens) = prepared();
    let held = ledger.participants()[ROOT_PARTICIPANT]
        .balance
        .get(Dimension::MoneyMicros);
    assert!(
        held < 1_000_000_000,
        "the offer must ask for more than exists"
    );
    assert!(matches!(
        expect_refusal(&mut ledger, &oversized_offer(&tokens)),
        CommitmentError::InsufficientBudget {
            dimension: Dimension::MoneyMicros,
            ..
        }
    ));

    let (mut weakened, tokens) = prepared_with(DisabledChecks {
        covering: true,
        ..DisabledChecks::default()
    });
    let refusal = expect_refusal(&mut weakened, &oversized_offer(&tokens));
    assert!(
        matches!(
            &refusal,
            CommitmentError::UncoveredDebit {
                account,
                dimension: Dimension::MoneyMicros,
            } if account.contains(ROOT_PARTICIPANT)
        ),
        "an impossible debit was not named in the result: {refusal}"
    );
    // The command was decided, so its facts exist and say a billion units left the sponsor. None of
    // them were committed: the offer does not exist, and no account moved.
    assert!(!weakened.offers().contains_key("offer-oversized"));
    let decided = weakened
        .decide(&oversized_offer(&tokens))
        .expect("the weakened kernel still decides the command");
    assert!(decided.iter().any(|event| matches!(
        event,
        CommitmentEvent::BudgetTransferred { amount, .. }
            if amount.get(Dimension::MoneyMicros) == 1_000_000_000
    )));
    assert_eq!(
        weakened.participants()[ROOT_PARTICIPANT]
            .balance
            .get(Dimension::MoneyMicros),
        held
    );
}

/// The projection is answerable for what it claims: a stream that no longer describes what was
/// committed parts company with the records, and a faithful one does not.
///
/// Nothing is asserted here about the projection adding up to the budget the run began with. It
/// always does: every fact that moves capacity takes it out of one account and puts the same
/// quantity into another, so that sum is the opening budget for any stream of facts at all,
/// including this altered one. What can be false is the comparison with the registry.
#[test]
fn a_fact_stream_that_no_longer_describes_what_was_committed_parts_from_the_registry() {
    let tokens = Tokens::variant("a");
    let mut ledger = new_ledger();
    let mut accounts = FactAccounts::opening(ROOT_PARTICIPANT, *ledger.initial_total());
    for command in setup(&tokens) {
        let events = ledger.execute(&command).expect("prefix command");
        accounts.observe(&events);
    }
    assert!(
        accounts.divergence(&ledger).is_empty(),
        "the facts as they were committed describe the accounts the kernel keeps"
    );

    // The same award, recorded as having moved one unit of money more than it did. The registry
    // holds what was actually committed, so the two now disagree about the two accounts the
    // altered fact names.
    let events = ledger
        .execute(&award_main("bid-alpha", "alpha"))
        .expect("award");
    let altered: Vec<CommitmentEvent> = events
        .iter()
        .map(|event| match event {
            CommitmentEvent::BudgetTransferred { from, to, amount } => {
                CommitmentEvent::BudgetTransferred {
                    from: from.clone(),
                    to: to.clone(),
                    amount: amount
                        .checked_add(&BudgetVector::unit(Dimension::MoneyMicros))
                        .expect("one unit more"),
                }
            }
            other => other.clone(),
        })
        .collect();
    assert_ne!(
        altered, events,
        "the stream must actually have been altered"
    );
    accounts.observe(&altered);

    let reported = accounts.divergence(&ledger);
    assert!(
        reported.iter().any(|violation| matches!(
            violation,
            Violation::RegistryDivergence {
                dimension: Dimension::MoneyMicros,
                ..
            }
        )),
        "an altered stream went unreported: {reported:?}"
    );
}

/// An escrow transfer that states less than the command reserved is caught by the command it came
/// from, and by nothing else.
///
/// This is the measurement this card was opened on. The corruption is applied where the fact is
/// generated, so every record the kernel keeps is built by applying the corrupted fact and agrees
/// with it: the projection, the comparison with the registry and every structural invariant stay
/// silent, and the whole run stays internally consistent. What parts company with it is the consent
/// the award named, which the schedule issued and the kernel never wrote.
#[test]
fn an_escrow_transfer_smaller_than_its_command_is_caught_by_that_command_alone() {
    let tokens = Tokens::variant("a");
    let understated = AlteredFacts {
        understated_escrow: true,
        ..AlteredFacts::default()
    };
    assert!(
        run_altered_schedule(
            0,
            &tokens,
            DisabledChecks::default(),
            AlteredFacts::default()
        )
        .violations
        .is_empty(),
        "the same seed is clean while the facts state what was commanded"
    );

    for seed in 0..SCHEDULE_SEEDS {
        let report = run_altered_schedule(seed, &tokens, DisabledChecks::default(), understated);
        let (against_the_command, elsewhere): (Vec<&Violation>, Vec<&Violation>) = report
            .violations
            .iter()
            .partition(|violation| matches!(violation, Violation::FactContradictsCommand { .. }));
        assert!(
            against_the_command.iter().any(|violation| matches!(
                violation,
                Violation::FactContradictsCommand {
                    dimension: Dimension::MoneyMicros,
                    commanded,
                    emitted,
                    ..
                } if *commanded == emitted + 1
            )),
            "seed {seed} committed an understated escrow transfer unreported: {:?}",
            report.violations
        );
        assert!(
            elsewhere.is_empty(),
            "seed {seed}: a fact corrupted at generation must be invisible to every check that \
             compares facts only with each other, or this test proves nothing about which check \
             caught it: {elsewhere:?}"
        );
    }
}

/// An escrow transfer that carries exactly the commanded amount into exactly the commanded account,
/// while taking it out of an account the command never named, is caught by the command it came
/// from, and by nothing else.
///
/// Everything about this transfer except its source is what the award asked for. The money reaches
/// the new contract, the registry is built by applying the same fact and therefore agrees with it,
/// and no total anywhere is short: what the sponsor paid twice over, the offer still holds and
/// returns when it settles. A comparison that reads only how much moved has nothing to say about
/// it, which is why the accounts a command names are compared as well as the quantities.
#[test]
fn an_escrow_transfer_out_of_an_unnamed_account_is_caught_by_that_command_alone() {
    let tokens = Tokens::variant("a");
    let redirected = AlteredFacts {
        redirected_escrow: true,
        ..AlteredFacts::default()
    };
    for seed in 0..SCHEDULE_SEEDS {
        let report = run_altered_schedule(seed, &tokens, DisabledChecks::default(), redirected);
        let (against_the_command, elsewhere): (Vec<&Violation>, Vec<&Violation>) = report
            .violations
            .iter()
            .partition(|violation| matches!(violation, Violation::FactContradictsCommand { .. }));
        // The offer the award named gave up nothing, and an account it never named gave up the
        // whole escrow. Both halves are reported, and both name the award that issued the fact.
        assert!(
            against_the_command.iter().any(|violation| matches!(
                violation,
                Violation::FactContradictsCommand {
                    command: "award" | "accept_open",
                    subject,
                    dimension: Dimension::MoneyMicros,
                    commanded,
                    emitted: 0,
                } if subject.contains("offer") && *commanded > 0
            )),
            "seed {seed} took escrow out of an account the award never named, unreported: {:?}",
            report.violations
        );
        assert!(
            against_the_command.iter().any(|violation| matches!(
                violation,
                Violation::FactContradictsCommand {
                    command: "award" | "accept_open",
                    subject,
                    dimension: Dimension::MoneyMicros,
                    commanded: 0,
                    emitted,
                } if subject.contains("participant") && *emitted > 0
            )),
            "seed {seed} moved money through an account no command named, unreported: {:?}",
            report.violations
        );
        assert!(
            elsewhere.is_empty(),
            "seed {seed}: a fact corrupted at generation must be invisible to every check that \
             compares facts only with each other, or this test proves nothing about which check \
             caught it: {elsewhere:?}"
        );
    }
}

/// The same for the other end of a movement, on the path the comparison used to say nothing about
/// at all: what a closing task contract still holds goes back to the participant that was executing
/// it instead of to the account that funded the offer.
///
/// The quantity is a remainder rather than anything the return command states, so it is the
/// destination account alone that can be wrong here, and the destination alone that is compared.
/// The contract is emptied either way, every account the facts describe still agrees with every
/// account the kernel keeps, and nothing structural notices — the money simply belongs to somebody
/// else afterwards.
#[test]
fn escrow_returned_to_the_wrong_account_is_caught_by_the_command_that_closed_the_contract() {
    let tokens = Tokens::variant("a");
    let misdirected = AlteredFacts {
        misdirected_settlement: true,
        ..AlteredFacts::default()
    };
    let (seed, violations) = (0..SCHEDULE_SEEDS)
        .find_map(|seed| {
            let report =
                run_altered_schedule(seed, &tokens, DisabledChecks::default(), misdirected);
            (!report.violations.is_empty()).then_some((seed, report.violations))
        })
        .expect("a settlement that reaches the wrong account must be reported");
    assert!(
        violations.iter().any(|violation| matches!(
            violation,
            Violation::FactContradictsCommand {
                command: "return_obligation" | "cancel_contract",
                ..
            }
        )),
        "seed {seed} did not report the command that closed the contract: {violations:?}"
    );
    let elsewhere: Vec<&Violation> = violations
        .iter()
        .filter(|violation| !matches!(violation, Violation::FactContradictsCommand { .. }))
        .collect();
    assert!(
        elsewhere.is_empty(),
        "seed {seed}: a destination corrupted at generation must be invisible to every check that \
         compares facts only with each other, or this test proves nothing about which check caught \
         it: {elsewhere:?}"
    );
}

/// The covering check is falsifiable: without it the schedules reach a command whose facts move
/// capacity no account ever held.
#[test]
fn removing_the_covering_check_produces_a_counterexample() {
    let (seed, violations) = first_counterexample(DisabledChecks {
        covering: true,
        ..DisabledChecks::default()
    })
    .expect("the covering check must be load-bearing");
    assert!(
        violations
            .iter()
            .any(|violation| matches!(violation, Violation::UncoveredDebit { .. })),
        "seed {seed} produced {violations:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Only the participant holding the lease closes the work
// ---------------------------------------------------------------------------------------------

/// A contract that has changed hands, closed by the participant that now holds it and by nobody
/// else: not by the participant it was taken from, and not by one that never held it. Both present
/// the generation that is current, so the fencing token has nothing to say and the holder rule is
/// the only thing standing between them and somebody else's obligation.
#[test]
fn a_participant_that_does_not_hold_the_lease_cannot_close_the_obligation() {
    let (mut ledger, _tokens) = prepared();
    ledger
        .execute(&award_main("bid-alpha", "alpha"))
        .expect("award");
    ledger
        .execute(&CommitmentCommand::AdvanceClock(AdvanceClock { to: 400 }))
        .expect("the lease runs out");
    ledger
        .execute(&CommitmentCommand::Reassign(Reassign {
            contract_id: "contract-alpha".to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            bid_id: "bid-gamma-spare".to_owned(),
            lease_id: "lease-alpha-2".to_owned(),
            lease_ms: LEASE_MS,
        }))
        .expect("the contract changes hands");
    assert_eq!(ledger.contracts()["contract-alpha"].lease.holder, GAMMA);
    assert_eq!(ledger.contracts()["contract-alpha"].lease.generation, 2);

    let closing = |participant: &str| {
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: "contract-alpha".to_owned(),
            participant: participant.to_owned(),
            generation: 2,
            outcome: Outcome::DeadEnd,
        })
    };
    for (who, participant) in [("the displaced holder", ALPHA), ("a stranger", BETA)] {
        assert!(
            matches!(
                expect_refusal(&mut ledger, &closing(participant)),
                CommitmentError::NotAuthorized { .. }
            ),
            "{who} closed an obligation it does not hold"
        );
    }
    assert_eq!(
        ledger.obligations()["obligation-alpha"].state,
        ObligationState::Active
    );
    ledger
        .execute(&closing(GAMMA))
        .expect("the participant that holds the lease closes the work");
    assert_eq!(
        ledger.obligations()["obligation-alpha"].state,
        ObligationState::Terminal
    );

    // Without the rule the displaced holder closes it instead, and the fact records a return by a
    // participant the same facts show does not hold the lease.
    let (mut weakened, _tokens) = prepared_with(DisabledChecks {
        return_holder: true,
        ..DisabledChecks::default()
    });
    for command in [
        award_main("bid-alpha", "alpha"),
        CommitmentCommand::AdvanceClock(AdvanceClock { to: 400 }),
        CommitmentCommand::Reassign(Reassign {
            contract_id: "contract-alpha".to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            bid_id: "bid-gamma-spare".to_owned(),
            lease_id: "lease-alpha-2".to_owned(),
            lease_ms: LEASE_MS,
        }),
    ] {
        weakened
            .execute(&command)
            .expect("the contract changes hands");
    }
    let events = weakened
        .execute(&closing(ALPHA))
        .expect("the weakened kernel accepts a return from a participant without the lease");
    assert!(events.iter().any(|event| matches!(
        event,
        CommitmentEvent::ObligationReturned { participant, .. } if participant == ALPHA
    )));
    assert_eq!(weakened.contracts()["contract-alpha"].lease.holder, GAMMA);
}

/// The same rule over generated orderings rather than one. A reassignment in the pool hands a
/// contract to another participant, and both the participant it was taken from and one that never
/// held it then present the current generation.
#[test]
fn removing_the_return_holder_check_produces_a_counterexample() {
    let (seed, violations) = first_counterexample(DisabledChecks {
        return_holder: true,
        ..DisabledChecks::default()
    })
    .expect("the holder check on the return path must be load-bearing");
    assert!(
        violations
            .iter()
            .any(|violation| matches!(violation, Violation::ClosedByNonHolder { .. })),
        "seed {seed} produced {violations:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Settlement reaches only an account that can still spend
// ---------------------------------------------------------------------------------------------

struct ChainRun {
    ledger: CommitmentLedger,
    chain: Vec<CommitmentCommand>,
    stopped_at: usize,
    refusal: CommitmentError,
}

/// Replay the deterministic prefix and then the settlement chain in the order one participant
/// issues it, stopping at the first command the kernel refuses. That a refusal leaves the registry
/// byte for byte as it was is checked here rather than assumed.
fn chain_until_refusal(disabled: DisabledChecks) -> ChainRun {
    let tokens = Tokens::variant("a");
    let mut ledger = new_ledger();
    ledger.disable_checks(disabled);
    for command in setup(&tokens) {
        ledger.execute(&command).expect("prefix command");
    }
    let chain = schedules::settlement_chain(&tokens);
    for (index, command) in chain.iter().enumerate() {
        let before = ledger.clone();
        if let Err(refusal) = ledger.execute(command) {
            assert_eq!(
                ledger, before,
                "a refused command left the registry changed: {refusal}"
            );
            return ChainRun {
                ledger,
                chain,
                stopped_at: index,
                refusal,
            };
        }
    }
    panic!("the whole settlement chain was accepted: nothing refused the closing contract");
}

fn withdraw_cross() -> CommitmentCommand {
    CommitmentCommand::WithdrawOffer(WithdrawOffer {
        offer_id: CROSS_OFFER.to_owned(),
        sponsor: ALPHA.to_owned(),
    })
}

fn settle_cross() -> CommitmentCommand {
    CommitmentCommand::SettleOffer(SettleOffer {
        offer_id: CROSS_OFFER.to_owned(),
        sponsor: ALPHA.to_owned(),
    })
}

/// The reviewer's reproduction, now refused at its first step, and the same branch closing
/// normally once the reservation it funded has come back.
#[test]
fn a_task_contract_cannot_close_while_a_reservation_is_still_due_back_to_it() {
    let run = chain_until_refusal(DisabledChecks::default());
    assert!(
        matches!(
            &run.chain[run.stopped_at],
            CommitmentCommand::ReturnObligation(command)
                if command.contract_id == SOLO_FUNDING_CONTRACT
        ),
        "the chain was stopped somewhere other than the closing contract"
    );
    assert!(matches!(
        &run.refusal,
        CommitmentError::ReservationOutstanding { contract_id, outstanding }
            if contract_id == SOLO_FUNDING_CONTRACT && outstanding == CROSS_OFFER
    ));

    // Cancellation closes a contract just as a return does, and is held to the same condition:
    // the sponsor cannot dispose of a contract that is still owed a reservation either.
    let mut ledger = run.ledger;
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &CommitmentCommand::CancelContract(CancelContract {
                contract_id: SOLO_FUNDING_CONTRACT.to_owned(),
                sponsor: ROOT_PARTICIPANT.to_owned(),
            })
        ),
        CommitmentError::ReservationOutstanding { contract_id, outstanding }
            if contract_id == SOLO_FUNDING_CONTRACT && outstanding == CROSS_OFFER
    ));

    // The refusal is not a dead end: the reservation comes back to a contract that is still an
    // account, and the same close is then accepted.
    let funded = ledger.offers()[CROSS_OFFER].escrow;
    assert!(!funded.is_zero());
    ledger.execute(&withdraw_cross()).expect("withdraw");
    ledger.execute(&settle_cross()).expect("settle");
    assert_eq!(
        ledger.contracts()[SOLO_FUNDING_CONTRACT].escrow,
        held_before_settlement(&funded),
        "the reservation returned to the account that reserved it"
    );
    ledger
        .execute(&run.chain[run.stopped_at])
        .expect("the contract closes once nothing is due back to it");
    assert_eq!(
        ledger.contracts()[SOLO_FUNDING_CONTRACT].state,
        ContractState::Returned
    );
    assert!(
        ledger.contracts()[SOLO_FUNDING_CONTRACT].escrow.is_zero(),
        "a contract that has closed holds nothing"
    );
    assert!(state_violations(&ledger).is_empty());
}

/// What the funding contract holds once the reservation it made is returned to it: everything it
/// had left, plus the whole pool back.
fn held_before_settlement(returned: &BudgetVector) -> BudgetVector {
    let spent_on_the_offer = schedules::cross_execution_escrow()
        .checked_add(&BudgetVector::unit(Dimension::OfferCreations))
        .expect("the offer cost its pool plus one creation authority");
    schedules::delegating_escrow()
        .checked_sub(&BudgetVector::units(Dimension::WallTimeMs, DEADLINE))
        .expect("the first lease was bought from the escrow")
        .checked_sub(&spent_on_the_offer)
        .expect("advertising the offer cost the pool and the authority")
        .checked_add(returned)
        .expect("and the pool came back")
}

/// The other half of the same rule, exercised where it can be reached: with the closing half
/// switched off a contract does reach a terminal state while a reservation is still due back to
/// it, and the settlement into it is refused instead of committed.
#[test]
fn a_settlement_into_a_task_contract_that_has_closed_is_refused_and_changes_nothing() {
    let run = chain_until_refusal(DisabledChecks {
        returning_reservation: true,
        ..DisabledChecks::default()
    });
    assert!(
        matches!(
            &run.chain[run.stopped_at],
            CommitmentCommand::SettleOffer(command) if command.offer_id == CROSS_OFFER
        ),
        "the chain was stopped somewhere other than the settlement"
    );
    assert!(matches!(
        &run.refusal,
        CommitmentError::AccountClosed { contract_id } if contract_id == SOLO_FUNDING_CONTRACT
    ));
    let mut ledger = run.ledger;
    assert_eq!(
        ledger.contracts()[SOLO_FUNDING_CONTRACT].state,
        ContractState::Returned
    );
    assert!(
        ledger.contracts()[SOLO_FUNDING_CONTRACT].escrow.is_zero(),
        "nothing landed in an account that had already closed"
    );

    // Nor may a funded action draw on it: the advertisement the chain issues next is refused for
    // the same reason, so no reservation is made out of a closed account either.
    let advertisement = &run.chain[run.stopped_at + 1];
    assert!(matches!(
        advertisement,
        CommitmentCommand::Advertise(command) if command.offer_id == SECOND_OFFER
    ));
    assert!(matches!(
        expect_refusal(&mut ledger, advertisement),
        CommitmentError::AccountClosed { contract_id } if contract_id == SOLO_FUNDING_CONTRACT
    ));
    assert!(!ledger.contracts().contains_key("contract-second"));
}

// ---------------------------------------------------------------------------------------------
// Property suite over generated schedules
// ---------------------------------------------------------------------------------------------

#[test]
fn generated_schedules_conserve_budgets_consent_fencing_and_causal_accounting() {
    let tokens = Tokens::variant("a");
    let mut committed = 0;
    let mut reached = std::collections::BTreeSet::new();
    let mut guarded = std::collections::BTreeSet::new();
    let mut compared = std::collections::BTreeSet::new();
    for seed in 0..SCHEDULE_SEEDS {
        let report = run_schedule(seed, &tokens, DisabledChecks::default());
        assert!(
            report.violations.is_empty(),
            "seed {seed} produced {:?}",
            report.violations
        );
        committed += report.committed;
        reached.extend(report.reached);
        guarded.extend(report.guarded);
        compared.extend(report.compared);
    }
    // Every command that moves capacity is compared with what it named, and the comparison is
    // required to have actually been applied to each of them. A command whose accounts the
    // comparison could not resolve would be passed over in silence and would look exactly like a
    // command it found nothing wrong with.
    for command in [
        "register_participant",
        "advertise",
        "award",
        "accept_open",
        "start_attempt",
        "renew_lease",
        "reassign",
        "settle_offer",
        "return_obligation",
        "cancel_contract",
        "start_invocation",
        "resume_invocation",
        "record_verification",
    ] {
        assert!(
            compared.contains(command),
            "no ordering ever compared the facts of {command} with the command that issued them"
        );
    }
    // A contract that never changes hands cannot show that only its holder may close it, so the
    // orderings are required to have actually refused such a return.
    assert!(
        guarded.contains("return_non_holder"),
        "no ordering ever offered a return from a participant without the lease"
    );
    // Likewise for the capacity a command draws on: unless some ordering was actually asked for
    // more than an account holds, the covering check holds by never being reached.
    assert!(
        guarded.contains("insufficient_budget"),
        "no ordering ever asked an account for more than it holds"
    );
    assert!(
        committed > SCHEDULE_SEEDS as usize * 8,
        "the schedules must actually commit work, not only be refused: {committed}"
    );
    // A pool that stopped reaching contention would satisfy every invariant by doing nothing, so
    // the facts the schedules must actually reach are named here.
    for fact in [
        "task_contract_formed",
        "obligation_created",
        "lease_issued",
        "lease_renewed",
        "contract_reassigned",
        "attempt_started",
        "submission_recorded",
        "obligation_returned",
        "contract_cancelled",
        "offer_settled",
        "invocation_started",
        "invocation_yielded",
        "invocation_resumed",
        "invocation_closed",
        "verification_recorded",
    ] {
        assert!(reached.contains(fact), "no schedule ever reached {fact}");
    }
    // The rules a process slice adds are reached the same way: unless some ordering actually
    // offered a resumption nothing had happened for, a close by a stranger, a stop by a participant
    // that owns nothing, and a verdict about a bundle that was never submitted, those rules hold by
    // never being asked.
    for guard in [
        "close_non_holder",
        "stop_run_unauthorized",
        "candidate_mismatch",
    ] {
        assert!(guarded.contains(guard), "no ordering ever provoked {guard}");
    }
}

#[test]
fn a_generated_schedule_is_reproducible_from_its_seed() {
    let tokens = Tokens::variant("a");
    let first = schedules::schedule(7, &tokens);
    let second = schedules::schedule(7, &tokens);
    assert_eq!(first, second);
    assert_ne!(first, schedules::schedule(8, &tokens));
}

// ---------------------------------------------------------------------------------------------
// Each guarded check is shown to be load-bearing
// ---------------------------------------------------------------------------------------------

fn first_counterexample(disabled: DisabledChecks) -> Option<(u64, Vec<Violation>)> {
    let tokens = Tokens::variant("a");
    (0..SCHEDULE_SEEDS).find_map(|seed| {
        let report = run_schedule(seed, &tokens, disabled);
        (!report.violations.is_empty()).then_some((seed, report.violations))
    })
}

#[test]
fn removing_the_funded_award_check_produces_a_counterexample() {
    let (seed, violations) = first_counterexample(DisabledChecks {
        reservation: true,
        ..DisabledChecks::default()
    })
    .expect("the funded-award check must be load-bearing");
    assert!(
        violations.iter().any(|violation| matches!(
            violation,
            Violation::AwardCountExceeded { .. } | Violation::ConsentSpentTwice { .. }
        )),
        "seed {seed} produced {violations:?}"
    );
}

#[test]
fn removing_the_fencing_check_produces_a_counterexample() {
    let (seed, violations) = first_counterexample(DisabledChecks {
        fencing: true,
        ..DisabledChecks::default()
    })
    .expect("the fencing check must be load-bearing");
    assert!(
        violations
            .iter()
            .any(|violation| matches!(violation, Violation::StaleGenerationAdvanced { .. })),
        "seed {seed} produced {violations:?}"
    );
}

/// Escrow belongs to the task contract, not to whoever is holding it at the time. A reservation
/// therefore still returns after the contract changes hands, while spending from that escrow
/// remains the current contractor's alone. Without this the reservation could never come back, and
/// the contract could then never close.
#[test]
fn a_reservation_returns_to_a_contract_that_has_changed_hands() {
    let (mut ledger, tokens) = prepared();
    ledger
        .execute(&award_main("bid-alpha", "alpha"))
        .expect("award");
    ledger
        .execute(&schedules::bid_on_main_for(
            &tokens,
            "bid-beta-spare",
            BETA,
            schedules::spare_requested_escrow(),
        ))
        .expect("consent held in reserve for a replacement holder");
    let delegate_from_contract_alpha = |offer_id: &str| {
        CommitmentCommand::Advertise(Advertise {
            offer_id: offer_id.to_owned(),
            sponsor: ALPHA.to_owned(),
            parent_obligation: "obligation-alpha".to_owned(),
            funding_source: FundingSource::TaskContract {
                contract_id: "contract-alpha".to_owned(),
            },
            task_scope: tokens.task_scope.clone(),
            base_digest: tokens.base_digest.clone(),
            intent_digest: tokens.intent_digest.clone(),
            artifact_class: tokens.artifact_class.clone(),
            dependencies: Vec::new(),
            capability_scope: Vec::new(),
            execution_escrow: BudgetVector::ZERO
                .with(Dimension::MoneyMicros, 2_000)
                .with(Dimension::ModelTokens, 2_000)
                .with(Dimension::WallTimeMs, 2_000),
            policy: OfferPolicy::Negotiated,
            bid_deadline: DEADLINE,
            offer_deadline: DEADLINE,
            max_awards: 1,
        })
    };
    ledger
        .execute(&delegate_from_contract_alpha("offer-delegated"))
        .expect("a contractor delegates from the escrow it holds");
    ledger
        .execute(&CommitmentCommand::AdvanceClock(AdvanceClock { to: 400 }))
        .expect("the lease runs out");
    ledger
        .execute(&CommitmentCommand::Reassign(Reassign {
            contract_id: "contract-alpha".to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            bid_id: "bid-beta-spare".to_owned(),
            lease_id: "lease-alpha-2".to_owned(),
            lease_ms: LEASE_MS,
        }))
        .expect("the contract changes hands");
    assert_eq!(ledger.contracts()["contract-alpha"].contractor, BETA);

    let held = ledger.contracts()["contract-alpha"].escrow;
    let reserved = ledger.offers()["offer-delegated"].escrow;
    ledger
        .execute(&CommitmentCommand::WithdrawOffer(WithdrawOffer {
            offer_id: "offer-delegated".to_owned(),
            sponsor: ALPHA.to_owned(),
        }))
        .expect("the participant that advertised it may withdraw it");
    ledger
        .execute(&CommitmentCommand::SettleOffer(SettleOffer {
            offer_id: "offer-delegated".to_owned(),
            sponsor: ALPHA.to_owned(),
        }))
        .expect("and settle it, whoever holds the contract now");
    assert_eq!(
        ledger.contracts()["contract-alpha"].escrow,
        held.checked_add(&reserved)
            .expect("the reservation returned")
    );

    // Spending from that escrow is another matter: it is the current contractor's alone, and the
    // sponsor of the contract has no more claim on it than anyone else.
    let mut spend_from_contract_alpha = delegate_from_contract_alpha("offer-again");
    if let CommitmentCommand::Advertise(command) = &mut spend_from_contract_alpha {
        command.sponsor = ROOT_PARTICIPANT.to_owned();
        command.parent_obligation = ROOT_OBLIGATION.to_owned();
    }
    assert!(matches!(
        expect_refusal(&mut ledger, &spend_from_contract_alpha),
        CommitmentError::FundingContractMismatch { .. }
    ));
    assert!(state_violations(&ledger).is_empty());
}

/// The settlement rule over generated schedules rather than one ordering. What is asserted is the
/// projection built from the emitted facts, not the registry fields the kernel itself updates.
#[test]
fn generated_schedules_settle_only_into_accounts_that_can_still_spend() {
    let tokens = Tokens::variant("a");
    let mut guarded = std::collections::BTreeSet::new();
    for seed in 0..SCHEDULE_SEEDS {
        let report = run_schedule(seed, &tokens, DisabledChecks::default());
        let broken: Vec<&Violation> = report
            .violations
            .iter()
            .filter(|violation| {
                matches!(
                    violation,
                    Violation::SpentAfterClose { .. }
                        | Violation::ClosedHoldingEscrow { .. }
                        | Violation::StrandedReservation { .. }
                )
            })
            .collect();
        assert!(broken.is_empty(), "seed {seed} produced {broken:?}");
        guarded.extend(report.guarded);
    }
    // A guard no ordering ever reaches holds by never being asked. Both halves of the rule are
    // therefore required to have refused something across the seeds.
    assert!(
        guarded.contains("reservation_outstanding"),
        "no ordering ever closed a contract that was still owed a reservation"
    );
    assert!(
        guarded.contains("account_closed"),
        "no ordering ever named a closed contract as an account"
    );
}

/// Both halves are removed together here, and that is not a convenience. Removing the live-account
/// half alone leaves the schedules clean, because a contract that closes under the other half in
/// force has already had its escrow swept and there is nothing in it left to draw on. The state in
/// which the live-account refusal has anything to say is exactly the state the closing half makes
/// unreachable, so over schedules the pair is what is falsifiable. That the refusal itself fires
/// when that state does exist is shown directly, one command at a time, by
/// `a_settlement_into_a_task_contract_that_has_closed_is_refused_and_changes_nothing`.
#[test]
fn removing_the_settlement_rule_produces_a_counterexample() {
    let (seed, violations) = first_counterexample(DisabledChecks {
        live_account: true,
        returning_reservation: true,
        ..DisabledChecks::default()
    })
    .expect("the settlement rule must be load-bearing");
    assert!(
        violations
            .iter()
            .any(|violation| matches!(violation, Violation::SpentAfterClose { .. })),
        "seed {seed} produced {violations:?}"
    );
}

/// Refusing to settle into a closed contract without also keeping that contract open would only
/// move the break: the reservation would then be owed to an account nobody can reach.
#[test]
fn removing_the_outstanding_reservation_check_strands_a_reservation() {
    let (seed, violations) = first_counterexample(DisabledChecks {
        returning_reservation: true,
        ..DisabledChecks::default()
    })
    .expect("the closing half of the settlement rule must be load-bearing");
    assert!(
        violations
            .iter()
            .any(|violation| matches!(violation, Violation::StrandedReservation { .. })),
        "seed {seed} produced {violations:?}"
    );
}

#[test]
fn removing_the_child_return_check_produces_a_counterexample() {
    let (seed, violations) = first_counterexample(DisabledChecks {
        child_return: true,
        ..DisabledChecks::default()
    })
    .expect("the child-return check must be load-bearing");
    assert!(
        violations
            .iter()
            .any(|violation| matches!(violation, Violation::WorkClosedTooEarly { .. })),
        "seed {seed} produced {violations:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// No transition reads meaning
// ---------------------------------------------------------------------------------------------

/// Replaying the same schedule over a different alphabet of opaque tokens must produce the same
/// facts in the same order, with only the tokens themselves substituted.
///
/// This is how the negative is established rather than asserted: intent, proposal, scope, class,
/// dependency and capability values differ completely between the two runs, so any transition that
/// read one of them — ordered it, matched a substring, preferred one spelling — would move a
/// decision and change the event stream in a way substitution cannot repair.
#[test]
fn no_transition_reads_an_opaque_payload_beyond_its_identity() {
    let first = Tokens::variant("a");
    let second = Tokens::variant("second-alphabet");
    for seed in 0..24 {
        let left = record_events(seed, &first);
        let right = record_events(seed, &second);
        let mut translated = right;
        for (from, to) in second.substitutions(&first) {
            translated = translated.replace(&from, &to);
        }
        assert_eq!(
            left, translated,
            "seed {seed} decided differently under a different alphabet"
        );
    }
}

fn record_events(seed: u64, tokens: &Tokens) -> String {
    let mut ledger = new_ledger();
    let mut lines = Vec::new();
    for command in setup(tokens) {
        let events = ledger.execute(&command).expect("prefix command");
        lines.push(serde_json::to_string(&events).expect("events serialize"));
    }
    for command in schedules::schedule(seed, tokens) {
        match ledger.execute(&command) {
            Ok(events) => lines.push(serde_json::to_string(&events).expect("events serialize")),
            Err(error) => lines.push(format!("refused: {error}")),
        }
    }
    lines.join("\n")
}

/// Every field of every command and every committed fact, as they are actually serialized. A new
/// field that could carry meaning — a skill, a score, a model, a rationale, free text — cannot be
/// added without this list changing, and changing it is the decision that has to be argued.
#[test]
fn control_records_carry_only_mechanical_fields() {
    let tokens = Tokens::variant("a");
    let mut keys = std::collections::BTreeSet::new();
    let mut ledger = new_ledger();
    // A causally valid replay: the deterministic prefix, the award the participant's own sequence
    // needs, that sequence, and then the contended pool. Every command is offered once, and what is
    // collected is the fields of whatever it committed.
    for command in setup(&tokens)
        .into_iter()
        .chain([award_main("bid-alpha", "alpha")])
        .chain(schedules::slice_chain())
        .chain(schedules::contention_pool(&tokens))
    {
        collect_keys(
            &serde_json::to_value(&command).expect("command serializes"),
            &mut keys,
        );
        if let Ok(events) = ledger.execute(&command) {
            for event in &events {
                collect_keys(
                    &serde_json::to_value(event).expect("event serializes"),
                    &mut keys,
                );
            }
        }
    }
    // The durable records themselves, taken one at a time: the maps that hold them are keyed by
    // identifiers, which are values rather than fields.
    for value in ledger.participants().values() {
        collect_keys(&serde_json::to_value(value).expect("record"), &mut keys);
    }
    for value in ledger.offers().values() {
        collect_keys(&serde_json::to_value(value).expect("record"), &mut keys);
    }
    for value in ledger.bids().values() {
        collect_keys(&serde_json::to_value(value).expect("record"), &mut keys);
    }
    for value in ledger.contracts().values() {
        collect_keys(&serde_json::to_value(value).expect("record"), &mut keys);
    }
    for value in ledger.obligations().values() {
        collect_keys(&serde_json::to_value(value).expect("record"), &mut keys);
    }
    for value in ledger.attempts().values() {
        collect_keys(&serde_json::to_value(value).expect("record"), &mut keys);
    }
    for value in ledger.invocations().values() {
        collect_keys(&serde_json::to_value(value).expect("record"), &mut keys);
    }
    for value in ledger.verifications() {
        collect_keys(&serde_json::to_value(value).expect("record"), &mut keys);
    }
    let ledger_value = serde_json::to_value(&ledger).expect("ledger serializes");
    let top_level: std::collections::BTreeSet<&str> = ledger_value
        .as_object()
        .expect("the ledger is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top_level,
        [
            "attempts",
            "bids",
            "consumed",
            "contracts",
            "facts",
            "initial_total",
            "invocations",
            "now",
            "obligations",
            "offers",
            "participants",
            "root_obligation",
            "stopped",
            "verifications",
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<&str>>()
    );

    let expected: std::collections::BTreeSet<String> = [
        "account",
        "amount",
        "artifact_class",
        "attempt_id",
        "authority",
        "awards_made",
        "balance",
        "base_digest",
        "bid_deadline",
        "bid_id",
        "bidder",
        "candidate_digest",
        "capability_scope",
        "children",
        "closer",
        "closure",
        "command",
        "conditions",
        "contract_id",
        "contractor",
        "cursor",
        "dependencies",
        "endowment",
        "escrow",
        "event",
        "execution_escrow",
        "expires_at",
        "from",
        "funding",
        "funding_source",
        "generation",
        "holder",
        "intent_digest",
        "invocation_id",
        "lease",
        "lease_id",
        "lease_ms",
        "matched_sequence",
        "max_awards",
        "obligation_id",
        "offer_deadline",
        "offer_id",
        "origin",
        "outcome",
        "owner",
        "parent",
        "parent_obligation",
        "participant",
        "participant_id",
        "policy",
        "previous_holder",
        "principal_id",
        "proposal_digest",
        "reason",
        "requested_escrow",
        "root_scope",
        "sponsor",
        "state",
        "task_scope",
        "to",
        "verdict",
        "wake",
        "wake_deadline",
        "wakes_used",
        "yielded_at",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    assert_eq!(
        keys, expected,
        "the mechanical field list changed; every addition must be justified"
    );
}

fn collect_keys(value: &serde_json::Value, keys: &mut std::collections::BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, nested) in map {
                keys.insert(key.clone());
                collect_keys(nested, keys);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_keys(item, keys);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// A process slice yields, resumes from its cursor, and stops for good
// ---------------------------------------------------------------------------------------------

/// A ledger with one participant holding one task contract directly under the root obligation, and
/// one attempt open on it. Everything the lifecycle tests do happens inside that position.
fn running_attempt() -> CommitmentLedger {
    let tokens = Tokens::variant("life");
    let mut ledger = new_ledger();
    for command in life_setup(&tokens) {
        ledger
            .execute(&command)
            .expect("the lifecycle prefix is well formed");
    }
    ledger
}

fn start_slice(cursor: u64) -> CommitmentCommand {
    CommitmentCommand::StartInvocation(StartInvocation {
        invocation_id: LIFE_INVOCATION.to_owned(),
        attempt_id: LIFE_ATTEMPT.to_owned(),
        contract_id: LIFE_CONTRACT.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        cursor,
    })
}

fn yield_slice(cursor: u64, conditions: Vec<WakeCondition>, deadline: u64) -> CommitmentCommand {
    CommitmentCommand::YieldInvocation(YieldInvocation {
        invocation_id: LIFE_INVOCATION.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        cursor,
        conditions,
        wake_deadline: deadline,
    })
}

fn resume_slice() -> CommitmentCommand {
    CommitmentCommand::ResumeInvocation(ResumeInvocation {
        invocation_id: LIFE_INVOCATION.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
    })
}

fn submit_life() -> CommitmentCommand {
    CommitmentCommand::SubmitResult(SubmitResult {
        contract_id: LIFE_CONTRACT.to_owned(),
        attempt_id: LIFE_ATTEMPT.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        candidate_digest: digest("candidate-life"),
    })
}

fn submission_condition() -> Vec<WakeCondition> {
    vec![WakeCondition::SubmissionRecorded {
        contract_id: LIFE_CONTRACT.to_owned(),
    }]
}

/// A yielded slice runs no process, is admitted only after a fact it named, and buys each admission
/// with one unit of creation authority.
#[test]
fn a_yielded_slice_has_no_process_and_resumes_only_on_a_committed_fact() {
    let mut ledger = running_attempt();
    let held = ledger.contracts()[LIFE_CONTRACT]
        .escrow
        .get(Dimension::InvocationStarts);
    ledger.execute(&start_slice(0)).expect("first slice");
    assert_eq!(
        ledger.contracts()[LIFE_CONTRACT]
            .escrow
            .get(Dimension::InvocationStarts),
        held - 1,
        "beginning a process slice spends one unit of creation authority"
    );
    assert_eq!(
        ledger.invocations()[LIFE_INVOCATION].state,
        InvocationState::Running
    );

    ledger
        .execute(&yield_slice(
            ledger.sequence(),
            submission_condition(),
            LIFE_WAKE_DEADLINE,
        ))
        .expect("yield");
    let yielded = &ledger.invocations()[LIFE_INVOCATION];
    assert_eq!(yielded.state, InvocationState::Yielded);
    assert_eq!(
        ledger.open_authority(),
        Some(OpenAuthority::FundedWake {
            invocation_id: LIFE_INVOCATION.to_owned()
        }),
        "a funded wake keeps the run open, and nothing else about a yielded slice does"
    );

    // Nothing it asked about has happened, so there is nothing to resume for.
    assert!(matches!(
        expect_refusal(&mut ledger, &resume_slice()),
        CommitmentError::NoMatchingEvent { .. }
    ));

    ledger.execute(&submit_life()).expect("submission");
    let matched = ledger
        .matching_fact(LIFE_INVOCATION)
        .expect("the submission is the fact the yield named");
    let events = ledger.execute(&resume_slice()).expect("resume");
    assert!(events.iter().any(|event| matches!(
        event,
        CommitmentEvent::InvocationResumed {
            matched_sequence,
            wakes_used: 1,
            ..
        } if *matched_sequence == matched
    )));
    assert_eq!(
        ledger.contracts()[LIFE_CONTRACT]
            .escrow
            .get(Dimension::InvocationStarts),
        held - 2,
        "an admission is bought with a unit of its own"
    );
    assert_eq!(
        ledger.invocations()[LIFE_INVOCATION].state,
        InvocationState::Running
    );
}

/// The same fact, delivered any number of times or not at all, wakes a slice once. What decides is
/// the committed stream after the cursor, so a reader that saw every notification and one that saw
/// none reach the same conclusion.
#[test]
fn duplicate_coalesced_and_lost_notifications_change_nothing() {
    let mut ledger = running_attempt();
    ledger.execute(&start_slice(0)).expect("first slice");
    let cursor = ledger.sequence();
    ledger
        .execute(&yield_slice(
            cursor,
            vec![
                WakeCondition::SubmissionRecorded {
                    contract_id: LIFE_CONTRACT.to_owned(),
                },
                WakeCondition::BidRecorded {
                    offer_id: schedules::LIFE_OFFER.to_owned(),
                },
            ],
            LIFE_WAKE_DEADLINE,
        ))
        .expect("yield");
    assert!(ledger.matching_fact(LIFE_INVOCATION).is_none());

    // Several facts the yield asked about, committed one after another. A channel that delivered
    // one of them, all of them, or none of them is not consulted at any point.
    ledger.execute(&submit_life()).expect("submission");
    let first = ledger
        .matching_fact(LIFE_INVOCATION)
        .expect("the first matching fact");
    ledger.execute(&submit_life()).expect("second submission");
    ledger
        .execute(&CommitmentCommand::RecordBid(RecordBid {
            bid_id: "bid-late".to_owned(),
            offer_id: schedules::LIFE_OFFER.to_owned(),
            bidder: BETA.to_owned(),
            requested_escrow: BudgetVector::ZERO,
            artifact_class: Tokens::variant("life").artifact_class.clone(),
            proposal_digest: None,
            expires_at: DEADLINE,
        }))
        .expect("consent");
    assert_eq!(
        ledger.matching_fact(LIFE_INVOCATION),
        Some(first),
        "three matching facts are one answer: whether there is anything to resume for"
    );

    ledger.execute(&resume_slice()).expect("one resumption");
    assert!(matches!(
        expect_refusal(&mut ledger, &resume_slice()),
        CommitmentError::InvocationNotYielded { .. }
    ));

    // The reader that missed everything recovers by asking what lies after the cursor it recorded,
    // and is told exactly the facts it missed.
    let missed = ledger.facts_after(cursor);
    assert_eq!(missed.len() as u64, ledger.sequence() - cursor);
    assert!(missed.iter().any(|fact| matches!(
        fact,
        CommitmentEvent::SubmissionRecorded { contract_id, .. } if contract_id == LIFE_CONTRACT
    )));
}

/// A cursor moves forward and never back, and never names a fact the run has not committed.
#[test]
fn a_cursor_only_moves_forward_into_facts_that_exist() {
    let mut ledger = running_attempt();
    ledger.execute(&start_slice(0)).expect("first slice");
    let cursor = ledger.sequence();
    ledger
        .execute(&yield_slice(
            cursor,
            submission_condition(),
            LIFE_WAKE_DEADLINE,
        ))
        .expect("yield");
    ledger.execute(&submit_life()).expect("submission");
    ledger.execute(&resume_slice()).expect("resume");
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &yield_slice(cursor - 1, submission_condition(), LIFE_WAKE_DEADLINE)
        ),
        CommitmentError::CursorRegression { .. }
    ));
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &yield_slice(u64::MAX, submission_condition(), LIFE_WAKE_DEADLINE)
        ),
        CommitmentError::CursorAhead { .. }
    ));
}

/// A wake is bounded three ways at once: by how many conditions it may name, by a deadline inside
/// the funded lease, and by how many times the slices of one attempt may be admitted at all.
#[test]
fn a_wake_is_bounded_in_conditions_in_time_and_in_number() {
    let mut ledger = running_attempt();
    ledger.execute(&start_slice(0)).expect("first slice");
    assert!(matches!(
        expect_refusal(&mut ledger, &yield_slice(0, Vec::new(), LIFE_WAKE_DEADLINE)),
        CommitmentError::InvalidWakeConditions
    ));
    let too_many = (0..=MAX_WAKE_CONDITIONS)
        .map(|index| WakeCondition::ObligationReturned {
            obligation_id: format!("obligation-{index}"),
        })
        .collect();
    assert!(matches!(
        expect_refusal(&mut ledger, &yield_slice(0, too_many, LIFE_WAKE_DEADLINE)),
        CommitmentError::InvalidWakeConditions
    ));
    // The lease is funded to exactly what the award bought. A wake may not outlive it.
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &yield_slice(0, submission_condition(), LIFE_LEASE_MS + 1)
        ),
        CommitmentError::WakeDeadlineUnfunded { .. }
    ));

    ledger.execute(&submit_life()).expect("submission");
    for _ in 0..MAX_ATTEMPT_WAKES {
        ledger
            .execute(&yield_slice(0, submission_condition(), LIFE_WAKE_DEADLINE))
            .expect("yield");
        ledger.execute(&resume_slice()).expect("resume");
    }
    ledger
        .execute(&yield_slice(0, submission_condition(), LIFE_WAKE_DEADLINE))
        .expect("yield");
    assert!(
        ledger.contracts()[LIFE_CONTRACT]
            .escrow
            .covers(&BudgetVector::unit(Dimension::InvocationStarts)),
        "the escrow still funds another slice, so only the wake bound can stop this"
    );
    assert!(matches!(
        expect_refusal(&mut ledger, &resume_slice()),
        CommitmentError::WakeBudgetExhausted { .. }
    ));
    assert_eq!(
        ledger.open_authority(),
        Some(OpenAuthority::Obligation {
            obligation_id: LIFE_OBLIGATION.to_owned()
        }),
        "what holds the run open is the outstanding obligation; a wake nobody may use any more \
         is not among the things it waits for"
    );
}

/// A wake whose deadline has passed stops being something the run waits for, and the slice can then
/// be recorded as expired rather than left open.
#[test]
fn an_expired_wake_stops_holding_the_run_open() {
    let mut ledger = running_attempt();
    ledger.execute(&start_slice(0)).expect("first slice");
    ledger
        .execute(&yield_slice(0, submission_condition(), LIFE_WAKE_DEADLINE))
        .expect("yield");
    // The deadline has not passed, so the expiry cannot be claimed.
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &CommitmentCommand::CloseInvocation(CloseInvocation {
                invocation_id: LIFE_INVOCATION.to_owned(),
                closer: ALPHA.to_owned(),
                reason: InvocationClosure::WakeDeadlineExpired,
            })
        ),
        CommitmentError::WakeDeadlinePassed { .. }
    ));
    ledger
        .execute(&CommitmentCommand::AdvanceClock(AdvanceClock {
            to: LIFE_WAKE_DEADLINE + 1,
        }))
        .expect("clock");
    assert_eq!(
        ledger.open_authority(),
        Some(OpenAuthority::Obligation {
            obligation_id: LIFE_OBLIGATION.to_owned()
        }),
        "what still holds the run open is the outstanding obligation, not the dead wake"
    );
    assert!(matches!(
        expect_refusal(&mut ledger, &resume_slice()),
        CommitmentError::WakeDeadlinePassed { .. }
    ));
    ledger
        .execute(&CommitmentCommand::CloseInvocation(CloseInvocation {
            invocation_id: LIFE_INVOCATION.to_owned(),
            closer: ALPHA.to_owned(),
            reason: InvocationClosure::WakeDeadlineExpired,
        }))
        .expect("the deadline has passed and may now be recorded");
    assert_eq!(
        ledger.invocations()[LIFE_INVOCATION].closure,
        Some(InvocationClosure::WakeDeadlineExpired)
    );
}

/// Turns are taken between principals and, inside one principal, in the order the facts recorded
/// the yields. Yielding more often lengthens a principal's own queue and buys it nothing.
#[test]
fn admission_alternates_between_principals_in_recorded_order() {
    let tokens = Tokens::variant("life");
    let mut ledger = new_ledger();
    for command in life_setup(&tokens) {
        ledger.execute(&command).expect("prefix");
    }
    // A second and third contract on the same offer would exceed its funded count, so the other
    // participants take their own uncontended work: one offer each, awarded to them.
    let mut open_contract =
        |offer: &str, bid: &str, contract: &str, obligation: &str, participant: &str| {
            ledger
                .execute(&CommitmentCommand::Advertise(Advertise {
                    offer_id: offer.to_owned(),
                    sponsor: ROOT_PARTICIPANT.to_owned(),
                    parent_obligation: ROOT_OBLIGATION.to_owned(),
                    funding_source: FundingSource::Participant,
                    task_scope: tokens.task_scope.clone(),
                    base_digest: tokens.base_digest.clone(),
                    intent_digest: tokens.intent_digest.clone(),
                    artifact_class: tokens.artifact_class.clone(),
                    dependencies: Vec::new(),
                    capability_scope: Vec::new(),
                    execution_escrow: requested_escrow()
                        .with(Dimension::WallTimeMs, 5_000)
                        .with(Dimension::InvocationStarts, 8),
                    policy: OfferPolicy::Negotiated,
                    bid_deadline: DEADLINE,
                    offer_deadline: DEADLINE,
                    max_awards: 1,
                }))
                .expect("offer");
            ledger
                .execute(&CommitmentCommand::RecordBid(RecordBid {
                    bid_id: bid.to_owned(),
                    offer_id: offer.to_owned(),
                    bidder: participant.to_owned(),
                    requested_escrow: requested_escrow()
                        .with(Dimension::WallTimeMs, 5_000)
                        .with(Dimension::InvocationStarts, 8),
                    artifact_class: tokens.artifact_class.clone(),
                    proposal_digest: None,
                    expires_at: DEADLINE,
                }))
                .expect("consent");
            ledger
                .execute(&CommitmentCommand::Award(Award {
                    contract_id: contract.to_owned(),
                    obligation_id: obligation.to_owned(),
                    lease_id: format!("lease-{contract}"),
                    offer_id: offer.to_owned(),
                    bid_id: bid.to_owned(),
                    sponsor: ROOT_PARTICIPANT.to_owned(),
                    lease_ms: LIFE_LEASE_MS,
                }))
                .expect("award");
        };
    open_contract(
        "offer-alpha2",
        "bid-alpha2",
        "contract-alpha2",
        "obligation-alpha2",
        ALPHA,
    );
    open_contract(
        "offer-beta",
        "bid-beta",
        "contract-beta",
        "obligation-beta",
        BETA,
    );
    open_contract(
        "offer-gamma",
        "bid-gamma",
        "contract-gamma",
        "obligation-gamma",
        GAMMA,
    );

    let mut open_slice = |invocation: &str, attempt: &str, contract: &str, participant: &str| {
        ledger
            .execute(&CommitmentCommand::StartAttempt(StartAttempt {
                attempt_id: attempt.to_owned(),
                contract_id: contract.to_owned(),
                participant: participant.to_owned(),
                generation: 1,
            }))
            .expect("attempt");
        // The cursor is where the run stands now, so only what happens after the wait is recorded
        // can answer it.
        let cursor = ledger.sequence();
        ledger
            .execute(&CommitmentCommand::StartInvocation(StartInvocation {
                invocation_id: invocation.to_owned(),
                attempt_id: attempt.to_owned(),
                contract_id: contract.to_owned(),
                participant: participant.to_owned(),
                generation: 1,
                cursor,
            }))
            .expect("slice");
        ledger
            .execute(&CommitmentCommand::YieldInvocation(YieldInvocation {
                invocation_id: invocation.to_owned(),
                participant: participant.to_owned(),
                generation: 1,
                cursor,
                conditions: vec![WakeCondition::BidRecorded {
                    offer_id: schedules::LIFE_OFFER.to_owned(),
                }],
                wake_deadline: LIFE_WAKE_DEADLINE,
            }))
            .expect("yield");
    };
    // Two waiting slices under one principal and one under each of the others. Alpha's two are
    // recorded first, which is exactly what must not put both of them ahead of the queue.
    open_slice(
        "invocation-alpha-a",
        "attempt-alpha-x",
        LIFE_CONTRACT,
        ALPHA,
    );
    open_slice(
        "invocation-alpha-b",
        "attempt-alpha-y",
        "contract-alpha2",
        ALPHA,
    );
    open_slice("invocation-beta-a", "attempt-beta", "contract-beta", BETA);
    open_slice(
        "invocation-gamma-a",
        "attempt-gamma",
        "contract-gamma",
        GAMMA,
    );

    let order: Vec<&str> = ledger
        .admission_order()
        .into_iter()
        .map(|invocation| invocation.invocation_id.as_str())
        .collect();
    assert!(
        order.is_empty(),
        "nothing any of them asked about has happened yet"
    );

    ledger
        .execute(&CommitmentCommand::RecordBid(RecordBid {
            bid_id: "bid-wakes-everyone".to_owned(),
            offer_id: schedules::LIFE_OFFER.to_owned(),
            bidder: BETA.to_owned(),
            requested_escrow: BudgetVector::ZERO,
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: None,
            expires_at: DEADLINE,
        }))
        .expect("one fact all three asked about");
    let order: Vec<String> = ledger
        .admission_order()
        .into_iter()
        .map(|invocation| invocation.invocation_id.clone())
        .collect();
    assert_eq!(
        order,
        vec![
            "invocation-alpha-a".to_owned(),
            "invocation-beta-a".to_owned(),
            "invocation-gamma-a".to_owned(),
            "invocation-alpha-b".to_owned(),
        ],
        "one turn each before anybody takes a second, and inside one principal in the order the \
         waits were recorded"
    );
}

/// A slice belongs to the participant running it. Nobody else stops it, starts it again, or takes
/// it over after the contract has changed hands.
#[test]
fn only_the_participant_or_its_sponsor_touches_a_slice() {
    let mut ledger = running_attempt();
    ledger.execute(&start_slice(0)).expect("first slice");
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &CommitmentCommand::YieldInvocation(YieldInvocation {
                invocation_id: LIFE_INVOCATION.to_owned(),
                participant: BETA.to_owned(),
                generation: 1,
                cursor: 0,
                conditions: submission_condition(),
                wake_deadline: LIFE_WAKE_DEADLINE,
            })
        ),
        CommitmentError::NotAuthorized { .. }
    ));
    ledger
        .execute(&yield_slice(0, submission_condition(), LIFE_WAKE_DEADLINE))
        .expect("yield");
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &CommitmentCommand::ResumeInvocation(ResumeInvocation {
                invocation_id: LIFE_INVOCATION.to_owned(),
                participant: BETA.to_owned(),
                generation: 1,
            })
        ),
        CommitmentError::NotAuthorized { .. }
    ));
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &CommitmentCommand::CloseInvocation(CloseInvocation {
                invocation_id: LIFE_INVOCATION.to_owned(),
                closer: BETA.to_owned(),
                reason: InvocationClosure::ParticipantLost,
            })
        ),
        CommitmentError::NotAuthorized { .. }
    ));
    // The sponsor of the task contract may record that the participant stopped answering.
    ledger
        .execute(&CommitmentCommand::CloseInvocation(CloseInvocation {
            invocation_id: LIFE_INVOCATION.to_owned(),
            closer: ROOT_PARTICIPANT.to_owned(),
            reason: InvocationClosure::ParticipantLost,
        }))
        .expect("the sponsor records the loss");
    assert_eq!(
        ledger.invocations()[LIFE_INVOCATION].state,
        InvocationState::Closed
    );
}

/// A protected query names the exact candidate its task contract recorded, and spends a reservation
/// whatever its verdict is.
#[test]
fn a_protected_query_names_the_exact_candidate_and_spends_its_reservation() {
    let mut ledger = running_attempt();
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &CommitmentCommand::RecordVerification(RecordVerification {
                contract_id: LIFE_CONTRACT.to_owned(),
                participant: ALPHA.to_owned(),
                generation: 1,
                candidate_digest: digest("candidate-life"),
                verdict: Verdict::Passed,
            })
        ),
        CommitmentError::NoCandidate { .. }
    ));
    ledger.execute(&submit_life()).expect("submission");
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &CommitmentCommand::RecordVerification(RecordVerification {
                contract_id: LIFE_CONTRACT.to_owned(),
                participant: ALPHA.to_owned(),
                generation: 1,
                candidate_digest: digest("candidate-elsewhere"),
                verdict: Verdict::Passed,
            })
        ),
        CommitmentError::CandidateMismatch { .. }
    ));
    let held = ledger.contracts()[LIFE_CONTRACT]
        .escrow
        .get(Dimension::VerificationQueries);
    ledger
        .execute(&CommitmentCommand::RecordVerification(RecordVerification {
            contract_id: LIFE_CONTRACT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            candidate_digest: digest("candidate-life"),
            verdict: Verdict::InfrastructureError,
        }))
        .expect("a query that could not be carried out is still a query");
    assert_eq!(
        ledger.contracts()[LIFE_CONTRACT]
            .escrow
            .get(Dimension::VerificationQueries),
        held - 1
    );
    assert!(ledger.verifications()[0].root_scope);
}

/// A stopped run winds down. Nothing new is begun, extended or paid for, and what already exists is
/// closed with the commands the protocol already has.
#[test]
fn a_stopped_run_creates_nothing_further_and_still_winds_down() {
    let mut ledger = running_attempt();
    ledger.execute(&start_slice(0)).expect("first slice");
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &CommitmentCommand::StopRun(StopRun {
                authority: ALPHA.to_owned(),
                reason: StopReason::Cancelled,
            })
        ),
        CommitmentError::NotAuthorized { .. }
    ));
    ledger
        .execute(&CommitmentCommand::StopRun(StopRun {
            authority: ROOT_PARTICIPANT.to_owned(),
            reason: StopReason::Cancelled,
        }))
        .expect("the participant the root obligation belongs to stops the run");
    assert!(matches!(
        expect_refusal(&mut ledger, &start_slice(0)),
        CommitmentError::RunStopped
    ));
    assert!(matches!(
        expect_refusal(
            &mut ledger,
            &CommitmentCommand::StartAttempt(StartAttempt {
                attempt_id: "attempt-after-stop".to_owned(),
                contract_id: LIFE_CONTRACT.to_owned(),
                participant: ALPHA.to_owned(),
                generation: 1,
            })
        ),
        CommitmentError::RunStopped
    ));
    // Winding down is still allowed, and is what actually closes the accounting.
    ledger
        .execute(&yield_slice(0, submission_condition(), LIFE_WAKE_DEADLINE))
        .expect("a yield ends a slice and begins nothing");
    assert_eq!(ledger.root_terminal(), None);
    for command in schedules::life_wind_down() {
        let _ = ledger.execute(&command);
    }
    assert_eq!(ledger.open_authority(), None);
    assert_eq!(ledger.root_terminal(), Some(RootTerminal::Cancelled));
}

// ---------------------------------------------------------------------------------------------
// Every run reaches the terminal state its scenario describes, and never acceptance by accident
// ---------------------------------------------------------------------------------------------

/// How many interleavings each scenario is replayed in.
const LIFECYCLE_SEEDS: u64 = 48;

#[test]
fn generated_lifecycle_schedules_reach_the_terminal_state_each_scenario_describes() {
    let mut guarded = std::collections::BTreeSet::new();
    let mut reached = std::collections::BTreeSet::new();
    for scenario in Scenario::ALL {
        for seed in 0..LIFECYCLE_SEEDS {
            let report = run_lifecycle(seed, scenario, DisabledChecks::default());
            assert!(
                report.violations.is_empty(),
                "{} at seed {seed} produced {:?}",
                scenario.label(),
                report.violations
            );
            assert_eq!(
                report.terminal,
                Some(scenario.expected()),
                "{} at seed {seed}",
                scenario.label()
            );
            guarded.extend(report.guarded);
            reached.extend(report.reached);
        }
    }
    // Exactly one scenario may end in acceptance, and it is the one whose protected query passed.
    for fact in [
        "invocation_started",
        "invocation_yielded",
        "invocation_resumed",
        "invocation_closed",
        "verification_recorded",
        "run_stopped",
        "obligation_returned",
        "contract_cancelled",
        "offer_settled",
    ] {
        assert!(reached.contains(fact), "no scenario ever reached {fact}");
    }
    // Each of these rules is reached by some ordering of some scenario rather than merely stated.
    for guard in [
        "no_matching_event",
        "wake_budget_exhausted",
        "wake_deadline_passed",
        "cursor_ahead",
        "candidate_mismatch",
        "run_stopped",
        "stale_generation",
        "not_authorized",
    ] {
        assert!(guarded.contains(guard), "no ordering ever provoked {guard}");
    }
}

/// Quiescence is not acceptance. A run that went quiet, spent its last unit, or was handed a result
/// nobody verified reaches a non-success state, and the one route to acceptance is a passing
/// protected query against the exact root candidate.
#[test]
fn quiescence_a_spent_budget_and_an_unverified_result_are_not_acceptance() {
    for scenario in Scenario::ALL {
        for seed in 0..LIFECYCLE_SEEDS {
            let report = run_lifecycle(seed, scenario, DisabledChecks::default());
            if scenario == Scenario::Accepted {
                assert_eq!(report.terminal, Some(RootTerminal::Accepted));
            } else {
                assert_ne!(
                    report.terminal,
                    Some(RootTerminal::Accepted),
                    "{} at seed {seed} claimed acceptance",
                    scenario.label()
                );
            }
        }
    }
    // The rejected run and the one that was never verified differ only in whether a query was
    // spent, and neither of them is accepted.
    assert_eq!(
        run_lifecycle(0, Scenario::Rejected, DisabledChecks::default()).terminal,
        Some(RootTerminal::Exhausted)
    );
    assert_eq!(
        run_lifecycle(0, Scenario::NoSolution, DisabledChecks::default()).terminal,
        Some(RootTerminal::Exhausted)
    );
}

// ---------------------------------------------------------------------------------------------
// Each finite condition of the lifecycle is shown to be load-bearing
// ---------------------------------------------------------------------------------------------

fn first_lifecycle_counterexample(disabled: DisabledChecks) -> Option<(String, Vec<Violation>)> {
    Scenario::ALL.into_iter().find_map(|scenario| {
        (0..LIFECYCLE_SEEDS).find_map(|seed| {
            let report = run_lifecycle(seed, scenario, disabled);
            (!report.violations.is_empty()).then(|| {
                (
                    format!("{} at seed {seed}", scenario.label()),
                    report.violations,
                )
            })
        })
    })
}

/// The finite creation condition: without it a process slice begins that nothing paid for, so a run
/// can run processes it was never funded to run.
#[test]
fn removing_the_invocation_start_check_produces_a_counterexample() {
    let (where_, violations) = first_lifecycle_counterexample(DisabledChecks {
        invocation_start: true,
        ..DisabledChecks::default()
    })
    .expect("the process-slice creation check must be load-bearing");
    assert!(
        violations
            .iter()
            .any(|violation| matches!(violation, Violation::UnfundedInvocationStart { .. })),
        "{where_} produced {violations:?}"
    );
}

/// The wake condition: without it a participant is admitted although nothing it asked about has
/// happened, so a run can be kept alive by resuming on nothing.
#[test]
fn removing_the_wake_match_check_produces_a_counterexample() {
    let (where_, violations) = first_lifecycle_counterexample(DisabledChecks {
        wake_match: true,
        ..DisabledChecks::default()
    })
    .expect("the wake-match check must be load-bearing");
    assert!(
        violations
            .iter()
            .any(|violation| matches!(violation, Violation::ResumedWithoutEvent { .. })),
        "{where_} produced {violations:?}"
    );
}

/// The finite wake count: without it the slices of one attempt are admitted past the bound the
/// protocol states, and a participant that keeps asking keeps being answered.
#[test]
fn removing_the_wake_count_check_produces_a_counterexample() {
    let (where_, violations) = first_lifecycle_counterexample(DisabledChecks {
        wake_count: true,
        ..DisabledChecks::default()
    })
    .expect("the wake-count check must be load-bearing");
    assert!(
        violations
            .iter()
            .any(|violation| matches!(violation, Violation::WakeCountExceeded { .. })),
        "{where_} produced {violations:?}"
    );
}

/// The obligation-return condition, seen from the terminal state rather than from the accounting:
/// without it causal work closes from the inside out, and the lifecycle schedules say so too.
#[test]
fn removing_the_child_return_check_breaks_the_lifecycle_schedules_as_well() {
    let disabled = DisabledChecks {
        child_return: true,
        ..DisabledChecks::default()
    };
    let (where_, violations) = first_counterexample(disabled)
        .map(|(seed, violations)| (format!("seed {seed}"), violations))
        .or_else(|| first_lifecycle_counterexample(disabled))
        .expect("the child-return check must be load-bearing");
    assert!(
        violations.iter().any(|violation| matches!(
            violation,
            Violation::WorkClosedTooEarly { .. } | Violation::DishonestTerminal { .. }
        )),
        "{where_} produced {violations:?}"
    );
}

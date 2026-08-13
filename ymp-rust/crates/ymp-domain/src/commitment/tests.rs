//! The transition suite and the property suite for local commitments.
//!
//! Three kinds of claim are made here. Transition tests state what one command does and what it
//! refuses. Property tests replay generated schedules and assert that no ordering breaks
//! conservation, the funded award count, consent, fencing or causal accounting. Mutation tests
//! switch off one guarded check at a time and require the property suite to produce a
//! counterexample, so that each check is shown to be load-bearing rather than asserted to be.

use super::budget::{BudgetVector, DIMENSIONS, Dimension, DimensionKind};
use super::ledger::{CommitmentLedger, DisabledChecks};
use super::protocol::{
    AcceptOpen, AdvanceClock, Advertise, Award, CommitmentCommand, CommitmentError,
    CommitmentEvent, Reassign, RecordBid, RenewLease, ReturnObligation, StartAttempt, SubmitResult,
};
use super::records::{
    AccountRef, BidState, ContractState, FundingSource, ObligationState, OfferPolicy, OfferState,
    Outcome,
};
use super::schedules::{
    self, ALPHA, BETA, DEADLINE, GAMMA, LEASE_MS, MAIN_MAX_AWARDS, MAIN_OFFER, ROOT_OBLIGATION,
    ROOT_PARTICIPANT, Tokens, Violation, advertise_main, award_main, digest, new_ledger,
    requested_escrow, run_schedule, setup, shuffled, state_violations,
};

/// How many generated schedules the property suite replays. Every seed is a different total order
/// over the same contended pool.
const SCHEDULE_SEEDS: u64 = 192;

fn prepared() -> (CommitmentLedger, Tokens) {
    let tokens = Tokens::variant("a");
    let mut ledger = new_ledger();
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
        4,
        "offer, obligation, participant and attempt creation are the authority dimensions"
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

// ---------------------------------------------------------------------------------------------
// Property suite over generated schedules
// ---------------------------------------------------------------------------------------------

#[test]
fn generated_schedules_conserve_budgets_consent_fencing_and_causal_accounting() {
    let tokens = Tokens::variant("a");
    let mut committed = 0;
    let mut reached = std::collections::BTreeSet::new();
    for seed in 0..SCHEDULE_SEEDS {
        let report = run_schedule(seed, &tokens, DisabledChecks::default());
        assert!(
            report.violations.is_empty(),
            "seed {seed} produced {:?}",
            report.violations
        );
        committed += report.committed;
        reached.extend(report.reached);
    }
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
    ] {
        assert!(reached.contains(fact), "no schedule ever reached {fact}");
    }
}

#[test]
fn a_generated_schedule_is_reproducible_from_its_seed() {
    let tokens = Tokens::variant("a");
    let first = shuffled(7, schedules::contention_pool(&tokens));
    let second = shuffled(7, schedules::contention_pool(&tokens));
    assert_eq!(first, second);
    assert_ne!(first, shuffled(8, schedules::contention_pool(&tokens)));
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
    for command in shuffled(seed, schedules::contention_pool(tokens)) {
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
    for command in setup(&tokens)
        .into_iter()
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
            "initial_total",
            "now",
            "obligations",
            "offers",
            "participants",
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<&str>>()
    );

    let expected: std::collections::BTreeSet<String> = [
        "account",
        "amount",
        "artifact_class",
        "attempt_id",
        "awards_made",
        "balance",
        "base_digest",
        "bid_deadline",
        "bid_id",
        "bidder",
        "candidate_digest",
        "capability_scope",
        "children",
        "command",
        "contract_id",
        "contractor",
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
        "lease",
        "lease_id",
        "lease_ms",
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
        "requested_escrow",
        "sponsor",
        "state",
        "task_scope",
        "to",
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

//! Delivery is allowed to fail; the committed stream is not.
//!
//! The domain suite proves that a resumption needs a committed fact and that a run stops honestly.
//! This suite proves the half that only exists once real readers and real channels are involved:
//! that a notification channel carries no state anybody depends on, that a reader which received
//! every notification and one which received none reach the same conclusion, and that the same
//! command delivered twice inside one controller interval has one effect and one sequence.

use std::sync::mpsc::TryRecvError;

use ymp_application::{CommitmentService, CommitmentServiceError};
use ymp_domain::commitment::{
    Advertise, Award, BudgetVector, CloseInvocation, CommitmentCommand, CommitmentEvent,
    CommitmentLedger, Dimension, FundingSource, InvocationClosure, OfferPolicy, RecordBid,
    RegisterParticipant, ResumeInvocation, RootTerminal, SettleOffer, StartAttempt,
    StartInvocation, SubmitResult, WakeCondition, WithdrawOffer, YieldInvocation,
};

const ROOT: &str = "sponsor-root";
const ROOT_PRINCIPAL: &str = "principal-root";
const ROOT_OBLIGATION: &str = "obligation-root";
const ALPHA: &str = "p-alpha";
const OFFER: &str = "offer-one";
const BID: &str = "bid-one";
const CONTRACT: &str = "contract-one";
const OBLIGATION: &str = "obligation-one";
const ATTEMPT: &str = "attempt-one";
const INVOCATION: &str = "invocation-one";
const CLASS: &str = "class-under-test";
const SCOPE: &str = "scope-under-test";
const DEADLINE: u64 = 10_000;
const LEASE_MS: u64 = 5_000;

fn digest(tag: &str) -> String {
    ymp_domain::digest_bytes(tag.as_bytes())
}

fn root_budget() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 1_000_000)
        .with(Dimension::ModelTokens, 1_000_000)
        .with(Dimension::WallTimeMs, 1_000_000)
        .with(Dimension::VerificationQueries, 100)
        .with(Dimension::ParticipantStarts, 8)
        .with(Dimension::AttemptStarts, 100)
        .with(Dimension::InvocationStarts, 100)
        .with(Dimension::OfferCreations, 100)
        .with(Dimension::ObligationCreations, 100)
}

fn escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 10_000)
        .with(Dimension::ModelTokens, 10_000)
        .with(Dimension::WallTimeMs, 20_000)
        .with(Dimension::VerificationQueries, 2)
        .with(Dimension::AttemptStarts, 4)
        .with(Dimension::InvocationStarts, 8)
}

/// A service holding one participant under one task contract with one attempt open on it.
fn prepared() -> CommitmentService {
    let service = CommitmentService::new(
        CommitmentLedger::new(ROOT, ROOT_PRINCIPAL, ROOT_OBLIGATION, root_budget())
            .expect("root ledger"),
    );
    let commands = [
        CommitmentCommand::RegisterParticipant(RegisterParticipant {
            participant_id: ALPHA.to_owned(),
            principal_id: "principal-alpha".to_owned(),
            sponsor: ROOT.to_owned(),
            endowment: BudgetVector::ZERO,
        }),
        CommitmentCommand::Advertise(Advertise {
            offer_id: OFFER.to_owned(),
            sponsor: ROOT.to_owned(),
            parent_obligation: ROOT_OBLIGATION.to_owned(),
            funding_source: FundingSource::Participant,
            task_scope: SCOPE.to_owned(),
            base_digest: digest("base"),
            intent_digest: digest("intent"),
            artifact_class: CLASS.to_owned(),
            dependencies: Vec::new(),
            capability_scope: Vec::new(),
            execution_escrow: escrow(),
            policy: OfferPolicy::Negotiated,
            bid_deadline: DEADLINE,
            offer_deadline: DEADLINE,
            max_awards: 1,
        }),
        CommitmentCommand::RecordBid(RecordBid {
            bid_id: BID.to_owned(),
            offer_id: OFFER.to_owned(),
            bidder: ALPHA.to_owned(),
            requested_escrow: escrow(),
            artifact_class: CLASS.to_owned(),
            proposal_digest: None,
            expires_at: DEADLINE,
        }),
        CommitmentCommand::Award(Award {
            contract_id: CONTRACT.to_owned(),
            obligation_id: OBLIGATION.to_owned(),
            lease_id: "lease-one".to_owned(),
            offer_id: OFFER.to_owned(),
            bid_id: BID.to_owned(),
            sponsor: ROOT.to_owned(),
            lease_ms: LEASE_MS,
        }),
        CommitmentCommand::StartAttempt(StartAttempt {
            attempt_id: ATTEMPT.to_owned(),
            contract_id: CONTRACT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
        }),
    ];
    for (index, command) in commands.iter().enumerate() {
        service
            .execute(&format!("prefix-{index}"), command)
            .expect("the prefix is well formed");
    }
    service
}

fn start_slice(cursor: u64) -> CommitmentCommand {
    CommitmentCommand::StartInvocation(StartInvocation {
        invocation_id: INVOCATION.to_owned(),
        attempt_id: ATTEMPT.to_owned(),
        contract_id: CONTRACT.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        cursor,
    })
}

fn yield_slice(cursor: u64) -> CommitmentCommand {
    CommitmentCommand::YieldInvocation(YieldInvocation {
        invocation_id: INVOCATION.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        cursor,
        conditions: vec![WakeCondition::SubmissionRecorded {
            contract_id: CONTRACT.to_owned(),
        }],
        wake_deadline: LEASE_MS,
    })
}

fn submit(tag: &str) -> CommitmentCommand {
    CommitmentCommand::SubmitResult(SubmitResult {
        contract_id: CONTRACT.to_owned(),
        attempt_id: ATTEMPT.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        candidate_digest: digest(tag),
    })
}

/// A reader that received nothing reaches the same conclusion as one that received everything, and
/// recovers exactly the facts it missed.
#[test]
fn a_reader_that_lost_every_notification_recovers_from_its_cursor() {
    let service = prepared();
    // One slot, and nobody drains it: after the first number every later one is dropped. That is
    // what coalescing and loss look like from the inside.
    let deaf = service.subscribe(1).expect("channel");
    let attentive = service.subscribe(64).expect("channel");

    service.execute("start", &start_slice(0)).expect("slice");
    let cursor = service.committed_facts().expect("log") as u64;
    service
        .execute("yield", &yield_slice(cursor))
        .expect("yield");
    assert!(
        service.admission_order().expect("order").is_empty(),
        "nothing the yield asked about has happened yet"
    );

    // A task contract records one result, so what arrives three times is the same submission
    // rather than three different ones: a retry after a lost answer is exactly how one command
    // reaches the committed stream more than once.
    for index in 0..3 {
        service
            .execute(&format!("submit-{index}"), &submit("first"))
            .expect("submission");
    }

    let lost: Vec<u64> = std::iter::from_fn(|| deaf.try_recv().ok()).collect();
    let received: Vec<u64> = std::iter::from_fn(|| attentive.try_recv().ok()).collect();
    assert!(
        lost.len() < received.len(),
        "the one-slot channel must actually have dropped numbers: {lost:?} against {received:?}"
    );
    assert_eq!(deaf.try_recv(), Err(TryRecvError::Empty));

    // Both readers ask the same question of the committed stream and are told the same thing.
    assert_eq!(
        service.admission_order().expect("order"),
        vec![INVOCATION.to_owned()],
        "several matching facts are one answer, and losing the notifications did not change it"
    );
    let missed = service.facts_after(cursor).expect("facts");
    assert_eq!(
        missed
            .iter()
            .filter(|fact| matches!(fact.event, CommitmentEvent::SubmissionRecorded { .. }))
            .count(),
        3,
        "the reader recovers every fact after its cursor, not only the ones it was told about"
    );
    assert!(missed.iter().all(|fact| fact.sequence > cursor));

    service
        .execute("resume", &resume())
        .expect("one admission for however many facts matched");
    assert!(
        service.admission_order().expect("order").is_empty(),
        "a running slice is not waiting for anything"
    );
}

fn resume() -> CommitmentCommand {
    CommitmentCommand::ResumeInvocation(ResumeInvocation {
        invocation_id: INVOCATION.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
    })
}

/// The same command delivered twice inside one controller interval has one effect, one sequence and
/// one set of facts. A different command under the same identifier is refused outright.
#[test]
fn a_repeated_delivery_of_one_command_has_one_effect() {
    let service = prepared();
    service.execute("start", &start_slice(0)).expect("slice");
    let cursor = service.committed_facts().expect("log") as u64;
    service
        .execute("yield", &yield_slice(cursor))
        .expect("yield");
    service
        .execute("submit", &submit("one"))
        .expect("submission");

    let first = service.execute("wake", &resume()).expect("admission");
    assert!(!first.replayed);
    let committed = service.committed_facts().expect("log");
    let again = service
        .execute("wake", &resume())
        .expect("repeated delivery");
    assert!(again.replayed);
    assert_eq!(again.first_sequence, first.first_sequence);
    assert_eq!(again.events, first.events);
    assert_eq!(
        service.committed_facts().expect("log"),
        committed,
        "a repeated delivery commits no second fact"
    );

    assert!(matches!(
        service.execute(
            "wake",
            &CommitmentCommand::CloseInvocation(CloseInvocation {
                invocation_id: INVOCATION.to_owned(),
                closer: ALPHA.to_owned(),
                reason: InvocationClosure::Completed,
            })
        ),
        Err(CommitmentServiceError::IdempotencyConflict { .. })
    ));
}

/// The sequence a reader records as its cursor is the position of that fact in the committed
/// stream, and the two accounts of that stream agree fact for fact.
#[test]
fn recorded_sequences_are_positions_in_the_committed_stream() {
    let service = prepared();
    service.execute("start", &start_slice(0)).expect("slice");
    let facts = service.facts_after(0).expect("facts");
    let ledger = service.snapshot().expect("snapshot");
    assert_eq!(facts.len(), ledger.facts().len());
    for fact in &facts {
        assert_eq!(
            ledger.facts()[fact.sequence as usize - 1],
            fact.event,
            "sequence {} names a different fact in each account",
            fact.sequence
        );
    }
    assert_eq!(ledger.sequence(), facts.len() as u64);
}

/// A run that nobody wakes still stops, and stops as something other than acceptance.
#[test]
fn a_run_nobody_wakes_still_reaches_an_honest_terminal_state() {
    let service = prepared();
    service.execute("start", &start_slice(0)).expect("slice");
    let cursor = service.committed_facts().expect("log") as u64;
    service
        .execute("yield", &yield_slice(cursor))
        .expect("yield");
    assert_eq!(service.root_terminal().expect("terminal"), None);

    // The sponsor records the loss, takes the contract back and settles the reservation. No state
    // is repaired by hand at any point.
    let wind_down = [
        CommitmentCommand::CloseInvocation(CloseInvocation {
            invocation_id: INVOCATION.to_owned(),
            closer: ROOT.to_owned(),
            reason: InvocationClosure::ParticipantLost,
        }),
        CommitmentCommand::CancelContract(ymp_domain::commitment::CancelContract {
            contract_id: CONTRACT.to_owned(),
            sponsor: ROOT.to_owned(),
        }),
        CommitmentCommand::WithdrawOffer(WithdrawOffer {
            offer_id: OFFER.to_owned(),
            sponsor: ROOT.to_owned(),
        }),
        CommitmentCommand::SettleOffer(SettleOffer {
            offer_id: OFFER.to_owned(),
            sponsor: ROOT.to_owned(),
        }),
    ];
    for (index, command) in wind_down.iter().enumerate() {
        service
            .execute(&format!("wind-{index}"), command)
            .expect("winding a run down needs only the commands the protocol already has");
    }
    assert_eq!(service.open_authority().expect("authority"), None);
    assert_eq!(
        service.root_terminal().expect("terminal"),
        Some(RootTerminal::Exhausted),
        "quiet is not acceptance"
    );
}

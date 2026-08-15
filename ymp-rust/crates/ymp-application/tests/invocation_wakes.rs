//! Delivery is allowed to fail; the committed stream is not.
//!
//! The domain suite proves that a resumption needs a committed fact and that a run stops honestly.
//! This suite proves the half that only exists once real readers and real channels are involved:
//! that a notification channel carries no state anybody depends on, that a reader which received
//! every notification and one which received none reach the same conclusion, and that the same
//! command delivered twice inside one controller interval has one effect and one sequence.
//!
//! Everything here goes through the boundary a run has: [`Application::execute_commitment`], which
//! decides against the ledger folded out of the run's own journal and records what it commits there
//! before answering. What a reader recovers is therefore read back out of that journal, in a
//! temporary store of the test's own.

use std::sync::mpsc::TryRecvError;

use tempfile::TempDir;
use ymp_application::{Application, ApplicationError, CommitmentOutcome};
use ymp_domain::commitment::{
    Advertise, Award, BudgetVector, CloseInvocation, CommitmentCommand, CommitmentEvent, Dimension,
    FundingSource, InvocationClosure, OfferPolicy, RecordBid, RegisterParticipant,
    ResumeInvocation, RootTerminal, SettleOffer, StartAttempt, StartInvocation, SubmitResult,
    WakeCondition, WithdrawOffer, YieldInvocation,
};
use ymp_domain::{Budget, EventKind};

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

/// One run whose kernel holds one participant under one task contract with one attempt open on it.
///
/// The directory is returned with the run: the journal is a file in it, and a store dropped while
/// the run is still in use would take the record with it.
fn prepared() -> (TempDir, Application) {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let mut application =
        Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
            .expect("create the run");
    application
        .open_commitment_kernel(
            "kernel",
            ROOT,
            ROOT_PRINCIPAL,
            ROOT_OBLIGATION,
            root_budget(),
        )
        .expect("the run's ledger");
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
        application
            .execute_commitment(format!("prefix-{index}"), command)
            .expect("the prefix is well formed");
    }
    (temporary, application)
}

/// One committed fact in the order it was committed, read back out of the run's journal.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RecordedFact {
    sequence: u64,
    event: CommitmentEvent,
}

/// The facts the run committed after a cursor, which is how a lagging reader recovers rather than
/// assuming it saw every notification.
fn facts_after(application: &Application, cursor: u64) -> Vec<RecordedFact> {
    let mut facts = Vec::new();
    for envelope in application
        .events_after(0)
        .expect("read the committed records")
    {
        let EventKind::CommitmentFactsRecorded { facts: committed } = envelope.event else {
            continue;
        };
        for event in committed {
            facts.push(RecordedFact {
                sequence: facts.len() as u64 + 1,
                event,
            });
        }
    }
    facts.retain(|fact| fact.sequence > cursor);
    facts
}

fn committed_facts(application: &Application) -> u64 {
    application
        .commitments()
        .expect("the run carries a kernel")
        .sequence()
}

fn admission_order(application: &Application) -> Vec<String> {
    application
        .commitments()
        .expect("the run carries a kernel")
        .admission_order()
        .into_iter()
        .map(|invocation| invocation.invocation_id.clone())
        .collect()
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
    let (_store, mut application) = prepared();
    // One slot, and nobody drains it: after the first number every later one is dropped. That is
    // what coalescing and loss look like from the inside.
    let deaf = application.subscribe(1).expect("channel");
    let attentive = application.subscribe(64).expect("channel");

    application
        .execute_commitment("start", &start_slice(0))
        .expect("slice");
    let cursor = committed_facts(&application);
    application
        .execute_commitment("yield", &yield_slice(cursor))
        .expect("yield");
    assert!(
        admission_order(&application).is_empty(),
        "nothing the yield asked about has happened yet"
    );

    // A task contract records one result, so what arrives three times is the same submission
    // rather than three different ones: a retry after a lost answer is exactly how one command
    // reaches the committed stream more than once.
    for index in 0..3 {
        application
            .execute_commitment(format!("submit-{index}"), &submit("first"))
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
        admission_order(&application),
        vec![INVOCATION.to_owned()],
        "several matching facts are one answer, and losing the notifications did not change it"
    );
    let missed = facts_after(&application, cursor);
    assert_eq!(
        missed
            .iter()
            .filter(|fact| matches!(fact.event, CommitmentEvent::SubmissionRecorded { .. }))
            .count(),
        3,
        "the reader recovers every fact after its cursor, not only the ones it was told about"
    );
    assert!(missed.iter().all(|fact| fact.sequence > cursor));

    application
        .execute_commitment("resume", &resume())
        .expect("one admission for however many facts matched");
    assert!(
        admission_order(&application).is_empty(),
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
    let (_store, mut application) = prepared();
    application
        .execute_commitment("start", &start_slice(0))
        .expect("slice");
    let cursor = committed_facts(&application);
    application
        .execute_commitment("yield", &yield_slice(cursor))
        .expect("yield");
    application
        .execute_commitment("submit", &submit("one"))
        .expect("submission");

    let first: CommitmentOutcome = application
        .execute_commitment("wake", &resume())
        .expect("admission");
    assert!(!first.replayed);
    let committed = committed_facts(&application);
    let again = application
        .execute_commitment("wake", &resume())
        .expect("repeated delivery");
    assert!(again.replayed);
    assert_eq!(again.first_sequence, first.first_sequence);
    assert_eq!(again.events, first.events);
    assert_eq!(
        committed_facts(&application),
        committed,
        "a repeated delivery commits no second fact"
    );

    assert!(matches!(
        application.execute_commitment(
            "wake",
            &CommitmentCommand::CloseInvocation(CloseInvocation {
                invocation_id: INVOCATION.to_owned(),
                closer: ALPHA.to_owned(),
                reason: InvocationClosure::Completed,
            })
        ),
        Err(ApplicationError::IdempotencyConflict { .. })
    ));
}

/// The sequence a reader records as its cursor is the position of that fact in the committed
/// stream, and the two accounts of that stream agree fact for fact.
#[test]
fn recorded_sequences_are_positions_in_the_committed_stream() {
    let (_store, mut application) = prepared();
    application
        .execute_commitment("start", &start_slice(0))
        .expect("slice");
    let facts = facts_after(&application, 0);
    let ledger = application
        .commitments()
        .expect("the run carries a kernel")
        .clone();
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
    let (_store, mut application) = prepared();
    application
        .execute_commitment("start", &start_slice(0))
        .expect("slice");
    let cursor = committed_facts(&application);
    application
        .execute_commitment("yield", &yield_slice(cursor))
        .expect("yield");
    assert_eq!(
        application
            .commitments()
            .expect("the run carries a kernel")
            .root_terminal(),
        None
    );

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
        application
            .execute_commitment(format!("wind-{index}"), command)
            .expect("winding a run down needs only the commands the protocol already has");
    }
    let ledger = application
        .commitments()
        .expect("the run carries a kernel")
        .clone();
    assert_eq!(ledger.open_authority(), None);
    assert_eq!(
        ledger.root_terminal(),
        Some(RootTerminal::Exhausted),
        "quiet is not acceptance"
    );
}

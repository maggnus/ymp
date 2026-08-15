//! Acceptance: a commitment fact is durable before it is reported, and a restart rebuilds the
//! ledger from the journal and from nothing else.
//!
//! Three halves are asserted here. Every fact a command commits is in the journal file by the
//! time the command answers, so a caller that has been told escrow moved is holding something the
//! record already states. A restart reads the store back and reaches the same ledger, fact for
//! fact, without any live state crossing over. And a repeated command identifier is answered from
//! the record instead of committing a second effect.
//!
//! One more half is asserted here: a store carrying two records that each open a kernel is refused
//! rather than read from whichever came first.
//!
//! The check that must fail: drop the journal write from the commitment path — remove the
//! `self.commit(...)` call in `Application::execute_commitment`, or make it append nothing — and
//! `a_fact_is_in_the_journal_before_the_command_answers` reports it with a non-zero exit, because
//! the store reopened at the end holds a ledger the run had already been told about.

use std::fs::OpenOptions;
use std::io::Write;

use tempfile::{TempDir, tempdir};
use ymp_application::{Application, ApplicationError};
use ymp_domain::commitment::{
    Advertise, Award, BudgetVector, CommitmentCommand, CommitmentError, CommitmentEvent, Dimension,
    FundingSource, OfferPolicy, RecordBid, RegisterParticipant,
};
use ymp_domain::{Budget, EventEnvelope, EventKind};

const SPONSOR: &str = "sponsor-root";
const SPONSOR_PRINCIPAL: &str = "principal-root";
const ROOT_OBLIGATION: &str = "obligation-root";
const CONTRACTOR: &str = "participant-one";
const OFFER: &str = "offer-one";
const BID: &str = "bid-one";
const CONTRACT: &str = "contract-one";
const CLASS: &str = "class-one";
const DEADLINE: u64 = 100_000;
const LEASE_MS: u64 = 1_000;

fn digest(tag: &str) -> String {
    ymp_domain::digest_bytes(tag.as_bytes())
}

fn capacity(units: u64) -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, units)
        .with(Dimension::ModelTokens, units)
        .with(Dimension::WallTimeMs, units)
}

fn root_budget() -> BudgetVector {
    capacity(1_000_000)
        .with(Dimension::VerificationQueries, 8)
        .with(Dimension::ParticipantStarts, 8)
        .with(Dimension::AttemptStarts, 16)
        .with(Dimension::OfferCreations, 8)
        .with(Dimension::ObligationCreations, 8)
}

/// The commands that form one task contract, in the order the kernel admits them.
fn forming_commands() -> Vec<(&'static str, CommitmentCommand)> {
    vec![
        (
            "register",
            CommitmentCommand::RegisterParticipant(RegisterParticipant {
                participant_id: CONTRACTOR.to_owned(),
                principal_id: "principal-one".to_owned(),
                sponsor: SPONSOR.to_owned(),
                endowment: capacity(50_000)
                    .with(Dimension::AttemptStarts, 4)
                    .with(Dimension::OfferCreations, 2)
                    .with(Dimension::ObligationCreations, 2),
            }),
        ),
        (
            "advertise",
            CommitmentCommand::Advertise(Advertise {
                offer_id: OFFER.to_owned(),
                sponsor: SPONSOR.to_owned(),
                parent_obligation: ROOT_OBLIGATION.to_owned(),
                funding_source: FundingSource::Participant,
                task_scope: "scope-one".to_owned(),
                base_digest: digest("base"),
                intent_digest: digest("intent"),
                artifact_class: CLASS.to_owned(),
                dependencies: Vec::new(),
                capability_scope: Vec::new(),
                execution_escrow: capacity(20_000).with(Dimension::AttemptStarts, 2),
                policy: OfferPolicy::Negotiated,
                bid_deadline: DEADLINE,
                offer_deadline: DEADLINE,
                max_awards: 1,
            }),
        ),
        (
            "bid",
            CommitmentCommand::RecordBid(RecordBid {
                bid_id: BID.to_owned(),
                offer_id: OFFER.to_owned(),
                bidder: CONTRACTOR.to_owned(),
                requested_escrow: capacity(20_000).with(Dimension::AttemptStarts, 2),
                artifact_class: CLASS.to_owned(),
                proposal_digest: Some(digest("proposal")),
                expires_at: DEADLINE,
            }),
        ),
        (
            "award",
            CommitmentCommand::Award(Award {
                contract_id: CONTRACT.to_owned(),
                obligation_id: "obligation-one".to_owned(),
                lease_id: "lease-one".to_owned(),
                offer_id: OFFER.to_owned(),
                bid_id: BID.to_owned(),
                sponsor: SPONSOR.to_owned(),
                lease_ms: LEASE_MS,
            }),
        ),
    ]
}

/// The facts the journal file holds right now, read back from the store rather than from memory.
fn journalled_facts(application: &Application) -> Vec<CommitmentEvent> {
    application
        .events_after(0)
        .expect("read the committed records")
        .into_iter()
        .filter_map(|envelope| match envelope.event {
            EventKind::CommitmentFactsRecorded { facts } => Some(facts),
            _ => None,
        })
        .flatten()
        .collect()
}

/// A store carrying a run and an opened commitment kernel.
fn opened() -> (TempDir, Application) {
    let temporary = tempdir().expect("temporary directory");
    let mut application =
        Application::create(temporary.path(), "run-1", Budget::new(2, 1)).expect("create the run");
    application
        .open_commitment_kernel(
            "kernel",
            SPONSOR,
            SPONSOR_PRINCIPAL,
            ROOT_OBLIGATION,
            root_budget(),
        )
        .expect("open the commitment kernel");
    (temporary, application)
}

#[test]
fn a_fact_is_in_the_journal_before_the_command_answers() {
    let (temporary, mut application) = opened();
    let mut expected: Vec<CommitmentEvent> = Vec::new();
    let mut next_sequence = 1;

    for (command_id, command) in forming_commands() {
        let outcome = application
            .execute_commitment(command_id, &command)
            .expect("a legal commitment command");
        assert!(!outcome.replayed);
        assert_eq!(
            outcome.first_sequence, next_sequence,
            "{command_id} answered with a run sequence the ledger does not stand at"
        );
        next_sequence += outcome.events.len() as u64;
        expected.extend(outcome.events.clone());

        // The store on disk, reread, already holds every fact the caller was just told about.
        assert_eq!(
            journalled_facts(&application),
            expected,
            "{command_id} answered before its facts were in the journal"
        );
    }

    assert!(
        expected
            .iter()
            .any(|fact| matches!(fact, CommitmentEvent::TaskContractFormed { .. })),
        "the sequence under test never formed a task contract"
    );
    assert!(
        expected
            .iter()
            .any(|fact| matches!(fact, CommitmentEvent::BudgetTransferred { .. })),
        "the sequence under test never moved escrow"
    );

    // And the store answers the same after the process that wrote it is gone.
    let held = application
        .commitments()
        .expect("the run carries a kernel")
        .clone();
    drop(application);
    let reopened = Application::open(temporary.path()).expect("reopen the store");
    assert_eq!(
        reopened.commitments().expect("the kernel is recovered"),
        &held,
        "a fact the run reported did not survive the restart"
    );
}

#[test]
fn a_restart_rebuilds_the_ledger_from_the_journal_alone() {
    let (temporary, mut application) = opened();
    for (command_id, command) in forming_commands() {
        application
            .execute_commitment(command_id, &command)
            .expect("a legal commitment command");
    }
    let held = application
        .commitments()
        .expect("the run carries a kernel")
        .clone();
    let recorded = journalled_facts(&application);
    drop(application);

    let reopened = Application::open(temporary.path()).expect("reopen the store");
    let recovered = reopened.commitments().expect("the kernel is recovered");
    assert_eq!(recovered, &held);
    // Fact for fact, in order: what the recovered ledger holds is the journal's own record and
    // not an accumulation beside it.
    assert_eq!(recovered.facts(), recorded.as_slice());
    assert_eq!(recovered.sequence(), recorded.len() as u64);
    assert!(recovered.contracts().contains_key(CONTRACT));
    assert_eq!(
        recovered.contracts()[CONTRACT].contractor,
        CONTRACTOR,
        "the recovered contract names a different holder"
    );
}

#[test]
fn a_repeated_command_after_a_restart_commits_nothing_further() {
    let (temporary, mut application) = opened();
    let commands = forming_commands();
    for (command_id, command) in &commands {
        application
            .execute_commitment(*command_id, command)
            .expect("a legal commitment command");
    }
    let records = application
        .events_after(0)
        .expect("read the committed records")
        .len();
    drop(application);

    let mut reopened = Application::open(temporary.path()).expect("reopen the store");
    let (command_id, command) = &commands[commands.len() - 1];
    let replayed = reopened
        .execute_commitment(*command_id, command)
        .expect("the repeated command is answered from the record");
    assert!(replayed.replayed);
    assert!(
        replayed
            .events
            .iter()
            .any(|fact| matches!(fact, CommitmentEvent::TaskContractFormed { .. })),
        "the replay answered without the facts the first delivery committed"
    );
    assert_eq!(
        reopened
            .events_after(0)
            .expect("read the committed records")
            .len(),
        records,
        "the repeated command wrote a second record"
    );

    // The same identifier carrying different content is refused rather than committed.
    let (_, other) = &commands[0];
    assert!(matches!(
        reopened.execute_commitment(*command_id, other),
        Err(ApplicationError::IdempotencyConflict { .. })
    ));
}

/// A store that opens a kernel twice is refused, rather than rebuilt from the first record.
///
/// The genesis states what a ledger is built from, and two of them state two different starting
/// points for one run: the facts that follow were decided against one of the two and cannot be
/// replayed into both. Reading the first and passing over the second rebuilt a run from half of
/// what its journal holds, and reported nothing about the half it ignored.
///
/// The second record is appended to the journal directly, because nothing the application offers
/// writes one — a second opening is refused before it reaches the record. What is measured here is
/// the reading of a store that carries it, which is where such a record would actually arrive.
///
/// The check that must fail: pass over a genesis that arrives with a kernel already open, and the
/// store below is read as a run whose ledger is the first record's, with no refusal anywhere.
#[test]
fn a_store_that_opens_a_kernel_twice_is_refused_rather_than_read_from_the_first() {
    let (temporary, mut application) = opened();
    for (command_id, command) in forming_commands() {
        application
            .execute_commitment(command_id, &command)
            .expect("a legal commitment command");
    }
    let records = application
        .events_after(0)
        .expect("read the committed records");
    let last = records.last().expect("the store holds records").clone();
    let genesis = records
        .iter()
        .find(|envelope| matches!(envelope.event, EventKind::CommitmentKernelOpened { .. }))
        .expect("the store holds the record that opened the kernel")
        .event
        .clone();
    drop(application);

    let repeated = EventEnvelope::new(
        &last.run_id,
        last.sequence + 1,
        "kernel-again",
        ymp_domain::digest_bytes(&serde_json::to_vec(&genesis).expect("the genesis event")),
        Some(last.digest.clone()),
        genesis,
    )
    .expect("a second record opening a kernel");
    let mut journal = OpenOptions::new()
        .append(true)
        .open(temporary.path().join("events.jsonl"))
        .expect("the journal file");
    writeln!(
        journal,
        "{}",
        serde_json::to_string(&repeated).expect("the record")
    )
    .expect("append the second genesis");
    drop(journal);

    let refused = Application::open(temporary.path())
        .err()
        .expect("a store that opens a kernel twice was read as a run");
    assert!(
        matches!(
            refused,
            ApplicationError::CommitmentKernelAlreadyOpen { ref root_obligation }
                if root_obligation == ROOT_OBLIGATION
        ),
        "the store was refused for another reason: {refused}"
    );
}

#[test]
fn a_refused_commitment_command_writes_no_record_and_leaves_the_ledger_where_it_stood() {
    let (_temporary, mut application) = opened();
    for (command_id, command) in forming_commands() {
        application
            .execute_commitment(command_id, &command)
            .expect("a legal commitment command");
    }
    let records = application
        .events_after(0)
        .expect("read the committed records")
        .len();
    let held = application
        .commitments()
        .expect("the run carries a kernel")
        .clone();

    // The one funded slot of the offer is already awarded, so a second award is refused.
    let (_, award) = &forming_commands()[3];
    let CommitmentCommand::Award(award) = award else {
        panic!("the fourth command of the sequence is the award");
    };
    let second = CommitmentCommand::Award(Award {
        contract_id: "contract-two".to_owned(),
        obligation_id: "obligation-two".to_owned(),
        lease_id: "lease-two".to_owned(),
        ..award.clone()
    });
    assert!(matches!(
        application.execute_commitment("award-again", &second),
        Err(ApplicationError::Commitment(
            CommitmentError::BidNotLive { .. }
        ))
    ));

    assert_eq!(
        application
            .events_after(0)
            .expect("read the committed records")
            .len(),
        records,
        "a refused command left a record behind"
    );
    assert_eq!(
        application.commitments().expect("the kernel is open"),
        &held,
        "a refused command changed the ledger"
    );
}

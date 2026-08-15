//! Contention against one run's commitment kernel from real threads.
//!
//! The domain suite proves that no *ordering* of contending commands can overspend a reservation
//! or duplicate an obligation. This suite proves the other half: that concurrent callers are
//! actually reduced to one such ordering, that a repeated command identifier commits nothing a
//! second time, and that a refusal consumes no sequence and leaves no fact.
//!
//! The boundary under test is the one a run has: [`Application::execute_commitment`], deciding
//! against the ledger folded out of the run's own journal and recording what it commits there
//! before answering. Every run in here has a store of its own in a temporary directory, and the
//! accounts are rebuilt from the records that store holds rather than from anything a process kept
//! beside them.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Barrier, Mutex};

use tempfile::TempDir;
use ymp_application::{Application, ApplicationError, CommitmentOutcome};
use ymp_domain::commitment::{
    AcceptOpen, AccountRef, Advertise, Award, BudgetVector, CommitmentCommand, CommitmentError,
    CommitmentEvent, CommitmentLedger, ContractState, DIMENSION_COUNT, DIMENSIONS, Dimension,
    FundingSource, OfferPolicy, OfferState, Outcome, RecordBid, RegisterParticipant,
    ReturnObligation, SettleOffer, StartAttempt, StopReason, StopRun, WithdrawOffer,
};
use ymp_domain::{Budget, Command, EventKind, RunStatus, TransitionError};

const ROOT: &str = "sponsor-root";
const ROOT_PRINCIPAL: &str = "principal-root";
const ROOT_OBLIGATION: &str = "obligation-root";
const MAIN_OFFER: &str = "offer-main";
const OPEN_OFFER: &str = "offer-open";
const MAIN_MAX_AWARDS: u32 = 2;
const OPEN_MAX_AWARDS: u32 = 1;
const CONTENDERS: usize = 8;
const LEASE_MS: u64 = 100;
const DEADLINE: u64 = 10_000;
const CLASS: &str = "class-under-test";

fn digest(tag: &str) -> String {
    ymp_domain::digest_bytes(tag.as_bytes())
}

fn root_budget() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 10_000_000)
        .with(Dimension::ModelTokens, 10_000_000)
        .with(Dimension::WallTimeMs, 10_000_000)
        .with(Dimension::VerificationQueries, 1_000)
        .with(Dimension::ExternalActions, 1_000)
        .with(Dimension::ParticipantStarts, 64)
        .with(Dimension::AttemptStarts, 1_000)
        .with(Dimension::OfferCreations, 1_000)
        .with(Dimension::ObligationCreations, 1_000)
}

fn endowment() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 50_000)
        .with(Dimension::ModelTokens, 50_000)
        .with(Dimension::WallTimeMs, 50_000)
        .with(Dimension::AttemptStarts, 5)
        .with(Dimension::OfferCreations, 5)
        .with(Dimension::ObligationCreations, 5)
}

/// What one award funds. Every bid asks for a fraction of it, so the pool keeps slack: an extra
/// award has to be stopped by the funded count and not by running out of money.
fn execution_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 20_000)
        .with(Dimension::ModelTokens, 20_000)
        .with(Dimension::WallTimeMs, 20_000)
        .with(Dimension::AttemptStarts, 4)
}

fn requested_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 1_000)
        .with(Dimension::ModelTokens, 1_000)
        .with(Dimension::WallTimeMs, 1_000)
        .with(Dimension::AttemptStarts, 1)
}

fn contender(index: usize) -> String {
    format!("p-{index}")
}

fn advertise(offer_id: &str, policy: OfferPolicy, max_awards: u32) -> CommitmentCommand {
    CommitmentCommand::Advertise(Advertise {
        offer_id: offer_id.to_owned(),
        sponsor: ROOT.to_owned(),
        parent_obligation: ROOT_OBLIGATION.to_owned(),
        funding_source: FundingSource::Participant,
        task_scope: "scope-under-test".to_owned(),
        base_digest: digest("base"),
        intent_digest: digest("intent"),
        artifact_class: CLASS.to_owned(),
        dependencies: Vec::new(),
        capability_scope: Vec::new(),
        execution_escrow: execution_escrow(),
        policy,
        bid_deadline: DEADLINE,
        offer_deadline: DEADLINE,
        max_awards,
    })
}

/// One run with a kernel of its own, holding two offers — one negotiated with recorded consent from
/// every contender, one open — and nothing awarded yet.
///
/// The directory is returned with the run: the journal is a file in it, and a store dropped while
/// the run is still in use would take the record with it.
fn prepared() -> (TempDir, Arc<Mutex<Application>>) {
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
    let application = Arc::new(Mutex::new(application));
    for index in 0..CONTENDERS {
        execute(
            &application,
            &format!("register-{index}"),
            &CommitmentCommand::RegisterParticipant(RegisterParticipant {
                participant_id: contender(index),
                principal_id: format!("principal-{index}"),
                sponsor: ROOT.to_owned(),
                endowment: endowment(),
            }),
        )
        .expect("registration");
    }
    execute(
        &application,
        "advertise-main",
        &advertise(MAIN_OFFER, OfferPolicy::Negotiated, MAIN_MAX_AWARDS),
    )
    .expect("negotiated offer");
    execute(
        &application,
        "advertise-open",
        &advertise(OPEN_OFFER, OfferPolicy::OpenAccept, OPEN_MAX_AWARDS),
    )
    .expect("open offer");
    for index in 0..CONTENDERS {
        execute(
            &application,
            &format!("bid-{index}"),
            &CommitmentCommand::RecordBid(RecordBid {
                bid_id: format!("bid-{index}"),
                offer_id: MAIN_OFFER.to_owned(),
                bidder: contender(index),
                requested_escrow: requested_escrow(),
                artifact_class: CLASS.to_owned(),
                proposal_digest: Some(digest(&format!("proposal-{index}"))),
                expires_at: DEADLINE,
            }),
        )
        .expect("consent");
    }
    (temporary, application)
}

/// Decide and record one command against the run's ledger.
fn execute(
    application: &Arc<Mutex<Application>>,
    command_id: &str,
    command: &CommitmentCommand,
) -> Result<CommitmentOutcome, ApplicationError> {
    application
        .lock()
        .expect("the run")
        .execute_commitment(command_id, command)
}

/// A consistent read of the run's whole ledger, taken from the journal the run writes.
fn snapshot(application: &Arc<Mutex<Application>>) -> CommitmentLedger {
    application
        .lock()
        .expect("the run")
        .recovered_commitments()
        .expect("the record is readable")
        .expect("the run carries a kernel")
        .clone()
}

/// One committed fact in the order it was committed, with the command that committed it.
///
/// It is read back out of the run's journal rather than out of anything held beside it: each
/// commitment record names the command that wrote it and carries the facts that command committed,
/// in order, so the position of a fact in the stream is where counting them puts it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RecordedFact {
    sequence: u64,
    command_id: String,
    event: CommitmentEvent,
}

fn facts_after(application: &Arc<Mutex<Application>>, cursor: u64) -> Vec<RecordedFact> {
    let records = application
        .lock()
        .expect("the run")
        .events_after(0)
        .expect("read the committed records");
    let mut facts = Vec::new();
    for envelope in records {
        let EventKind::CommitmentFactsRecorded { facts: committed } = envelope.event else {
            continue;
        };
        for event in committed {
            facts.push(RecordedFact {
                sequence: facts.len() as u64 + 1,
                command_id: envelope.command_id.clone(),
                event,
            });
        }
    }
    facts.retain(|fact| fact.sequence > cursor);
    facts
}

fn committed_facts(application: &Arc<Mutex<Application>>) -> usize {
    snapshot(application).sequence() as usize
}

fn award(index: usize) -> CommitmentCommand {
    CommitmentCommand::Award(Award {
        contract_id: format!("contract-{index}"),
        obligation_id: format!("obligation-{index}"),
        lease_id: format!("lease-{index}"),
        offer_id: MAIN_OFFER.to_owned(),
        bid_id: format!("bid-{index}"),
        sponsor: ROOT.to_owned(),
        lease_ms: LEASE_MS,
    })
}

fn accept_open(index: usize) -> CommitmentCommand {
    CommitmentCommand::AcceptOpen(AcceptOpen {
        contract_id: format!("contract-open-{index}"),
        obligation_id: format!("obligation-open-{index}"),
        lease_id: format!("lease-open-{index}"),
        bid_id: format!("bid-open-{index}"),
        offer_id: OPEN_OFFER.to_owned(),
        participant: contender(index),
        requested_escrow: requested_escrow(),
        artifact_class: CLASS.to_owned(),
        proposal_digest: None,
        lease_ms: LEASE_MS,
    })
}

/// Run one command per thread, released together, and report what each of them was told.
fn race<F>(
    application: &Arc<Mutex<Application>>,
    command: F,
) -> Vec<Result<CommitmentOutcome, ApplicationError>>
where
    F: Fn(usize) -> (String, CommitmentCommand) + Sync,
{
    let barrier = Arc::new(Barrier::new(CONTENDERS));
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..CONTENDERS)
            .map(|index| {
                let barrier = Arc::clone(&barrier);
                let command = &command;
                scope.spawn(move || {
                    let (command_id, command) = command(index);
                    barrier.wait();
                    execute(application, &command_id, &command)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("contending thread"))
            .collect()
    })
}

/// Every account of the run as the committed facts describe it, added up by this test rather than
/// read out of the ledger the run holds.
///
/// Adding up those records and comparing the sum with the budget the run began with states nothing:
/// each movement takes capacity out of one record and puts the same quantity into another, so that
/// sum is the opening budget for any run at all, including one that lost track of what it moved.
/// What can be false is the comparison below, because the two sides are built by different code —
/// one by the kernel applying each fact to its records, the other here from the same facts as they
/// were committed to the journal.
///
/// The one quantity read from the ledger is the opening budget, which is the premise of the run
/// rather than something a command wrote. Balances are signed, so a movement is followed wherever
/// it leads instead of saturating at zero.
#[derive(Debug)]
struct FactAccounts {
    balances: BTreeMap<AccountRef, [i128; DIMENSION_COUNT]>,
    consumed: [i128; DIMENSION_COUNT],
}

impl FactAccounts {
    fn rebuilt(root: &str, opening: BudgetVector, facts: &[RecordedFact]) -> Self {
        let mut accounts = Self {
            balances: BTreeMap::new(),
            consumed: [0; DIMENSION_COUNT],
        };
        accounts.credit(
            &AccountRef::Participant {
                participant_id: root.to_owned(),
            },
            1,
            &opening,
        );
        for fact in facts {
            accounts.apply(&fact.event);
        }
        accounts
    }

    fn apply(&mut self, event: &CommitmentEvent) {
        match event {
            CommitmentEvent::BudgetTransferred { from, to, amount } => {
                self.credit(from, -1, amount);
                self.credit(to, 1, amount);
            }
            CommitmentEvent::BudgetConsumed { account, amount } => {
                self.credit(account, -1, amount);
                for dimension in DIMENSIONS {
                    self.consumed[dimension.index()] += i128::from(amount.get(dimension));
                }
            }
            _ => {}
        }
    }

    fn credit(&mut self, account: &AccountRef, sign: i128, amount: &BudgetVector) {
        let balance = self
            .balances
            .entry(account.clone())
            .or_insert([0; DIMENSION_COUNT]);
        for dimension in DIMENSIONS {
            balance[dimension.index()] += sign * i128::from(amount.get(dimension));
        }
    }

    /// Where the accounts the facts describe and the accounts the run holds have parted company, in
    /// any dimension of any account, including the capacity that has left the accounts for good.
    fn divergence(&self, ledger: &CommitmentLedger) -> Vec<String> {
        let mut registry: BTreeMap<AccountRef, BudgetVector> = BTreeMap::new();
        for participant in ledger.participants().values() {
            registry.insert(
                AccountRef::Participant {
                    participant_id: participant.participant_id.clone(),
                },
                participant.balance,
            );
        }
        for offer in ledger.offers().values() {
            registry.insert(
                AccountRef::Offer {
                    offer_id: offer.offer_id.clone(),
                },
                offer.escrow,
            );
        }
        for contract in ledger.contracts().values() {
            registry.insert(
                AccountRef::TaskContract {
                    contract_id: contract.contract_id.clone(),
                },
                contract.escrow,
            );
        }
        let mut reported = Vec::new();
        let accounts: BTreeSet<&AccountRef> = self.balances.keys().chain(registry.keys()).collect();
        for account in accounts {
            let facts = self.balances.get(account).copied().unwrap_or_default();
            let held = registry.get(account).copied().unwrap_or_default();
            for dimension in DIMENSIONS {
                let recorded = i128::from(held.get(dimension));
                if facts[dimension.index()] != recorded {
                    reported.push(format!(
                        "{account:?} {dimension}: the facts say {} and the run holds {recorded}",
                        facts[dimension.index()]
                    ));
                }
            }
        }
        for dimension in DIMENSIONS {
            let recorded = i128::from(ledger.consumed().get(dimension));
            if self.consumed[dimension.index()] != recorded {
                reported.push(format!(
                    "consumed {dimension}: the facts say {} and the run holds {recorded}",
                    self.consumed[dimension.index()]
                ));
            }
        }
        reported
    }

    /// What a decided command would take out of an account that never held it. Its facts are
    /// followed into a copy of the accounts, so what is reported is the shortfall those facts
    /// themselves describe.
    fn uncovered(&self, events: &[CommitmentEvent]) -> Vec<String> {
        let mut projected = Self {
            balances: self.balances.clone(),
            consumed: self.consumed,
        };
        let mut reported = Vec::new();
        for event in events {
            projected.apply(event);
        }
        for (account, balance) in &projected.balances {
            for dimension in DIMENSIONS {
                if balance[dimension.index()] < 0 {
                    reported.push(format!(
                        "{account:?} {dimension} would be left at {}",
                        balance[dimension.index()]
                    ));
                }
            }
        }
        reported
    }
}

/// An advertisement reserving a hundred times the money the run was ever funded with. It is never
/// executed: what it is for is to ask the kernel, from whatever state the run has reached, whether
/// it would decide a movement the accounts cannot pay for.
fn oversized_offer() -> CommitmentCommand {
    CommitmentCommand::Advertise(Advertise {
        offer_id: "offer-oversized".to_owned(),
        sponsor: ROOT.to_owned(),
        parent_obligation: ROOT_OBLIGATION.to_owned(),
        funding_source: FundingSource::Participant,
        task_scope: "scope-under-test".to_owned(),
        base_digest: digest("base"),
        intent_digest: digest("intent"),
        artifact_class: CLASS.to_owned(),
        dependencies: Vec::new(),
        capability_scope: Vec::new(),
        execution_escrow: BudgetVector::ZERO.with(Dimension::MoneyMicros, 1_000_000_000),
        policy: OfferPolicy::Negotiated,
        bid_deadline: DEADLINE,
        offer_deadline: DEADLINE,
        max_awards: 1,
    })
}

/// Conservation, asserted against the facts the run committed rather than against the ledger it
/// holds.
///
/// Two things have to hold, and the second is not implied by the first. The accounts rebuilt from
/// the facts must agree with the accounts the run holds; and every movement the kernel is willing
/// to decide must be one those accounts can pay for. A kernel that stopped establishing the
/// capacity before deciding would keep books that add up perfectly — every fact it commits still
/// moves capacity out of one account and into another — while committing facts that move capacity
/// nobody ever held.
fn assert_conserved(application: &Arc<Mutex<Application>>) {
    let ledger = snapshot(application);
    let accounts =
        FactAccounts::rebuilt(ROOT, *ledger.initial_total(), &facts_after(application, 0));
    let divergence = accounts.divergence(&ledger);
    assert!(
        divergence.is_empty(),
        "the accounts the facts describe and the accounts the run keeps disagree: {divergence:?}"
    );

    match ledger.decide(&oversized_offer()) {
        Err(CommitmentError::InsufficientBudget {
            dimension: Dimension::MoneyMicros,
            ..
        }) => {}
        Err(other) => panic!(
            "a reservation larger than the whole run was refused for an unrelated reason: {other}"
        ),
        Ok(events) => {
            let uncovered = accounts.uncovered(&events);
            assert!(
                uncovered.is_empty(),
                "the kernel decided a movement the accounts the facts describe cannot pay for: {uncovered:?}"
            );
            panic!(
                "a reservation of a billion units was decided against a run funded with ten million"
            );
        }
    }
}

#[test]
fn concurrent_awards_cannot_exceed_the_funded_award_count() {
    let (_store, application) = prepared();
    let outcomes = race(&application, |index| {
        (format!("award-{index}"), award(index))
    });
    let committed = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
    assert_eq!(
        committed, MAIN_MAX_AWARDS as usize,
        "exactly the funded number of awards may commit"
    );
    for outcome in outcomes.iter().filter(|outcome| outcome.is_err()) {
        assert!(
            matches!(
                outcome,
                Err(ApplicationError::Commitment(
                    CommitmentError::AwardsExhausted { .. }
                ))
            ),
            "an award beyond the funded count must be refused for that reason: {outcome:?}"
        );
    }

    let ledger = snapshot(&application);
    assert_eq!(ledger.offers()[MAIN_OFFER].awards_made, MAIN_MAX_AWARDS);
    assert_eq!(ledger.contracts().len(), MAIN_MAX_AWARDS as usize);
    assert_eq!(
        ledger.obligations().len(),
        MAIN_MAX_AWARDS as usize + 1,
        "one child obligation per award, and the root"
    );
    let consent: BTreeSet<&str> = ledger
        .contracts()
        .values()
        .map(|contract| contract.bid_id.as_str())
        .collect();
    assert_eq!(
        consent.len(),
        ledger.contracts().len(),
        "no consent record was spent twice"
    );
    // The pool still holds far more than one award: the refusals were the funded count, not
    // arithmetic.
    assert!(
        ledger.offers()[MAIN_OFFER]
            .escrow
            .covers(&requested_escrow())
    );
    assert_conserved(&application);
}

#[test]
fn concurrent_open_acceptance_forms_exactly_one_contract() {
    let (_store, application) = prepared();
    let outcomes = race(&application, |index| {
        (format!("accept-{index}"), accept_open(index))
    });
    assert_eq!(
        outcomes.iter().filter(|outcome| outcome.is_ok()).count(),
        OPEN_MAX_AWARDS as usize
    );
    let ledger = snapshot(&application);
    assert_eq!(ledger.offers()[OPEN_OFFER].awards_made, OPEN_MAX_AWARDS);
    let contracts: Vec<_> = ledger
        .contracts()
        .values()
        .filter(|contract| contract.offer_id == OPEN_OFFER)
        .collect();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].lease.generation, 1);
    assert_conserved(&application);
}

#[test]
fn a_repeated_command_identifier_commits_nothing_a_second_time() {
    let (_store, application) = prepared();
    let first = execute(&application, "award-once", &award(0)).expect("first award");
    assert!(!first.replayed);
    let second = execute(&application, "award-once", &award(0)).expect("the same delivery again");
    assert!(second.replayed);
    assert_eq!(first.events, second.events);
    assert_eq!(first.first_sequence, second.first_sequence);

    let ledger = snapshot(&application);
    assert_eq!(ledger.offers()[MAIN_OFFER].awards_made, 1);
    assert_eq!(ledger.contracts().len(), 1);
    assert_eq!(
        committed_facts(&application),
        first.first_sequence as usize + first.events.len() - 1
    );

    // The same identifier carrying different content is a conflict, not a replay.
    assert!(matches!(
        execute(&application, "award-once", &award(1)),
        Err(ApplicationError::IdempotencyConflict { .. })
    ));
    assert_conserved(&application);
}

#[test]
fn concurrent_retries_of_one_command_identifier_commit_one_effect() {
    let (_store, application) = prepared();
    let outcomes = race(&application, |_| ("award-shared".to_owned(), award(0)));
    assert!(
        outcomes.iter().all(Result::is_ok),
        "every delivery of the same command returns the same committed result"
    );
    let ledger = snapshot(&application);
    assert_eq!(ledger.offers()[MAIN_OFFER].awards_made, 1);
    assert_eq!(ledger.contracts().len(), 1);
    assert_conserved(&application);
}

#[test]
fn a_refused_command_consumes_no_sequence_and_leaves_no_fact() {
    let (_store, application) = prepared();
    let before = committed_facts(&application);
    let held = snapshot(&application);
    let stranger = CommitmentCommand::Award(Award {
        contract_id: "contract-stranger".to_owned(),
        obligation_id: "obligation-stranger".to_owned(),
        lease_id: "lease-stranger".to_owned(),
        offer_id: MAIN_OFFER.to_owned(),
        bid_id: "bid-0".to_owned(),
        sponsor: contender(3),
        lease_ms: LEASE_MS,
    });
    assert!(matches!(
        execute(&application, "award-stranger", &stranger),
        Err(ApplicationError::Commitment(
            CommitmentError::NotAuthorized { .. }
        ))
    ));
    assert_eq!(committed_facts(&application), before);
    assert_eq!(snapshot(&application), held);

    // The identifier stays free: the same identifier may carry a command that is valid.
    let outcome = execute(&application, "award-stranger", &award(0))
        .expect("a valid command under the same identifier");
    assert!(!outcome.replayed);
}

/// A run whose journal has ended begins nothing further in its kernel, and still records what its
/// commitments settle.
///
/// The two records of one run end at different moments. The operator's cancellation moves the
/// journal at once, and the kernel is stopped only when what was running has been wound down;
/// everything created in that window would be work offered, consented to and funded for a run that
/// had already ended. So creation is refused from the journal's ending onwards, whether or not the
/// ledger has been stopped — the ledger has not been, here — and a refusal leaves no fact and no
/// sequence behind it. The accounting of the ending still lands, because it is the whole reason a
/// commitment may be recorded after the run has ended at all.
#[test]
fn a_run_whose_journal_has_ended_creates_nothing_and_still_settles() {
    let (_store, application) = prepared();
    execute(&application, "award-0", &award(0)).expect("award");
    application
        .lock()
        .expect("the run")
        .execute(
            "cancel",
            Command::Cancel {
                reason: "stopped by the operator".to_owned(),
            },
        )
        .expect("the journal records the cancellation");
    let before = committed_facts(&application);
    assert!(
        snapshot(&application).stopped().is_none(),
        "the kernel was stopped before the window under test"
    );

    for (command_id, command) in [
        (
            "advertise-late",
            advertise("offer-late", OfferPolicy::Negotiated, 1),
        ),
        ("award-late", award(1)),
        (
            "attempt-late",
            CommitmentCommand::StartAttempt(StartAttempt {
                attempt_id: "attempt-late".to_owned(),
                contract_id: "contract-0".to_owned(),
                participant: contender(0),
                generation: 1,
            }),
        ),
    ] {
        let refusal = execute(&application, command_id, &command);
        assert!(
            matches!(
                refusal,
                Err(ApplicationError::Transition(TransitionError::Terminal(
                    RunStatus::Cancelled
                )))
            ),
            "{command_id} was admitted on a run that had ended: {refusal:?}"
        );
    }
    assert_eq!(
        committed_facts(&application),
        before,
        "a refused command left a fact behind"
    );

    // What the ending itself needs: the work is returned, the offer is withdrawn and settled, and
    // the run is stopped.
    for (command_id, command) in [
        (
            "return-late",
            CommitmentCommand::ReturnObligation(ReturnObligation {
                contract_id: "contract-0".to_owned(),
                participant: contender(0),
                generation: 1,
                outcome: Outcome::Cancelled,
            }),
        ),
        (
            "withdraw-late",
            CommitmentCommand::WithdrawOffer(WithdrawOffer {
                offer_id: MAIN_OFFER.to_owned(),
                sponsor: ROOT.to_owned(),
            }),
        ),
        (
            "settle-late",
            CommitmentCommand::SettleOffer(SettleOffer {
                offer_id: MAIN_OFFER.to_owned(),
                sponsor: ROOT.to_owned(),
            }),
        ),
    ] {
        execute(&application, command_id, &command).unwrap_or_else(|error| {
            panic!("{command_id} was refused after the run ended: {error}")
        });
    }
    // Measured before the kernel is stopped, because the probe it uses asks the kernel to decide a
    // reservation, and a stopped kernel refuses every reservation whatever the accounts hold.
    assert_conserved(&application);
    execute(
        &application,
        "stop-late",
        &CommitmentCommand::StopRun(StopRun {
            authority: ROOT.to_owned(),
            reason: StopReason::Cancelled,
        }),
    )
    .expect("stopping the run was refused after the run ended");
    let ledger = snapshot(&application);
    assert_eq!(
        ledger.stopped(),
        Some(StopReason::Cancelled),
        "the kernel of a stopped run holds no ending"
    );
    assert_eq!(
        ledger.contracts()["contract-0"].state,
        ContractState::Returned,
        "the work of a stopped run was left open"
    );
    assert_eq!(ledger.offers()[MAIN_OFFER].state, OfferState::Settled);
}

#[test]
fn committed_facts_are_ordered_and_readable_from_a_cursor() {
    let (_store, application) = prepared();
    execute(&application, "award-0", &award(0)).expect("award");
    let all = facts_after(&application, 0);
    assert!(!all.is_empty());
    for (position, fact) in all.iter().enumerate() {
        assert_eq!(fact.sequence, position as u64 + 1);
    }
    let tail = facts_after(&application, all.len() as u64 - 1);
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0], all[all.len() - 1]);
    assert!(
        all.iter()
            .any(|fact| matches!(fact.event, CommitmentEvent::TaskContractFormed { .. })),
        "the award is visible as a committed fact"
    );
    assert!(facts_after(&application, all.len() as u64).is_empty());
    assert!(
        all.iter().any(|fact| fact.command_id == "award-0"),
        "the record does not name the command that committed the award"
    );
}

//! Contention against the commitment service from real threads.
//!
//! The domain suite proves that no *ordering* of contending commands can overspend a reservation
//! or duplicate an obligation. This suite proves the other half: that concurrent callers are
//! actually reduced to one such ordering, that a repeated command identifier commits nothing a
//! second time, and that a refusal consumes no sequence and leaves no fact.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Barrier};

use ymp_application::{CommitmentService, CommitmentServiceError, RecordedFact};
use ymp_domain::commitment::{
    AcceptOpen, AccountRef, Advertise, Award, BudgetVector, CommitmentCommand, CommitmentError,
    CommitmentEvent, CommitmentLedger, DIMENSION_COUNT, DIMENSIONS, Dimension, FundingSource,
    OfferPolicy, RecordBid, RegisterParticipant,
};

const ROOT: &str = "sponsor-root";
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

/// A service holding two offers — one negotiated with recorded consent from every contender, one
/// open — and nothing awarded yet.
fn prepared() -> CommitmentService {
    let service = CommitmentService::new(
        CommitmentLedger::new(ROOT, "principal-root", ROOT_OBLIGATION, root_budget())
            .expect("root ledger"),
    );
    for index in 0..CONTENDERS {
        service
            .execute(
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
    service
        .execute(
            "advertise-main",
            &advertise(MAIN_OFFER, OfferPolicy::Negotiated, MAIN_MAX_AWARDS),
        )
        .expect("negotiated offer");
    service
        .execute(
            "advertise-open",
            &advertise(OPEN_OFFER, OfferPolicy::OpenAccept, OPEN_MAX_AWARDS),
        )
        .expect("open offer");
    for index in 0..CONTENDERS {
        service
            .execute(
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
    service
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

/// Run one command per thread, released together, and report which ones were committed.
fn race<F>(service: &CommitmentService, command: F) -> Vec<Result<bool, CommitmentServiceError>>
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
                    service.execute(&command_id, &command).map(|_| true)
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
/// read out of the records the service keeps.
///
/// Adding up those records and comparing the sum with the budget the run began with states nothing:
/// each movement takes capacity out of one record and puts the same quantity into another, so that
/// sum is the opening budget for any run at all, including one that lost track of what it moved.
/// What can be false is the comparison below, because the two sides are built by different code —
/// one by the kernel applying each fact to its records, the other here from the same facts as they
/// were committed to the log.
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

    /// Where the accounts the facts describe and the accounts the service keeps have parted
    /// company, in any dimension of any account, including the capacity that has left the accounts
    /// for good.
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
                        "{account:?} {dimension}: the facts say {} and the service holds {recorded}",
                        facts[dimension.index()]
                    ));
                }
            }
        }
        for dimension in DIMENSIONS {
            let recorded = i128::from(ledger.consumed().get(dimension));
            if self.consumed[dimension.index()] != recorded {
                reported.push(format!(
                    "consumed {dimension}: the facts say {} and the service holds {recorded}",
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

/// Conservation, asserted against the facts the service committed rather than against the records
/// it keeps.
///
/// Two things have to hold, and the second is not implied by the first. The accounts rebuilt from
/// the facts must agree with the accounts the service holds; and every movement the kernel is
/// willing to decide must be one those accounts can pay for. A kernel that stopped establishing
/// the capacity before deciding would keep books that add up perfectly — every fact it commits
/// still moves capacity out of one account and into another — while committing facts that move
/// capacity nobody ever held.
fn assert_conserved(service: &CommitmentService) {
    let ledger = service.snapshot().expect("snapshot");
    let accounts = FactAccounts::rebuilt(
        ROOT,
        *ledger.initial_total(),
        &service.facts_after(0).expect("facts"),
    );
    let divergence = accounts.divergence(&ledger);
    assert!(
        divergence.is_empty(),
        "the accounts the facts describe and the accounts the service keeps disagree: {divergence:?}"
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
    let service = prepared();
    let outcomes = race(&service, |index| (format!("award-{index}"), award(index)));
    let committed = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
    assert_eq!(
        committed, MAIN_MAX_AWARDS as usize,
        "exactly the funded number of awards may commit"
    );
    for outcome in outcomes.iter().filter(|outcome| outcome.is_err()) {
        assert!(
            matches!(
                outcome,
                Err(CommitmentServiceError::Refused(
                    CommitmentError::AwardsExhausted { .. }
                ))
            ),
            "an award beyond the funded count must be refused for that reason: {outcome:?}"
        );
    }

    let ledger = service.snapshot().expect("snapshot");
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
    assert_conserved(&service);
}

#[test]
fn concurrent_open_acceptance_forms_exactly_one_contract() {
    let service = prepared();
    let outcomes = race(&service, |index| {
        (format!("accept-{index}"), accept_open(index))
    });
    assert_eq!(
        outcomes.iter().filter(|outcome| outcome.is_ok()).count(),
        OPEN_MAX_AWARDS as usize
    );
    let ledger = service.snapshot().expect("snapshot");
    assert_eq!(ledger.offers()[OPEN_OFFER].awards_made, OPEN_MAX_AWARDS);
    let contracts: Vec<_> = ledger
        .contracts()
        .values()
        .filter(|contract| contract.offer_id == OPEN_OFFER)
        .collect();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].lease.generation, 1);
    assert_conserved(&service);
}

#[test]
fn a_repeated_command_identifier_commits_nothing_a_second_time() {
    let service = prepared();
    let first = service
        .execute("award-once", &award(0))
        .expect("first award");
    assert!(!first.replayed);
    let second = service
        .execute("award-once", &award(0))
        .expect("the same delivery again");
    assert!(second.replayed);
    assert_eq!(first.events, second.events);
    assert_eq!(first.first_sequence, second.first_sequence);

    let ledger = service.snapshot().expect("snapshot");
    assert_eq!(ledger.offers()[MAIN_OFFER].awards_made, 1);
    assert_eq!(ledger.contracts().len(), 1);
    assert_eq!(
        service.committed_facts().expect("log"),
        first.first_sequence as usize + first.events.len() - 1
    );

    // The same identifier carrying different content is a conflict, not a replay.
    assert!(matches!(
        service.execute("award-once", &award(1)),
        Err(CommitmentServiceError::IdempotencyConflict { .. })
    ));
    assert_conserved(&service);
}

#[test]
fn concurrent_retries_of_one_command_identifier_commit_one_effect() {
    let service = prepared();
    let outcomes = race(&service, |_| ("award-shared".to_owned(), award(0)));
    assert!(
        outcomes.iter().all(Result::is_ok),
        "every delivery of the same command returns the same committed result"
    );
    let ledger = service.snapshot().expect("snapshot");
    assert_eq!(ledger.offers()[MAIN_OFFER].awards_made, 1);
    assert_eq!(ledger.contracts().len(), 1);
    assert_conserved(&service);
}

#[test]
fn a_refused_command_consumes_no_sequence_and_leaves_no_fact() {
    let service = prepared();
    let before = service.committed_facts().expect("log");
    let snapshot = service.snapshot().expect("snapshot");
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
        service.execute("award-stranger", &stranger),
        Err(CommitmentServiceError::Refused(
            CommitmentError::NotAuthorized { .. }
        ))
    ));
    assert_eq!(service.committed_facts().expect("log"), before);
    assert_eq!(service.snapshot().expect("snapshot"), snapshot);

    // The identifier stays free: the same identifier may carry a command that is valid.
    let outcome = service
        .execute("award-stranger", &award(0))
        .expect("a valid command under the same identifier");
    assert!(!outcome.replayed);
}

#[test]
fn committed_facts_are_ordered_and_readable_from_a_cursor() {
    let service = prepared();
    service.execute("award-0", &award(0)).expect("award");
    let all = service.facts_after(0).expect("facts");
    assert!(!all.is_empty());
    for (position, fact) in all.iter().enumerate() {
        assert_eq!(fact.sequence, position as u64 + 1);
    }
    let tail = service.facts_after(all.len() as u64 - 1).expect("facts");
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0], all[all.len() - 1]);
    assert!(
        all.iter()
            .any(|fact| matches!(fact.event, CommitmentEvent::TaskContractFormed { .. })),
        "the award is visible as a committed fact"
    );
    assert!(
        service
            .facts_after(all.len() as u64)
            .expect("facts")
            .is_empty()
    );
}

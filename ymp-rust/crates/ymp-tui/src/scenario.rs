//! Deterministic runs, built as domain state.
//!
//! Every scenario here is a legal sequence of `ymp-domain` events folded through
//! `RunState::apply`, exactly as the application would have committed them. Nothing in this
//! module produces a screen value: the projection derives every rendered field from the state
//! these functions build, which is what makes a rendered buffer evidence about the binding
//! rather than about a fixture.
//!
//! The identifiers are deliberately generic. A scenario is a shape of run, not a re-enactment
//! of the design artifact's illustration.

use ymp_domain::commitment::{
    Advertise, Award, BudgetVector, CommitmentCommand, CommitmentLedger, Dimension, FundingSource,
    OfferPolicy, RecordBid, RegisterParticipant,
};
use ymp_domain::pool::FrozenEntry;
use ymp_domain::{
    Budget, Command, EventEnvelope, EventKind, RunState, VerificationDecision, VerificationRecord,
};

/// A run and the events that produced it.
#[derive(Clone, Debug)]
pub struct Run {
    pub state: RunState,
    pub events: Vec<EventEnvelope>,
}

impl Run {
    /// The budget the run started with, as its first event records it.
    pub fn initial_budget(&self) -> Option<Budget> {
        self.events.first().and_then(|event| match &event.event {
            EventKind::RunStarted { budget } => Some(budget.clone()),
            _ => None,
        })
    }
}

/// A digest-shaped value the domain accepts, distinct per seed.
pub fn digest(seed: u8) -> String {
    let mut value = String::with_capacity(64);
    for index in 0..32 {
        value.push_str(&format!("{:02x}", seed.wrapping_add(index)));
    }
    value
}

struct Builder {
    state: RunState,
    events: Vec<EventEnvelope>,
}

impl Builder {
    fn start(run_id: &str, budget: Budget) -> Self {
        let start = envelope(run_id, 1, None, EventKind::RunStarted { budget });
        let state = RunState::from_start(&start).expect("run_started is the first event");
        Self {
            state,
            events: vec![start],
        }
    }

    fn command(self, command: &Command) -> Self {
        let kind = self.state.decide(command).expect("legal command");
        self.commit(kind)
    }

    fn verification(self, record: &VerificationRecord) -> Self {
        let kind = self
            .state
            .decide_verification(record)
            .expect("legal verification");
        self.commit(kind)
    }

    fn commit(mut self, kind: EventKind) -> Self {
        let sequence = self.state.last_sequence + 1;
        let envelope = envelope(
            &self.state.run_id,
            sequence,
            Some(self.state.last_event_digest.clone()),
            kind,
        );
        self.state.apply(&envelope);
        self.events.push(envelope);
        self
    }

    fn finish(self) -> Run {
        Run {
            state: self.state,
            events: self.events,
        }
    }
}

fn envelope(
    run_id: &str,
    sequence: u64,
    predecessor: Option<String>,
    event: EventKind,
) -> EventEnvelope {
    EventEnvelope::new(
        run_id,
        sequence,
        format!("cmd-{sequence}"),
        digest(sequence as u8),
        predecessor,
        event,
    )
    .expect("event envelope")
}

fn verification_record(candidate: &str, decision: VerificationDecision) -> VerificationRecord {
    VerificationRecord {
        candidate_digest: candidate.to_owned(),
        contract_digest: digest(0xc0),
        oracle_digest: digest(0x0a),
        environment_digest: digest(0xe0),
        evidence_digest: digest(0xed),
        decision,
    }
}

fn attempt(builder: Builder, id: &str) -> Builder {
    builder.command(&Command::StartAttempt {
        attempt_id: id.to_owned(),
    })
}

fn submit(builder: Builder, id: &str, object: &str) -> Builder {
    builder.command(&Command::SubmitCandidate {
        attempt_id: id.to_owned(),
        base_digest: digest(0xba),
        object_digest: object.to_owned(),
    })
}

/// A run created against a pool, as every run is: the freeze follows the start, and the pool it
/// carries permits two entries of which admission found one live.
///
/// The unavailable entry stands first, so what the surfaces show is a run that ignited on the
/// second entry of its own declared order — the shape a screen would get wrong if it read the
/// first entry rather than the first live one.
pub fn with_a_frozen_pool() -> Run {
    Builder::start("demo-run", Budget::new(3, 2))
        .command(&Command::FreezePool {
            pool: "default".to_owned(),
            entries: vec![
                FrozenEntry::unavailable(
                    "openai",
                    "codex",
                    "gpt-5",
                    "the account states no credential",
                ),
                FrozenEntry::admissible("anthropic", "claude-code", "claude-opus-5"),
            ],
            digest: digest(0xf0),
        })
        .finish()
}

/// A live run: one rejected candidate, a second attempt still working.
pub fn running() -> Run {
    running_named("demo-run")
}

/// The same shape under a caller-chosen run identifier. Run identifiers the product generates
/// are long — `ymp internal managed-candidate-smoke` produces a prefixed UUID — and a surface
/// has to stay correct under one.
pub fn running_named(run_id: &str) -> Run {
    let builder = Builder::start(run_id, Budget::new(3, 2));
    let builder = attempt(builder, "attempt-1");
    let first = digest(0x11);
    let builder = submit(builder, "attempt-1", &first);
    let builder = builder.verification(&verification_record(&first, VerificationDecision::Reject));
    attempt(builder, "attempt-2").finish()
}

/// The same run, ended by an accepted candidate.
pub fn accepted() -> Run {
    let builder = Builder::start("demo-run", Budget::new(3, 2));
    let builder = attempt(builder, "attempt-1");
    let candidate = digest(0x22);
    let builder = submit(builder, "attempt-1", &candidate);
    builder
        .verification(&verification_record(
            &candidate,
            VerificationDecision::Accept,
        ))
        .finish()
}

/// A run that ran out of attempts.
pub fn exhausted() -> Run {
    let builder = Builder::start("demo-run", Budget::new(1, 1));
    let builder = attempt(builder, "attempt-1");
    attempt(builder, "attempt-2").finish()
}

/// A run the verifier could not decide.
pub fn abstained() -> Run {
    let builder = Builder::start("demo-run", Budget::new(2, 1));
    let builder = attempt(builder, "attempt-1");
    builder
        .command(&Command::Abstain {
            reason: "the verifier could not decide within its wall-time limit".to_owned(),
        })
        .finish()
}

/// A run the operator ended from the terminal interface.
pub fn cancelled() -> Run {
    let builder = Builder::start("demo-run", Budget::new(2, 1));
    let builder = attempt(builder, "attempt-1");
    builder
        .command(&Command::Cancel {
            reason: crate::decisions::cancellation_reason(),
        })
        .finish()
}

/// A run the machinery failed under.
pub fn infrastructure_error() -> Run {
    let builder = Builder::start("demo-run", Budget::new(2, 1));
    let builder = attempt(builder, "attempt-1");
    builder
        .command(&Command::FailInfrastructure {
            reason: "the verifier environment object could not be read".to_owned(),
        })
        .finish()
}

/// A live run whose commitment kernel formed one task contract.
///
/// The facts are not written by hand: a real ledger is asked to decide each command, and what it
/// commits is what the journal records carry — exactly as the application commits them. A screen
/// built over this run is therefore evidence about the binding and not about a fixture.
pub fn with_commitments() -> Run {
    committed_run(escrow_units())
}

/// The same run with a different escrow, so a screen bound to the ledger cannot draw the same
/// values for both.
pub fn with_commitments_of(units: u64) -> Run {
    committed_run(units)
}

/// What one award moves into the task contract it forms, in every dimension the work spends.
const fn escrow_units() -> u64 {
    2_000
}

fn committed_run(units: u64) -> Run {
    const SPONSOR: &str = "sponsor-root";
    const SPONSOR_PRINCIPAL: &str = "principal-root";
    const ROOT_OBLIGATION: &str = "obligation-root";
    const CONTRACTOR: &str = "participant-one";
    const OFFER: &str = "offer-one";
    const BID: &str = "bid-one";
    const CLASS: &str = "class-one";

    // The three dimensions the work itself spends. The authorities to start, offer and create are
    // added where each account needs them, so no account carries capacity it never uses.
    let capacity = |units: u64| {
        BudgetVector::ZERO
            .with(Dimension::MoneyMicros, units)
            .with(Dimension::ModelTokens, units)
            .with(Dimension::WallTimeMs, units)
    };
    let root_budget = capacity(1_000_000)
        .with(Dimension::VerificationQueries, 8)
        .with(Dimension::ParticipantStarts, 8)
        .with(Dimension::AttemptStarts, 16)
        .with(Dimension::InvocationStarts, 16)
        .with(Dimension::OfferCreations, 8)
        .with(Dimension::ObligationCreations, 8);
    let endowment = capacity(50_000)
        .with(Dimension::AttemptStarts, 4)
        .with(Dimension::InvocationStarts, 4)
        .with(Dimension::OfferCreations, 2)
        .with(Dimension::ObligationCreations, 2);
    let escrow = |units: u64| {
        capacity(units)
            .with(Dimension::AttemptStarts, 2)
            .with(Dimension::InvocationStarts, 2)
    };

    let mut ledger =
        CommitmentLedger::new(SPONSOR, SPONSOR_PRINCIPAL, ROOT_OBLIGATION, root_budget)
            .expect("the root ledger of the scenario");
    let mut builder = Builder::start("demo-run", Budget::new(3, 2));
    builder = builder.commit(EventKind::CommitmentKernelOpened {
        root_participant: SPONSOR.to_owned(),
        root_principal: SPONSOR_PRINCIPAL.to_owned(),
        root_obligation: ROOT_OBLIGATION.to_owned(),
        budget: root_budget,
    });

    let commands = vec![
        CommitmentCommand::RegisterParticipant(RegisterParticipant {
            participant_id: CONTRACTOR.to_owned(),
            principal_id: "principal-one".to_owned(),
            sponsor: SPONSOR.to_owned(),
            endowment,
        }),
        CommitmentCommand::Advertise(Advertise {
            offer_id: OFFER.to_owned(),
            sponsor: SPONSOR.to_owned(),
            parent_obligation: ROOT_OBLIGATION.to_owned(),
            funding_source: FundingSource::Participant,
            task_scope: "scope-one".to_owned(),
            base_digest: digest(0xba),
            intent_digest: digest(0x1e),
            artifact_class: CLASS.to_owned(),
            dependencies: Vec::new(),
            capability_scope: Vec::new(),
            execution_escrow: escrow(units),
            policy: OfferPolicy::Negotiated,
            bid_deadline: 100_000,
            offer_deadline: 100_000,
            max_awards: 1,
        }),
        CommitmentCommand::RecordBid(RecordBid {
            bid_id: BID.to_owned(),
            offer_id: OFFER.to_owned(),
            bidder: CONTRACTOR.to_owned(),
            requested_escrow: escrow(units),
            artifact_class: CLASS.to_owned(),
            proposal_digest: Some(digest(0x9a)),
            expires_at: 100_000,
        }),
        CommitmentCommand::Award(Award {
            contract_id: "contract-one".to_owned(),
            obligation_id: "obligation-one".to_owned(),
            lease_id: "lease-one".to_owned(),
            offer_id: OFFER.to_owned(),
            bid_id: BID.to_owned(),
            sponsor: SPONSOR.to_owned(),
            lease_ms: units,
        }),
    ];
    for command in &commands {
        let facts = ledger.execute(command).expect("a legal commitment command");
        builder = builder.commit(EventKind::CommitmentFactsRecorded { facts });
    }
    attempt(builder, "attempt-1").finish()
}

/// A long run: many candidates, so a page has to window its rows.
pub fn high_volume(candidates: usize) -> Run {
    let mut builder = Builder::start("demo-run", Budget::new(1, 0));
    builder = attempt(builder, "attempt-1");
    for index in 0..candidates {
        builder = submit(builder, "attempt-1", &digest((index % 200) as u8));
    }
    builder.finish()
}

#[cfg(test)]
mod tests {
    use ymp_domain::RunStatus;

    use super::*;

    #[test]
    fn every_scenario_reaches_the_status_it_is_named_for() {
        assert_eq!(running().state.status, RunStatus::Running);
        assert_eq!(accepted().state.status, RunStatus::Accepted);
        assert_eq!(exhausted().state.status, RunStatus::Exhausted);
        assert_eq!(abstained().state.status, RunStatus::Abstained);
        assert_eq!(cancelled().state.status, RunStatus::Cancelled);
        assert_eq!(
            infrastructure_error().state.status,
            RunStatus::InfrastructureError
        );
    }
}

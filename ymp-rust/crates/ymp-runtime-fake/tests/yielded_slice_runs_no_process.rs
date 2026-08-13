//! A yielded process slice against a real runtime session rather than against a record of one.
//!
//! The ledger says a yielded slice has no process. That is a claim about something outside the
//! ledger, so it is checked where the process actually is: the deterministic runtime is driven
//! through a whole yield and resumption, and the session is asked for events at every step. While
//! the slice is yielded it produces none, and the only thing that makes it produce any again is the
//! ledger admitting the wake — which it does only after a fact the yield named has been committed.

use ymp_domain::commitment::{
    Advertise, Award, BudgetVector, CommitmentCommand, CommitmentLedger, Dimension, FundingSource,
    OfferPolicy, RecordBid, RegisterParticipant, ResumeInvocation, StartAttempt, StartInvocation,
    SubmitResult, WakeCondition, YieldInvocation,
};
use ymp_runtime_api::{InvocationRequest, RuntimeDriver, RuntimeEventKind, RuntimeSession, Usage};
use ymp_runtime_fake::{FakeRuntime, ScriptStep};

const ROOT: &str = "sponsor-root";
const ALPHA: &str = "p-alpha";
const OFFER: &str = "offer-one";
const CONTRACT: &str = "contract-one";
const ATTEMPT: &str = "attempt-one";
const INVOCATION: &str = "invocation-one";
const CLASS: &str = "class-under-test";
const DEADLINE: u64 = 10_000;
const LEASE_MS: u64 = 5_000;

fn digest(tag: &str) -> String {
    ymp_domain::digest_bytes(tag.as_bytes())
}

fn escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 10_000)
        .with(Dimension::ModelTokens, 10_000)
        .with(Dimension::WallTimeMs, 20_000)
        .with(Dimension::AttemptStarts, 4)
        .with(Dimension::InvocationStarts, 8)
}

/// A ledger holding one attempt, open and ready for its first process slice.
fn prepared() -> CommitmentLedger {
    let budget = escrow()
        .checked_scale(4)
        .expect("the root budget")
        .with(Dimension::ParticipantStarts, 4)
        .with(Dimension::OfferCreations, 8)
        .with(Dimension::ObligationCreations, 8);
    let mut ledger =
        CommitmentLedger::new(ROOT, "principal-root", "obligation-root", budget).expect("ledger");
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
            parent_obligation: "obligation-root".to_owned(),
            funding_source: FundingSource::Participant,
            task_scope: "scope-under-test".to_owned(),
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
            bid_id: "bid-one".to_owned(),
            offer_id: OFFER.to_owned(),
            bidder: ALPHA.to_owned(),
            requested_escrow: escrow(),
            artifact_class: CLASS.to_owned(),
            proposal_digest: None,
            expires_at: DEADLINE,
        }),
        CommitmentCommand::Award(Award {
            contract_id: CONTRACT.to_owned(),
            obligation_id: "obligation-one".to_owned(),
            lease_id: "lease-one".to_owned(),
            offer_id: OFFER.to_owned(),
            bid_id: "bid-one".to_owned(),
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
    for command in &commands {
        ledger.execute(command).expect("the prefix is well formed");
    }
    ledger
}

fn drain_until_yield(session: &mut Box<dyn RuntimeSession>) {
    loop {
        let event = session
            .next_event()
            .expect("the deterministic runtime produces events")
            .expect("the session has not finished");
        if matches!(event.event, RuntimeEventKind::Yielded { .. }) {
            return;
        }
    }
}

#[test]
fn a_yielded_slice_produces_no_events_until_the_ledger_admits_its_wake() {
    let mut ledger = prepared();
    let runtime = FakeRuntime::with_script(vec![
        ScriptStep::Output("working".to_owned()),
        ScriptStep::Yield("cursor".to_owned()),
        ScriptStep::Output("resumed".to_owned()),
        ScriptStep::Complete(Usage::default()),
    ]);
    let mut session = runtime
        .start(InvocationRequest {
            invocation_id: INVOCATION.to_owned(),
            attempt_id: ATTEMPT.to_owned(),
            workspace: std::env::current_dir().expect("current directory"),
            mcp: None,
            prompt: "fixture".to_owned(),
            cancellation: Default::default(),
        })
        .expect("session");

    ledger
        .execute(&CommitmentCommand::StartInvocation(StartInvocation {
            invocation_id: INVOCATION.to_owned(),
            attempt_id: ATTEMPT.to_owned(),
            contract_id: CONTRACT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            cursor: 0,
        }))
        .expect("the slice begins");
    drain_until_yield(&mut session);

    let cursor = ledger.sequence();
    ledger
        .execute(&CommitmentCommand::YieldInvocation(YieldInvocation {
            invocation_id: INVOCATION.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            cursor,
            conditions: vec![WakeCondition::SubmissionRecorded {
                contract_id: CONTRACT.to_owned(),
            }],
            wake_deadline: LEASE_MS,
        }))
        .expect("the slice yields");

    // Nothing it asked about has happened. The session produces nothing however often it is asked,
    // and the ledger will not admit it.
    for _ in 0..4 {
        assert!(
            session.next_event().expect("no failure").is_none(),
            "a yielded slice runs no process"
        );
    }
    assert!(ledger.admission_order().is_empty());

    ledger
        .execute(&CommitmentCommand::SubmitResult(SubmitResult {
            contract_id: CONTRACT.to_owned(),
            attempt_id: ATTEMPT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            candidate_digest: digest("candidate"),
        }))
        .expect("the fact the yield named");
    assert!(
        session.next_event().expect("no failure").is_none(),
        "a committed fact does not by itself start a process"
    );
    assert_eq!(
        ledger
            .admission_order()
            .into_iter()
            .map(|invocation| invocation.invocation_id.clone())
            .collect::<Vec<_>>(),
        vec![INVOCATION.to_owned()]
    );

    ledger
        .execute(&CommitmentCommand::ResumeInvocation(ResumeInvocation {
            invocation_id: INVOCATION.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
        }))
        .expect("the ledger admits the wake");
    session
        .resume("wake".to_owned())
        .expect("and only then is the process resumed");
    assert!(matches!(
        session
            .next_event()
            .expect("no failure")
            .expect("the session runs again")
            .event,
        RuntimeEventKind::Output { text } if text == "resumed"
    ));
}

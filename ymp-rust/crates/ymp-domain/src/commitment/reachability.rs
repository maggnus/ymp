//! An exhaustive traversal of every registry state one attempt can reach.
//!
//! A generated schedule replays one ordering at a time and asks what it broke. That finds a defect
//! some seed happens to reach, and says nothing about the ones no seed reached. What is done here
//! instead is a breadth-first traversal of the whole reachable state space of a small alphabet: from
//! the deterministic prefix, every command is offered in every state, and every state a command
//! accepts is explored in turn. A question answered over that space is answered for every ordering
//! the alphabet admits, including the ones no seeded interleaving would produce.
//!
//! What makes the space finite is that the states are compared as registries rather than as
//! histories. The list of committed facts grows with every command, so no two paths ever produce
//! equal ledgers and a traversal keyed on the whole ledger would be the command tree itself. The
//! key therefore leaves out the fact list and the sequence a yield was recorded at. One transition
//! does read the facts — resumption matches a committed fact against the yield's condition — so the
//! omission is sound only under a property of this alphabet, stated here as its condition: every
//! yield in this alphabet registers cursor zero, resumption never advances a cursor, and the
//! candidate digest a condition names is set once by the submission fact and never reset, so the
//! match is equivalent to reading the registry field the key already carries. An alphabet that
//! violates this — a non-zero cursor, a moving digest — would glue states with different behaviour
//! and silently under-explore; extend the key before extending the alphabet. Two states with the
//! same key then accept the same commands and refuse the same commands, so exploring one of them
//! is exploring both.

use std::collections::{BTreeSet, VecDeque};

use super::invocations::{
    InvocationClosure, InvocationState, OpenAuthority, RootTerminal, StopReason, Verdict,
    WakeCondition,
};
use super::ledger::{CommitmentLedger, DisabledChecks};
use super::protocol::{
    AdvanceClock, CancelContract, CloseInvocation, CommitmentCommand, CommitmentError,
    RecordVerification, ResumeInvocation, ReturnObligation, SettleOffer, StartInvocation, StopRun,
    SubmitResult, WithdrawOffer, YieldInvocation,
};
use super::records::Outcome;
use super::schedules::{
    ALPHA, LIFE_ATTEMPT, LIFE_CONTRACT, LIFE_INVOCATION, LIFE_OFFER, LIFE_WAKE_DEADLINE,
    ROOT_PARTICIPANT, Tokens, Violation, digest, life_setup, new_ledger, permanent_refusal,
    resumption, state_violations, terminal_violations,
};

/// The second slice of the same attempt. Nothing distinguishes it from the first except its
/// identifier, which is exactly what makes it the question this sweep asks: whether an attempt the
/// run counts once may be running twice.
pub(crate) const SECOND_INVOCATION: &str = "invocation-life-second";

/// A ceiling on the traversal, far above the space the alphabet actually reaches. Hitting it would
/// mean the sweep stopped early, which is reported rather than passed over: a truncated traversal
/// that found nothing has not established that there is nothing to find.
const MAX_STATES: usize = 200_000;

/// What one exhaustive traversal found.
#[derive(Debug)]
pub(crate) struct SweepReport {
    /// How many distinct registry states the alphabet reaches.
    pub states: usize,
    /// States where the run counts a yielded slice as the funded wake holding it open while
    /// refusing that slice's resumption for a reason no later command can lift.
    pub held_open_by_dead_wake: usize,
    /// Of those, the ones held open by a slice whose resumption is refused because the run was
    /// stopped.
    pub held_open_after_stop: usize,
    /// States where two process slices of one attempt are running at the same time.
    pub two_running_slices: usize,
    /// States where the run is stopped and a slice of it is yielded. The shape the first question is
    /// about: a sweep that never reached it would answer the question by never asking it.
    pub stopped_with_yielded_slice: usize,
    /// States where both slices of the attempt exist and neither has been closed. The shape the
    /// second question is about.
    pub two_open_slices: usize,
    /// Everything else the invariants of the lifecycle model say about a state, gathered over the
    /// whole space rather than over one interleaving.
    pub violations: Vec<Violation>,
    /// Which terminal states the space reaches, so a sweep that stopped exercising the terminal
    /// checks can be told from one that found them satisfied.
    pub terminals: BTreeSet<&'static str>,
    pub truncated: bool,
}

/// The commands the traversal offers in every state.
///
/// They are the whole life of one attempt: two slices that start, yield, resume and close, the
/// submission their wakes name, the stop, the two clock readings that pass the wake deadline and
/// then the lease, and the commands a sponsor winds a run down with. Nothing here is ordered — the
/// order is what the traversal enumerates.
fn alphabet() -> Vec<CommitmentCommand> {
    let mut commands = Vec::new();
    for invocation_id in [LIFE_INVOCATION, SECOND_INVOCATION] {
        commands.push(CommitmentCommand::StartInvocation(StartInvocation {
            invocation_id: invocation_id.to_owned(),
            attempt_id: LIFE_ATTEMPT.to_owned(),
            contract_id: LIFE_CONTRACT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            cursor: 0,
        }));
        commands.push(CommitmentCommand::YieldInvocation(YieldInvocation {
            invocation_id: invocation_id.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            cursor: 0,
            conditions: vec![WakeCondition::SubmissionRecorded {
                contract_id: LIFE_CONTRACT.to_owned(),
            }],
            wake_deadline: LIFE_WAKE_DEADLINE,
        }));
        commands.push(CommitmentCommand::ResumeInvocation(ResumeInvocation {
            invocation_id: invocation_id.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
        }));
        commands.push(CommitmentCommand::CloseInvocation(CloseInvocation {
            invocation_id: invocation_id.to_owned(),
            closer: ALPHA.to_owned(),
            reason: InvocationClosure::Completed,
        }));
    }
    commands.push(CommitmentCommand::SubmitResult(SubmitResult {
        contract_id: LIFE_CONTRACT.to_owned(),
        attempt_id: LIFE_ATTEMPT.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        candidate_digest: digest("candidate-life"),
    }));
    commands.push(CommitmentCommand::RecordVerification(RecordVerification {
        contract_id: LIFE_CONTRACT.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        candidate_digest: digest("candidate-life"),
        verdict: Verdict::Passed,
    }));
    commands.push(CommitmentCommand::ReturnObligation(ReturnObligation {
        contract_id: LIFE_CONTRACT.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        outcome: Outcome::Result {
            candidate_digest: digest("candidate-life"),
        },
    }));
    commands.push(CommitmentCommand::StopRun(StopRun {
        authority: ROOT_PARTICIPANT.to_owned(),
        reason: StopReason::Cancelled,
    }));
    commands.push(CommitmentCommand::AdvanceClock(AdvanceClock { to: 600 }));
    commands.push(CommitmentCommand::AdvanceClock(AdvanceClock { to: 2_000 }));
    commands.push(CommitmentCommand::CancelContract(CancelContract {
        contract_id: LIFE_CONTRACT.to_owned(),
        sponsor: ROOT_PARTICIPANT.to_owned(),
    }));
    commands.push(CommitmentCommand::WithdrawOffer(WithdrawOffer {
        offer_id: LIFE_OFFER.to_owned(),
        sponsor: ROOT_PARTICIPANT.to_owned(),
    }));
    commands.push(CommitmentCommand::SettleOffer(SettleOffer {
        offer_id: LIFE_OFFER.to_owned(),
        sponsor: ROOT_PARTICIPANT.to_owned(),
    }));
    commands
}

/// Every question this sweep asks of one reachable state.
fn inspect(ledger: &CommitmentLedger, report: &mut SweepReport) {
    let running: Vec<&str> = ledger
        .invocations()
        .values()
        .filter(|invocation| invocation.state == InvocationState::Running)
        .map(|invocation| invocation.invocation_id.as_str())
        .collect();
    if running.len() > 1 {
        report.two_running_slices += 1;
    }
    let open = ledger
        .invocations()
        .values()
        .filter(|invocation| invocation.state != InvocationState::Closed)
        .count();
    if open > 1 {
        report.two_open_slices += 1;
    }
    let yielded = ledger
        .invocations()
        .values()
        .any(|invocation| invocation.state == InvocationState::Yielded);
    if yielded && ledger.stopped().is_some() {
        report.stopped_with_yielded_slice += 1;
    }
    if let Some(OpenAuthority::FundedWake { invocation_id }) = ledger.open_authority()
        && let Some(invocation) = ledger.invocations().get(&invocation_id)
        && let Err(refusal) = ledger.decide(&resumption(invocation))
        && permanent_refusal(&refusal).is_some()
    {
        report.held_open_by_dead_wake += 1;
        if matches!(refusal, CommitmentError::RunStopped) {
            report.held_open_after_stop += 1;
        }
    }
    report.violations.extend(state_violations(ledger));
    let terminal = ledger.root_terminal();
    if let Some(terminal) = terminal {
        report.terminals.insert(terminal_name(terminal));
    }
    report
        .violations
        .extend(terminal_violations(ledger, terminal));
}

const fn terminal_name(terminal: RootTerminal) -> &'static str {
    match terminal {
        RootTerminal::InfrastructureError => "infrastructure_error",
        RootTerminal::Cancelled => "cancelled",
        RootTerminal::Accepted => "accepted",
        RootTerminal::Abstained => "abstained",
        RootTerminal::Exhausted => "exhausted",
    }
}

/// Traverse every registry state the alphabet reaches and answer every question of each of them.
pub(crate) fn sweep(disabled: DisabledChecks) -> SweepReport {
    let tokens = Tokens::variant("life");
    let mut origin = new_ledger();
    origin.disable_checks(disabled);
    for command in life_setup(&tokens) {
        origin
            .execute(&command)
            .unwrap_or_else(|error| panic!("the deterministic prefix must be accepted: {error}"));
    }
    let commands = alphabet();
    let mut report = SweepReport {
        states: 0,
        held_open_by_dead_wake: 0,
        held_open_after_stop: 0,
        two_running_slices: 0,
        stopped_with_yielded_slice: 0,
        two_open_slices: 0,
        violations: Vec::new(),
        terminals: BTreeSet::new(),
        truncated: false,
    };
    let mut seen = BTreeSet::new();
    let mut frontier = VecDeque::new();
    seen.insert(origin.registry_snapshot());
    inspect(&origin, &mut report);
    report.states += 1;
    frontier.push_back(origin);
    while let Some(state) = frontier.pop_front() {
        for command in &commands {
            let mut next = state.clone();
            if next.execute(command).is_err() {
                continue;
            }
            if !seen.insert(next.registry_snapshot()) {
                continue;
            }
            report.states += 1;
            inspect(&next, &mut report);
            if report.states >= MAX_STATES {
                report.truncated = true;
                return report;
            }
            frontier.push_back(next);
        }
    }
    report.violations.dedup();
    report
}

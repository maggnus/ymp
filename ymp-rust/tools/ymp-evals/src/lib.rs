#![forbid(unsafe_code)]

use serde::Serialize;
use std::collections::{HashMap, VecDeque};

const CHILD_COUNT: usize = 2;
const INITIAL_CREATION_BUDGET: u8 = 3;
const INITIAL_START_BUDGET: u8 = 4;
const INITIAL_WAIT_BUDGET: u8 = 2;
const INITIAL_QUERY_BUDGET: u8 = 1;
const INITIAL_SPONSOR_FUNDS: u8 = 2;
const OFFER_DEADLINE: u8 = 2;
const MAX_LOGICAL_TIME: u8 = 4;
const LEASE_DURATION: u8 = 2;
const WAKE_DURATION: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mutation {
    None,
    SkipAwardReservation,
    IgnoreLeaseFence,
    AllowSecondObligationReturn,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum RunStatus {
    Running,
    Accepted,
    Exhausted,
    Abstained,
    Cancelled,
    InfrastructureError,
}

impl RunStatus {
    fn is_terminal(self) -> bool {
        self != Self::Running
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum OfferStatus {
    Absent,
    Advertised,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum InvocationPhase {
    Absent,
    Running,
    Yielded,
    Dormant,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum ReturnKind {
    Result,
    Exhausted,
    Abstained,
    Cancelled,
    InfrastructureError,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum ChildId {
    One,
    Two,
}

impl ChildId {
    const ALL: [Self; CHILD_COUNT] = [Self::One, Self::Two];

    fn index(self) -> usize {
        match self {
            Self::One => 0,
            Self::Two => 1,
        }
    }

    fn obligation_index(self) -> usize {
        self.index() + 1
    }

    fn bit(self) -> u8 {
        1 << self.index()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum VerificationState {
    None,
    Pending(ChildId),
    Passed(ChildId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Budget {
    remaining: u8,
    consumed: u8,
}

impl Budget {
    fn new(initial: u8) -> Self {
        Self {
            remaining: initial,
            consumed: 0,
        }
    }

    fn spend(&mut self) {
        debug_assert!(self.remaining > 0);
        self.remaining -= 1;
        self.consumed += 1;
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Obligation {
    present: bool,
    parent: Option<u8>,
    terminal_return: Option<ReturnKind>,
    return_count: u8,
}

impl Obligation {
    const fn absent(parent: Option<u8>) -> Self {
        Self {
            present: false,
            parent,
            terminal_return: None,
            return_count: 0,
        }
    }

    const fn root() -> Self {
        Self {
            present: true,
            parent: None,
            terminal_return: None,
            return_count: 0,
        }
    }

    fn is_open(self) -> bool {
        self.present && self.terminal_return.is_none()
    }

    fn is_terminal(self) -> bool {
        self.present && self.terminal_return.is_some()
    }

    fn return_once(&mut self, kind: ReturnKind) {
        debug_assert!(self.is_open());
        self.terminal_return = Some(kind);
        self.return_count += 1;
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct CandidateRecord {
    generation: u8,
    fence_was_current: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Attempt {
    phase: InvocationPhase,
    generation: u8,
    lease_deadline: u8,
    wake_deadline: Option<u8>,
    stale_generation: Option<u8>,
    stale_record_generation: Option<u8>,
    candidate: Option<CandidateRecord>,
}

impl Attempt {
    const fn absent() -> Self {
        Self {
            phase: InvocationPhase::Absent,
            generation: 0,
            lease_deadline: 0,
            wake_deadline: None,
            stale_generation: None,
            stale_record_generation: None,
            candidate: None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ModelState {
    status: RunStatus,
    logical_time: u8,
    creation: Budget,
    starts: Budget,
    waits: Budget,
    queries: Budget,
    sponsor_available: u8,
    award_reserve: u8,
    awards_formed: u8,
    offer_status: OfferStatus,
    offer_was_created: bool,
    bids: u8,
    awards: u8,
    duplicate_advertise_pending: bool,
    obligations: [Obligation; CHILD_COUNT + 1],
    attempts: [Attempt; CHILD_COUNT],
    verification: VerificationState,
    invalid_candidate_commits: u8,
    authority_interval_open: bool,
}

impl Default for ModelState {
    fn default() -> Self {
        Self {
            status: RunStatus::Running,
            logical_time: 0,
            creation: Budget::new(INITIAL_CREATION_BUDGET),
            starts: Budget::new(INITIAL_START_BUDGET),
            waits: Budget::new(INITIAL_WAIT_BUDGET),
            queries: Budget::new(INITIAL_QUERY_BUDGET),
            sponsor_available: INITIAL_SPONSOR_FUNDS,
            award_reserve: 0,
            awards_formed: 0,
            offer_status: OfferStatus::Absent,
            offer_was_created: false,
            bids: 0,
            awards: 0,
            duplicate_advertise_pending: false,
            obligations: [
                Obligation::root(),
                Obligation::absent(Some(0)),
                Obligation::absent(Some(0)),
            ],
            attempts: [Attempt::absent(); CHILD_COUNT],
            verification: VerificationState::None,
            invalid_candidate_commits: 0,
            authority_interval_open: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    AdvertiseOffer,
    DeliverDuplicateAdvertise,
    Bid(ChildId),
    Award(ChildId),
    CloseOfferWindow,
    AdvanceTime,
    Yield(ChildId),
    Wake(ChildId),
    ExpireLease(ChildId),
    RecordStaleSubmission(ChildId),
    ReissueLease(ChildId),
    SubmitCurrent(ChildId),
    RequestVerification(ChildId),
    CompleteVerification,
    ReturnResult(ChildId),
    ReturnExhausted(ChildId),
    ReturnAgain(ChildId),
    ReturnRootResult,
    ReturnRootExhausted,
    ReturnRootAbstained,
    CancelRun,
    CrashController,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelBounds {
    pub creation_budget: u8,
    pub invocation_start_budget: u8,
    pub wait_budget: u8,
    pub protected_query_budget: u8,
    pub sponsor_funds_and_award_slots: u8,
    pub child_obligations: usize,
    pub offer_deadline: u8,
    pub maximum_logical_time: u8,
    pub duplicate_deliveries: u8,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelAssumptions {
    pub bounds: ModelBounds,
    pub fault_model: Vec<&'static str>,
    pub abstraction: Vec<&'static str>,
    pub termination_argument: &'static str,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct TerminalCounts {
    pub accepted: u64,
    pub exhausted: u64,
    pub abstained: u64,
    pub cancelled: u64,
    pub infrastructure_error: u64,
}

impl TerminalCounts {
    fn add(&mut self, status: RunStatus) {
        match status {
            RunStatus::Running => {}
            RunStatus::Accepted => self.accepted += 1,
            RunStatus::Exhausted => self.exhausted += 1,
            RunStatus::Abstained => self.abstained += 1,
            RunStatus::Cancelled => self.cancelled += 1,
            RunStatus::InfrastructureError => self.infrastructure_error += 1,
        }
    }

    pub fn total(&self) -> u64 {
        self.accepted + self.exhausted + self.abstained + self.cancelled + self.infrastructure_error
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelReport {
    pub mutation: Mutation,
    pub passed: bool,
    pub state_space_exhaustive: bool,
    pub reachable_states: u64,
    pub transitions: u64,
    pub terminal_states: TerminalCounts,
    pub nonterminal_states: u64,
    pub nonterminal_deadlocks: u64,
    pub nonterminal_cycles: u64,
    pub max_shortest_trace: usize,
    pub max_steps_to_terminal: Option<usize>,
    pub assumptions: ModelAssumptions,
    pub violation: Option<String>,
    pub shortest_counterexample: Vec<String>,
}

pub fn check(mutation: Mutation) -> ModelReport {
    let initial = ModelState::default();
    let mut states = vec![initial.clone()];
    let mut depths = vec![0_usize];
    let mut predecessors: Vec<Option<(usize, Action)>> = vec![None];
    let mut edges: Vec<Vec<usize>> = vec![Vec::new()];
    let mut state_ids = HashMap::from([(initial, 0_usize)]);
    let mut queue = VecDeque::from([0_usize]);
    let mut transitions = 0_u64;
    let mut first_violation: Option<(usize, String)> = None;
    let mut max_shortest_trace = 0_usize;

    while let Some(state_id) = queue.pop_front() {
        let state = states[state_id].clone();
        if first_violation.is_none()
            && let Some(violation) = invariant_violation(&state)
        {
            first_violation = Some((state_id, violation));
        }

        for action in enabled_actions(&state, mutation) {
            let next = apply(state.clone(), action, mutation);
            transitions += 1;
            if first_violation.is_none() && next == state {
                first_violation = Some((
                    state_id,
                    format!("enabled transition {action:?} did not advance model state"),
                ));
            }
            let next_id = if let Some(existing) = state_ids.get(&next) {
                *existing
            } else {
                let next_id = states.len();
                let next_depth = depths[state_id] + 1;
                max_shortest_trace = max_shortest_trace.max(next_depth);
                state_ids.insert(next.clone(), next_id);
                states.push(next);
                depths.push(next_depth);
                predecessors.push(Some((state_id, action)));
                edges.push(Vec::new());
                queue.push_back(next_id);
                next_id
            };
            edges[state_id].push(next_id);
        }
    }

    let mut terminal_states = TerminalCounts::default();
    let mut nonterminal_states = 0_u64;
    let mut nonterminal_deadlocks = 0_u64;
    for (state_id, state) in states.iter().enumerate() {
        if state.status.is_terminal() {
            terminal_states.add(state.status);
            if first_violation.is_none() && !edges[state_id].is_empty() {
                first_violation = Some((
                    state_id,
                    "terminal state retained an enabled transition".to_owned(),
                ));
            }
        } else {
            nonterminal_states += 1;
            if edges[state_id].is_empty() {
                nonterminal_deadlocks += 1;
                if first_violation.is_none() {
                    first_violation = Some((state_id, "nonterminal deadlock".to_owned()));
                }
            }
        }
    }

    let (topological_order, first_cycle_state, nonterminal_cycles) =
        topological_order(&states, &edges);
    if first_violation.is_none()
        && let Some(cycle_state) = first_cycle_state
    {
        first_violation = Some((
            cycle_state,
            "reachable nonterminal cycle permits an unbounded schedule".to_owned(),
        ));
    }

    let max_steps_to_terminal = if nonterminal_cycles == 0 && nonterminal_deadlocks == 0 {
        Some(longest_path_to_terminal(
            &states,
            &edges,
            &topological_order,
        ))
    } else {
        None
    };
    let (violation, shortest_counterexample) = if let Some((state_id, violation)) = first_violation
    {
        (Some(violation), reconstruct_trace(state_id, &predecessors))
    } else {
        (None, Vec::new())
    };

    ModelReport {
        mutation,
        passed: violation.is_none(),
        state_space_exhaustive: true,
        reachable_states: states.len() as u64,
        transitions,
        terminal_states,
        nonterminal_states,
        nonterminal_deadlocks,
        nonterminal_cycles,
        max_shortest_trace,
        max_steps_to_terminal,
        assumptions: assumptions(),
        violation,
        shortest_counterexample,
    }
}

fn assumptions() -> ModelAssumptions {
    ModelAssumptions {
        bounds: ModelBounds {
            creation_budget: INITIAL_CREATION_BUDGET,
            invocation_start_budget: INITIAL_START_BUDGET,
            wait_budget: INITIAL_WAIT_BUDGET,
            protected_query_budget: INITIAL_QUERY_BUDGET,
            sponsor_funds_and_award_slots: INITIAL_SPONSOR_FUNDS,
            child_obligations: CHILD_COUNT,
            offer_deadline: OFFER_DEADLINE,
            maximum_logical_time: MAX_LOGICAL_TIME,
            duplicate_deliveries: 1,
        },
        fault_model: vec![
            "one duplicated advertise command with an ambiguous reply",
            "arbitrary finite delay represented by interleavings and bounded logical time",
            "lease and wake-deadline expiry with a stale submission after fencing",
            "authorized cancellation at every running state",
            "foreground-controller crash at every running state",
        ],
        abstraction: vec![
            "one funded negotiated offer with two independently reserved award slots",
            "one root obligation and at most two child obligations with fixed causal parentage",
            "one matching wake event class and one exact-candidate protected verification result",
            "mechanical actions only; the checker assigns no task meaning, priority, or winner",
        ],
        termination_argument: "the complete reachable graph is enumerated without a depth limit; every nonterminal state has a successor, every sink is terminal, and the reachable graph is acyclic, so the reported longest path is a finite bound for every maximal generated schedule",
    }
}

fn enabled_actions(state: &ModelState, mutation: Mutation) -> Vec<Action> {
    if state.status.is_terminal() {
        return Vec::new();
    }

    let mut actions = Vec::new();
    if state.offer_status == OfferStatus::Absent
        && state.logical_time < OFFER_DEADLINE
        && state.creation.remaining > 0
        && state.sponsor_available >= INITIAL_SPONSOR_FUNDS
        && !state.offer_was_created
    {
        actions.push(Action::AdvertiseOffer);
    }
    if state.duplicate_advertise_pending {
        actions.push(Action::DeliverDuplicateAdvertise);
    }

    if state.offer_status == OfferStatus::Advertised && state.logical_time < OFFER_DEADLINE {
        for child in ChildId::ALL {
            if state.bids & child.bit() == 0 {
                actions.push(Action::Bid(child));
            }
            if state.bids & child.bit() != 0
                && state.awards & child.bit() == 0
                && state.award_reserve > 0
                && state.creation.remaining > 0
                && state.starts.remaining > 0
            {
                actions.push(Action::Award(child));
            }
        }
    }
    if (state.offer_status == OfferStatus::Absent && state.logical_time >= OFFER_DEADLINE)
        || (state.offer_status == OfferStatus::Advertised
            && (state.logical_time >= OFFER_DEADLINE
                || state.awards_formed == INITIAL_SPONSOR_FUNDS))
    {
        actions.push(Action::CloseOfferWindow);
    }
    if state.logical_time < MAX_LOGICAL_TIME {
        actions.push(Action::AdvanceTime);
    }

    for child in ChildId::ALL {
        let obligation = state.obligations[child.obligation_index()];
        let attempt = state.attempts[child.index()];
        if obligation.is_open() {
            match attempt.phase {
                InvocationPhase::Running => {
                    if state.logical_time < attempt.lease_deadline {
                        if attempt.candidate.is_none() {
                            actions.push(Action::SubmitCurrent(child));
                        }
                        if state.waits.remaining > 0 {
                            actions.push(Action::Yield(child));
                        }
                    } else {
                        actions.push(Action::ExpireLease(child));
                    }
                }
                InvocationPhase::Yielded => {
                    let wake_is_live = attempt
                        .wake_deadline
                        .is_some_and(|deadline| state.logical_time < deadline)
                        && state.logical_time < attempt.lease_deadline;
                    if wake_is_live && state.starts.remaining > 0 {
                        actions.push(Action::Wake(child));
                    }
                    if !wake_is_live || state.logical_time >= attempt.lease_deadline {
                        actions.push(Action::ExpireLease(child));
                    }
                }
                InvocationPhase::Dormant => {
                    if attempt.stale_generation.is_some() {
                        actions.push(Action::RecordStaleSubmission(child));
                    } else if attempt.candidate.is_none()
                        && state.starts.remaining > 0
                        && state.logical_time < MAX_LOGICAL_TIME
                    {
                        actions.push(Action::ReissueLease(child));
                    } else if attempt.candidate.is_none() {
                        actions.push(Action::ReturnExhausted(child));
                    }
                }
                InvocationPhase::Absent | InvocationPhase::Complete => {}
            }
            if attempt
                .candidate
                .is_some_and(|candidate| candidate.fence_was_current)
            {
                actions.push(Action::ReturnResult(child));
            }
        } else if obligation.is_terminal()
            && mutation == Mutation::AllowSecondObligationReturn
            && obligation.return_count == 1
        {
            actions.push(Action::ReturnAgain(child));
        }

        if state.verification == VerificationState::None
            && state.queries.remaining > 0
            && attempt
                .candidate
                .is_some_and(|candidate| candidate.fence_was_current)
        {
            actions.push(Action::RequestVerification(child));
        }
    }

    if matches!(state.verification, VerificationState::Pending(_)) {
        actions.push(Action::CompleteVerification);
    }

    let descendants_terminal = state.obligations[1..]
        .iter()
        .filter(|obligation| obligation.present)
        .all(|obligation| obligation.is_terminal());
    let root_can_return = state.offer_status == OfferStatus::Closed
        && state.obligations[0].is_open()
        && descendants_terminal
        && !matches!(state.verification, VerificationState::Pending(_));
    if root_can_return {
        match state.verification {
            VerificationState::Passed(_) => actions.push(Action::ReturnRootResult),
            VerificationState::None => {
                if state.attempts.iter().any(|attempt| {
                    attempt
                        .candidate
                        .is_some_and(|candidate| candidate.fence_was_current)
                }) {
                    actions.push(Action::ReturnRootAbstained);
                } else {
                    actions.push(Action::ReturnRootExhausted);
                }
            }
            VerificationState::Pending(_) => {}
        }
    }

    actions.push(Action::CancelRun);
    actions.push(Action::CrashController);
    actions
}

fn apply(mut state: ModelState, action: Action, mutation: Mutation) -> ModelState {
    debug_assert_eq!(state.status, RunStatus::Running);
    match action {
        Action::AdvertiseOffer => {
            state.creation.spend();
            state.offer_status = OfferStatus::Advertised;
            state.offer_was_created = true;
            state.award_reserve = INITIAL_SPONSOR_FUNDS;
            state.sponsor_available -= INITIAL_SPONSOR_FUNDS;
            state.duplicate_advertise_pending = true;
        }
        Action::DeliverDuplicateAdvertise => {
            state.duplicate_advertise_pending = false;
        }
        Action::Bid(child) => {
            state.bids |= child.bit();
        }
        Action::Award(child) => {
            state.creation.spend();
            state.starts.spend();
            if mutation != Mutation::SkipAwardReservation {
                state.award_reserve -= 1;
            }
            state.awards_formed += 1;
            state.awards |= child.bit();
            state.obligations[child.obligation_index()] = Obligation {
                present: true,
                parent: Some(0),
                terminal_return: None,
                return_count: 0,
            };
            state.attempts[child.index()] = Attempt {
                phase: InvocationPhase::Running,
                generation: 1,
                lease_deadline: (state.logical_time + LEASE_DURATION).min(MAX_LOGICAL_TIME),
                wake_deadline: None,
                stale_generation: None,
                stale_record_generation: None,
                candidate: None,
            };
        }
        Action::CloseOfferWindow => close_offer(&mut state),
        Action::AdvanceTime => state.logical_time += 1,
        Action::Yield(child) => {
            state.waits.spend();
            let attempt = &mut state.attempts[child.index()];
            attempt.phase = InvocationPhase::Yielded;
            attempt.wake_deadline = Some(
                (state.logical_time + WAKE_DURATION)
                    .min(attempt.lease_deadline)
                    .min(MAX_LOGICAL_TIME),
            );
        }
        Action::Wake(child) => {
            state.starts.spend();
            let attempt = &mut state.attempts[child.index()];
            attempt.phase = InvocationPhase::Running;
            attempt.wake_deadline = None;
        }
        Action::ExpireLease(child) => {
            let attempt = &mut state.attempts[child.index()];
            let stale_generation = attempt.generation;
            attempt.generation += 1;
            attempt.phase = InvocationPhase::Dormant;
            attempt.wake_deadline = None;
            attempt.stale_generation = Some(stale_generation);
        }
        Action::RecordStaleSubmission(child) => {
            let attempt = &mut state.attempts[child.index()];
            let stale_generation = attempt
                .stale_generation
                .take()
                .expect("enabled stale submission has a generation");
            attempt.stale_record_generation = Some(stale_generation);
            if mutation == Mutation::IgnoreLeaseFence {
                attempt.candidate = Some(CandidateRecord {
                    generation: stale_generation,
                    fence_was_current: false,
                });
                state.invalid_candidate_commits += 1;
            }
        }
        Action::ReissueLease(child) => {
            state.starts.spend();
            let attempt = &mut state.attempts[child.index()];
            attempt.phase = InvocationPhase::Running;
            attempt.lease_deadline = (state.logical_time + LEASE_DURATION).min(MAX_LOGICAL_TIME);
        }
        Action::SubmitCurrent(child) => {
            let attempt = &mut state.attempts[child.index()];
            attempt.candidate = Some(CandidateRecord {
                generation: attempt.generation,
                fence_was_current: true,
            });
        }
        Action::RequestVerification(child) => {
            state.queries.spend();
            state.verification = VerificationState::Pending(child);
        }
        Action::CompleteVerification => {
            let VerificationState::Pending(child) = state.verification else {
                unreachable!("only a pending verification can complete");
            };
            state.verification = VerificationState::Passed(child);
        }
        Action::ReturnResult(child) => return_child(&mut state, child, ReturnKind::Result),
        Action::ReturnExhausted(child) => {
            return_child(&mut state, child, ReturnKind::Exhausted);
        }
        Action::ReturnAgain(child) => {
            state.obligations[child.obligation_index()].return_count += 1;
        }
        Action::ReturnRootResult => {
            state.obligations[0].return_once(ReturnKind::Result);
            finish_run(&mut state, RunStatus::Accepted, ReturnKind::Result);
        }
        Action::ReturnRootExhausted => {
            state.obligations[0].return_once(ReturnKind::Exhausted);
            finish_run(&mut state, RunStatus::Exhausted, ReturnKind::Exhausted);
        }
        Action::ReturnRootAbstained => {
            state.obligations[0].return_once(ReturnKind::Abstained);
            finish_run(&mut state, RunStatus::Abstained, ReturnKind::Abstained);
        }
        Action::CancelRun => finish_run(&mut state, RunStatus::Cancelled, ReturnKind::Cancelled),
        Action::CrashController => finish_run(
            &mut state,
            RunStatus::InfrastructureError,
            ReturnKind::InfrastructureError,
        ),
    }
    state
}

fn close_offer(state: &mut ModelState) {
    if state.offer_status == OfferStatus::Advertised {
        state.sponsor_available += state.award_reserve;
        state.award_reserve = 0;
    }
    state.offer_status = OfferStatus::Closed;
}

fn return_child(state: &mut ModelState, child: ChildId, kind: ReturnKind) {
    state.obligations[child.obligation_index()].return_once(kind);
    let attempt = &mut state.attempts[child.index()];
    attempt.phase = InvocationPhase::Complete;
    attempt.wake_deadline = None;
    attempt.stale_generation = None;
}

fn finish_run(state: &mut ModelState, status: RunStatus, return_kind: ReturnKind) {
    close_offer(state);
    for obligation in &mut state.obligations {
        if obligation.is_open() {
            obligation.return_once(return_kind);
        }
    }
    for attempt in &mut state.attempts {
        if attempt.phase != InvocationPhase::Absent {
            attempt.phase = InvocationPhase::Complete;
            attempt.wake_deadline = None;
            attempt.stale_generation = None;
        }
    }
    if status != RunStatus::Accepted {
        state.verification = VerificationState::None;
    }
    state.duplicate_advertise_pending = false;
    state.authority_interval_open = false;
    state.status = status;
}

fn invariant_violation(state: &ModelState) -> Option<String> {
    for (name, budget, initial) in [
        ("creation", state.creation, INITIAL_CREATION_BUDGET),
        ("invocation start", state.starts, INITIAL_START_BUDGET),
        ("wait", state.waits, INITIAL_WAIT_BUDGET),
        ("protected query", state.queries, INITIAL_QUERY_BUDGET),
    ] {
        if budget.remaining + budget.consumed != initial {
            return Some(format!("{name} budget was not conserved"));
        }
    }
    if state.sponsor_available + state.award_reserve + state.awards_formed != INITIAL_SPONSOR_FUNDS
    {
        return Some("award formed without consuming its separate reservation".to_owned());
    }
    if state.creation.consumed != u8::from(state.offer_was_created) + state.awards_formed {
        return Some("creation authority did not match created control objects".to_owned());
    }
    if state.awards.count_ones() as u8 != state.awards_formed {
        return Some("award count diverged from award records".to_owned());
    }
    if state.invalid_candidate_commits != 0
        || state.attempts.iter().any(|attempt| {
            attempt.candidate.is_some_and(|candidate| {
                !candidate.fence_was_current
                    || candidate.generation == 0
                    || candidate.generation > attempt.generation
            })
        })
    {
        return Some("stale lease generation created a current candidate".to_owned());
    }

    let root = state.obligations[0];
    if !root.present || root.parent.is_some() {
        return Some("root obligation lost its unique causal position".to_owned());
    }
    for child in ChildId::ALL {
        let obligation = state.obligations[child.obligation_index()];
        let was_awarded = state.awards & child.bit() != 0;
        if obligation.present != was_awarded || (obligation.present && obligation.parent != Some(0))
        {
            return Some("child obligation did not retain the root as causal parent".to_owned());
        }
        if obligation.return_count > 1 {
            return Some("obligation recorded more than one terminal return".to_owned());
        }
        if (obligation.return_count == 0) != obligation.terminal_return.is_none() {
            return Some("obligation return count diverged from terminal return".to_owned());
        }
        let attempt = state.attempts[child.index()];
        if obligation.present == (attempt.phase == InvocationPhase::Absent) {
            return Some("attempt presence diverged from its child obligation".to_owned());
        }
        if obligation.is_terminal() && attempt.phase != InvocationPhase::Complete {
            return Some("terminal child obligation retained a live attempt".to_owned());
        }
    }
    if root.is_terminal()
        && (state.offer_status != OfferStatus::Closed
            || state.obligations[1..]
                .iter()
                .filter(|obligation| obligation.present)
                .any(|obligation| !obligation.is_terminal()))
    {
        return Some("root obligation returned before its descendants".to_owned());
    }

    if state.status == RunStatus::Running && !state.authority_interval_open {
        return Some("running state lost its controller authority interval".to_owned());
    }
    if state.status.is_terminal() {
        if state.authority_interval_open {
            return Some("terminal state retained controller authority".to_owned());
        }
        if !root.is_terminal()
            || state
                .obligations
                .iter()
                .filter(|obligation| obligation.present)
                .any(|obligation| !obligation.is_terminal())
        {
            return Some("terminal run retained an open obligation".to_owned());
        }
    }
    if state.status == RunStatus::Accepted
        && (!matches!(state.verification, VerificationState::Passed(_))
            || root.terminal_return != Some(ReturnKind::Result))
    {
        return Some("run accepted without exact-candidate verification".to_owned());
    }
    if state.status == RunStatus::InfrastructureError
        && root.terminal_return != Some(ReturnKind::InfrastructureError)
    {
        return Some("controller crash did not return infrastructure_error".to_owned());
    }
    None
}

fn topological_order(
    states: &[ModelState],
    edges: &[Vec<usize>],
) -> (Vec<usize>, Option<usize>, u64) {
    let mut indegree = vec![0_usize; states.len()];
    for successors in edges {
        for successor in successors {
            indegree[*successor] += 1;
        }
    }
    let mut queue: VecDeque<_> = indegree
        .iter()
        .enumerate()
        .filter_map(|(state_id, degree)| (*degree == 0).then_some(state_id))
        .collect();
    let mut order = Vec::with_capacity(states.len());
    while let Some(state_id) = queue.pop_front() {
        order.push(state_id);
        for successor in &edges[state_id] {
            indegree[*successor] -= 1;
            if indegree[*successor] == 0 {
                queue.push_back(*successor);
            }
        }
    }
    let nonterminal_cycles = states
        .iter()
        .enumerate()
        .filter(|(state_id, state)| !state.status.is_terminal() && indegree[*state_id] > 0)
        .count() as u64;
    let first_cycle_state = states.iter().enumerate().find_map(|(state_id, state)| {
        (!state.status.is_terminal() && indegree[state_id] > 0).then_some(state_id)
    });
    (order, first_cycle_state, nonterminal_cycles)
}

fn longest_path_to_terminal(
    states: &[ModelState],
    edges: &[Vec<usize>],
    topological_order: &[usize],
) -> usize {
    let mut longest = vec![0_usize; states.len()];
    for state_id in topological_order.iter().rev() {
        if !states[*state_id].status.is_terminal() {
            longest[*state_id] = 1 + edges[*state_id]
                .iter()
                .map(|successor| longest[*successor])
                .max()
                .expect("a checked nonterminal state has a successor");
        }
    }
    longest[0]
}

fn reconstruct_trace(mut state_id: usize, predecessors: &[Option<(usize, Action)>]) -> Vec<String> {
    let mut trace = Vec::new();
    while let Some((previous, action)) = predecessors[state_id] {
        trace.push(format!("{action:?}"));
        state_id = previous;
    }
    trace.reverse();
    trace
}

#[cfg(test)]
mod tests {
    use super::{
        Action, ChildId, INITIAL_CREATION_BUDGET, INITIAL_QUERY_BUDGET, INITIAL_START_BUDGET,
        INITIAL_WAIT_BUDGET, InvocationPhase, ModelState, Mutation, OfferStatus, RunStatus,
        VerificationState, apply, check, enabled_actions,
    };

    fn awarded_child() -> ModelState {
        let state = apply(
            ModelState::default(),
            Action::AdvertiseOffer,
            Mutation::None,
        );
        let state = apply(state, Action::Bid(ChildId::One), Mutation::None);
        apply(state, Action::Award(ChildId::One), Mutation::None)
    }

    #[test]
    fn baseline_exhaustively_proves_safety_and_terminal_sinks() {
        let report = check(Mutation::None);
        assert!(report.passed, "{:?}", report.violation);
        assert!(report.state_space_exhaustive);
        assert_eq!(report.nonterminal_deadlocks, 0);
        assert_eq!(report.nonterminal_cycles, 0);
        assert!(report.max_steps_to_terminal.is_some());
        assert!(report.terminal_states.total() > 0);
        assert_eq!(
            report.terminal_states.total() + report.nonterminal_states,
            report.reachable_states
        );
        assert!(report.shortest_counterexample.is_empty());
        let serialized = serde_json::to_value(&report).expect("model report serializes");
        assert!(serialized.get("reachable_states").is_some());
        assert!(serialized.get("transitions").is_some());
        assert!(serialized.get("assumptions").is_some());
        assert!(serialized.get("shortest_counterexample").is_some());
    }

    #[test]
    fn budgets_are_independent_and_conserved() {
        let initial = ModelState::default();
        let advertised = apply(initial, Action::AdvertiseOffer, Mutation::None);
        assert_eq!(advertised.creation.remaining, INITIAL_CREATION_BUDGET - 1);
        assert_eq!(advertised.starts.remaining, INITIAL_START_BUDGET);
        assert_eq!(advertised.waits.remaining, INITIAL_WAIT_BUDGET);
        assert_eq!(advertised.queries.remaining, INITIAL_QUERY_BUDGET);

        let awarded = apply(
            apply(advertised, Action::Bid(ChildId::One), Mutation::None),
            Action::Award(ChildId::One),
            Mutation::None,
        );
        assert_eq!(awarded.starts.remaining, INITIAL_START_BUDGET - 1);
        assert_eq!(awarded.award_reserve, 1);
        assert_eq!(awarded.obligations[1].parent, Some(0));
    }

    #[test]
    fn duplicate_command_has_one_domain_effect() {
        let once = apply(
            ModelState::default(),
            Action::AdvertiseOffer,
            Mutation::None,
        );
        let replayed = apply(
            once.clone(),
            Action::DeliverDuplicateAdvertise,
            Mutation::None,
        );
        assert_eq!(once.creation, replayed.creation);
        assert_eq!(once.sponsor_available, replayed.sponsor_available);
        assert_eq!(once.award_reserve, replayed.award_reserve);
        assert_eq!(once.offer_status, replayed.offer_status);
        assert!(once.duplicate_advertise_pending);
        assert!(!replayed.duplicate_advertise_pending);
    }

    #[test]
    fn yield_and_wake_consume_separate_finite_budgets() {
        let awarded = awarded_child();
        let yielded = apply(awarded, Action::Yield(ChildId::One), Mutation::None);
        assert_eq!(yielded.waits.remaining, INITIAL_WAIT_BUDGET - 1);
        assert_eq!(yielded.attempts[0].phase, InvocationPhase::Yielded);
        let woken = apply(yielded, Action::Wake(ChildId::One), Mutation::None);
        assert_eq!(woken.starts.remaining, INITIAL_START_BUDGET - 2);
        assert_eq!(woken.attempts[0].phase, InvocationPhase::Running);
    }

    #[test]
    fn expired_wake_is_fenced_and_records_stale_submission_only() {
        let state = apply(awarded_child(), Action::Yield(ChildId::One), Mutation::None);
        let state = apply(state, Action::AdvanceTime, Mutation::None);
        assert!(
            enabled_actions(&state, Mutation::None).contains(&Action::ExpireLease(ChildId::One))
        );
        assert!(!enabled_actions(&state, Mutation::None).contains(&Action::Wake(ChildId::One)));
        let state = apply(state, Action::ExpireLease(ChildId::One), Mutation::None);
        let state = apply(
            state,
            Action::RecordStaleSubmission(ChildId::One),
            Mutation::None,
        );
        assert!(state.attempts[0].stale_record_generation.is_some());
        assert!(state.attempts[0].candidate.is_none());
    }

    #[test]
    fn current_candidate_spends_only_the_protected_query_budget() {
        let state = apply(
            awarded_child(),
            Action::SubmitCurrent(ChildId::One),
            Mutation::None,
        );
        let state = apply(
            state,
            Action::RequestVerification(ChildId::One),
            Mutation::None,
        );
        assert_eq!(state.queries.remaining, 0);
        assert!(matches!(
            state.verification,
            VerificationState::Pending(ChildId::One)
        ));
        assert_eq!(state.creation.remaining, INITIAL_CREATION_BUDGET - 2);
    }

    #[test]
    fn cancellation_and_crash_are_terminal_and_cannot_resume() {
        let cancelled = apply(awarded_child(), Action::CancelRun, Mutation::None);
        assert_eq!(cancelled.status, RunStatus::Cancelled);
        assert!(enabled_actions(&cancelled, Mutation::None).is_empty());

        let crashed = apply(awarded_child(), Action::CrashController, Mutation::None);
        assert_eq!(crashed.status, RunStatus::InfrastructureError);
        assert!(!crashed.authority_interval_open);
        assert!(
            crashed
                .obligations
                .iter()
                .filter(|obligation| obligation.present)
                .all(|obligation| obligation.return_count == 1)
        );
        assert!(enabled_actions(&crashed, Mutation::None).is_empty());
    }

    #[test]
    fn each_rule_change_has_a_shortest_counterexample_from_the_same_checker() {
        let cases = [
            (
                Mutation::SkipAwardReservation,
                "award formed without consuming its separate reservation",
            ),
            (
                Mutation::IgnoreLeaseFence,
                "stale lease generation created a current candidate",
            ),
            (
                Mutation::AllowSecondObligationReturn,
                "obligation recorded more than one terminal return",
            ),
        ];
        for (mutation, expected) in cases {
            let report = check(mutation);
            assert!(!report.passed, "mutation {mutation:?} survived");
            assert_eq!(report.violation.as_deref(), Some(expected));
            assert!(report.state_space_exhaustive);
            assert!(!report.shortest_counterexample.is_empty());
        }
    }

    #[test]
    fn two_children_and_root_each_return_once() {
        let state = apply(
            ModelState::default(),
            Action::AdvertiseOffer,
            Mutation::None,
        );
        let state = apply(state, Action::Bid(ChildId::One), Mutation::None);
        let state = apply(state, Action::Bid(ChildId::Two), Mutation::None);
        let state = apply(state, Action::Award(ChildId::One), Mutation::None);
        let state = apply(state, Action::Award(ChildId::Two), Mutation::None);
        let state = apply(state, Action::CloseOfferWindow, Mutation::None);
        assert_eq!(state.offer_status, OfferStatus::Closed);
        assert!(!enabled_actions(&state, Mutation::None).contains(&Action::ReturnRootExhausted));
        let state = apply(
            apply(state, Action::SubmitCurrent(ChildId::One), Mutation::None),
            Action::ReturnResult(ChildId::One),
            Mutation::None,
        );
        assert_eq!(state.obligations[1].return_count, 1);
        assert_eq!(state.obligations[2].return_count, 0);
        assert!(!enabled_actions(&state, Mutation::None).contains(&Action::ReturnRootAbstained));
        let state = apply(
            apply(state, Action::SubmitCurrent(ChildId::Two), Mutation::None),
            Action::ReturnResult(ChildId::Two),
            Mutation::None,
        );
        let state = apply(
            state,
            Action::RequestVerification(ChildId::One),
            Mutation::None,
        );
        let state = apply(state, Action::CompleteVerification, Mutation::None);
        let state = apply(state, Action::ReturnRootResult, Mutation::None);
        assert_eq!(state.status, RunStatus::Accepted);
        assert!(
            state
                .obligations
                .iter()
                .all(|obligation| obligation.return_count == 1)
        );
    }
}

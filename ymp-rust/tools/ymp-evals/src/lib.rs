#![forbid(unsafe_code)]

use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mutation {
    None,
    MintCreationBudget,
    IgnoreLeaseFence,
    AcceptQuiescence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Status {
    Running,
    Accepted,
    Exhausted,
    Cancelled,
    InfrastructureError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ModelState {
    status: Status,
    initial_creation_budget: u8,
    creation_budget: u8,
    generation: u8,
    open_obligation: bool,
    candidate_generation: Option<u8>,
    verified: bool,
    seen_commands: BTreeSet<&'static str>,
}

impl Default for ModelState {
    fn default() -> Self {
        Self {
            status: Status::Running,
            initial_creation_budget: 1,
            creation_budget: 1,
            generation: 0,
            open_obligation: false,
            candidate_generation: None,
            verified: false,
            seen_commands: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Open,
    DuplicateOpen,
    Expire,
    SubmitStale,
    SubmitCurrent,
    Verify,
    Tick,
    Cancel,
    Crash,
}

impl Action {
    const ALL: [Self; 9] = [
        Self::Open,
        Self::DuplicateOpen,
        Self::Expire,
        Self::SubmitStale,
        Self::SubmitCurrent,
        Self::Verify,
        Self::Tick,
        Self::Cancel,
        Self::Crash,
    ];
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelReport {
    pub mutation: Mutation,
    pub schedules_explored: u64,
    pub max_depth: usize,
    pub violation: Option<String>,
    pub counterexample: Vec<String>,
}

pub fn check(mutation: Mutation, max_depth: usize) -> ModelReport {
    let mut report = ModelReport {
        mutation,
        schedules_explored: 0,
        max_depth,
        violation: None,
        counterexample: Vec::new(),
    };
    let mut schedule = Vec::new();
    explore(
        ModelState::default(),
        mutation,
        max_depth,
        &mut schedule,
        &mut report,
    );
    report
}

fn explore(
    state: ModelState,
    mutation: Mutation,
    remaining: usize,
    schedule: &mut Vec<Action>,
    report: &mut ModelReport,
) {
    if report.violation.is_some() {
        return;
    }
    report.schedules_explored += 1;
    if let Some(violation) = invariant_violation(&state) {
        report.violation = Some(violation.to_owned());
        report.counterexample = schedule
            .iter()
            .map(|action| format!("{action:?}"))
            .collect();
        return;
    }
    if remaining == 0 {
        return;
    }
    for action in Action::ALL {
        schedule.push(action);
        explore(
            apply(state.clone(), action, mutation),
            mutation,
            remaining - 1,
            schedule,
            report,
        );
        schedule.pop();
        if report.violation.is_some() {
            return;
        }
    }
}

fn apply(mut state: ModelState, action: Action, mutation: Mutation) -> ModelState {
    if state.status != Status::Running {
        return state;
    }
    match action {
        Action::Open | Action::DuplicateOpen => {
            let command_id = "open-1";
            if !state.seen_commands.insert(command_id) {
                return state;
            }
            if state.creation_budget > 0 {
                if mutation == Mutation::MintCreationBudget {
                    state.creation_budget += 1;
                } else {
                    state.creation_budget -= 1;
                }
                state.open_obligation = true;
            }
        }
        Action::Expire => {
            state.generation += 1;
            state.open_obligation = false;
            state.candidate_generation = None;
        }
        Action::SubmitStale => {
            if mutation == Mutation::IgnoreLeaseFence {
                state.candidate_generation = Some(state.generation.saturating_sub(1));
            }
        }
        Action::SubmitCurrent if state.open_obligation => {
            state.candidate_generation = Some(state.generation);
            state.open_obligation = false;
        }
        Action::SubmitCurrent => {}
        Action::Verify if state.candidate_generation == Some(state.generation) => {
            state.verified = true;
            state.status = Status::Accepted;
            state.open_obligation = false;
        }
        Action::Verify => {}
        Action::Tick
            if state.creation_budget == 0
                && !state.open_obligation
                && state.candidate_generation.is_none() =>
        {
            state.status = if mutation == Mutation::AcceptQuiescence {
                Status::Accepted
            } else {
                Status::Exhausted
            };
        }
        Action::Tick => {}
        Action::Cancel => {
            state.status = Status::Cancelled;
            state.open_obligation = false;
        }
        Action::Crash => {
            state.status = Status::InfrastructureError;
            state.open_obligation = false;
        }
    }
    state
}

fn invariant_violation(state: &ModelState) -> Option<&'static str> {
    if state.creation_budget > state.initial_creation_budget {
        return Some("creation budget was minted");
    }
    if state
        .candidate_generation
        .is_some_and(|generation| generation != state.generation)
    {
        return Some("stale lease generation created a candidate");
    }
    if state.status == Status::Accepted && !state.verified {
        return Some("run accepted without exact-candidate verification");
    }
    if state.status != Status::Running && state.open_obligation {
        return Some("terminal run retained an open obligation");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{Action, Status};
    use super::{ModelState, Mutation, apply, check};

    #[test]
    fn baseline_preserves_invariants() {
        assert!(check(Mutation::None, 6).violation.is_none());
    }

    #[test]
    fn negative_controls_produce_counterexamples() {
        for mutation in [
            Mutation::MintCreationBudget,
            Mutation::IgnoreLeaseFence,
            Mutation::AcceptQuiescence,
        ] {
            let report = check(mutation, 6);
            assert!(report.violation.is_some(), "mutation {mutation:?} survived");
            assert!(!report.counterexample.is_empty());
        }
    }

    #[test]
    fn duplicate_command_has_one_effect() {
        let initial = ModelState::default();
        let once = apply(initial, Action::Open, Mutation::None);
        let twice = apply(once.clone(), Action::DuplicateOpen, Mutation::None);
        assert_eq!(once, twice);
    }

    #[test]
    fn crash_is_terminal_infrastructure_error() {
        let crashed = apply(ModelState::default(), Action::Crash, Mutation::None);
        assert_eq!(crashed.status, Status::InfrastructureError);
        assert!(!crashed.open_obligation);
    }
}

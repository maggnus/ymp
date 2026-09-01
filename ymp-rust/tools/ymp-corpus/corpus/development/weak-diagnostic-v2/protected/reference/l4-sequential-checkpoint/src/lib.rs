use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command {
    pub id: String,
    pub expected_generation: u64,
    pub delta: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub generation: u64,
    pub value: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyError {
    CommandConflict,
    GenerationMismatch { expected: u64, actual: u64 },
    ValueOverflow,
}

#[derive(Default)]
pub struct Ledger {
    generation: u64,
    value: i64,
    outcomes: BTreeMap<String, (Command, Result<Receipt, ApplyError>)>,
}

impl Ledger {
    pub fn apply(&mut self, command: Command) -> Result<Receipt, ApplyError> {
        if let Some((recorded, outcome)) = self.outcomes.get(&command.id) {
            return if recorded == &command {
                outcome.clone()
            } else {
                Err(ApplyError::CommandConflict)
            };
        }
        let outcome = if command.expected_generation != self.generation {
            Err(ApplyError::GenerationMismatch {
                expected: command.expected_generation,
                actual: self.generation,
            })
        } else if let Some(next) = self.value.checked_add(command.delta) {
            self.generation += 1;
            self.value = next;
            Ok(Receipt {
                generation: self.generation,
                value: self.value,
            })
        } else {
            Err(ApplyError::ValueOverflow)
        };
        self.outcomes
            .insert(command.id.clone(), (command, outcome.clone()));
        outcome
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn value(&self) -> i64 {
        self.value
    }
}

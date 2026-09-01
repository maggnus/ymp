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
    accepted: BTreeMap<String, (Command, Receipt)>,
}

impl Ledger {
    pub fn apply(&mut self, command: Command) -> Result<Receipt, ApplyError> {
        if let Some((recorded, receipt)) = self.accepted.get(&command.id) {
            return if recorded == &command {
                Ok(receipt.clone())
            } else {
                Err(ApplyError::CommandConflict)
            };
        }
        if command.expected_generation != self.generation {
            return Err(ApplyError::GenerationMismatch {
                expected: command.expected_generation,
                actual: self.generation,
            });
        }
        let next = self
            .value
            .checked_add(command.delta)
            .ok_or(ApplyError::ValueOverflow)?;
        self.generation += 1;
        self.value = next;
        let receipt = Receipt {
            generation: self.generation,
            value: self.value,
        };
        self.accepted
            .insert(command.id.clone(), (command, receipt.clone()));
        Ok(receipt)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn value(&self) -> i64 {
        self.value
    }
}

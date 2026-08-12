use std::collections::HashMap;

use crate::{Command, CommandResult, LedgerError, Outcome};

pub struct Ledger {
    available: u64,
    completed: HashMap<String, CommandResult>,
}

impl Ledger {
    pub fn new(available: u64) -> Self {
        Self {
            available,
            completed: HashMap::new(),
        }
    }

    pub fn available(&self) -> u64 {
        self.available
    }

    pub fn execute(&mut self, command_id: &str, command: Command) -> Result<Outcome, LedgerError> {
        if command_id.is_empty() {
            return Err(LedgerError::InvalidCommandId);
        }
        if let Some(result) = self.completed.get(command_id) {
            return Ok(Outcome {
                result: result.clone(),
                replayed: true,
            });
        }
        let result = match command {
            Command::Reserve { amount: 0 } | Command::Credit { amount: 0 } => {
                return Err(LedgerError::InvalidAmount);
            }
            Command::Reserve { amount } if amount > self.available => {
                return Err(LedgerError::Insufficient);
            }
            Command::Reserve { amount } => {
                self.available -= amount;
                CommandResult::Reserved {
                    remaining: self.available,
                }
            }
            Command::Credit { amount } => {
                self.available = self
                    .available
                    .checked_add(amount)
                    .ok_or(LedgerError::Overflow)?;
                CommandResult::Credited {
                    available: self.available,
                }
            }
        };
        self.completed.insert(command_id.to_owned(), result.clone());
        Ok(Outcome {
            result,
            replayed: false,
        })
    }
}

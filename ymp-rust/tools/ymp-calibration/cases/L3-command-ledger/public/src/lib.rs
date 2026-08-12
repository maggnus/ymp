#![forbid(unsafe_code)]

mod ledger;

pub use ledger::Ledger;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Reserve { amount: u64 },
    Credit { amount: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandResult {
    Reserved { remaining: u64 },
    Credited { available: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LedgerError {
    InvalidCommandId,
    InvalidAmount,
    Insufficient,
    Overflow,
    IdempotencyConflict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Outcome {
    pub result: CommandResult,
    pub replayed: bool,
}

#[cfg(test)]
mod tests {
    use super::{Command, CommandResult, Ledger, Outcome};

    #[test]
    fn applies_public_examples() {
        let mut ledger = Ledger::new(10);
        assert_eq!(
            ledger.execute("reserve-1", Command::Reserve { amount: 4 }),
            Ok(Outcome {
                result: CommandResult::Reserved { remaining: 6 },
                replayed: false,
            })
        );
        assert_eq!(
            ledger.execute("credit-1", Command::Credit { amount: 3 }),
            Ok(Outcome {
                result: CommandResult::Credited { available: 9 },
                replayed: false,
            })
        );
        assert_eq!(ledger.available(), 9);
    }
}

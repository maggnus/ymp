use ymp_calibration_case::{Command, CommandResult, Ledger, LedgerError, Outcome};

#[test]
fn protected_rejected_results_survive_state_changes() {
    let mut ledger = Ledger::new(2);
    assert_eq!(
        ledger.execute("reserve", Command::Reserve { amount: 3 }),
        Err(LedgerError::Insufficient)
    );
    assert!(
        ledger
            .execute("credit", Command::Credit { amount: 5 })
            .is_ok()
    );
    assert_eq!(
        ledger.execute("reserve", Command::Reserve { amount: 3 }),
        Err(LedgerError::Insufficient)
    );
    assert_eq!(ledger.available(), 7);

    let mut overflow = Ledger::new(u64::MAX);
    assert_eq!(
        overflow.execute("credit", Command::Credit { amount: 1 }),
        Err(LedgerError::Overflow)
    );
    assert!(
        overflow
            .execute("reserve", Command::Reserve { amount: 1 })
            .is_ok()
    );
    assert_eq!(
        overflow.execute("credit", Command::Credit { amount: 1 }),
        Err(LedgerError::Overflow)
    );
    assert_eq!(overflow.available(), u64::MAX - 1);
}

#[test]
fn protected_success_replay_and_conflict_are_side_effect_free() {
    let mut ledger = Ledger::new(10);
    assert_eq!(
        ledger.execute("same", Command::Reserve { amount: 4 }),
        Ok(Outcome {
            result: CommandResult::Reserved { remaining: 6 },
            replayed: false,
        })
    );
    assert_eq!(
        ledger.execute("same", Command::Reserve { amount: 4 }),
        Ok(Outcome {
            result: CommandResult::Reserved { remaining: 6 },
            replayed: true,
        })
    );
    assert_eq!(
        ledger.execute("same", Command::Reserve { amount: 5 }),
        Err(LedgerError::IdempotencyConflict)
    );
    assert_eq!(ledger.available(), 6);
}

#[test]
fn protected_empty_identifier_never_claims_a_key() {
    let mut ledger = Ledger::new(1);
    assert_eq!(
        ledger.execute("", Command::Reserve { amount: 1 }),
        Err(LedgerError::InvalidCommandId)
    );
    assert_eq!(
        ledger.execute("", Command::Credit { amount: 1 }),
        Err(LedgerError::InvalidCommandId)
    );
    assert_eq!(ledger.available(), 1);
}

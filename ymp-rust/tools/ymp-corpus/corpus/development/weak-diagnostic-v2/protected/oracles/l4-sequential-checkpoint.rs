use l4_sequential_checkpoint::{ApplyError, Command, Ledger};

fn command(id: &str, generation: u64, delta: i64) -> Command {
    Command {
        id: id.into(),
        expected_generation: generation,
        delta,
    }
}

#[test]
fn rejected_identity_is_stable_after_progress() {
    let mut ledger = Ledger::default();
    let rejected = command("held", 3, 9);
    assert_eq!(
        ledger.apply(rejected.clone()),
        Err(ApplyError::GenerationMismatch {
            expected: 3,
            actual: 0,
        })
    );
    ledger.apply(command("advance", 0, 1)).unwrap();
    assert_eq!(
        ledger.apply(rejected),
        Err(ApplyError::GenerationMismatch {
            expected: 3,
            actual: 0,
        })
    );
}

#[test]
fn rejected_transition_preserves_state() {
    let mut ledger = Ledger::default();
    ledger.apply(command("seed", 0, i64::MAX)).unwrap();
    assert_eq!(
        ledger.apply(command("overflow", 1, 1)),
        Err(ApplyError::ValueOverflow)
    );
    assert_eq!((ledger.generation(), ledger.value()), (1, i64::MAX));
}

#[test]
fn identifier_conflict_precedes_current_state() {
    let mut ledger = Ledger::default();
    let original = command("fixed", 2, 8);
    assert!(ledger.apply(original.clone()).is_err());
    ledger.apply(command("advance", 0, 1)).unwrap();
    let mut changed = original;
    changed.delta = 7;
    assert_eq!(ledger.apply(changed), Err(ApplyError::CommandConflict));
}

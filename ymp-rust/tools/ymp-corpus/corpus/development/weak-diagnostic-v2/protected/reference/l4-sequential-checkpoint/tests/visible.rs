use l4_sequential_checkpoint::{ApplyError, Command, Ledger};

#[test]
fn commits_one_ordered_transition() {
    let mut ledger = Ledger::default();
    let receipt = ledger
        .apply(Command {
            id: "first".into(),
            expected_generation: 0,
            delta: 7,
        })
        .unwrap();
    assert_eq!(receipt.generation, 1);
    assert_eq!(receipt.value, 7);
}

#[test]
fn rejects_the_wrong_generation_without_mutation() {
    let mut ledger = Ledger::default();
    assert_eq!(
        ledger.apply(Command {
            id: "late".into(),
            expected_generation: 2,
            delta: 4,
        }),
        Err(ApplyError::GenerationMismatch {
            expected: 2,
            actual: 0,
        })
    );
    assert_eq!((ledger.generation(), ledger.value()), (0, 0));
}

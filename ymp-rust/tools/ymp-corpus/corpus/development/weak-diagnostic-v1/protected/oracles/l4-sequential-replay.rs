use l4_sequential_replay::Ledger;

#[test]
fn dependency_order_is_enforced() {
    let mut ledger = Ledger::default();
    assert!(ledger.apply_line("b|not-the-head|5").is_err());
    assert_eq!(ledger.state(), (0, None));
    let first = ledger.apply_line("a|GENESIS|2").unwrap();
    assert_eq!(ledger.state(), (2, Some(first.head.as_str())));
}

#[test]
fn rejected_identity_is_replayed_after_state_changes() {
    let mut ledger = Ledger::default();
    let rejection = ledger.apply_line("x|wrong|4").unwrap_err();
    let first = ledger.apply_line("a|GENESIS|2").unwrap();
    let replay = ledger
        .apply_line(&format!("x|{}|4", first.head))
        .unwrap_err();
    assert_eq!(replay, rejection);
    assert_eq!(ledger.state(), (2, Some(first.head.as_str())));
}

use l4_sequential_replay::Ledger;

#[test]
fn visible_chain_advances_in_order() {
    let mut ledger = Ledger::default();
    let first = ledger.apply_line("a|GENESIS|2").unwrap();
    let second = ledger.apply_line(&format!("b|{}|3", first.head)).unwrap();
    assert_eq!(second.total, 5);
}

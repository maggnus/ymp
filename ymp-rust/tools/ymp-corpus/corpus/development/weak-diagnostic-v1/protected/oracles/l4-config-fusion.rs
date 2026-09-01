use l4_config_fusion::{inventory::normalize_inventory, release_plan, routing::normalize_routes};

#[test]
fn branch_a_inventory_contract() {
    let exact = normalize_inventory(&[" api = 7 ", "api=7", "worker=3"]).unwrap();
    assert_eq!(exact.len(), 2);
    assert_eq!(exact["api"], 7);
    assert!(normalize_inventory(&["api=7", "api=8"]).is_err());
    assert!(normalize_inventory(&["=7"]).is_err());
}

#[test]
fn branch_b_routing_contract() {
    let exact = normalize_routes(&[" web -> api ", "web->api", "jobs->worker"]).unwrap();
    assert_eq!(exact.len(), 2);
    assert_eq!(exact["web"], "api");
    assert!(normalize_routes(&["web->api", "web->worker"]).is_err());
    assert!(normalize_routes(&["web->"]).is_err());
}

#[test]
fn integration_requires_complete_join() {
    assert!(release_plan(&["api=7"], &["web->missing"]).is_err());
    assert!(release_plan(&["api=7", "worker=3"], &["web->api"]).is_err());
    assert_eq!(
        release_plan(&["api=7", "worker=3"], &["web->api", "jobs->worker"]).unwrap(),
        ["jobs@worker#3", "web@api#7"]
    );
}

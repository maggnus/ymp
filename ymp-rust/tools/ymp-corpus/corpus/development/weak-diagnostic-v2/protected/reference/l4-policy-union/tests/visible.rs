use l4_policy_union::{QuotaPatch, RoutePatch, compile_plan};

#[test]
fn compiles_a_complete_plan() {
    let plan = compile_plan(
        [QuotaPatch {
            service: "api".into(),
            limit: 5,
        }],
        [RoutePatch {
            path: "/v1".into(),
            service: "api".into(),
        }],
    )
    .unwrap();
    assert_eq!(plan.quotas["api"], 5);
    assert_eq!(plan.routes["/v1"], "api");
}

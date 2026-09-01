use l4_policy_union::{PolicyError, QuotaPatch, RoutePatch, compile_plan};

fn quota(service: &str, limit: u32) -> QuotaPatch {
    QuotaPatch {
        service: service.into(),
        limit,
    }
}

fn route(path: &str, service: &str) -> RoutePatch {
    RoutePatch {
        path: path.into(),
        service: service.into(),
    }
}

#[test]
fn quota_branch_rejects_conflicts() {
    assert_eq!(
        compile_plan([quota("api", 2), quota("api", 3)], []),
        Err(PolicyError::QuotaConflict("api".into()))
    );
}

#[test]
fn route_branch_rejects_conflicts() {
    assert_eq!(
        compile_plan(
            [quota("a", 1), quota("b", 1)],
            [route("/x", "a"), route("/x", "b")],
        ),
        Err(PolicyError::RouteConflict("/x".into()))
    );
}

#[test]
fn integration_requires_complete_join() {
    assert_eq!(
        compile_plan([quota("api", 2)], [route("/jobs", "worker")]),
        Err(PolicyError::MissingQuota("worker".into()))
    );
}

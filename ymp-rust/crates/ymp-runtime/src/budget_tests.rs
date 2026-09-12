#[tokio::test]
async fn budget_resume_preserves_unknown_usage_stop_and_captured_limits() {
    let mut fixture = RunFixture::new("", false);
    fixture.engine.config.limits.resources.as_mut().unwrap().observed_tokens = Some(1000);
    fixture.engine.config.limits.resources.as_mut().unwrap().invocation_tokens = Some(100);
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "paused");
    assert!(outcome.summary.contains("unknown_usage"));
    let before = fixture.store.trace(&outcome.session.id).unwrap();
    assert!(before.usage.total.is_partial());
    assert_eq!(before.usage.total.known_total(), None);
    fixture.engine.config.limits = Limits::default();
    let resumed = fixture.engine.run(&fixture.project, "", Some(&outcome.session.id)).await.unwrap();
    let after = fixture.store.trace(&outcome.session.id).unwrap();
    assert_eq!(resumed.session.status, "paused");
    assert_eq!(after.invocations.len(), before.invocations.len());
    assert_eq!(after.budget.as_ref().unwrap().limits.resources.as_ref().unwrap().observed_tokens, Some(1000));
    assert_eq!(after.usage.total, before.usage.total);
    assert_eq!(after.budget.unwrap().last_denial.unwrap().code, "unknown_usage");
}
#[tokio::test]
async fn budget_context_limit_rejects_before_native_work_without_spending_invocations() {
    let mut fixture = RunFixture::new("", false);
    fixture.engine.config.limits.resources.as_mut().unwrap().startup_context_chars = 100;
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "paused");
    assert!(outcome.summary.contains("context_limit"));
    assert_eq!(outcome.session.turns_used, 0);
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    assert!(trace.invocations.is_empty());
    assert_eq!(trace.budget.unwrap().admitted_invocations, 0);
}
#[tokio::test]
async fn budget_all_purposes_share_usage_and_timeout_controls() {
    let fixture = RunFixture::new("[mock:usage]", true);
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "completed");
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    for purpose in ["plan", "review_plan", "bid", "execute", "review", "final_review", "learn", "review_memory", "synthesis"] {
        assert!(trace.assignments.iter().any(|a| a.purpose == purpose), "missing consumer purpose {purpose}");
    }
    let budget = trace.budget.unwrap();
    assert_eq!(budget.admitted_invocations, trace.assignments.len() as u64);
    assert_eq!(budget.observed_usage.known_total(), Some(budget.admitted_invocations * 120));
    assert_eq!(budget.in_flight_invocations, 0);
    assert_eq!(trace.history.iter().filter(|e| e.kind == "budget_controls").count(), trace.invocations.len());
    assert!(budget.require_strict_token_bound().unwrap_err().to_string().contains("opaque_native_token_bound"));
}
#[tokio::test]
async fn budget_minimum_startup_allowance_keeps_plan_review_possible() {
    let mut fixture = RunFixture::new("[mock:usage]", false);
    fixture.engine.config.limits.resources.as_mut().unwrap().startup_invocations = 2;
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "completed");
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    assert_eq!(trace.assignments.iter().filter(|a| a.purpose == "plan").count(), 1);
    assert_eq!(trace.budget.unwrap().startup_invocations, 2);
}
#[tokio::test]
async fn budget_native_output_stop_reaches_trace_with_actual_usage() {
    let mut fixture = RunFixture::new("[mock:usage]", false);
    fixture.engine.config.limits.resources.as_mut().unwrap().max_output_chars = 1;
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "paused");
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    assert_eq!(trace.budget.as_ref().unwrap().last_denial.as_ref().unwrap().code, "output_limit");
    assert!(trace.invocations.iter().all(|i| i.terminal_reason.as_deref() == Some("output_limit")));
    assert_eq!(trace.usage.total.known_total(), Some(trace.invocations.len() as u64 * 120));
    assert_eq!(trace.budget.unwrap().in_flight_invocations, 0);
}

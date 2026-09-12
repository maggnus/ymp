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

#[tokio::test]
async fn budget_public_resume_preserves_legacy_spend_with_missing_or_sparse_rows() {
    for sparse in [false, true] {
        let mut fixture = RunFixture::new("[mock:usage]", false);
        fixture.engine.config.limits.turns = 3;
        fixture.engine.config.limits.resources = None;
        let project = fixture.store.project(&fixture.project).unwrap();
        let session = Session { id:new_id(), project_id:project.id, title:"Legacy task".into(),
            status:"paused".into(), created_at:now(), team:fixture.engine.config.members(), turns_used:2 };
        fixture.store.save_session(&session).unwrap();
        fixture.store.capture_legacy_budget_limits(&session.id, &fixture.engine.config.limits).unwrap();
        if sparse {
            fixture.store.begin_usage(&session.id, 1, &session.team[0].id).unwrap();
            fixture.store.finish_usage(&session.id, 1, "interrupted").unwrap();
        }
        let outcome = fixture.engine.run(&fixture.project, "Create a greeting", Some(&session.id)).await.unwrap();
        assert_eq!(outcome.session.status, "paused");
        assert_eq!(outcome.session.turns_used, 3);
        let trace = fixture.store.trace(&session.id).unwrap();
        assert_eq!(trace.invocations.len(), 1);
        assert_eq!(trace.invocations[0].turn, 3);
        assert_eq!(trace.usage.total.calls, 3);
        assert_eq!(trace.usage.total.reported, 1);
        assert!(trace.usage.total.is_partial());
        assert_eq!(trace.budget.unwrap().last_denial.unwrap().code, "invocation_limit");
        let resumed = fixture.engine.run(&fixture.project, "", Some(&session.id)).await.unwrap();
        assert_eq!(resumed.session.turns_used, 3);
        assert_eq!(fixture.store.trace(&session.id).unwrap().invocations.len(), 1);
    }
}

#[tokio::test]
async fn joint_engine_budget_stops_never_leave_unaccounted_or_active_grants() {
    for mode in ["context", "unknown", "output"] {
        let mut fixture = RunFixture::new(if mode == "unknown" { "" } else { "[mock:usage]" }, false);
        let resources = fixture.engine.config.limits.resources.as_mut().unwrap();
        match mode {
            "context" => resources.startup_context_chars = 100,
            "unknown" => { resources.observed_tokens = Some(1000); resources.invocation_tokens = Some(100); }
            _ => resources.max_output_chars = 1,
        }
        let outcome = fixture.run().await;
        assert_eq!(outcome.session.status, "paused");
        let trace = fixture.store.trace(&outcome.session.id).unwrap();
        assert_eq!(trace.invocations.len(), if mode == "context" { 0 } else { 2 });
        assert_eq!(trace.history.iter().filter(|e| e.data["change"] == "grant_issued").count(), trace.invocations.len());
        assert_eq!(trace.history.iter().filter(|e| e.data["change"] == "grant_revoked").count(), trace.invocations.len());
        assert_eq!(trace.history.iter().filter(|e| e.kind == "budget_reserved").count(), trace.invocations.len());
        assert_eq!(trace.history.iter().filter(|e| e.kind == "budget_released").count(), trace.invocations.len());
        for assignment in &trace.assignments {
            assert_eq!(assignment.grant_ids.len(), 1);
            assert!(fixture.store.team_grant(&outcome.session.id, &assignment.grant_ids[0]).unwrap().revoked_at.is_some());
        }
        let budget = trace.budget.unwrap();
        assert_eq!(budget.admitted_invocations, trace.invocations.len() as u64);
        assert_eq!(budget.in_flight_invocations, 0);
        assert_eq!(budget.observed_usage.known_total(), match mode { "context" => Some(0), "unknown" => None, _ => Some(240) });
    }
}

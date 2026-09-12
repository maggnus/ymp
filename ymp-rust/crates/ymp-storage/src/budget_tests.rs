fn budget_fixture(total: u64, each: u64) -> Fixture {
    Fixture::with_limits(Limits { parallel: 4, resources: Some(ResourceLimits {
        observed_tokens: Some(total), invocation_tokens: Some(each), required_review_invocations: 1,
        ..Default::default()
    }), ..Default::default() })
}
fn budget_observe(f: &Fixture, invocation: &InvocationRecord, tokens: u64, partial: bool) {
    f.store.observe_invocation(&f.session.id, &invocation.id, &InvocationObservation {
        usage: Some(UsageSnapshot { counts: TokenCounts { input:Some(tokens), output:Some(0), ..Default::default() },
            finalized: !partial, partial, ..Default::default() }), ..Default::default()
    }).unwrap();
}
fn budget_assignment(f: &Fixture, purpose: &str) -> (AssignmentRecord, InvocationRecord) {
    let (mut a, i) = f.invocation(1);
    // The atomic allowance test does not create pending task-review obligations.
    a.task = None;
    a.purpose = purpose.into();
    (a, i)
}
#[test]
fn budget_concurrent_connections_reserve_once_and_keep_review_usable() {
    let f = budget_fixture(100, 30);
    f.store.db().unwrap().execute("DELETE FROM tasks WHERE session_id=?", [&f.session.id]).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let jobs: Vec<_> = (0..3).map(|_| {
        let store = Store::open(&f.store.home).unwrap();
        let (a, i) = budget_assignment(&f, "execute");
        let barrier = barrier.clone();
        std::thread::spawn(move || { barrier.wait(); store.admit_invocation(&a, i) })
    }).collect();
    barrier.wait();
    let results: Vec<_> = jobs.into_iter().map(|j| j.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 2);
    let admitted: Vec<_> = results.into_iter().filter_map(Result::ok).collect();
    assert_ne!(admitted[0].turn, admitted[1].turn);
    let open = f.store.session_budget(&f.session.id).unwrap().unwrap();
    assert_eq!(open.reserved_tokens, Some(60));
    assert_eq!(open.last_denial.unwrap().code, "token_review_reserve");
    for i in &admitted {
        budget_observe(&f, i, 35, false);
        f.store.finish_invocation(&f.session.id, &i.id, InvocationState::Completed, None).unwrap();
    }
    let (a, i) = budget_assignment(&f, "retry");
    assert!(f.store.admit_invocation(&a, i).is_err());
    let (a, i) = budget_assignment(&f, "review");
    let review = f.store.admit_invocation(&a, i).unwrap();
    budget_observe(&f, &review, 20, false);
    f.store.finish_invocation(&f.session.id, &review.id, InvocationState::Completed, None).unwrap();
    let (a, i) = budget_assignment(&f, "consultation");
    assert!(f.store.admit_invocation(&a, i).is_err());
    let closed = Store::open(&f.store.home).unwrap().session_budget(&f.session.id).unwrap().unwrap();
    assert_eq!(closed.admitted_invocations, 3);
    assert_eq!(closed.observed_usage.known_total(), Some(90));
    assert_eq!(closed.reserved_tokens, Some(0));
    assert_eq!(closed.last_denial.unwrap().code, "token_limit");
}
#[test]
fn budget_cancel_interrupt_and_failure_release_without_refunding_unknown_or_partial_spend() {
    for state in [InvocationState::Cancelled, InvocationState::Interrupted, InvocationState::Failed] {
        for partial in [None, Some(7)] {
            let f = budget_fixture(100, 20);
            let (a, i) = budget_assignment(&f, "execute");
            let i = f.store.admit_invocation(&a, i).unwrap();
            if let Some(tokens) = partial { budget_observe(&f, &i, tokens, true); }
            if state == InvocationState::Interrupted { f.store.interrupt_open_invocations(&f.session.id).unwrap(); }
            else { f.store.finish_invocation(&f.session.id, &i.id, state, None).unwrap(); }
            let b = f.store.session_budget(&f.session.id).unwrap().unwrap();
            assert_eq!(b.in_flight_invocations, 0);
            assert_eq!(b.reserved_tokens, Some(0));
            assert_eq!(b.observed_usage.known_total(), partial);
            assert!(b.observed_usage.is_partial());
            assert!(b.require_strict_token_bound().unwrap_err().to_string().contains("incomplete_native_accounting"));
            let (a, i) = budget_assignment(&f, "review");
            let error = f.store.admit_invocation(&a, i).unwrap_err();
            assert_eq!(error.downcast_ref::<BudgetDenial>().unwrap().code, "unknown_usage");
            assert_eq!(f.store.trace(&f.session.id).unwrap().invocations.len(), 1);
        }
    }
}
#[test]
fn budget_retains_observed_overshoot_and_rejects_retracted_spend() {
    let f = budget_fixture(100, 20);
    let (a, i) = budget_assignment(&f, "plan");
    let i = f.store.admit_invocation(&a, i).unwrap();
    budget_observe(&f, &i, 130, true);
    let error = f.store.observe_invocation(&f.session.id, &i.id, &InvocationObservation {
        usage: Some(UsageSnapshot { counts: TokenCounts::zero(), ..Default::default() }), ..Default::default()
    });
    assert!(error.is_err());
    let (a, next) = budget_assignment(&f, "final_review");
    assert!(f.store.admit_invocation(&a, next).is_err());
    f.store.interrupt_open_invocations(&f.session.id).unwrap();
    let b = f.store.session_budget(&f.session.id).unwrap().unwrap();
    assert_eq!(b.observed_token_overshoot, Some(30));
    assert_eq!(b.observed_usage.known_total(), Some(130));
    assert_eq!(b.reserved_tokens, Some(0));
    assert!(!b.strict_token_bound);
}
#[test]
fn budget_failed_event_rolls_back_reservation_and_ordinal() {
    let f = budget_fixture(100, 20);
    f.store.db().unwrap().execute_batch("CREATE TRIGGER fail_budget BEFORE INSERT ON events WHEN NEW.kind='budget_reserved' BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").unwrap();
    let (a, i) = budget_assignment(&f, "execute");
    assert!(f.store.admit_invocation(&a, i.clone()).is_err());
    let b = f.store.session_budget(&f.session.id).unwrap().unwrap();
    assert_eq!(b.admitted_invocations, 0);
    assert_eq!(b.reserved_tokens, Some(0));
    f.store.db().unwrap().execute_batch("DROP TRIGGER fail_budget;").unwrap();
    assert_eq!(f.store.admit_invocation(&a, i).unwrap().turn, 1);
}
#[test]
fn budget_legacy_resume_captures_new_limits_once_without_inventing_old_policy() {
    let f = Fixture::new();
    f.store.db().unwrap().execute("DELETE FROM session_policies WHERE session_id=?", [&f.session.id]).unwrap();
    let limits = Limits { turns: 1, resources: None, ..Default::default() };
    f.store.capture_legacy_budget_limits(&f.session.id, &limits).unwrap();
    let (a, i) = budget_assignment(&f, "review");
    let i = f.store.admit_invocation(&a, i).unwrap();
    f.store.finish_invocation(&f.session.id, &i.id, InvocationState::Cancelled, None).unwrap();
    f.store.capture_legacy_budget_limits(&f.session.id, &Limits::default()).unwrap();
    let (a, i) = budget_assignment(&f, "review");
    assert!(f.store.admit_invocation(&a, i).unwrap_err().to_string().contains("invocation_limit"));
    assert!(f.store.session_policy(&f.session.id).unwrap().is_none());
    let b = f.store.session_budget(&f.session.id).unwrap().unwrap();
    assert_eq!(b.limits.turns, 1);
    assert_eq!(b.observed_usage.known_total(), None);
}

#[test]
fn budget_legacy_missing_calls_remain_additive_across_sparse_concurrent_admission() {
    for sparse in [false, true] {
        for policy_present in [false, true] {
            let limits = Limits { turns: 3, parallel: 4, resources: None, ..Default::default() };
            let mut f = Fixture::with_limits(limits.clone());
            f.session.turns_used = 2;
            f.store.save_session(&f.session).unwrap();
            if !policy_present {
                f.store.db().unwrap().execute("DELETE FROM session_policies WHERE session_id=?", [&f.session.id]).unwrap();
                f.store.capture_legacy_budget_limits(&f.session.id, &limits).unwrap();
            }
            if sparse {
                f.store.begin_usage(&f.session.id, 1, "writer").unwrap();
                f.store.update_usage(&f.session.id, 1, &UsageSnapshot {
                    counts: TokenCounts { input: Some(7), output: Some(0), ..Default::default() },
                    finalized: true, ..Default::default()
                }).unwrap();
                f.store.finish_usage(&f.session.id, 1, "completed").unwrap();
            }
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
            let jobs: Vec<_> = (0..3).map(|_| {
                let store = Store::open(&f.store.home).unwrap();
                let (a, i) = budget_assignment(&f, "review");
                let barrier = barrier.clone();
                std::thread::spawn(move || { barrier.wait(); store.admit_invocation(&a, i) })
            }).collect();
            barrier.wait();
            let results: Vec<_> = jobs.into_iter().map(|j| j.join().unwrap()).collect();
            assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1, "sparse={sparse}, policy={policy_present}");
            let i = results.into_iter().find_map(Result::ok).unwrap();
            assert_eq!(i.turn, 3);
            budget_observe(&f, &i, 9, false);
            f.store.finish_invocation(&f.session.id, &i.id, InvocationState::Completed, None).unwrap();
            let reopened = Store::open(&f.store.home).unwrap();
            let b = reopened.session_budget(&f.session.id).unwrap().unwrap();
            assert_eq!(b.admitted_invocations, 3);
            assert_eq!(b.observed_usage.calls, 3);
            assert_eq!(b.observed_usage.reported, if sparse { 2 } else { 1 });
            assert_eq!(b.observed_usage.known_total(), Some(if sparse { 16 } else { 9 }));
            assert!(b.observed_usage.is_partial());
            assert_eq!(b.in_flight_invocations, 0);
            let (a, next) = budget_assignment(&f, "review");
            assert!(reopened.admit_invocation(&a, next).unwrap_err().to_string().contains("invocation_limit"));
        }
    }
}

#[test]
fn budget_session_save_cannot_rewind_missing_historical_spend() {
    let mut f = Fixture::new();
    f.session.turns_used = 4;
    f.store.save_session(&f.session).unwrap();
    f.session.turns_used = 1;
    f.store.save_session(&f.session).unwrap();
    assert_eq!(f.store.session(&f.session.id).unwrap().turns_used, 4);
    assert_eq!(f.store.session_usage(&f.session.id).unwrap().total.calls, 4);
    let (a, i) = budget_assignment(&f, "review");
    assert_eq!(f.store.admit_invocation(&a, i).unwrap().turn, 5);
}

#[test]
fn joint_grant_and_budget_events_roll_back_as_one_admission() {
    let f = budget_fixture(100, 20);
    let (mut assignment, invocation) = budget_assignment(&f, "plan");
    let grant = GrantRecord::for_assignment(&assignment,&invocation,TeamOperation::coordination());
    assignment.grant_ids.push(grant.id.clone());
    f.store.db().unwrap().execute_batch("CREATE TRIGGER fail_joint BEFORE INSERT ON events WHEN NEW.kind='budget_reserved' BEGIN SELECT RAISE(ABORT, 'injected budget event failure'); END;").unwrap();
    assert!(f.store.admit_invocation_with_grants(&assignment,invocation.clone(),std::slice::from_ref(&grant)).is_err());
    assert!(f.store.team_grant(&f.session.id,&grant.id).is_err());
    let trace = f.store.trace(&f.session.id).unwrap();
    assert!(trace.invocations.is_empty());
    assert!(!trace.history.iter().any(|e|e.data["change"]=="grant_issued"));
    assert_eq!(trace.budget.unwrap().reserved_tokens,Some(0));
    f.store.db().unwrap().execute_batch("DROP TRIGGER fail_joint;").unwrap();
    let invocation = f.store.admit_invocation_with_grants(&assignment,invocation,std::slice::from_ref(&grant)).unwrap();
    assert_eq!(invocation.turn,1);
    assert_eq!(f.store.session_budget(&f.session.id).unwrap().unwrap().reserved_tokens,Some(20));
    assert!(f.store.team_grant(&f.session.id,&grant.id).unwrap().revoked_at.is_none());
}

#[test]
fn explicit_storage_wrappers_cannot_replace_unknown_history_or_skip_allowance() {
    for with_grants in [false, true] {
        for sparse in [false, true] {
            for legacy in [false, true] {
                let limits = Limits { turns: 3, parallel: 3, resources: None, ..Default::default() };
                let mut f = Fixture::with_limits(limits.clone());
                f.session.turns_used = 2;
                f.store.save_session(&f.session).unwrap();
                if legacy {
                    f.store.db().unwrap().execute("DELETE FROM session_policies WHERE session_id=?", [&f.session.id]).unwrap();
                    f.store.capture_legacy_budget_limits(&f.session.id, &limits).unwrap();
                }
                if sparse {
                    f.store.begin_usage(&f.session.id, 1, "writer").unwrap();
                    f.store.finish_usage(&f.session.id, 1, "interrupted").unwrap();
                }
                for turn in [1, 2, 4] {
                    let (mut a, mut i) = budget_assignment(&f, "review");
                    i.turn = turn;
                    let grant = GrantRecord::for_assignment(&a, &i, TeamOperation::coordination());
                    let result = if with_grants {
                        a.grant_ids.push(grant.id.clone());
                        f.store.begin_invocation_with_grants(&a, &i, std::slice::from_ref(&grant))
                    } else { f.store.begin_invocation(&a, &i) };
                    assert!(result.is_err(), "accepted historical or skipped ordinal {turn}; grants={with_grants}, sparse={sparse}, legacy={legacy}");
                    assert!(f.store.team_grant(&f.session.id, &grant.id).is_err());
                    assert!(f.store.trace(&f.session.id).unwrap().invocations.is_empty());
                    assert_eq!(f.store.session_usage(&f.session.id).unwrap().total.calls, 2);
                }
                let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
                let jobs: Vec<_> = ["writer", "reviewer"].into_iter().map(|agent| {
                    let store = Store::open(&f.store.home).unwrap();
                    let (mut a, mut i) = budget_assignment(&f, "review");
                    a.agent_id = agent.into();
                    i.turn = 3;
                    let grant = GrantRecord::for_assignment(&a, &i, TeamOperation::coordination());
                    let barrier = barrier.clone();
                    std::thread::spawn(move || {
                        barrier.wait();
                        let result = if with_grants {
                            a.grant_ids.push(grant.id.clone());
                            store.begin_invocation_with_grants(&a, &i, std::slice::from_ref(&grant))
                        } else { store.begin_invocation(&a, &i) };
                        (a, i, result)
                    })
                }).collect();
                barrier.wait();
                let results: Vec<_> = jobs.into_iter().map(|job|job.join().unwrap()).collect();
                assert_eq!(results.iter().filter(|(_,_,r)|r.is_ok()).count(), 1);
                let (_, invocation, _) = results.into_iter().find(|(_,_,r)|r.is_ok()).unwrap();
                budget_observe(&f, &invocation, 9, false);
                f.store.finish_invocation(&f.session.id, &invocation.id, InvocationState::Cancelled, None).unwrap();
                let reopened = Store::open(&f.store.home).unwrap();
                let usage = reopened.session_usage(&f.session.id).unwrap().total;
                assert_eq!(usage.calls, 3);
                assert_eq!(usage.reported, 1);
                assert_eq!(usage.known_total(), Some(9));
                assert!(usage.is_partial());
                let (mut a, mut i) = budget_assignment(&f, "review");
                i.turn = 4;
                let grant = GrantRecord::for_assignment(&a, &i, TeamOperation::coordination());
                let result = if with_grants {
                    a.grant_ids.push(grant.id.clone());
                    reopened.begin_invocation_with_grants(&a, &i, std::slice::from_ref(&grant))
                } else { reopened.begin_invocation(&a, &i) };
                assert!(result.is_err());
                assert!(reopened.team_grant(&f.session.id, &grant.id).is_err());
            }
        }
    }
}

fn variable_budget_fixture(total: u64, ceiling: u64, review: u64) -> Fixture {
    let f = Fixture::with_limits(Limits { parallel: 4, resources: Some(ResourceLimits {
        observed_tokens: Some(total), invocation_tokens: Some(ceiling), review_reserve_tokens: Some(review), required_review_invocations: 1,
        ..Default::default()
    }), ..Default::default() });
    // This ledger fixture has no task graph; only its captured required review.
    f.store.db().unwrap().execute("DELETE FROM tasks WHERE session_id=?", [&f.session.id]).unwrap();
    f
}
fn variable_assignment(f: &Fixture, purpose: &str, tokens: u64) -> (AssignmentRecord, InvocationRecord) {
    let (mut assignment, mut invocation) = budget_assignment(f, purpose);
    assignment.token_reservation = Some(tokens);
    assignment.requested.effort = Some("low".into());
    invocation.requested = assignment.requested.clone();
    (assignment, invocation)
}
#[test]
fn variable_reservations_race_atomically_and_cannot_override_captured_ceiling() {
    let f = variable_budget_fixture(100, 60, 20);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let jobs = [60, 60].into_iter().map(|tokens| {
        let store = Store::open(&f.store.home).unwrap();
        let (assignment, invocation) = variable_assignment(&f, "execution", tokens);
        let barrier = barrier.clone();
        std::thread::spawn(move || { barrier.wait(); store.admit_invocation(&assignment, invocation) })
    }).collect::<Vec<_>>();
    barrier.wait();
    let results = jobs.into_iter().map(|job|job.join().unwrap()).collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result|result.is_ok()).count(), 1);
    let budget = f.store.session_budget(&f.session.id).unwrap().unwrap();
    assert_eq!(budget.reserved_tokens, Some(60));
    assert_eq!(budget.protected_review_tokens, Some(20));
    for tokens in [0, 61, u64::MAX] {
        let (assignment, invocation) = variable_assignment(&f, "review", tokens);
        let denied = f.store.admit_invocation(&assignment, invocation).unwrap_err();
        assert_eq!(denied.downcast_ref::<BudgetDenial>().unwrap().code, "token_reservation_limit");
    }
    assert_eq!(f.store.trace(&f.session.id).unwrap().invocations.len(), 1);
}
#[test]
fn live_review_reservation_is_counted_once_and_completed_review_releases_unused_protection() {
    let f = variable_budget_fixture(100, 100, 20);
    let (assignment, invocation) = variable_assignment(&f, "review", 20);
    let review = f.store.admit_invocation(&assignment, invocation).unwrap();
    budget_observe(&f, &review, 7, true);
    let budget = f.store.session_budget(&f.session.id).unwrap().unwrap();
    assert_eq!(budget.observed_usage.known_total(), Some(7));
    assert_eq!(budget.reserved_tokens, Some(13));
    assert_eq!(budget.protected_review_tokens, Some(0));
    let (assignment, invocation) = variable_assignment(&f, "execution", 80);
    let execution = f.store.admit_invocation(&assignment, invocation).unwrap();
    assert_eq!(f.store.session_budget(&f.session.id).unwrap().unwrap().reserved_tokens, Some(93));
    budget_observe(&f, &execution, 0, false);
    f.store.finish_invocation(&f.session.id, &execution.id, InvocationState::Completed, None).unwrap();
    budget_observe(&f, &review, 15, false);
    f.store.finish_invocation(&f.session.id, &review.id, InvocationState::Completed, None).unwrap();
    let budget = f.store.session_budget(&f.session.id).unwrap().unwrap();
    assert_eq!(budget.observed_usage.known_total(), Some(15));
    assert_eq!(budget.protected_review_tokens, Some(0));
    let (assignment, invocation) = variable_assignment(&f, "consultation", 85);
    assert!(f.store.admit_invocation(&assignment, invocation).is_ok());
}
#[test]
fn variable_reservations_preserve_partial_usage_stop_and_reject_arithmetic_overflow() {
    let f = variable_budget_fixture(100, 60, 20);
    let (assignment, invocation) = variable_assignment(&f, "execution", 10);
    let invocation = f.store.admit_invocation(&assignment, invocation).unwrap();
    budget_observe(&f, &invocation, 7, true);
    f.store.finish_invocation(&f.session.id, &invocation.id, InvocationState::Interrupted, None).unwrap();
    let (assignment, next) = variable_assignment(&f, "review", 1);
    let denied = f.store.admit_invocation(&assignment, next).unwrap_err();
    assert_eq!(denied.downcast_ref::<BudgetDenial>().unwrap().code, "unknown_usage");
    assert_eq!(f.store.session_budget(&f.session.id).unwrap().unwrap().observed_usage.known_total(), Some(7));
    let f = variable_budget_fixture(u64::MAX, u64::MAX, 1);
    let (assignment, invocation) = variable_assignment(&f, "execution", u64::MAX);
    let denied = f.store.admit_invocation(&assignment, invocation).unwrap_err();
    assert_eq!(denied.downcast_ref::<BudgetDenial>().unwrap().code, "token_review_reserve");
    assert!(f.store.trace(&f.session.id).unwrap().invocations.is_empty());
}

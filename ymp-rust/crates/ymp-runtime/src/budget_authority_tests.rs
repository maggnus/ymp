// Consumer checks for the combined admission transaction and process capability.
struct BudgetAuthorityFixture {
    _dir: tempfile::TempDir,
    store: Store,
    session: Session,
}
impl BudgetAuthorityFixture {
    fn new() -> Self {
        Self::with_limits(Limits { parallel: 3, resources: Some(ResourceLimits {
            observed_tokens: Some(100), invocation_tokens: Some(30), required_review_invocations: 1,
            ..Default::default()
        }), ..Default::default() }, false)
    }
    fn with_limits(limits: Limits, legacy: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("state")).unwrap();
        let project = store.project(dir.path()).unwrap();
        let team: Vec<_> = (0..3).map(|n| AgentProfile {
            id: format!("agent-{n}"), name: format!("Agent {n}"), provider: "mock".into(),
            model: None, instructions: String::new(), enabled: true,
        }).collect();
        let session = Session { id: new_id(), project_id: project.id, title: "Budget and authority".into(),
            status: "running".into(), created_at: now(), team: team.clone(), turns_used: 0 };
        if legacy {
            store.save_session(&session).unwrap();
            store.capture_legacy_budget_limits(&session.id, &limits).unwrap();
        } else { store.create_session(&session, &SessionPolicy {
            team_constraints: None,
            session_id: session.id.clone(), goal: "Inspect a bounded result".into(), constraints: None,
            cwd: dir.path().into(), limits, eligible_pool: team.clone(), captured_team: team,
            execution: Default::default(), assignment_settings: vec![], parent_session_id: None,
            evaluation: None, captured_at: now(),
        }).unwrap(); }
        Self { _dir: dir, store, session }
    }
    async fn server(&self) -> Arc<TeamServer> {
        let (tx, _) = mpsc::unbounded_channel();
        Arc::new(TeamServer::start(Store::open(&self.store.home).unwrap(), &self.session, tx).await.unwrap())
    }
    fn pair(&self, agent: usize) -> (AssignmentRecord, InvocationRecord) {
        let assignment = AssignmentRecord {
            id: new_id(), session_id: self.session.id.clone(), task: None,
            agent_id: self.session.team[agent].id.clone(), agent_config_version: "fixture".into(),
            provider_id: "mock".into(), purpose: "plan".into(), reason: "Bounded joint fixture".into(),
            cwd: self._dir.path().into(), requested: ExecutionSettings::default(), timeout_secs: 10,
            grant_ids: vec![], context: vec![], state: InvocationState::Running, started_at: now(), ended_at: None,
        };
        let invocation: InvocationRecord = serde_json::from_value(json!({
            "id":new_id(),"session_id":self.session.id,"assignment_id":assignment.id,"turn":1,
            "requested":{},"sent":{},"reported":{},"state":"running","started_at":now()
        })).unwrap();
        (assignment, invocation)
    }
}

#[tokio::test]
async fn joint_budget_denial_creates_no_grant_or_live_capability_across_connections() {
    let f = BudgetAuthorityFixture::new();
    let mut servers = vec![];
    for _ in 0..3 { servers.push(f.server().await); }
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let jobs: Vec<_> = servers.iter().enumerate().map(|(n, server)| {
        let server = server.clone();
        let barrier = barrier.clone();
        let (mut assignment, mut invocation) = f.pair(n);
        std::thread::spawn(move || {
            barrier.wait();
            let result = server.admit_reserved(&mut assignment, &mut invocation, TeamOperation::coordination());
            (assignment, invocation, result)
        })
    }).collect();
    barrier.wait();
    let results: Vec<_> = jobs.into_iter().map(|j| j.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|(_, _, result)| result.is_ok()).count(), 2);
    let trace = f.store.trace(&f.session.id).unwrap();
    assert_eq!(trace.invocations.len(), 2);
    assert_eq!(trace.history.iter().filter(|e| e.data["change"] == "grant_issued").count(), 2);
    assert_eq!(trace.history.iter().filter(|e| e.kind == "budget_reserved").count(), 2);
    assert_eq!(trace.budget.unwrap().reserved_tokens, Some(60));
    let mut ordinals = HashSet::new();
    for (index, (assignment, invocation, result)) in results.into_iter().enumerate() {
        match result {
            Ok(token) => {
                assert!(ordinals.insert(invocation.turn));
                assert_eq!(assignment.grant_ids.len(), 1);
                let grant = f.store.team_grant(&f.session.id, &assignment.grant_ids[0]).unwrap();
                assert_eq!(grant.invocation_id, invocation.id);
                assert_eq!(servers[index].active.lock().unwrap().len(), 1);
                assert_eq!(call(&servers[index].socket, json!({"token":token,"request_id":"admitted","name":"team_read","arguments":{}})).await["ok"], true);
            }
            Err(error) => {
                assert_eq!(error.downcast_ref::<BudgetDenial>().unwrap().code, "token_review_reserve");
                assert!(assignment.grant_ids.is_empty());
                assert!(servers[index].active.lock().unwrap().is_empty());
                assert!(!trace.invocations.iter().any(|i| i.id == invocation.id));
            }
        }
    }
    assert_eq!(ordinals, HashSet::from([1, 2]));
}

#[tokio::test]
async fn joint_terminal_and_restart_release_reservations_and_reject_capabilities() {
    for state in [InvocationState::Completed, InvocationState::Failed, InvocationState::Cancelled, InvocationState::Interrupted] {
        let f = BudgetAuthorityFixture::new();
        let server = f.server().await;
        let (mut assignment, mut invocation) = f.pair(0);
        let token = server.admit_reserved(&mut assignment, &mut invocation, TeamOperation::coordination()).unwrap();
        f.store.observe_invocation(&f.session.id, &invocation.id, &InvocationObservation {
            usage: Some(UsageSnapshot { counts: TokenCounts { input:Some(7),output:Some(0),..Default::default() },
                finalized: false, partial:true, ..Default::default() }), ..Default::default()
        }).unwrap();
        assert_eq!(f.store.session_budget(&f.session.id).unwrap().unwrap().reserved_tokens, Some(23));
        if state == InvocationState::Interrupted { drop(server); }
        else {
            server.finish(&invocation.id, state, Some("fixture terminal")).unwrap();
            assert!(server.active.lock().unwrap().is_empty());
            assert_eq!(call(&server.socket,json!({"token":token,"request_id":"terminal","name":"team_post","arguments":{"text":"must be denied"}})).await["ok"],false);
        }
        let restarted = f.server().await;
        assert_eq!(call(&restarted.socket,json!({"token":token,"request_id":"restarted","name":"team_read","arguments":{}})).await["ok"],false);
        let budget = f.store.session_budget(&f.session.id).unwrap().unwrap();
        assert_eq!(budget.admitted_invocations,1);
        assert_eq!(budget.in_flight_invocations,0);
        assert_eq!(budget.reserved_tokens,Some(0));
        assert_eq!(budget.observed_usage.known_total(),Some(7));
        assert!(budget.observed_usage.is_partial());
        assert!(f.store.team_grant(&f.session.id,&assignment.grant_ids[0]).unwrap().revoked_at.is_some());
        let (mut next, mut invocation) = f.pair(1);
        next.purpose = "review".into();
        let error = restarted.admit_reserved(&mut next,&mut invocation,TeamOperation::coordination()).unwrap_err();
        assert_eq!(error.downcast_ref::<BudgetDenial>().unwrap().code,"unknown_usage");
        assert!(restarted.active.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn explicit_server_admission_rejects_historical_ordinals_before_issuing_capabilities() {
    for sparse in [false, true] {
        for legacy in [false, true] {
            let mut f = BudgetAuthorityFixture::with_limits(Limits { turns: 3, parallel: 3, resources: None, ..Default::default() }, legacy);
            f.session.turns_used = 2;
            f.store.save_session(&f.session).unwrap();
            if sparse {
                f.store.begin_usage(&f.session.id, 1, &f.session.team[0].id).unwrap();
                f.store.finish_usage(&f.session.id, 1, "interrupted").unwrap();
            }
            let mut servers = vec![];
            for _ in 0..3 { servers.push(f.server().await); }
            for turn in [1, 2, 4] {
                let (mut a, mut i) = f.pair(0);
                i.turn = turn;
                assert!(servers[0].admit(&mut a, &i, TeamOperation::coordination()).is_err(), "historical/skipped ordinal {turn} issued a capability");
                assert!(a.grant_ids.is_empty());
                assert!(servers[0].active.lock().unwrap().is_empty());
                assert!(f.store.trace(&f.session.id).unwrap().invocations.is_empty());
            }
            let barrier = Arc::new(std::sync::Barrier::new(4));
            let jobs: Vec<_> = servers.iter().enumerate().map(|(n,server)| {
                let server = server.clone();
                let barrier = barrier.clone();
                let (mut a, mut i) = f.pair(n);
                i.turn = 3;
                std::thread::spawn(move || { barrier.wait(); let result = server.admit(&mut a,&i,TeamOperation::coordination()); (a,i,result) })
            }).collect();
            barrier.wait();
            let results: Vec<_> = jobs.into_iter().map(|j|j.join().unwrap()).collect();
            assert_eq!(results.iter().filter(|(_,_,r)|r.is_ok()).count(), 1);
            for (n,(a,i,result)) in results.into_iter().enumerate() {
                if let Ok(token) = result {
                    assert_eq!(call(&servers[n].socket,json!({"token":token,"request_id":"fresh","name":"team_read","arguments":{}})).await["ok"],true);
                    f.store.observe_invocation(&f.session.id,&i.id,&InvocationObservation { usage:Some(UsageSnapshot {
                        counts:TokenCounts { input:Some(9),output:Some(0),..Default::default() },finalized:true,..Default::default()
                    }),..Default::default() }).unwrap();
                    servers[n].finish(&i.id,InvocationState::Completed,None).unwrap();
                    assert_eq!(call(&servers[n].socket,json!({"token":token,"request_id":"terminal","name":"team_read","arguments":{}})).await["ok"],false);
                    assert!(f.store.team_grant(&f.session.id,&a.grant_ids[0]).unwrap().revoked_at.is_some());
                } else { assert!(a.grant_ids.is_empty()); }
                assert!(servers[n].active.lock().unwrap().is_empty());
            }
            let reopened = Store::open(&f.store.home).unwrap();
            let trace = reopened.trace(&f.session.id).unwrap();
            assert_eq!(trace.invocations.len(),1);
            assert_eq!(trace.usage.total.calls,3);
            assert_eq!(trace.usage.total.known_total(),Some(9));
            assert!(trace.usage.total.is_partial());
            assert_eq!(trace.history.iter().filter(|e|e.data["change"]=="grant_issued").count(),1);
            let restarted = f.server().await;
            let (mut a,mut i) = f.pair(0);
            i.turn=4;
            assert!(restarted.admit(&mut a,&i,TeamOperation::coordination()).is_err());
            assert!(restarted.active.lock().unwrap().is_empty());
        }
    }
}

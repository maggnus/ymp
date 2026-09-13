mod backend_contract_tests {
    use super::*;
    use ymp_providers::{ExecutionFuture, TurnResult};

    #[derive(Clone, Copy)]
    enum Behavior {
        Script,
        Hang,
        StreamOverflow,
        ResultOverflow,
        SecretError,
        ReviewLimit(usize),
        WriterLimit,
        ReviewStop(&'static str),
    }

    struct ScriptedBackend {
        identity: ExecutionBackendIdentity,
        behavior: Behavior,
        requests: Mutex<Vec<TurnRequest>>,
        dropped: AtomicUsize,
        native_total: Option<TokenCounts>,
        cancel_review: Option<CancellationToken>,
        partial_review_usage: bool,
        review_output: u64,
    }

    impl ScriptedBackend {
        fn new(id: &str, version: &str, behavior: Behavior) -> Arc<Self> {
            Arc::new(Self {
                identity: ExecutionBackendIdentity {
                    id: id.into(),
                    version: version.into(),
                },
                behavior,
                requests: Mutex::new(vec![]),
                dropped: AtomicUsize::new(0),
                native_total: None,
                cancel_review: None,
                partial_review_usage: false,
                review_output: 3,
            })
        }
    }

    struct DropCount<'a>(&'a AtomicUsize);
    impl Drop for DropCount<'_> {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl ExecutionBackend for ScriptedBackend {
        fn identity(&self) -> ExecutionBackendIdentity {
            self.identity.clone()
        }

        fn execute(
            &self,
            req: TurnRequest,
            events: mpsc::UnboundedSender<ProviderEvent>,
        ) -> ExecutionFuture<'_> {
            Box::pin(async move {
                let _drop = DropCount(&self.dropped);
                self.requests.lock().unwrap().push(req.clone());
                assert!(req.mcp.as_ref().is_some_and(|m| !m.token.is_empty()));
                let hanging = matches!(self.behavior, Behavior::Hang | Behavior::StreamOverflow);
                let _ = events.send(ProviderEvent::Usage(UsageSnapshot {
                    counts: TokenCounts {
                        input: Some(7),
                        output: (!hanging).then_some(if req.purpose == "review" { self.review_output } else { 3 }),
                        ..Default::default()
                    },
                    finalized: !hanging,
                    partial: hanging || (req.purpose == "review" && self.partial_review_usage),
                    native_total: self.native_total.clone(),
                    ..Default::default()
                }));
                if req.purpose == "review" {
                    if let Some(cancel) = &self.cancel_review {
                        cancel.cancel();
                        return Err(ymp_providers::NativeOutputLimit.into());
                    }
                }
                match self.behavior {
                    Behavior::ReviewStop(message) if req.purpose == "review" => {
                        bail!("{message}");
                    }
                    Behavior::ReviewLimit(limit) if req.purpose == "review" => {
                        let count = self.requests.lock().unwrap().iter().filter(|r| r.purpose == "review").count();
                        if count <= limit {
                            return Err(ymp_providers::NativeOutputLimit.into());
                        }
                    }
                    Behavior::WriterLimit if req.purpose == "execute" => {
                        return Err(ymp_providers::NativeOutputLimit.into());
                    }
                    Behavior::Hang => std::future::pending::<()>().await,
                    Behavior::StreamOverflow => {
                        let _ = events.send(ProviderEvent::Delta("x".repeat(70_000)));
                        // Usage queued after the offending delta must survive terminal drain.
                        let _ = events.send(ProviderEvent::Usage(UsageSnapshot {
                            counts: TokenCounts {
                                input: Some(9),
                                ..Default::default()
                            },
                            partial: true,
                            ..Default::default()
                        }));
                        std::future::pending::<()>().await;
                    }
                    Behavior::SecretError => {
                        let token = &req.mcp.as_ref().unwrap().token;
                        return Err(anyhow::anyhow!("transport rejected {token}")
                            .context(format!("script_error: YMP_MCP_TOKEN={token}")));
                    }
                    Behavior::Script | Behavior::ResultOverflow | Behavior::ReviewLimit(_) | Behavior::WriterLimit | Behavior::ReviewStop(_) => {}
                }
                let text = if matches!(self.behavior, Behavior::ResultOverflow) {
                    "x".repeat(70_000)
                } else {
                    match req.purpose.as_str() {
                        "plan" => json!({"summary":"Calculate the series", "tasks":[{
                            "title":"Calculate exact sum", "description":"Write series.txt with 55", "competence":"implementation",
                            "difficulty":"simple", "dependencies":[], "checks":["test \"$(cat series.txt)\" = 55"]
                        }]}).to_string(),
                        "review_plan" | "review" | "final_review" => json!({"approved":true,"reason":"The series sums to 55."}).to_string(),
                        "bid" => json!({"willing":true,"approach":"Sum the integers from one through ten."}).to_string(),
                        "execute" => {
                            assert!(!req.read_only);
                            tokio::fs::write(req.cwd.join("series.txt"), "55\n").await?;
                            "Wrote the exact sum to series.txt.".into()
                        }
                        "conversation" => json!({"action":"answer","answer":"The series sum is 55."}).to_string(),
                        "synthesis" => "The exact sum is 55, recorded in series.txt.".into(),
                        purpose => bail!("Unexpected scripted assignment: {purpose}"),
                    }
                };
                Ok(TurnResult {
                    text,
                    session_id: req.resume.unwrap_or_else(|| format!("script-{}", new_id())),
                    usage: None,
                })
            })
        }
    }

    #[tokio::test]
    async fn native_review_limit_recovers_without_replaying_the_writer() {
        for (failures, attempts, turns, expected_reviews, completed) in [
            (1, 2, 80, 2, true),
            (9, 8, 80, 2, false),
            (1, 1, 80, 1, false),
        ] {
            let mut fixture = RunFixture::new("", false);
            fixture.engine.config.limits.attempts = attempts;
            fixture.engine.config.limits.turns = turns;
            fixture.engine.set_assignment_settings(fixture.engine.config.agents.iter()
                .map(|agent| settings_rule(agent, "review", Some("fixture-model"), Some("fixture-low")))
                .collect()).unwrap();
            let script = ScriptedBackend::new("example.review-limit", "1", Behavior::ReviewLimit(failures));
            fixture.engine = fixture.engine.with_execution_backend(script.clone()).unwrap();
            let outcome = fixture.run().await;
            assert_eq!(outcome.session.status == "completed", completed, "{}", outcome.summary);
            let requests = script.requests.lock().unwrap();
            assert_eq!(requests.iter().filter(|r| r.purpose == "execute").count(), 1, "turns={turns}: {}; purposes={:?}", outcome.summary, requests.iter().map(|r| &r.purpose).collect::<Vec<_>>());
            let reviews = requests.iter().filter(|r| r.purpose == "review").collect::<Vec<_>>();
            assert_eq!(reviews.len(), expected_reviews);
            for review in &reviews {
                assert!(review.read_only);
                assert_eq!(review.profile.id, reviews[0].profile.id);
                assert_eq!(review.settings, reviews[0].settings);
                assert_eq!(review.settings.effort.as_deref(), Some("fixture-low"));
            }
            if reviews.len() == 2 {
                assert!(reviews[1].resume.is_none());
                assert!(reviews[0].mcp.as_ref().unwrap().token != reviews[1].mcp.as_ref().unwrap().token, "Each attempt needs a fresh capability");
            }
            assert_eq!(std::fs::read_to_string(fixture.project.join("series.txt")).unwrap(), "55\n");
            let tasks = fixture.store.tasks(&outcome.session.id).unwrap();
            assert_eq!(tasks[0].attempts, 1);
            assert_eq!(tasks[0].state, if completed { TaskState::Accepted } else { TaskState::Review });
            let trace = fixture.store.trace(&outcome.session.id).unwrap();
            assert_eq!(trace.usage.total.known_total(), Some(10 * requests.len() as u64));
            let failed = trace.invocations.iter().filter(|i| i.terminal_reason.as_deref() == Some("native_output_limit")).collect::<Vec<_>>();
            assert_eq!(failed.len(), failures.min(expected_reviews));
            assert!(failed.iter().all(|i| i.state == InvocationState::Failed));
            assert_closed(&fixture, &outcome.session.id);
        }
    }

    #[tokio::test]
    async fn native_output_limit_never_automatically_retries_a_writer() {
        let mut fixture = RunFixture::new("", false);
        let script = ScriptedBackend::new("example.writer-limit", "1", Behavior::WriterLimit);
        fixture.engine = fixture.engine.with_execution_backend(script.clone()).unwrap();
        let outcome = fixture.run().await;
        assert_ne!(outcome.session.status, "completed");
        assert_eq!(script.requests.lock().unwrap().iter().filter(|r| r.purpose == "execute").count(), 1);
        assert_closed(&fixture, &outcome.session.id);
    }

    #[tokio::test]
    async fn native_review_limit_does_not_retry_unrelated_or_runtime_stops() {
        for message in ["Provider turn cancelled", "Provider turn timed out",
            "output_limit: fixture output exceeded", "ACP turn stopped: max_turn_requests",
            "ACP turn stopped: refusal", "untyped max_tokens diagnostic"] {
            let mut fixture = RunFixture::new("", false);
            let script = ScriptedBackend::new("example.review-stop", "1", Behavior::ReviewStop(message));
            fixture.engine = fixture.engine.with_execution_backend(script.clone()).unwrap();
            let outcome = fixture.run().await;
            assert_ne!(outcome.session.status, "completed");
            assert_eq!(script.requests.lock().unwrap().iter().filter(|r| r.purpose == "review").count(), 1);
            assert_eq!(fixture.store.tasks(&outcome.session.id).unwrap()[0].state, TaskState::Review);
            assert_closed(&fixture, &outcome.session.id);
        }
    }

    #[tokio::test]
    async fn native_review_limit_respects_cancellation_partial_usage_and_token_ceiling() {
        for mode in ["cancel", "partial", "tokens"] {
            let mut fixture = RunFixture::new("", false);
            let mut script = ScriptedBackend::new("example.review-budget", "1", Behavior::ReviewLimit(1));
            let backend = Arc::get_mut(&mut script).unwrap();
            if mode == "cancel" {
                backend.cancel_review = Some(fixture.engine.cancel.clone());
            } else {
                let resources = fixture.engine.config.limits.resources.as_mut().unwrap();
                resources.observed_tokens = Some(if mode == "tokens" { 50 } else { 100 });
                resources.invocation_tokens = Some(10);
                resources.required_review_invocations = 1;
                backend.partial_review_usage = mode == "partial";
                if mode == "tokens" { backend.review_output = 13; }
            }
            fixture.engine = fixture.engine.with_execution_backend(script.clone()).unwrap();
            let outcome = fixture.run().await;
            assert_eq!(outcome.session.status, "paused", "{mode}: {}", outcome.summary);
            assert_eq!(script.requests.lock().unwrap().iter().filter(|r| r.purpose == "review").count(), 1, "{mode}: {}", outcome.summary);
            let trace = fixture.store.trace(&outcome.session.id).unwrap();
            assert_eq!(trace.usage.total.known_total(), Some(if mode == "tokens" { 50 } else { 40 }));
            assert_eq!(fixture.store.tasks(&outcome.session.id).unwrap()[0].state, TaskState::Review);
            if mode != "cancel" {
                assert_eq!(trace.budget.as_ref().unwrap().last_denial.as_ref().unwrap().code,
                    if mode == "partial" { "unknown_usage" } else { "token_limit" });
            }
            assert_closed(&fixture, &outcome.session.id);
        }
    }

    #[tokio::test]
    async fn native_review_retry_cannot_exceed_remaining_invocations() {
        let mut fixture = RunFixture::new("[mock:usage]", false);
        fixture.engine.config.limits.turns = 7;
        let session = fixture.run().await.session;
        assert_eq!(session.turns_used, 6);
        let prior = fixture.store.trace(&session.id).unwrap().usage.total.known_total().unwrap();
        let script = ScriptedBackend::new("example.last-invocation", "1", Behavior::ReviewLimit(1));
        fixture.engine = fixture.engine.with_execution_backend(script.clone()).unwrap();
        let ctx = settings_context(&fixture, session).await;
        let error = fixture.engine.ask_scoped(&ctx, &ctx.session.team[1], &fixture.project,
            "review", "Inspect the retained result", true, None).await.err().unwrap();
        assert_eq!(error.downcast_ref::<BudgetDenial>().unwrap().code, "invocation_limit");
        assert_eq!(script.requests.lock().unwrap().len(), 1);
        let trace = fixture.store.trace(&ctx.session.id).unwrap();
        assert_eq!(trace.invocations.len(), 7);
        assert_eq!(trace.usage.total.known_total(), Some(prior + 10));
        assert_eq!(trace.invocations.last().unwrap().terminal_reason.as_deref(), Some("native_output_limit"));
        assert_closed(&fixture, &ctx.session.id);
    }

    fn assert_backend_admitted(
        fixture: &RunFixture,
        session: &str,
        script: &ScriptedBackend,
        error: &anyhow::Error,
    ) {
        if !script.requests.lock().unwrap().is_empty() {
            return;
        }
        // Admission can fail before the backend receives a capability. Describe
        // that boundary without formatting errors or requests that might contain one.
        let io_code = error
            .downcast_ref::<std::io::Error>()
            .and_then(|error| error.raw_os_error());
        let trace = fixture.store.trace(session).unwrap();
        let events = trace
            .history
            .iter()
            .rev()
            .take(8)
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>();
        panic!("Scripted backend was not admitted: io_code={io_code:?}, session_status={}, last_events={events:?}", trace.session.status);
    }

    fn assert_closed(fixture: &RunFixture, session: &str) {
        let trace = fixture.store.trace(session).unwrap();
        assert_eq!(trace.budget.as_ref().unwrap().in_flight_invocations, 0);
        for assignment in &trace.assignments {
            assert_ne!(assignment.state, InvocationState::Running);
            for id in &assignment.grant_ids {
                let grant = fixture.store.team_grant(session, id).unwrap();
                assert!(grant.revoked_at.is_some());
            }
        }
    }

    #[tokio::test]
    async fn backend_public_engine_executes_a_distinct_workflow_and_records_actual_identity() {
        let mut fixture = RunFixture::new("", false);
        let script = ScriptedBackend::new("example.series", "1", Behavior::Script);
        fixture.engine = fixture
            .engine
            .with_execution_backend(script.clone())
            .unwrap();
        let result = fixture
            .engine
            .run(&fixture.project, "Sum one through ten", None)
            .await
            .unwrap();
        assert_eq!(result.session.status, "completed");
        assert_eq!(
            std::fs::read_to_string(fixture.project.join("series.txt")).unwrap(),
            "55\n"
        );
        assert!(!fixture.project.join("greeting.txt").exists());
        assert!(result.summary.contains("55"));
        let trace = fixture.store.trace(&result.session.id).unwrap();
        for message in fixture.store.messages(&result.session.id, 0, 1000).unwrap()
            .iter().filter(|m| m.author == "one" || m.author == "two") {
            let origin = trace.message_attribution(message).expect("Native response lost its invocation origin");
            assert_eq!(origin.agent_id, message.author);
            assert!(trace.assignments.iter().any(|a| a.id == origin.assignment_id && a.purpose == message.kind));
        }
        assert_eq!(
            trace.invocations.len(),
            script.requests.lock().unwrap().len()
        );
        for invocation in &trace.invocations {
            assert_eq!(
                invocation.execution_backend.as_ref(),
                Some(&script.identity)
            );
            assert_eq!(invocation.sent, ExecutionSettings::default());
            assert_eq!(invocation.reported, ExecutionSettings::default());
            assert_eq!(invocation.native_version, None);
            assert!(invocation
                .native_session_id
                .as_ref()
                .unwrap()
                .starts_with("script-"));
        }
        assert_eq!(
            trace.usage.total.known_total(),
            Some(10 * trace.invocations.len() as u64)
        );
        assert_eq!(trace.usage.agents.len(), 2);
        assert!(trace
            .usage
            .agents
            .keys()
            .all(|id| id == "one" || id == "two"));
        assert_closed(&fixture, &result.session.id);
        let mut legacy = serde_json::to_value(&trace.invocations[0]).unwrap();
        legacy.as_object_mut().unwrap().remove("execution_backend");
        let legacy: InvocationRecord = serde_json::from_value(legacy).unwrap();
        assert_eq!(legacy.execution_backend, None);
        let reopened = Store::open_read_only(&fixture.store.home).unwrap();
        assert_eq!(
            serde_json::to_value(reopened.trace(&result.session.id).unwrap()).unwrap(),
            serde_json::to_value(trace).unwrap()
        );
    }

    #[tokio::test]
    async fn visible_follow_up_answer_keeps_its_origin_after_decoding() {
        let mut fixture = RunFixture::new("", false);
        let script = ScriptedBackend::new("example.answer-origin", "1", Behavior::Script);
        fixture.engine = fixture.engine.with_execution_backend(script).unwrap();
        let session = fixture.run().await.session;
        fixture.engine.follow_up(&fixture.project, "What is the sum?", &session.id).await.unwrap();
        let trace = fixture.store.trace(&session.id).unwrap();
        let messages = fixture.store.messages(&session.id, 0, 1000).unwrap();
        let answer = messages.iter().rev().find(|m| m.kind == "answer").unwrap();
        let raw = messages.iter().rev().find(|m| m.kind == "conversation").unwrap();
        let answer_origin = trace.message_attribution(answer).expect("Decoded answer lost its invocation origin");
        assert_eq!(answer_origin.invocation_id, trace.message_attribution(raw).unwrap().invocation_id);
        assert_eq!(answer_origin.agent_id, answer.author);
        assert_eq!(answer.text, "The series sum is 55.");
    }

    #[tokio::test]
    async fn backend_public_engine_stops_stream_overflow_and_drains_usage() {
        assert_output_stop(Behavior::StreamOverflow).await;
    }

    #[tokio::test]
    async fn backend_public_engine_stops_result_overflow_and_drains_usage() {
        assert_output_stop(Behavior::ResultOverflow).await;
    }

    async fn assert_output_stop(behavior: Behavior) {
        let mut fixture = RunFixture::new("", false);
        let session = fixture.run().await.session;
        let script = ScriptedBackend::new("example.overflow", "1", behavior);
        fixture.engine = fixture
            .engine
            .with_execution_backend(script.clone())
            .unwrap();
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            fixture
                .engine
                .follow_up(&fixture.project, "What is the sum?", &session.id),
        )
        .await
        .expect("common output guard did not stop execution")
        .unwrap_err();
        assert!(error.to_string().contains("output_limit"), "{error:#}");
        let trace = fixture.store.trace(&session.id).unwrap();
        let invocation = trace.invocations.last().unwrap();
        assert_eq!(
            invocation.execution_backend.as_ref(),
            Some(&script.identity)
        );
        assert_eq!(invocation.terminal_reason.as_deref(), Some("output_limit"));
        assert_eq!(invocation.state, InvocationState::Failed);
        let usage = invocation.usage.as_ref().unwrap();
        if matches!(behavior, Behavior::StreamOverflow) {
            assert_eq!(usage.counts.input, Some(9));
            assert!(usage.partial);
        } else {
            assert_eq!(usage.counts.known_total(), Some(10));
            assert!(usage.finalized);
        }
        assert_eq!(script.dropped.load(Ordering::SeqCst), 1);
        assert_closed(&fixture, &session.id);
    }

    #[tokio::test]
    async fn backend_public_engine_cancels_a_noncooperative_future() {
        assert_hanging_stop(true).await;
    }

    #[tokio::test]
    async fn backend_public_engine_times_out_a_noncooperative_future() {
        assert_hanging_stop(false).await;
    }

    async fn assert_hanging_stop(cancellation: bool) {
        let mut fixture = RunFixture::new("", false);
        fixture.engine.config.limits.turn_timeout_secs = 1;
        let session = fixture.run().await.session;
        let script = ScriptedBackend::new("example.hang", "1", Behavior::Hang);
        fixture.engine = fixture
            .engine
            .with_execution_backend(script.clone())
            .unwrap();
        let cancel = fixture.engine.cancel.clone();
        let stop = async {
            if cancellation {
                while script.requests.lock().unwrap().is_empty() {
                    tokio::task::yield_now().await;
                }
                cancel.cancel();
            }
        };
        let call = fixture
            .engine
            .follow_up(&fixture.project, "What is the sum?", &session.id);
        let result = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            tokio::pin!(call);
            tokio::select! {
                result = &mut call => result,
                () = stop => call.await,
            }
        })
        .await
        .expect("common guard did not stop execution");
        let error = result.unwrap_err();
        assert_backend_admitted(&fixture, &session.id, &script, &error);
        assert!(
            error.to_string().contains(if cancellation {
                "cancelled"
            } else {
                "timeout_limit"
            }),
            "{error:#}"
        );
        let trace = fixture.store.trace(&session.id).unwrap();
        let invocation = trace.invocations.last().unwrap();
        assert_eq!(
            invocation.state,
            if cancellation {
                InvocationState::Cancelled
            } else {
                InvocationState::Failed
            }
        );
        assert_eq!(invocation.usage.as_ref().unwrap().counts.input, Some(7));
        assert!(invocation.usage.as_ref().unwrap().partial);
        assert_eq!(script.dropped.load(Ordering::SeqCst), 1);
        assert_closed(&fixture, &session.id);
    }

    #[tokio::test]
    async fn backend_public_engine_redacts_nested_capability_errors() {
        let mut fixture = RunFixture::new("", false);
        let session = fixture.run().await.session;
        let script = ScriptedBackend::new("example.error", "1", Behavior::SecretError);
        fixture.engine = fixture
            .engine
            .with_execution_backend(script.clone())
            .unwrap();
        let error = fixture
            .engine
            .follow_up(&fixture.project, "What is the sum?", &session.id)
            .await
            .unwrap_err();
        assert_backend_admitted(&fixture, &session.id, &script, &error);
        let token = script.requests.lock().unwrap()[0]
            .mcp
            .as_ref()
            .unwrap()
            .token
            .clone();
        for rendered in [
            format!("{error}"),
            format!("{error:#}"),
            format!("{error:?}"),
            format!("{error:#?}"),
        ] {
            assert!(!rendered.contains(&token));
            assert!(!rendered.contains("YMP_MCP_TOKEN="));
            assert!(rendered.contains("script_error"));
            assert!(rendered.contains("redacted team capability"));
        }
        assert!(error
            .chain()
            .all(|cause| !cause.to_string().contains(&token)));
        let trace = fixture.store.trace(&session.id).unwrap();
        assert!(!serde_json::to_string(&trace).unwrap().contains(&token));
        while let Ok(event) = fixture.events.try_recv() {
            assert!(!format!("{event:?}").contains(&token));
        }
        assert_closed(&fixture, &session.id);
    }

    #[tokio::test]
    async fn backend_public_engine_admission_precedes_script_execution() {
        let mut fixture = RunFixture::new("", false);
        fixture
            .engine
            .config
            .limits
            .resources
            .as_mut()
            .unwrap()
            .startup_context_chars = 100;
        let script = ScriptedBackend::new("example.series", "1", Behavior::Script);
        fixture.engine = fixture
            .engine
            .with_execution_backend(script.clone())
            .unwrap();
        let result = fixture
            .engine
            .run(&fixture.project, "Sum one through ten", None)
            .await
            .unwrap();
        assert_eq!(result.session.status, "paused");
        assert!(result.summary.contains("context_limit"));
        assert!(script.requests.lock().unwrap().is_empty());
        assert!(fixture
            .store
            .trace(&result.session.id)
            .unwrap()
            .invocations
            .is_empty());
    }

    #[tokio::test]
    async fn backend_identity_scopes_continuations_and_competence_in_both_directions() {
        let fixture = RunFixture::new("", false);
        let session = fixture.run().await.session;
        let ctx = settings_context(&fixture, session).await;
        let agent = &ctx.session.team[0];
        let original_engine = fixture.engine.clone();
        let mut previous: Option<InvocationRecord> = None;
        let mut versions = vec![];
        for (id, version, model, reuse) in [
            ("example.a", "1", None, false),
            ("example.a", "1", None, true),
            ("example.b", "1", None, false),
            ("example.b", "2", None, false),
            ("example.b", "2", Some("changed"), false),
            ("example.b", "2", Some("changed"), true),
        ] {
            let mut script = ScriptedBackend::new(id, version, Behavior::Script);
            if id == "example.a" {
                Arc::get_mut(&mut script).unwrap().native_total = Some(TokenCounts {
                    input: Some(70),
                    output: Some(30),
                    ..Default::default()
                });
            }
            let engine = fixture
                .engine
                .clone()
                .with_execution_backend(script.clone())
                .unwrap();
            engine
                .set_assignment_settings(vec![settings_rule(agent, "conversation", model, None)])
                .unwrap();
            let before = engine
                .selection_version(&ctx, agent, "conversation", None)
                .unwrap();
            if previous.is_some() {
                let old = versions.last().unwrap();
                if reuse {
                    assert_eq!(&before, old);
                } else {
                    assert_ne!(&before, old);
                    assert_ne!(
                        engine
                            .backend_config_version(
                                agent,
                                engine.config.provider(&agent.provider).unwrap(),
                                &engine
                                    .requested_settings(&ctx, agent, "conversation", None, true)
                                    .unwrap()
                            )
                            .unwrap(),
                        fixture
                            .store
                            .trace(&ctx.session.id)
                            .unwrap()
                            .assignments
                            .last()
                            .unwrap()
                            .agent_config_version,
                        "backend identity/version did not change configuration"
                    );
                }
            }
            let response = engine
                .ask_scoped(
                    &ctx,
                    agent,
                    &fixture.project,
                    "conversation",
                    "What is the sum?",
                    true,
                    None,
                )
                .await
                .unwrap();
            let invocation = fixture
                .store
                .invocation(&ctx.session.id, &response.invocation_id)
                .unwrap();
            assert_eq!(
                script.requests.lock().unwrap()[0].usage_baseline,
                if reuse {
                    previous
                        .as_ref()
                        .unwrap()
                        .usage
                        .as_ref()
                        .and_then(|u| u.native_total.clone())
                } else {
                    None
                },
                "usage baseline belongs to a different continuation"
            );
            assert_eq!(
                invocation.resumed_from,
                if reuse {
                    previous.as_ref().unwrap().native_session_id.clone()
                } else {
                    None
                }
            );
            let observed = engine
                .observed_version(&ctx, agent, "conversation", None)
                .unwrap()
                .unwrap();
            assert_eq!(
                engine
                    .selection_version(&ctx, agent, "conversation", None)
                    .unwrap(),
                observed
            );
            if reuse {
                assert_eq!(versions.last().unwrap(), &observed);
            } else {
                assert!(!versions.contains(&observed));
            }
            versions.push(observed);
            previous = Some(invocation);
        }
        // A clone made before injection still executes the original native backend.
        original_engine.set_assignment_settings(vec![]).unwrap();
        let response = original_engine
            .ask_scoped(
                &ctx,
                agent,
                &fixture.project,
                "review",
                "Inspect",
                true,
                None,
            )
            .await
            .unwrap();
        let native = fixture
            .store
            .invocation(&ctx.session.id, &response.invocation_id)
            .unwrap();
        assert_eq!(
            native.execution_backend,
            Some(NativeExecutionBackend.identity())
        );
        assert_eq!(native.resumed_from, None);
        assert_eq!(native.sent.permission_mode.as_deref(), Some("read_only"));
    }
}

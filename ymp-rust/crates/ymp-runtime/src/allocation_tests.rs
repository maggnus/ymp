mod allocation_contract_tests {
    use super::*;
    use crate::{AllocationPolicy, BoundedAllocationPolicy, ResourceAllocationPolicy};
    use ymp_providers::{ExecutionFuture, TurnResult};

    fn add_agent(f: &mut RunFixture, id: &str) {
        let mut profile = f.engine.config.agents[0].clone();
        profile.id = id.into();
        profile.name = id.into();
        f.engine.config.agents.push(profile);
        f.engine.config.team.push(id.into());
    }
    fn demand(purpose: &str, ready: usize) -> AllocationDemand {
        AllocationDemand {
            purpose: purpose.into(),
            task_id: None,
            competence: "implementation".into(),
            difficulty: "standard".into(),
            risk: TaskRisk::Standard,
            ready_work: ready,
        }
    }

    struct MembershipPolicy {
        ids: Vec<String>,
    }
    impl AllocationPolicy for MembershipPolicy {
        fn identity(&self) -> ExecutionBackendIdentity {
            ExecutionBackendIdentity {
                id: "fixture.membership".into(),
                version: "2".into(),
            }
        }
        fn propose(&self, input: &AllocationInput) -> Result<AllocationProposal> {
            let mut proposal = BoundedAllocationPolicy.propose(input)?;
            proposal.members = self.ids.clone();
            proposal.rationale =
                "Idle boundary changes membership without invoking participants".into();
            Ok(proposal)
        }
    }

    #[tokio::test]
    async fn allocation_fixed_size_replacement_preserves_history_and_rejects_retired_admission() {
        let mut f = RunFixture::new("[mock:usage]", false);
        add_agent(&mut f, "three");
        f.engine.config.team_constraints.fixed_size = Some(2);
        let completed = f.run().await;
        let session = &completed.session.id;
        let initial = f.store.session_policy(session).unwrap().unwrap();
        let before = f.store.session_usage(session).unwrap();
        let mut ids = f
            .store
            .team_state(session)
            .unwrap()
            .unwrap()
            .current_members;
        let incoming = ["one", "two", "three"]
            .into_iter()
            .find(|id| !ids.iter().any(|i| i == id))
            .unwrap()
            .to_owned();
        let retired = ids.pop().unwrap();
        ids.push(incoming.clone());
        let engine = f
            .engine
            .clone()
            .with_allocation_policy(Arc::new(MembershipPolicy { ids: ids.clone() }))
            .unwrap();
        engine
            .reconsider_allocation(
                session,
                AllocationBoundary::ParticipantUnavailable,
                demand("execute", 0),
            )
            .unwrap();
        let current = f.store.team_state(session).unwrap().unwrap();
        assert_eq!(current.current_members, ids);
        assert_eq!(f.store.session(session).unwrap().team.len(), 3);
        assert_eq!(
            serde_json::to_value(f.store.session_policy(session).unwrap().unwrap()).unwrap(),
            serde_json::to_value(initial).unwrap()
        );
        assert_eq!(
            serde_json::to_value(before).unwrap(),
            serde_json::to_value(f.store.session_usage(session).unwrap()).unwrap()
        );
        let trace = f.store.trace(session).unwrap();
        let view =
            SessionAgentView::from_captured(&trace.session, Some(&current.current_members), None)
                .unwrap();
        assert!(view.captured_name(&incoming).is_some());
        let ctx = settings_context(&f, f.store.session(session).unwrap()).await;
        // Even an explicit-ordinal public admission wrapper must reject a retired identity.
        let mut assignment = trace.assignments[0].clone();
        assignment.agent_id = retired.clone();
        assignment.id = new_id();
        assignment.task = None;
        assignment.purpose = "conversation".into();
        assignment.state = InvocationState::Running;
        assignment.ended_at = None;
        assignment.grant_ids.clear();
        let mut invocation = trace.invocations[0].clone();
        invocation.id = new_id();
        invocation.assignment_id = assignment.id.clone();
        invocation.turn = trace.usage.total.calls + 1;
        invocation.requested = assignment.requested.clone();
        invocation.state = InvocationState::Running;
        invocation.ended_at = None;
        invocation.usage = None;
        invocation.terminal_reason = None;
        let rejected =
            ctx.server
                .admit(&mut assignment, &invocation, TeamOperation::coordination());
        assert!(
            rejected.is_err(),
            "An ineligible participant received admission"
        );
        assert!(rejected
            .err()
            .unwrap()
            .to_string()
            .contains("ineligible_member"));
        assert!(assignment.grant_ids.is_empty());
        let bad = engine
            .with_allocation_policy(Arc::new(MembershipPolicy {
                ids: vec!["one".into(), "two".into(), "three".into()],
            }))
            .unwrap();
        assert!(bad
            .reconsider_allocation(session, AllocationBoundary::WorkReady, demand("execute", 0))
            .unwrap_err()
            .to_string()
            .contains("fixed_size"));
    }

    #[tokio::test]
    async fn allocation_dynamic_ceiling_and_fixed_roster_are_distinct() {
        for fixed in [false, true] {
            let mut f = RunFixture::new("", false);
            add_agent(&mut f, "three");
            f.engine.config.team_constraints.max_members = 2;
            if fixed {
                f.engine.config.team_constraints.fixed_roster =
                    Some(vec!["one".into(), "two".into()]);
            }
            let outcome = f.run().await;
            let session = &outcome.session.id;
            let ids = if fixed {
                vec!["one".into(), "three".into()]
            } else {
                vec!["one".into(), "two".into(), "three".into()]
            };
            let engine = f
                .engine
                .clone()
                .with_allocation_policy(Arc::new(MembershipPolicy { ids }))
                .unwrap();
            let error = engine
                .reconsider_allocation(
                    session,
                    AllocationBoundary::ResourcesChanged,
                    demand("execute", 0),
                )
                .unwrap_err();
            assert!(
                error.to_string().contains(if fixed {
                    "fixed_roster"
                } else {
                    "membership_ceiling"
                }),
                "{error:#}"
            );
            assert!(
                !f.store
                    .allocation_decisions(session)
                    .unwrap()
                    .last()
                    .unwrap()
                    .accepted
            );
            if !fixed {
                let engine = engine
                    .with_allocation_policy(Arc::new(MembershipPolicy {
                        ids: vec!["one".into()],
                    }))
                    .unwrap();
                engine
                    .reconsider_allocation(
                        session,
                        AllocationBoundary::ResultAvailable,
                        demand("execute", 0),
                    )
                    .unwrap();
                assert_eq!(
                    f.store
                        .team_state(session)
                        .unwrap()
                        .unwrap()
                        .current_members,
                    vec!["one"]
                );
                assert!(f.store.session(session).unwrap().team.len() >= 2);
            }
        }
    }

    struct TwoTasks {
        requests: Mutex<Vec<TurnRequest>>,
    }
    impl ExecutionBackend for TwoTasks {
        fn identity(&self) -> ExecutionBackendIdentity {
            ExecutionBackendIdentity {
                id: "fixture.two-tasks".into(),
                version: "1".into(),
            }
        }
        fn execute(
            &self,
            req: TurnRequest,
            _: mpsc::UnboundedSender<ProviderEvent>,
        ) -> ExecutionFuture<'_> {
            Box::pin(async move {
                self.requests.lock().unwrap().push(req.clone());
                let text = match req.purpose.as_str() {
                    "plan" => json!({"summary":"Two useful results","tasks":[
                        {"title":"First result","description":"Explain the first fact","competence":"implementation","difficulty":"simple","dependencies":[],"checks":[]},
                        {"title":"Second result","description":"Explain the second fact","competence":"implementation","difficulty":"complex","dependencies":[],"checks":[]}
                    ]}).to_string(),
                    "review_plan" | "review" | "final_review" => json!({"approved":true,"reason":"The result satisfies its qualitative criterion"}).to_string(),
                    "execute" => "A qualitative result with a clear explanation".into(),
                    "synthesis" => "Both independently reviewed results are available".into(),
                    "conversation" => json!({"action":"steer","answer":"The clarification is recorded for the same task"}).to_string(),
                    other => bail!("Unexpected assignment: {other}"),
                };
                Ok(TurnResult {
                    text,
                    session_id: new_id(),
                    usage: None,
                })
            })
        }
    }

    #[tokio::test]
    async fn allocation_two_task_fixed_roster_keeps_final_review_feasible() {
        let mut f = RunFixture::new("", false);
        f.engine.config.team_constraints.fixed_roster = Some(vec!["one".into(), "two".into()]);
        f.engine.config.limits.parallel = 1;
        let backend = Arc::new(TwoTasks {
            requests: Mutex::new(vec![]),
        });
        f.engine = f.engine.with_execution_backend(backend.clone()).unwrap();
        let outcome = f.run().await;
        assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
        let trace = f.store.trace(&outcome.session.id).unwrap();
        let producers: HashSet<_> = trace
            .assignments
            .iter()
            .filter(|a| a.purpose == "execute")
            .map(|a| &a.agent_id)
            .collect();
        assert_eq!(trace.tasks.len(), 2);
        assert_eq!(producers.len(), 1);
        let reviewer = trace
            .assignments
            .iter()
            .find(|a| a.purpose == "final_review")
            .unwrap();
        assert!(!producers.contains(&reviewer.agent_id));
        assert!(trace.assignments.iter().all(|a| a.purpose != "bid"));
        assert_eq!(trace.team_state.unwrap().current_members.len(), 2);
        assert_eq!(trace.policy.unwrap().limits.parallel, 1);
        let original = outcome.session.id;
        let follow = f
            .engine
            .follow_up(
                &f.project,
                "Use clearer wording for the first fact",
                &original,
            )
            .await
            .unwrap();
        assert_eq!(follow.session.id, original);
        assert_eq!(f.store.sessions(None).unwrap().len(), 1);
        assert_eq!(f.store.tasks(&original).unwrap().len(), 2);
    }

    #[tokio::test]
    async fn allocation_contradictions_and_unavailable_pins_never_invoke() {
        for constraints in [
            TeamConstraints {
                fixed_size: Some(3),
                fixed_roster: Some(vec!["one".into(), "two".into()]),
                ..Default::default()
            },
            TeamConstraints {
                fixed_roster: Some(vec!["one".into(), "absent".into()]),
                ..Default::default()
            },
            TeamConstraints {
                fixed_size: Some(3),
                ..Default::default()
            },
            TeamConstraints {
                fixed_roster: Some(vec!["one".into()]),
                ..Default::default()
            },
        ] {
            let mut f = RunFixture::new("", false);
            f.engine.config.team_constraints = constraints;
            let backend = Arc::new(TwoTasks {
                requests: Mutex::new(vec![]),
            });
            f.engine = f.engine.with_execution_backend(backend.clone()).unwrap();
            let result = f.engine.run(&f.project, "Create a result", None).await;
            assert!(
                result.is_err(),
                "Contradictory constraints must fail before startup"
            );
            assert!(backend.requests.lock().unwrap().is_empty());
        }
    }

    struct Overspend;
    impl ResourceAllocationPolicy for Overspend {
        fn identity(&self) -> ExecutionBackendIdentity {
            ExecutionBackendIdentity {
                id: "fixture.overspend".into(),
                version: "1".into(),
            }
        }
        fn propose(&self, input: &ResourceAllocationInput) -> Result<InvocationAllowance> {
            Ok(InvocationAllowance {
                timeout_secs: input.budget.limits.turn_timeout_secs + 1,
                native_max_turns: 1,
                max_output_chars: 10,
                rationale: "Attempt to override a captured ceiling".into(),
            })
        }
    }
    #[tokio::test]
    async fn allocation_resource_substitution_cannot_override_runtime_ceilings() {
        let mut f = RunFixture::new("", false);
        let backend = Arc::new(TwoTasks {
            requests: Mutex::new(vec![]),
        });
        f.engine = f
            .engine
            .with_execution_backend(backend.clone())
            .unwrap()
            .with_resource_policy(Arc::new(Overspend))
            .unwrap();
        let outcome = f.run().await;
        assert_ne!(outcome.session.status, "completed");
        assert!(backend.requests.lock().unwrap().is_empty());
        let trace = f.store.trace(&outcome.session.id).unwrap();
        assert!(trace.invocations.is_empty());
        let decision = trace
            .decisions
            .iter()
            .find_map(|d| d.links.resource_allocation.as_ref())
            .unwrap();
        assert!(!decision.accepted);
        assert_eq!(decision.implementation.id, "fixture.overspend");
    }
    fn native_catalog(f: &mut RunFixture) {
        let provider = f.engine.config.agents[0].provider.clone();
        f.engine.config.capabilities.insert(
            provider,
            ProviderCapabilities {
                models_complete: true,
                models: vec![
                    ModelCapabilities {
            picker_id: None,
            display_name: None, aliases: vec![], resolved_model: None,
                        id: "model-a".into(),
                        controls: Some(vec![NativeControl {
            display_name: None, value_names: Default::default(),
                            id: "effort".into(),
                            values: NativeControlValues::Choices {
                                options: vec!["brief".into(), "deliberate".into()],
                            },
                            default: None,
                        }]),
                    },
                    ModelCapabilities {
            picker_id: None,
            display_name: None, aliases: vec![], resolved_model: None,
                        id: "model-b".into(),
                        controls: Some(vec![NativeControl {
            display_name: None, value_names: Default::default(),
                            id: "effort".into(),
                            values: NativeControlValues::Choices {
                                options: vec!["adaptive".into()],
                            },
                            default: None,
                        }]),
                    },
                ],
                default_model: Some("model-a".into()),
                ..Default::default()
            },
        );
        for agent in &f.engine.config.agents {
            f.engine.config.execution.insert(
                agent.id.clone(),
                AgentExecutionPolicy {
                    defaults: ModelEffort {
                        model: Some("model-a".into()),
                        effort: Some("brief".into()),
                    },
                    ..Default::default()
                },
            );
        }
    }

    struct NativeChoice {
        forbidden: bool,
    }
    impl AllocationPolicy for NativeChoice {
        fn identity(&self) -> ExecutionBackendIdentity {
            ExecutionBackendIdentity {
                id: "fixture.native-choice".into(),
                version: "3".into(),
            }
        }
        fn propose(&self, input: &AllocationInput) -> Result<AllocationProposal> {
            let mut proposal = BoundedAllocationPolicy.propose(input)?;
            if let Some(executor) = &mut proposal.executor {
                let effort = if input.demand.difficulty == "complex" || self.forbidden {
                    "deliberate"
                } else {
                    "brief"
                };
                executor.settings = ModelEffort {
                    model: Some("model-a".into()),
                    effort: Some(effort.into()),
                };
            }
            proposal.method = "native_control_experiment".into();
            proposal.rationale = "Use exact model-a labels for this assignment only".into();
            Ok(proposal)
        }
    }

    #[tokio::test]
    async fn allocation_policy_substitution_reaches_native_settings_without_a_universal_effort_ladder(
    ) {
        let mut f = RunFixture::new("", false);
        native_catalog(&mut f);
        let backend = Arc::new(TwoTasks {
            requests: Mutex::new(vec![]),
        });
        f.engine = f
            .engine
            .with_execution_backend(backend.clone())
            .unwrap()
            .with_allocation_policy(Arc::new(NativeChoice { forbidden: false }))
            .unwrap();
        let outcome = f.run().await;
        assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
        let requests = backend.requests.lock().unwrap();
        let executions: Vec<_> = requests.iter().filter(|r| r.purpose == "execute").collect();
        assert_eq!(executions.len(), 2);
        assert_eq!(executions[0].profile.id, executions[1].profile.id);
        assert_eq!(executions[0].settings.effort.as_deref(), Some("brief"));
        assert_eq!(executions[1].settings.effort.as_deref(), Some("deliberate"));
        let decisions = f.store.allocation_decisions(&outcome.session.id).unwrap();
        assert!(decisions
            .iter()
            .all(|d| d.implementation.id == "fixture.native-choice"
                && d.implementation.version == "3"));
        assert!(decisions
            .iter()
            .any(|d| d.input.boundary == AllocationBoundary::ResultAvailable
                && !d.input.evidence.is_empty()));
        assert!(decisions
            .iter()
            .flat_map(|d| &d.input.candidates)
            .all(|c| !(c.settings.model.as_deref() == Some("model-b")
                && c.settings.effort.as_deref() == Some("deliberate"))));
    }

    #[tokio::test]
    async fn allocation_policy_cannot_override_captured_effort_pins() {
        let mut f = RunFixture::new("", false);
        native_catalog(&mut f);
        for policy in f.engine.config.execution.values_mut() {
            policy.fixed.effort = Some("brief".into());
        }
        let outcome = f.run().await;
        let before = f
            .store
            .trace(&outcome.session.id)
            .unwrap()
            .invocations
            .len();
        f.engine.config.execution.clear();
        for profile in &mut f.engine.config.agents {
            profile.model = Some("unlisted-new-default".into());
        }
        let engine = f
            .engine
            .with_allocation_policy(Arc::new(NativeChoice { forbidden: true }))
            .unwrap();
        let error = engine
            .reconsider_allocation(
                &outcome.session.id,
                AllocationBoundary::GoalChanged,
                demand("execute", 1),
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("invalid_execution_choice"),
            "{error:#}"
        );
        assert_eq!(
            f.store
                .trace(&outcome.session.id)
                .unwrap()
                .invocations
                .len(),
            before
        );
        assert!(
            !f.store
                .allocation_decisions(&outcome.session.id)
                .unwrap()
                .last()
                .unwrap()
                .accepted
        );
    }

    struct ConsumeReviewer;
    impl AllocationPolicy for ConsumeReviewer {
        fn identity(&self) -> ExecutionBackendIdentity {
            ExecutionBackendIdentity {
                id: "fixture.consume-reviewer".into(),
                version: "1".into(),
            }
        }
        fn propose(&self, input: &AllocationInput) -> Result<AllocationProposal> {
            let mut proposal = BoundedAllocationPolicy.propose(input)?;
            let reviewer = proposal.reserved_final_reviewer.as_ref().unwrap();
            proposal.executor = input
                .candidates
                .iter()
                .find(|c| &c.agent_id == reviewer)
                .cloned();
            Ok(proposal)
        }
    }
    #[tokio::test]
    async fn allocation_infeasible_final_review_is_explicit_even_for_a_dishonest_policy() {
        let mut f = RunFixture::new("", false);
        f.engine.config.team_constraints.fixed_roster = Some(vec!["one".into(), "two".into()]);
        let outcome = f.run().await;
        let before = f
            .store
            .trace(&outcome.session.id)
            .unwrap()
            .invocations
            .len();
        let engine = f
            .engine
            .with_allocation_policy(Arc::new(ConsumeReviewer))
            .unwrap();
        let error = engine
            .reconsider_allocation(
                &outcome.session.id,
                AllocationBoundary::WorkReady,
                demand("execute", 1),
            )
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("no_independent_eligible_reviewer"),
            "{error:#}"
        );
        assert_eq!(
            f.store
                .trace(&outcome.session.id)
                .unwrap()
                .invocations
                .len(),
            before
        );
        assert_eq!(
            f.store
                .team_state(&outcome.session.id)
                .unwrap()
                .unwrap()
                .current_members
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn allocation_verified_experience_changes_native_choice_but_unconfirmed_claims_do_not() {
        let mut f = RunFixture::new("", false);
        native_catalog(&mut f);
        f.engine
            .acceptance_contracts
            .push(super::super::confirmation_tests::exact_contract());
        // Acquire one actual, confirmed outcome in a deliberately different configuration.
        f.engine
            .set_assignment_settings(vec![AssignmentSettingsRule {
                agent_id: "one".into(),
                purpose: Some("execute".into()),
                task_id: None,
                settings: ModelEffort {
                    model: Some("model-b".into()),
                    effort: Some("adaptive".into()),
                },
            }])
            .unwrap();
        let first = f.run().await;
        assert_eq!(first.session.status, "completed", "{}", first.summary);
        let observed = f.store.observations().unwrap();
        assert_eq!(observed.len(), 1);
        let mut unsupported = observed[0].clone();
        unsupported.id = new_id();
        unsupported.confirmation = ConfirmationStatus::Unknown;
        assert!(f.store.observe(&unsupported).unwrap());
        f.engine.set_assignment_settings(vec![]).unwrap();
        let second = f.run().await;
        assert_ne!(second.session.id, first.session.id);
        assert_eq!(second.session.status, "completed", "{}", second.summary);
        let decisions = f.store.allocation_decisions(&second.session.id).unwrap();
        let selection = decisions
            .iter()
            .find(|d| d.input.demand.purpose == "execute")
            .unwrap();
        assert_eq!(
            selection
                .proposal
                .executor
                .as_ref()
                .unwrap()
                .settings
                .model
                .as_deref(),
            Some("model-b")
        );
        assert_eq!(
            selection
                .proposal
                .executor
                .as_ref()
                .unwrap()
                .experience
                .successes,
            1
        );
        assert!(
            selection
                .input
                .candidates
                .iter()
                .any(|c| c.settings.model.as_deref() == Some("model-a")
                    && c.experience.successes == 0)
        );
    }

    #[tokio::test]
    async fn allocation_failed_check_reconsiders_evidence_without_automatic_effort_escalation() {
        let mut f = RunFixture::new("[mock:reject:review]", false);
        native_catalog(&mut f);
        let outcome = f.run().await;
        assert_eq!(outcome.session.status, "blocked");
        let trace = f.store.trace(&outcome.session.id).unwrap();
        let executions: Vec<_> = trace
            .assignments
            .iter()
            .filter(|a| a.purpose == "execute")
            .collect();
        assert!(executions.len() > 1);
        assert!(executions
            .iter()
            .all(|a| a.requested.effort.as_deref() == Some("brief")));
        assert!(f
            .store
            .allocation_decisions(&outcome.session.id)
            .unwrap()
            .iter()
            .any(|d| d.input.boundary == AllocationBoundary::CheckFailed
                && d.input.evidence.iter().any(|e| e.kind == "task_rejected")));
    }

    #[tokio::test]
    async fn allocation_availability_refresh_blocks_grants_and_active_responsibilities_cannot_depart(
    ) {
        let mut f = RunFixture::new("", false);
        add_agent(&mut f, "three");
        let outcome = f.run().await;
        let session = &outcome.session.id;
        let before = f.store.trace(session).unwrap();
        let current = f.store.team_state(session).unwrap().unwrap();
        let held = current.current_members[0].clone();
        let ctx = settings_context(&f, f.store.session(session).unwrap()).await;
        let mut assignment = before.assignments[0].clone();
        assignment.id = new_id();
        assignment.agent_id = held.clone();
        assignment.task = None;
        assignment.purpose = "conversation".into();
        assignment.state = InvocationState::Running;
        assignment.ended_at = None;
        assignment.grant_ids.clear();
        let mut invocation = before.invocations[0].clone();
        invocation.id = new_id();
        invocation.assignment_id = assignment.id.clone();
        invocation.requested = assignment.requested.clone();
        invocation.turn = before.usage.total.calls + 1;
        invocation.state = InvocationState::Running;
        invocation.ended_at = None;
        invocation.usage = None;
        invocation.terminal_reason = None;
        ctx.server
            .admit(&mut assignment, &invocation, TeamOperation::coordination())
            .unwrap();
        let ids = ["one", "two", "three"]
            .into_iter()
            .filter(|id| *id != held)
            .map(str::to_owned)
            .collect();
        let engine = f
            .engine
            .clone()
            .with_allocation_policy(Arc::new(MembershipPolicy { ids }))
            .unwrap();
        assert!(engine
            .reconsider_allocation(
                session,
                AllocationBoundary::ParticipantUnavailable,
                demand("execute", 0)
            )
            .unwrap_err()
            .to_string()
            .contains("active_responsibility"));
        assert_eq!(
            f.store
                .team_state(session)
                .unwrap()
                .unwrap()
                .current_members,
            current.current_members
        );
        ctx.server
            .finish(
                &invocation.id,
                InvocationState::Completed,
                Some("fixture complete"),
            )
            .unwrap();
        f.engine
            .config
            .agents
            .iter_mut()
            .find(|a| a.id == held)
            .unwrap()
            .enabled = false;
        f.engine.refresh_team_eligibility(session).unwrap();
        assert!(!f
            .store
            .team_state(session)
            .unwrap()
            .unwrap()
            .eligible_agents
            .contains(&held));
        assignment.id = new_id();
        assignment.grant_ids.clear();
        invocation.id = new_id();
        invocation.assignment_id = assignment.id.clone();
        invocation.turn += 1;
        let rejected =
            ctx.server
                .admit(&mut assignment, &invocation, TeamOperation::coordination());
        assert!(
            rejected.is_err(),
            "An ineligible participant received admission"
        );
        assert!(rejected
            .err()
            .unwrap()
            .to_string()
            .contains("ineligible_member"));
        assert!(f
            .store
            .session(session)
            .unwrap()
            .team
            .iter()
            .any(|a| a.id == held));
    }

    struct SmallerResources;
    impl ResourceAllocationPolicy for SmallerResources {
        fn identity(&self) -> ExecutionBackendIdentity {
            ExecutionBackendIdentity {
                id: "fixture.small-resources".into(),
                version: "1".into(),
            }
        }
        fn propose(&self, input: &ResourceAllocationInput) -> Result<InvocationAllowance> {
            Ok(InvocationAllowance {
                timeout_secs: input.budget.limits.turn_timeout_secs.min(3),
                native_max_turns: 1,
                max_output_chars: 10000,
                rationale: "Small bounded native invocation for the scripted workload".into(),
            })
        }
    }
    #[tokio::test]
    async fn allocation_resource_substitution_changes_actual_controls_with_the_same_admission_ledger(
    ) {
        let mut f = RunFixture::new("", false);
        let backend = Arc::new(TwoTasks {
            requests: Mutex::new(vec![]),
        });
        f.engine = f
            .engine
            .with_execution_backend(backend.clone())
            .unwrap()
            .with_resource_policy(Arc::new(SmallerResources))
            .unwrap();
        let outcome = f.run().await;
        assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
        assert!(backend
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| r.timeout_secs == 3
                && r.resource_controls.max_turns == Some(1)
                && r.resource_controls.max_output_chars == Some(10000)));
        let trace = f.store.trace(&outcome.session.id).unwrap();
        assert_eq!(
            trace.budget.unwrap().admitted_invocations,
            trace.invocations.len() as u64
        );
        assert!(trace.assignments.iter().all(|a| a.timeout_secs == 3));
        assert!(trace
            .decisions
            .iter()
            .filter_map(|d| d.links.resource_allocation.as_ref())
            .all(|d| d.accepted && d.implementation.id == "fixture.small-resources"));
    }
    #[tokio::test]
    async fn allocation_fixed_size_one_replaces_identity_for_independent_review() {
        let mut f = RunFixture::new("", false);
        f.engine.config.team_constraints.fixed_size = Some(1);
        let outcome = f.run().await;
        assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
        let trace = f.store.trace(&outcome.session.id).unwrap();
        assert_eq!(trace.policy.unwrap().captured_team.len(), 1);
        assert_eq!(trace.team_state.unwrap().current_members.len(), 1);
        assert_eq!(trace.session.team.len(), 2);
        let producer = trace
            .assignments
            .iter()
            .find(|a| a.purpose == "execute")
            .unwrap();
        let reviewer = trace
            .assignments
            .iter()
            .find(|a| a.purpose == "final_review")
            .unwrap();
        assert_ne!(producer.agent_id, reviewer.agent_id);
    }
}

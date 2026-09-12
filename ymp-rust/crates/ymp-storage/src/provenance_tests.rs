use super::*;

struct Fixture {
    _temp: tempfile::TempDir,
    store: Store,
    session: Session,
    task: Task,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(&temp.path().join("state")).unwrap();
        let project = store.project(temp.path()).unwrap();
        let team = ["writer", "reviewer"]
            .map(|id| AgentProfile {
                id: id.into(),
                name: id.into(),
                provider: "mock".into(),
                model: Some("requested-model".into()),
                instructions: String::new(),
                enabled: true,
            })
            .to_vec();
        let session = Session {
            id: new_id(),
            project_id: project.id,
            title: "original goal".into(),
            status: "running".into(),
            created_at: now(),
            team: team.clone(),
            turns_used: 0,
        };
        let policy = SessionPolicy {
            session_id: session.id.clone(),
            goal: "complete original goal with constraints".into(),
            constraints: None,
            cwd: temp.path().canonicalize().unwrap(),
            limits: Limits::default(),
            eligible_pool: team.clone(),
            execution: Default::default(),
            assignment_settings: Default::default(),
            captured_team: team,
            parent_session_id: None,
            evaluation: None,
            captured_at: now(),
        };
        store.create_session(&session, &policy).unwrap();
        let task = Task {
            id: new_id(),
            session_id: session.id.clone(),
            title: "same title".into(),
            description: "expected result".into(),
            competence: "implementation".into(),
            difficulty: "simple".into(),
            dependencies: vec![],
            checks: vec![],
            state: TaskState::Running,
            assignee: Some("writer".into()),
            reviewer: None,
            attempts: 1,
            result: None,
            workspace: None,
            base_commit: None,
            interrupted: false,
        };
        store.save_task(&task).unwrap();
        Self {
            _temp: temp,
            store,
            session,
            task,
        }
    }
    fn invocation(&self, turn: u64) -> (AssignmentRecord, InvocationRecord) {
        let requested = ExecutionSettings {
            model: Some("requested-model".into()),
            effort: None,
            permission_mode: Some("read_only".into()),
        };
        let assignment = AssignmentRecord {
            id: new_id(),
            session_id: self.session.id.clone(),
            task: Some(TaskAttemptRef::from(&self.task)),
            agent_id: "writer".into(),
            agent_config_version: "immutable-config-v1".into(),
            provider_id: "mock".into(),
            purpose: "execute".into(),
            reason: "Selected for this task".into(),
            cwd: self._temp.path().into(),
            requested: requested.clone(),
            timeout_secs: 10,
            grant_ids: vec![],
            context: vec![],
            state: InvocationState::Running,
            started_at: now(),
            ended_at: None,
        };
        let invocation = InvocationRecord {
            id: new_id(),
            session_id: self.session.id.clone(),
            assignment_id: assignment.id.clone(),
            turn,
            requested,
            sent: ExecutionSettings::default(),
            reported: ExecutionSettings::default(),
            resumed_from: Some("old-native-context".into()),
            native_session_id: None,
            native_turn_id: None,
            native_version: None,
            state: InvocationState::Running,
            started_at: now(),
            ended_at: None,
            usage: None,
            terminal_reason: None,
        };
        (assignment, invocation)
    }

    fn reviewed_plan(&self) -> (Vec<Task>, DecisionRecord) {
        let plan = Plan {
            summary: "Inspect, draft, then integrate the result".into(),
            tasks: vec![
                PlanTask {
                    title: "Integrate".into(),
                    description: "Integrate the draft with source findings".into(),
                    competence: "implementation".into(),
                    difficulty: "complex".into(),
                    dependencies: vec![2, 1],
                    checks: vec![
                        "check-integrated-content".into(),
                        "check-source-attribution".into(),
                    ],
                },
                PlanTask {
                    title: "Inspect".into(),
                    description: "Inspect the source material".into(),
                    competence: "analysis".into(),
                    difficulty: "simple".into(),
                    dependencies: vec![],
                    checks: vec!["check-source-findings".into()],
                },
                PlanTask {
                    title: "Draft".into(),
                    description: "Write a draft using the findings".into(),
                    competence: "implementation".into(),
                    difficulty: "standard".into(),
                    dependencies: vec![1],
                    checks: vec!["check-draft".into()],
                },
            ],
        };
        plan.validate().unwrap();
        let (mut producer, producer_invocation) = self.invocation(1);
        producer.task = None;
        producer.purpose = "plan".into();
        self.store
            .begin_invocation(&producer, &producer_invocation)
            .unwrap();
        self.store
            .finish_invocation(
                &self.session.id,
                &producer_invocation.id,
                InvocationState::Completed,
                None,
            )
            .unwrap();
        let version = PlanVersion {
            proposal_id: new_id(),
            revision: 1,
            producer_assignment_id: producer.id.clone(),
            producer_invocation_id: producer_invocation.id.clone(),
            plan,
        };
        self.store
            .record_decision(&DecisionRecord {
                id: new_id(),
                session_id: self.session.id.clone(),
                kind: "plan_proposed".into(),
                actor: Some(producer.agent_id),
                reason: version.plan.summary.clone(),
                outcome: None,
                links: RecordLinks {
                    assignment_id: Some(producer.id),
                    invocation_id: Some(producer_invocation.id),
                    plan_proposal: Some(version.clone()),
                    ..Default::default()
                },
                created_at: now(),
            })
            .unwrap();
        let (mut reviewer, review_invocation) = self.invocation(2);
        reviewer.task = None;
        reviewer.agent_id = "reviewer".into();
        reviewer.purpose = "review_plan".into();
        self.store
            .begin_invocation(&reviewer, &review_invocation)
            .unwrap();
        self.store
            .finish_invocation(
                &self.session.id,
                &review_invocation.id,
                InvocationState::Completed,
                None,
            )
            .unwrap();
        let review_id = new_id();
        self.store
            .record_decision(&DecisionRecord {
                id: review_id.clone(),
                session_id: self.session.id.clone(),
                kind: "plan_review".into(),
                actor: Some(reviewer.agent_id),
                reason: "The tasks and dependency graph cover the requested result".into(),
                outcome: Some(DecisionOutcome::Accepted {
                    confirmation: ConfirmationStatus::Unknown,
                }),
                links: RecordLinks {
                    assignment_id: Some(reviewer.id.clone()),
                    invocation_id: Some(review_invocation.id.clone()),
                    plan_proposal: Some(version.clone()),
                    ..Default::default()
                },
                created_at: now(),
            })
            .unwrap();
        let ids = [
            "generated-integrate",
            "generated-inspect",
            "generated-draft",
        ];
        let tasks = version
            .plan
            .tasks
            .iter()
            .enumerate()
            .map(|(index, definition)| Task {
                id: ids[index].into(),
                session_id: self.session.id.clone(),
                title: definition.title.clone(),
                description: definition.description.clone(),
                competence: definition.competence.clone(),
                difficulty: definition.difficulty.clone(),
                checks: definition.checks.clone(),
                dependencies: match index {
                    0 => vec![ids[2].into(), ids[1].into()],
                    1 => vec![],
                    2 => vec![ids[1].into()],
                    _ => unreachable!(),
                },
                state: TaskState::Ready,
                assignee: None,
                reviewer: None,
                attempts: 0,
                result: None,
                workspace: None,
                base_commit: None,
                interrupted: false,
            })
            .collect::<Vec<_>>();
        let commitment = DecisionRecord {
            id: new_id(),
            session_id: self.session.id.clone(),
            kind: "plan_committed".into(),
            actor: None,
            reason: "Commit the independently reviewed plan".into(),
            outcome: None,
            links: RecordLinks {
                assignment_id: Some(reviewer.id),
                invocation_id: Some(review_invocation.id),
                review_ids: vec![review_id],
                related_task_ids: ids.iter().map(|id| (*id).into()).collect(),
                plan_proposal: Some(version),
                ..Default::default()
            },
            created_at: now(),
        };
        (tasks, commitment)
    }
}

#[test]
fn plan_commit_rejects_altered_task_bodies_without_state_or_event_changes() {
    for alteration in [
        "description",
        "missing_checks",
        "changed_checks",
        "missing_dependencies",
        "wrong_dependency",
        "dependency_order",
        "extra_task",
        "missing_task",
        "task_order",
        "link_order",
        "title",
        "competence",
        "difficulty",
        "missing_version",
    ] {
        let f = Fixture::new();
        let (mut tasks, mut decision) = f.reviewed_plan();
        match alteration {
            "description" => tasks[0].description = "Perform different work".into(),
            "missing_checks" => tasks[0].checks.clear(),
            "changed_checks" => tasks[0].checks[0] = "weaker-check".into(),
            "missing_dependencies" => tasks[0].dependencies.clear(),
            "wrong_dependency" => tasks[0].dependencies[0] = tasks[0].id.clone(),
            "dependency_order" => tasks[0].dependencies.reverse(),
            "extra_task" => {
                let mut extra = tasks[1].clone();
                extra.id = "unreviewed-extra-task".into();
                tasks.push(extra);
            }
            "missing_task" => {
                tasks.remove(0);
            }
            "task_order" => tasks.swap(0, 1),
            "link_order" => {}
            "title" => tasks[0].title = "Different title".into(),
            "competence" => tasks[0].competence = "analysis".into(),
            "difficulty" => tasks[0].difficulty = "simple".into(),
            "missing_version" => decision.links.plan_proposal = None,
            _ => unreachable!(),
        }
        decision.links.related_task_ids = tasks.iter().map(|task| task.id.clone()).collect();
        if alteration == "link_order" {
            decision.links.related_task_ids.reverse();
        }
        let before = serde_json::to_value(f.store.trace(&f.session.id).unwrap()).unwrap();
        assert!(
            f.store.save_plan_with_decision(&tasks, &decision).is_err(),
            "committed unreviewed alteration: {alteration}"
        );
        assert_eq!(
            serde_json::to_value(f.store.trace(&f.session.id).unwrap()).unwrap(),
            before,
            "rejected {alteration} changed tasks, decisions, or events"
        );
    }
}

#[test]
fn plan_commit_preserves_reviewed_order_and_forward_dependency_mapping() {
    let f = Fixture::new();
    let (tasks, decision) = f.reviewed_plan();
    f.store.save_plan_with_decision(&tasks, &decision).unwrap();
    let trace = f.store.trace(&f.session.id).unwrap();
    let committed = trace
        .tasks
        .iter()
        .filter(|task| decision.links.related_task_ids.contains(&task.id))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        serde_json::to_value(&committed).unwrap(),
        serde_json::to_value(&tasks).unwrap()
    );
    assert_eq!(
        committed[0].dependencies,
        ["generated-draft", "generated-inspect"]
    );
    assert!(committed[1].dependencies.is_empty());
    assert_eq!(committed[2].dependencies, ["generated-inspect"]);
    assert_eq!(
        trace.decisions.last().unwrap().links.plan_proposal,
        decision.links.plan_proposal
    );
    let reopened = Store::open_read_only(&f.store.home).unwrap();
    assert_eq!(
        serde_json::to_value(reopened.trace(&f.session.id).unwrap()).unwrap(),
        serde_json::to_value(trace).unwrap()
    );
}

#[test]
fn provenance_is_immutable_attributed_and_exportable_after_reopen() {
    let f = Fixture::new();
    let mut policy = f.store.session_policy(&f.session.id).unwrap().unwrap();
    policy.goal = "silently changed goal".into();
    assert!(f.store.create_session(&f.session, &policy).is_err());
    let (mut assignment, invocation) = f.invocation(1);
    f.store.begin_invocation(&assignment, &invocation).unwrap();
    assignment.agent_config_version = "changed-after-admission".into();
    assignment.requested.model = Some("different-model".into());
    assert!(f.store.begin_invocation(&assignment, &invocation).is_err());
    let observed = InvocationObservation {
        permission_limitations: vec![],
        sent: Some(ExecutionSettings {
            model: Some("wire-model".into()),
            ..Default::default()
        }),
        reported: Some(ExecutionSettings {
            model: Some("resolved-native-model".into()),
            ..Default::default()
        }),
        native_session_id: Some("native-session".into()),
        native_turn_id: Some("native-turn".into()),
        native_version: Some("native-1.2".into()),
        usage: Some(UsageSnapshot {
            counts: TokenCounts {
                input: Some(100),
                output: Some(20),
                ..Default::default()
            },
            finalized: true,
            ..Default::default()
        }),
    };
    f.store
        .observe_invocation(&f.session.id, &invocation.id, &observed)
        .unwrap();
    f.store
        .finish_invocation(
            &f.session.id,
            &invocation.id,
            InvocationState::Completed,
            None,
        )
        .unwrap();
    assert!(f
        .store
        .update_usage(&f.session.id, 1, observed.usage.as_ref().unwrap())
        .is_err());
    assert!(f
        .store
        .finish_invocation(&f.session.id, &invocation.id, InvocationState::Failed, None)
        .is_err());
    assert!(f.store.begin_usage(&f.session.id, 1, "reviewer").is_err());
    let trace = f.store.trace(&f.session.id).unwrap();
    assert_eq!(
        trace.assignments[0].agent_config_version,
        "immutable-config-v1"
    );
    assert_eq!(
        trace.invocations[0].requested.model.as_deref(),
        Some("requested-model")
    );
    assert_eq!(
        trace.invocations[0].sent.model.as_deref(),
        Some("wire-model")
    );
    assert_eq!(
        trace.invocations[0].reported.model.as_deref(),
        Some("resolved-native-model")
    );
    assert!(trace.invocations[0].reported.effort.is_none());
    assert_eq!(trace.usage.agents.len(), 1);
    assert_eq!(trace.usage.agents["writer"].known_total(), Some(120));
    assert!(!trace.usage.total.is_partial());
    assert!(trace
        .history
        .iter()
        .filter(|e| e.kind == "provenance")
        .all(|e| serde_json::from_value::<ProvenanceEvent>(e.data.clone()).is_ok()));
    let raw = serde_json::to_value(&trace).unwrap();
    let _: SessionTrace = serde_json::from_value(raw.clone()).unwrap();
    let reopened = Store::open_read_only(&f.store.home).unwrap();
    assert_eq!(
        serde_json::to_value(reopened.trace(&f.session.id).unwrap()).unwrap(),
        raw
    );
    assert!(reopened.save_task(&f.task).is_err());
    assert!(!f.store.home.join("config.toml").exists());
}

#[test]
fn wrong_scope_stale_attempt_and_identity_collisions_leave_no_partial_writes() {
    let f = Fixture::new();
    let (assignment, invocation) = f.invocation(1);
    let mut other = f.session.clone();
    other.id = new_id();
    f.store.save_session(&other).unwrap();
    let mut collision = f.task.clone();
    collision.session_id = other.id.clone();
    assert!(f.store.save_task(&collision).is_err());
    let mut foreign = assignment.clone();
    foreign.session_id = other.id.clone();
    let mut foreign_invocation = invocation.clone();
    foreign_invocation.session_id = other.id.clone();
    assert!(f
        .store
        .begin_invocation(&foreign, &foreign_invocation)
        .is_err());
    let mut stale = assignment.clone();
    stale.task.as_mut().unwrap().attempt = 0;
    assert!(f.store.begin_invocation(&stale, &invocation).is_err());
    f.store.begin_invocation(&assignment, &invocation).unwrap();
    assert!(f.store.invocation(&other.id, &invocation.id).is_err());
    assert!(f
        .store
        .observe_invocation(&other.id, &invocation.id, &InvocationObservation::default())
        .is_err());
    assert!(f
        .store
        .finish_invocation(&other.id, &invocation.id, InvocationState::Completed, None)
        .is_err());
    let (duplicate_assignment, duplicate_invocation) = f.invocation(1);
    assert!(f
        .store
        .begin_invocation(&duplicate_assignment, &duplicate_invocation)
        .is_err());
    let mut candidate = f.task.clone();
    candidate.state = TaskState::Review;
    let decision = DecisionRecord {
        id: new_id(),
        session_id: f.session.id.clone(),
        kind: "result_submitted".into(),
        actor: Some("writer".into()),
        reason: "Produced this result".into(),
        outcome: None,
        links: RecordLinks {
            task: Some(TaskAttemptRef::from(&candidate)),
            invocation_id: Some("nonexistent".into()),
            ..Default::default()
        },
        created_at: now(),
    };
    let error = f
        .store
        .save_task_with_decision(&candidate, &decision)
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<rusqlite::Error>(),
        Some(rusqlite::Error::QueryReturnedNoRows)
    ));
    let trace = f.store.trace(&f.session.id).unwrap();
    assert_eq!(trace.tasks[0].state, TaskState::Running);
    assert_eq!(trace.assignments.len(), 1);
    assert_eq!(trace.invocations.len(), 1);
    assert!(trace.decisions.is_empty());
    assert_eq!(trace.usage.total.calls, 1);
}

#[test]
fn stale_task_decisions_cannot_rewind_attempts_or_overwrite_a_later_state() {
    let f = Fixture::new();
    let mut candidate = f.task.clone();
    candidate.state = TaskState::Review;
    candidate.result = Some("attempt-one candidate".into());
    f.store.save_task(&candidate).unwrap();
    let (mut assignment, invocation) = f.invocation(1);
    assignment.agent_id = "reviewer".into();
    assignment.purpose = "review".into();
    f.store.begin_invocation(&assignment, &invocation).unwrap();
    f.store
        .finish_invocation(
            &f.session.id,
            &invocation.id,
            InvocationState::Completed,
            None,
        )
        .unwrap();
    let mut accepted = candidate.clone();
    accepted.state = TaskState::Accepted;
    accepted.reviewer = Some("reviewer".into());
    let decision = DecisionRecord {
        id: new_id(),
        session_id: f.session.id.clone(),
        kind: "task_accepted".into(),
        actor: Some("reviewer".into()),
        reason: "Acceptance of attempt one".into(),
        outcome: Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Unknown,
        }),
        links: RecordLinks {
            task: Some(TaskAttemptRef::from(&accepted)),
            assignment_id: Some(assignment.id),
            invocation_id: Some(invocation.id),
            ..Default::default()
        },
        created_at: now(),
    };
    for (attempts, state) in [
        (2, TaskState::Running),
        (2, TaskState::Review),
        (1, TaskState::Running),
        (1, TaskState::Ready),
        (1, TaskState::Accepted),
        (1, TaskState::Blocked),
    ] {
        let mut current = candidate.clone();
        current.attempts = attempts;
        current.state = state;
        current.result = Some("current result must survive".into());
        f.store.save_task(&current).unwrap();
        let before = serde_json::to_value(f.store.trace(&f.session.id).unwrap()).unwrap();
        assert!(
            f.store
                .save_task_with_decision(&accepted, &decision)
                .is_err(),
            "stale acceptance overwrote attempt {attempts} in {state:?}"
        );
        assert_eq!(
            serde_json::to_value(f.store.trace(&f.session.id).unwrap()).unwrap(),
            before,
            "a rejected stale write changed state or events"
        );
    }
    // The same candidate can be accepted while its actual attempt is in review.
    f.store.save_task(&candidate).unwrap();
    f.store
        .save_task_with_decision(&accepted, &decision)
        .unwrap();
    assert_eq!(
        f.store.tasks(&f.session.id).unwrap()[0].state,
        TaskState::Accepted
    );
}

#[test]
fn out_of_order_admissions_do_not_rewind_the_durable_turn_counter() {
    let f = Fixture::new();
    for turn in [2, 1] {
        let (assignment, invocation) = f.invocation(turn);
        f.store.begin_invocation(&assignment, &invocation).unwrap();
    }
    assert_eq!(
        f.store.value(&format!("turns:{}", f.session.id)).unwrap(),
        Some(serde_json::json!(2))
    );
}

#[test]
fn event_failure_rolls_back_invocation_and_usage_state() {
    let f = Fixture::new();
    let (assignment, invocation) = f.invocation(1);
    let trigger = "CREATE TRIGGER reject_event BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'injected event failure'); END";
    f.store.db().unwrap().execute_batch(trigger).unwrap();
    assert!(f.store.begin_invocation(&assignment, &invocation).is_err());
    let trace = f.store.trace(&f.session.id).unwrap();
    assert!(trace.assignments.is_empty());
    assert!(trace.invocations.is_empty());
    assert_eq!(trace.usage.total.calls, 0);
    f.store
        .db()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_event")
        .unwrap();
    f.store.begin_invocation(&assignment, &invocation).unwrap();
    f.store.db().unwrap().execute_batch(trigger).unwrap();
    assert!(f
        .store
        .finish_invocation(
            &f.session.id,
            &invocation.id,
            InvocationState::Completed,
            None
        )
        .is_err());
    assert_eq!(
        f.store
            .invocation(&f.session.id, &invocation.id)
            .unwrap()
            .state,
        InvocationState::Running
    );
    assert_eq!(
        f.store
            .session_usage(&f.session.id)
            .unwrap()
            .total
            .open_calls,
        1
    );
    f.store
        .db()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_event")
        .unwrap();
    f.store
        .finish_invocation(&f.session.id, &invocation.id, InvocationState::Failed, None)
        .unwrap();
}

#[test]
fn cancellation_failure_and_recovery_preserve_partial_and_unknown_usage() {
    let f = Fixture::new();
    for (turn, state) in [
        (1, InvocationState::Failed),
        (2, InvocationState::Cancelled),
        (3, InvocationState::Running),
    ] {
        let (assignment, invocation) = f.invocation(turn);
        f.store.begin_invocation(&assignment, &invocation).unwrap();
        if turn == 2 {
            f.store
                .observe_invocation(
                    &f.session.id,
                    &invocation.id,
                    &InvocationObservation {
                        usage: Some(UsageSnapshot {
                            counts: TokenCounts {
                                input: Some(17),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        if state != InvocationState::Running {
            f.store
                .finish_invocation(&f.session.id, &invocation.id, state, None)
                .unwrap();
        }
    }
    let reopened = Store::open(&f.store.home).unwrap();
    assert_eq!(
        reopened.interrupt_open_invocations(&f.session.id).unwrap(),
        1
    );
    assert_eq!(
        reopened.interrupt_open_invocations(&f.session.id).unwrap(),
        0
    );
    let trace = reopened.trace(&f.session.id).unwrap();
    assert_eq!(
        trace
            .invocations
            .iter()
            .map(|i| i.state)
            .collect::<Vec<_>>(),
        vec![
            InvocationState::Failed,
            InvocationState::Cancelled,
            InvocationState::Interrupted
        ]
    );
    assert!(trace.invocations[0].usage.is_none());
    assert!(trace.invocations[1].usage.as_ref().unwrap().partial);
    assert!(trace.invocations[2].usage.is_none());
    assert_eq!(trace.usage.total.open_calls, 0);
    assert_eq!(trace.usage.total.known_total(), Some(17));
    assert!(trace.usage.total.is_partial());
    assert_eq!(trace.tasks[0].state, TaskState::Running);
}

#[test]
fn decisions_preserve_confirmation_distinctions_without_grading() {
    let f = Fixture::new();
    for (index, outcome) in [
        None,
        Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Unknown,
        }),
        Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Unconfirmed,
        }),
        Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Confirmed,
        }),
        Some(DecisionOutcome::Rejected),
    ]
    .into_iter()
    .enumerate()
    {
        let decision = DecisionRecord {
            id: format!("decision-{index}"),
            session_id: f.session.id.clone(),
            kind: "assessment".into(),
            actor: Some("reviewer".into()),
            reason: "Captured assessment supplied by the trusted caller".into(),
            outcome: outcome.clone(),
            links: RecordLinks {
                task: Some(TaskAttemptRef::from(&f.task)),
                ..Default::default()
            },
            created_at: now(),
        };
        f.store.record_decision(&decision).unwrap();
        assert!(f.store.record_decision(&decision).is_err());
        assert_eq!(
            f.store.trace(&f.session.id).unwrap().decisions[index].outcome,
            outcome
        );
    }
    // Merely recording a declared grade has no acceptance/reputation side effect.
    assert_eq!(
        f.store.tasks(&f.session.id).unwrap()[0].state,
        TaskState::Running
    );
    assert!(f.store.observations().unwrap().is_empty());
}

#[test]
fn provenance_migration_is_transactional_and_legacy_capture_stays_unknown() {
    let f = Fixture::new();
    f.store.db().unwrap().execute_batch("DROP TABLE invocations; DROP TABLE assignments; DROP TABLE decisions; DROP TABLE session_policies; CREATE TABLE assignments(collision TEXT); PRAGMA user_version=2").unwrap();
    assert!(Store::open(&f.store.home).is_err());
    assert_eq!(
        f.store
            .db()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        2
    );
    assert!(f
        .store
        .db()
        .unwrap()
        .prepare("SELECT * FROM session_policies")
        .is_err());
    f.store
        .db()
        .unwrap()
        .execute_batch("DROP TABLE assignments")
        .unwrap();
    let migrated = Store::open(&f.store.home).unwrap();
    let trace = migrated.trace(&f.session.id).unwrap();
    assert!(trace.policy.is_none());
    assert!(trace.assignments.is_empty());
    assert_eq!(trace.tasks[0].id, f.task.id);
    migrated
        .db()
        .unwrap()
        .execute_batch("PRAGMA user_version=99")
        .unwrap();
    assert!(Store::open(&f.store.home).is_err());
    assert!(Store::open_read_only(&f.store.home).is_err());
    assert_eq!(
        migrated
            .db()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        99
    );
}

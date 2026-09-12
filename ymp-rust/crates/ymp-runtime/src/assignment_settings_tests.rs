async fn settings_context(fixture: &RunFixture, session: Session) -> RunContext {
    let limits = fixture.store.session_policy(&session.id).unwrap().unwrap().limits;
    let server = Arc::new(TeamServer::start(fixture.store.clone(), &session, fixture.engine.events.clone()).await.unwrap());
    RunContext {
        turns: Arc::new(AtomicUsize::new(session.turns_used)),
        permits: Arc::new(Semaphore::new(limits.parallel)),
        workspace: Workspace::open(&fixture.project, &fixture.store.session_dir(&session).join("workspace")).unwrap(),
        limits, session, server,
    }
}

fn settings_rule(agent: &AgentProfile, purpose: &str, model: Option<&str>, effort: Option<&str>) -> AssignmentSettingsRule {
    AssignmentSettingsRule { agent_id: agent.id.clone(), purpose: Some(purpose.into()), task_id: None, settings: ModelEffort { model: model.map(str::to_owned), effort: effort.map(str::to_owned) } }
}

#[tokio::test]
async fn assignment_settings_isolate_changes_and_preserve_known_default_and_explicit_continuations() {
    let fixture = RunFixture::new("", false);
    let session = fixture.run().await.session;
    let ctx = settings_context(&fixture, session).await;
    let agent = &ctx.session.team[0];
    let mut previous: Option<InvocationRecord> = None;
    for (model, effort, read_only, should_reuse) in [
        (None,None,true,false),
        (None,None,true,true),
        (Some("model-a"),Some("high"),true,false),
        (Some("model-a"),Some("high"),true,true),
        (Some("model-b"),Some("max"),true,false),
        (None,None,true,false),
        (None,None,true,true),
        (None,None,false,false),
    ] {
        fixture.engine.set_assignment_settings(vec![settings_rule(agent,"review",model,effort)]).unwrap();
        // Remove earlier demonstration continuations before the first control.
        if previous.is_none() {
            fixture.store.put_value(&format!("native:{}:{}:{}:read",ctx.session.id,agent.id,fixture.project.display()), &json!("legacy-unverified-session")).unwrap();
        }
        let response = fixture.engine.ask_scoped(&ctx,agent,&fixture.project,"review","Inspect the fixture",read_only,None).await.unwrap();
        let invocation = fixture.store.invocation(&ctx.session.id,&response.invocation_id).unwrap();
        assert_eq!(invocation.requested.model.as_deref(),model);
        assert_eq!(invocation.sent.effort.as_deref(),effort);
        assert_eq!(invocation.reported.effort,None);
        if should_reuse {
            assert_eq!(invocation.resumed_from,previous.as_ref().unwrap().native_session_id);
            assert_eq!(invocation.native_session_id,previous.as_ref().unwrap().native_session_id);
        } else { assert!(invocation.resumed_from.is_none()); }
        previous = Some(invocation);
    }
}

#[tokio::test]
async fn captured_pins_reject_conflicting_overrides_before_invocation_and_survive_config_edits() {
    let mut fixture = RunFixture::new("",false);
    fixture.engine.config.execution.insert("one".into(), AgentExecutionPolicy { fixed:ModelEffort { model:Some("pinned".into()),effort:Some("max".into()) }, ..Default::default() });
    let session = fixture.run().await.session;
    let ctx = settings_context(&fixture, session).await;
    let agent = ctx.session.team.iter().find(|a|a.id=="one").unwrap();
    fixture.engine.config.execution.clear();
    fixture.engine.set_assignment_settings(vec![settings_rule(agent,"review",Some("other"),None)]).unwrap();
    let turns = ctx.turns.load(Ordering::SeqCst);
    let before = fixture.store.trace(&ctx.session.id).unwrap().invocations.len();
    let result = fixture.engine.ask_scoped(&ctx,agent,&fixture.project,"review","Inspect",true,None).await;
    assert!(result.err().unwrap().to_string().contains("fixed model"));
    assert_eq!(ctx.turns.load(Ordering::SeqCst),turns);
    assert_eq!(fixture.store.trace(&ctx.session.id).unwrap().invocations.len(),before);
    fixture.engine.set_assignment_settings(vec![]).unwrap();
    let requested = fixture.engine.requested_settings(&ctx,agent,"review",None,true).unwrap();
    assert_eq!(requested.model.as_deref(),Some("pinned")); assert_eq!(requested.effort.as_deref(),Some("max"));
}

#[tokio::test]
async fn competence_lookup_and_updates_share_the_original_execution_configuration() {
    let fixture = RunFixture::new("",false);
    let outcome = fixture.run().await;
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    let ctx = settings_context(&fixture,outcome.session).await;
    let task = &trace.tasks[0];
    let agent = ctx.session.team.iter().find(|a|Some(&a.id)==task.assignee.as_ref()).unwrap();
    let producing_version = fixture.engine.observed_version(&ctx,agent,"execute",Some(TaskAttemptRef::from(task))).unwrap().unwrap();
    assert_eq!(fixture.engine.selection_version(&ctx,agent,"execute",Some(&task.id)).unwrap(),producing_version);
    let observation = fixture.store.observations().unwrap().into_iter().find(|o|o.id==format!("task:{}:{}",task.id,task.attempts)).unwrap();
    assert_eq!(observation.agent_version,producing_version);
    assert_ne!(producing_version,agent.version(fixture.engine.config.provider(&agent.provider).unwrap()));
    fixture.engine.set_assignment_settings(vec![settings_rule(agent,"review",Some("other-model"),Some("max"))]).unwrap();
    fixture.engine.ask_scoped(&ctx,agent,&fixture.project,"review","Inspect",true,None).await.unwrap();
    assert_eq!(fixture.engine.observed_version(&ctx,agent,"execute",Some(TaskAttemptRef::from(task))).unwrap(),Some(producing_version.clone()));
    assert_ne!(fixture.engine.selection_version(&ctx,agent,"review",None).unwrap(),producing_version);
}

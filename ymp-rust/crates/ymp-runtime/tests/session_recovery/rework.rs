//! Accepted YMP-146 RETURN controls, using only public runtime and scripted providers.
use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

struct ReadyCommitmentScript {
    script: Arc<Script>,
}
impl ExecutionBackend for ReadyCommitmentScript {
    fn identity(&self) -> ExecutionBackendIdentity {
        self.script.identity()
    }
    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        self.script.workspace_access(request)
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            if request.purpose == "execute" && request.prompt.contains("\nRead\n") {
                let board = team_call(&request, "board_read", json!({})).await?;
                let task = board["tasks"].as_array().unwrap().iter()
                    .find(|t| t["task"]["title"] == "Second read").unwrap();
                team_call(&request, "task_propose", json!({
                    "plan_version": board["plan_version"],
                    "rationale": "Take responsibility before later execution admission",
                    "change": {"kind":"accept_responsibility", "task": {
                        "task_id": task["task"]["id"], "version": task["version"]
                    }, "settings": {"model":request.settings.model,"effort":request.settings.effort}}
                })).await?;
            }
            let mut result = self.script.execute(request.clone(), events).await?;
            if request.purpose == "plan" {
                let mut plan: serde_json::Value = serde_json::from_str(&result.text)?;
                plan["tasks"].as_array_mut().unwrap().push(json!({
                    "title":"Second read", "description":"Use the first finding",
                    "access":"read_only", "competence":"analysis", "difficulty":"simple",
                    "dependencies":[0], "checks":[]
                }));
                result.text = plan.to_string();
            }
            Ok(result)
        })
    }
}
async fn team_call(request: &TurnRequest, name: &str, arguments: serde_json::Value) -> Result<serde_json::Value> {
    let endpoint = request.mcp.as_ref().unwrap();
    let mut socket = UnixStream::connect(endpoint.args.last().unwrap()).await?;
    socket.write_all(format!("{}\n", json!({"token":endpoint.token,"request_id":new_id(),"name":name,"arguments":arguments})).as_bytes()).await?;
    let mut line = String::new();
    BufReader::new(socket).read_line(&mut line).await?;
    let reply: serde_json::Value = serde_json::from_str(&line)?;
    anyhow::ensure!(reply["ok"] == true, "Team call failed: {reply}");
    Ok(reply["value"].clone())
}

async fn ready_departure(replace: bool) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let store = Store::open(&temp.path().join("state"))?;
    let script = Arc::new(Script {
        failure_purpose: "review", gate: None, mode: Mode::Unknown, writes: false,
        failures: AtomicUsize::new(1), calls: Mutex::new(vec![]),
    });
    let mut engine = engine(store.clone(), script.clone())
        .with_execution_backend(Arc::new(ReadyCommitmentScript { script: script.clone() }))?;
    engine.config.team = vec!["author".into(), "reviewer".into(), "reserve".into()];
    engine.config.team_constraints.fixed_roster = Some(engine.config.team.clone());
    let mut newcomer = engine.config.agents[0].clone();
    newcomer.id = "replacement".into();
    newcomer.name = "replacement".into();
    engine.config.agents.push(newcomer);
    let first = engine.run(&path, "Inspect", None).await?;
    assert_ne!(first.session.status, "completed");
    let session = &first.session.id;
    engine.commit_board_proposals(session)?;
    let board = engine.board(session)?;
    let target = board.tasks.iter().find(|t| t.task.title == "Second read").unwrap();
    let commitment = target.commitment.as_ref().unwrap_or_else(|| panic!("Real accepted board commitment required; outcome: {}; board: {board:?}; decisions: {:?}", first.summary, store.decisions(session).unwrap().iter().filter(|d| d.links.board.is_some()).collect::<Vec<_>>()));
    let departing = commitment.agent_id.clone();
    assert_eq!(target.task.state, TaskState::Ready);
    assert!(!store.trace(session)?.assignments.iter().any(|a| a.task.as_ref().is_some_and(|t| t.task_id == target.task.id)));
    let view = engine.team_control(session)?;
    let mut command = team_command(&view, if replace {
        OwnerTeamAction::Replace { agent_id: departing.clone(), replacement_id: "replacement".into() }
    } else { OwnerTeamAction::Remove { agent_id: departing.clone() } });
    command.revise_pinned_roster = true;
    let receipt = engine.owner_team_command(&command)?;
    assert_eq!(engine.owner_team_command(&command)?, receipt);
    let after_command = store.trace(session)?;
    let mut stale = command.clone();
    stale.command_id = new_id();
    assert!(engine.owner_team_command(&stale).unwrap_err().to_string().contains("stale_owner_command"));
    let mut conflict = command.clone();
    conflict.action = OwnerTeamAction::Pause;
    assert!(engine.owner_team_command(&conflict).unwrap_err().to_string().contains("owner_command_conflict"));
    assert_eq!(serde_json::to_value(store.trace(session)?)?, serde_json::to_value(after_command)?);
    let stage = engine.recovery_stages(session)?.into_iter().find(|s| s.purpose == "review").unwrap();
    engine.control_recovery(&RecoveryControlCommand {
        session_id: session.clone(), stage_id: stage.id, expected_revision: stage.revision,
        command_id: new_id(), action: RecoveryControl::Continue,
    })?;
    let before_calls = script.calls.lock().unwrap().len();
    let result = engine.run(&path, "Inspect", Some(session)).await?;
    assert_eq!(result.session.status, "completed", "{}", result.summary);
    let calls = script.calls.lock().unwrap();
    assert!(calls[before_calls..].iter().all(|r| r.profile.id != departing));
    assert!(calls[before_calls..].iter().any(|r| r.purpose == "execute" && r.prompt.contains("\nSecond read\n")));
    drop(calls);
    let after = engine.team_control(session)?;
    assert!(after.pending_departures.is_empty());
    assert!(!after.effective.current_members.contains(&departing));
    assert!(store.tasks(session)?.iter().all(|t| t.state == TaskState::Accepted));
    Ok(())
}
#[tokio::test]
async fn r1_remove_releases_ready_commitment_and_continues() -> Result<()> { ready_departure(false).await }
#[tokio::test]
async fn r1_replace_releases_ready_commitment_and_continues() -> Result<()> { ready_departure(true).await }

async fn membership_preserves_stop(exhausted: bool, replace: bool) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let store = Store::open(&temp.path().join("state"))?;
    let script = Arc::new(Script {
        failure_purpose: "review_plan", gate: None,
        mode: if exhausted { Mode::Transient } else { Mode::Unknown }, writes: false,
        failures: AtomicUsize::new(if exhausted { 2 } else { 1 }), calls: Mutex::new(vec![]),
    });
    let mut engine = engine(store.clone(), script.clone()).with_recovery_policy(Arc::new(BoundedRecoveryPolicy(RecoveryConfiguration {
        max_attempts: 1, max_provider_failures: 8, delay_ms: 0,
    })))?;
    engine.config.team_constraints.fixed_roster = Some(vec!["author".into(), "reviewer".into()]);
    let first = engine.run(&path, "Inspect", None).await?;
    let session = &first.session.id;
    let mut stage = engine.recovery_stages(session)?.into_iter().find(|s| s.purpose == "review_plan").unwrap();
    if !exhausted {
        engine.control_recovery(&RecoveryControlCommand {
            session_id: session.clone(), stage_id: stage.id.clone(), expected_revision: stage.revision,
            command_id: new_id(), action: RecoveryControl::Wait { condition: "Owner is inspecting the proposal".into() },
        })?;
        stage = engine.recovery_stages(session)?.into_iter().find(|s| s.id == stage.id).unwrap();
    } else {
        assert_eq!(stage.recovery_attempts, 1);
        assert_eq!(stage.failures.len(), 2);
    }
    let departing = stage.selected_agent.clone().unwrap();
    let before_calls = script.calls.lock().unwrap().len();
    let before = store.trace(session)?;
    let mut command = team_command(&engine.team_control(session)?, if replace {
        OwnerTeamAction::Replace { agent_id: departing.clone(), replacement_id: "reserve".into() }
    } else { OwnerTeamAction::Remove { agent_id: departing.clone() } });
    command.revise_pinned_roster = true;
    engine.owner_team_command(&command)?;
    if !replace {
        let mut add = team_command(&engine.team_control(session)?, OwnerTeamAction::Add { agent_id: "reserve".into() });
        add.revise_pinned_roster = true;
        engine.owner_team_command(&add)?;
    }
    let stopped = engine.run(&path, "Inspect", Some(session)).await?;
    assert_eq!(script.calls.lock().unwrap().len(), before_calls, "Membership must not authorize a retry");
    assert_ne!(stopped.session.status, "completed");
    let retained = engine.recovery_stages(session)?.into_iter().find(|s| s.id == stage.id).unwrap();
    assert_eq!(retained.status, stage.status);
    assert_eq!(retained.condition, stage.condition);
    assert!(!retained.manual_permit);
    assert_eq!(retained.recovery_attempts, stage.recovery_attempts);
    assert_eq!(retained.failures, stage.failures);
    assert_eq!(retained.plan, stage.plan);
    assert_eq!(store.trace(session)?.budget, before.budget);
    engine.control_recovery(&RecoveryControlCommand {
        session_id: session.clone(), stage_id: retained.id, expected_revision: retained.revision,
        command_id: new_id(), action: RecoveryControl::Continue,
    })?;
    let continued = engine.run(&path, "Inspect", Some(session)).await?;
    assert_eq!(continued.session.status, "completed", "{}", continued.summary);
    let calls = script.calls.lock().unwrap();
    assert!(calls[before_calls..].iter().all(|r| r.profile.id != departing));
    assert_eq!(calls[before_calls..].iter().filter(|r| r.purpose == "review_plan").count(), 1);
    drop(calls);
    let completed = engine.recovery_stages(session)?.into_iter().find(|s| s.id == stage.id).unwrap();
    assert_eq!(completed.recovery_attempts, stage.recovery_attempts);
    assert_eq!(completed.failures, stage.failures);
    Ok(())
}
#[tokio::test]
async fn r2_membership_preserves_explicit_owner_wait() -> Result<()> {
    for replace in [false, true] { membership_preserves_stop(false, replace).await?; }
    Ok(())
}
#[tokio::test]
async fn r2_membership_preserves_exhausted_policy() -> Result<()> {
    for replace in [false, true] { membership_preserves_stop(true, replace).await?; }
    Ok(())
}

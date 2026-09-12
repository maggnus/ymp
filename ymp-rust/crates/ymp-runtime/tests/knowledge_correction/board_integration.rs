//! Joint consumer of the actual board, captured correction and later retrieval.
use super::*;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
use ymp_providers::TurnResult;

struct CoordinatedCorrection {
    requests: Mutex<Vec<TurnRequest>>,
}

async fn team_reply(request: &TurnRequest, name: &str, arguments: Value) -> Result<Value> {
    let endpoint = request.mcp.as_ref().expect("Real team endpoint required");
    let mut stream = UnixStream::connect(endpoint.args.last().unwrap()).await?;
    stream.write_all(format!("{}\n", json!({"token":endpoint.token,"request_id":new_id(),"name":name,"arguments":arguments})).as_bytes()).await?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).await?;
    Ok(serde_json::from_str(&line)?)
}

async fn team_call(request: &TurnRequest, name: &str, arguments: Value) -> Result<Value> {
    let response = team_reply(request, name, arguments).await?;
    ensure!(
        response["ok"] == true,
        "Team operation failed: {}",
        response["error"]
    );
    Ok(response["value"].clone())
}

fn task_reference(board: &Value, title: &str) -> Value {
    let task = board["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["task"]["title"] == title)
        .unwrap();
    json!({"task_id":task["task"]["id"],"version":task["version"]})
}

async fn propose(request: &TurnRequest, board: &Value, change: Value) -> Result<()> {
    let reply = team_call(request, "task_propose", json!({"plan_version":board["plan_version"],"rationale":"Preserve the captured correction while distributing and revising pending work","change":change})).await?;
    ensure!(
        reply["status"] == "proposed",
        "A team proposal cannot commit itself"
    );
    Ok(())
}

impl ExecutionBackend for CoordinatedCorrection {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "test.board-knowledge-integration".into(),
            version: "1".into(),
        }
    }
    fn execute(
        &self,
        request: TurnRequest,
        _: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            ensure!(
                request.provider.kind == ProviderKind::Mock
                    && request.settings.effort.as_deref() == Some("low"),
                "Only mock/low execution is allowed"
            );
            self.requests.lock().unwrap().push(request.clone());
            let is_task = |title: &str| request.prompt.contains(&format!("\n{title}\n"));
            if request.purpose == "execute" {
                if is_task("Commit correction owner")
                    || is_task("Revise correction approach")
                    || is_task("Reassign correction owner")
                {
                    ensure!(
                        request.read_only,
                        "Coordination assignments remain read-only"
                    );
                    let board = team_call(&request, "board_read", json!({})).await?;
                    let target = task_reference(&board, "Record observation");
                    if is_task("Commit correction owner") {
                        propose(&request,&board,json!({"kind":"assign","task":target,"agent_id":"a","settings":{"effort":"low"}})).await?;
                    } else if is_task("Revise correction approach") {
                        let denied = team_reply(&request,"task_propose",json!({"plan_version":board["plan_version"],"rationale":"An agent cannot replace captured correction authority","change":{"kind":"revise","task":target,"approach":"Discard correction authority","dependencies":[],"checks":[],"knowledge_correction":null}})).await?;
                        ensure!(
                            denied["ok"] == false,
                            "A board revision must reject hidden correction authority"
                        );
                        ensure!(
                            team_call(&request, "board_read", json!({})).await?["proposals"]
                                == board["proposals"],
                            "Denied authority must not create a board proposal"
                        );
                        propose(&request,&board,json!({"kind":"revise","task":target,"approach":"Compare the changed source with the captured historical O04 claim before replacing it","dependencies":[task_reference(&board,"Commit correction owner")["task_id"]],"checks":["test -f inputs/observations-corrected.csv"]})).await?;
                    } else {
                        for target in [target, task_reference(&board, "Busy sibling")] {
                            propose(&request,&board,json!({"kind":"assign","task":target,"agent_id":"b","settings":{"effort":"low"}})).await?;
                        }
                    }
                } else if is_task("Record observation") {
                    ensure!(
                        request.profile.id == "b",
                        "The actual correction must honor its reassigned owner"
                    );
                    ensure!(request.prompt.contains("Board approach:") && request.prompt.contains("source_replacement") && request.prompt.contains("historical context, not current evidence"), "Revised execution must retain its captured correction obligation and historical source");
                    std::fs::write(
                        request.cwd.join("claim-60.json"),
                        serde_json::to_vec(
                            &json!({"row":"O04","site":"Hill","week":"2026-W36","value":60}),
                        )?,
                    )?;
                } else if is_task("Later failure") {
                    let found = team_call(
                        &request,
                        "memory_search",
                        json!({"query":"observation","scope":scope()}),
                    )
                    .await?;
                    ensure!(found["items"].as_array().unwrap().iter().any(|row| row["value"]["entry"]["supersedes"].is_string() && row["value"]["entry"]["content"].as_str().is_some_and(|content| content.contains("\"value\":60"))), "The actual board participant must retrieve the supported correction through scoped team search");
                    let mut harbor = scope();
                    harbor.insert("site".into(), "Harbor".into());
                    ensure!(
                        team_call(
                            &request,
                            "memory_search",
                            json!({"query":"observation","scope":harbor})
                        )
                        .await?["items"]
                            == json!([]),
                        "Scoped team lookup must exclude another site's claims"
                    );
                    std::fs::write(
                        request.cwd.join("uncertain-later.txt"),
                        "Later admitted effect requires inspection before retry\n",
                    )?;
                    bail!("Scripted later failure after the confirmed correction");
                } else {
                    ensure!(
                        is_task("Busy sibling") && request.profile.id == "b" && request.read_only,
                        "Unexpected execution or owner"
                    );
                }
            }
            let text = match request.purpose.as_str() {
                "plan" => json!({"summary":"Coordinate a captured correction before preserving it through a later failure","tasks":[
                    {"title":"Commit correction owner","description":"Accept temporary responsibility for the future correction","competence":"implementation","difficulty":"simple","access":"read_only","dependencies":[],"checks":[]},
                    {"title":"Revise correction approach","description":"Add a source-inspection approach without changing the trusted obligation","competence":"implementation","difficulty":"simple","access":"read_only","dependencies":[0],"checks":[]},
                    {"title":"Reassign correction owner","description":"Explicitly move the correction and sibling to another agent","competence":"implementation","difficulty":"simple","access":"read_only","dependencies":[1],"checks":[]},
                    {"title":"Busy sibling","description":"Inspect the captured source before the same owner executes the correction","competence":"verification","difficulty":"simple","access":"read_only","dependencies":[2],"checks":[]},
                    {"title":"Record observation","description":"Record O04, Hill, 2026-W36 completion percentage from the declared corrected input","competence":"implementation","difficulty":"simple","dependencies":[2],"checks":[]},
                    {"title":"Later failure","description":"Attempt a separate later result after the correction is accepted","competence":"implementation","difficulty":"simple","dependencies":[4],"checks":[]}
                ]}).to_string(),
                "review_plan"|"review"|"final_review" => json!({"approved":true,"reason":"Independently inspected the exact assigned result, captured correction obligation and actual check output"}).to_string(),
                "execute" => "Completed the assigned coordination or checked result".into(),
                _ => bail!("Unexpected phase {}",request.purpose),
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
async fn board_revision_reassignment_and_busy_owner_preserve_corrected_knowledge_after_failure() {
    let mut f = Fixture::new();
    let old = f.first().await;
    let original_source = old
        .provenance
        .as_ref()
        .unwrap()
        .source
        .as_ref()
        .unwrap()
        .clone();
    let original_acceptance = f
        .store
        .decisions(&old.source_session)
        .unwrap()
        .into_iter()
        .find(|d| d.id == original_source.acceptance_id)
        .unwrap();
    let mut third = f.engine.config.agents[0].clone();
    third.id = "c".into();
    third.name = "C".into();
    f.engine.config.agents.push(third);
    f.engine.config.team.push("c".into());
    f.engine.config.team_constraints.fixed_roster = Some(vec!["a".into(), "b".into(), "c".into()]);
    f.engine
        .config
        .execution
        .insert("c".into(), f.engine.config.execution["a"].clone());
    f.engine.config.limits.parallel = 2;
    let backend = Arc::new(CoordinatedCorrection {
        requests: Mutex::new(vec![]),
    });
    f.engine = f.engine.with_execution_backend(backend.clone()).unwrap();
    let contract = f.contract(60, Some(&old));
    let outcome = f.run(contract.clone()).await;
    assert_eq!(outcome.session.status, "blocked", "{}", outcome.summary);
    assert!(
        outcome
            .summary
            .contains("Scripted later failure after the confirmed correction"),
        "{}",
        outcome.summary
    );
    let trace = f.store.trace(&outcome.session.id).unwrap();
    let board = f.store.board(&outcome.session.id).unwrap();
    let corrected = board
        .tasks
        .iter()
        .find(|t| t.task.title == "Record observation")
        .unwrap();
    assert_eq!(corrected.task.state, TaskState::Accepted);
    assert_eq!(corrected.task.assignee.as_deref(), Some("b"));
    assert!(corrected.task.description.contains("Board approach:"));
    assert_eq!(
        corrected.task.checks,
        vec!["test -f inputs/observations-corrected.csv"]
    );
    let decisions = trace
        .decisions
        .iter()
        .filter_map(|d| d.links.board.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 4);
    assert!(decisions.iter().all(|d| d.accepted), "{decisions:?}");
    assert_eq!(
        decisions
            .iter()
            .filter_map(|d| d.commitment.as_ref())
            .filter(|c| c.task_id == corrected.task.id)
            .map(|c| c.agent_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "a", "b"]
    );
    assert!(
        trace.decisions.iter().any(|d| d
            .links
            .workspace_wait
            .as_ref()
            .is_some_and(|wait| wait.code == "commitment_busy")),
        "A temporarily busy committed correction must wait rather than lose its owner or block"
    );
    let captures = trace
        .decisions
        .iter()
        .filter_map(|d| d.links.acceptance_contract.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].contract, contract);
    assert_eq!(captures[0].version, captures[0].digest().unwrap());
    let acceptance = trace
        .decisions
        .iter()
        .find(|d| {
            d.kind == "task_accepted"
                && d.links
                    .task
                    .as_ref()
                    .is_some_and(|t| t.task_id == corrected.task.id)
        })
        .unwrap();
    assert_eq!(
        acceptance.outcome,
        Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Confirmed
        })
    );
    let result = acceptance.links.result.as_ref().unwrap();
    assert_eq!(
        result.task_definition.as_ref().unwrap().description,
        corrected.task.description
    );
    assert!(backend
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|request| request.purpose == "review"
            && request.prompt.contains("Task: Record observation\n")
            && request.prompt.contains("Board approach:")
            && request.prompt.contains(&old.id)
            && request.prompt.contains("source_replacement")));
    // This session has multiple retained outcomes; select the actual corrected source.
    let new = f
        .store
        .memory_inventory(old.project_id.as_deref())
        .unwrap()
        .into_iter()
        .find(|m| {
            m.provenance
                .as_ref()
                .and_then(|p| p.source.as_ref())
                .is_some_and(|s| s.acceptance_id == acceptance.id)
                && m.kind == "outcome"
        })
        .unwrap();
    assert_eq!(new.supersedes.as_deref(), Some(old.id.as_str()));
    assert_eq!(new.author, "b");
    assert!(new.content.contains("\"value\":60"));
    assert!(trace
        .assignments
        .iter()
        .all(|a| a.state != InvocationState::Running));
    assert!(trace
        .assignments
        .iter()
        .flat_map(|a| &a.grant_ids)
        .all(|id| f
            .store
            .team_grant(&outcome.session.id, id)
            .unwrap()
            .revoked_at
            .is_some()));
    assert!(f.directory.join("uncertain-later.txt").exists());
    let reopened = Store::open(&f.store.home).unwrap();
    assert_eq!(
        serde_json::to_value(
            reopened
                .decisions(&old.source_session)
                .unwrap()
                .into_iter()
                .find(|d| d.id == original_source.acceptance_id)
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(original_acceptance).unwrap()
    );
    let history = reopened
        .inspect_knowledge(old.project_id.as_deref(), &scope())
        .unwrap();
    assert_eq!(
        history
            .iter()
            .find(|r| r.id == old.id)
            .unwrap()
            .availability,
        KnowledgeAvailability::Superseded
    );
    assert_eq!(
        history
            .iter()
            .find(|r| r.id == new.id)
            .unwrap()
            .availability,
        KnowledgeAvailability::Available
    );
    assert_eq!(reopened.observations().unwrap().len(), 2);
    let observation = reopened
        .observations()
        .unwrap()
        .into_iter()
        .find(|o| o.evidence == acceptance.id)
        .unwrap();
    assert!(!reopened
        .observe_confirmed(&observation, &acceptance.id)
        .unwrap());
    let before = reopened.trace(&outcome.session.id).unwrap();
    assert!(f
        .engine
        .commit_board_proposals(&outcome.session.id)
        .unwrap()
        .is_empty());
    assert_eq!(
        reopened
            .trace(&outcome.session.id)
            .unwrap()
            .invocations
            .len(),
        before.invocations.len()
    );
    assert!(matches!(
        reopened
            .commit_knowledge_correction(
                &KnowledgeCorrectionProposal {
                    target: reference(&old),
                    acceptance_id: acceptance.id.clone()
                },
                &BoundKnowledgeCorrections.identity()
            )
            .unwrap(),
        KnowledgeCorrectionOutcome::AlreadyApplied { .. }
    ));
    f.engine.config.acceptance_contracts = None;
    f.backend.fail_plan.store(true, Ordering::SeqCst);
    f.engine = f.engine.with_execution_backend(f.backend.clone()).unwrap();
    let later = f
        .engine
        .run(&f.directory, "Find observation", None)
        .await
        .unwrap();
    let retrieved = reopened
        .trace(&later.session.id)
        .unwrap()
        .history
        .into_iter()
        .find(|e| e.kind == "memory_retrieval")
        .unwrap();
    assert!(retrieved.data["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == new.id));
    assert!(!retrieved.data["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == old.id));
    assert!(f
        .backend
        .requests
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .prompt
        .contains("\"value\":60"));
    assert_eq!(reopened.observations().unwrap().len(), 2);
}

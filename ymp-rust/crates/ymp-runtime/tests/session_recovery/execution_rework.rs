//! R4: public runtime continuation after known-ended read-only execution failures.
use super::*;

struct ExecutionFailures {
    writes: bool,
    calls: Mutex<Vec<TurnRequest>>,
    failed: Mutex<std::collections::HashSet<String>>,
}
impl ExecutionBackend for ExecutionFailures {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "review.execution-failures".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, _: &TurnRequest) -> WorkspaceAccess {
        if self.writes {
            WorkspaceAccess::WriteAll
        } else {
            WorkspaceAccess::ReadAll
        }
    }
    fn execute(
        &self,
        r: TurnRequest,
        _: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(r.clone());
            let current = r.prompt.rsplit("Your current assignment (").next().unwrap();
            let text = match r.purpose.as_str() {
                "plan" => json!({"summary":"Two failures and independent work","tasks":(0..5).map(|i|
                    json!({"title":format!("T{i}"),"description":format!("Return finding {i}"),"access":"read_only","competence":"analysis","difficulty":"simple",
                        "dependencies":match i {0=>vec![],1..=3=>vec![0],_=>vec![1,2]},"checks":[]})).collect::<Vec<_>>()} ).to_string(),
                "execute" => {
                    let id=(0..5).find(|i|current.contains(&format!("\nT{i}\n"))).unwrap();
                    if (id==1||id==2) && self.failed.lock().unwrap().insert(format!("T{id}")) {
                        return Err(std::io::Error::new(std::io::ErrorKind::ConnectionReset,"Scripted execute reset").into());
                    }
                    format!("Completed T{id}")
                },
                "review" if current.contains("Executor report: Execution was interrupted") || current.contains("Reviewer objection: Missing failed execution result") =>
                    json!({"approved":false,"reason":"Missing failed execution result"}).to_string(),
                "review_plan"|"review"|"final_review" =>json!({"approved":true,"reason":"Independent recorded result inspected"}).to_string(),
                "synthesis"=>"Completed".into(),
                p=>bail!("Unexpected {p}")
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
async fn r4_free_executor_progresses_before_failed_work_is_inspected_without_restart() -> Result<()>
{
    execution_failure_case(false).await
}

#[tokio::test]
async fn r4_unknown_write_effects_retain_responsibility_and_claim_busy() -> Result<()> {
    execution_failure_case(true).await
}

async fn execution_failure_case(writes: bool) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let store = Store::open(&temp.path().join("state"))?;
    let script = Arc::new(ExecutionFailures {
        writes,
        calls: Mutex::new(vec![]),
        failed: Mutex::new(Default::default()),
    });
    let (tx, _) = mpsc::unbounded_channel();
    let mut cfg = config();
    cfg.limits.parallel = 2;
    let mut provider = cfg.providers[0].clone();
    provider.id = "healthy".into();
    cfg.providers.push(provider);
    cfg.agents
        .iter_mut()
        .find(|a| a.id == "reserve")
        .unwrap()
        .provider = "healthy".into();
    let mut extra = cfg.agents[0].clone();
    extra.id = "fourth".into();
    extra.name = "fourth".into();
    extra.provider = "healthy".into();
    cfg.agents.push(extra);
    let mut engine = Engine::new(store.clone(), cfg, tx, CancellationToken::new())?
        .with_execution_backend(script.clone())?;
    engine.use_memory = false;
    let first = engine.run(&path, "Return five findings", None).await?;
    let trace = store.trace(&first.session.id)?;
    let calls = script.calls.lock().unwrap();
    let assignment = |r: &TurnRequest| {
        r.prompt
            .rsplit("Your current assignment (")
            .next()
            .unwrap()
            .to_owned()
    };
    let t3 = calls
        .iter()
        .position(|r| r.purpose == "execute" && assignment(r).contains("\nT3\n"));
    assert!(
        t3.is_some(),
        "Independent T3 must execute despite retained failed ownership: {}",
        first.summary
    );
    assert_eq!(calls[t3.unwrap()].provider.id, "healthy");
    if writes {
        assert_ne!(first.session.status, "completed");
        assert_eq!(
            trace
                .tasks
                .iter()
                .filter(|t| t.state == TaskState::Running)
                .count(),
            2
        );
        assert!(!trace
            .decisions
            .iter()
            .any(|d| d.kind == "execution_interruption_review_ready"));
        let failed = trace
            .tasks
            .iter()
            .find(|t| t.state == TaskState::Running)
            .unwrap();
        let pending = engine
            .board(&first.session.id)?
            .tasks
            .into_iter()
            .find(|t| t.task.title == "T4")
            .unwrap();
        let mut claim = pending.task.clone();
        claim.assign(
            failed.assignee.as_ref().unwrap(),
            &claim.dependencies.iter().cloned().collect(),
        )?;
        assert!(store
            .claim_board_task(
                &BoardTaskRef {
                    task_id: claim.id.clone(),
                    version: pending.version
                },
                &claim
            )
            .unwrap_err()
            .to_string()
            .contains("claim_busy"));
        assert_eq!(
            serde_json::to_value(store.tasks(&first.session.id)?)?,
            serde_json::to_value(&trace.tasks)?
        );
        return Ok(());
    }
    let first_inspection = calls.iter().position(|r| {
        r.purpose == "review"
            && assignment(r).contains("Executor report: Execution was interrupted")
    });
    assert!(
        first_inspection.is_some_and(|i| i > t3.unwrap()),
        "Free work must precede inspection of failed work"
    );
    assert_eq!(first.session.status, "completed", "{}", first.summary);
    assert_eq!(script.failed.lock().unwrap().len(), 2);
    assert!(trace.tasks.iter().all(|t| t.state == TaskState::Accepted));
    assert_eq!(trace.tasks.iter().filter(|t| t.attempts == 2).count(), 2);
    assert_eq!(calls.iter().filter(|r| r.purpose == "plan").count(), 1);
    assert_eq!(
        calls
            .iter()
            .filter(|r| r.purpose == "execute" && assignment(r).contains("\nT0\n"))
            .count(),
        1
    );
    let accepted = trace
        .decisions
        .iter()
        .filter(|d| d.kind == "task_accepted")
        .collect::<Vec<_>>();
    assert_eq!(accepted.len(), 5);
    assert!(accepted.iter().all(|d| d.outcome
        == Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Unconfirmed
        })));
    assert!(store.observations()?.is_empty());
    assert_eq!(trace.decisions.iter().filter(|d| d.kind == "execution_interruption_review_ready" && d.links.failure.is_some()).count(), 2);
    Ok(())
}

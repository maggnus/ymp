//! Offline walks through the public engine; every home and workspace is temporary.
use anyhow::{bail, Result};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};
use ymp_runtime::*;
use ymp_storage::Store;

#[derive(Clone, Copy)]
enum Mode {
    Unknown,
    Transient,
    Auth,
    Negative,
    Malformed,
}
struct Gate {
    purpose: &'static str,
    started: tokio::sync::Semaphore,
    release: tokio::sync::Semaphore,
}
struct Script {
    failure_purpose: &'static str,
    gate: Option<Arc<Gate>>,
    mode: Mode,
    writes: bool,
    failures: AtomicUsize,
    calls: Mutex<Vec<TurnRequest>>,
}
impl ExecutionBackend for Script {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "fixture.recovery".into(),
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
        request: TurnRequest,
        _: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(request.clone());
            if let Some(gate) = &self.gate {
                if request.purpose == gate.purpose {
                    gate.started.add_permits(1);
                    gate.release.acquire().await.unwrap().forget();
                }
            }
            if request.purpose == self.failure_purpose
                && self
                    .failures
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                    .is_ok()
            {
                if self.writes { std::fs::write(request.cwd.join("review-side-effect.txt"), "known local fixture effect")?; }
                match self.mode {
                    Mode::Unknown => bail!("Connection error."),
                    Mode::Transient => return Err(std::io::Error::new(std::io::ErrorKind::ConnectionReset, "Scripted transport reset").into()),
                    Mode::Auth => return Err(ymp_providers::NativeFailure { class:FailureClass::Authentication, code:Some("401".into()) }.into()),
                    Mode::Negative => return Ok(TurnResult { text:json!({"approved":false,"reason":"Keep this objection: missing analysis"}).to_string(), session_id:new_id(), usage:None }),
                    Mode::Malformed => return Ok(TurnResult { text:"incomplete {".into(), session_id:new_id(), usage:None }),
                }
            }
            let text = match request.purpose.as_str() {
                "plan" => json!({"summary":"Saved proposal", "tasks":[{"title":"Read", "description":"Return findings", "access":"read_only", "competence":"analysis", "difficulty":"simple", "dependencies":[], "checks":[]}]}).to_string(),
                "review_plan" | "review" | "final_review" => json!({"approved":true,"reason":"Independent evidence inspected"}).to_string(),
                "execute" => "Findings from this invocation".into(),
                "synthesis" => "Accepted findings".into(),
                p => bail!("Unexpected purpose {p}"),
            };
            let text = if request.purpose == "plan"
                && self.gate.as_ref().is_some_and(|g| g.purpose == "execute")
            {
                let mut plan: serde_json::Value = serde_json::from_str(&text)?;
                plan["tasks"].as_array_mut().unwrap().push(json!({"title":"Second read","description":"Use the first finding","access":"read_only","competence":"analysis","difficulty":"simple","dependencies":[0],"checks":[]}));
                plan.to_string()
            } else {
                text
            };
            Ok(TurnResult {
                text,
                session_id: new_id(),
                usage: None,
            })
        })
    }
}
fn config() -> Config {
    Config {
        providers: vec![ProviderConfig {
            id: "offline".into(),
            kind: ProviderKind::Mock,
            command: "internal".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        }],
        agents: ["author", "reviewer", "reserve"]
            .into_iter()
            .map(|id| AgentProfile {
                id: id.into(),
                name: id.into(),
                provider: "offline".into(),
                model: None,
                instructions: "Scripted fixture".into(),
                enabled: true,
            })
            .collect(),
        team: vec!["author".into(), "reviewer".into()],
        limits: Limits {
            parallel: 1,
            turns: 200,
            turn_timeout_secs: 10,
            attempts: 3,
            resources: Some(ResourceLimits {
                startup_invocations: 6,
                ..Default::default()
            }),
        },
        ..Default::default()
    }
}
fn engine(store: Store, script: Arc<Script>) -> Engine {
    let (events, _) = mpsc::unbounded_channel();
    let mut engine = Engine::new(store, config(), events, CancellationToken::new())
        .unwrap()
        .with_execution_backend(script)
        .unwrap();
    engine.use_memory = false;
    engine
}

fn reconstruct_legacy(source: &SessionTrace, path: &std::path::Path, legacy: &Store) -> Result<String> {
    let mut session = source.session.clone();
    session.project_id = legacy.project(path)?.id;
    session.turns_used = 0;
    legacy.create_session(&session, source.policy.as_ref().unwrap())?;
    legacy.put_value(&format!("team_state:v1:{}", session.id), &serde_json::to_value(source.team_state.as_ref().unwrap())?)?;
    legacy.put_value(&format!("prompt:{}", session.id), &json!("Inspect this directory"))?;
    for original in &source.invocations {
        let mut assignment = source.assignments.iter().find(|a| a.id == original.assignment_id).unwrap().clone();
        assignment.grant_ids.clear();
        assignment.state = InvocationState::Running;
        assignment.ended_at = None;
        let mut invocation = original.clone();
        invocation.state = InvocationState::Running;
        invocation.ended_at = None;
        invocation.terminal_reason = None;
        invocation.usage = None;
        legacy.begin_invocation(&assignment, &invocation)?;
        legacy.finish_invocation(&session.id, &invocation.id, original.state, Some(original.state.as_str()))?;
    }
    for d in source.decisions.iter().filter(|d| matches!(d.kind.as_str(),
        "plan_proposed" | "workspace_access_acquired" | "workspace_access_admitted" | "workspace_access_released")) {
        legacy.record_decision(d)?;
    }
    let plan = source.decisions.iter().find(|d| d.kind == "plan_proposed").unwrap().links.plan_proposal.as_ref().unwrap();
    legacy.invocation_message(&session.id, &plan.producer_invocation_id, "plan", &serde_json::to_string(&plan.plan)?)?;
    assert!(legacy.recovery_stages(&session.id)?.is_empty());
    Ok(session.id)
}

async fn legacy_case(writes: bool) -> Result<()> {
    let temp = tempfile::Builder::new().prefix(if writes {"legacy-write-"} else {"legacy-read-"})
        .tempdir_in("/Users/maggnus/.paseo/worktrees/1ms2ynax/ymp146-review-findings/ymp-docs/evidence/ymp-146/rework/independent-probe/candidate-output")?;
    let path = temp.path().join("work");
    std::fs::create_dir(&path)?;
    let initial = Store::open(&temp.path().join("source"))?;
    let failing = Arc::new(Script { failure_purpose:"review_plan", gate:None, mode:Mode::Unknown,
        writes, failures:AtomicUsize::new(1), calls:Mutex::new(vec![]) });
    let first = engine(initial.clone(), failing.clone()).run(&path, "Inspect this directory", None).await?;
    let source = initial.trace(&first.session.id)?;
    assert_eq!(source.invocations.len(), 2);
    assert!(source.tasks.is_empty());
    let proposal = source.decisions.iter().find(|d| d.kind == "plan_proposed").unwrap().links.plan_proposal.clone();
    let failed = source.assignments.iter().find(|a| a.purpose == "review_plan").unwrap().agent_id.clone();
    let legacy = Store::open(&temp.path().join("legacy"))?;
    let session = reconstruct_legacy(&source, &path, &legacy)?;
    // This backend performs no remote work or detached execution. Its optional local write has ended.
    if writes { assert_eq!(std::fs::read_to_string(path.join("review-side-effect.txt"))?, "known local fixture effect"); }
    let healthy = Arc::new(Script { failure_purpose:"never", gate:None, mode:Mode::Unknown,
        writes:false, failures:AtomicUsize::new(0), calls:Mutex::new(vec![]) });
    let e = engine(legacy.clone(), healthy.clone());
    let first_resume = e.run(&path, "", Some(&session)).await?;
    assert_ne!(first_resume.session.status, "completed");
    assert!(healthy.calls.lock().unwrap().is_empty());
    let stage = e.recovery_stages(&session)?.into_iter().find(|s| s.purpose == "review_plan").unwrap();
    eprintln!("LEGACY writes={writes} initial={:?} termination={:?} access={:?} actions={:?}",stage.status,
        stage.failures[0].termination,stage.failures[0].effective_access,stage.manual_actions().controls);
    for action in [RecoveryControl::Continue, RecoveryControl::Retry] {
        let stage = e.recovery_stages(&session)?.into_iter().find(|s| s.purpose == "review_plan").unwrap();
        let outcome = e.control_recovery(&RecoveryControlCommand { session_id:session.clone(),stage_id:stage.id,
            expected_revision:stage.revision,command_id:new_id(),action:action.clone() });
        eprintln!("LEGACY writes={writes} control={action:?} result={outcome:?}");
        if writes { assert!(outcome.unwrap_err().to_string().contains("uncertain_effects")); }
        else { outcome?; break; }
    }
    let view = e.team_control(&session)?;
    e.owner_team_command(&OwnerTeamCommand { session_id:session.clone(),expected_revision:view.revision,
        command_id:new_id(),revise_pinned_roster:false,
        action:OwnerTeamAction::Replace {agent_id:failed,replacement_id:"reserve".into()} })?;
    let view = e.team_control(&session)?;
    e.owner_team_command(&OwnerTeamCommand { session_id:session.clone(),expected_revision:view.revision,
        command_id:new_id(),revise_pinned_roster:false,action:OwnerTeamAction::Continue })?;
    let resumed = e.run(&path, "", Some(&session)).await?;
    let after = legacy.trace(&session)?;
    let calls = healthy.calls.lock().unwrap();
    assert!(!calls.iter().any(|r| r.purpose == "plan"), "Saved proposal must not be replanned");
    assert_eq!(after.decisions.iter().find(|d| d.kind == "plan_proposed").unwrap().links.plan_proposal, proposal);
    let report = json!({"writes":writes,"status":resumed.session.status,"summary":resumed.summary,
        "new_calls":calls.iter().map(|r| &r.purpose).collect::<Vec<_>>(),"stages":e.recovery_stages(&session)?,
        "task_count":after.tasks.len(),"invocation_count":after.invocations.len(),"proposal":proposal});
    std::fs::write(format!("/Users/maggnus/.paseo/worktrees/1ms2ynax/ymp146-review-findings/ymp-docs/evidence/ymp-146/rework/independent-probe/candidate-output/legacy-{writes}.json"),serde_json::to_string_pretty(&report)?)?;
    eprintln!("LEGACY writes={writes} final={} new_calls={} tasks={}",resumed.session.status,calls.len(),after.tasks.len());
    assert_eq!(resumed.session.status, "completed", "Missing public inspection-to-continuation transition: {}",resumed.summary);
    Ok(())
}

#[tokio::test]
async fn probe_legacy_read_only_positive_control() -> Result<()> { legacy_case(false).await }

#[tokio::test]
async fn probe_legacy_write_saved_plan_can_finish_after_replacement() -> Result<()> { legacy_case(true).await }

struct ExecutionFailures { calls:Mutex<Vec<TurnRequest>>, failed:Mutex<std::collections::HashSet<String>> }
impl ExecutionBackend for ExecutionFailures {
    fn identity(&self)->ExecutionBackendIdentity { ExecutionBackendIdentity{id:"review.execution-failures".into(),version:"1".into()} }
    fn workspace_access(&self,_:&TurnRequest)->WorkspaceAccess { WorkspaceAccess::ReadAll }
    fn execute(&self,r:TurnRequest,_:mpsc::UnboundedSender<ProviderEvent>)->ExecutionFuture<'_> {
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
            Ok(TurnResult{text,session_id:new_id(),usage:None})
        })
    }
}

#[tokio::test]
async fn probe_two_execute_failures_drain_unrelated_work_then_need_resume()->Result<()> {
    let temp=tempfile::Builder::new().prefix("execute-n-").tempdir_in("/Users/maggnus/.paseo/worktrees/1ms2ynax/ymp146-review-findings/ymp-docs/evidence/ymp-146/rework/independent-probe/candidate-output")?;
    let path=temp.path().join("work");std::fs::create_dir(&path)?;
    let store=Store::open(&temp.path().join("state"))?;
    let script=Arc::new(ExecutionFailures{calls:Mutex::new(vec![]),failed:Mutex::new(Default::default())});
    let (tx,_)=mpsc::unbounded_channel();let mut cfg=config();cfg.limits.parallel=2;
    let mut healthy_provider=cfg.providers[0].clone();healthy_provider.id="healthy".into();cfg.providers.push(healthy_provider);
    cfg.agents.iter_mut().find(|a|a.id=="reserve").unwrap().provider="healthy".into();
    let mut extra=cfg.agents[0].clone();extra.id="fourth".into();extra.name="fourth".into();extra.provider="healthy".into();cfg.agents.push(extra);
    let mut e=Engine::new(store.clone(),cfg,tx,CancellationToken::new())?.with_execution_backend(script.clone())?;e.use_memory=false;
    let first=e.run(&path,"Return five findings",None).await?;
    let before=store.trace(&first.session.id)?;
    let accepted=before.decisions.iter().filter(|d|d.kind=="task_accepted").cloned().collect::<Vec<_>>();
    eprintln!("EXECUTE first={} summary={} tasks={:?} failed={:?}",first.session.status,first.summary,before.tasks.iter().map(|t|(&t.title,t.state,t.attempts)).collect::<Vec<_>>(),script.failed.lock().unwrap());
    std::fs::write("/Users/maggnus/.paseo/worktrees/1ms2ynax/ymp146-review-findings/ymp-docs/evidence/ymp-146/rework/independent-probe/candidate-output/execute-n-first.json",serde_json::to_string_pretty(&json!({"summary":first.summary,"tasks":before.tasks,
        "team":before.team_state,"allocations":before.decisions.iter().filter_map(|d|d.links.allocation.as_ref()).collect::<Vec<_>>(),
        "calls":script.calls.lock().unwrap().iter().map(|r|json!({"agent":r.profile.id,"purpose":r.purpose})).collect::<Vec<_>>() }))?)?;
    assert_eq!(script.failed.lock().unwrap().len(),2);
    assert_ne!(first.session.status,"completed");
    let accepted_before_resume=accepted.len();
    assert_eq!(before.tasks.iter().filter(|t|t.state==TaskState::Running).count(),2,"Execute failures still need explicit resume inspection");
    assert!(!e.recovery_stages(&first.session.id)?.iter().any(|s|s.purpose=="execute"));
    assert!(!before.decisions.iter().filter_map(|d|d.links.recovery.as_ref()).any(|d|d.input.stage.purpose=="execute"));
    let second=e.run(&path,"",Some(&first.session.id)).await?;
    let after=store.trace(&first.session.id)?;
    let report=json!({"first_status":first.session.status,"first_tasks":before.tasks,"second_status":second.session.status,"summary":second.summary,"second_tasks":after.tasks,
        "calls":script.calls.lock().unwrap().iter().map(|r|json!({"agent":r.profile.id,"purpose":r.purpose})).collect::<Vec<_>>()});
    std::fs::write("/Users/maggnus/.paseo/worktrees/1ms2ynax/ymp146-review-findings/ymp-docs/evidence/ymp-146/rework/independent-probe/candidate-output/execute-n.json",serde_json::to_string_pretty(&report)?)?;
    assert_eq!(second.session.status,"completed","{}",second.summary);
    for d in accepted { assert!(after.decisions.iter().any(|a|serde_json::to_value(a).unwrap()==serde_json::to_value(&d).unwrap())); }
    assert_eq!(script.calls.lock().unwrap().iter().filter(|r|r.purpose=="plan").count(),1);
    assert_eq!(accepted_before_resume,2,"Independent T0 and T3 must be accepted before explicit resume");
    Ok(())
}

struct ReviewStages { revision_failure:bool, calls:Mutex<Vec<TurnRequest>>, plans:AtomicUsize, reviews:AtomicUsize }
impl ExecutionBackend for ReviewStages {
    fn identity(&self)->ExecutionBackendIdentity { ExecutionBackendIdentity{id:"review.saved-stages".into(),version:"1".into()} }
    fn workspace_access(&self,_:&TurnRequest)->WorkspaceAccess { WorkspaceAccess::ReadAll }
    fn execute(&self,r:TurnRequest,_:mpsc::UnboundedSender<ProviderEvent>)->ExecutionFuture<'_> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(r.clone());
            let text=match r.purpose.as_str() {
                "plan"=>{
                    let n=self.plans.fetch_add(1,Ordering::SeqCst);
                    if self.revision_failure && n==1 { return Err(std::io::Error::new(std::io::ErrorKind::ConnectionReset,"Revision transport reset").into()); }
                    json!({"summary":"Versioned findings","tasks":[{"title":"Read","description":"Return findings","access":"read_only","competence":"analysis","difficulty":"simple","dependencies":[],"checks":[]}]}).to_string()
                },
                "review_plan" if self.revision_failure && self.plans.load(Ordering::SeqCst)==1 =>json!({"approved":false,"reason":"Preserve this plan objection"}).to_string(),
                "review" if !self.revision_failure=>{
                    let n=self.reviews.fetch_add(1,Ordering::SeqCst);
                    match n {0=>json!({"approved":false,"reason":"Preserve this candidate objection"}).to_string(),1=>bail!("Connection error."),_=>json!({"approved":true,"reason":"Independent arbitration addressed the saved objection"}).to_string()}
                },
                "review_plan"|"review"|"final_review"=>json!({"approved":true,"reason":"Independent review complete"}).to_string(),
                "execute"=>"Read-only finding".into(),"synthesis"=>"Complete".into(),p=>bail!("Unexpected {p}")
            };
            Ok(TurnResult{text,session_id:new_id(),usage:None})
        })
    }
}

#[tokio::test]
async fn probe_revision_retry_and_arbitration_resume_preserve_objections()->Result<()> {
    for revision in [true,false] {
        let temp=tempfile::Builder::new().prefix("review-stages-").tempdir_in("/Users/maggnus/.paseo/worktrees/1ms2ynax/ymp146-review-findings/ymp-docs/evidence/ymp-146/rework/independent-probe/candidate-output")?;
        let path=temp.path().join("work");std::fs::create_dir(&path)?;
        let store=Store::open(&temp.path().join("state"))?;
        let backend=Arc::new(ReviewStages{revision_failure:revision,calls:Mutex::new(vec![]),plans:AtomicUsize::new(0),reviews:AtomicUsize::new(0)});
        let (tx,_)=mpsc::unbounded_channel();let mut e=Engine::new(store.clone(),config(),tx,CancellationToken::new())?.with_execution_backend(backend.clone())?;e.use_memory=false;
        let first=e.run(&path,"Inspect",None).await?;
        let first_trace=store.trace(&first.session.id)?;
        let result=if revision {first} else {
            assert_ne!(first.session.status,"completed");
            let stage=e.recovery_stages(&first.session.id)?.into_iter().find(|s|s.id.starts_with("candidate_arbitration-")).unwrap();
            assert_eq!(stage.failures[0].class,FailureClass::Unknown);
            e.control_recovery(&RecoveryControlCommand{session_id:first.session.id.clone(),stage_id:stage.id,expected_revision:stage.revision,command_id:new_id(),action:RecoveryControl::Continue})?;
            e.run(&path,"",Some(&first.session.id)).await?
        };
        let trace=store.trace(&result.session.id)?;
        eprintln!("REVIEW_STAGES revision={revision} status={} plan_calls={} review_calls={} summary={}",result.session.status,backend.plans.load(Ordering::SeqCst),backend.reviews.load(Ordering::SeqCst),result.summary);
        assert_eq!(result.session.status,"completed","{}",result.summary);
        assert_eq!(backend.plans.load(Ordering::SeqCst),if revision {3}else{1});
        for d in first_trace.decisions.iter().filter(|d|d.outcome==Some(DecisionOutcome::Rejected)) {
            assert!(trace.decisions.iter().any(|a|serde_json::to_value(a).unwrap()==serde_json::to_value(d).unwrap()));
        }
        if revision {
            let versions=trace.decisions.iter().filter(|d|d.kind=="plan_proposed").filter_map(|d|d.links.plan_proposal.as_ref()).collect::<Vec<_>>();
            assert_eq!(versions.len(),2);assert_eq!(versions[0].proposal_id,versions[1].proposal_id);assert_eq!(versions[1].revision,2);
        } else {
            assert_eq!(backend.calls.lock().unwrap().iter().filter(|r|r.purpose=="execute").count(),1);
            assert_eq!(trace.decisions.iter().filter(|d|d.kind=="candidate_review").count(),1);
            assert_eq!(trace.decisions.iter().filter(|d|d.kind=="candidate_arbitration").count(),1);
        }
    }
    Ok(())
}

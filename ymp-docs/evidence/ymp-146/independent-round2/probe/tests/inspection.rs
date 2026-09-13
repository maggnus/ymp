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

struct ScopedBackend { script: Arc<Script> }
impl ExecutionBackend for ScopedBackend {
    fn identity(&self)->ExecutionBackendIdentity { ExecutionBackendIdentity{id:"rereview.enforced-local".into(),version:"1".into()} }
    fn workspace_access(&self,r:&TurnRequest)->WorkspaceAccess {
        if r.purpose=="review_plan" {WorkspaceAccess::WriteAll} else {WorkspaceAccess::ReadAll}
    }
    fn local_effect_scope(&self,r:&TurnRequest)->Option<LocalEffectScope> {
        (r.purpose=="review_plan").then(||LocalEffectScope{files:vec!["effect.txt".into()]})
    }
    fn execute(&self,r:TurnRequest,events:mpsc::UnboundedSender<ProviderEvent>)->ExecutionFuture<'_> {
        Box::pin(async move {
            if r.purpose=="review_plan" {std::fs::write(r.cwd.join("effect.txt"),"complete local-only scope")?;}
            self.script.execute(r,events).await
        })
    }
}

async fn scoped_inspection_case(malformed:bool)->Result<()> {
    let temp=tempfile::Builder::new().prefix("scoped-inspection-").tempdir_in("/tmp/ymp146-recovery-round2-review")?;
    let path=temp.path().join("work");std::fs::create_dir(&path)?;
    let store=Store::open(&temp.path().join("state"))?;
    let failing=Arc::new(Script {failure_purpose:"review_plan",gate:None,mode:if malformed {Mode::Malformed}else{Mode::Unknown},writes:false,failures:AtomicUsize::new(1),calls:Mutex::new(vec![])});
    let initial=engine(store.clone(),failing.clone()).with_execution_backend(Arc::new(ScopedBackend{script:failing}))?
        .run(&path,"Inspect",None).await?;
    assert_ne!(initial.session.status,"completed");
    let stage=store.recovery_stages(&initial.session.id)?.into_iter().find(|s|s.purpose=="review_plan").unwrap();
    assert_eq!(stage.failures.len(),1);
    assert_eq!(stage.failures[0].termination,TerminationEvidence::BackendEnded);
    let original=store.trace(&initial.session.id)?;
    let access=original.decisions.iter().find(|d|d.kind=="workspace_access_admitted" && d.links.invocation_id.as_ref()==Some(&stage.failures[0].invocation_id)).unwrap().links.workspace_access.as_ref().unwrap();
    assert_eq!(access.local_effect_scope.as_ref().unwrap().files,vec![std::path::PathBuf::from("effect.txt")]);
    let invocation=store.invocation(&stage.session_id,&stage.failures[0].invocation_id)?;
    let healthy=Arc::new(Script{failure_purpose:"never",gate:None,mode:Mode::Unknown,writes:false,failures:AtomicUsize::new(0),calls:Mutex::new(vec![])});
    let e=engine(store.clone(),healthy.clone());
    let command=RecoveryInspectionCommand{session_id:stage.session_id.clone(),stage_id:stage.id.clone(),expected_revision:stage.revision,command_id:new_id()};
    let inspection=e.inspect_recovery(&command).await;
    eprintln!("SCOPED malformed={malformed} invocation={:?} class={:?} termination={:?} inspection={inspection:?}",invocation.state,stage.failures[0].class,stage.failures[0].termination);
    let report=json!({"malformed":malformed,"invocation_state":invocation.state,"failure":stage.failures[0],"scope":access.local_effect_scope,
        "inspection_error":inspection.as_ref().err().map(|e|e.to_string()),"inspection_calls":healthy.calls.lock().unwrap().len(),"stage":stage});
    std::fs::write(format!("/tmp/ymp146-recovery-round2-review/scoped-{malformed}.json"),serde_json::to_string_pretty(&report)?)?;
    let receipt=inspection?;
    e.control_recovery(&RecoveryControlCommand{session_id:stage.session_id.clone(),stage_id:stage.id,expected_revision:receipt.resulting_revision,command_id:new_id(),action:RecoveryControl::Continue})?;
    let result=e.run(&path,"",Some(&stage.session_id)).await?;
    assert_eq!(result.session.status,"completed","{}",result.summary);
    assert!(!healthy.calls.lock().unwrap().iter().any(|r|r.purpose=="plan"));
    assert!(store.observations()?.is_empty());
    Ok(())
}

#[tokio::test]
async fn scoped_failed_invocation_positive_control()->Result<()> {scoped_inspection_case(false).await}

#[tokio::test]
async fn scoped_completed_malformed_review_can_be_inspected()->Result<()> {scoped_inspection_case(true).await}

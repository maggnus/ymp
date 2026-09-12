#![cfg(test)]
use ymp_core::*;
use ymp_runtime::*;
use ymp_storage::Store;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use std::{path::{Path,PathBuf},sync::Arc};
struct Fixture { _temp: tempfile::TempDir, project: PathBuf, store: Store, engine: Engine, events: mpsc::UnboundedReceiver<UiEvent> }
fn fixture(marker: &str, contracted: bool) -> Fixture {
 let temp=tempfile::tempdir().unwrap(); let project=temp.path().join("project"); std::fs::create_dir(&project).unwrap();
 let store=Store::open(&temp.path().join("metadata")).unwrap();
 let config=Config { version:1, execution:Default::default(), capabilities:Default::default(), limits:Limits {parallel:2,turns:80,turn_timeout_secs:10,attempts:2,..Default::default()},providers:vec![ProviderConfig { id:"mock".into(),kind:ProviderKind::Mock,command:"internal".into(),args:vec![],env_refs:Default::default(),enabled:true}], agents:["one","two"].into_iter().map(|id|AgentProfile{id:id.into(),name:id.into(),provider:"mock".into(),model:None,instructions:format!("{id}{marker}"),enabled:true}).collect(), team:vec!["one".into(),"two".into()],..Default::default() };
 let (tx,events)=mpsc::unbounded_channel(); let mut engine=Engine::new(store.clone(),config,tx,CancellationToken::new()).unwrap(); engine.use_memory=false;
 if contracted {engine.acceptance_contracts.push(contract())}
 Fixture{_temp:temp,project,store,engine,events}
}
fn contract()->AcceptanceContract { AcceptanceContract{knowledge_correction:None,task_title:"Create a greeting".into(), criteria:vec![AcceptanceCriterion{id:"content".into(),description:"Exactly the requested greeting".into()}], artifacts:vec!["greeting.txt".into()],inputs:vec![],checks:vec![TrustedCheck{id:"content-v1".into(),criterion_ids:vec!["content".into()],assertion:CheckAssertion::ExactBytes{artifact:"greeting.txt".into(),expected:b"Hello from ymp\n".to_vec()}}]} }

struct DifferentChecker { mode: &'static str }
impl ConfirmationChecker for DifferentChecker {
 fn identity(&self)->CheckerIdentity{CheckerIdentity{id:"review.different-checker".into(),version:format!("1-{}",self.mode)}}
 fn execute<'a>(&'a self,_check:&'a TrustedCheck,directory:&'a Path,_artifacts:&'a [FileSnapshot],_inputs:&'a [FileSnapshot],_cancel:CancellationToken)->CheckFuture<'a>{Box::pin(async move{
  if self.mode=="mutate-input"{std::fs::write(directory.join("source.txt"),"changed input\n")?;}
  if self.mode=="mutate-artifact"{std::fs::write(directory.join("greeting.txt"),"changed artifact\n")?;}
  Ok(CheckExecution{exit_code:if self.mode=="unknown"{None}else{Some(0)},stdout:b"Different implementation raw result".to_vec(),stderr:vec![]})
 })}
}
#[tokio::test]
async fn alternative_checker_changes_execution_but_not_scope_and_freshness(){
 for mode in ["raw-success","unknown","mutate-input","mutate-artifact"] {
  let mut f=fixture("[mock:no-checks]",true);
  std::fs::write(f.project.join("source.txt"),b"Hello from ymp\n").unwrap();
  f.engine.acceptance_contracts[0].inputs.push("source.txt".into());
  f.engine.confirmation_checker=Arc::new(DifferentChecker{mode});
  let out=f.engine.run(&f.project,"Create a greeting",None).await.unwrap();
  let t=f.store.trace(&out.session.id).unwrap();let captured=t.decisions.iter().find_map(|d|d.links.acceptance_contract.as_ref()).unwrap();
  assert_eq!(captured.checker,f.engine.confirmation_checker.identity());
  let count=f.store.observations().unwrap().len();eprintln!("alternative mode={mode} status={} observations={count} check={:?}",out.session.status,t.decisions.iter().find_map(|d|d.links.check.as_ref()).map(|c|c.outcome));
  if mode=="raw-success"{assert_eq!(count,1);}else{assert_eq!(count,0);assert!(!t.decisions.iter().any(|d|matches!(d.outcome,Some(DecisionOutcome::Accepted{confirmation:ConfirmationStatus::Confirmed}))));}
 }
}

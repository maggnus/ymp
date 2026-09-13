//! Native adapter protocol fixtures and genuine historical records with no effect scope.
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_runtime::Engine;
use ymp_storage::Store;

struct Fixture {
    _temp: tempfile::TempDir,
    path: PathBuf,
    log: PathBuf,
    store: Store,
    engine: Engine,
    stage: RecoveryStage,
    original: SessionTrace,
}
impl Fixture {
    async fn new() -> Result<Self> {
        Self::configured(40, false).await
    }
    async fn configured(turns: usize, unknown: bool) -> Result<Self> {
        let temp = tempfile::Builder::new().prefix("native-independent-").tempdir_in("/tmp/ymp146-recovery-round2-review")?;
        let path = temp.path().join("work");
        std::fs::create_dir(&path)?;
        let log = temp.path().join("protocol.jsonl");
        let source = Store::open(&temp.path().join("source"))?;
        let script =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fresh_plan_review.py");
        let config: Config = serde_json::from_value(json!({
            "version":1,
            "providers":[
                {"id":"author-native","kind":"codex","command":"python3","args":[script,"codex","author",log,"native-v1"]},
                {"id":"failed-native","kind":"acp","command":"python3","args":[script,"acp","failed",log,"native-v1"]},
                {"id":"fresh-native","kind":"codex","command":"python3","args":[script,"codex","fresh",log,"native-v1"]}
            ],
            "agents":[{"id":"author","name":"Author","provider":"author-native"},{"id":"failed","name":"Failed","provider":"failed-native"},{"id":"fresh","name":"Fresh","provider":"fresh-native"}],
            "team":["author","failed"],"team_constraints":{"fixed_roster":["author","failed"]},"limits":{"parallel":1,"turns":turns,"turn_timeout_secs":5,"attempts":3,"resources":{"startup_invocations":10}}
        }))?;
        let make_engine = |store: Store| -> Result<Engine> {
            let (events, _) = mpsc::unbounded_channel();
            let mut engine = Engine::new(store, config.clone(), events, CancellationToken::new())?;
            engine.use_memory = false;
            Ok(engine)
        };
        let first = make_engine(source.clone())?
            .run(&path, "Inspect the directory", None)
            .await?;
        assert_ne!(first.session.status, "completed", "{}", first.summary);
        let original = source.trace(&first.session.id)?;
        assert_eq!(
            original.invocations.len(),
            2,
            "{}; calls={:?}; protocol={}",
            first.summary,
            original
                .assignments
                .iter()
                .map(|a| (&a.agent_id, &a.purpose))
                .collect::<Vec<_>>(),
            std::fs::read_to_string(&log)?
        );
        assert!(original.tasks.is_empty());
        let failed = original
            .assignments
            .iter()
            .find(|a| a.purpose == "review_plan")
            .unwrap();
        assert_eq!(failed.agent_id, "failed");
        let store = Store::open(&temp.path().join("legacy"))?;
        let mut session = original.session.clone();
        session.project_id = store.project(&path)?.id;
        session.turns_used = 0;
        store.create_session(&session, original.policy.as_ref().unwrap())?;
        store.put_value(
            &format!("team_state:v1:{}", session.id),
            &serde_json::to_value(&original.team_state)?,
        )?;
        store.put_value(
            &format!("prompt:{}", session.id),
            &json!("Inspect the directory"),
        )?;
        for invocation in &original.invocations {
            let mut assignment = original
                .assignments
                .iter()
                .find(|a| a.id == invocation.assignment_id)
                .unwrap()
                .clone();
            assignment.state = InvocationState::Running;
            assignment.ended_at = None;
            assignment.grant_ids.clear();
            let mut opened = invocation.clone();
            opened.state = InvocationState::Running;
            opened.ended_at = None;
            opened.usage = None;
            opened.terminal_reason = None;
            store.begin_invocation(&assignment, &opened)?;
            if let Some(usage) = invocation.usage.clone() {
                store.observe_invocation(&session.id, &opened.id, &InvocationObservation {
                    usage: Some(usage), ..Default::default()
                })?;
            }
            store.finish_invocation(
                &session.id,
                &opened.id,
                if unknown && assignment.purpose == "review_plan" {
                    InvocationState::Interrupted
                } else {
                    invocation.state
                },
                Some(invocation.state.as_str()),
            )?;
        }
        for record in original.decisions.iter().filter(|d| {
            matches!(
                d.kind.as_str(),
                "plan_proposed"
                    | "workspace_access_acquired"
                    | "workspace_access_admitted"
                    | "workspace_access_released"
            )
        }) {
            assert!(record
                .links
                .workspace_access
                .as_ref()
                .is_none_or(|a| a.local_effect_scope.is_none()));
            store.record_decision(record)?;
        }
        let plan = original
            .decisions
            .iter()
            .find_map(|d| {
                (d.kind == "plan_proposed")
                    .then_some(d.links.plan_proposal.as_ref())
                    .flatten()
            })
            .unwrap();
        store.invocation_message(
            &session.id,
            &plan.producer_invocation_id,
            "plan",
            &serde_json::to_string(&plan.plan)?,
        )?;
        let mut engine = make_engine(store.clone())?;
        ymp_providers::discovery::refresh_catalog(
            &mut engine.config,
            &temp.path().join("native-catalog"),
            &path,
            &PathBuf::new(),
            ymp_providers::discovery::ScanOptions {
                provider: Some("fresh-native".into()),
                timeout_secs: 5,
            },
            CancellationToken::new(),
        )
        .await?;
        let model = engine
            .config
            .native_provider_snapshot("fresh-native")
            .unwrap()
            .catalog
            .as_ref()
            .unwrap()
            .default_model
            .clone()
            .unwrap();
        engine
            .config
            .agents
            .iter_mut()
            .find(|a| a.id == "fresh")
            .unwrap()
            .model = Some(model);
        let before = std::fs::read(&log)?;
        assert_ne!(
            engine
                .run(&path, "", Some(&session.id))
                .await?
                .session
                .status,
            "completed"
        );
        assert_eq!(std::fs::read(&log)?, before);
        let stage = engine
            .recovery_stages(&session.id)?
            .into_iter()
            .find(|s| s.purpose == "review_plan")
            .unwrap();
        assert_eq!(
            stage.failures[0].effective_access,
            WorkspaceAccess::WriteAll
        );
        assert_eq!(
            stage.failures[0].termination,
            if unknown {
                TerminationEvidence::UnverifiedAfterRestart
            } else {
                TerminationEvidence::BackendEnded
            }
        );
        Ok(Self {
            _temp: temp,
            path,
            log,
            store,
            engine,
            stage,
            original,
        })
    }
    fn owner(&self, action: OwnerTeamAction) -> Result<()> {
        let view = self.engine.team_control(&self.stage.session_id)?;
        self.engine.owner_team_command(&OwnerTeamCommand {
            session_id: view.session_id,
            expected_revision: view.revision,
            command_id: new_id(),
            revise_pinned_roster: true,
            action,
        })?;
        Ok(())
    }
    fn command(&self) -> Result<FreshPlanReviewCommand> {
        let stage = self
            .engine
            .recovery_stages(&self.stage.session_id)?
            .into_iter()
            .find(|s| s.id == self.stage.id)
            .unwrap();
        Ok(FreshPlanReviewCommand {
            session_id: stage.session_id,
            stage_id: stage.id,
            expected_revision: stage.revision,
            command_id: new_id(),
            proposal: stage.plan.unwrap(),
            reviewer_id: "fresh".into(),
        })
    }
    async fn warm_fresh_context(&mut self) -> Result<()> {
        self.owner(OwnerTeamAction::Add {
            agent_id: "fresh".into(),
        })?;
        self.owner(OwnerTeamAction::Remove {
            agent_id: "failed".into(),
        })?;
        self.engine.config.team = vec!["fresh".into(), "author".into()];
        self.engine
            .follow_up(
                &self.path,
                "Give the current status only",
                &self.stage.session_id,
            )
            .await?;
        assert!(self
            .requests()?
            .iter()
            .any(|r| r["actor"] == "fresh" && r["purpose"] == "conversation"));
        Ok(())
    }
    fn control(&self, action: RecoveryControl) -> Result<RecoveryControlReceipt> {
        let command = self.command()?;
        self.engine.control_recovery(&RecoveryControlCommand {
            session_id: command.session_id,
            stage_id: command.stage_id,
            expected_revision: command.expected_revision,
            command_id: new_id(),
            action,
        })
    }
    fn requests(&self) -> Result<Vec<Value>> {
        std::fs::read_to_string(&self.log)?
            .lines()
            .map(|line| Ok(serde_json::from_str(line)?))
            .collect()
    }
}

fn reopen_independent(f:&Fixture)->Result<Engine> {
    let (tx,_)=mpsc::unbounded_channel();
    let mut e=Engine::new(Store::open(&f.store.home)?,f.engine.config.clone(),tx,CancellationToken::new())?;
    e.use_memory=false;Ok(e)
}

async fn current_files_case(negative:bool)->Result<()> {
    let mut f=Fixture::new().await?;
    f.warm_fresh_context().await?;
    let old=f.store.trace(&f.stage.session_id)?;
    let initial_plan=old.invocations.iter().find(|i|old.assignments.iter().any(|a|a.id==i.assignment_id&&a.purpose=="plan")).unwrap();
    let usage=initial_plan.usage.as_ref().unwrap();
    assert_eq!(usage.counts.input,Some(21));assert_eq!(usage.counts.output,Some(3));assert_eq!(usage.counts.cache_read,Some(7));
    assert!(old.usage.total.is_partial(),"The failed ACP invocation still has unknown usage");
    if negative {std::fs::write(f.log.parent().unwrap().join("control.json"),json!({"approved":false}).to_string())?;}
    f.control(RecoveryControl::Wait{condition:"Independent owner hold".into()})?;
    let calls=f.requests()?;
    assert!(f.engine.review_saved_plan_fresh(&f.command()?).await.is_err());
    assert_eq!(f.requests()?,calls);
    f.control(RecoveryControl::ReleaseHold)?;
    let stage=f.engine.recovery_stages(&f.stage.session_id)?.into_iter().find(|s|s.id==f.stage.id).unwrap();
    assert!(!stage.manual_permit);assert!(stage.effect_resolution.is_none());
    let command=f.command()?;
    let fresh=f.engine.review_saved_plan_fresh(&command).await?;
    assert_eq!(fresh.record.approved,!negative);
    let receipt_again=reopen_independent(&f)?.review_saved_plan_fresh(&command).await?;
    assert_eq!(fresh,receipt_again);
    let fresh_invocation=f.store.invocation(&f.stage.session_id,&fresh.record.response.invocation_id)?;
    assert!(fresh_invocation.resumed_from.is_none());
    if negative {
        assert!(f.engine.review_saved_plan_fresh(&f.command()?).await.is_err());
        std::fs::write(f.log.parent().unwrap().join("control.json"),json!({"approved":true}).to_string())?;
    }
    let context=f.engine.current_files_context(&f.stage.session_id,&f.stage.id)?;
    assert!(context.budget.observed_usage.is_partial());
    let command=ContinueWithCurrentFilesCommand{command_id:new_id(),context};
    let before=f.store.trace(&f.stage.session_id)?;
    let calls_before=f.requests()?;
    let authorization=f.engine.continue_with_current_files(&command)?;
    assert_eq!(f.requests()?,calls_before,"Authorization must not itself execute work");
    let e=reopen_independent(&f)?;
    assert_eq!(e.continue_with_current_files(&command)?,authorization);
    let result=e.run(&f.path,"",Some(&f.stage.session_id)).await?;
    let after=f.store.trace(&f.stage.session_id)?;
    let after_stage=e.recovery_stages(&f.stage.session_id)?.into_iter().find(|s|s.id==f.stage.id).unwrap();
    let plans=after.decisions.iter().filter(|d|d.kind=="plan_proposed").collect::<Vec<_>>();
    let verdicts=after.decisions.iter().filter(|d|d.kind=="plan_review").collect::<Vec<_>>();
    std::fs::write(format!("/tmp/ymp146-recovery-round2-review/native-negative-{negative}.json"),serde_json::to_string_pretty(&json!({
        "negative":negative,"status":result.session.status,"summary":result.summary,"plan_count":plans.len(),"review_count":verdicts.len(),"original_usage":initial_plan.usage,
        "after_usage":after.usage,"stage":after_stage,"protocol":f.requests()?,"authorization":authorization
    }))?)?;
    assert_eq!(result.session.status,"completed","{}",result.summary);
    assert!(f.path.join("current-result.txt").exists());
    assert_eq!(plans.len(),if negative {2}else{1});
    assert_eq!(verdicts.len(),if negative {2}else{1});
    if negative {
        assert_eq!(verdicts[0].outcome,Some(DecisionOutcome::Rejected));
        assert_eq!(plans[1].links.plan_proposal.as_ref().unwrap().revision,2);
        assert!(plans[1].links.review_ids.contains(&fresh.review_record_id));
        assert_ne!(verdicts[0].links.plan_proposal,verdicts[1].links.plan_proposal);
    }
    for old in &before.invocations {
        assert_eq!(serde_json::to_value(old)?,serde_json::to_value(f.store.invocation(&f.stage.session_id,&old.id)?)?);
    }
    assert_eq!(after.budget.as_ref().unwrap().limits,before.budget.as_ref().unwrap().limits);
    assert!(after.usage.total.is_partial());
    assert_eq!(after_stage.failures,f.stage.failures);assert!(!after_stage.manual_permit);assert!(after_stage.effect_resolution.is_none());
    assert!(after.decisions.iter().filter_map(|d|d.links.workspace_access.as_ref()).all(|a|a.local_effect_scope.is_none()));
    assert!(!f.requests()?[calls_before.len()..].iter().any(|r|r["actor"]=="failed"));
    assert!(f.store.observations()?.is_empty());
    eprintln!("INDEPENDENT negative={negative} completed; plans={}, verdicts={}, known historical input=21/output=3/cache=7 retained; failed ACP usage remains unknown",plans.len(),verdicts.len());
    Ok(())
}

#[tokio::test]
async fn independent_known_and_unknown_usage_survive_authorized_restart()->Result<()> {current_files_case(false).await}

#[tokio::test]
async fn independent_negative_verdict_requires_revision_even_after_owner_authorization()->Result<()> {current_files_case(true).await}

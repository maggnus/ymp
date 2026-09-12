use crate::{export, protocols, script::NativeJournal, workflows, write_json};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};
use ymp_runtime::Engine;
use ymp_storage::Store;
struct Partial {
    journal: Arc<NativeJournal>,
    store: Store,
    session: String,
    tokens: u64,
    requests: u64,
}
struct NativeStop {
    store: Store,
    session: String,
    journal: Arc<NativeJournal>,
    id: String,
}
impl Drop for NativeStop {
    fn drop(&mut self) {
        // A failed journal write is caught by the raw/durable source audit.
        let _ = export::record_native(
            &self.store,
            &self.session,
            &self.journal,
            json!({"type":"native_stopped","native_id":self.id}),
        );
    }
}
impl ExecutionBackend for Partial {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.evals.partial-timeout".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, _: &TurnRequest) -> WorkspaceAccess {
        WorkspaceAccess::ReadAll
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            let id = new_id();
            let _stop = NativeStop {
                store: self.store.clone(),
                session: self.session.clone(),
                journal: self.journal.clone(),
                id: id.clone(),
            };
            export::record_native(
                &self.store,
                &self.session,
                &self.journal,
                json!({"type":"native_started","native_id":id,"agent_id":request.profile.id,"controls":request.resource_controls,"timeout_seconds":request.timeout_secs}),
            )?;
            ensure!(
                request
                    .resource_controls
                    .max_turns
                    .is_some_and(|n| self.requests <= n),
                "Script exceeds actual native request controls"
            );
            events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
                sent: Some(request.settings.clone()),
                reported: Some(request.settings.clone()),
                native_session_id: Some(id.clone()),
                native_turn_id: Some(id.clone()),
                native_version: Some("scripted-partial-v1".into()),
                ..Default::default()
            })))?;
            for request in 0..self.requests {
                export::record_native(
                    &self.store,
                    &self.session,
                    &self.journal,
                    json!({"type":"native_request","native_id":id,"request_ordinal":request+1}),
                )?;
            }
            let usage = UsageSnapshot {
                counts: TokenCounts {
                    input: Some(self.tokens),
                    output: Some(0),
                    ..TokenCounts::zero()
                },
                finalized: false,
                partial: true,
                note: Some("Scripted partial native accounting before timeout".into()),
                native_total: None,
            };
            export::record_native(
                &self.store,
                &self.session,
                &self.journal,
                json!({"type":"native_usage","native_id":id,"usage":usage}),
            )?;
            events.send(ProviderEvent::Usage(usage))?;
            std::future::pending::<Result<TurnResult>>().await
        })
    }
}
pub async fn run(root: &Path, directory: &Path, spec: &Value) -> Result<Value> {
    let started = Instant::now();
    let work = directory.join("work");
    std::fs::create_dir(&work)?;
    let store = Store::open(&directory.join("metadata"))?;
    let mut config = workflows::config();
    config.limits.turn_timeout_secs = spec["setup"]["native_controls"]["timeout_seconds"]
        .as_u64()
        .unwrap();
    config.limits.resources.as_mut().unwrap().native_max_turns = spec["setup"]["native_controls"]
        ["max_turns"]
        .as_u64()
        .unwrap();
    let session = Session {
        id: new_id(),
        project_id: store.project(&work)?.id,
        title: "Inspect partial native usage without changing files".into(),
        status: "running".into(),
        created_at: now(),
        team: config.agents.clone(),
        turns_used: 0,
    };
    store.create_session(
        &session,
        &SessionPolicy {
            session_id: session.id.clone(),
            goal: session.title.clone(),
            constraints: None,
            cwd: work.canonicalize()?,
            limits: config.limits.clone(),
            eligible_pool: config.agents.clone(),
            captured_team: session.team.clone(),
            team_constraints: Some(config.team_constraints.clone()),
            execution: config.execution.clone(),
            assignment_settings: vec![],
            parent_session_id: None,
            evaluation: None,
            captured_at: now(),
        },
    )?;
    let actions = spec["script"].as_array().unwrap();
    let tokens = actions
        .iter()
        .find(|a| a["action"] == "native_usage")
        .unwrap()["tokens"]
        .as_u64()
        .unwrap();
    let requests = actions.iter().find(|a| a["action"] == "invoke").unwrap()["native_requests"]
        .as_u64()
        .unwrap();
    let journal = Arc::new(NativeJournal::default());
    let backend = Arc::new(Partial {
        store: store.clone(),
        session: session.id.clone(),
        journal: journal.clone(),
        tokens,
        requests,
    });
    let (events, _) = mpsc::unbounded_channel();
    let mut engine = Engine::new(store.clone(), config, events, CancellationToken::new())?
        .with_execution_backend(backend)?;
    engine.use_memory = false;
    let response = engine
        .follow_up(
            &work,
            "Inspect current context without changing files",
            &session.id,
        )
        .await;
    let budget = store.session_budget(&session.id)?.unwrap();
    let guarantee = budget.require_strict_token_bound();
    let boundary = json!({"operation":"require_strict_token_bound","accepted":guarantee.is_ok(),"error":guarantee.as_ref().err().map(|e|e.to_string()),"budget":budget});
    store.event(&session.id, "evaluation_boundary_observed", &boundary)?;
    let trace = store.trace(&session.id)?;
    write_json(&directory.join("runtime.json"), &trace)?;
    journal.save(directory)?;
    let aliases = trace
        .assignments
        .iter()
        .enumerate()
        .map(|(index, a)| (a.id.clone(), format!("a{}", index + 1)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let projected = project(&trace, &journal.snapshot(), &aliases)?;
    let active = trace
        .assignments
        .iter()
        .flat_map(|a| &a.grant_ids)
        .map(|id| store.team_grant(&session.id, id))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|g| g.revoked_at.is_none())
        .map(|g| aliases[&g.assignment_id].clone())
        .collect::<Vec<_>>();
    let observed = json!({"schema_version":1,"case_id":"partial-usage","events":projected.events,"final_state":{"reported_tokens":trace.usage.total.known_total(),"usage_coverage":if trace.usage.total.is_partial(){"partial"}else{"complete"},"cost":Value::Null,"strict_token_bound_claimed":budget.strict_token_bound,"active_grants":active}});
    write_json(&directory.join("runtime.json"), &trace)?;
    journal.save(directory)?;
    write_json(&directory.join("observed.json"), &observed)?;
    write_json(
        &directory.join("alias-map.json"),
        &json!({"assignments":aliases,"projection_sources":projected.sources,"ordering":"Durable native observations and runtime transitions share the Store event sequence"}),
    )?;
    write_json(
        &directory.join("boundary-actions.json"),
        &json!({"follow_up_error":response.err().map(|e|e.to_string()),"strict_guarantee":boundary}),
    )?;
    let metrics = export::metrics(&trace, started.elapsed().as_secs_f64(), 1)?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, "partial-usage").await?;
    Ok(
        json!({"case_id":"partial-usage","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}

pub(crate) fn project(
    trace: &SessionTrace,
    native: &[Value],
    aliases: &std::collections::BTreeMap<String, String>,
) -> Result<export::Projection> {
    let history = export::ordered_history([trace])?;
    export::audit_native_sources(&history, native)?;
    let mut projected = export::Projection::default();
    let mut lifecycle = export::LifecycleAudit::default();
    for event in history {
        if let Some(anomaly) = lifecycle.anomaly(event, trace, native, "native_stopped")? {
            projected.push(anomaly, export::event_source(event));
            continue;
        }
        let value = if event.kind == "evaluation_native_observed" {
            let id = event.data["native_id"]
                .as_str()
                .context("Missing native identity")?;
            if let Some(invocation) = trace
                .invocations
                .iter()
                .find(|i| i.native_session_id.as_deref() == Some(id))
            {
                let assignment = trace
                    .assignments
                    .iter()
                    .find(|a| a.id == invocation.assignment_id)
                    .context("Native invocation has no assignment")?;
                let alias = aliases.get(&assignment.id).unwrap_or(&assignment.id);
                match event.data["type"].as_str() {
                    Some("native_started") => Some(
                        json!({"type":"invocation_started","assignment":alias,"agent":assignment.agent_id,"native_max_turns":event.data["controls"]["max_turns"],"native_timeout_seconds":event.data["timeout_seconds"]}),
                    ),
                    Some("native_stopped") => Some(
                        json!({"type":"invocation_closed","assignment":alias,"agent":assignment.agent_id,"outcome":if invocation.terminal_reason.as_deref()==Some("timeout_limit"){"timeout"}else{invocation.state.as_str()},"tokens":invocation.usage.as_ref().and_then(|u|u.counts.known_total()),"coverage":export::coverage(invocation.usage.as_ref()),"cost":Value::Null}),
                    ),
                    Some("native_usage" | "native_request") => None,
                    _ => Some(json!({"type":"unexpected_native_event","native":event.data})),
                }
            } else {
                Some(json!({"type":"unattributed_native_event","native":event.data}))
            }
        } else if event.kind == "provenance" {
            match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
                ProvenanceEvent::GrantRevoked { grant } => Some(
                    json!({"type":"grant_revoked","assignment":aliases.get(&grant.assignment_id).unwrap_or(&grant.assignment_id)}),
                ),
                _ => None,
            }
        } else if event.kind == "evaluation_boundary_observed" {
            if event.data["operation"] == "require_strict_token_bound" {
                Some(
                    json!({"type":if event.data["accepted"]==true{"guarantee_accepted"}else{"guarantee_rejected"},"reason":event.data["error"].as_str().map(|e|e.split(':').next().unwrap())}),
                )
            } else {
                Some(json!({"type":"unexpected_boundary_event","observation":event.data}))
            }
        } else {
            None
        };
        if let Some(value) = value {
            let mut source = if event.kind == "evaluation_native_observed" {
                export::native_source(event, native)?
            } else {
                export::event_source(event)
            };
            if let Some(invocation) = trace.invocations.iter().find(|i| {
                i.native_session_id.as_deref() == event.data["native_id"].as_str()
                    && i.native_session_id.is_some()
            }) {
                source["invocation_id"] = json!(invocation.id);
            }
            projected.push(value, source);
        }
    }
    Ok(projected)
}

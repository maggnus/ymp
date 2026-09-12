use crate::{
    concurrency_protocol::{execute_reserved, HeldBackend},
    export, protocols,
    script::NativeJournal,
    workflows, write_json,
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Instant};
use tokio::sync::{mpsc, Notify};
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::TurnRequest;
use ymp_runtime::{mcp::TeamServer, Engine, WorkspaceAdmission};
use ymp_storage::Store;

pub async fn run(root: &Path, directory: &Path, spec: &Value) -> Result<Value> {
    let started = Instant::now();
    let work = directory.join("work");
    std::fs::create_dir(&work)?;
    let store = Store::open(&directory.join("metadata"))?;
    let gates = [("a".to_owned(), Arc::new(Notify::new()))]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let (notice, mut beginnings) = mpsc::unbounded_channel();
    let journal = Arc::new(NativeJournal::default());
    let tokens = spec["script"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["action"] == "finish")
        .unwrap()["tokens"]
        .as_u64()
        .unwrap();
    let backend = Arc::new(HeldBackend {
        gates: gates.clone(),
        started: notice,
        journal: journal.clone(),
        reported_tokens: tokens,
        reports_effort: false,
    });
    let mut engines = BTreeMap::new();
    let mut sessions = BTreeMap::new();
    let mut servers = BTreeMap::new();
    for (name, pins) in spec["setup"]["sessions"].as_object().unwrap() {
        let mut config = workflows::config();
        for agent in &mut config.agents {
            agent.model = Some("model-one".into());
        }
        for policy in config.execution.values_mut() {
            policy.fixed = ModelEffort {
                model: pins["fixed_model"].as_str().map(str::to_owned),
                effort: pins["fixed_effort"].as_str().map(str::to_owned),
            };
            policy.defaults.effort = Some("low".into());
        }
        config.capabilities.insert(
            "scripted".into(),
            ProviderCapabilities {
                source: CapabilitySource::Configured,
                models_complete: true,
                default_model: Some("model-one".into()),
                models: spec["setup"]["capabilities"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(model, efforts)| ModelCapabilities {
                        id: model.clone(),
                        controls: Some(vec![NativeControl {
                            display_name: None,
                            value_names: Default::default(),
                            id: "effort".into(),
                            values: NativeControlValues::Choices {
                                options: efforts
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .map(|e| e.as_str().unwrap().into())
                                    .collect(),
                            },
                            default: None,
                        }]),
                        ..Default::default()
                    })
                    .collect(),
            },
        );
        let session = Session {
            id: new_id(),
            project_id: store.project(&work)?.id,
            title: "Native effort admission protocol".into(),
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
        let (events, _) = mpsc::unbounded_channel();
        let server = Arc::new(TeamServer::start(store.clone(), &session, events.clone()).await?);
        let engine = Engine::new(store.clone(), config, events, CancellationToken::new())?
            .with_execution_backend(backend.clone())?;
        engines.insert(name.clone(), engine);
        servers.insert(name.clone(), server);
        sessions.insert(name.clone(), session);
    }
    let mut pending = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    let mut observed = Vec::new();
    let mut calls = Vec::new();
    let mut count = 0;
    for action in spec["script"].as_array().unwrap() {
        match action["action"].as_str().unwrap() {
            "request_invocation" => {
                let name = action["session"].as_str().unwrap();
                let engine = &engines[name];
                let session = &sessions[name];
                let agent = action["agent"].as_str().unwrap();
                let request = TurnRequest {
                    profile: engine.config.agent(agent)?.clone(),
                    provider: engine.config.providers[0].clone(),
                    settings: ExecutionSettings {
                        model: action["model"].as_str().map(str::to_owned),
                        effort: action["effort"].as_str().map(str::to_owned),
                        permission_mode: Some("read_only".into()),
                    },
                    cwd: work.clone(),
                    prompt: serde_json::to_string(&WorkspaceAccess::ReadAll)?,
                    purpose: "conversation".into(),
                    read_only: true,
                    resume: None,
                    usage_baseline: None,
                    mcp: None,
                    resource_controls: NativeResourceControls {
                        max_turns: Some(2),
                        max_output_chars: Some(2000),
                    },
                    timeout_secs: 10,
                    bridge: Path::new("").into(),
                };
                let before = store.trace(&session.id)?.invocations.len();
                match engine.try_reserve_workspace(&session.id, &request, None) {
                    Err(error) => {
                        let message = error.to_string();
                        let reason = if message.contains("violates fixed model") {
                            "fixed_model"
                        } else if message.contains("violates fixed effort") {
                            "fixed_effort"
                        } else if message.contains("unsupported_effort") {
                            "unsupported_effort"
                        } else {
                            message.as_str()
                        };
                        observed.push(json!({"type":"admission_denied","reason":reason,"model":request.settings.model,"effort":request.settings.effort,"session":name}));
                        calls.push(json!({"request":action,"error":message,"invocations_before":before,"invocations_after":store.trace(&session.id)?.invocations.len()}));
                    }
                    Ok(WorkspaceAdmission::Deferred(wait)) => {
                        observed.push(
                            json!({"type":"admission_deferred","reason":wait.code,"session":name}),
                        );
                        calls.push(json!({"request":action,"deferred":wait}));
                    }
                    Ok(WorkspaceAdmission::Acquired(lease)) => {
                        count += 1;
                        let alias = format!("{agent}{count}");
                        let assignment_id = lease.assignment_id().to_owned();
                        aliases.insert(alias.clone(), assignment_id.clone());
                        let mut handle = tokio::spawn(execute_reserved(
                            store.clone(),
                            servers[name].clone(),
                            backend.clone(),
                            lease,
                            request,
                            session.id.clone(),
                        ));
                        tokio::select! {notice=beginnings.recv()=>ensure!(notice.as_deref()==Some(agent),"Unexpected native start"),result=&mut handle=>{result??;anyhow::bail!("Native request ended before its scripted barrier")}}
                        // Native observation delivery and durable capture are asynchronous;
                        // exporting starts after finish below reads its actual final fields.
                        observed.push(json!({"pending_started_assignment":assignment_id,"alias":alias,"session":name}));
                        pending.insert(alias, (name.to_owned(), agent.to_owned(), handle));
                        calls.push(json!({"request":action,"assignment_id":assignment_id,"invocations_before":before}));
                    }
                }
            }
            "finish" => {
                let alias = action["assignment"].as_str().unwrap();
                let (name, agent, handle) = pending
                    .remove(alias)
                    .context("Missing requested invocation")?;
                gates[&agent].notify_one();
                handle.await??;
                let trace = store.trace(&sessions[&name].id)?;
                let invocation = trace
                    .invocations
                    .iter()
                    .find(|i| i.assignment_id == aliases[alias])
                    .context("Missing actual invocation")?;
                observed.push(json!({"type":"invocation_closed","assignment":alias,"agent":agent,"tokens":invocation.usage.as_ref().and_then(|u|u.counts.known_total()),"coverage":if invocation.usage.as_ref().is_some_and(|u|u.finalized&&!u.partial){"complete"}else{"partial"}}));
            }
            action => anyhow::bail!("Unknown effort action {action}"),
        }
    }
    for (alias, (name, agent, handle)) in pending {
        gates[&agent].notify_one();
        handle.await??;
        observed
            .push(json!({"type":"unexpected_invocation_closed","assignment":alias,"session":name}));
    }
    let traces = sessions
        .iter()
        .map(|(name, session)| Ok((name.clone(), store.trace(&session.id)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    for row in &mut observed {
        if let Some(id) = row["pending_started_assignment"].as_str() {
            let name = row["session"].as_str().unwrap();
            let trace = &traces[name];
            let invocation = trace
                .invocations
                .iter()
                .find(|i| i.assignment_id == id)
                .unwrap();
            let assignment = trace.assignments.iter().find(|a| a.id == id).unwrap();
            *row = json!({"type":"invocation_started","assignment":row["alias"],"agent":assignment.agent_id,"model":invocation.requested.model,"requested_effort":invocation.requested.effort,"sent_effort":invocation.sent.effort,"reported_effort":invocation.reported.effort,"session":name});
        }
    }
    let calls_count = traces.values().map(|t| t.invocations.len()).sum::<usize>();
    let known_effort = traces
        .values()
        .flat_map(|t| &t.invocations)
        .any(|i| i.reported.effort.is_some());
    let observed = json!({"schema_version":1,"case_id":"effort-support","events":observed,"final_state":{"invocations":calls_count,"model_calls_for_discovery":traces.values().flat_map(|t|&t.assignments).filter(|a|a.purpose=="discovery").count(),"effort_verified_from_native_report":known_effort}});
    write_json(&directory.join("observed.json"), &observed)?;
    write_json(&directory.join("runtime.json"), &traces)?;
    journal.save(directory)?;
    write_json(&directory.join("boundary-actions.json"), &calls)?;
    write_json(
        &directory.join("alias-map.json"),
        &json!({"assignments":aliases,"sessions":sessions.iter().map(|(name,s)|(name.clone(),s.id.clone())).collect::<BTreeMap<_,_>>()}),
    )?;
    let metrics = traces
        .iter()
        .map(|(name, trace)| {
            Ok((
                name.clone(),
                export::metrics(trace, started.elapsed().as_secs_f64(), 1)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, "effort-support").await?;
    Ok(
        json!({"case_id":"effort-support","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}

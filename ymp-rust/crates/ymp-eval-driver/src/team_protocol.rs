use crate::{export, protocols, workflows, write_json};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_runtime::{AllocationPolicy, Engine};
use ymp_storage::Store;

struct ProposedMembers(Mutex<(Vec<String>, String)>);
impl AllocationPolicy for ProposedMembers {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.evals.explicit-members".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, input: &AllocationInput) -> Result<AllocationProposal> {
        let (members, rationale) = self.0.lock().unwrap().clone();
        Ok(AllocationProposal {
            members,
            executor: None,
            reserved_final_reviewer: input
                .current
                .as_ref()
                .and_then(|s| s.reserved_final_reviewer.clone())
                .or_else(|| input.eligible.last().map(|a| a.id.clone())),
            method: "scripted_boundary_probe".into(),
            rationale,
        })
    }
}
fn identities(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().into())
        .collect()
}
pub async fn run(root: &Path, directory: &Path, case: &str, spec: &Value) -> Result<Value> {
    let started = Instant::now();
    let work = directory.join("work");
    std::fs::create_dir(&work)?;
    let store = Store::open(&directory.join("metadata"))?;
    let project = store.project(&work)?;
    let mut config = workflows::config();
    let pool = identities(&spec["setup"]["pool"]);
    config.agents = pool
        .iter()
        .map(|id| {
            let mut agent = config.agents[0].clone();
            agent.id = id.clone();
            agent.name = format!("Scripted {id}");
            agent
        })
        .collect();
    let initial = identities(&spec["setup"]["initial_team"]);
    config.team = initial.clone();
    config.team_constraints.fixed_roster = spec["setup"]["fixed_roster"]
        .as_array()
        .map(|_| identities(&spec["setup"]["fixed_roster"]));
    config.team_constraints.fixed_size = spec["setup"]["fixed_size"].as_u64().map(|v| v as usize);
    let session = Session {
        id: new_id(),
        project_id: project.id,
        title: spec["purpose"].as_str().unwrap().into(),
        status: "running".into(),
        created_at: now(),
        team: initial
            .iter()
            .map(|id| config.agents.iter().find(|a| &a.id == id).unwrap().clone())
            .collect(),
        turns_used: 0,
    };
    let policy = SessionPolicy {
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
        evaluation: Some(EvaluationReference {
            run_id: directory.parent().unwrap().display().to_string(),
            scenario_id: case.into(),
            revision: Some("1".into()),
        }),
        captured_at: now(),
    };
    let proposed = Arc::new(ProposedMembers(Mutex::new((
        initial,
        "initial_capture".into(),
    ))));
    let (events, _) = mpsc::unbounded_channel();
    let engine = Engine::new(store.clone(), config, events, CancellationToken::new())?
        .with_allocation_policy(proposed.clone())?;
    let demand = AllocationDemand {
        purpose: "execute".into(),
        task_id: None,
        competence: "implementation".into(),
        difficulty: "simple".into(),
        risk: TaskRisk::Standard,
        ready_work: 0,
    };
    let mut action_results = Vec::new();
    for action in spec["script"].as_array().unwrap() {
        let result = match action["action"].as_str().unwrap() {
            "start_session" => {
                store.create_session(&session, &policy)?;
                engine
                    .reconsider_allocation(&session.id, AllocationBoundary::Startup, demand.clone())
                    .map(|v| json!(v))
            }
            "propose_replace" | "propose_add" => {
                let state = store
                    .team_state(&session.id)?
                    .context("Missing initial membership")?;
                let mut members = state.current_members;
                let reason = if action["action"] == "propose_replace" {
                    members.retain(|id| Some(id.as_str()) != action["departing"].as_str());
                    members.push(action["incoming"].as_str().unwrap().into());
                    "replacement_at_boundary"
                } else {
                    members.push(action["agent"].as_str().unwrap().into());
                    "proposed_addition"
                };
                *proposed.0.lock().unwrap() = (members, reason.into());
                engine
                    .reconsider_allocation(
                        &session.id,
                        AllocationBoundary::GoalChanged,
                        demand.clone(),
                    )
                    .map(|v| json!(v))
            }
            "no_ready_work" => {
                *proposed.0.lock().unwrap() = (
                    store.team_state(&session.id)?.unwrap().current_members,
                    "no_ready_work".into(),
                );
                engine
                    .reconsider_allocation(
                        &session.id,
                        AllocationBoundary::WorkReady,
                        demand.clone(),
                    )
                    .map(|v| json!(v))
            }
            action => anyhow::bail!("Unsupported team boundary action {action}"),
        };
        action_results.push(json!({"action":action,"result":result.as_ref().ok(),"error":result.err().map(|e|e.to_string())}));
    }
    let trace = store.trace(&session.id)?;
    let mut observed = Vec::new();
    let mut sources = Vec::new();
    for event in &trace.history {
        if event.kind != "provenance" {
            continue;
        }
        let parsed: ProvenanceEvent = serde_json::from_value(event.data.clone())?;
        let projection = match parsed {
            ProvenanceEvent::SessionCaptured { policy } => Some(
                json!({"type":"team_captured","session":"s1","members":policy.captured_team.iter().map(|a|a.id.clone()).collect::<Vec<_>>()}),
            ),
            ProvenanceEvent::DecisionRecorded { decision } => {
                if let Some(allocation) = decision.links.allocation {
                    if !allocation.accepted {
                        let prior = allocation
                            .input
                            .current
                            .as_ref()
                            .map(|s| s.current_members.as_slice())
                            .unwrap_or_default();
                        let agent = allocation
                            .proposal
                            .members
                            .iter()
                            .find(|id| !prior.contains(id))
                            .cloned();
                        Some(
                            json!({"type":"proposal_rejected","reason":allocation.reason.split(':').next().unwrap_or(&allocation.reason),"agent":agent}),
                        )
                    } else if allocation
                        .input
                        .current
                        .as_ref()
                        .is_some_and(|s| s.current_members != allocation.proposal.members)
                    {
                        Some(
                            json!({"type":"team_changed","session":"s1","members":allocation.proposal.members,"reason":allocation.proposal.rationale}),
                        )
                    } else if allocation.input.boundary == AllocationBoundary::WorkReady
                        && allocation.input.demand.ready_work == 0
                        && allocation.proposal.executor.is_none()
                    {
                        Some(json!({"type":"admission_idle","reason":"no_ready_work"}))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(projection) = projection {
            sources.push(json!({"projected_index":observed.len(),"runtime_event_seq":event.seq}));
            observed.push(projection);
        }
    }
    let state = trace
        .team_state
        .as_ref()
        .context("Missing actual final membership")?;
    let observed = json!({"schema_version":1,"case_id":case,"events":observed,"final_state":{"members":state.current_members,"historical_members":trace.session.team.iter().map(|a|a.id.clone()).collect::<Vec<_>>(),"invocations":trace.invocations.len()}});
    write_json(&directory.join("runtime.json"), &trace)?;
    write_json(&directory.join("native.json"), &json!([]))?;
    let mut aliases = export::aliases(&trace);
    aliases["projection_sources"] = json!(sources);
    write_json(&directory.join("alias-map.json"), &aliases)?;
    write_json(&directory.join("boundary-actions.json"), &action_results)?;
    write_json(&directory.join("observed.json"), &observed)?;
    let metrics = export::metrics(&trace, started.elapsed().as_secs_f64(), 0)?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, case).await?;
    Ok(
        json!({"case_id":case,"complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}

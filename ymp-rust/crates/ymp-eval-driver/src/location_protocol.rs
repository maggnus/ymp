use crate::{export, protocols, read_json, workflows, write_json};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, time::Instant};
use ymp_core::*;
use ymp_storage::Store;

pub async fn run(root: &Path, directory: &Path) -> Result<Value> {
    let started = Instant::now();
    // The document runner performs production followed by its real zero-call
    // follow-up, exactly the first two actions in this protocol.
    let spec = read_json(&root.join("ymp-evals/scenarios/universal-workflows.json"))?;
    let completed = workflows::run(
        root,
        directory,
        "document",
        serde_json::from_value(spec["workflows"]["document"].clone())?,
    )
    .await?;
    ensure!(
        completed["complete"] == true,
        "Document seed did not receive actual confirmed acceptance"
    );
    std::fs::copy(
        directory.join("validator.json"),
        directory.join("workflow-validator.json"),
    )?;
    let receipt = read_json(&directory.join("workflow.json"))?;
    let store = Store::open(&directory.join("metadata"))?;
    let source = receipt["session_id"]
        .as_str()
        .context("Missing actual source session")?;
    let trace = store.trace(source)?;
    let outcomes = store.outcomes(source)?;
    let stored = outcomes
        .iter()
        .find(|outcome| {
            Some(outcome.result_id.as_str()) == receipt["artifact"]["result_id"].as_str()
        })
        .context("Missing actual stored outcome")?;
    let artifact = stored
        .artifacts
        .first()
        .context("Stored outcome lacks a path")?;
    let current = store.session(source)?;
    let mut later = current.clone();
    later.id = new_id();
    later.title = "Locate the previously accepted workshop artifact".into();
    later.status = "running".into();
    later.created_at = now();
    later.turns_used = 0;
    let mut policy = store
        .session_policy(source)?
        .context("Missing source policy")?;
    policy.session_id = later.id.clone();
    policy.goal = later.title.clone();
    policy.captured_at = now();
    policy.parent_session_id = None;
    store.create_session(&later, &policy)?;
    let before = store.session_usage(&later.id)?.total.calls;
    let retrieved = store.outcomes(source)?;
    let after = store.session_usage(&later.id)?.total.calls;
    let retrieval = json!({"operation":"Store::outcomes","requesting_session":later.id,"source_session":source,"returned":retrieved,"invocations_before":before,"invocations_after":after});
    store.event(&later.id, "evaluation_boundary_observed", &retrieval)?;
    write_json(
        &directory.join("boundary-actions.json"),
        &json!([retrieval]),
    )?;
    let later_trace = store.trace(&later.id)?;
    write_json(&directory.join("runtime-later.json"), &later_trace)?;
    let sessions = [
        (source.to_owned(), "s1".to_owned()),
        (later.id.clone(), "s2".to_owned()),
    ]
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let results = [(stored.result_id.clone(), "r1".to_owned())]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let projected = project(&[&trace, &later_trace], &outcomes, &sessions, &results)?;
    let observed = json!({"schema_version":1,"case_id":"location-retrieval","events":projected.events,"final_state":{"production_runs":([&trace,&later_trace].iter().flat_map(|t|&t.assignments).filter(|a|a.purpose=="execute").count()),"follow_up_invocations":receipt["follow_up"]["invocations_started"],"working_directory":stored.directory,"is_git_worktree":stored.directory.ancestors().any(|p|p.join(".git").exists()),"artifact_path":artifact.path}});
    write_json(&directory.join("observed.json"), &observed)?;
    let mut aliases = export::aliases(&trace);
    aliases["aliases"]["s2"] = json!(later.id);
    aliases["aliases"]["r1"] = json!(stored.result_id);
    aliases["projection_sources"] = json!(projected.sources);
    aliases["ordering"] =
        json!("Actual acceptance, answer and metadata-query observation sequence");
    aliases["later_session_records"] = export::aliases(&later_trace);
    write_json(&directory.join("alias-map.json"), &aliases)?;
    let metrics = json!({"s1":export::metrics(&trace, started.elapsed().as_secs_f64(),1)?,"s2":export::metrics(&later_trace, started.elapsed().as_secs_f64(),0)?});
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, "location-retrieval").await?;
    Ok(
        json!({"case_id":"location-retrieval","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}

pub(crate) fn project(
    traces: &[&SessionTrace],
    outcomes: &[StoredOutcome],
    sessions: &BTreeMap<String, String>,
    results: &BTreeMap<String, String>,
) -> Result<export::Projection> {
    let history = export::ordered_history(traces.iter().copied())?;
    let alias = |map: &BTreeMap<String, String>, id: &str| {
        map.get(id).cloned().unwrap_or_else(|| id.into())
    };
    let mut projected = export::Projection::default();
    let mut calls = BTreeMap::<String, u64>::new();
    let mut user_calls = BTreeMap::<String, u64>::new();
    for event in history {
        if event.kind == "provenance" {
            match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
                ProvenanceEvent::AssignmentStarted { .. } => {
                    *calls.entry(event.session_id.clone()).or_default() += 1;
                }
                ProvenanceEvent::DecisionRecorded { decision }
                    if decision.kind == "task_accepted" =>
                {
                    let observed = outcomes
                        .iter()
                        .filter(|o| o.acceptance_id == decision.id)
                        .collect::<Vec<_>>();
                    ensure!(
                        observed.len() == 1,
                        "Accepted outcome has missing or ambiguous actual metadata"
                    );
                    for artifact in &observed[0].artifacts {
                        let mut source = export::event_source(event);
                        source["acceptance_id"] = json!(decision.id);
                        source["artifact_path"] = json!(artifact.path);
                        projected.push(json!({"type":"outcome_stored","session":alias(sessions,&event.session_id),"result":alias(results,&observed[0].result_id),"path":artifact.path,"sha256":artifact.sha256}),source);
                    }
                }
                _ => {}
            }
        } else if event.kind == "message" {
            if event.data["kind"] == "user" {
                user_calls.insert(
                    event.session_id.clone(),
                    *calls.get(&event.session_id).unwrap_or(&0),
                );
            } else if event.data["kind"] == "answer" {
                let text = event.data["text"].as_str().context("Answer text missing")?;
                let mut paths = outcomes
                    .iter()
                    .filter(|o| o.source_session == event.session_id)
                    .flat_map(|o| o.artifacts.iter().map(|a| a.path.display().to_string()))
                    .collect::<Vec<_>>();
                paths.sort();
                paths.dedup();
                if text != format!("Recorded output paths: {}", paths.join(", "))
                    || paths.is_empty()
                {
                    projected.push(json!({"type":"unresolved_location_answer","session":alias(sessions,&event.session_id),"text":text}),export::event_source(event));
                    continue;
                }
                let count = calls
                    .get(&event.session_id)
                    .copied()
                    .unwrap_or(0)
                    .checked_sub(
                        *user_calls
                            .get(&event.session_id)
                            .context("Answer has no observed requesting message")?,
                    )
                    .context("Invalid invocation ordering")?;
                for path in paths {
                    let matching = outcomes
                        .iter()
                        .filter(|o| {
                            o.source_session == event.session_id
                                && o.artifacts
                                    .iter()
                                    .any(|a| a.path.display().to_string() == path)
                        })
                        .collect::<Vec<_>>();
                    ensure!(
                        matching.len() == 1,
                        "Location answer cannot identify one actual result for its path"
                    );
                    let mut source = export::event_source(event);
                    source["message_seq"] = event.data["seq"].clone();
                    source["acceptance_id"] = json!(matching[0].acceptance_id);
                    projected.push(json!({"type":"location_answered","session":alias(sessions,&event.session_id),"result":alias(results,&matching[0].result_id),"path":path,"invocations_started":count}),source);
                }
            }
        } else if event.kind == "evaluation_boundary_observed" {
            if event.data["operation"] != "Store::outcomes" {
                projected.push(
                    json!({"type":"unexpected_boundary_event","observation":event.data}),
                    export::event_source(event),
                );
                continue;
            }
            let returned: Vec<StoredOutcome> =
                serde_json::from_value(event.data["returned"].clone())?;
            let count = event.data["invocations_after"]
                .as_u64()
                .context("Missing actual invocation count")?
                .checked_sub(
                    event.data["invocations_before"]
                        .as_u64()
                        .context("Missing actual invocation count")?,
                )
                .context("Invocation count decreased")?;
            for (index, outcome) in returned.iter().enumerate() {
                for artifact in &outcome.artifacts {
                    let mut source = export::event_source(event);
                    source["returned_index"] = json!(index);
                    source["acceptance_id"] = json!(outcome.acceptance_id);
                    projected.push(json!({"type":"outcome_retrieved","session":alias(sessions,&event.session_id),"source_session":alias(sessions,&outcome.source_session),"result":alias(results,&outcome.result_id),"path":artifact.path,"sha256":artifact.sha256,"invocations_started":count}),source);
                }
            }
        }
    }
    Ok(projected)
}

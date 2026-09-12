use crate::{export, protocols, read_json, workflows, write_json};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{path::Path, time::Instant};
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
    let stored = store
        .outcomes(source)?
        .into_iter()
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
    let retrieved = store
        .outcomes(source)?
        .into_iter()
        .find(|outcome| outcome.result_id == stored.result_id)
        .context("Later metadata query could not retrieve the source result")?;
    let after = store.session_usage(&later.id)?.total.calls;
    let retrieval = json!({"operation":"Store::outcomes","requesting_session":later.id,"source_session":source,"returned":retrieved,"invocations_before":before,"invocations_after":after});
    write_json(
        &directory.join("boundary-actions.json"),
        &json!([retrieval]),
    )?;
    write_json(
        &directory.join("runtime-later.json"),
        &store.trace(&later.id)?,
    )?;
    let observed = json!({"schema_version":1,"case_id":"location-retrieval","events":[{"type":"outcome_stored","session":"s1","result":"r1","path":artifact.path,"sha256":artifact.sha256},{"type":"location_answered","session":"s1","result":"r1","path":receipt["follow_up"]["answer_path"],"invocations_started":receipt["follow_up"]["invocations_started"]},{"type":"outcome_retrieved","session":"s2","source_session":"s1","result":"r1","path":retrieved.artifacts[0].path,"sha256":retrieved.artifacts[0].sha256,"invocations_started":after.saturating_sub(before)}],"final_state":{"production_runs":trace.assignments.iter().filter(|a|a.purpose=="execute").count(),"follow_up_invocations":receipt["follow_up"]["invocations_started"],"working_directory":stored.directory,"is_git_worktree":stored.directory.ancestors().any(|p|p.join(".git").exists()),"artifact_path":artifact.path}});
    write_json(&directory.join("observed.json"), &observed)?;
    let mut aliases = export::aliases(&trace);
    aliases["aliases"]["s2"] = json!(later.id);
    aliases["aliases"]["r1"] = json!(stored.result_id);
    aliases["projection_sources"] = json!([{"projected_index":0,"acceptance_id":stored.acceptance_id},{"projected_index":1,"message_seq":trace.history.iter().rev().find(|event|event.kind=="message"&&event.data["kind"]=="answer").map(|e|e.seq)},{"projected_index":2,"boundary_action_index":0}]);
    write_json(&directory.join("alias-map.json"), &aliases)?;
    let metrics = export::metrics(&trace, started.elapsed().as_secs_f64(), 1)?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, "location-retrieval").await?;
    Ok(
        json!({"case_id":"location-retrieval","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}

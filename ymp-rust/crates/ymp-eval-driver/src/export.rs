//! Projections over retained runtime records. Never reads expected event traces.
use crate::file_digest;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};
use ymp_core::*;

pub fn manifest(root: &Path) -> Result<Value> {
    let git = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    ensure!(git.status.success(), "Cannot identify source revision");
    let mut source = BTreeMap::new();
    for area in ["ymp-rust/crates", "ymp-evals/driver"] {
        for entry in walkdir::WalkDir::new(root.join(area))
            .into_iter()
            .filter_entry(|entry| entry.file_name() != "target")
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                source.insert(
                    entry.path().strip_prefix(root)?.display().to_string(),
                    file_digest(entry.path())?,
                );
            }
        }
    }
    for path in ["Cargo.toml", "Cargo.lock"] {
        source.insert(path.into(), file_digest(&root.join(path))?);
    }
    let mut fixtures = BTreeMap::new();
    for area in [
        "ymp-evals/scenarios",
        "ymp-evals/fixtures/universal",
        "ymp-evals/validators",
    ] {
        for entry in walkdir::WalkDir::new(root.join(area))
            .into_iter()
            .filter_entry(|entry| entry.file_name() != "__pycache__")
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                fixtures.insert(
                    entry.path().strip_prefix(root)?.display().to_string(),
                    file_digest(entry.path())?,
                );
            }
        }
    }
    Ok(
        json!({"schema_version":1,"execution_kind":"scripted-runtime","application_revision":String::from_utf8(git.stdout)?.trim(),"source_tree_sha256":content_digest(&serde_json::to_string(&source)?),"source_files":source,"fixture_hashes":fixtures,"exporter":{"id":"ymp-eval-driver","version":env!("CARGO_PKG_VERSION"),"sha256":file_digest(&root.join("ymp-rust/crates/ymp-eval-driver/src/export.rs"))?},"scripted_provider":{"id":"ymp.evals.scripted","version":env!("CARGO_PKG_VERSION"),"sha256":file_digest(&root.join("ymp-rust/crates/ymp-eval-driver/src/script.rs"))?,"workload_sha256":file_digest(&root.join("ymp-evals/driver/artifact_writer.py"))?},"reference_outputs_in_provider_context":false,"native_inference":false}),
    )
}
pub fn aliases(trace: &SessionTrace) -> Value {
    let mut map = BTreeMap::new();
    map.insert("s1".to_owned(), trace.session.id.clone());
    for (prefix, ids) in [
        (
            "assignment",
            trace
                .assignments
                .iter()
                .map(|a| a.id.clone())
                .collect::<Vec<_>>(),
        ),
        (
            "invocation",
            trace.invocations.iter().map(|a| a.id.clone()).collect(),
        ),
        (
            "decision",
            trace.decisions.iter().map(|d| d.id.clone()).collect(),
        ),
        ("task", trace.tasks.iter().map(|t| t.id.clone()).collect()),
    ] {
        for (index, id) in ids.into_iter().enumerate() {
            map.insert(format!("{prefix}{}", index + 1), id);
        }
    }
    json!({"schema_version":1,"aliases":map,"note":"Audit identifiers only; live capability secrets are never exported"})
}
fn phase(assignment: &AssignmentRecord) -> &'static str {
    match assignment.purpose.as_str() {
        "plan" | "bid" => "planning",
        "execute" if assignment.task.as_ref().is_some_and(|t| t.attempt > 1) => "retry",
        "execute" => "execution",
        "review" | "review_plan" | "final_review" | "review_memory" => "review",
        "learn" => "consultation",
        _ => "communication",
    }
}
pub fn usage(trace: &SessionTrace) -> Result<Vec<Value>> {
    trace.invocations.iter().map(|invocation|{
        let assignment=trace.assignments.iter().find(|a|a.id==invocation.assignment_id).context("Invocation has no actual assignment")?;
        let tokens=invocation.usage.as_ref().and_then(|u|u.counts.known_total());
        let coverage=if tokens.is_none(){"unknown"}else if invocation.usage.as_ref().is_some_and(|u|u.finalized&&!u.partial&&u.counts.input.is_some()&&u.counts.output.is_some()){"complete"}else{"partial"};
        Ok(json!({"agent_id":assignment.agent_id,"assignment_id":assignment.id,"invocation_id":invocation.id,"phase":phase(assignment),"tokens":tokens,"coverage":coverage}))
    }).collect()
}
pub fn metrics(trace: &SessionTrace, elapsed: f64, _native_peak: usize) -> Result<Value> {
    let mut active = std::collections::HashSet::new();
    let mut seen = std::collections::HashSet::new();
    let mut peak = 0;
    for event in trace
        .history
        .iter()
        .filter(|event| event.kind == "provenance")
    {
        match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
            ProvenanceEvent::AssignmentStarted { invocation, .. } => {
                seen.insert(invocation.id.clone());
                active.insert(invocation.id);
                peak = peak.max(active.len());
            }
            ProvenanceEvent::InvocationFinished { invocation } => {
                active.remove(&invocation.id);
            }
            _ => {}
        }
    }
    let peak = (seen.len() == trace.invocations.len()).then_some(peak);

    Ok(
        json!({"elapsed_seconds":elapsed,"attempts":trace.tasks.iter().map(|t|t.attempts).sum::<usize>(),"peak_active_invocations":peak,"usage":usage(trace)?,"reported_tokens":trace.usage.total.known_total(),"usage_coverage":if trace.usage.total.reported==0{"unknown"}else if trace.usage.total.is_partial(){"partial"}else{"complete"},"reported_cost":Value::Null,"cost_coverage":"unknown","strict_token_bound_claimed":trace.budget.as_ref().is_some_and(|b|b.strict_token_bound),"native_inference":false}),
    )
}
pub fn workflow(
    case: &str,
    work: &Path,
    trace: &SessionTrace,
    follow_up: Option<Value>,
) -> Result<Value> {
    let acceptances = trace
        .decisions
        .iter()
        .filter(|d| d.kind == "task_accepted")
        .collect::<Vec<_>>();
    ensure!(
        acceptances.len() == 1,
        "Controlled workflow requires exactly one actual task acceptance; observed {}",
        acceptances.len()
    );
    let acceptance = acceptances[0];
    let result = acceptance
        .links
        .result
        .as_ref()
        .context("Acceptance has no result identity")?;
    ensure!(
        result.artifacts.len() == 1,
        "Controlled workflow has unexpected artifact cardinality"
    );
    let artifact = &result.artifacts[0];
    let producers = result
        .producer_assignment_ids
        .iter()
        .map(|id| {
            trace
                .assignments
                .iter()
                .find(|a| &a.id == id)
                .map(|a| a.agent_id.clone())
                .context("Missing actual producer")
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        acceptance.links.review_ids.len() == 1,
        "Unexpected independent review count"
    );
    let review = trace
        .decisions
        .iter()
        .find(|d| d.id == acceptance.links.review_ids[0])
        .context("Missing linked review")?;
    let review_result = review
        .links
        .result
        .as_ref()
        .context("Review has no result binding")?;
    let confirmations=trace.decisions.iter().filter(|d|d.kind=="check_observed"&&d.links.result.as_ref().is_some_and(|r|r.id==result.id&&r.version==result.version)).map(|d|{
        let check=d.links.check.as_ref().context("Missing runtime check capture")?;
        let checked_result=d.links.result.as_ref().unwrap();
        let checked_artifact=checked_result.artifacts.iter().find(|a|a.path==artifact.path).context("Check has no scoped artifact")?;
        Ok(json!({"confirmation_id":d.id,"observer":if d.actor.is_none(){"runtime"}else{"agent"},"outcome":match check.outcome{ConfirmationCheckOutcome::Passed=>"passed",ConfirmationCheckOutcome::Failed=>"failed",ConfirmationCheckOutcome::Inconclusive=>"inconclusive"},"check_id":check.check_id,"result_id":checked_result.id,"result_version":checked_result.version,"artifact_sha256":checked_artifact.sha256,"criterion_ids":check.criterion_ids,"inputs":check.inputs.iter().map(|i|json!({"path":i.path,"sha256":i.sha256})).collect::<Vec<_>>()}))
    }).collect::<Result<Vec<_>>>()?;
    let observations=trace.decisions.iter().filter(|d|d.kind=="reputation_observed"&&d.links.result.as_ref().is_some_and(|r|r.id==result.id&&r.version==result.version)).map(|d|json!({"observation_id":d.links.observation_id,"agent_id":d.actor,"result_id":result.id,"result_version":result.version,"confirmation_id":d.links.confirmation_ids.first()})).collect::<Vec<_>>();
    let confirmation = match acceptance.outcome {
        Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Confirmed,
        }) => "confirmed",
        Some(DecisionOutcome::Accepted {
            confirmation: ConfirmationStatus::Unconfirmed,
        }) => "unconfirmed",
        _ => "unknown",
    };
    let reviewed_artifact = review_result
        .artifacts
        .iter()
        .find(|a| a.path == artifact.path)
        .context("Review has no artifact snapshot")?;
    let mut receipt = json!({"schema_version":1,"case_id":case,"session_id":trace.session.id,"working_directory":work.canonicalize()?,"artifact":{"path":artifact.path,"sha256":artifact.sha256,"result_id":result.id,"result_version":result.version,"producer_agent_ids":producers},"review":{"review_id":review.id,"reviewer_agent_id":review.actor,"result_id":review_result.id,"result_version":review_result.version,"artifact_sha256":reviewed_artifact.sha256,"decision":if matches!(review.outcome,Some(DecisionOutcome::Accepted{..})){"accepted"}else{"rejected"},"rationale":review.reason},"acceptance":{"result_id":result.id,"result_version":result.version,"review_id":acceptance.links.review_ids[0],"state":"accepted","confirmation":confirmation,"confirmation_ids":acceptance.links.confirmation_ids},"confirmations":confirmations,"reputation_observations":observations,"usage":usage(trace)?});
    if let Some(follow_up) = follow_up {
        receipt["follow_up"] = follow_up;
    }
    Ok(receipt)
}

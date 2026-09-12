//! Projections over retained runtime records. Never reads expected event traces.
use crate::file_digest;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};
use ymp_core::*;

const BUILD_SNAPSHOT: &str = include_str!(concat!(env!("OUT_DIR"), "/native-source-snapshot.json"));

fn inventory(root: &Path, areas: &[&str]) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for area in areas {
        for entry in walkdir::WalkDir::new(root.join(area))
            .into_iter()
            .filter_entry(|e| e.file_name() != "target" && e.file_name() != "__pycache__")
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                files.insert(
                    entry.path().strip_prefix(root)?.display().to_string(),
                    file_digest(entry.path())?,
                );
            }
        }
    }
    Ok(files)
}
fn verify_build(root: &Path, build: &Value) -> Result<()> {
    let source = inventory(
        root,
        &[
            "Cargo.toml",
            "Cargo.lock",
            "ymp-rust/crates",
            "ymp-evals/driver",
        ],
    )?;
    let fixtures = inventory(
        root,
        &[
            "ymp-evals/scenarios",
            "ymp-evals/fixtures/universal",
            "ymp-evals/validators",
        ],
    )?;
    ensure!(
        serde_json::to_value(source)? == build["source_files"],
        "Evaluation source differs from the compiled build; rebuild before running"
    );
    ensure!(
        serde_json::to_value(fixtures)? == build["fixture_hashes"],
        "Evaluation fixtures/checkers differ from the compiled build; rebuild before running"
    );
    Ok(())
}
fn executable_binding() -> Result<Value> {
    let executable = std::env::current_exe()?;
    let build: Value = serde_json::from_str(BUILD_SNAPSHOT)?;
    verify_build(&crate::root(), &build)?;
    Ok(
        json!({"path":executable,"sha256":file_digest(&executable)?,"source_tree_sha256":build["source_tree_sha256"],"build_revision":build["revision"]}),
    )
}
pub fn manifest(root: &Path) -> Result<Value> {
    let build: Value = serde_json::from_str(BUILD_SNAPSHOT)?;
    verify_build(root, &build)?;
    Ok(
        json!({"schema_version":1,"execution_kind":"scripted-runtime","application_revision":build["revision"],"source_tree_sha256":build["source_tree_sha256"],"source_files":build["source_files"],"fixture_hashes":build["fixture_hashes"],"build":build,"executable":executable_binding()?,"exporter":{"id":"ymp-eval-driver","version":env!("CARGO_PKG_VERSION"),"source":"ymp-rust/crates/ymp-eval-driver/src/export.rs"},"execution_backends":{"source":"Each case's metrics.execution_backends, including nested session metrics","identity_origin":"actual invocation records; absent native/backend metadata stays null","implementation_binding":"executable.sha256 and source_tree_sha256"},"reference_outputs_in_provider_context":false,"native_inference":false}),
    )
}
fn execution_backends(trace: &SessionTrace) -> Result<Vec<Value>> {
    let mut identities = BTreeMap::<String, Value>::new();
    for invocation in &trace.invocations {
        let key = serde_json::to_string(&invocation.execution_backend)?;
        let row = identities.entry(key).or_insert_with(|| json!({"identity":invocation.execution_backend,"invocation_ids":[],"native_versions":[]}));
        row["invocation_ids"]
            .as_array_mut()
            .unwrap()
            .push(json!(invocation.id));
        row["native_versions"]
            .as_array_mut()
            .unwrap()
            .push(json!({"invocation_id":invocation.id,"version":invocation.native_version}));
    }
    Ok(identities.into_values().collect())
}

/// Every projected occurrence retains its exact durable source. No expected
/// trace, alias value or desired order participates in event selection.
#[derive(Default)]
pub(crate) struct Projection {
    pub events: Vec<Value>,
    pub sources: Vec<Value>,
}
impl Projection {
    pub fn push(&mut self, value: Value, mut source: Value) {
        source["projected_index"] = json!(self.events.len());
        self.events.push(value);
        self.sources.push(source);
    }
}
pub(crate) fn ordered_history<'a>(
    traces: impl IntoIterator<Item = &'a SessionTrace>,
) -> Result<Vec<&'a HistoryEvent>> {
    let mut events = Vec::new();
    for trace in traces {
        ensure!(
            trace
                .history
                .windows(2)
                .all(|pair| pair[0].seq < pair[1].seq),
            "Runtime history is not in its recorded sequence order"
        );
        events.extend(&trace.history);
    }
    events.sort_by_key(|event| event.seq);
    ensure!(
        events.windows(2).all(|pair| pair[0].seq != pair[1].seq),
        "Runtime source sequence is ambiguous across supplied traces"
    );
    Ok(events)
}
pub(crate) fn event_source(event: &HistoryEvent) -> Value {
    json!({"runtime_event_seq":event.seq,"session_id":event.session_id})
}
pub(crate) fn record_native(
    store: &ymp_storage::Store,
    session: &str,
    journal: &crate::script::NativeJournal,
    mut value: Value,
) -> Result<()> {
    value["observation_id"] = json!(new_id());
    journal.push(value.clone());
    store.event(session, "evaluation_native_observed", &value)
}
pub(crate) fn native_source(event: &HistoryEvent, native: &[Value]) -> Result<Value> {
    let id = event.data["observation_id"]
        .as_str()
        .context("Native observation identity missing")?;
    let matches = native
        .iter()
        .filter(|row| row["observation_id"] == id)
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1,
        "Native observation has missing or ambiguous raw journal provenance"
    );
    let mut source = event_source(event);
    source["native_seq"] = matches[0]["seq"].clone();
    source["native_observation_id"] = json!(id);
    Ok(source)
}
pub(crate) fn audit_native_sources(events: &[&HistoryEvent], native: &[Value]) -> Result<()> {
    ensure!(native.windows(2).all(|pair| matches!((pair[0]["seq"].as_u64(),pair[1]["seq"].as_u64()),(Some(a),Some(b)) if a < b)), "Raw native journal sequence is ambiguous");
    let mut observed = std::collections::HashSet::new();
    for event in events
        .iter()
        .filter(|event| event.kind == "evaluation_native_observed")
    {
        ensure!(
            native
                .iter()
                .filter(|row| row["observation_id"] == event.data["observation_id"])
                .count()
                == 1,
            "Durable native observation has missing or duplicate raw provenance"
        );
    }
    let mut prior_runtime_seq = None;
    for row in native {
        let id = row["observation_id"]
            .as_str()
            .context("Raw native observation is not durably bound")?;
        ensure!(
            observed.insert(id),
            "Duplicate raw native observation identity"
        );
        let matches = events
            .iter()
            .filter(|event| {
                event.kind == "evaluation_native_observed" && event.data["observation_id"] == id
            })
            .collect::<Vec<_>>();
        ensure!(
            matches.len() == 1,
            "Raw native observation is missing or duplicated in the durable journal"
        );
        ensure!(
            prior_runtime_seq.is_none_or(|prior| prior < matches[0].seq),
            "Raw and durable native observation order disagree"
        );
        prior_runtime_seq = Some(matches[0].seq);
        let mut payload = row.clone();
        payload
            .as_object_mut()
            .context("Native row is not an object")?
            .remove("seq");
        payload.as_object_mut().unwrap().remove("observed_at");
        ensure!(
            payload == matches[0].data,
            "Native observation differs between raw and durable journals"
        );
    }
    Ok(())
}
#[derive(Default)]
pub(crate) struct LifecycleAudit {
    starts: BTreeMap<String, usize>,
    closes: BTreeMap<String, usize>,
}
impl LifecycleAudit {
    pub fn anomaly(
        &mut self,
        event: &HistoryEvent,
        trace: &SessionTrace,
        native: &[Value],
        native_close: &str,
    ) -> Result<Option<Value>> {
        if event.kind != "provenance" {
            return Ok(None);
        }
        let (invocation, counter, native_kind, kind) =
            match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
                ProvenanceEvent::AssignmentStarted { invocation, .. } => (
                    invocation,
                    &mut self.starts,
                    "native_started",
                    "unexpected_invocation_started",
                ),
                ProvenanceEvent::InvocationFinished { invocation } => (
                    invocation,
                    &mut self.closes,
                    native_close,
                    "unexpected_invocation_closed",
                ),
                _ => return Ok(None),
            };
        let count = counter.entry(invocation.id.clone()).or_default();
        *count += 1;
        let native_id = trace
            .invocations
            .iter()
            .find(|i| i.id == invocation.id)
            .and_then(|i| i.native_session_id.as_deref());
        let matched = native_id.is_some()
            && native
                .iter()
                .any(|row| row["type"] == native_kind && row["native_id"].as_str() == native_id);
        Ok((!matched || *count > 1)
            .then(|| json!({"type":kind,"invocation_id":invocation.id,"state":invocation.state})))
    }
}
pub(crate) fn coverage(usage: Option<&UsageSnapshot>) -> &'static str {
    match usage {
        None => "unknown",
        Some(u)
            if u.finalized
                && !u.partial
                && u.counts.input.is_some()
                && u.counts.output.is_some() =>
        {
            "complete"
        }
        Some(_) => "partial",
    }
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
        "execute" | "execution" => "execution",
        "retry" => "retry",
        "consultation" => "consultation",
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
        json!({"executable":executable_binding()?,"execution_backends":execution_backends(trace)?,"elapsed_seconds":elapsed,"attempts":trace.tasks.iter().map(|t|t.attempts).sum::<usize>(),"peak_active_invocations":peak,"usage":usage(trace)?,"reported_tokens":trace.usage.total.known_total(),"usage_coverage":if trace.usage.total.reported==0{"unknown"}else if trace.usage.total.is_partial(){"partial"}else{"complete"},"reported_cost":Value::Null,"cost_coverage":"unknown","strict_token_bound_claimed":trace.budget.as_ref().is_some_and(|b|b.strict_token_bound),"native_inference":false}),
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

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;

//! Negative controls start from actual runtime executions and mutate only audit
//! copies. They never introduce expected traces into runtime authority.
use crate::{export, knowledge_protocol, read_json, root, workflows, write_json};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};
use ymp_core::*;

fn temporary() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::Builder::new()
        .prefix("ymp-driver-control-")
        .tempdir_in("/tmp")
        .unwrap();
    let directory = temp.path().join("case");
    std::fs::create_dir(&directory).unwrap();
    (temp, directory.canonicalize().unwrap())
}
fn validate(directory: &Path, kind: &str, case: &str, observed: &Value) -> Value {
    let path = directory.join("control-observed.json");
    write_json(&path, observed).unwrap();
    let result = std::process::Command::new("python3")
        .arg(root().join("ymp-evals/validators/universal.py"))
        .args([kind, "--case", case, "--workdir"])
        .arg(directory.join("work"))
        .arg("--observed")
        .arg(path)
        .output()
        .unwrap();
    serde_json::from_slice(&result.stdout).unwrap()
}
#[tokio::test]
async fn actual_workflow_export_rejects_missing_acceptance_and_retains_extra_usage_and_credit() {
    let (_temp, directory) = temporary();
    let spec = read_json(&root().join("ymp-evals/scenarios/universal-workflows.json")).unwrap();
    assert_eq!(
        workflows::run(
            &root(),
            &directory,
            "document",
            serde_json::from_value(spec["workflows"]["document"].clone()).unwrap()
        )
        .await
        .unwrap()["complete"],
        true
    );
    let trace: SessionTrace =
        serde_json::from_value(read_json(&directory.join("runtime.json")).unwrap()).unwrap();
    let baseline = read_json(&directory.join("workflow.json")).unwrap();
    let mut missing = trace.clone();
    missing.decisions.retain(|d| d.kind != "task_accepted");
    assert!(export::workflow(
        "document",
        &directory.join("work"),
        &missing,
        Some(baseline["follow_up"].clone())
    )
    .is_err());
    let mut extra = trace.clone();
    let credit = extra
        .decisions
        .iter()
        .find(|d| d.kind == "reputation_observed")
        .unwrap()
        .clone();
    extra.decisions.push(credit);
    let mut invocation = extra.invocations[0].clone();
    invocation.id = new_id();
    extra.invocations.push(invocation);
    let projected = export::workflow(
        "document",
        &directory.join("work"),
        &extra,
        Some(baseline["follow_up"].clone()),
    )
    .unwrap();
    assert_eq!(
        projected["usage"].as_array().unwrap().len(),
        baseline["usage"].as_array().unwrap().len() + 1
    );
    assert_eq!(
        projected["reputation_observations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        validate(&directory, "workflow", "document", &projected)["validator_passed"],
        false
    );
    let native = read_json(&directory.join("native.json")).unwrap();
    for row in native.as_array().unwrap() {
        if let Some(prompt) = row["prompt"].as_str() {
            assert!(!prompt.contains("reference/"));
            assert!(!prompt.contains("universal-protocol.json"));
            assert!(!prompt.contains("\"expected\": "));
        }
    }
}
#[tokio::test]
async fn actual_knowledge_export_retains_duplicates_order_and_rejects_missing_sources() {
    let (_temp, directory) = temporary();
    let spec = read_json(&root().join("ymp-evals/scenarios/universal-protocol.json")).unwrap();
    assert_eq!(
        knowledge_protocol::run(&root(), &directory, &spec["cases"]["knowledge-correction"])
            .await
            .unwrap()["complete"],
        true
    );
    let original: BTreeMap<String, SessionTrace> =
        serde_json::from_value(read_json(&directory.join("runtime.json")).unwrap()).unwrap();
    let bindings = read_json(&directory.join("projection-bindings.json")).unwrap();
    let values = bindings["observed_values"].as_array().unwrap();
    let sources = bindings["source_bindings"].as_array().unwrap();
    let boundary = read_json(&directory.join("boundary-actions.json")).unwrap();
    let boundary = boundary.as_array().unwrap();
    let baseline = knowledge_protocol::project(&original, values, sources, boundary).unwrap();
    assert_eq!(baseline.events.len(), baseline.sources.len());
    let mut observed = read_json(&directory.join("observed.json")).unwrap();
    for target in ["knowledge_retained", "eval_knowledge_boundary"] {
        let mut traces = original.clone();
        let selected = traces
            .values()
            .flat_map(|t| &t.history)
            .find(|e| {
                e.kind == target && (target != "knowledge_retained" || e.data["status"] == "active")
            })
            .unwrap()
            .clone();
        let seq = selected.seq;
        for trace in traces.values_mut() {
            for event in &mut trace.history {
                if event.seq > seq {
                    event.seq += 1;
                }
            }
        }
        let trace = traces.get_mut(&selected.session_id).unwrap();
        let mut duplicate = selected;
        duplicate.seq += 1;
        trace.history.push(duplicate);
        trace.history.sort_by_key(|e| e.seq);
        let projection = knowledge_protocol::project(&traces, values, sources, boundary).unwrap();
        assert_eq!(projection.events.len(), baseline.events.len() + 1);
        observed["events"] = json!(projection.events);
        assert_eq!(
            validate(&directory, "protocol", "knowledge-correction", &observed)["validator_passed"],
            false
        );
    }
    let mut reordered = original.clone();
    let seqs = export::ordered_history(reordered.values())
        .unwrap()
        .iter()
        .filter(|e| e.kind == "eval_knowledge_boundary")
        .take(2)
        .map(|e| e.seq)
        .collect::<Vec<_>>();
    for trace in reordered.values_mut() {
        for e in &mut trace.history {
            if e.seq == seqs[0] {
                e.seq = seqs[1]
            } else if e.seq == seqs[1] {
                e.seq = seqs[0]
            }
            e.created_at = "2020-01-01T00:00:00Z".into();
        }
        trace.history.sort_by_key(|e| e.seq);
    }
    observed["events"] = json!(
        knowledge_protocol::project(&reordered, values, sources, boundary)
            .unwrap()
            .events
    );
    assert_eq!(
        validate(&directory, "protocol", "knowledge-correction", &observed)["validator_passed"],
        false
    );
    let mut missing = original.clone();
    for trace in missing.values_mut() {
        trace
            .history
            .retain(|e| e.kind != "eval_knowledge_boundary");
    }
    assert!(knowledge_protocol::project(&missing, values, sources, boundary).is_err());
}
#[test]
fn qualitative_rubric_rejects_well_formatted_unrelated_content() {
    let (_temp, directory) = temporary();
    std::fs::create_dir_all(directory.join("work/outputs")).unwrap();
    let irrelevant=b"# Workshop opening\n\nThe outer planets move around the sun and their orbital periods vary in length across the solar system.\n\nA calculator can multiply numbers and provide a result on its display when the arithmetic button is pressed.\n";
    std::fs::write(directory.join("work/outputs/ideas.md"), irrelevant).unwrap();
    let result = std::process::Command::new("python3")
        .arg(root().join("ymp-evals/validators/universal.py"))
        .args(["artifact", "--case", "qualitative", "--workdir"])
        .arg(directory.join("work"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "Structure control failed before review: {}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(!crate::script::qualitative_assessment(irrelevant).0);
}

//! These controls mutate copies of actual retained records, never runtime authority.
use super::*;
use crate::{effort_protocol, location_protocol, partial_protocol, read_json};
use std::{path::PathBuf, process::Command};
use ymp_storage::Store;

async fn case(name: &str) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::Builder::new()
        .prefix("ymp-export-")
        .tempdir_in("/tmp")
        .unwrap();
    let directory = temp.path().join("case");
    std::fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let spec =
        read_json(&crate::root().join("ymp-evals/scenarios/universal-protocol.json")).unwrap();
    let result = crate::protocols::run(&crate::root(), &directory, name, &spec["cases"][name])
        .await
        .unwrap();
    assert_eq!(result["complete"], true, "{result}");
    (temp, directory)
}
fn rejected(directory: &Path, name: &str, projection: &Projection) {
    mapped(projection);
    let mut observed = read_json(&directory.join("observed.json")).unwrap();
    observed["events"] = json!(projection.events);
    let path = directory.join("control-observed.json");
    crate::write_json(&path, &observed).unwrap();
    let result = Command::new("python3")
        .arg(crate::root().join("ymp-evals/validators/universal.py"))
        .args(["protocol", "--case", name, "--workdir"])
        .arg(directory.join("work"))
        .arg("--observed")
        .arg(path)
        .output()
        .unwrap();
    let verdict: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(
        !result.status.success(),
        "Unexpected occurrence or order was accepted: {verdict}"
    );
    assert_eq!(verdict["validator_passed"], false);
}
fn mapped(projection: &Projection) {
    assert_eq!(projection.events.len(), projection.sources.len());
    for (index, source) in projection.sources.iter().enumerate() {
        assert_eq!(source["projected_index"], index);
        assert!(source["runtime_event_seq"].as_i64().is_some());
    }
}
fn duplicate(traces: &mut BTreeMap<String, SessionTrace>, mut event: HistoryEvent) {
    event.seq += 1;
    for trace in traces.values_mut() {
        for row in &mut trace.history {
            if row.seq >= event.seq {
                row.seq += 1;
            }
        }
    }
    let trace = traces
        .values_mut()
        .find(|trace| trace.session.id == event.session_id)
        .unwrap();
    trace.history.push(event);
    trace.history.sort_by_key(|row| row.seq);
}

#[test]
fn build_binding_rejects_source_and_checker_drift_and_unknown_backends_stay_unknown() {
    let temp = tempfile::tempdir().unwrap();
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "ymp-rust/crates/sample/code.rs",
        "ymp-evals/driver/workload.py",
        "ymp-evals/scenarios/case.json",
        "ymp-evals/fixtures/universal/reference/output.txt",
        "ymp-evals/validators/check.py",
    ] {
        let path = temp.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "captured").unwrap();
    }
    let build = json!({"source_files":inventory(temp.path(), &["Cargo.toml","Cargo.lock","ymp-rust/crates","ymp-evals/driver"]).unwrap(),"fixture_hashes":inventory(temp.path(), &["ymp-evals/scenarios","ymp-evals/fixtures/universal","ymp-evals/validators"]).unwrap()});
    verify_build(temp.path(), &build).unwrap();
    for path in [
        "ymp-rust/crates/sample/code.rs",
        "ymp-evals/validators/check.py",
        "ymp-evals/driver/workload.py",
        "ymp-evals/fixtures/universal/reference/output.txt",
    ] {
        std::fs::write(temp.path().join(path), "changed").unwrap();
        assert!(
            verify_build(temp.path(), &build).is_err(),
            "Drift was hidden: {path}"
        );
        std::fs::write(temp.path().join(path), "captured").unwrap();
    }
    let actual = manifest(&crate::root()).unwrap();
    assert_eq!(
        actual["executable"]["sha256"],
        file_digest(&std::env::current_exe().unwrap()).unwrap()
    );
    assert!(actual.get("scripted_provider").is_none());
}

#[tokio::test]
async fn effort_projection_preserves_extra_early_unattributed_and_duplicate_runtime_events() {
    let (_temp, directory) = case("effort-support").await;
    let original: BTreeMap<String, SessionTrace> =
        serde_json::from_value(read_json(&directory.join("runtime.json")).unwrap()).unwrap();
    let native: Vec<Value> =
        serde_json::from_value(read_json(&directory.join("native.json")).unwrap()).unwrap();
    let aliases: BTreeMap<String, String> = serde_json::from_value(
        read_json(&directory.join("alias-map.json")).unwrap()["assignments"].clone(),
    )
    .unwrap();
    let baseline = effort_protocol::project(&original, &aliases, &native).unwrap();
    mapped(&baseline);
    assert_eq!(baseline.events.len(), 5);
    let actual = execution_backends(&original["s1"]).unwrap();
    assert_eq!(actual[0]["identity"]["id"], "ymp.evals.effort-observations");
    assert!(execution_backends(&original["s2"]).unwrap().is_empty());
    let mut unknown = original["s1"].clone();
    unknown.invocations[0].execution_backend = None;
    assert!(execution_backends(&unknown).unwrap()[0]["identity"].is_null());
    let closed = original
        .values()
        .flat_map(|trace| &trace.history)
        .find(|event| {
            event.kind == "evaluation_native_observed" && event.data["type"] == "native_closed"
        })
        .unwrap();
    let mut extra = original.clone();
    let mut copy = closed.clone();
    let id = new_id();
    copy.data["observation_id"] = json!(id);
    let mut raw = native.clone();
    let mut row = raw
        .iter()
        .find(|row| row["observation_id"] == closed.data["observation_id"])
        .unwrap()
        .clone();
    row["observation_id"] = json!(id);
    row["seq"] = json!(raw.len() + 1);
    raw.push(row);
    duplicate(&mut extra, copy);
    let projected = effort_protocol::project(&extra, &aliases, &raw).unwrap();
    assert_eq!(projected.events.len(), baseline.events.len() + 1);
    rejected(&directory, "effort-support", &projected);
    let mut early = original.clone();
    let denied = early
        .values()
        .flat_map(|trace| &trace.history)
        .find(|event| event.kind == "evaluation_boundary_observed")
        .unwrap()
        .seq;
    for trace in early.values_mut() {
        for event in &mut trace.history {
            if event.seq == closed.seq {
                event.seq = denied;
            } else if event.seq == denied {
                event.seq = closed.seq;
            }
            event.created_at = "2020-01-01T00:00:00Z".into();
        }
        trace.history.sort_by_key(|event| event.seq);
    }
    let projected = effort_protocol::project(&early, &aliases, &native).unwrap();
    assert_eq!(projected.events[1]["type"], "admission_denied");
    assert_eq!(projected.events[2]["type"], "invocation_closed");
    rejected(&directory, "effort-support", &projected);
    let mut duplicate_runtime = original.clone();
    let event = duplicate_runtime
        .values()
        .flat_map(|trace| &trace.history)
        .find(|e| {
            e.kind == "provenance"
                && matches!(
                    serde_json::from_value::<ProvenanceEvent>(e.data.clone()).unwrap(),
                    ProvenanceEvent::InvocationFinished { .. }
                )
        })
        .unwrap()
        .clone();
    duplicate(&mut duplicate_runtime, event);
    rejected(
        &directory,
        "effort-support",
        &effort_protocol::project(&duplicate_runtime, &aliases, &native).unwrap(),
    );
    let mut unattributed = original.clone();
    let mut event = closed.clone();
    event.data["observation_id"] = json!(new_id());
    event.data["native_id"] = json!("unadmitted-native-call");
    let mut raw = native.clone();
    let mut row = event.data.clone();
    row["seq"] = json!(raw.len() + 1);
    row["observed_at"] = json!(now());
    raw.push(row);
    duplicate(&mut unattributed, event);
    rejected(
        &directory,
        "effort-support",
        &effort_protocol::project(&unattributed, &aliases, &raw).unwrap(),
    );
    assert!(effort_protocol::project(&original, &aliases, &native[..1]).is_err());
}

#[tokio::test]
async fn location_projection_preserves_extra_answers_outcomes_and_query_results() {
    let (_temp, directory) = case("location-retrieval").await;
    let source: SessionTrace =
        serde_json::from_value(read_json(&directory.join("runtime.json")).unwrap()).unwrap();
    let later: SessionTrace =
        serde_json::from_value(read_json(&directory.join("runtime-later.json")).unwrap()).unwrap();
    let outcomes = Store::open(&directory.join("metadata"))
        .unwrap()
        .outcomes(&source.session.id)
        .unwrap();
    let sessions = [
        (source.session.id.clone(), "s1".into()),
        (later.session.id.clone(), "s2".into()),
    ]
    .into_iter()
    .collect();
    let results = [(outcomes[0].result_id.clone(), "r1".into())]
        .into_iter()
        .collect();
    let original = [("s1".into(), source), ("s2".into(), later)]
        .into_iter()
        .collect::<BTreeMap<String, SessionTrace>>();
    let project = |traces: &BTreeMap<String, SessionTrace>| {
        location_protocol::project(
            &traces.values().collect::<Vec<_>>(),
            &outcomes,
            &sessions,
            &results,
        )
        .unwrap()
    };
    let baseline = project(&original);
    assert_eq!(baseline.events.len(), 3);
    mapped(&baseline);
    for kind in ["answer", "acceptance", "query"] {
        let mut extra = original.clone();
        if kind == "query" {
            let event = extra
                .get_mut("s2")
                .unwrap()
                .history
                .iter_mut()
                .find(|e| e.kind == "evaluation_boundary_observed")
                .unwrap();
            let row = event.data["returned"][0].clone();
            event.data["returned"].as_array_mut().unwrap().push(row);
        } else {
            let event=extra["s1"].history.iter().find(|event|if kind=="answer" {event.kind=="message" && event.data["kind"]=="answer"}else{event.kind=="provenance" && matches!(serde_json::from_value::<ProvenanceEvent>(event.data.clone()).unwrap(),ProvenanceEvent::DecisionRecorded{decision} if decision.kind=="task_accepted")}).unwrap().clone();
            duplicate(&mut extra, event);
        }
        let projected = project(&extra);
        assert_eq!(projected.events.len(), 4);
        rejected(&directory, "location-retrieval", &projected);
    }
}

#[tokio::test]
async fn partial_projection_uses_causal_sequence_and_maps_the_guarantee_read() {
    let (_temp, directory) = case("partial-usage").await;
    let original: SessionTrace =
        serde_json::from_value(read_json(&directory.join("runtime.json")).unwrap()).unwrap();
    let native: Vec<Value> =
        serde_json::from_value(read_json(&directory.join("native.json")).unwrap()).unwrap();
    let aliases: BTreeMap<String, String> = serde_json::from_value(
        read_json(&directory.join("alias-map.json")).unwrap()["assignments"].clone(),
    )
    .unwrap();
    let baseline = partial_protocol::project(&original, &native, &aliases).unwrap();
    mapped(&baseline);
    assert_eq!(baseline.events.len(), 4);
    let guarantee = original
        .history
        .iter()
        .find(|event| event.kind == "evaluation_boundary_observed")
        .unwrap();
    assert_eq!(baseline.sources[3]["runtime_event_seq"], guarantee.seq);
    let mut tied = original.clone();
    for event in &mut tied.history {
        event.created_at = "2020-01-01T00:00:00Z".into();
    }
    assert_eq!(
        partial_protocol::project(&tied, &native, &aliases)
            .unwrap()
            .events,
        baseline.events
    );
    let stop = tied
        .history
        .iter()
        .position(|e| e.kind == "evaluation_native_observed" && e.data["type"] == "native_stopped")
        .unwrap();
    let revoked = tied
        .history
        .iter()
        .position(|e| {
            e.kind == "provenance"
                && matches!(
                    serde_json::from_value::<ProvenanceEvent>(e.data.clone()).unwrap(),
                    ProvenanceEvent::GrantRevoked { .. }
                )
        })
        .unwrap();
    let stop_seq = tied.history[stop].seq;
    tied.history[stop].seq = tied.history[revoked].seq;
    tied.history[revoked].seq = stop_seq;
    tied.history.sort_by_key(|e| e.seq);
    let reversed = partial_protocol::project(&tied, &native, &aliases).unwrap();
    assert_eq!(reversed.events[1]["type"], "grant_revoked");
    rejected(&directory, "partial-usage", &reversed);
    let mut extra = [("s1".into(), original.clone())]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    duplicate(&mut extra, guarantee.clone());
    rejected(
        &directory,
        "partial-usage",
        &partial_protocol::project(&extra["s1"], &native, &aliases).unwrap(),
    );
    let mut duplicate_grant = [("s1".into(), original.clone())]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let grant = original
        .history
        .iter()
        .find(|event| {
            event.kind == "provenance"
                && matches!(
                    serde_json::from_value::<ProvenanceEvent>(event.data.clone()).unwrap(),
                    ProvenanceEvent::GrantRevoked { .. }
                )
        })
        .unwrap()
        .clone();
    duplicate(&mut duplicate_grant, grant);
    rejected(
        &directory,
        "partial-usage",
        &partial_protocol::project(&duplicate_grant["s1"], &native, &aliases).unwrap(),
    );
    let mut ambiguous = native.clone();
    ambiguous.swap(0, 1);
    for (index, row) in ambiguous.iter_mut().enumerate() {
        row["seq"] = json!(index + 1);
    }
    assert!(partial_protocol::project(&original, &ambiguous, &aliases).is_err());
    let mut corrupt = native.clone();
    corrupt[0]["agent_id"] = json!("invented-agent");
    assert!(partial_protocol::project(&original, &corrupt, &aliases).is_err());
}

//! No installed native provider is executed by these consumer integration tests.
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_owned()
}
fn manifest(root: &Path) -> Value {
    let result = Command::new("python3")
        .args([
            "-B",
            repo()
                .join("ymp-evals/weak-pilot/consumer_manifest.py")
                .to_str()
                .unwrap(),
            "--runner",
            env!("CARGO_BIN_EXE_ymp-weak-pilot"),
            "--output",
            root.join("controller").to_str().unwrap(),
            "--workspaces",
            root.join("workspaces").to_str().unwrap(),
            "--phase",
            "protocol-e2e",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}
fn launch(root: &Path, manifest: &Value, command: &str) -> Output {
    let path = root.join("manifest.json");
    fs::write(&path, serde_json::to_vec(manifest).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_ymp-weak-pilot"))
        .args([command, "--manifest", path.to_str().unwrap()])
        .output()
        .unwrap()
}

#[test]
fn all_six_conditions_consume_real_task_exports_and_freeze_one_blind_result() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let spec = manifest(&root);
    let output = launch(&root, &spec, "scripted");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let report: Value =
        serde_json::from_slice(&fs::read(root.join("controller/run.json")).unwrap()).unwrap();
    assert_eq!(report["complete"], true);
    assert_eq!(report["execution_kind"], "protocol-fixture");
    assert_eq!(report["native_measurements"], false);
    let outcomes = report["outcomes"].as_array().unwrap();
    assert_eq!(outcomes.len(), 12);
    for row in outcomes {
        assert_eq!(row["objective_success"], true, "{row}");
        assert_eq!(row["interpretable"], true, "{row}");
        assert_eq!(row["external_score"]["scope"], "artifact_only");
        assert!(row["external_score"].get("condition").is_none());
        if row["condition"] == "cooperation-3" {
            assert_eq!(row["runtime"]["actual_participant_count"], 3);
        }
        if row["condition"] == "cooperation-2" {
            assert_eq!(row["runtime"]["actual_participant_count"], 2);
        }
        let id = row["attempt_id"].as_str().unwrap();
        let requests = fs::read_to_string(
            root.join("controller")
                .join(id)
                .join("protocol-requests.jsonl"),
        )
        .unwrap();
        for line in requests.lines() {
            let request: Value = serde_json::from_str(line).unwrap();
            if request["method"] == "thread/start" || request["method"] == "thread/resume" {
                assert_eq!(request["legacy_sandbox_present"], false);
                assert!(request["permissions"].is_string());
            }
            if request["method"] == "turn/start" {
                assert_eq!(request["effort"], "low");
                let hidden = repo()
                    .join("ymp-evals/weak-pilot/fixtures")
                    .join(row["variant"].as_str().unwrap())
                    .join(row["task"].as_str().unwrap())
                    .join("private");
                assert!(!request["input"]
                    .to_string()
                    .contains(hidden.to_str().unwrap()));
                if request["purpose"] == "pilot_candidate" {
                    assert_eq!(request["input"][0]["text"], spec["task_prompt"]);
                }
            }
        }
    }
}

#[test]
fn approval_kind_and_source_drift_fail_before_any_provider() {
    for mode in [
        "native",
        "wrong-kind",
        "source-drift",
        "fixture-to-native-program",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut spec = manifest(&root);
        let command = if mode == "native" {
            "native"
        } else {
            "scripted"
        };
        if mode == "wrong-kind" {
            spec["execution_kind"] = json!("native");
        }
        if mode == "source-drift" {
            spec["frozen_files"]["ymp-evals/weak-pilot/observer.py"] = json!("incorrect");
        }
        if mode == "fixture-to-native-program" {
            spec["codex"] = json!("/usr/bin/false");
        }
        let result = launch(&root, &spec, command);
        assert!(!result.status.success(), "{mode}");
        assert!(!root.join("controller").exists(), "{mode}");
    }
}

#[test]
fn unknown_usage_and_deadline_stop_the_same_consumer_without_losing_observations() {
    for fault in [
        "unknown-usage",
        "pending",
        "provider-error",
        "broken-envelope",
        "contradictory-reported",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut spec = manifest(&root);
        spec["attempts"] = json!([{"id":"first","condition":"weak-solo","task":"reconcile","variant":"preparation","blind_id":"00000000000000000000000000000001","fixture_fault":fault},{"id":"second","condition":"weak-solo","task":"repair","variant":"preparation","blind_id":"00000000000000000000000000000002"}]);
        if fault == "pending" {
            set_seconds(&mut spec, 2);
        }
        let result = launch(&root, &spec, "scripted");
        assert!(!result.status.success(), "{fault}");
        let report: Value =
            serde_json::from_slice(&fs::read(root.join("controller/run.json")).unwrap()).unwrap();
        assert_eq!(report["complete"], false);
        assert_eq!(report["outcomes"][1]["status"], "not_started");
        let trace: Value = serde_json::from_slice(
            &fs::read(root.join("controller/first/group/trace.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(trace["invocations"].as_array().unwrap().len(), 1);
        if ["pending", "provider-error"].contains(&fault) {
            assert_eq!(trace["usage"]["total"]["counts"]["input"], 80);
            assert_eq!(trace["usage"]["total"]["counts"]["output"], 20);
        }
    }
}

fn set_seconds(spec: &mut Value, seconds: u64) {
    spec["group_seconds"] = json!(seconds);
    spec["config"]["limits"]["turn_timeout_secs"] = json!(seconds);
}

fn paired(root: &Path, condition: &str, fault: &str) -> Value {
    let mut spec = manifest(root);
    spec["attempts"] = json!([
        {"id":"first","condition":condition,"task":"reconcile","variant":"preparation","blind_id":"00000000000000000000000000000001","fixture_fault":fault},
        {"id":"second","condition":"weak-solo","task":"repair","variant":"preparation","blind_id":"00000000000000000000000000000002"}]);
    spec
}

#[test]
fn ordinary_negative_results_do_not_gate_calibration_or_later_conditions() {
    for (condition, fault) in [
        ("weak-solo", "wrong-answer"),
        ("weak-solo", "missing-deliverable"),
        ("cooperation-2", "negative-final-review"),
        ("independent-2", "known-budget"),
        ("weak-solo", "deadline-after-completed"),
        ("cooperation-3", "underused-roster"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut spec = paired(&root, condition, fault);
        if fault == "known-budget" {
            spec["config"]["limits"]["resources"]["observed_tokens"] = json!(110);
            spec["config"]["limits"]["resources"]["invocation_tokens"] = json!(60);
            spec["config"]["limits"]["resources"]["review_reserve_tokens"] = json!(60);
        }
        if fault == "deadline-after-completed" {
            set_seconds(&mut spec, 4);
        }
        let result = launch(&root, &spec, "scripted");
        let report: Value =
            serde_json::from_slice(&fs::read(root.join("controller/run.json")).unwrap()).unwrap();
        assert!(
            result.status.success(),
            "{fault}: {}\n{report}",
            String::from_utf8_lossy(&result.stderr)
        );
        let first = &report["outcomes"][0];
        assert_eq!(first["measurement_valid"], true, "{fault}: {first}");
        assert_eq!(
            report["outcomes"][1]["status"], "completed",
            "{fault}: {report}"
        );
        assert_eq!(report["calibration_allows_pilot"], true, "{fault}");
        if fault == "underused-roster" {
            assert_eq!(first["runtime"]["requested_participant_count"], 3);
            assert_eq!(first["runtime"]["actual_participant_count"], 2);
        } else {
            assert_eq!(first["task_outcome"], "unsuccessful", "{fault}: {first}");
        }
        if fault == "wrong-answer" {
            assert_eq!(first["objective_success"], false);
        }
        if fault == "known-budget" {
            assert_eq!(first["runtime"]["usage"]["total"]["counts"]["input"], 80);
            assert_eq!(first["runtime"]["actual_participant_count"], 1);
        }
    }
}

#[test]
fn optional_reported_settings_follow_the_same_rule_in_all_conditions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let mut spec = manifest(&root);
    for row in spec["attempts"].as_array_mut().unwrap() {
        row["fixture_fault"] = json!("missing-reported");
    }
    let result = launch(&root, &spec, "scripted");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value =
        serde_json::from_slice(&fs::read(root.join("controller/run.json")).unwrap()).unwrap();
    for row in report["outcomes"].as_array().unwrap() {
        assert_eq!(row["measurement_valid"], true, "{row}");
        assert_eq!(row["metadata_complete"], false);
        for record in row["runtime"]["settings_observations"].as_array().unwrap() {
            assert!(record["reported"]["model"].is_null());
            assert!(record["reported"]["effort"].is_null());
            assert_eq!(record["requested"]["effort"], "low");
            assert_eq!(record["sent"]["effort"], "low");
        }
    }
}

#[test]
fn solo_uses_whole_remaining_time_and_independent_members_do_not_reset_it() {
    for condition in ["weak-solo", "independent-2"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut spec = paired(&root, condition, "slow-complete");
        set_seconds(&mut spec, 12);
        spec["config"]["limits"]["resources"]["invocation_tokens"] = json!(40);
        let result = launch(&root, &spec, "scripted");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let trace: Value = serde_json::from_slice(
            &fs::read(root.join("controller/first/group/trace.json")).unwrap(),
        )
        .unwrap();
        let assignments = trace["assignments"].as_array().unwrap();
        assert!(assignments[0]["timeout_secs"].as_u64().unwrap() > 1);
        assert_eq!(
            assignments.len(),
            if condition == "weak-solo" { 1 } else { 2 }
        );
        if assignments.len() == 2 {
            assert!(
                assignments[1]["timeout_secs"].as_u64().unwrap()
                    < assignments[0]["timeout_secs"].as_u64().unwrap()
            );
        }
        assert!(trace["usage"]["total"]["counts"]["input"].as_u64().unwrap() > 40);
        let mut invalid = spec.clone();
        invalid["config"]["limits"]["turn_timeout_secs"] = json!(1);
        invalid["output"] = json!(root.join("invalid-controller"));
        invalid["workspace_root"] = json!(root.join("invalid-work"));
        let refused = launch(&root, &invalid, "scripted");
        assert!(!refused.status.success());
        assert!(!root.join("invalid-controller").exists());
    }
}

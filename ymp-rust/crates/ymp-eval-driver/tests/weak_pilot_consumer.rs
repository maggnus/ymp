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
                assert!(!request["input"].to_string().contains("private/"));
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
    for fault in ["unknown-usage", "pending", "provider-error"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut spec = manifest(&root);
        spec["attempts"] = json!([{"id":"first","condition":"weak-solo","task":"reconcile","variant":"preparation","blind_id":"00000000000000000000000000000001","fixture_fault":fault},{"id":"second","condition":"weak-solo","task":"repair","variant":"preparation","blind_id":"00000000000000000000000000000002"}]);
        if fault == "pending" {
            spec["group_seconds"] = json!(2);
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
        if fault != "unknown-usage" {
            assert_eq!(trace["usage"]["total"]["counts"]["input"], 80);
            assert_eq!(trace["usage"]["total"]["counts"]["output"], 20);
        }
    }
}

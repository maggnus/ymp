//! No installed native provider is executed by these consumer integration tests.
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::OnceLock,
};

const ACCEPTED_PRODUCT_BASE: &str = "1c17f4e447b839e20f8efbdee0900963a5d8fcad";
const PRODUCT_PATHS: [&str; 10] = [
    "Cargo.toml",
    "Cargo.lock",
    "ymp-bridges",
    "ymp-rust/crates/ymp-core",
    "ymp-rust/crates/ymp-storage",
    "ymp-rust/crates/ymp-providers",
    "ymp-rust/crates/ymp-runtime",
    "ymp-rust/crates/ymp-cli",
    "ymp-rust/crates/ymp-tui",
    "ymp-rust/crates/ymp-workspace",
];
const CONSUMER_PATHS: [&str; 3] = [
    "ymp-evals/weak-pilot",
    "ymp-evals/validators",
    "ymp-rust/crates/ymp-eval-driver/src/bin",
];

fn workspace_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_owned()
}

struct FrozenRepository {
    _temp: tempfile::TempDir,
    root: PathBuf,
}

impl FrozenRepository {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let source = workspace_repo();
        let revision = git_stdout(&source, &["rev-parse", "HEAD"]);
        git_ok(
            &source,
            &[
                "clone",
                "--quiet",
                "--shared",
                "--no-checkout",
                source.to_str().unwrap(),
                root.to_str().unwrap(),
            ],
        );
        git_ok(
            &root,
            &["checkout", "--quiet", "--detach", ACCEPTED_PRODUCT_BASE],
        );
        let mut clear_consumer = vec!["rm", "-r", "-f", "--ignore-unmatch", "--"];
        clear_consumer.extend(CONSUMER_PATHS);
        git_ok(&root, &clear_consumer);
        let mut restore_consumer = vec!["checkout", &revision, "--"];
        restore_consumer.extend(CONSUMER_PATHS);
        git_ok(&root, &restore_consumer);
        let mut diff = vec!["diff", "--quiet", ACCEPTED_PRODUCT_BASE, "--"];
        diff.extend(PRODUCT_PATHS);
        git_ok(&root, &diff);
        Self { _temp: temp, root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

fn git_ok(repository: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repository)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout(repository: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repository)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn sha256(path: &Path) -> String {
    let output = Command::new("python3")
        .args([
            "-B",
            "-c",
            "import hashlib, pathlib, sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn frozen_repo() -> &'static Path {
    static REPOSITORY: OnceLock<FrozenRepository> = OnceLock::new();
    REPOSITORY.get_or_init(FrozenRepository::new).path()
}

fn manifest_from(repository: &Path, root: &Path) -> Value {
    let result = Command::new("python3")
        .args([
            "-B",
            repository
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

fn manifest(root: &Path) -> Value {
    manifest_from(frozen_repo(), root)
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
                let hidden = frozen_repo()
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
fn one_manifest_bound_product_file_change_is_rejected_before_fixture_work() {
    let repository = FrozenRepository::new();
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let spec = manifest_from(repository.path(), &root);
    let changed = repository
        .path()
        .join("ymp-rust/crates/ymp-core/src/lib.rs");
    let mut bytes = fs::read(&changed).unwrap();
    bytes.extend_from_slice(b"\n// YMP-161 manifest-bound product mutation control.\n");
    fs::write(&changed, bytes).unwrap();
    let changed_paths = git_stdout(
        repository.path(),
        &[
            "diff",
            "--name-only",
            ACCEPTED_PRODUCT_BASE,
            "--",
            "ymp-rust/crates/ymp-core",
        ],
    );
    assert_eq!(changed_paths, "ymp-rust/crates/ymp-core/src/lib.rs");

    let result = launch(&root, &spec, "scripted");
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("Product bytes differ from accepted P0 base 1c17f4e"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!root.join("controller").exists());
}

#[test]
fn exact_old_accepted_manifests_remain_inactive_for_the_current_runner() {
    let current_runner = sha256(Path::new(env!("CARGO_BIN_EXE_ymp-weak-pilot")));
    for (relative, expected_manifest, expected_runner) in [
        (
            "ymp-docs/evidence/ymp-201/consumer-v2/calibration-manifest.proposal.json",
            "34993fee70abbffb0db35a77277b42b4d22dc3c7ec6bb77ced8c05e33f70c076",
            "ecb46cb19cb6d22017be05ef52d386307d76b8d0766c2f758c581bd4b2201f46",
        ),
        (
            "ymp-docs/evidence/ymp-201/consumer-v2/pilot-manifest.proposal.json",
            "a3eaadab9d484f3d2adabaf486a959f58b49dd2c333490d2bebe6d1053fd282a",
            "ecb46cb19cb6d22017be05ef52d386307d76b8d0766c2f758c581bd4b2201f46",
        ),
        (
            "ymp-docs/evidence/ymp-201/proposed-run/calibration-manifest.proposal.json",
            "b21ad8dc537fe9c11244b43293c6162cf5c774d328fc47c0be9762310386b179",
            "4790774e92090df62a62083c471a05e96f855efd25862577ca4266cc918a92bc",
        ),
        (
            "ymp-docs/evidence/ymp-201/proposed-run/pilot-manifest.proposal.json",
            "dffb4e86c30afe58da32564e474064793aab0c88208bd757f7cc4c41d295f98b",
            "4790774e92090df62a62083c471a05e96f855efd25862577ca4266cc918a92bc",
        ),
        (
            "ymp-docs/evidence/ymp-201/rework-round1/calibration-manifest.proposal.json",
            "b21ad8dc537fe9c11244b43293c6162cf5c774d328fc47c0be9762310386b179",
            "4790774e92090df62a62083c471a05e96f855efd25862577ca4266cc918a92bc",
        ),
        (
            "ymp-docs/evidence/ymp-201/rework-round1/pilot-manifest.proposal.json",
            "dffb4e86c30afe58da32564e474064793aab0c88208bd757f7cc4c41d295f98b",
            "4790774e92090df62a62083c471a05e96f855efd25862577ca4266cc918a92bc",
        ),
    ] {
        let path = workspace_repo().join(relative);
        assert_eq!(sha256(&path), expected_manifest);
        let spec: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(spec["runner_sha256"], expected_runner);
        assert_ne!(spec["runner_sha256"], current_runner);
    }
}

#[test]
fn unapproved_native_manifest_stops_before_provider_work() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let mut spec = manifest(&root);
    let sentinel = root.join("provider-started");
    let provider = root.join("provider-must-not-run");
    fs::write(
        &provider,
        format!(
            "#!/bin/sh\nprintf started > {}\nexit 91\n",
            sentinel.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&provider, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let provider_sha256 = sha256(&provider);
    let evidence = root.join("control-evidence.json");
    fs::write(
        &evidence,
        serde_json::to_vec(&json!({
            "inference_calls": 0,
            "pass": true,
            "binary_sha256": provider_sha256.clone(),
        }))
        .unwrap(),
    )
    .unwrap();

    spec["execution_kind"] = json!("native");
    spec["codex"] = json!(provider);
    spec["codex_sha256"] = json!(provider_sha256);
    spec["native_home"] = json!(root.join("artificial-native-home"));
    spec["protected_roots"] = json!([root.join("artificial-protected-root")]);
    spec["control_evidence"] = json!(evidence.clone());
    spec["control_evidence_sha256"] = json!(sha256(&evidence));
    spec["weak_model"] = json!("native-weak-control");
    spec["strong_model"] = json!("native-strong-control");
    spec["config"]["providers"][0]["command"] = spec["codex"].clone();
    for agent in spec["config"]["agents"].as_array_mut().unwrap() {
        let model = if agent["id"] == "strong-1" {
            "native-strong-control"
        } else {
            "native-weak-control"
        };
        agent["name"] = json!(model);
        agent["model"] = json!(model);
    }
    for (id, execution) in spec["config"]["execution"].as_object_mut().unwrap() {
        execution["fixed"]["model"] = json!(if id == "strong-1" {
            "native-strong-control"
        } else {
            "native-weak-control"
        });
    }
    for model in spec["catalog"]["models"].as_array_mut().unwrap() {
        let id = if model["id"] == "fixture-strong" {
            "native-strong-control"
        } else {
            "native-weak-control"
        };
        model["id"] = json!(id);
        model["picker_id"] = json!(id);
    }
    spec["catalog"]["source"]["method"] = json!("scripted no-inference control");
    spec["config"]["capabilities"]["codex"] = spec["catalog"].clone();

    let result = launch(&root, &spec, "native");
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("owner_approval_required: no measured launch is authorized"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!root.join("controller").exists());
    assert!(!root.join("test-approval-ledger").exists());
    assert!(!sentinel.exists());
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

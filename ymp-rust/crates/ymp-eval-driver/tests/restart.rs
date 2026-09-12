//! A clean Cargo invocation builds the real driver for this test. The exporter
//! under test is included verbatim, then fed mutations of actual crash records.
#[path = "../src/restart_projection.rs"]
mod restart_projection;
use serde_json::{json, Value};
use ymp_core::*;
fn read(path: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
#[test]
fn real_process_recovery_projection_preserves_extra_closure_revocation_and_order() {
    let temp = tempfile::Builder::new()
        .prefix("ymp-restart-control-")
        .tempdir_in("/tmp")
        .unwrap();
    let output = temp.path().join("run");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_ymp-eval-driver"))
        .arg("--output")
        .arg(&output)
        .args(["--case", "restart-inspection"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let case = output.join("restart-inspection");
    let trace: SessionTrace = serde_json::from_value(read(&case.join("runtime.json"))).unwrap();
    let before: SessionTrace =
        serde_json::from_value(read(&case.join("before-crash-runtime.json"))).unwrap();
    let baseline = restart_projection::project(&trace, &before).unwrap();
    assert_eq!(baseline.0, read(&case.join("observed.json")));
    assert_eq!(
        baseline.1["projection_sources"].as_array().unwrap().len(),
        9
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let rejected = |mutant: &SessionTrace| {
        let (observed, sources) = restart_projection::project(mutant, &before).unwrap();
        assert_eq!(
            observed["events"].as_array().unwrap().len(),
            sources["projection_sources"].as_array().unwrap().len()
        );
        let path = case.join("mutant.json");
        std::fs::write(&path, serde_json::to_vec_pretty(&observed).unwrap()).unwrap();
        let verdict = std::process::Command::new("python3")
            .arg(root.join("ymp-evals/validators/universal.py"))
            .args(["protocol", "--case", "restart-inspection", "--workdir"])
            .arg(case.join("work"))
            .arg("--observed")
            .arg(path)
            .output()
            .unwrap();
        assert!(
            !verdict.status.success(),
            "Mutated actual projection passed: {observed}"
        );
        observed
    };
    for kind in [
        "eval_recovery_native_closed",
        "grant_revoked",
        "invocation_finished",
    ] {
        let mut mutant = trace.clone();
        let selected = mutant
            .history
            .iter()
            .rev()
            .find(|e| e.kind == kind || (e.kind == "provenance" && e.data["change"] == kind))
            .unwrap()
            .clone();
        let seq = selected.seq;
        for e in &mut mutant.history {
            if e.seq > seq {
                e.seq += 1;
            }
        }
        let mut extra = selected;
        extra.seq += 1;
        mutant.history.push(extra);
        mutant.history.sort_by_key(|e| e.seq);
        assert_eq!(rejected(&mutant)["events"].as_array().unwrap().len(), 10);
    }
    let mut reordered = trace.clone();
    let closed = reordered
        .history
        .iter()
        .find(|e| e.kind == "eval_recovery_native_closed")
        .unwrap()
        .seq;
    let inspected = reordered
        .history
        .iter()
        .find(|e| e.kind == "eval_recovery_inspected")
        .unwrap()
        .seq;
    for e in &mut reordered.history {
        if e.seq == closed {
            e.seq = inspected
        } else if e.seq == inspected {
            e.seq = closed
        }
        e.created_at = "2020-01-01T00:00:00Z".into();
    }
    reordered.history.sort_by_key(|e| e.seq);
    rejected(&reordered);
    let process = read(&case.join("processes.json"));
    assert_ne!(process["seed_pid"], process["inspection_pid"]);
    assert_eq!(
        read(&case.join("observed.json"))["final_state"]["reported_spent_units"],
        json!(50)
    );
}

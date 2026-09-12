use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_providers::{run_turn, ProviderEvent, TurnRequest};

async fn run(
    kind: &str,
    model: Option<&str>,
    effort: Option<&str>,
    variant: &str,
) -> (
    anyhow::Result<ymp_providers::TurnResult>,
    Vec<Value>,
    Vec<ProviderEvent>,
) {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("wire.jsonl");
    let req: TurnRequest = serde_json::from_value(json!({
        "profile":{"id":"agent", "name":"agent","provider":"fixture","model":"stale-profile", "instructions":""},
        "provider":{"id":"fixture","kind":kind,"command":"python3","args":[format!("{}/tests/fixtures/settings.py",env!("CARGO_MANIFEST_DIR")),kind,log,variant]},
        "settings":{"model":model,"effort":effort,"permission_mode":null},
        "cwd":dir.path(),"prompt":"request","purpose":"execute","read_only":true,"resume":null,"usage_baseline":null,"mcp":null,"timeout_secs":5,"bridge":""
    })).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let result = run_turn(req, CancellationToken::new(), tx).await;
    let wire = std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let mut events = vec![];
    while let Ok(e) = rx.try_recv() {
        events.push(e);
    }
    (result, wire, events)
}

#[tokio::test]
async fn codex_sends_assignment_model_and_open_catalog_effort_to_the_turn() {
    let (r, w, e) = run("codex", Some("model-b"), Some("future-level"), "ok").await;
    r.unwrap();
    let t = w.iter().find(|q| q["method"] == "turn/start").unwrap();
    assert_eq!(t["params"]["model"], "model-b");
    assert_eq!(t["params"]["effort"], "future-level");
    assert!(e.iter().any(|e| matches!(e,ProviderEvent::Execution(o) if o.sent.as_ref().is_some_and(|s|s.effort.as_deref()==Some("future-level")))));
}
#[tokio::test]
async fn codex_rejects_unadvertised_effort_before_prompt() {
    let (r, w, _) = run("codex", Some("model-b"), Some("made-up"), "ok").await;
    assert!(r.is_err());
    assert!(!w.iter().any(|q| q["method"] == "turn/start"));
}
#[tokio::test]
async fn acp_uses_refreshed_model_controls_and_checks_acknowledgement() {
    let (r, w, _) = run("acp", Some("model-b"), Some("none"), "ok").await;
    r.unwrap();
    let t = w
        .iter()
        .find(|q| q["method"] == "session/set_config_option")
        .unwrap();
    assert_eq!(t["params"]["value"], "none");
    assert!(
        w.iter()
            .position(|q| q["method"] == "session/set_model")
            .unwrap()
            < w.iter()
                .position(|q| q["method"] == "session/set_config_option")
                .unwrap()
    );
    for (effort, variant) in [("max", "ok"), ("none", "clamp")] {
        let (r, w, _) = run("acp", Some("model-b"), Some(effort), variant).await;
        assert!(r.is_err());
        assert!(!w.iter().any(|q| q["method"] == "session/prompt"));
    }
}

#[tokio::test]
async fn native_defaults_stay_omitted_and_model_drift_is_rejected_before_prompt() {
    let (r, w, e) = run("codex", None, None, "ok").await;
    r.unwrap();
    let turn = w.iter().find(|q| q["method"] == "turn/start").unwrap();
    assert!(turn["params"].get("model").is_none());
    assert!(turn["params"].get("effort").is_none());
    assert!(e.iter().any(|e|matches!(e,ProviderEvent::Execution(o) if o.reported.as_ref().is_some_and(|s|s.model.as_deref()==Some("model-a") && s.effort.as_deref()==Some("high")))));
    let (r, w, _) = run("codex", Some("model-b"), Some("max"), "model-drift").await;
    assert!(r.is_err());
    assert!(!w.iter().any(|q| q["method"] == "turn/start"));
}
#[tokio::test]
async fn acp_unknown_model_can_use_observed_controls_but_stale_controls_cannot_validate_effort() {
    let (r, w, _) = run("acp", Some("custom-model"), Some("none"), "ok").await;
    r.unwrap();
    assert!(w
        .iter()
        .any(|q| q["method"] == "session/set_model" && q["params"]["modelId"] == "custom-model"));
    let (r, w, _) = run("acp", Some("model-b"), Some("none"), "no-refresh").await;
    assert!(r.is_err());
    assert!(!w.iter().any(|q| q["method"] == "session/prompt"));
}

#[tokio::test]
async fn missing_and_explicitly_absent_controls_have_distinct_no_prompt_diagnostics() {
    for (variant, expected) in [
        (
            "no-controls",
            "Model model-b does not support native thought_level control",
        ),
        (
            "no-refresh",
            "Native thought_level support was not observed for model-b",
        ),
    ] {
        let (result, wire, _) = run("acp", Some("model-b"), Some("none"), variant).await;
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert!(
            !wire
                .iter()
                .any(|q| q["method"] == "session/prompt"
                    || q["method"] == "session/set_config_option")
        );
    }
}

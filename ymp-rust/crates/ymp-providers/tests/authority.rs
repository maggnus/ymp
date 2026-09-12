use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_providers::{run_turn, McpEndpoint, ProviderEvent, TurnRequest};

async fn run(
    kind: &str,
    permission: Option<&str>,
    variant: &str,
) -> (
    anyhow::Result<ymp_providers::TurnResult>,
    Vec<Value>,
    Vec<ProviderEvent>,
) {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("wire.jsonl");
    let request: TurnRequest = serde_json::from_value(json!({
        "profile":{"id":"agent","name":"agent","provider":"fixture","instructions":""},
        "provider":{"id":"fixture","kind":kind,"command":"python3","args":[format!("{}/tests/fixtures/settings.py", env!("CARGO_MANIFEST_DIR")),kind,log,variant]},
        "settings":{"permission_mode":permission},"cwd":dir.path(),"prompt":"offline","purpose":"plan","read_only":true,
        "resume":"prior-native-context","mcp":{"command":"ymp-fixture","args":["mcp","--socket","fresh-assignment.sock"],"token":"synthetic-fresh-secret"},"timeout_secs":5,"bridge":""
    })).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let result = run_turn(request, CancellationToken::new(), tx).await;
    let wire = std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let mut events = vec![];
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    (result, wire, events)
}
#[tokio::test]
async fn codex_resume_replaces_saved_mcp_configuration_and_native_sandbox() {
    let (result, wire, _) = run("codex", Some("read_only"), "ok").await;
    result.unwrap();
    let resume = wire
        .iter()
        .find(|v| v["method"] == "thread/resume")
        .unwrap();
    assert_eq!(resume["params"]["threadId"], "prior-native-context");
    assert_eq!(resume["params"]["sandbox"], "read-only");
    assert_eq!(
        resume["params"]["config"]["mcp_servers"]["ymp"]["command"],
        "ymp-fixture"
    );
    assert_eq!(
        resume["params"]["config"]["mcp_servers"]["ymp"]["env_vars"],
        json!(["YMP_MCP_TOKEN"])
    );
    assert!(!resume.to_string().contains("synthetic-fresh-secret"));
}
#[tokio::test]
async fn acp_resume_resets_prior_bypass_mode_before_prompt() {
    let (result, wire, events) = run("acp", Some("read_only"), "ok").await;
    result.unwrap();
    assert!(events.iter().any(|event| matches!(event, ProviderEvent::Execution(observation) if !observation.permission_limitations.is_empty())));
    let load = wire.iter().find(|v| v["method"] == "session/load").unwrap();
    assert_eq!(load["params"]["sessionId"], "prior-native-context");
    assert_eq!(
        load["params"]["mcpServers"][0]["env"][0]["value"],
        "synthetic-fresh-secret"
    );
    let mode = wire
        .iter()
        .position(|v| v["method"] == "session/set_mode")
        .expect("Read assignment must reset previous permission mode");
    assert_eq!(wire[mode]["params"]["modeId"], "default");
    assert!(
        mode < wire
            .iter()
            .position(|v| v["method"] == "session/prompt")
            .unwrap()
    );
}
#[tokio::test]
async fn unsupported_permission_guarantees_and_unsafe_acp_restore_never_prompt() {
    for (kind, permission, variant) in [
        ("codex", "os-contained", "ok"),
        ("acp", "read_only", "unsafe-modes"),
    ] {
        let (result, wire, _) = run(kind, Some(permission), variant).await;
        assert!(
            result.is_err(),
            "Unsupported native guarantee was silently accepted"
        );
        assert!(!wire.iter().any(
            |v| ["turn/start", "session/prompt"].contains(&v["method"].as_str().unwrap_or(""))
        ));
    }
}
#[test]
fn debug_output_never_contains_the_live_capability_secret() {
    let endpoint = McpEndpoint {
        command: "ymp".into(),
        args: vec![],
        token: "synthetic-secret".into(),
    };
    assert!(!format!("{endpoint:?}").contains("synthetic-secret"));
}

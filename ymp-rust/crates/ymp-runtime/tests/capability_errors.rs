use serde_json::json;
use std::time::Duration;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::UnixListener,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use ymp_core::{Config, InvocationState};
use ymp_runtime::Engine;
use ymp_storage::Store;

#[tokio::test]
async fn native_errors_preserve_diagnosis_without_exposing_current_capabilities() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let store = Store::open(&temp.path().join("state")).unwrap();
    // Observe only synthetic capabilities through transient IPC, never a token
    // file, native authentication source, or captured diagnostic output.
    let report_path = temp.path().join("capabilities.sock");
    let reports = UnixListener::bind(&report_path).unwrap();
    let config: Config = serde_json::from_value(json!({
        "version":1,
        "providers":[{"id":"fixture","kind":"codex","command":"python3","args":[
            concat!(env!("CARGO_MANIFEST_DIR"),"/tests/fixtures/native_capability_error.py"),report_path
        ]}],
        "agents":[{"id":"one","name":"One","provider":"fixture"},{"id":"two","name":"Two","provider":"fixture"}],
        "team":["one","two"],"limits":{"turn_timeout_secs":5}
    })).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let engine = Engine::new(store.clone(), config, tx, CancellationToken::new()).unwrap();
    let collect_capabilities = async {
        let mut capabilities = vec![];
        for _ in 0..2 {
            let (stream, _) = reports.accept().await.unwrap();
            let mut capability = String::new();
            BufReader::new(stream)
                .read_line(&mut capability)
                .await
                .unwrap();
            capabilities.push(capability.trim().to_owned());
        }
        capabilities
    };
    let (outcome, capabilities) = tokio::time::timeout(Duration::from_secs(15), async {
        tokio::join!(
            engine.run(&project, "Inspect this complex offline fixture", None),
            collect_capabilities
        )
    })
    .await
    .expect("Offline fixture did not report two assignment capabilities");
    let outcome = outcome.unwrap();
    assert_eq!(outcome.session.status, "blocked");
    assert!(capabilities.iter().all(|value| !value.is_empty()));
    assert!(
        capabilities[0] != capabilities[1],
        "Assignments reused the same capability"
    );
    let session = &outcome.session.id;
    let messages = store.messages(session, 0, 100).unwrap();
    let trace = store.trace(session).unwrap();
    assert_eq!(trace.invocations.len(), 2);
    assert!(trace
        .invocations
        .iter()
        .all(|i| i.state == InvocationState::Failed));
    assert_eq!(
        messages
            .iter()
            .filter(|m| m.text.contains("MCP setup failed"))
            .count(),
        2
    );
    assert!(messages.iter().any(|m| m.text.contains("initialize:")
        && m.text.contains("-32000")
        && m.text.contains("Transport rejected")));
    let mut ui = vec![];
    while let Ok(event) = rx.try_recv() {
        ui.push(format!("{event:?}"));
    }
    for (surface, contents) in [
        ("messages", serde_json::to_string(&messages).unwrap()),
        ("trace", serde_json::to_string(&trace).unwrap()),
        ("UI events", ui.join("\n")),
    ] {
        for capability in &capabilities {
            assert!(
                !contents.contains(capability),
                "Current capability leaked through {surface}"
            );
        }
    }
    for assignment in &trace.assignments {
        assert!(store
            .team_grant(session, &assignment.grant_ids[0])
            .unwrap()
            .revoked_at
            .is_some());
    }
    let reopened = Store::open(&temp.path().join("state")).unwrap();
    let restored = serde_json::to_string(&reopened.trace(session).unwrap()).unwrap();
    assert!(
        capabilities
            .iter()
            .all(|capability| !restored.contains(capability)),
        "Capability persisted across restart"
    );
}

use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::{AgentProfile, ProviderConfig, ProviderKind};
use ymp_providers::{run_turn, ProviderEvent, TurnRequest, TurnResult};

struct Fixture {
    cwd: TempDir,
    request: TurnRequest,
}

impl Fixture {
    fn new(notifications: Vec<Value>) -> Self {
        let cwd = tempfile::tempdir().unwrap();
        std::fs::write(
            cwd.path().join("scenario.json"),
            json!({"notifications": notifications}).to_string(),
        )
        .unwrap();
        let request = TurnRequest {
            profile: AgentProfile {
                id: "agent-fixture".into(),
                name: "Offline fixture".into(),
                provider: "codex-fixture".into(),
                model: None,
                instructions: String::new(),
                enabled: true,
            },
            provider: ProviderConfig {
                id: "codex-fixture".into(),
                kind: ProviderKind::Codex,
                command: "python3".into(),
                args: vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/codex_retry.py")
                    .to_string_lossy()
                    .into_owned()],
                env_refs: Default::default(),
                enabled: true,
            },
            cwd: cwd.path().into(),
            prompt: "Exercise the offline protocol fixture".into(),
            purpose: "execute".into(),
            read_only: false,
            resume: None,
            usage_baseline: None,
            mcp: None,
            timeout_secs: 5,
            bridge: PathBuf::new(),
        };
        Self { cwd, request }
    }

    fn option(&self, name: &str, value: Value) {
        let path = self.cwd.path().join("scenario.json");
        let mut settings: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        settings[name] = value;
        std::fs::write(path, settings.to_string()).unwrap();
    }

    async fn run(&self) -> (anyhow::Result<TurnResult>, Vec<ProviderEvent>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let result = tokio::time::timeout(
            Duration::from_secs(10),
            run_turn(self.request.clone(), CancellationToken::new(), tx),
        )
        .await
        .expect("Offline fixture must finish promptly");
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }
        self.assert_single_invocation();
        (result, events)
    }

    fn assert_single_invocation(&self) {
        let requests = std::fs::read_to_string(self.cwd.path().join("requests.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        let methods = requests
            .iter()
            .map(|request| request["method"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            methods,
            ["initialize", "initialized", "thread/start", "turn/start"]
        );
        assert_eq!(
            std::fs::read_to_string(self.cwd.path().join("effects.log")).unwrap(),
            "turn-started\n"
        );
    }
}

fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

fn error(will_retry: Value) -> Value {
    notification(
        "error",
        json!({
            "threadId": "thread-fixture",
            "turnId": "turn-fixture",
            "willRetry": will_retry,
            "error": {"codexErrorInfo": "serverOverloaded", "message": "Synthetic native retry"}
        }),
    )
}

fn usage(input: u64, output: u64) -> Value {
    notification(
        "thread/tokenUsage/updated",
        json!({
            "threadId": "thread-fixture",
            "turnId": "turn-fixture",
            "tokenUsage": {
                "total": {"inputTokens": input, "outputTokens": output},
                "last": {"inputTokens": input, "outputTokens": output}
            }
        }),
    )
}

fn final_message() -> Value {
    notification(
        "item/completed",
        json!({
            "threadId": "thread-fixture",
            "turnId": "turn-fixture",
            "item": {"type": "agentMessage", "text": "Synthetic completion"}
        }),
    )
}

fn completed(status: &str) -> Value {
    notification(
        "turn/completed",
        json!({"threadId": "thread-fixture", "turn": {"id": "turn-fixture", "status": status}}),
    )
}

#[tokio::test]
async fn codex_native_retry_completes_the_same_invocation() {
    let fixture = Fixture::new(vec![
        usage(10, 2),
        error(json!(true)),
        usage(10, 2),
        error(json!(true)),
        usage(15, 5),
        final_message(),
        completed("completed"),
    ]);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        run_turn(fixture.request.clone(), CancellationToken::new(), tx),
    )
    .await
    .expect("Offline fixture must finish promptly")
    .expect("Native retry notices must not fail the invocation");
    assert_eq!(result.text, "Synthetic completion");
    assert_eq!(result.session_id, "thread-fixture");
    assert_eq!(
        result.usage,
        Some(usage(15, 5)["params"]["tokenUsage"].clone())
    );
    fixture.assert_single_invocation();
    let mut snapshots = Vec::new();
    let mut sessions = Vec::new();
    let mut retries = Vec::new();
    while let Some(event) = rx.recv().await {
        match event {
            ProviderEvent::Usage(snapshot) => snapshots.push(snapshot),
            ProviderEvent::Session(session) => sessions.push(session),
            ProviderEvent::Retry {
                session_id,
                turn_id,
                error_code,
            } => retries.push((session_id, turn_id, error_code)),
            _ => {}
        }
    }
    assert_eq!(sessions, ["thread-fixture"]);
    assert_eq!(
        retries,
        vec![
            (
                "thread-fixture".into(),
                "turn-fixture".into(),
                Some("serverOverloaded".into())
            );
            2
        ]
    );
    assert_eq!(snapshots.len(), 4);
    assert_eq!(
        snapshots
            .iter()
            .map(|snapshot| snapshot.counts.known_total())
            .collect::<Vec<_>>(),
        [Some(12), Some(12), Some(20), Some(20)]
    );
    assert!(snapshots[..3].iter().all(|snapshot| !snapshot.finalized));
    assert!(snapshots[3].finalized);
    assert!(snapshots.iter().all(|snapshot| !snapshot.partial));
}

#[tokio::test]
async fn codex_native_retry_before_start_response_preserves_unknown_usage() {
    let fixture = Fixture::new(vec![
        error(json!(true)),
        final_message(),
        completed("completed"),
    ]);
    fixture.option("before_response", json!(true));
    let (result, events) = fixture.run().await;
    assert_eq!(result.unwrap().usage, None);
    assert!(events
        .iter()
        .any(|e| matches!(e, ProviderEvent::Retry { .. })));
    assert!(!events.iter().any(|e| matches!(e, ProviderEvent::Usage(_))));
}

#[tokio::test]
async fn codex_native_retry_requires_an_explicit_boolean_true() {
    let mut missing = error(json!(true));
    missing["params"]
        .as_object_mut()
        .unwrap()
        .remove("willRetry");
    for terminal in [
        error(json!(false)),
        error(Value::Null),
        error(json!("true")),
        missing,
    ] {
        let fixture = Fixture::new(vec![terminal, final_message(), completed("completed")]);
        let (result, events) = fixture.run().await;
        assert!(result.unwrap_err().to_string().starts_with("Codex error:"));
        assert!(!events
            .iter()
            .any(|e| matches!(e, ProviderEvent::Retry { .. })));
    }
}

#[tokio::test]
async fn codex_native_retry_can_still_end_in_terminal_failure() {
    for (terminal, expected) in [
        (error(json!(false)), "Codex error:"),
        (completed("failed"), "Codex turn did not complete:"),
        (completed("interrupted"), "Codex turn did not complete:"),
    ] {
        let fixture = Fixture::new(vec![
            usage(10, 2),
            error(json!(true)),
            terminal,
            final_message(),
            completed("completed"),
        ]);
        let (result, events) = fixture.run().await;
        assert!(result.unwrap_err().to_string().starts_with(expected));
        assert_pending_retry_evidence(&events);
    }
}

#[tokio::test]
async fn codex_native_retry_requires_completion_and_final_text() {
    for tail in [vec![], vec![final_message()], vec![completed("completed")]] {
        let mut notifications = vec![error(json!(true))];
        notifications.extend(tail);
        let fixture = Fixture::new(notifications);
        let (result, _) = fixture.run().await;
        assert!(result.is_err(), "Retry evidence is not a successful result");
    }
}

#[tokio::test]
async fn codex_native_retry_filters_other_threads_and_turns() {
    let mut wrong_thread = error(json!(true));
    wrong_thread["params"]["threadId"] = json!("another-thread");
    let mut wrong_turn = error(json!(false));
    wrong_turn["params"]["turnId"] = json!("another-turn");
    let fixture = Fixture::new(vec![
        wrong_thread,
        wrong_turn,
        error(json!(true)),
        final_message(),
        completed("completed"),
    ]);
    let (result, events) = fixture.run().await;
    result.unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ProviderEvent::Retry { .. }))
            .count(),
        1
    );
}

#[tokio::test]
async fn codex_native_retry_keeps_only_known_error_codes() {
    let mut structured = error(json!(true));
    structured["params"]["error"] = json!({
        "message": "sensitive-fixture-detail",
        "additionalDetails": "sensitive-fixture-detail",
        "codexErrorInfo": {"responseStreamDisconnected": {"httpStatusCode": 503}}
    });
    let mut unknown = structured.clone();
    unknown["params"]["error"]["codexErrorInfo"] = json!("sensitive-fixture-detail");
    let fixture = Fixture::new(vec![
        structured,
        unknown,
        final_message(),
        completed("completed"),
    ]);
    let (result, events) = fixture.run().await;
    result.unwrap();
    let codes = events
        .iter()
        .filter_map(|event| match event {
            ProviderEvent::Retry { error_code, .. } => Some(error_code.as_deref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(codes, [Some("responseStreamDisconnected"), None]);
    assert!(!format!("{events:?}").contains("sensitive-fixture-detail"));
}

fn assert_pending_retry_evidence(events: &[ProviderEvent]) {
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ProviderEvent::Retry { .. }))
            .count(),
        1
    );
    let snapshots = events
        .iter()
        .filter_map(|event| match event {
            ProviderEvent::Usage(snapshot) => Some(snapshot),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].counts.known_total(), Some(12));
    assert!(!snapshots[0].finalized);
}

#[tokio::test]
async fn codex_native_retry_does_not_disable_timeout() {
    let mut fixture = Fixture::new(vec![usage(10, 2), error(json!(true))]);
    fixture.option("exit_after_notifications", json!(false));
    fixture.request.timeout_secs = 1;
    let (result, events) = fixture.run().await;
    assert_eq!(
        result.unwrap_err().to_string(),
        "Provider turn timed out; outcome may be incomplete"
    );
    assert_pending_retry_evidence(&events);
}

#[tokio::test]
async fn codex_native_retry_does_not_disable_cancellation() {
    let fixture = Fixture::new(vec![usage(10, 2), error(json!(true))]);
    fixture.option("exit_after_notifications", json!(false));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();
    let job = tokio::spawn(run_turn(fixture.request.clone(), cancel.clone(), tx));
    let mut events = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = rx
                .recv()
                .await
                .expect("Retry must be observed before cancellation");
            let retry = matches!(event, ProviderEvent::Retry { .. });
            events.push(event);
            if retry {
                break;
            }
        }
    })
    .await
    .expect("Fixture must reach the native retry");
    cancel.cancel();
    let result = tokio::time::timeout(Duration::from_secs(2), job)
        .await
        .expect("Cancellation must stop a pending retry promptly")
        .unwrap();
    assert_eq!(
        result.unwrap_err().to_string(),
        "Turn cancelled; process resources released"
    );
    while let Some(event) = rx.recv().await {
        events.push(event);
    }
    assert_pending_retry_evidence(&events);
    fixture.assert_single_invocation();
}

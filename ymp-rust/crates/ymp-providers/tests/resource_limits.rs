use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_providers::{run_turn, ProviderEvent, TurnRequest};

fn request() -> TurnRequest {
    serde_json::from_value(serde_json::json!({
        "profile": {"id":"agent", "name":"agent", "provider":"mock", "model":null, "instructions":"[mock:usage]"},
        "provider": {"id":"mock", "kind":"mock", "command":"internal"},
        "cwd":std::env::temp_dir(), "prompt":"inspect", "purpose":"review", "read_only":true,
        "resume":null, "mcp":null, "timeout_secs":10, "bridge":"unused"
    })).unwrap()
}
#[tokio::test]
async fn output_limit_stops_the_provider_path_and_keeps_final_usage() {
    let mut req = request();
    req.resource_controls.max_output_chars = Some(1);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let error = run_turn(req, CancellationToken::new(), tx)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("output_limit"));
    let mut last = None;
    while let Ok(event) = rx.try_recv() {
        if let ProviderEvent::Usage(usage) = event {
            last = Some(usage);
        }
    }
    let usage = last.unwrap();
    assert_eq!(usage.counts.known_total(), Some(120));
    assert!(usage.finalized);
}
#[tokio::test]
async fn timeout_also_bounds_mock_backend_and_keeps_partial_usage() {
    let mut req = request();
    req.timeout_secs = 0;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let error = run_turn(req, CancellationToken::new(), tx)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("timed out"));
    let mut partial = None;
    while let Ok(event) = rx.try_recv() {
        if let ProviderEvent::Usage(usage) = event {
            partial = Some(usage);
        }
    }
    assert_eq!(partial.unwrap().counts.input, Some(100));
}

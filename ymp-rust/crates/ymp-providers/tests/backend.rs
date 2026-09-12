use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_providers::{
    run_turn_with_backend, ExecutionBackend, ExecutionBackendIdentity, ExecutionFuture,
    ProviderEvent, TurnRequest, TurnResult,
};

struct MustNotExecute(AtomicUsize);
impl ExecutionBackend for MustNotExecute {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "example.unreachable".into(),
            version: "1".into(),
        }
    }
    fn execute(
        &self,
        _: TurnRequest,
        _: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Ok(TurnResult {
                text: "invalid request ran".into(),
                session_id: "invalid".into(),
                usage: None,
            })
        })
    }
}

#[tokio::test]
async fn backend_common_validation_rejects_invalid_settings_and_permission_mismatch() {
    let backend = MustNotExecute(AtomicUsize::new(0));
    for (settings, expected) in [
        (serde_json::json!({"model":""}), "model"),
        (serde_json::json!({"permission_mode":"write"}), "permission"),
        (serde_json::json!({"permission_mode":"admin"}), "permission"),
    ] {
        let request: TurnRequest = serde_json::from_value(serde_json::json!({
            "profile":{"id":"a","name":"a","provider":"mock","model":null},
            "provider":{"id":"mock","kind":"mock","command":"internal"},
            "cwd":std::env::temp_dir(), "prompt":"Inspect", "purpose":"review", "read_only":true,
            "resume":null,"mcp":null,"timeout_secs":1,"bridge":"unused", "settings":settings
        }))
        .unwrap();
        let (events, _rx) = mpsc::unbounded_channel();
        let error = run_turn_with_backend(&backend, request, CancellationToken::new(), events)
            .await
            .unwrap_err();
        assert!(
            error.to_string().to_lowercase().contains(expected),
            "{error:#}"
        );
        assert_eq!(backend.0.load(Ordering::SeqCst), 0);
    }
}

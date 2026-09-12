# Replaceable execution backends

YMP-122 supplies the first compiled extension described in [subsystem interfaces](subsystem-interfaces.md). `ymp_providers::ExecutionBackend` provides `identity()` (an ID and version), `execute(TurnRequest, Sender<ProviderEvent>)` (a boxed `Send` future yielding `TurnResult`), and `workspace_access(&TurnRequest)` (the access the backend actually enforces, with a conservative default). There is no store, assignment-commit API or scheduler in that interface. Runtime scheduling may reserve broader access, but cannot narrow the backend's actual guarantee.

`Engine::new` selects `NativeExecutionBackend`, whose ID is `ymp.native` and whose version is the provider crate's package version. It runs the existing Codex, Claude, ACP and explicit offline mock adapters. Native authentication and capability inspection are unchanged. `Engine::with_execution_backend(Arc<dyn ExecutionBackend>)` explicitly replaces execution for that engine and subsequent clones; existing clones and active turns keep their original backend. There is no dynamic ABI, registry, loader, plugin service or dependency container.

## Substitution example

This small compiled backend supports an offline follow-up answer. Unsupported purposes fail explicitly. A backend intended for `Engine::run` also implements the planning, execution and review response contracts; the runtime's scripted contract test demonstrates that complete path with an exact-sum artifact.

```rust
use std::sync::Arc;
use tokio::sync::mpsc;
use ymp_providers::{
    ExecutionBackend, ExecutionBackendIdentity, ExecutionFuture,
    ProviderEvent, TurnRequest, TurnResult,
};

struct OfflineAnswer;
impl ExecutionBackend for OfflineAnswer {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "example.offline-answer".into(), version: "1".into(),
        }
    }

    fn execute(&self, req: TurnRequest,
        _events: mpsc::UnboundedSender<ProviderEvent>) -> ExecutionFuture<'_> {
        Box::pin(async move {
            anyhow::ensure!(req.purpose == "conversation", "Only follow-up answers are supported");
            Ok(TurnResult {
                text: r#"{"action":"answer","answer":"The offline backend is selected."}"#.into(),
                session_id: req.resume.unwrap_or_else(ymp_core::new_id),
                usage: None,
            })
        })
    }
}

// Given an Engine, selected project directory and previously recorded session:
// let engine = engine.with_execution_backend(Arc::new(OfflineAnswer))?;
// let answer = engine.follow_up(&project, "Which backend?", &session_id).await?;
```

## Runtime-owned controls

The Engine resolves captured settings, admits the assignment and issues its scoped team capability before calling the backend. Both native and injected execution pass through `run_turn_with_backend`; `run_turn` remains the default-native convenience API. The shared wrapper validates model/effort syntax and permission consistency, enforces cancellation and timeout by dropping the backend future, counts streamed and returned visible output, drains queued observations on terminal paths, and redacts the current assignment capability from complete error chains. Runtime usage accounting, token-stop observation, invocation closure and grant revocation remain outside backend execution. A `TurnResult` cannot override any of these decisions.

The backend is trusted in-process executable code. Its future must yield and own subprocesses/resources with cleanup on drop; detached work and blocking the executor violate the extension contract. This is not an OS sandbox or a hard memory/token ceiling. Output limits act after observation. Native `max_turns` enforcement remains implementation-specific; the budget trace leaves support unknown for alternatives. Provider-specific capability validation, actual settings transmission and native permission/tool enforcement belong to the implementation. Observations must describe what was actually sent or reported. Missing usage and native settings stay unknown; the wrapper does not fabricate native acknowledgements or infer token use from result text. Raw `TurnResult.usage` is retained for adapter compatibility; runtime accounting consumes typed usage events.

## Provenance and compatibility

Each admitted invocation captures optional `execution_backend` ID/version with the assignment transaction, independently of backend observations. The same field survives terminal records and trace export. Historical records without the field deserialize as `None`; they do not acquire the default implementation's identity.

The Engine's configuration identity combines the selected backend ID/version with the existing agent/provider/requested-settings identity. Continuations compare that configuration plus settings and read/write scope. A changed ID, version, settings or profile starts a fresh context. The same compatible configuration retains its context. Beginning a turn invalidates its previous success marker; failure cannot resurrect it. The successful typed marker also owns its cumulative usage baseline, preventing another backend or context's totals from entering resumed accounting. An absent baseline remains unknown.

Effective competence identities include the producing invocation's captured backend identity, configuration, sent/reported settings and observed native version. Selection lookup uses that same configuration key; observations stay bound to the original producing invocation even when a reviewer or resumed engine uses another backend. Old profile-only or backend-unknown experience is not silently reassigned. Backend authors must keep their identity stable and change the version for private settings or implementation changes that alter execution or context compatibility; two different implementations must not claim the same ID/version.

Policy, coordination, retrieval/update and confirmation remain owned by YMP-110, YMP-112, YMP-113/114 and YMP-117. This execution interface neither implements those extension points nor grants authority to their proposals.

## Verification

Offline checks exercise the public `Engine::run` and `Engine::follow_up`, a distinct exact-sum script, hanging futures, cancellation, streamed/result overflow, nested capability errors, captured admission, terminal usage drain, agent attribution, grant revocation, persistent identity, legacy unknown identity, and compatible/incompatible continuation and competence identities. Built-in native wire checks remain in the provider suite. Negative controls and command exits are retained in [execution backend controls](../research/evidence/execution-backend-controls.txt). No provider inference or credential reads are required.

## Integration with confirmed experience

Runtime observation creation and storage attribution use the same core `effective_execution_version` function. Invocations with backend metadata use the backend-aware v2 identity. Historical invocations without that field retain their original v1 identity; this preserves existing bindings without inferring a backend or promoting legacy observations.

The first combined consumer run exposed a mismatch between the runtime v2 calculation and the storage v1 calculation: confirmed work stopped with `Observation attribution mismatch` (exit 101). The shared calculation makes the unchanged confirmed-success/fallback control pass (exit 0). A fixed historical identity control protects compatibility. Phase-sensitive tests now fail when a run finishes before the phase being exercised, instead of waiting indefinitely; restoring the mismatched validation reproduces that failure at the intended boundary.

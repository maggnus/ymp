# Workspace access policy substitution control

Base: `897b4ebb0afe7ce95c64e759c7e786c563cab061`. Checked at `2026-09-12T20:56:08Z` in an isolated worktree. This adds one positive substitution control for the advertised YMP-121 interface criterion; production code, UI, intent and task registry are unchanged. Independent review and final release verification remain separate.

The new `substituted_workspace_policy_serializes_the_same_public_engine_read_workload` test in [concurrency.rs](../../ymp-rust/crates/ymp-runtime/tests/concurrency.rs) runs the same two scripted read-only contributions through public `Engine::run`. The alternate is installed through `Engine::with_workspace_access_policy`; it reserves the whole workspace exclusively while preserving the backend's read-only requests.

| Observation | Default policy | Alternate policy |
| --- | --- | --- |
| Recorded policy | `ymp.direct-mvp`, version `1` | `fixture.exclusive-workspace`, version `1` |
| Actual backend access | `ReadAll` | `ReadAll` |
| Effective reservation | `ReadAll` | `WriteAll` |
| Held native executions | Both start before either barrier is released | Second waits until the first barrier is released |
| Peak production invocations in the runtime journal | 2 | 1 |

The alternate branch observes a durable `resource_conflict` wait while the first native execution remains held, checking the actual holder, policy identity, backend identity and effective access. After release, the waiting reservation must appear in actual admission records. Both runs finish with accepted tasks, unchanged read-only backend permissions, no generated files, closed invocations, no in-flight budget reservations and revoked grants. The pre-existing default overlap test and dishonest narrowing rejection remain unchanged and pass.

The failing control temporarily changed only the alternate test policy's `resolve` method to return `input.backend_access.clone()`. The targeted test failed with exit `101` at the behavioral assertion:

```text
Alternate policy overlapped held read executions: B and Some("A")
```

The correct test was restored before the required checks. No production source was changed for this control. SHA-256 of `concurrency.rs`: corrected `125eb26230b85d85ad99f0f171e51ccd77a7604fbe4e639be620ffbba845b421`; control `a6f703cc6165dea0cdc20b74d049cbabe6d69fa3428a12cfb6dcfb43030fd4d7`.

Validation passed:

- Targeted new test: 1 passed before the failing control; it also passed after restoration in the workspace run.
- `cargo fmt --all --check`: exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
- `cargo test --workspace`: exit 0; all 11 concurrency tests passed. The separately gated Claude SDK catalog fixture remains the workspace suite's single explicit ignore; this change does not claim that separate fixture run.

All Cargo checks used `CARGO_TARGET_DIR=target`, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` and `CARGO_BUILD_JOBS=4`. The targeted command is `cargo test -p ymp-runtime --test concurrency substituted_workspace_policy_serializes_the_same_public_engine_read_workload`.

Full logs and structured command results are retained beside the isolated worktree under `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp121-workspace-policy-livy4fi2`: `workspace-policy-default-access-control.log`, `workspace-policy-control.json`, `workspace-policy-final-checks.log` and `workspace-policy-checks.json`. All workloads were scripted/mock; no installed-provider inference was used.

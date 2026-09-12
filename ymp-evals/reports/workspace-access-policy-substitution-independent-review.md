# Workspace policy substitution independent review

13/09 05:01 HKT, 2026-09-13 (2026-09-12T21:01:54Z) — **ACCEPT, 9/10** for exact candidate `d4550eb1af15760dde3433fceeb85be16effaa80`, base `897b4ebb0afe7ce95c64e759c7e786c563cab061`. The bounded positive workspace-policy substitution gap identified in the earlier YMP-121 preflight is closed. No blocker found.

The complete two-file diff was read. Production source is unchanged. The test uses the same `Mode::Reads` fixture, two task definitions, provider/backend, roster configuration, request and public `Engine::run` path in both branches. The alternate is installed through the public `Engine::with_workspace_access_policy` API. Only the workspace policy and the test's causal release sequence differ; backend execution and permissions remain read-only.

Under the default policy, both production executions reach their held barriers before either is released. Under `ExclusiveWorkspacePolicy`, the first execution remains held while the test waits for a durable `resource_conflict` record naming its actual assignment/reservation as holder. A second start during that interval fails immediately. The wait record checks policy ID/version, backend identity, actual ReadAll access, effective WriteAll reservation and a distinct waiting reservation ID. After release, that exact waiting access record must match an admitted production reservation. This is positive causal evidence of serialization, not an inference from a short period without output.

Ordered runtime events independently yield peak production concurrency two for the default and one for the alternate. Both branches complete with accepted tasks, exactly two producer assignments, unchanged read-only request/permission fields and no generated output files. The shared closure assertions require ended invocations, zero in-flight budget, revoked grants and balanced access acquisition/release records. WriteAll here broadens resource exclusion; it does not grant write permission to the backend.

## Independent execution and failing-control assessment

Independently run in the exact candidate:

```sh
cargo test -p ymp-runtime --test concurrency substituted_workspace_policy_serializes_the_same_public_engine_read_workload -- --nocapture
```

Exit **0**, one test passed. Log `/tmp/ymp121-workspace-policy-independent-focused.log`, SHA-256 `f1c0e8bdf745ad17b52832151e9a553b55aa81367e04bc5c371f6bd53a1935e3`.

The author's retained control replaces the exclusive test policy's resolution with the backend's unchanged access. Its actual log exits **101** with `Alternate policy overlapped held read executions: B and Some("A")`. This targets the intended behavioral distinction: preserving the backend and alternate identity while removing the stricter reservation restores overlap. The failure occurs at the held-execution assertion, before final metadata comparisons. The control is well targeted; it cannot be counted as proof of successful production-policy substitution by itself, but it demonstrates that this new positive test rejects loss of the intended behavior.

The control log and structured result were inspected, not independently replayed in the read-only candidate. Current test bytes match both exact HEAD and the recorded restored SHA-256 `125eb26230b85d85ad99f0f171e51ccd77a7604fbe4e639be620ffbba845b421`. Raw evidence is beside the worktree in `workspace-policy-default-access-control.log`, `workspace-policy-control.json`, `workspace-policy-final-checks.log` and `workspace-policy-checks.json`.

Author formatting, denied-warning Clippy and workspace results were inspected. The complete log contains **376 passing tests** and the single explicit gated Claude SDK fixture ignore; its SHA-256 is `cf58424a7bcd81239fdbf92c409718cc2debca514dcff57c5be147ac863e5eaf`. These are retained author checks, not a newly claimed reviewer full-suite run. `git diff --check` passes. The existing default overlap and dishonest-narrowing consumers are unchanged.

This accepts the substitution test addition, not the full release, native permission guarantees, model quality or the pending YMP-126 reservation work. All execution is mock/scripted, including calls routed through the built-in offline adapter. No native inference, credentials, UI, main, production source, intent or task registry was changed. Only this review report was added; no commit was made.

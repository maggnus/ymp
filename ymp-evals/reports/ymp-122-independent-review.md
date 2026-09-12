# YMP-122 independent review

12/09 21:50 HKT — **ACCEPT R1(9/10)**. No contracted defect remains open. Code: 9/10; evidence: 9/10; public Rust consumer experience: 9/10.

Reviewed candidate: `c322649ae7931be1bfbf00cbb5c389e168a8fa64` in `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/122`. The complete 11-file candidate diff was read against current main's YMP-122 acceptance and subsystem-interface direction, the checkout's AGENTS.md, approved intent, runtime contract, and team skill. The accepted combined YMP-102/120/106/107 baseline and budget correction `7e0a40e` are included. Separately integrated YMP-117 confirmation work is outside this verdict.

## Findings and contract assessment

No return finding. The public `ExecutionBackend` interface supplies a typed request, typed observations and a boxed asynchronous result. `Engine::new` retains `NativeExecutionBackend`; `with_execution_backend` validates and captures the selected identity before execution. Replacement affects the intended execution boundary without adding a scheduler, mutable store parameter, dynamic ABI, service or dependency container. See `ymp-providers/src/backend.rs:8` and `ymp-runtime/src/engine.rs:139` under `ymp-rust/crates`.

The actual Engine call at `engine.rs:886` reaches `run_turn_with_backend`. Common request validation, cancellation, timeout, streamed/result character checks, terminal observation drain and complete capability-error redaction surround both implementations (`ymp-providers/src/lib.rs:90–163`). Assignment admission precedes the call at `engine.rs:835`; terminal invocation accounting, observed-token cancellation and grant closure stay in the runtime. Injected result usage does not substitute for typed usage observations. Independent probes retained 13 observed tokens per successful call despite a conflicting raw result field claiming 999999 input tokens.

Backend identity is captured on `InvocationRecord` before admission completes (`engine.rs:817–838`). A backend observation cannot overwrite it. Configuration hashing includes the backend ID/version; selection lookup uses that configuration; effective competence includes the producing invocation's captured identity and actual execution observations (`engine.rs:72–82`, `150–168`, `255–306`). Compatible context and its cumulative baseline share one successful marker; a new invocation invalidates that marker until success (`engine.rs:673–680`, `851`, `1044–1052`). Missing legacy identity remains `None`, and absent native settings/version remain unknown. No legacy native identity is invented.

The extension documentation correctly leaves policy, coordination, knowledge and confirmation interfaces to their owning tasks. Native settings transmission, native capability enforcement and resource cleanup remain backend implementation duties within the documented trusted-code boundary.

## Independent execution evidence

Mandatory checks ran in the candidate checkout. Each exited **0**:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` — **217 tests**, zero failures.
- `git diff --check`

The workspace run included the eight backend runtime tests, provider request validation, native Codex retry/provenance checks, native settings and ACP acknowledgement checks, native authority/restoration checks, output/timeout checks, capability-error formatting checks, and budget/assignment-authority regressions. These are offline protocol fixtures; they performed no provider inference.

A separate temporary crate imports the candidate's actual public packages through path dependencies. Its source and raw logs remain at `/tmp/ymp122-review.GCD5Tt`; no checkout source was copied or mutated. Command:

```sh
cargo test --manifest-path /tmp/ymp122-review.GCD5Tt/Cargo.toml --offline -- --nocapture
```

Final exit: **0**, six tests. Observed consumer outcomes:

- `Engine::run` executes an independently written backend, creates `answer.txt` containing `42\n`, completes the task, and returns the forty-two summary. Every invocation records the selected implementation; trace and usage survive reopening the store.
- Public `follow_up` retains the same context and cumulative baseline for compatible execution, separates changed ID and version, leaves missing totals unknown, and starts fresh after failure. Legacy invocation JSON without backend metadata remains unknown.
- Hanging futures stop on timeout and explicit cancellation. Unicode streamed and returned output exceeding 64000 characters stops with `output_limit`. Usage queued after the offending delta survives as input count 23. Futures are dropped and all assignment grants/reservations close.
- A backend reporting 1000 input and 2 output tokens and then hanging stops with `token_limit`. Changing the Engine's configured invocation allowance after a session captured one allowed invocation does not admit a second call.
- An injected capability-bearing error retains `probe_error`, while alternate/debug formatting and every error source omit the current capability.
- With producing agent and requested settings held constant, repeated backend identity produces the same competence version; changed backend ID or version produces different competence versions. The stored selection lookup agrees with the producing observation. Invalid backend identifiers are rejected before any invocation.

An early version of the independent competence probe let different agents win separate runs; comparing those versions correctly failed. Fixing the probe's bid response to hold the producing agent constant resolved that fixture error without changing candidate code. An initial continuation-corruption setup used `/tmp` instead of the canonical `/private/tmp` metadata key; that setup failure was excluded from evidence and corrected before the discriminating runs below.

## Observed failing controls

The same temporary crate's `wrapper_falsifier` can invoke the real common wrapper or directly invoke the same backend. For each `CASE` in `validation`, `output`, `error`, `timeout`:

```sh
YMP_REVIEW_CASE=CASE cargo test --manifest-path /tmp/ymp122-review.GCD5Tt/Cargo.toml --offline wrapper_falsifier -- --nocapture
YMP_REVIEW_CASE=CASE YMP_REVIEW_BYPASS=1 cargo test --manifest-path /tmp/ymp122-review.GCD5Tt/Cargo.toml --offline wrapper_falsifier -- --nocapture
```

All four guarded forms exited **0**. All four bypass forms exited **101**: invalid settings and oversized output returned success (`Common guard missing`); errors exposed the synthetic capability (`Capability redaction missing`); hanging execution exceeded the probe deadline (`Timeout wrapper missing: Elapsed(())`). These checks distinguish the shared wrapper from direct backend execution; they do not establish containment of trusted in-process code.

The continuation test also runs against deliberately corrupted disposable metadata:

```sh
YMP_REVIEW_CORRUPT_CONTINUATION=CASE cargo test --manifest-path /tmp/ymp122-review.GCD5Tt/Cargo.toml --offline public_identity_baseline -- --nocapture
```

Each of `context`, `baseline`, and `compatibility` exited **101** at the intended assertion. Actual failures were resumed context `foreign-context` versus the preceding successful context; baseline input/output `999/999` versus `50/50`; and absent resume versus the compatible preceding context. The uncorrupted six-test suite then exited **0**. These controls demonstrate that the consumer probe checks actual context/baseline values and useful reuse, not just backend labels. They make no claim of metadata tamper resistance.

The author's `ymp-docs/research/evidence/execution-backend-controls.txt` was checked against the real call site, guards, marker writes and test assertions. Its wrapper-bypass, validation, identity, provenance, reuse and drain failure claims match the paths in this candidate. The reported cumulative-baseline correction is present: no separate `native_usage` lookup remains in Engine execution. Author log text alone was not used as proof; independent wrapper bypasses, public Engine observations, and corrupt-input controls supplied the evidence above. The author's exact source mutations were not repeated because checkout source was read-only for this review.

## Limits and repository state

This acceptance covers compiled backend replacement and common runtime controls. Blocking the executor, detached work, dishonest observations, or two implementations deliberately claiming one identity violate the documented trusted-backend contract; no OS sandbox, hard native token ceiling, or native inference quality was established. Provider-native `max_turns` remains implementation-specific. Bridge source, TurnRequest serialization and native authentication were unchanged; separate bridge checks and actual provider inference were not run. No credentials, TUI walkthrough, intent edits or task-registry edits were performed. YMP-117 acceptance/confirmation changes require their separate integration verdict.

Approved intent SHA-256 remains `4479e919c5b47934e69e81ac493d16e90a0dc420a9a370382e79bca3f3c754d9`. The checkout was clean before this report; this report is its sole review-authored change.

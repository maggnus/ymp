# YMP-126 driver preparation

This is incomplete integration preparation on base `6e6493da367f2c45066d1ece79a6eb949e16c5a6`, not release acceptance. The task registry and immutable expected traces are unchanged. Eight of seventeen declared cases currently have passing production-boundary adapters: document, transform, grounded, qualitative, fixed-size, concurrency-conflicts, location-retrieval and effort-support. Other adapters remain incomplete, and the executable returns failure for missing or mismatched cases.

## Shared workspace admission

The trusted public API is `Engine::try_reserve_workspace(session_id, &TurnRequest, Option<TaskAttemptRef>) -> Result<WorkspaceAdmission>`. `Acquired(Box<WorkspaceReservation>)` owns the actual coordinator lease; `Deferred(WorkspaceWait)` creates no assignment, grant, spend or queued work. Scope comes from the installed execution backend and runtime policy. The request must match captured directory, profile/provider, settings, native ceilings and current eligibility. A policy cannot narrow actual enforced backend access or assert fictitious containment.

The ordinary Engine path uses the same access derivation and coordinator acquisition helpers. A reservation's `admit_reserved` binds exact assignment/request/backend facts before using the existing TeamServer budget/grant transaction. Dropping it ends any still-live bound capability before releasing filesystem ownership. Real Engine follow-up is independently tested waiting on this same public reservation and continuing after release; additional tests cover conflict/capacity one-shot probes, false-scope/provider rejection and identity/lifetime binding. No evaluation-only lock model or alternate engine was introduced.

## Actual evidence already exercised

The reproducible executable is `cargo run -p ymp-eval-driver -- --output /absolute/fresh/non-git/evidence-directory`, optionally with `--case CASE_ID`. Every selected directory is fresh and outside Git. Only declared input files are copied into its `work` directory. Scripted production computes from those files; it never reads reference artifacts or validator code. Runtime-installed trusted checks invoke the independent artifact validator outside provider context. Native usage is explicit synthetic data and nullable cost remains unknown.

Passing preparation bundles are `/tmp/ymp126-workflow-second-20260912` (document), `/tmp/ymp126-transform-20260912`, `/tmp/ymp126-grounded-second-20260912`, `/tmp/ymp126-qualitative-20260912`, `/tmp/ymp126-fixed-size-first-20260912`, `/tmp/ymp126-concurrency-first-20260912`, `/tmp/ymp126-location-first-20260912` and `/tmp/ymp126-effort-first-20260912`. Their normalized files are derived from actual runtime decisions, native observations or returned public boundary reads, with complete raw traces/journals, alias maps, metrics and source/fixture hashes. These are preparation runs; final combined runs and stronger exporter negative controls remain required.

## Concrete reservation incompatibility

The unchanged budget-reservations fixture requests different per-assignment allowances (60, 10, 20 and 21 tokens) and a 20-token protected review reserve within 100. Current runtime `ResourceLimits.invocation_tokens` supplies one global estimate for all reservations and multiplies it by protected review invocations. With 60 captured for the requested execution, the actual protected review allowance becomes 60 as well. The runtime rejects the first planning admission with `token_review_reserve`, without invoking the scripted backend.

`/tmp/ymp126-budget-gap-20260912/budget-reservations` retains the actual runtime, normalized mismatch, validator failure, metrics and `compatibility.json`. The exact command was `cargo run -p ymp-eval-driver -- --output /tmp/ymp126-budget-gap-20260912 --case budget-reservations`, exit 1. The driver does not map different requested allowances onto the global estimate or change the expected accounting. A separate production correction must introduce bounded per-assignment reservations and a captured review token reserve while retaining the incomplete-usage stop and native overshoot limitations.

## Build cache incident

An intermediate build lacked generated `libsqlite3-sys` `bindgen.rs`; `/tmp/ymp126-effort-first-20260912.log` retains the successful rebuild. No product conclusion or skipped case followed. `CARGO_TARGET_DIR` was unset, and Cargo used this checkout's own `sibling126/target`. Repair was limited to `cargo clean -p libsqlite3-sys --target-dir .../sibling126/target` (12.6 MiB). No other target directory was deleted.

This checkpoint precedes composition with the accepted YMP-112/YMP-114 commits. Final task completion still requires all seventeen authentic scenarios, independent review, negative controls, complete required checks and the YMP-121 final release verification.

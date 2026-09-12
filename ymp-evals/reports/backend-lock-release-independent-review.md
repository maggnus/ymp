# Shared project/session lock release independent review

13/09 01:08 HKT — **ACCEPT R1(9/10)** for `861bf918168125c526b241c860bad1cc741cf7d5`, based on MCP candidate `ced4aec16825a962fd46dc42bf042254b465a72a`. Code 9/10; evidence 9/10; runtime-consumer experience 9/10. No blocking or major finding remains in this bounded correction.

The complete three-file commit, correction report, diagnostic patch/command record and actual failure/control logs were inspected. The approved runtime ownership and temporary-authority requirements remain the review boundary. This acceptance applies to the stated candidate, not the later composition with accepted YMP-115.

## Ownership and lifecycle

`ymp-rust/crates/ymp-storage/src/lib.rs:34` introduces an owning `StoreLock` with a private `File`. It has no `Clone`, `Deref`, raw-descriptor conversion or file accessor. Only successful exclusive-lock acquisition constructs the guard. A failed acquisition cannot run its unlock path. Drop explicitly calls `FileExt::unlock` on that owner's open description before the file closes.

All runtime `lock_session` and `lock_project` call sites were read. Their lexical scopes and explicit follow-up handoff drops are unchanged. No runtime authority, grant, budget, session transition, workspace scope or native backend implementation changed. The type conversion does not shorten a live guard's lifetime. The source has no runtime manual-fork route that duplicates and later drops a Rust `StoreLock`; the native subprocess route inherits descriptors, not another independently droppable guard.

The installed fs2 Unix implementation was read: unlock invokes `flock(fd, LOCK_UN)` on this host. An inherited descriptor refers to the old open description; a later owner acquires through a separately opened description. Explicit old-owner release therefore permits immediate reacquisition, while closing the stale inherited descriptor cannot release the new owner's lock. Both these directions were tested across actual processes, including exclusion while each owner is live.

## Consumer diagnostics and bounded failure handling

The cancellation test now terminates its admission-wait partner when the tested future returns early. Its one-second invocation timeout and three-second outer bound are unchanged. The original checks for cancellation/timeout classification, dropped noncooperative future, partial usage, and closed budget/grants remain. The redaction test still inspects all error renderings, error chains, stored traces and UI events for leaked synthetic capabilities.

`assert_backend_admitted` checks the request list before indexing it. On missing admission, diagnostics contain only an IO number, session status and recent durable event kinds; neither raw errors nor requests/capability values are formatted. This makes pre-admission failure visible without replacing the intended redaction assertion or inflating a timeout.

The captured `runtime-default-01.log` establishes IO 35/EWOULDBLOCK, completed setup, no scripted admission and the static session-lock error. That is an admission failure, not an observed setup timeout. The separate fork/CLOEXEC control and deterministic lock controls demonstrate the retained-open-description mechanism. The particular retaining child in the original runtime failure was not identified. The older redaction occurrence remains unclassified, as the correction report states; this review makes no retrospective cause claim for it and no ENOSPC claim.

## Independent checks and falsifier

| Command / control | Actual result |
| --- | --- |
| `CARGO_INCREMENTAL=0 cargo test -p ymp-storage completed_ -- --nocapture` on exact candidate | **Exit 0**, both new lock controls passed |
| `CARGO_INCREMENTAL=0 cargo test -p ymp-runtime backend_contract_tests -- --nocapture` on exact candidate | **Exit 0**, all eight backend consumer tests passed |
| `CARGO_INCREMENTAL=0 cargo test -p ymp-storage independent_cross_process_lock_tests -- --nocapture` in disposable candidate | **Exit 0**, both independent session/project process controls passed |
| Identical independent process tests with only explicit unlock bypassed | **Exit 101**, both failed at completed-owner reacquisition: contender returned busy status 75 instead of 0 |
| `git diff --check` and candidate status before report creation | **Exit 0**, source unchanged |

Each independent process test acquires a real `StoreLock`, passes a duplicate of its open description as an actual Python child's stdin, and waits for the child's readiness signal. Separate Python contenders open the lock path afresh and use nonblocking exclusive `flock`. The asserted sequence is:

1. A contender is excluded while the initial owner is live.
2. After owner drop, a contender can acquire while the inherited descriptor is still open in the child.
3. A new Rust owner excludes contenders.
4. The child closes its stale inherited descriptor; contenders remain excluded by the new owner.
5. The new owner drops; a contender can acquire again.

The child has a bounded lifetime. The same probe bytes were used for the corrected and unlock-bypassed runs. Both bypass failures were the intended ownership assertion, not compilation, timing, fixture setup or provider behavior. Source was restored in the disposable tree after the control.

Independent test code, full outputs and exact command exits remain under `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-lock-independent-mrzn0inz` in `ymp-rust/crates/ymp-storage/src/lib.rs`, `cross-process-corrected.log`, `cross-process-unlock-bypassed.log`, and `results.json`.

## Required evidence reused and limits

The correction's final restored-source evidence was read rather than rerunning the full workspace without a new source concern: `cargo fmt --all --check` **0**; `cargo clippy --workspace --all-targets -- -D warnings` **0**; `cargo test --workspace` **0**, with **273 passed** confirmed from the log's test-group totals. The candidate source was clean at exact HEAD during the independent focused runs.

All seven SHA-256 values published in `backend-lock-release-correction.md` were recomputed and matched: the default-concurrency failure log, diagnostic patch, original lock-control log/patch, corrected control log, unlock-bypass log and final workspace log. The actual control logs show both session/project tests failing on original or bypassed release and passing corrected. Passing diagnostic searches were not used as proof of a cause.

Every workload was offline and scripted. The independent lock probes used local Unix filesystem locks on this host; other operating systems and network filesystems were not tested. This change does not promise recovery after forced process termination with surviving descendants, eliminate external side effects, or provide workspace isolation. Source, main, intent, task registry and UI were not changed. No credentials were inspected, and no inference or commit occurred. Only this report was written in the candidate.

Round ledger: 13/09 01:08 HKT — `R1(9/10) ACCEPT`: explicit owner unlock closes reproduced false admission contention; independent actual-process controls preserve live and replacement-owner exclusion and fail when unlock is bypassed; combined integration remains separate.

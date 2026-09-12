# YMP-126 runtime corrections — independent review

Reviewed immutable composed commit: `812b10199e27129684265ca8dda23fe13593b955`, covering the public seam introduced in `46347661967c15e4db3e08493e14437cf6476e54` and corrections in `52b951c8443222510d8306eec7c8dae199f38b80`. The author's `ymp-evals/reports/ymp-126-runtime-corrections.md` was read. Review date: 2026-09-13. Verdict: **request changes**, score **7/10** for the bounded runtime delta. This is not acceptance of the 17-case evaluation driver.

## F1 — Actual context limits can be bypassed at public admission (P1)

`ymp-rust/crates/ymp-runtime/src/engine/workspace_access.rs:101` validates native turn/output controls but does not measure the actual bound prompt and profile instructions against captured phase limits. `ymp-rust/crates/ymp-runtime/src/reservation.rs:115` requires a matching prompt digest but accepts an absent or understated `included_chars`. `ymp-rust/crates/ymp-storage/src/budget.rs:249` then uses those caller-supplied counts for context admission.

An independent control reused the committed public workspace fixture, compiled outside the source tree against the frozen libraries. It supplied a prompt of 128001 characters under the captured 128000-character context cap, retained the correct prompt digest, and declared zero included characters. The actual public acquisition, TeamServer admission and bound `run_turn` all succeeded, reaching the scripted backend once:

```text
actual_prompt_chars=128001 captured_limit=128000 declared_prompt_chars=0 admitted=true ran=true backend_calls=1
```

The expected-denial assertion failed with exit 101. Measure the actual reserved request before acquisition/admission and bind exact prompt/instruction accounting before issuing a grant or spending an invocation. Missing, zero and understated evidence must not suppress actual supplied characters. Retain controls for the ordinary valid request and captured startup versus regular phase ceilings.

## F2 — Public reservations bypass ownership held by another process (P1)

`Engine::run_internal` (`ymp-rust/crates/ymp-runtime/src/engine.rs:587`) and `follow_up` (line 413) hold `Store::lock_project`. Public `try_reserve_workspace` has no equivalent ownership requirement and reaches only the process-local coordinator (`ymp-rust/crates/ymp-runtime/src/engine/workspace_access.rs:150`). Its guard can therefore conflict with a live owner's resource in a different process.

An independent control used two actual OS processes sharing one temporary application home, session and selected workspace. The parent held `Store::lock_project` and a public writer reservation for `outputs/shared.txt`. While both remained held, the child reopened the same home and obtained another public writer reservation for agent `b` over the same path:

```text
child_acquired_conflicting_writer=true
```

The expected-exclusion control failed with exit 101. No native invocation was required to expose the overlapping ownership. Enforce project ownership at this boundary or require an unforgeable owned guard; keep legitimate in-process reservation sharing and waiting behavior. The author confirmed that ownership was not an intended caller-only prerequisite and accepted this as an enforcement correction.

## Verified scope and limits

All three variable-budget storage controls independently passed: competing independent database connections admit only affordable reservations; zero/oversized requests cannot override the captured ceiling; live review use and outstanding reservations are counted once; completed review releases unused protection; partial closed usage remains a stop; overflow is denied. Code inspection confirms that reservation checks and invocation/grant insertion share the existing immediate transaction, captured limits remain the source, and historical observed spend is not refunded. No token-ledger blocker was found in this review.

All four existing public workspace controls independently passed, covering scope/configuration rejection, one-shot conflict/capacity deferral without assignment/spend/queue, real Engine waiting on a public reservation, identity/request/replay binding, and normal lease-drop grant revocation before access release. Normal Engine execution uses the bound `run_turn` path and retains its terminal observation drain. The two new failures above exercise limits and process ownership absent from those tests.

Only focused checks were run, with `CARGO_TARGET_DIR=target`, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_BUILD_JOBS=4`:

- `cargo test -p ymp-storage --lib variable_`: 2 passed.
- `cargo test -p ymp-storage --lib live_review_reservation`: 1 passed.
- `cargo test -p ymp-runtime --test workspace_reservation`: 4 passed.
- Outside-tree expected-denial context control: failed as described, exit 101.
- Outside-tree two-process expected-exclusion control: failed as described, exit 101.

The immutable checkout is `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp126-runtime-review-codaf9v5/worktree`. Failure transcripts are retained alongside this report as `public-reservation-context-control.log` and `public-reservation-cross-process-control.log`. Controls were compiled from the frozen test fixture plus review assertions in temporary files; implementation and fixture source were not changed. The report lives outside the checkout. No main, sibling126, UI, owner-home, credentials or installed-provider inference were touched. Driver exporter/adapters and the parent's separate long-home socket correction remain outside this verdict.

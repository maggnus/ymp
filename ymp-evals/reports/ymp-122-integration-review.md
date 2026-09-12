# YMP-122 integration review

12/09 22:56 HKT — **ACCEPT I1(9/10)**. No integration defect remains open. Code, evidence and public consumer behavior: 9/10 each.

Reviewed `a2eac85d3618d282554124c2269f8fdf584517ef` in `integrate_backends`: accepted main through YMP-117, the previously accepted YMP-122 backend change mapped to `c24b1ca`, shared identity correction `01b2053`, and regression/legacy coverage `a2eac85`. The complete five-file integration diff was read with the actual runtime credit, storage validation, selection, confirmation and backend call paths. AGENTS.md, approved runtime contract and the previously loaded team skill governed this review.

## Invariant assessment

The original integration failure is corrected at its source. `ymp-core/src/provenance.rs:15` now owns `effective_execution_version`; runtime production (`ymp-runtime/src/engine.rs:82`) and storage credit validation (`ymp-storage/src/confirmation.rs:803`) both call it. The storage attribution checks remain intact; no mismatch is ignored and no forgery assertion was narrowed.

Invocations with captured backend metadata retain the previous backend-aware v2 digest. Records without metadata use the exact pre-backend v1 tuple, verified against accepted pre-integration source and the fixed historical value `ca91aef9422e5f35948eaf04`. Missing identity is not assigned to the native implementation. Existing unknown observations still do not qualify for reputation merely because their version can be computed.

Selection uses the backend-scoped configuration lookup, which records the same effective version used for the producing observation. Storage still binds credit to the confirmed accepted result, producing assignment/invocation, current attempt, competence and difficulty, before checking duplicate IDs. Backend ID/version changes cannot redirect a previous producer's credit.

The phase-sensitive confirmation tests retain their original artifact/input changes and outcome assertions. `await_phase` now rejects early `Finished`, a closed event channel or ten seconds without an event. It fixes the observed early-completion hang without weakening the delivery-freshness checks; its timer is an event-silence bound, not a total runtime deadline.

## Commands and observed results

Executed in the integration checkout, each with exit **0**:

- `cargo fmt --all --check`
- `CARGO_TARGET_DIR=../integrate_confirmation/target cargo clippy --workspace --all-targets -- -D warnings`
- `CARGO_TARGET_DIR=../integrate_confirmation/target cargo test --workspace` — **230 tests**, zero failures.
- `git diff --check`

The actual Cargo target argument used the absolute equivalent of the relative path shown above. The workspace run included unchanged confirmed-success/narration-fallback, forged result/review/credit, final-phase binding, partial-coverage, failed applicable assertion, stale artifact/input, alternate checker, historical identity and backend-isolation tests. Native settings, authority, retry, capability-error, cancellation/output, budget and grant regressions also passed.

The separate public-API harness at `/tmp/ymp122-integration.tU7ASI` imports the actual integration packages. It adapts the previous independent forty-two backend with a trusted `ExactBytes` acceptance contract for `answer.txt`; successful model assertions alone are never treated as confirmed credit.

```sh
cargo test --manifest-path /tmp/ymp122-integration.tU7ASI/Cargo.toml --offline -- --nocapture
```

Exit **0**, **10 tests**. Raw output: `/tmp/ymp122-integration.tU7ASI/integration-suite.log`.

Independent observations:

- Distinct execution creates `answer.txt` containing `42\n`, records its actual backend, completes confirmed acceptance and persists one qualified producer observation. The same confirmed result survives an injected synthesis failure.
- Duplicate qualified credit returns `false`; wrong backend ID/version, a v1 identity for a backend-present invocation, profile-only identity, wrong competence and wrong difficulty all fail with `Observation attribution mismatch`. Adding an unknown observation with the same effective version does not increase qualified reputation. Reopening preserves the qualified success count.
- Removing the trusted contract gives accepted/unconfirmed work and zero observations. Changing the trusted expected bytes to `43\n` blocks the work despite approving scripted reviews and gives zero observations.
- Actual adaptive `Engine::run` selection reads prior qualified successes. For backend sequence A/1, A/1, B/1, B/2, A/1, the producing agent's recorded selection counts are respectively **0, 1, 0, 0, 2**. This exercises the selection consumer, not only equality between helper outputs.
- Compatible continuation and cumulative baseline reuse, backend-change separation, failure invalidation, captured admission, token cancellation, explicit cancellation, timeout, Unicode output overflow, terminal usage drain and grant closure continue to pass through public Engine calls.

## Failing evidence and limits

The independent credit probe's adverse form attempts to reuse the historical v1 calculation for a backend-present confirmed invocation:

```sh
YMP_REVIEW_ASSUME_OLD_CREDIT=1 cargo test --manifest-path /tmp/ymp122-integration.tU7ASI/Cargo.toml --offline confirmed_credit_rejects_wrong_backend_and_old_hash -- --nocapture
```

Exit **101** at the intended assertion: `Old storage identity should have matched: Observation attribution mismatch`. The same command without the adverse environment variable exits **0**. This distinguishes correctly attributed qualified credit from the old calculation; it does not claim historical records should be rejected.

The supplied original failure log `/tmp/ymp-backend-confirmation-version-before.log` was checked against the former runtime/store calculations and unchanged consumer assertion: confirmed completion failed with `Observation attribution mismatch`. `/tmp/ymp-backend-confirmation-phase-before.log` was checked against the new helper and preserved delivery test: forced early termination failed with `Run finished before phase synthesis`. Exact checkout source mutations were not repeated because this review kept source read-only. The passing integration consumers and independent adverse-input controls above provide fresh execution evidence rather than treating those logs as proof.

No provider inference, credential access, UI change/walkthrough or bridge change was performed. The review establishes offline compiled composition and invariant preservation; it does not establish native model quality, hard token bounds or containment of trusted in-process extensions. Checkout source, intent and task registry remained unchanged. This report is the sole review-authored checkout change.

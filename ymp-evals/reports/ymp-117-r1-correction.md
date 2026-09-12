# YMP-117 R1 correction evidence

12/09 21:20 HKT — Author response to [R1(2/10) RETURN](ymp-117-independent-review.md). The original report remains unchanged. This response closes F1–F4 for independent R2 evaluation; it is not an acceptance verdict.

| Finding | Correction | Discriminating result |
| --- | --- | --- |
| F1: forged leaf final acceptance | Final phases require the latest recorded aggregate covering every current accepted task and its actual final review; decision kind and outcome must agree | Exact public forged-final probe now rejects with no state/event change; added stale-aggregate and outcome-shape cases pass |
| F2: reused or contradictory review invocation | Review admission captures the immutable result decision ID/digest. Storage requires the exact candidate/final purpose, task scope, result binding and one assessment per invocation | Exact cross-phase and contradictory-assessment probes reject; new identical-replay test also rejects |
| F3: accepted text/definition replacement | Candidate records capture the task definition; acceptance checks its exact text, definition and producer. Plain writes cannot reopen an accepted attempt | Public precursor is rejected with an unchanged trace. Eight accepted-field mutation cases reject atomically, followed by successful acceptance of the original candidate |
| F4: narration-time artifact drift | Both narration branches recheck current task/artifact/input bindings and confirmation before returning. Persistent drift appends invalidation and returns blocked/unconfirmed while retaining historical acceptance and supported observations | Original consumer probe now returns blocked with current grade unconfirmed. Eight artifact/input × normal/failed narration × confirmed/unconfirmed cases pass |

## Commands and captures

- Before corrections: `cargo test --manifest-path /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-117-review-probes-j4afrbna/Cargo.toml -- --nocapture` exited **101**, reproducing the exact five failed assertions and five passing controls. Source was unchanged.
- After corrections: the same command exited **0**, all ten tests passing. Raw output is `author-r1-after.txt` in that external crate; `author-r1-before.txt` retains the reproduced failures. Original R1 captures remain in `captures-r1-original/` and the author's before-run captures in `captures-author-r1-before/`.
- `YMP_TEST_CAPTURE_DIR=/tmp/ymp-117-r1-atomicity cargo test -p ymp-runtime confirmation_ -- --nocapture` exited **0**, twelve tests passing.
- `YMP_TEST_CAPTURE_DIR=/tmp/ymp-117-r1-atomicity cargo test -p ymp-storage stale_task_decisions_cannot_rewind_attempts_or_overwrite_a_later_state -- --nocapture` exited **0**.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` each exited **0**. Raw logs are `/tmp/ymp-117-r1-fmt.log`, `/tmp/ymp-117-r1-clippy.log`, and `/tmp/ymp-117-r1-workspace.log`.

The external after-captures show forged final/review operations leaving 13 decisions and 92 events unchanged. The accepted-attempt precursor and eight candidate-field rejection captures under `/tmp/ymp-117-r1-atomicity` each contain identical full before/after traces. Delivery captures contain the actual blocked summaries, immutable historical acceptance, current task state and appended invalidation. The original external accepted-content test now returns at the rejected precursor; the additional local captures cover that rejection and the direct acceptance transaction separately.

## Interface and limits

`ConfirmationChecker` remains unchanged and returns only raw check output. The correction adds an optional task-definition snapshot to `ResultVersion`, a `Result` context reference for review admission, and `Store.result_is_current` for current consumer validation. Historical records deserialize and remain inspectable; new transitions require complete result bindings. No task registry, intent, UI, bridge, native-provider, grant/admission or knowledge-promotion changes are part of this correction. Snapshot validation still does not provide an OS sandbox against change-and-restore races.

Round response: `R1(2/10) RETURN 12/09 21:20 — F1–F4 public forgeries and stale consumer grade → exact before/after probes, atomicity traces and narration matrix → corrected; independent R2 pending`.

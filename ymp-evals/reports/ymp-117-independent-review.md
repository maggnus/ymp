# YMP-117 independent review

12/09 21:05 HKT — **R1(2/10) RETURN** on `52e692b316cd400f490791b8f7b3b8bd1e0eb6b3`, baseline `57c325b`.

The required workspace checks pass. Four defects remain in the contracted outcome: public storage accepts forged final acceptance and review bindings, accepts replacement task content under an earlier reviewed result, and the final consumer can receive a confirmed grade after an artifact changes during narration. These are YMP-117 acceptance/evidence defects, not extensions into admission, native grants, workspace policy, or knowledge activation.

Score: code 2/10 (open acceptance-authority blockers), evidence 7/10 (full diff, public consumer probes, actual adverse outcomes, and required checks), consumer experience 5/10 (normal paths work, but the final grade can contradict the current artifact). The round score is the lowest applicable axis. This is the first return; one return remains in the initial review budget.

## Authority and scope inspected

- Checkout `AGENTS.md`, the unchanged approved `intent.md`, and `ymp-docs/architecture/runtime-contract.md`.
- Current main `/Users/maggnus/Code/ymp2/ymp-docs/tasks/tasks.json`, YMP-117, including the added injectable typed checker criterion; current main `ymp-docs/architecture/subsystem-interfaces.md`.
- Team skill at `/Users/maggnus/.codex/plugins/cache/maggnus/team/1.3.0+codex.20260826013847/skills/team/SKILL.md`.
- Both universal scenario files, particularly qualitative acceptance, actual supplied data, evidence boundaries, aggregate coverage, and artifact versions; the production-facing validator interface in `ymp-evals/validators/universal.py`.
- The complete 16-file implementation diff and the actual `Engine::run`, acceptance, final narration, storage mutation, grading, reputation, and trace consumers.

The ordinary Engine contract list starts empty. Trusted contracts are installed through a generic public Rust API before invocations; model plan commands cannot populate it. This is a usable production integration seam for the universal validator. The integrated universal fixture driver remains YMP-121 work, and this review does not claim it was implemented or run.

## Findings requiring correction

### F1 — Final acceptance accepts a leaf result and candidate review

Location: `ymp-rust/crates/ymp-storage/src/confirmation.rs:436` and `:467`.

`Store::record_decision` accepts a real `task_accepted` record copied with a fresh ID and `kind = "final_accepted"`. Its leaf result, task reference, and candidate-review invocation remain unchanged. The call returns `Ok(())`, adds one decision and one journal event, and claims confirmed final acceptance without a final aggregate or final review.

Reproduction: external test `final_acceptance_requires_aggregate_and_final_review`. A public `Engine::run` first produces a genuinely confirmed greeting; the test then performs the two-field mutation above and asserts rejection. The assertion fails. Capture `leaf-final-acceptance.json` shows decisions 13 → 14, events 92 → 93, unchanged tasks.

Required correction: validate final transition shape and phase explicitly, require the recorded current aggregate covering all current accepted tasks, and require its actual final review. A task result/candidate review must not satisfy final acceptance. Reject incompatible kind/outcome combinations, preserving the state and journal on rejection.

### F2 — Review evidence can reuse another phase or contradict the same invocation

Location: `ymp-rust/crates/ymp-storage/src/confirmation.rs:312`.

Review validation accepts either `review` or `final_review` assignment purpose for every review kind and does not prevent reusing an already-recorded review invocation. Two public probes return `Ok(())`:

1. Copy the genuine `final_review` with a fresh ID, replacing actor, assignment ID, and invocation ID with the candidate review's values. The aggregate is now purportedly reviewed by an invocation that reviewed only a leaf.
2. Copy the genuine candidate review with a fresh ID and opposite outcome/rationale. Storage records two contradictory judgments for the same completed invocation.

Reproduction: `final_review_cannot_reuse_candidate_invocation` and `review_outcome_cannot_be_rewritten_for_same_invocation`. Both rejection assertions fail. Captures `candidate-as-final-invocation.json` and `contradictory-review.json` each show decisions 13 → 14 and events 92 → 93.

Required correction: bind a review to its actual assignment purpose, exact reviewed result/version, and single observed invocation outcome. Replays must not create a different assessment or allow a candidate invocation to become an aggregate review. The existing exclusion of all result producers must remain enforced.

### F3 — Accepted task text can be replaced under the old reviewed result

Location: `ymp-rust/crates/ymp-storage/src/provenance.rs:593` and `ymp-rust/crates/ymp-storage/src/confirmation.rs:440`; public precursor `ymp-rust/crates/ymp-storage/src/lib.rs:251`.

The new direct accepted-state guard is bypassable through a plain task write to `Review`. After a real confirmed run, copy the task, set its state to `Review`, and call `save_task`. Then set it back to `Accepted`, replace its result text with `A NEW unreviewed deliverable asserted at acceptance`, and call `save_task_with_decision` with a fresh-ID copy of the old acceptance. The old immutable result, review, and confirmation links remain unchanged. Both calls succeed. The stored accepted task now exposes the new text while its evidence still identifies the old summary.

Reproduction: `accepted_state_cannot_replace_reviewed_result`. The rejection assertion fails. Capture `accepted-content-replacement.json` records the immediate pre-acceptance and post-acceptance state: decisions 13 → 14, events 93 → 95, and changed task content.

Required correction: acceptance must commit exactly the submitted/reviewed result and task definition for that attempt. Protect the public mutation path against replaying an already accepted attempt with changed content; a changed result requires a new result version and review. Rejected forgeries must leave both current state and journal unchanged.

### F4 — Final narration can return a stale confirmed grade

Location: `ymp-rust/crates/ymp-runtime/src/engine.rs:1259` through the normal and fallback summary branches.

The confirmation grade is computed before optional work and synthesis, then reused in the consumer summary. An external public `Engine::run` probe waits for `UiEvent::AgentStatus` with status `synthesis`, overwrites the checked `greeting.txt`, and awaits the result. The returned session is `completed`, and its summary ends with `Acceptance: accepted; confirmation: confirmed.` At that same point `Store::confirmation_grade` for the final result returns `Unconfirmed`.

Reproduction: `summary_stays_bound_to_checked_final_artifacts`. The assertion that the consumer grade agrees with current evidence fails. `narration-artifact-drift.json` preserves the actual final trace. The altered bytes remain different; this is not an undetectable change-and-restore race.

Required correction: reconcile the final consumer outcome with current artifact/input versions after optional work, preserving historical acceptance separately. Record invalidation or report the current grade honestly and require a fresh result/review where appropriate. Keep normal narration and YMP-104 fallback consistent for both accepted grades. This does not require a filesystem sandbox or prevent concurrent external writers; it requires detecting the persistent change at the existing final consumer boundary.

## Commands and discriminating evidence

Working directory for repository commands: `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/117`.

| Command | Actual result |
| --- | --- |
| `git diff 57c325b..52e692b` (all changed files read in bounded groups) | Exit 0; complete diff inspected |
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| `cargo test --workspace` | Exit 0; all suites pass |
| `cargo test --manifest-path /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-117-review-probes-j4afrbna/Cargo.toml -- --nocapture` | Exit 101; 10 tests, 5 pass and 5 fail at the rejection/consumer assertions above |

The external crate uses path dependencies on the unchanged reviewed production packages. Its exact test source is `src/lib.rs`; raw output is `probe-output.txt`; full before/after traces are in `captures/`, all under the manifest directory above. No production source was copied or modified for probes. Failed public calls are expected to preserve the captured state and journal after correction.

Independent passing controls from that crate:

- No contract and an unrelated passing command both complete as accepted/unconfirmed with zero observations.
- A passed evidence record changed to exit code 1 is rejected with `Passed check needs existing artifacts and success`.
- A failed exact-byte assertion changed to a claimed pass is rejected with `Checker success contradicts captured typed assertion`; no credit appears. The actual broken artifact provides the adverse input, not a changed expected test result.
- Changing a declared source after real confirmation changes the current grade to unconfirmed and makes `observe_confirmed` reject replay.
- A deliberately different injected executor returns its own raw output without delegating to the built-in checker. The runtime captures its ID/version. Raw success over matching typed bytes yields one observation; missing exit status yields an inconclusive check and zero credit; changing input bytes during the check yields inconclusive/unconfirmed with zero credit; changing artifact bytes yields an inconclusive blocked result with zero credit. This demonstrates replacement changes execution while leaving scope/freshness/credit authority outside the executor.

The repository tests also exercise genuine exact-byte and pinned-command success, supplied-input mismatch, failed applicable evidence despite approving agents, partial criteria coverage, independent final-review exclusion when every agent contributed, exact observation replay, historical unknown observations excluded from selection, changed artifacts requiring a second attempt, and unchanged-artifact narration failure with both accepted grades. The original no-check regression on baseline `57c325b` (exit 101 with two positive observations) is parent-provided prior evidence; this review independently reran the current public no-check and unrelated-command controls rather than treating that prior claim as its only evidence.

## Limits and next round

No real provider inference, native credential reads, bridge execution, interactive TUI walk, deployment, intent edits, or task-registry edits occurred. Bridge code is unaffected. The required workspace tests include unattended TUI tests only. Native model quality, live permission enforcement, knowledge activation, the universal integration exporter, and combined YMP-102/YMP-120 admission remain outside this verdict.

The storage forgery probes establish an invariant failure in the public trusted API; they do not claim an external agent can directly call arbitrary Rust methods. The public API rejection requirement is explicitly part of this review scope. The command checker remains trusted in-process code for correct execution of a declared pinned command; the review does not claim a sandbox against a malicious in-process implementation.

R1 correction is bounded to F1–F4 and their regression evidence in the existing confirmation/runtime/storage surfaces. The next review must rerun these failing public probes, verify rejected writes leave no partial changes, exercise normal and failed narration for both grades, and rerun formatting, Clippy, and the workspace tests. No verdict is granted by the existing green suite alone.

Round ledger: `R1(2/10) RETURN 12/09 21:05 — forged acceptance/review bindings, replaced accepted text, and stale final grade → five failing public assertions and complete trace captures → correction pending`.

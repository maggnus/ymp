# YMP-102 independent review

12/09 20:41 HKT, 2026-09-12 — **RETURN R1(6/10)** for candidate `0f53ee7e894832b1156cfcc6544ecf1bef027149` against its parent `56d95d5`. Critical shared-budget outcome. Code 6/10, work 8/10, experience 6/10; the lowest score reflects a reproduced invocation-budget violation after legacy resume.

## Required correction

**F1 — Historical unknown invocations are reused as fresh allowance during a resumed run (major).**

`ymp-storage/src/budget.rs:38,104` uses `session_usage.total.calls` for admission. `ymp-storage/src/usage.rs:121` treats the historical session count as a lower bound on the number of currently detailed rows. New admission allocates an ordinal beyond that historical count in `ymp-storage/src/provenance.rs:379`, but does not advance the durable count used by the next admission. New detailed rows consequently replace the previously unknown calls in the budget view.

An independent probe using only public storage APIs saved a legacy session with `turns_used=2`, no detailed usage rows, then captured `Limits { turns:3, parallel:4, resources:None, ..Default::default() }`. The first admission correctly received ordinal 3. A second admission incorrectly succeeded with ordinal 4; its snapshot still reported `admitted_invocations=2`, `in_flight_invocations=2`, and no denial. Both calls fit only because historical spend was lost from the admission calculation.

The same condition was exercised through public `Engine::run(path, "Create a greeting", Some(session_id))`, using the offline mock backend and two captured agents. The session started with two spent invocations and a captured total of three. It admitted three new native invocations and finished with `turns_used=5`, finally denying a bid with `invocation_limit`. This is observable runtime overspend, not only a storage helper discrepancy.

Correction must preserve historical unknown invocation spend alongside new admissions atomically, without fabricating token usage or changing captured policy. Cover legacy sessions both with no usage rows and with sparse historical rows; the next admission after the final remaining invocation must be denied across independent connections and after reopening. Preserve the existing ordinal/assignment/reservation/event transaction and all existing usage coverage semantics. Re-run the public storage and runtime falsifiers and the required checks. No change to intent, task registry, UI, grants, or workspace policy is required.

## Evidence

The complete candidate diff was inspected, including the Rust core/config/provenance structures, SQLite admission and release paths, runtime call sites, provider forwarding and terminal drains, Claude SDK options, CLI request construction, tests, and resource-budget documentation. Acceptance was read from YMP-102, approved intent, runtime-contract admission requirements, and the universal `budget-reservations` and `partial-usage` fixtures. Source files remained unchanged; only this report was written in the checkout. Disposable probes and logs are under `/tmp/ymp102-independent.Krc5VQ`.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all --check` | Exit 0; `cargo-0.log` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0; `cargo-1.log` |
| `cargo test --workspace` | Exit 0; 160 passed, 0 failed, 0 ignored; doc tests passed; `cargo-2.log` |
| `npm run check` in `ymp-bridges/claude` | Exit 0; `bridge-0.log` |
| `npm test` in `ymp-bridges/claude` | Exit 0; 12 passed, 0 failed; `bridge-1.log` |
| `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s ymp-evals/tests -v` | Exit 0; 26 passed; `evals.log` |
| `CARGO_TARGET_DIR=<candidate>/target cargo test --manifest-path /tmp/ymp102-independent.Krc5VQ/Cargo.toml -- --nocapture` | Exit 101; 3 passed, 2 failed; both failures reproduce F1; `probes2.log` |

Independent probe results:

- `historical_spend_cannot_be_reused_during_concurrent_admission` failed on the actual candidate: the second admission returned ordinal 4 with limit 3. Its failing input isolates historic invocation preservation; it does not establish native token bounds.
- `public_runtime_resume_preserves_legacy_invocation_spend` failed on the actual candidate: historical spend 2, limit 3, final count 5, three new native invocations. This distinguishes an actual runtime budget violation from an accounting display issue.
- `atomic_reserve_partial_release_and_review_denial` passed: three concurrent independent SQLite connections competed for total 100, invocation reservation 30, protected review 30; exactly two succeeded with different ordinals and reserved total 60. After observing seven partial tokens and interruption, reopening preserved seven observed tokens, released reservations, rejected strict accounting, and denied even review with `unknown_usage`.
- `runtime_context_stop_then_usage_exhaustion_survives_resume` passed through `Engine::run`: context limit 10 rejected before any native invocation; a separate token-limited run retained positive observed spend and its captured 1,000-token policy after current configuration was replaced with defaults and the session resumed.
- `native_terminal_output_timeout_and_cancel_preserve_reported_usage` passed through `ymp_providers::run_turn` and the actual Codex stdio adapter, using synthetic native protocol messages. Completion and oversized terminal result retained nine finalized reported tokens; timeout and cancellation retained nine unfinalized tokens. These are transport checks, not model inference.

Candidate tests additionally exercised bounded ten-agent startup, minimum startup allowance with independent plan review, the shared counters for planning/bidding/execution/review/learning/synthesis, review reserve, monotonic observed spend, overshoot display, transactional event failure, and captured limit persistence. The bridge suite exercised `max_turns=2` reaching the physical SDK transport as `--max-turns 2`, plus invalid limit rejection. Those positive checks were run independently; the candidate documentation's separate source-mutation claims were not treated as independently observed evidence. Universal fixture tests validate the scripted protocol expectations, not integrated runtime quality.

## Scope and remaining limits

The new session path uses SQLite `IMMEDIATE` transactions for concurrent admission and accounts observed raw tokens plus unobserved reservation remainders. Incomplete closed accounting blocks further token-limited admission; absence remains absence. Native context is not fully measured, supplied prompt characters are not token counts, visible output limits do not cap hidden reasoning/tool data, and Claude `maxTurns` does not bound opaque retries. Codex/ACP native loop support is not claimed. No finite native token overshoot bound or strict equal-compute guarantee was established; the explicit refusal of that claim is correct.

The supplied context check rejects an oversized full prompt rather than trimming it, preserving full prompt provenance for later integration with accepted memory work. YMP-120 grant enforcement and the pending product workspace policy are separate and were not accepted by this review. No credential reads, paid provider inference, UI changes, intent changes, or task-registry changes occurred. Native process-tree teardown, quality comparisons, and real backend maximum-turn behavior were not re-established by these offline tests.

Round ledger: `R1(6/10) RETURN 12/09 20:41 — legacy historical spend is reused during admission; public Store and Engine probes fail → correction pending.` One of two returns used.

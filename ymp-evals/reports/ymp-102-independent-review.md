# YMP-102 independent review

12/09 21:36 HKT, 2026-09-12 — **ACCEPT R3(9/10)** on combined candidate `f215df35896d0293044da68d8958f47850cd42dd`. Critical shared-budget outcome, including the necessary YMP-120 admission integration. Code 9/10, work 9/10, experience 9/10. F1 and F1b are closed: historical invocation spend is conserved on automatic and explicit admission paths, valid next-ordinal admission works, and rejection commits no new grant. No contracted finding remains open.

## Full round ledger

| Round or decision | Verdict and moment | Finding, answer, and result |
| --- | --- | --- |
| R1 | RETURN R1(6/10), 12/09 20:41 HKT | F1: historical spent count was replaced by new detailed rows. Public Store admitted ordinals 3 and 4 with spent 2 / limit 3; public Engine resumed to count 5. Correction `899e990` / combined `7c31a4b` introduced a durable cumulative count, preserved stale saves, and changed runtime recovery to use that count. The exact original public probes subsequently passed. |
| R2 | RETURN R2(6/10), 12/09 21:17 HKT | F1b: the explicit-ordinal compatibility path still admitted fresh assignments at historical ordinals 1, 2, and 3 with spent 2 / limit 3. All three capabilities worked through the team socket. Independent suite: eight passed, this ninth probe failed. The initial two-return allowance was exhausted. |
| Parent decision | Limited extension, 12/09 21:30 HKT | Parent granted one additional allowance of at most two returns, strictly for F1b: conserve historical spend through every wrapper and make only the necessary compatibility adjustments. No broader scope was authorized. |
| R3 | ACCEPT R3(9/10), 12/09 21:36 HKT | `f215df3` requires every explicit admission to supply exactly the next unspent ordinal inside the shared transaction, before grant issuance. The CLI diagnostic starts at zero spent calls. Exact nine public probes changed from exit 101 to exit 0; additional public wrapper-matrix and post-grant rollback probes pass. No additional return was used. |

The original R1 report remains unchanged in the standalone `102` checkout at `ymp-evals/reports/ymp-102-independent-review.md`. The pre-R3 combined report is retained at `/tmp/ymp102-r3-independent.HF68CW/r2-report-before.md`.

## R3 correction and independent evidence

The complete nine-file delta `cd86d49..f215df3` was inspected. The change stays within F1b, its required compatibility adjustment, tests and documentation. `Store::begin_invocation_inner` now computes `next_turn` once inside the existing SQLite `IMMEDIATE` transaction. Automatic admission uses it; explicit admission requires equality before `authority::issue`. Historical holes and skipped ordinals cannot create fresh invocations or grants. The explicit `Store::begin_invocation`, `Store::begin_invocation_with_grants`, and `TeamServer::admit` APIs retain valid next-ordinal calls. Their former out-of-order behavior is deliberately tightened and documented; callers with uncertain order use automatic allocation.

The CLI team diagnostic previously saved one spent call before admitting ordinal 1. It now starts with zero and charges at actual admission. Its production-function test reaches the expected offline missing-`team_post` diagnostic, retains exactly one invocation and 120 reported tokens, and revokes the issued grant. The retained adverse precharge test fails with `invocation_ordinal` instead of reaching that diagnostic, so it distinguishes the compatibility regression.

The unchanged nine public probes from R2 were rerun against the exact R3 candidate. They all passed. The previously failing `explicit_ordinal_compatibility_cannot_reuse_historical_budget` now denies the historical ordinals and admits at most the one remaining fresh invocation. These are the same assertions that independently failed on `cd86d49`, not weakened replacement assertions.

Two additional independently written public probes establish the positive and failure boundaries:

- **Every explicit wrapper, twelve combinations:** `Store::begin_invocation`, `Store::begin_invocation_with_grants`, and `TeamServer::admit` were each exercised with absent/sparse usage rows and captured-policy/legacy-limit sessions. With spent 2 / limit 3, ordinals 1, 2, and 4 were rejected with `invocation_ordinal`; no invocation, grant-issued event or spent count committed. Three independent connections then competed for valid ordinal 3: exactly one succeeded. Its reported nine tokens survived completion, stale session save and reopen; cumulative calls remained three and coverage remained partial because history was unknown. The server token worked through the actual Unix socket and failed after completion. Another admission was denied by the captured invocation limit.
- **Rollback genuinely occurs after grant issuance:** a completed invocation at ordinal 1 was followed by an explicit attempt at valid ordinal 2 using the old invocation ID and a fresh assignment/grant. The observed error was specifically `UNIQUE constraint failed: invocations.id`, confirming the attempt reached the invocation insert rather than failing ordinal validation. Source order places grant issuance and assignment insertion before that insert. No new grant, event, assignment or counter survived rollback. Retrying with a fresh invocation ID admitted ordinal 2, produced a working socket capability, and expired it on cancellation.

The adjusted permanent rollback test uses the same valid ordinal 2 / duplicated invocation ID construction. Its former ordinal collision would now reject before grant issuance, so this adjustment preserves the test's original invariant rather than merely preserving a green result.

## Preserved budget and authority behavior

The nine public probes also reconfirmed the earlier automatic path and joint controls:

- Historical spent 2 / total 3 permits only one automatically allocated admission, including through public `Engine::run` resume. Absent/sparse histories, policy/no-policy captures, separate connections, reopen and stale saves preserve the cumulative count and unknown coverage.
- Three competing automatic admissions with total 100, reservation 30 each and protected review 30 yield exactly two grants at ordinals 1 and 2. The third denial creates no invocation or grant. After cancellation, both socket capabilities fail; fourteen observed partial tokens remain, reservations release, and even required review is denied under the incomplete-token policy.
- An injected reservation-event failure rolls back admission and grant issuance together. An injected terminal-event failure removes live authority while retaining the durable invocation and reservation for recovery; recovery preserves seven observed tokens, revokes the grant, and releases the reservation.
- Actual Codex stdio adapter fixtures retain nine reported tokens through completion, oversized terminal output, timeout and cancellation. Completion/output accounting remains finalized; timeout/cancellation accounting remains unfinalized. These are offline protocol checks, not provider inference.

The cumulative counter introduced for F1 remains the maximum of saved session count, durable ordinal and highest detailed ordinal. Missing historical calls retain unknown coverage alongside new calls. The joint admission transaction still commits ordinal/accounting/grants/events together; live capabilities are published only after successful commit. Terminal revocation and reservation release remain atomic, with live authority removed before attempting a terminal write.

The unchanged workspace checks cover bounded startup, protected required review, monotonic observed spend, partial/unknown fail-closed token admission, overshoot reporting, context rejection, native output/time stopping, grant expiry/recovery, memory provenance and recorded checks. Provider error redaction remains outside native validation, output/timeout handling and the final event drain. Physical Claude SDK transport still receives `--max-turns 2` and fresh assignment credentials on continuation. YMP-106 memory retrieval/source evidence and YMP-107 recorded checks were preserved by the integration and were not edited by R3.

## Commands and actual results

Independent R3 logs are under `/tmp/ymp102-r3-independent.HF68CW`.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all --check` | Exit 0; `cargo-0.log` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0; `cargo-1.log` |
| `cargo test --workspace` | Exit 0; 208 passed, 0 failed, 0 ignored; doc tests passed; `cargo-2.log` |
| `npm run check` in `ymp-bridges/claude` | Exit 0; `bridge-0.log` |
| `npm test` in `ymp-bridges/claude` | Exit 0; 13 passed, 0 failed; `bridge-1.log` |
| `CARGO_TARGET_DIR=<candidate>/target cargo test --manifest-path /tmp/ymp102-r2-independent.Roza5C/probes/Cargo.toml -- --nocapture` | Exit 0; exact nine R2 public probes passed; `nine-public-probes.log` |
| `CARGO_TARGET_DIR=<candidate>/target cargo test --manifest-path /tmp/ymp102-r3-independent.HF68CW/probes/Cargo.toml r3_ -- --nocapture` | Exit 0; two additional public probes passed, nine already-run probes filtered; `additional-public-probes.log` |
| `git diff cd86d49 f215df3 --check` | Exit 0 |
| `git diff cd86d49 f215df3 --name-only -- intent.md ymp-docs/tasks ymp-rust/crates/ymp-tui` | Exit 0, no paths |

The new public tests are `r3_all_explicit_wrappers_conserve_legacy_calls_and_allow_the_next_one` and `r3_duplicate_invocation_id_fails_after_grant_issuance_and_rolls_back`, in `probes/src/lib.rs`. They use disposable application databases and live local sockets. No test prints a capability value.

## Adverse evidence and prior-round record

F1 was independently reproduced in R1 at the public Store and Engine boundaries: `/tmp/ymp102-independent.Krc5VQ/probes2.log`, exit 101, three passed and two failed. F1b was independently reproduced in R2 through public explicit admission and working socket capabilities: `/tmp/ymp102-r2-independent.Roza5C/public-final.log`, exit 101, eight passed and one failed. The unchanged nine-probe suite now exits 0 against R3. These before/after checks distinguish actual conservation failures from accounting presentation alone.

Retained author-executed artifacts were read and checked against the relevant implementation; they were not re-executed by the reviewer in the read-only checkout:

- `/tmp/ymp102-f1b-r_90argd/storage-wrapper-before.log` and `permanent-controls-before.log` show exit 101 on accepted historical ordinal 1 through storage and server wrappers. Current permanent controls pass in the independent workspace run.
- `cli-precharge-mutation.log` shows exit 101 when precharging the CLI diagnostic count: admission requests next ordinal 2 instead of reaching the native result. The current diagnostic test passes independently.
- `/tmp/ymp102120-integration-t_0dy069/joint-budget-disabled.log`, `joint-ordinal-allocation-disabled.log`, and `joint-grant-revocation-disabled.log` respectively show three admissions where two fit, failure to allocate distinct ordinals, and missing durable revocation, each exit 101. Their code and controls were inspected in R2 and remain covered by the mandatory run.
- `/tmp/ymp102-f1-my4lvywe/admission-mutation-replay.log`, `queued-terminal-usage-mutation-replay.log`, and `physical-maxturns-mutation-replay.log` are explicitly labelled fresh replays, not the original tool outputs. They distinguish disabled admission, lost queued terminal usage, and absent physical loop-limit forwarding.

The R1 mandatory run passed 160 Rust and 12 bridge tests; the R2 mandatory run passed 205 Rust and 13 bridge tests. Those historical counts are separate from the independently observed R3 counts above. Universal fixture expectations were read for the review; their standalone validator suite was last run in R1 with 26 passing tests and was not rerun in R2 or R3.

## Explicit limits

Acceptance covers supported resource accounting/admission and the necessary budget/authority integration. No finite native token overshoot bound, strict equal-compute claim, native-loaded-context bound, hidden reasoning/output bound, or opaque-retry bound has been established. The explicit refusal of strict token-budget claims and preservation of partial/unknown coverage are correct. Native loop forwarding is verified offline; real inference quality, real maximum-turn enforcement and process-tree teardown were not newly established by these reviews. The pending product workspace policy remains outside scope.

No native authentication source was read, no provider inference was run, and no source, intent, task-registry or TUI file was edited by the reviewer. Synthetic assignment capabilities existed only in disposable test runtimes and transport. Only this review report was updated in the combined checkout; the original standalone R1 report was preserved.

Parent integration note: the limited extension was granted at 21:17 HKT; 21:30 was the subsequent R3 review request. The reviewer ledger above retains its originally reported timestamp. Main integration preserves the reviewed source and passes 208 Rust tests and 13 bridge tests.

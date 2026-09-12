# YMP-110 independent review

12/09 23:37 HKT (2026-09-12T15:37:13Z) — **ACCEPT R2(9/10)** for correction 535b6193913057eb7d808bfadf3f5aeb0b944e05 over outcome ec552bbec842928c33b257b6d385f047bb0f25c1.

Both R1 findings are closed. Code: **9/10**; work/evidence: **9/10**; public API experience: **9/10**. The complete six-file correction diff and permanent public controls were read. No contracted defect remains open in this backend outcome.

**R2 answers and independent evidence**

F1 is closed by checks in the shared immediate admission transaction and in allocation commitment under the same SQLite write serialization. Both explicit and automatically allocated ordinal wrappers deny production by the reserved reviewer, reject a reservation that is no longer eligible or already produced work, and keep ordinary current executors admissible. Reservation commitment rechecks actual producer assignments, including assignments admitted after the policy snapshot and those already closed. The barrier-controlled public tests exercise both operation orders: production is admitted while a reservation proposal waits, and reservation is committed after assignment records have been prepared but before admission. Rejection preserves membership, spent calls and grant state; the prepared-record denial preserves the complete trace. Reservation remains eligibility, without issuing standing authority.

F2 is closed by metadata-only pin validation before startup and review-configuration validation at allocation boundaries. Known unsupported pinned model, unsupported effort across allowed models, absence of an applicable effort control, and unusable required review settings fail with the affected agent/settings identified before any startup session or backend request. Supported pins still reach actual requests; an effort pin can choose a different supported model without changing the effort. Absent catalogs, unknown controls and an unlisted model in an incomplete catalog remain unknown, proceed through the scripted consumer, and gain no invented native effort acknowledgment.

The exact seven-probe disposable suite used in R1 now exits 0 with all seven probes passing. Its previously observed three failures are retained in the historical R1 report below; those same commands now discriminate the corrected behavior. The permanent public suite additionally exercises the two reservation/admission interleavings. The author's correction evidence was checked against source and actual command results. Source mutations were not replayed under the read-only assignment; this acceptance relies on the independently observed R1 failures, unchanged retained probes, actual public transitions and reviewed transaction logic.

| Independently run command | Exit | Observed result |
| --- | --- | --- |
| cargo fmt --all --check | 0 | Reviewed workspace formatting. |
| cargo clippy --workspace --all-targets -- -D warnings | 0 | Warnings denied across all workspace targets. |
| cargo test --workspace | 0 | 256 tests passed; /tmp/ymp110-review-r2-workspace.log. |
| cargo test --manifest-path /tmp/ymp110-review-u71mgk/Cargo.toml -- --nocapture | 0 | Original seven public probes passed; /tmp/ymp110-review-r2-probes.log. |
| cargo test -p ymp-runtime --test allocation_admission -- --nocapture | 0 | All 13 permanent public controls passed; /tmp/ymp110-review-r2-admission.log. |
| PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s ymp-evals/tests -v | 0 | 26 validator tests, including deliberate malformed protocol controls; /tmp/ymp110-review-r2-protocol.log. |
| git diff --check | 0 | Tracked source remains unchanged. |

The correction does not change membership ceilings, fixed size/roster semantics, captured limits, historical identity or invocation attribution. The complete workspace run retains the allocation, resource substitution, qualified-experience, fixed-size replacement and same/distinct-task consumer coverage recorded in R1.

**R2 limits and integration**

Only this English review report was changed by the reviewer. Tested workloads remained mock/scripted; no real provider inference, credentials, UI, intent or task-registry edits occurred. Concurrent checks use deterministic barriers and the real public storage/admission operations; they establish the two relevant serialization orders, not arbitrary scheduler throughput or production isolation. The owner-approved direct working directory policy applies only to the 0.4.0 MVP. YMP-115 integration and post-MVP isolation/recoverable publication in YMP-124 remain separate.

The YMP-118 shared DTO integration requirements and the YMP-121 runtime-to-universal exporter limits recorded below still apply. This accepts the backend correction and outcome; it does not independently accept the UI, bridge behavior, live-provider efficiency, workspace concurrency or the entire release.

**Historical R1 record — findings subsequently closed by R2**


12/09 23:12 HKT (2026-09-12T15:12:19Z) — **RETURN R1(5/10)** for code outcome ec552bbec842928c33b257b6d385f047bb0f25c1.

The bounded allocation implementation works through the normal engine consumer, and current membership prevents retired identities receiving fresh grants. Two contracted invariants remain open: the public admission wrappers can consume the reserved final reviewer, and a reviewer with contradictory native effort pins is treated as eligible until after startup spends resources.

Code: **5/10**; work/evidence: **7/10**; public API experience: **5/10**. The lowest axis sets the round score. The open findings concern review feasibility and public admission, not numerical policy optimization.

**Reviewed scope and authority**

The complete 22-file outcome diff was read against current-main YMP-110 acceptance, the approved intent.md, entity definitions, team/effort policy, runtime contract, replaceable subsystem interfaces, and the fixed-size, fixed-roster, adaptive-team and effort-support universal cases. The team skill governed independent review. The preceding effective-execution identity correction 979661b was retained as the supplied parent baseline; its configuration-qualified consumer is exercised by the existing experience test. This report does not reclassify the separately mapped parent correction 01b2053.

Source remained read-only. The only repository write is this report. Disposable Rust consumer code and logs are in /tmp/ymp110-review-u71mgk, with path dependencies on the reviewed packages. Workloads used only scripted execution or the repository's existing offline fixtures. No live model, native inference, credential read, token logging, or credential copying occurred. Synthetic live grant secrets were discarded; diagnostics show grant counts only.

**F1 — Major: public admission consumes the reserved final reviewer**

Locations: ymp-rust/crates/ymp-storage/src/provenance.rs:422, ymp-rust/crates/ymp-runtime/src/engine/allocation.rs:447, and the existing public wrappers in ymp-rust/crates/ymp-runtime/src/mcp.rs:80.

The allocation validator rejects an execute proposal whose executor is the reserved final reviewer. That protection ends before the common admission transaction. begin_invocation_inner checks captured identity, current membership and observed eligibility, but never checks reserved_final_reviewer or preserves an independent eligible identity against the newly admitted production assignment.

Reproduction through the real public API:

1. Complete an ordinary scripted two-agent engine run. Its captured/current team contains one and two; production is attributed to one, and TeamState.reserved_final_reviewer is two.
2. Save a fresh ready task, assign it to two, and build fresh running assignment/invocation records for this task attempt from the real trace. Retain the same session and provider, clear terminal fields and prior grant IDs, and use the next unspent ordinal.
3. Start the public TeamServer and call admit(..., TeamOperation::coordination()), or the automatically allocated ordinal wrapper admit_reserved.
4. Both calls succeed and issue one fresh live grant. The synthetic invocation is explicitly finished immediately afterward.

Observed failing assertions:

    PUBLIC ADMISSION CONSUMED RESERVED REVIEWER:
    agent=two; grants=1; members=["two", "one"]

    RESERVED-ORDINAL ADMISSION CONSUMED RESERVED REVIEWER:
    agent=two; grants=1

The ordinary current executor control succeeds through the same records and wrapper. The retired-member negative control is rejected with ineligible_member and issues no grant. Therefore this failure concerns reservation protection, not invalid fixture identities or a generally unusable admission path. Neither an LLM nor a forged acceptance record is involved.

Correction required: enforce final-review feasibility in the shared atomic admission boundary, including both public wrappers. Keep reservation/membership decisions consistent with assignments admitted since their input snapshot, so a policy commitment cannot install a reviewer that has become a producer concurrently. Reject without creating a grant, spending an invocation or partially changing authoritative state; do not silently expand a pinned roster. Reserving eligibility must still confer no standing assignment authority.

Closing evidence: both public wrappers reject the above fresh production assignment while an ordinary current executor remains admissible; membership/reservation changes and admission cannot interleave to leave the last eligible final reviewer producing the result. The source currently serializes membership commits and admissions, but there is no reviewer guard in the admission transaction. A concurrent reservation-change interleaving was inspected, not dynamically stress-tested in this round.

**F2 — Major: infeasible pinned reviewer passes startup feasibility**

Locations: ymp-rust/crates/ymp-runtime/src/allocation.rs:24, ymp-rust/crates/ymp-runtime/src/engine/allocation.rs:283, ymp-rust/crates/ymp-runtime/src/engine/allocation.rs:400, and ymp-rust/crates/ymp-runtime/src/engine/allocation.rs:437.

Reservation uses pool eligibility by identity. Native setting validation removes invalid configurations from execution candidates, but does not establish a feasible configuration for the reserved reviewer. The startup validator only requires two pool identities and a nonproducing reserved identity.

Reproduction through Engine::run:

- Pin roster ["one", "two"].
- Configure a complete capability catalog containing model-a, whose effort choices contain only brief.
- Pin agent two to model-a and effort unsupported; leave one usable.
- Run the ordinary scripted task.

The engine invokes plan using one, then blocks when independent plan review has no valid executor. It does not reject the already known contradiction before spending startup resources, and its final diagnosis hides the unsupported effort.

    INFEASIBLE PINNED REVIEWER SPENT STARTUP:
    purposes=["plan"];
    outcome=blocked: Run blocked: no_independent_eligible_reviewer:
    no executor remains while preserving independent final review

Changing only the pinned effort from unsupported to supported brief completes the same workload, and every request to two carries brief. This pair distinguishes an actual captured native-setting contradiction from a general failure of fixed rosters or the script.

Correction required: derive and validate usable independent review configurations under the captured model/effort pins and applicable native capabilities before admitting startup and at subsequent allocation boundaries. A pool identity alone cannot satisfy the protected-review requirement. Report the affected agent/model/effort conflict; never change its pin or add an undeclared reviewer.

Closing evidence: the unsupported pinned reviewer yields a precise contradiction with zero model/backend invocations, zero grants and zero admitted spend; the supported control still completes.

**Evidence observed in this round**

| Command | Exit | Result and practical limit |
| --- | --- | --- |
| cargo fmt --all --check | 0 | Reviewed workspace formatting. |
| cargo clippy --workspace --all-targets -- -D warnings | 0 | Reviewed workspace, warnings denied. |
| cargo test --workspace | 0 | Full reviewed workspace suite completed. Native-error checks are existing offline shim fixtures; no real provider inference. |
| cargo test -p ymp-runtime allocation_contract_tests -- --nocapture | 0 | All 13 allocation contract tests passed. These do not cover F1 or F2. |
| PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s ymp-evals/tests -v | 0 | All 26 validator tests passed, including missing/extra protocol events and deliberate fixed-size, roster, history and sent-effort corruption. Validator discrimination does not prove a runtime export. |
| cargo test --manifest-path /tmp/ymp110-review-u71mgk/Cargo.toml -- --nocapture | 101 | Seven independent public-consumer probes: four passing controls and three contract failures establishing F1 through both wrappers and F2. Raw output: /tmp/ymp110-review-u71mgk/results.log. |

The independent passing controls cover ordinary public executor admission; fixed-size replacement with append-only captured profiles and unchanged original policy/spend; retired-ID denial through the reserved-ordinal wrapper; a rejected three-member proposal against fixed size two; supported pinned effort reaching actual backend settings; and same-task steering versus a distinct task. Steering increments the existing session's calls without resetting history, whereas the distinct task gets a new session with a parent link.

The allocation suite exercises dynamic reduction versus fixed-roster rejection, independent review under fixed size one and a two-agent roster, active-responsibility departure denial, changed availability, fixed effort across config edits, native model-specific effort options, configuration-qualified experience versus unconfirmed observations, failed-check reconsideration without automatic effort escalation, and two operational resource policies. The ten-agent startup test in the workspace suite confirms useful work starts without mandatory pool-wide bids. Substituted selection and resource policies reach actual TurnRequest settings and native controls; rejected proposals remain decisions without invocation side effects.

The author's five restored mutation transcripts in ymp110-allocation-checks.json were read, and the guarded tests were run on the final source. Those source mutations were not replayed under this assignment's read-only source constraint. Independent failing forms were observed through malformed policy/native inputs and actual public admission, rather than treating the author's mutation account as independent evidence. The two newly found invariant failures remain reproducible despite the green authored suite.

**Contract assessment and integration limits**

Current TeamState membership and latest observed eligibility are checked in the same immediate SQLite transaction that creates assignment, invocation, budget reservation and grants. Allocation commits preserve historical Session.team profiles and record membership, chosen settings and the decision together. Stale team revisions and removal of active assignments are rejected. Initial SessionPolicy.captured_team, constraints and source names remain separate from current membership and cumulative usage.

Allocation and resource interfaces carry typed inputs and ID/versioned decisions without exposing a mutable store to the policy. Runtime validation protects fixed size, roster, pool restrictions, membership ceilings, model/effort choices and resource ceilings. The reserved-review defects above are the remaining material gaps in that boundary. Complexity, risk, ready work, resource limits and qualified experience reach policy input; the built-in uses explicit heuristics without a universal effort ladder or an optimization claim. Agent-created task proposals inform task demand and bounded board suggestions are available to replacement policies. Coordination revision commitments remain YMP-112.

YMP-118 must render current participants from SessionTrace.team_state.current_members, preserve labels and historical attribution through SessionAgentView::from_captured, and distinguish current size, historical participation, eligible pool and live invocations. It must expose the selected method, reservation as eligibility only, and rejection reasons from allocation/resource decisions without displaying the reserved reviewer as a permanent authority. Initial captured policy remains independently inspectable. No TUI source was reviewed or edited here; Opus owns that integration and it is not accepted by this backend report.

The declarative universal cases were read and their validator negative controls run; no complete runtime-to-universal exporter was added or certified. YMP-121 still owns integrated projected trace acceptance. Useful concurrent scheduling/workspace policy (YMP-115), board revision commitments (YMP-112), knowledge lifecycle (YMP-113/114), bridges and live provider performance remain outside this outcome. No additional feature work is requested by this return.

Round ledger: **R1(5/10) RETURN 12/09 23:12** — public admission consumes reserved final eligibility; unsupported pinned reviewer spends startup before contradiction → three independent failing assertions, four valid controls → correction pending.

Round ledger: **R2(9/10) ACCEPT 12/09 23:37** — F1/F2 → original seven probes change from exit 101 to exit 0; both public admission paths and both reservation/admission orders hold, and known pin conflicts fail before startup while supported/unknown settings proceed → shared transaction guards and metadata feasibility checks accepted in 535b619.

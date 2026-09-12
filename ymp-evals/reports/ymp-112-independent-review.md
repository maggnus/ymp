# YMP-112 independent backend review

13/09 02:58 HKT, 2026-09-13 (2026-09-12T18:58:29Z) — **RETURN R1(6/10)** for exact HEAD `92691607c2652c9fcc913f658e5971509c4dab74`, based on `9ad43dccb29840089a3f351c5651867aa808135d`. Code **6/10**; evidence **8/10**; public Engine experience **6/10**. F1 is the sole return condition.

The durable proposal, source authority, version and revision paths work in the submitted serial scenarios. An independently added ordinary parallel scenario blocks otherwise feasible work by treating temporary wave occupancy as permanent unavailability of a committed agent. It also leaves another selected task in Running state before that task has received an invocation. Whole-task YMP-112 acceptance additionally remains open for the separately delegated Opus UI work.

## F1 — A current commitment blocks the wave when its agent was selected for another ready task

Affected paths: `ymp-rust/crates/ymp-runtime/src/engine/board.rs:457` and `ymp-rust/crates/ymp-runtime/src/engine.rs:1679`, `:1691`, `:1707`, `:1711`.

The scheduler removes identities already selected in the current wave from `candidates`. `committed_executor` interprets any absence of the committed identity from that filtered slice as `commitment_unavailable`, claiming explicit reassignment is necessary. When some other eligible candidate remains, the scheduler reaches this error rather than its empty-candidate waiting branch. It has already committed preceding task claims, but only spawns their invocations after finishing the entire selection loop. The error therefore also prevents the previously selected work from starting.

A public Engine consumer, using the actual team socket and unchanged production crates at the reviewed HEAD, exercises this sequence:

1. First produces the existing exact-byte confirmed proof and uses `task_propose` to assign Third to agent `one`, with the captured `small` model and `low` effort.
2. Second and Third both depend only on First. They are ready together under `parallel=2`, the existing fixed roster `one/two/three`, and the usual independent reserved reviewer.
3. The default allocator selects `one` for Second and atomically claims it. Third still has a valid explicit commitment to that same current, eligible agent.
4. The attempt to select Third returns `commitment_unavailable`; the run stops before Second starts. No membership, native capability, pin, budget or prerequisite makes completion infeasible.

Independently observed response:

```text
STATUS blocked
SUMMARY Run blocked: commitment_unavailable: explicit reassignment is required for an unavailable responsible agent
TASKS [(First, Accepted, one), (Second, Running, one), (Third, Ready, none)]
COMMITMENTS [(accepted, Third -> one)]
EXECUTIONS 1
```

The completion assertion fails **101**, reproduced twice. The same test source, tasks, proposal, roster, settings and assertions pass **0** when only `PROBE_PARALLEL=1` is used: all three tasks are accepted, Third executes as `one`, and the session completes. This localizes the failure to wave scheduling rather than unsupported responsibility or model settings. The final aggregate honestly remains unconfirmed because only First has the supplied exact-byte contract; the probe does not equate completion with universal confirmation.

Required correction: distinguish an agent temporarily occupied by selected/current work from an agent that truly requires explicit reassignment. Preserve the recorded commitment and safely defer or order that task while letting feasible selected work run. The wave must not turn a normal dependency on agent availability into a user-visible permanent block or abandon earlier claims before invocation. Retain current membership, pins, reviewer eligibility, atomic claim protection and uncertain-effect recovery. Add the parallel same-owner case as a permanent public regression; lowering the configured concurrency is only a diagnostic control.

## Independent reproducer

The external crate `/tmp/ymp112-review-mpha21ll` depends on the exact reviewed public core/runtime/storage/provider crates. It copies the existing socket fixture into test code, explicitly rejects any non-Mock provider, and adds no runtime source patch. In its `tests/board.rs`, the Reassign scenario assigns Third to `one`, changes Third's dependency from Second to First, and appends `independent_parallel_commitment_waits_for_its_owner_without_blocking_feasible_work`. The parallel width comes from `PROBE_PARALLEL`, defaulting to two. This is an ordinary reviewed plan plus an actual admitted participant proposal, not manufactured task completion or board history.

Exact repeated command, with `PROBE_PARALLEL=2` and then `PROBE_PARALLEL=1`:

```sh
cargo test --offline --manifest-path /tmp/ymp112-review-mpha21ll/Cargo.toml --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/sibling112/target --test board independent_parallel_commitment_waits_for_its_owner_without_blocking_feasible_work -- --nocapture
```

| Control | Actual exit | Retained log |
| --- | --- | --- |
| Parallel width two, initial public failure | 101 | `/tmp/ymp112-independent-parallel.log` |
| Parallel width two, unchanged-source repeat | 101 | `/tmp/ymp112-independent-parallel-repeat.log` |
| Parallel width one, same workload | 0 | `/tmp/ymp112-independent-serial.log` |
| Parallel width one, same-source repeat | 0 | `/tmp/ymp112-independent-serial-repeat.log` |

The same repeated probe source SHA-256 is `b922138e29e425718b8eee3eb115442c948e8300b20f7df86bc3248c53956413`. The repeated failure log SHA-256 is `287dce3c62b9aadabf6868a73b19720d855f5f42babbf6d08878c50804ab9e59`; the repeated passing log is `12642a418758e92ec7e17c69f3ea92baa7c2da3d8ff3a0faf3d41630d0f9f910`. Exact arguments and statuses are retained in the external crate's `review-results.json`.

## Passing authority and transition evidence

The complete 21-file outcome diff, task acceptance criteria, `AGENTS.md`, architecture note and validation record were read. The shared allocation extraction preserves existing occupied-actor and independent-review checks while allowing board changes to share their transaction. The 15 changed Rust source files independently match their exact HEAD blobs. No UI or registry diff belongs to this submitted outcome.

`cargo test -p ymp-runtime --test board_coordination -- --nocapture` independently exits **0**, all nine submitted tests passing; `/tmp/ymp112-independent-board.log`. Those tests exercise actual Engine invocations and socket capabilities, not direct creation of committed board results. Their source and observed results support:

- Exact plan/task references, actual caller identity and grant/assignment/invocation binding; hidden identity and contract fields are rejected. An expired capability cannot add a proposal during another live assignment.
- Two conflicting agent proposals produce one committed responsibility and one stale-task rejection. Reverse policy ordering changes the actual executing winner while preserving runtime validation and recording the injected strategy identity.
- Task versioning includes the latest committed responsibility generation. Plan versions retain task identity/definitions while excluding lifecycle/location changes; team versions sort identity sets while retaining actual membership, eligibility and reviewer differences. A real membership change rejects the older proposal.
- Atomic competing public task claims have one winner and create no assignment, invocation or capability by themselves. The scheduler still uses native settings, workspace, budget and grant admission after claiming work.
- Additive revisions retain objectives, existing checks/dependencies, access and trusted contract bindings; a current responsibility survives an approach revision. Added work reaches independent task acceptance. Explicit reassignment retains its history and selects the new responsible identity.
- Previously confirmed First survives the recovery scenario unchanged. The runtime inspects Second's actual uncertain file before a fresh execution, rather than treating a failed invocation as proof of absent effects or replaying immediately.

An additional independent adversarial test, `independent_failed_source_proposal_remains_bound_and_cannot_commit` in external `tests/failed_origin.rs`, also exits **0**. It makes Second submit a genuine typed add-task proposal over its live team socket and then fail after writing the existing uncertain-effect artifact. The resulting proposal remains pending and names that exact failed assignment, invocation, agent and grant. Public `Engine.commit_board_proposals` rejects it with `proposal_origin`, preserves the Running uncertain task and earlier confirmed acceptance, and changes neither invocation count nor usage. This directly exercises failed-source proposal denial, which the original recovery fixture did not submit a proposal to test.

The command was the same external manifest/target command above with `--test failed_origin independent_failed_source_proposal_remains_bound_and_cannot_commit`. Evidence: `/tmp/ymp112-independent-failed-origin.log`. No capability value was printed or written into the review evidence.

## Required checks and evidence limits

The submitted validation files record formatting and denied-warning workspace Clippy passing, plus a complete **306-test** workspace pass. The workspace log was read and its summaries independently total 306; SHA-256 `e3a5d1091c754f9f8c0d683fcbea2d5e7ccf8d9b6ccdddab92a4450a1fc448d4`. These are author-run checks committed with the outcome, not a newly claimed reviewer full-suite execution. The reviewer reran the focused public board suite and the independent adverse/positive controls; no duplicate complete suite was needed to establish F1.

Both recorded removed-guard controls were inspected: disabling the engine consumer yields no durable decisions, and removing the prior commitment generation admits the second conflicting claim. Their logs show **101**, and current source retains both guards. Those source mutations were not independently replayed in this read-only review. F1 instead comes from an unchanged-production public workload that the original nine scenarios did not cover.

`git diff --check` passes. The only checkout addition is this report. All execution remained mock/scripted with small/low fixture settings; no live inference, credentials, main changes, intent changes, task-registry edits or commit occurred.

## Acceptance boundary

F1 concerns the interaction of temporary wave occupancy with explicit responsibility. It does not request new isolation, larger budgets, changed pins or a permanent agent role. Additive-only revision and existing native accounting limits remain documented MVP boundaries. Strategy efficiency and model quality are not established by these checks. The separately assigned UI and MCP output correction remain outside this review; whole YMP-112 is not complete while UI acceptance is outstanding.

Ledger: `R1(6/10) RETURN 13/09 02:58 HKT — durable proposal and authority controls pass, but a valid committed agent selected earlier in a parallel wave blocks feasible work before invocation; public width-two failure repeats and identical serial control passes → preserve commitment through ordinary waiting/scheduling and add regression`.


## R2 — F1 closed; backend accepted

13/09 03:26 HKT, 2026-09-13 (2026-09-12T19:26:14Z) — **ACCEPT R2(9/10)** for exact correction `154ed3f7a3d16b3c07c81182cfb91aecc6e84cda`. Code **9/10**; evidence **9/10**; public Engine experience **9/10**. **F1 is closed; no backend return condition remains.** Whole YMP-112 remains open for separate Opus UI acceptance. R1 above is preserved byte for byte; its pre-append SHA-256 is `90b64e5c78ee6510495e3ba7f84b65015d558221fe70219d69df224c95ff3468`.

The entire eleven-file correction diff and correction record were read. Production changes are confined to wave selection in `engine.rs`. The extracted `select_wave_task` checks the current commitment against identities already selected in that wave. A busy responsible agent produces `commitment_busy` and leaves the task Ready with its responsibility intact. Successful selection still performs native configuration/allocation validation and the unchanged atomic task claim, adding the actor to the wave's busy set only after the claim succeeds.

Already claimed tasks are spawned and independently reviewed even when selecting subsequent work returns a real error. The error is retained while those tasks drain. A wave with no selected task returns its concrete error or a bounded no-executor failure, so the new deferral cannot cause an empty busy loop. The correction neither reassigns temporarily busy commitments nor relaxes admission to force progress.

### Independently observed closure

The original external test source is unchanged from R1, SHA-256 `b922138e29e425718b8eee3eb115442c948e8300b20f7df86bc3248c53956413`. The exact width-two public Engine/socket probe that twice failed **101** in R1 now independently exits **0**. First, Second and Third are accepted; Third retains agent `one`; exactly three execution assignments occur. The overall result remains accepted/unconfirmed because the fixture provides an objective contract only for First. Scheduling is fixed without inventing aggregate confirmation.

The complete permanent board suite independently passes all **11 tests**. Its new same-owner consumer passes both widths two and one, checks the explicit waiting record, retained effort and responsibility, three execution assignments, one Third attempt and accepted task states. Its separate injected late-selection error establishes that previously claimed Second executes and is independently accepted before the run returns the real selection error. Third remains Ready with its commitment to `two`; no task is stranded Running.

The original nine scenarios also pass, retaining conflicting-proposal/version rejection, reverse-policy ordering, malformed authority/pin/contract denial, historical membership/reassignment, additive revision, atomic competing claims and uncertain-effect inspection. Source comparisons confirm that board preparation/commitment logic, runtime allocation constraints, storage board/allocation/admission, native authority, workspace coordination and recovery logic are unchanged by R2. The R1 independently passing failed-source proposal probe remains applicable to those unchanged paths; it was not repeated.

### R2 commands and evidence

| Independently run command | Actual result |
| --- | --- |
| `cargo test -p ymp-runtime --test board_coordination -- --nocapture` | 0; all 11 tests; `/tmp/ymp112-r2-independent-board.log` |
| Original external command below, `PROBE_PARALLEL=2` | 0; unchanged previously failing probe; `/tmp/ymp112-r2-independent-external.log` |
| Source comparisons against R1 and exact R2 HEAD | Passed; the only changed production path is the reviewed scheduler |
| `git diff --check` | 0; review remains report-only |

```sh
PROBE_PARALLEL=2 cargo test --offline --manifest-path /tmp/ymp112-review-mpha21ll/Cargo.toml --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/sibling112/target --test board independent_parallel_commitment_waits_for_its_owner_without_blocking_feasible_work -- --nocapture
```

The author's permanent regression also records the original R1 `commitment_unavailable` failure before correction. That agrees with the independent unchanged-test before/after evidence above; another temporary source mutation was unnecessary.

The final author formatting, Clippy and full-workspace logs were inspected. Full-workspace passing summaries total **308 tests**; log SHA-256 `ecf1611e2a9c8b64776f01cf6514b024074c6bc9f7622fff935a3ddce149fa46`. The current changed source/test files match their exact R2 Git blobs, with independently computed hashes:

- `ymp-runtime/src/engine.rs`: `e012cb0fdca39cac62dbba143c05721582f95c9ca54026b0f813490c2cbf7564`.
- `ymp-runtime/tests/board_coordination.rs`: `7cdaf7d73bcb4eab7e347e8122098760e02e1d9bef289e651a64db3cf097d3f6`.

Those full checks are author-run evidence delivered with the exact correction, not a newly claimed reviewer full-suite execution. Focused independent consumers and the unchanged original failing probe establish this bounded F1 closure without a redundant whole-suite repeat.

No source patch, native inference, credentials, UI work, main change, intent edit, task-registry edit or commit occurred in R2. Only this appendix was written. The updated owner instructions on native-scanned agent identities remain authoritative; this correction changes no discovery, selectable identity or native metadata behavior. Static small/low values here remain scripted fixture data. Existing MVP workspace, accounting and strategy-quality limits remain unchanged.

Updated ledger: `R1(6/10) RETURN → R2(9/10) ACCEPT 13/09 03:26 HKT — unchanged width-two failure becomes a passing public run; busy commitments wait without reassignment, prior claims complete before later selection errors return, and authority/recovery controls remain intact; whole-task UI acceptance still pending`.

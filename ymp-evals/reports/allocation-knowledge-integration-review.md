# Allocation and knowledge integration independent review

13/09 00:19 HKT, 2026-09-13 (2026-09-12T16:19:46Z) — **RETURN R1(7/10)** for exact combined HEAD `addba636a56653b1f1d2f14932cd3672e528f515`.

The allocation and knowledge authority integration passes. One bounded location-consumer defect remains: the deterministic answer names the original artifact correctly, but its public `RunOutcome.workspace` names the empty relocated registration. The existing TUI uses that field to say where the files are. Code **7/10**; evidence **9/10**; public consumer **7/10**. F1 is the only return condition; the individual YMP-110 and YMP-113 backend acceptances are not reopened.

## F1 — Location answer returns a contradictory workspace to its existing consumer

At `ymp-rust/crates/ymp-runtime/src/engine.rs:426`, the location-only branch sets `workspace: project.path.clone()`. After `Store.relocate_project` changes only registration, that directory can contain none of the recorded artifacts. The summary correctly reports the original absolute artifact paths. However, the unchanged consumer at `ymp-rust/crates/ymp-tui/src/lib.rs:111` renders `Run {}. Files are in {}.` using `outcome.workspace`; its status therefore contradicts the successful answer and directs the user to the empty directory. This consumer was read, not edited or independently accepted as UI work. CLI output labels the same field `Workspace`, but its current command path calls `Engine.run`, so this review does not claim an observed CLI follow-up failure.

An independent assertion was added only to an archived disposable copy of the exact reviewed source, immediately after the retained test's assertions about the location summary:

```rust
assert_eq!(
    answer.workspace, directory,
    "RunOutcome.workspace is consumed as Files are in by the existing TUI"
);
```

The unchanged joint workload still uses the public `Engine.follow_up(&relocated, "Where did you save it?", &source.session.id)`. The added assertion exits **101** with `left: .../relocated-empty` and `right: .../project`. The original artifact exists with its expected bytes; the destination is empty. Evidence: `/tmp/ymp-state-independent-result-workspace.log`, SHA-256 `1e37190d8a0898f358cebb425f04b95950ebf3d4a3bc1b8886df85c15a0304a1`. The reproducing one-assertion patch is `/tmp/ymp-state-independent-workspace-assertion.patch`. The temporary assertion was restored after the observation; reviewed implementation source was never modified.

Required correction: the location-only result must carry the captured directory expected by its existing location consumer, including a consistent stored-workspace fallback. Missing historical capture must remain explicit rather than silently becoming today's project registration. Extend the permanent public consumer to check the returned directory as well as the summary, retaining the zero-inference and unchanged-artifact controls. No new phrase router or UI redesign is required.

## Independently reproduced passing integration

The retained public joint test passes unchanged at the exact HEAD. It confirms the first task by exact bytes, persists supported knowledge before a later sibling fails, and retains exactly one supported competence observation without a final learning assignment. A policy replacement retires the original producer while preserving fixed size two, immutable initial policy/spend and all three historical agent names. Both public admission wrappers reject the retired producer and reserved reviewer with unchanged trace and no grant, while admitting the current ordinary executor. Two synthetic admitted failures add two calls and no credit.

Storage reopening and a distinct later session preserve the source identity, result version and supported entry. The actual captured later-session provider prompt receives `Hello from ymp`. Candidate lessons remain excluded by identity. Metadata-only relocation preserves the complete `Store.outcomes` value; the literal follow-up returns the original artifact path with no new invocations or backend requests and creates no destination artifact. The wrapper rejects every provider other than `Mock` before execution.

The exact universal-workflow phrase `Where did you save it?` is already present in both the pre-correction `e1a1519` route and the reviewed route, and is the phrase used by this passing joint test. There is no phrase-coverage defect here. The combined correction repairs the premature `Workspace::open` validation and mutable-root path assembly; F1 concerns the remaining structured result field.

## Shared authority and accepted source preservation

The YMP-110 R2 and YMP-113 R2 independent reports were read. Byte comparisons establish that the combined allocation policy, runtime allocation implementation, storage allocation transaction and all 13 permanent public allocation-admission controls match accepted mapping `388f099`. The knowledge policy, storage knowledge implementation and complete confirmation implementation match accepted YMP-113 correction `80017df6f226678a25aae1f7ab6864db6a00ed76`. Knowledge integration does not modify the accepted storage invocation-admission implementation.

Source inspection confirms one shared `effective_execution_version` implementation in `ymp-core/src/provenance.rs`, unchanged from parent base `2695d69`; runtime attribution delegates to it and `Store.observe_confirmed` validates against it. There is no duplicate v1/v2 identity correction. Confirmed credit still requires a current confirmed accepted result, a completed single producing invocation, matching task and execution identity, and the deterministic observation ID. Knowledge uses the same confirmation/current-source checks and does not require historical authors to remain current members.

Both public admission wrappers still enter the same immediate SQLite transaction. Current membership and eligibility are checked before grants; production cannot consume the reserved independent reviewer. Allocation commitment rechecks already admitted producer assignments under the same write serialization. The combined optional-learning branch uses current eligible-agent selection and persists free text only as an unconfirmed proposed candidate even after peer agreement. Accepted runtime resource ceilings, immutable captures, review eligibility and budget/grant checks remain intact.

## Commands, adverse controls and restoration

In the reviewed checkout, independently run commands all exit **0**:

| Command | Evidence |
| --- | --- |
| `cargo fmt --all --check` | Exact reviewed checkout; no formatting changes |
| `cargo clippy --workspace --all-targets -- -D warnings` | All targets, warnings denied |
| `cargo test --workspace` | 267 tests pass; `/tmp/ymp-state-independent-workspace.log` |
| `cargo test -p ymp-runtime --test state_integration -- --nocapture` | Joint public consumer passes; `/tmp/ymp-state-independent-joint.log` |
| `git diff --check` | Review report has no whitespace errors |

The required workspace checks were independently repeated once because the inherited report/logs did not bind their runs to an exact source fingerprint. The existing individual external probe results were read as prior evidence, not claimed as newly replayed here. The joint consumer and complete workspace suite provide the independently observed combined results.

A `git archive HEAD` development copy was made at `/tmp/ymp-state-independent-mv3obj2b`. Only this disposable copy was mutated. The command used for each adverse control and the restored run was:

```sh
cargo test --offline -p ymp-runtime --test state_integration --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/integrate_state/target -- --nocapture
```

| Disposable source state | Exit | Observed result |
| --- | --- | --- |
| Remove only `team.current_members.contains(&assignment.agent_id)` from shared admission, retaining eligibility | 101 | `Retired or reserved producer acquired a grant`; `/tmp/ymp-state-independent-membership-mutant.log` |
| Restore membership; substitute only pre-correction `e1a1519` engine source | 101 | Public follow-up fails with `Stored workspace belongs to a different working directory`; `/tmp/ymp-state-independent-location-mutant.log` |
| Restore every source byte to reviewed HEAD | 0 | Joint test passes; `/tmp/ymp-state-independent-restored.log` |
| Exact production source, add only the independent public `answer.workspace` assertion | 101 | Actual F1: returned directory is the empty relocated association; `/tmp/ymp-state-independent-result-workspace.log` |

All temporary source/test changes were restored. A byte comparison of every tracked file in the disposable copy against reviewed HEAD found no differences. `/tmp/ymp-state-independent-controls.json` records mutation commands, statuses and restoration. The reviewed checkout remains at the requested HEAD and its only addition is this unique report; no commit was created.

## Limits and acceptance boundary

F1 is a backend result/consumer contract correction. UI implementation and acceptance, broader routing, YMP-115 concurrency, and the known MCP search payload bound assigned to YMP-123 remain separate work. These checks establish deterministic transitions and evidence preservation, not model quality, team superiority, memory savings or scheduler throughput. Direct selected-directory execution remains the owner-approved 0.4.0 MVP limitation; the disposable review copy is not application behavior and implies no isolation or rollback guarantee. No real inference, credential reads, intent changes, task-registry edits or UI changes occurred.

Ledger: `R1(7/10) RETURN 13/09 00:19 HKT — combined authority and original-path summary pass; public returned workspace still names empty relocated registration and existing TUI labels it as artifact location → correct backend directory field and retain zero-inference consumer coverage`.

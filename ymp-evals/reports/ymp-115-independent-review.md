# YMP-115 independent review

13/09 00:19 HKT — **RETURN R1(5/10)**. A completed sibling can lose its available independent review after two other concurrent tasks fail. Code 5/10; evidence 8/10; public Engine experience 5/10. The minimum score reflects this major defect in the contracted sibling-failure outcome, despite the passing existing checks.

Candidate: `32624efd0b666e442068601c0828aeb174735542`, compared with parent `2e0c8ae`. All 30 changed files were inspected, including the allocation heuristic change, `occupied_agent_ids`, access DTOs, storage admission/plan commitment, implementation notes and recorded negative controls. The sole TUI edit is a default task-access literal in a fixture; UI behavior was not reviewed or changed.

The governing sources were current-main `AGENTS.md`, approved `intent.md`, YMP-115/YMP-116/YMP-124 task definitions, `runtime-contract.md`, `workspace-policy.md`, team/allocation policy, and universal concurrency/restart scenarios. The candidate's concurrency and allocation implementation documents and the team skill were also read. Direct selected-directory execution is authorized only for the MVP. Production isolation and recoverable publication remain YMP-124; this review does not request them.

## F1 — Major: retained failed actors prevent an otherwise feasible sibling review

Affected code: `ymp-rust/crates/ymp-runtime/src/allocation.rs:88` through the membership check at line 110, together with the intentionally retained running task actors in `ymp-rust/crates/ymp-runtime/src/engine/allocation.rs:438`.

The default heuristic computes an ordinary review target as `max(2, occupied_agent_ids.len())`. It then retains those occupied actors and appends the selected reviewer. If two failed task selections still own unresolved responsibilities and the selected reviewer is a third identity, the resulting three members exceed the heuristic's target of two. The actual `max_members` ceiling is four. The policy returns `active_responsibility` before review admission even though no configured ceiling or budget prevents the review.

A public `Engine::run` probe uses three independent scripted producers under ordinary default allocation (`parallel=3`, `max_members=4`, `turns=80`). A produces a file with an exact-byte acceptance contract. B and C write separate files and fail. Their per-purpose review settings have no supported configuration, while the reserved fourth agent has a valid review configuration. All three producers reach causal execution barriers; B and C fail before A is released. This makes the viable independent reviewer a distinct identity without changing the default allocation policy.

Actual response and state on the unchanged candidate:

```text
Outcome: blocked: Run blocked: active_responsibility: selected work cannot displace occupied members within the captured ceiling
Tasks: [("A", Review, Some("one")), ("B", Running, Some("two")), ("C", Running, Some("three"))]
admitted_invocations: 5
in_flight_invocations: 0
protected_review_invocations: 4
observed input tokens: Some(14), partial_calls: 2
last_denial: None
assertion failed: Completed sibling lost independent review although reviewer is eligible, max_members=4, and budget remains
  left: Review
 right: Accepted
```

Command: `cargo test -p ymp-runtime --test concurrency independent_probe_two_failed_siblings_keep_eligible_review_reachable -- --nocapture` — **exit 101**, reproduced twice. This is a defect in YMP-115's explicit requirement to process completed work independently of unrelated failure. It does not erase A's bytes, but it withholds a feasible independent acceptance and replaces the actual sibling failure diagnosis with a false ceiling diagnosis.

Required correction: make the membership proposal account for an additional selected reviewer while preserving occupied actors, pins, actual ceilings, final-review eligibility and existing admission validation. Do not remove unresolved task actors merely to make the count fit. Prove that A reaches confirmed acceptance in the same invocation under remaining budget, while the run honestly retains the B/C failures and closes their grants/reservations.

A disposable diagnostic correction changed the final lower bound to include the selected executor only when it is not already occupied:

```rust
.max(input.occupied_agent_ids.len()
    + usize::from(executor.as_ref().is_some_and(|choice|
        !input.occupied_agent_ids.contains(&choice.agent_id))));
```

The identical public probe then exited **0**, A was accepted, and the run's reason became the actual `Scripted sibling infrastructure failure`. This is localization evidence, not an applied or fully reviewed production fix. The candidate source remains unchanged.

## Reproduction

Disposable checkout and captured logs remain at:

`/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp115-independent-9omb88fn`

Start from `git archive 32624efd0b666e442068601c0828aeb174735542`. In its `ymp-rust/crates/ymp-runtime/tests/concurrency.rs`, let `Mode::Failure` use A/B/C by changing the two-name match arm to `Mode::Reads => &["A", "B"]`; change the failure predicate to `matches!(self.mode, Mode::Failure) && label != "A"`. Append this test (the existing fixture already supplies independent file scopes and A's exact-byte contract):

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_probe_two_failed_siblings_keep_eligible_review_reachable() {
    let mut f = fixture(Mode::Failure);
    f.engine.config.team_constraints = TeamConstraints::default();
    f.engine.config.team = vec!["one".into(), "two".into()];
    f.engine.config.capabilities.insert("offline".into(), ProviderCapabilities {
        models_complete: true,
        models: vec![ModelCapabilities {
            id: "available".into(), controls: None,
        }],
        default_model: Some("available".into()),
        ..Default::default()
    });
    f.engine.set_assignment_settings(["two", "three"].into_iter().map(|id|
        AssignmentSettingsRule {
            agent_id: id.into(), purpose: Some("review".into()), task_id: None,
            settings: ModelEffort {
                model: Some("unavailable".into()), effort: None,
            },
        }
    ).collect()).unwrap();
    let run = start(&f);
    for _ in 0..3 { next(&mut f).await; }
    release(&f, "B");
    release(&f, "C");
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let session = f.store.sessions(None).unwrap().remove(0);
            let trace = f.store.trace(&session.id).unwrap();
            if trace.invocations.iter()
                .filter(|i| i.state == InvocationState::Failed).count() == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    release(&f, "A");
    let outcome = finish(run).await;
    let trace = f.store.trace(&outcome.session.id).unwrap();
    assert_eq!(trace.tasks.iter().find(|t| t.title == "A").unwrap().state,
        TaskState::Accepted);
    assert_closed(&f, &outcome);
}
```

Logs: `review-two-failures-original.log`, `review-two-failures.log`, `review-two-failures-correction.log`, `review-reader-positive.log`, `review-reader-falsifier.log`, `review-native-execute-authority.log`; exact independent command exits are in `review-results.json`. The disposable allocation and resource-exclusion mutations were restored after their controls.

## Verification and scope findings

| Command/check | Actual result | What it distinguishes |
| --- | --- | --- |
| `cargo fmt --all --check` | Exit 0 | Required formatting check on candidate |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 | Required workspace lint check on candidate |
| `cargo test --workspace` | Exit 0; 267 tests | Existing public/runtime/storage/provider tests, including all nine concurrency tests |
| `npm ci --ignore-scripts` in disposable Claude bridge | Exit 0 | Installed pinned test dependencies; no inference |
| `npm run check` in disposable Claude bridge | Exit 0 | Bridge type check |
| `npm test` in disposable Claude bridge | Exit 0; 13 tests | Real SDK transport into a Python fake executable, including read tool restriction on resume |
| `cargo test -p ymp-providers --test authority -- --nocapture`, disposable request purpose changed from `plan` to `execute` | Exit 0; 4 tests | Codex read-only sandbox request and ACP mode/reset limitations on execution-purpose transport; all provider processes are scripted |
| `cargo test -p ymp-runtime --test concurrency independent_probe_scoped_conflicting_reader_waits -- --nocapture` | Exit 0 | Scoped B reads `outputs/shared.txt` instead of independent input; no reader/writer may enter while another conflicting assignment barrier is held |
| Same conflicting-reader command with coordinator resource exclusion disabled | Exit 101: `Conflicting reader/writer overlapped` | The positive check detects removal of the real scheduler exclusion guard |
| Completed-sibling probe above, original candidate | Exit 101 twice | Exposes F1 through public Engine admission and review |
| Same completed-sibling probe with disposable sizing correction | Exit 0 | Isolates F1 to missing room for the selected reviewer |
| `git diff --check` before report creation | Exit 0 | Candidate source remained unmodified |

The required Cargo commands ran as one `&&` chain with final exit 0, establishing exit 0 for each constituent command. Native SDK fixture values such as `max` are data only. Every workload was mock/scripted; no model inference or credential inspection occurred.

The following candidate claims are supported by source and exercised tests:

- `Task`, `PlanTask` and `TaskDefinition` bind explicit access. Omitted legacy values conservatively remain write, including digest-compatible serialization. Analysis/synthesis are not categorically read-only. A file-producing analysis receives write authority and an exact-byte confirmation in the sibling fixture.
- Ordinary default-policy production overlap is reached through public `Engine::run`, independent task barriers, and built-in offline native execution. This is useful task execution, not planning/bidding traffic. Requested read-only assignment modes and actual `TurnRequest.read_only` values are asserted; the mock must leave no output files.
- The compiled execution backend supplies effective scope. Policy proposals cannot narrow it. Unknown/custom backend access defaults to whole-directory writing; ACP remains a whole-directory writer even for a requested read-only mode. Scoped fixture I/O is bounded by the same literal paths it reports, with no general shell/file tool route. Native transports establish their existing permission request/tool limits, not scoped filesystem isolation.
- Atomic process coordination holds agent occupancy, concurrency capacity and filesystem access before budget/grant admission. Conflicting resource waits have stable codes and holder identities. Production keeps its lease through candidate snapshotting; verification owns the whole directory through checks and acceptance.
- The shared public storage admission path rejects enlarged read-only task authority, and reviewed plan commitment rejects changed access. Runtime and transactional storage membership checks retain committed/occupied actors. The F1 correction must preserve these guards.
- The original one-failed-sibling fixture retains confirmed A, records partial usage, revokes grants, balances lease records, and inspects uncertain work on resume without replay. Cancellation before admission creates no invocation for waiting selections; resume executes their still-needed work. These passing cases do not cover the distinct two-failure/member-sizing combination in F1.

The eight author negative-control records were checked against their source/test mutations: scheduling width, resource exclusion, sibling review, false policy scope, read-only admission, changed reviewed authority, restart resource release, and mock read-only writes. Their retained logs show exit 101. They were not all independently rerun; independent controls above add a conflicting scoped reader and expose the uncovered default-policy failure. The universal scenarios were read as contracts, not treated as executed application evidence.

Minor documentation residue: `ymp-docs/architecture/allocation-implementation.md` still identifies `ymp.bounded-allocation` as version 1, while this candidate and `concurrency-implementation.md` identify version 2. Reconcile the implementation note when correcting F1. This does not independently force the return.

No real-provider quality, native narrow-path sandbox, multi-process nested-root containment, production publication, recovery of external side effects, or UI experience claim is established. A direct MVP workspace cannot promise rollback. Only this English report was written in the candidate; intent, task registry and source were left unchanged.

Round ledger: 13/09 00:19 HKT — `R1(5/10) RETURN`: two failed occupied actors plus a distinct viable reviewer exceed the heuristic target despite a larger actual ceiling; public probe fails, disposable sizing correction passes; production correction remains required.


## R2 — F1 closed

13/09 00:35 HKT — **ACCEPT R2(9/10)** for corrected candidate `ecf12787043861964dc18ef47d9d397cf94c9c41`. Code 9/10; evidence 9/10; public Engine experience 9/10. No contracted defect remains open in this bounded correction. The R1 report above is preserved verbatim; its SHA-256 before this appendix was `13ca6f34411f1a8facd8933e772addf03f99b14583f2c13b471650bb7f30af44`.

The complete correction from `32624efd0b666e442068601c0828aeb174735542` was inspected: the twelve-line allocation change, separate `TwoFailures` fixture and regression assertions, both implementation-note updates, correction report and evidence JSON. Runtime/storage admission, occupied-actor derivation, final-review validation, native adapters, workspace coordination, grants and UI code are unchanged. The earlier allocation-policy version mismatch is corrected to version 2.

`required_members` now counts retained occupied identities plus the selected identity only when distinct. Actual member and eligible-pool ceilings still cap the target; fixed roster/size handling and downstream runtime/storage validation still apply. This supplies the third member needed to review A without discarding B/C's unresolved responsibilities or enlarging a captured constraint.

The retained public consumer observes all three production barriers, completes B/C failures before releasing A, and then establishes:

- A is accepted with exact-byte confirmation and its requested file retained in the same run.
- Both sibling invocations remain failed, their task responsibilities remain unresolved, and the run remains blocked for the actual scripted failure.
- Both occupied actors remain in the committed review membership; the distinct fourth identity receives the independent review assignment using the permitted inherited model setting.
- The recorded 14 input tokens and two partial calls remain; review uses available budget, with no budget denial.
- All invocations and grants close, and workspace acquisition/release counts balance.
- The same captured input rejects an actual `max_members=2` or `fixed_size=2`; selecting an already occupied reviewer does not add a member.

The original one-failure/restart fixture retains its two tasks and was exercised again. Ordinary default-policy read-only overlap, scoped and whole-directory exclusion, unsupported scope rejection, public read-only authority protection, cancellation before admission and inspection without uncertain replay also remain covered by the ten-test consumer suite.

### Independent R2 commands and failing control

| Command/source | Actual exit and result |
| --- | --- |
| `cargo test -p ymp-runtime --test concurrency -- --nocapture` on exact corrected candidate | **0**, all 10 tests passed |
| `cargo test -p ymp-runtime --test concurrency independent_probe_two_failed_siblings_keep_eligible_review_reachable -- --nocapture` in disposable exact candidate with only original allocation source restored | **101**, A remained `Review` instead of `Accepted` |
| Identical command and test bytes after restoring corrected allocation source | **0**, A accepted and the original sibling failure retained |
| Working-tree comparison against exact HEAD for `Cargo.toml`, `Cargo.lock`, `ymp-rust`, and `ymp-bridges` | **0** for each; no source changes during review |

The same regression bytes were used for both independent failing and passing runs: SHA-256 `b97139dae556ac86c0833e0bd37a047e599d80dcb074ec83a29b089ea9ff11d1`. The original allocation source hash was `4ca035ea313b1e019c5662070ae1d44c8b36ee870d5d5af8f636876d7cee2883`; the corrected hash was `ccf094b921c8aff197c565845590a01299e307d8058de607ad6195fd56c00848`. These independently match the final author control and current candidate files. The earlier compiler/assertion construction failures are labeled accurately in the author's evidence and are not relied on as behavioral falsifiers.

Independent logs and `results.json` are in `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp115-r2-independent-t28qk8h4`. Its corrected source was restored after the control; candidate source was never mutated.

The exact-source required-check evidence in `ymp-docs/research/evidence/ymp115-r1-correction-checks.json` records `cargo fmt --all --check` **0**, `cargo clippy --workspace --all-targets -- -D warnings` **0**, and `cargo test --workspace` **0**, with 268 tests. Source/test hashes and the bounded diff were checked; these unchanged full checks were reused rather than rerun in R2. The unchanged bridge likewise retains R1's independent type check and 13 passing scripted SDK transport tests, plus its prior offline execution-purpose authority checks. No bridge or provider inference check was repeated or needed for this membership-only correction.

This acceptance applies to the stated isolated candidate. It does not claim verification of the later combined integration with main `5834e7f`. The direct-directory MVP limitations and deferred YMP-124 production work remain exactly as in R1. All R2 workloads were offline mock/scripted; no credentials, source, main, intent, task registry or UI were edited, and no commit was made. Only this appendix was written.

Round ledger: 13/09 00:35 HKT — `R2(9/10) ACCEPT`: F1 answered by counting a distinct selected reviewer alongside retained actors; the identical public test fails under old sizing and passes corrected; actual ceilings, accounting, grants and resource scope remain intact.

# YMP-115 combined integration independent review

13/09 00:56 HKT, 2026-09-13 (2026-09-12T16:56:08Z) — **ACCEPT R1(9/10)** for exact HEAD `b08eb57f32018b7292bec7a958cc3c39c2b2bac6`. Code **9/10**; evidence **9/10**; public Engine consumer **9/10**. No contracted integration defect or return condition was found.

This review accepts the composition of standalone YMP-115 R2 with the accepted YMP-110/YMP-113 state and location outcome. It does not accept the UI, public MCP integration or the complete release. Governing `AGENTS.md`, the combined report/evidence ledger, standalone YMP-115 R1/R2 report and the relevant runtime/storage consumers were read. The inherited direct selected-directory policy remains limited to the approved MVP.

## Integration and source independence

The base is `8ed01ac27e94a38cf88aa3f78e58d23bb6e387ba`; accepted YMP-115 commits `32624efd0b666e442068601c0828aeb174735542` and `ecf12787043861964dc18ef47d9d397cf94c9c41` map to `c4cf0cd8d57ff15613575238cf9554ff54a5e705` and `7e9a02661dd3116f61d28ab10604ceb0e06e8437`. The final integration commit changes consumers and evidence, adding no production behavior. Module declarations retain both knowledge and workspace-access implementation.

Independent byte comparisons show that runtime allocation, runtime occupied-actor validation, storage allocation and admission, both workspace-access implementation modules, core access DTOs and provider access declaration match accepted standalone R2. Runtime/storage knowledge and complete storage confirmation implementations match accepted main. The effective-version wrapper, entire corrected location branch and complete incremental knowledge/observation block are byte-identical to that main base. There is no duplicate execution-identity correction or alternate knowledge acceptance path.

The only inherited TUI diff is `TaskAccess::default()` in the existing task fixture. No UI implementation was changed or independently reviewed here.

## Independently observed public outcomes

One focused command ran all **24 tests** successfully: the ten concurrency consumers, thirteen allocation-admission controls and the combined membership/knowledge consumer. Its exact command and log are recorded below.

The extended two-failure consumer reaches all three production barriers under the ordinary bounded policy. B/C fail before A is released. Their unresolved task actors remain occupied, while a distinct eligible reviewer confirms A by exact bytes. The blocked outcome reports the actual sibling failure, retains 14 input tokens and two partial calls, and does not invent unsupported credit. Reviewer settings remain applicable; actual `max_members=2` and `fixed_size=2` controls still reject the impossible composition. Selecting an already retained reviewer does not inflate membership.

That same run retains precisely one supported observation and an outcome-derived knowledge entry tied to the actual acceptance, result/version and confirmation IDs. No final learning assignment runs. Storage reopening and a newly constructed Engine deliver the exact retained entry/version and `Contribution A` excerpt into another session's actual backend prompt. This establishes public delivery in the combined runtime, beyond checking inventory membership.

Metadata-only relocation leaves the complete original outcome unchanged. The public `Where did you save it?` follow-up returns the original directory and absolute A artifact path, with unchanged tasks, assignments, invocations, decisions, membership, budget and observations after excluding conversation-history additions. Backend requests do not increase, original bytes remain intact and the relocated directory stays empty.

The earlier membership consumer now correctly refuses to retire a producer that still owns an interrupted second task. Its denial preserves membership and spend. It then invokes the actual Engine recovery path, verifies that the two original production assignments are not replayed, and only retires the actor after inspection resolves the interrupted task. Original knowledge, confirmed credit, current/retired admission protections and all recorded-location fallbacks still pass afterward. This is a valid adjustment to the stronger responsibility contract; it does not clear a task by fabricating completion records.

## Lifetime, authority and evidence assessment

Resource acquisition records are produced before budget/grant admission and retain agent occupancy, invocation capacity and filesystem exclusion. The access proposal must cover the compiled backend's declared access; planner text cannot establish a narrow guarantee. Unknown backend access remains whole-directory writing. The public negative-policy consumer rejects a false narrowed guarantee before spending calls or invoking the backend. Ordinary read-only task execution overlaps, while scoped conflicts and whole-directory writers/readers serialize in the passing consumers.

For production, `RecordedResponse` retains its access lease through `candidate_result` and the submitted-result transaction. This keeps the artifact snapshot inside the producing reservation even after the provider invocation has ended. Verification acquires an exclusive directory lease and keeps it through source resolution, trusted checks, independent review, acceptance, incremental retention and supported observation. The nested reviewer request uses the parent's reservation relationship; it does not discard the outer protection. Error/cancellation paths drop runtime leases and close invocation grants. The public traces show no remaining running invocation, zero in-flight budget, revoked grants and balanced acquisition/release counts. Source inspection supports the per-reservation lifetime; the permanent trace helper checks aggregate balance rather than claiming a separate per-ID ordering audit.

Unresolved task responsibility is deliberately longer-lived than a failed invocation or workspace lease. Both runtime validation and the immediate storage allocation transaction retain Running task actors. The accepted sizing correction reserves room for a distinct selected reviewer without changing fixed-size, roster or actual membership ceilings. Existing public reservation/admission interleavings remain green, including retired/reserved denial and ordinary current-agent admission.

Task access is bound into `PlanTask`, committed `Task` and captured `TaskDefinition`. The storage plan transaction compares it with the reviewed plan, and public execution admission rejects enlargement of a read-only task's authority. Omitted historical access still means write and is omitted when serializing that default, preserving historical definition digests. The passing consumers cover both the legacy encoding and changed explicit-access digest. Knowledge freshness continues to use the same current task definition and original captured files, so the additional access field participates in source identity without inventing new confirmation.

## Commands and verified evidence

Independently run on the exact reviewed HEAD:

```sh
CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test -p ymp-runtime --test concurrency --test state_integration --test allocation_admission -- --nocapture
```

Exit **0**: **24 tests passed**. Log: `/tmp/ymp115-combined-independent-focused.log`; SHA-256 `8657592f68a9441a438d8eb2f1ff3837f00ba97cdbfffb736f096cc41c8cf8b4`. The reduced-debug/incremental settings affect build artifacts, not the assertions or modeled provider behavior. `git diff --check` also exits **0**.

Every one of the ledger's **106 source-manifest files** was independently hashed and matches the exact checkout. Recomputing the aggregate manifest hash yields `2e1f0feaf44fbbc5dead292a270a362c52c79de30a9c15407b80a1e212831c46`, matching the final verification records. All five referenced check-log hashes were also recomputed successfully.

| Exact-source author check, independently inspected | Exit / observed evidence |
| --- | --- |
| Focused 24-test command | 0; `/tmp/ymp115-combined-final-0.log` |
| `cargo fmt --all --check` | 0; `/tmp/ymp115-combined-final-1.log` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0; `/tmp/ymp115-combined-final-2.log` |
| Initial `cargo test --workspace` | 101; `/tmp/ymp115-combined-final-3.log` contains compiler/linker ENOSPC failures before a product test verdict |
| `CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test --workspace` after local cache cleanup | 0; **278 tests passed**, `/tmp/ymp115-combined-final-workspace-retry.log` |

The successful complete log has SHA-256 `cb23416fcfe7891c362437509edf436ebf7520ef455e109fd18cdff9b6de63c0`. Its passing summaries independently total 278. These formatting/lint/full-suite results are author-run evidence bound to independently verified exact source, not a claimed new reviewer full-suite run. A redundant complete run was unnecessary after the focused integration consumers passed.

All three retained deliberate controls were inspected and their hashes verified. Against identical combined consumer bytes, old review sizing leaves A in Review (101), disabled incremental retention leaves no supported outcome entry (101), and returning mutable project registration fails the original-directory assertion (101). Current source matches each recorded restored hash. The standalone R2 report additionally contains its independently observed old-sizing failure and restored success. These controls were not reintroduced in this read-only review; this round independently reran the actual corrected public consumers. Construction errors and compiler ENOSPC were not treated as product-defect controls.

## Limits and handoff

No real inference, credentials, source mutations, main edits, intent changes, task-registry changes or commit occurred. The only review addition is this report. The separate backend setup/cancellation anomaly has another assigned reviewer and was not rediagnosed or declared resolved here.

The reviewed result establishes coordinated mock/scripted execution, preserved responsibilities, source-bound knowledge and deterministic historical locations. It does not establish model quality, resource savings, native narrow-path isolation, arbitrary multi-process containment, external-side-effect recovery, publication or rollback. Those limitations do not reopen the owner-approved MVP scope or the separately tracked YMP-124 work.

Ledger: `R1(9/10) ACCEPT 13/09 00:56 HKT — accepted concurrency and state implementations retain their authority and evidence behavior in the same public runs; focused 24 pass and exact-source 278-test evidence validates; no integration blocker found`.

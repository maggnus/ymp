# Captured unknown-usage policy and actual restart review

13/09 06:29 HKT, 2026-09-13 (2026-09-12T22:29:19Z) — **ACCEPT, 9/10** for policy correction `d31d49f1e28e498fa85bb3d0e6a4287cbad8c0d2` and the bounded restart-inspection path in adapter checkpoint `32e7c5fe0fb8304d17e981132ce89c9e8ec74752`. No blocker found in this scope. This does not accept all seventeen driver cases, other adapter changes or the final release.

Review used immutable snapshot `/tmp/ymp126-unknown-review-mehhjlfz/worktree` at `32e7c5f`. The production policy diff, restart adapter, relevant runtime/storage consumers and original default-stop evidence were read. No source or authoritative expected fixture was changed.

## Policy remains explicit and bounded by reported accounting

`UnknownUsagePolicy::Stop` is both the Rust default and missing-field deserialization behavior. `BoundedNative` is an explicit captured choice. The production change conditions only the existing incomplete-accounting stop; it does not remove token limits or replace resources with an unlimited configuration.

Admission still checks reported spend plus live/requested reservations, protected review allowance, checked arithmetic, invocation count, concurrency, timeout and context limits. Native turn/output controls retain their existing runtime/backend enforcement. Historical reported spend and partial/unknown coverage are not erased. `strict_token_bound` remains false, and `require_strict_token_bound` rejects incomplete accounting. This policy permits continued admission against reported spend; it does not establish a hard bound on unreported native expenditure.

The targeted storage tests independently pass: missing-field Stop; Stop denial for absent/partial usage without an added invocation; explicit bounded continuation retaining the reported counters; captured policy preservation across reopening and a later legacy-limit call; continued context/invocation denial; strict-bound refusal; and a two-connection race where reported 75 plus one 20-unit reservation leaves the second caller denied at the captured 100 ceiling.

The original retained default-stop run was inspected at `/tmp/ymp126-restart-stop-second-20260912/restart-inspection`. Its policy stays identical before/after, reported usage remains 47, the limit remains 100, admission records `unknown_usage`, and inspection invocation count is zero. That is the actual previous policy behavior, not a shortened or altered comparison workload.

## Fresh real-process restart independently reproduced

Built both requested packages and ran:

```sh
cargo build --offline --locked -p ymp-eval-driver -p ymp-cli
cargo test --offline --locked -p ymp-storage unknown_usage -- --nocapture
./target/debug/ymp-eval-driver --output /tmp/ymp126-unknown-review-mehhjlfz/run --case restart-inspection
```

All exit **0**. The storage filter runs three matching tests. The driver runs the requested single case in a fresh non-Git directory; its top-level all-case completion remains false. Builds use debug information disabled, incremental compilation disabled and two build jobs. No full workspace suite was duplicated.

Independent comparison of raw records establishes:

| Observation | Actual result |
| --- | --- |
| Process transition | Seed PID 41068 exited by SIGKILL; new inspection PID 41112 exited 0 |
| Captured policy | Byte-equivalent JSON before/after; bounded_native, reported ceiling 100, invocation allowance 20 and review allowance 20 retained |
| Reported spend | 47 before crash/recovery, 50 afterward; one partial call remains |
| New native work | Exactly one new completed review invocation, reporting three units; no new production invocation |
| Inspection authority | Read-only requested mode; actual admitted Scoped reads name both artifacts and writes is empty |
| Inspection evidence | Its native ID matches the new runtime invocation; both recorded byte arrays/digests match the actual files |
| Preserved work | Original confirmed document acceptance record is identical; document digest and the incomplete totals bytes are unchanged |
| Grants and accounting | All six durable grants revoked; no in-flight reservation remains; strict token bound false |
| Result | Nine projected events pass the unchanged protocol validator; reported remainder 50, partial coverage, no restored old role and zero production replay |

The recovered application session keeps totals unresolved; passing this inspection scenario does not declare those totals complete. The remaining reported amount also does not claim complete native-cost coverage or unrestricted available production allowance; review protection remains represented in the actual budget.

## No fabricated inspection or persisted capability

The inspector is admitted by the real Engine recovery path. Its scripted backend enforces review/read-only mode, reads both actual files, records their bytes/digests and returns a rejecting review for unresolved totals. The custom inspection marker is therefore not accepted as sole proof: this review checked it against the completed runtime invocation, actual access record, file contents and three-unit runtime usage increment. The projection derives preserved results and unresolved tasks from actual records. It writes no task acceptance to manufacture success.

The old private capability travels from the seed to its parent through a private stdout pipe, then to the new inspector through a private stdin pipe. The parent does not write that payload to a file. Native journals select prompt/settings/IDs and omit the MCP endpoint; retained JSON contains no token-bearing fields. The real new TeamServer rejects the supplied old capability. Source inspection and the saved response support that control; no replacement dummy token was used in the fresh run.

Expected protocol fixtures and validators are unchanged between the preceding checkpoint and this snapshot. Other new adapters in `32e7c5f` were not accepted by this review.

## Retained evidence and handoff

Everything is under `/tmp/ymp126-unknown-review-mehhjlfz/`: immutable worktree, `build.log`, `unknown-usage.log`, `restart.log`, exact command results, full `run/restart-inspection` bundle, `independent-audit.json` and `review-evidence.json`. The latter binds 141 source files and the captured result/log hashes. The actual driver executable SHA-256 is `a56d4ec9209eb9121cc375bdd42d2ccb7c7f181dc6e65dd9336fe32a31f4a35e`; the fresh restart command completed in 3.279 seconds.

All source files still match the immutable commit. No main, sibling writer, UI, intent, registry, real application home or credential was edited or accessed. All execution was scripted/mock with captured low effort; no installed-provider inference occurred. Only external review evidence was written and no commit was made. Integrating this opt-in policy and completing independent all-case driver verification remain parent work.

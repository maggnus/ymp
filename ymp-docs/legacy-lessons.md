# Legacy lessons

Status: reference, harvested 2026-09-16 by owner decision from the previous
iteration's code. This document replaces reading that code. `AGENTS.md` sets the
rule: the code stays at `legacy-*` tags, the facts live here, and nothing is copied.

Each entry states a fact, its source (`<tag>:<path>` or a commit), the task that
owns the decision, and a falsifier: what would make the entry obsolete. An entry
without a falsifier that still holds is a fact; an entry whose falsifier fires is
deleted, not kept "for history".

## 1. Source inventory

| Attempt | Where | What it is |
|---|---|---|
| 1 | tag `legacy-attempt-1` (commit 7cdb238, 2026-08-11 to 2026-09-06) | Nineteen crates, 134k lines, about 850 tests, documentation in Russian: hash-chained JSONL journal with content-addressed objects, isolated candidate copies with deterministic submissions, a verifier with mandatory negative controls, a commitment ledger with leases and fencing generations, an inert board, a four-level registry, Codex and Claude Code driven as CLI processes. Two authorized live runs, both stopped before a model answered. Archived by owner direction on 2026-09-06; harvested in section 11. |
| 2 | tag `legacy-attempt-2` (commit aa6d722, 2026-09-12 to 2026-09-15) | ymp 0.4.0 to 0.4.6: seven crates, 96k lines, 666 passing tests, a Node bridge to the Claude Agent SDK, ACP support for GLM, a SQLite store, a Ratatui interface, an evaluation harness, and the only real-model team runs of the project. Its `architecture/v3` proposal is the ancestor of the approved model of this iteration; its migration plan and an external assessment (findings F1 to F8) led to the greenfield restart. Harvested in section 10. |
| 3 | `~/Code/ymp-damaged-20260915` (not tagged) | One commit on 2026-09-15 with documents and an empty task register, no code. Nothing to harvest. |
| 4 ("foundation") | tag `legacy-foundation` (commit f95d8df, 2026-09-15 to 2026-09-16) | Five crates, 22,488 lines: in-memory and SQLite journal, single-task dispatcher, Codex App Server adapter, sandboxed command checks, `ymp run` / `ymp show` CLI. Removed from the working tree in 95b7e6f. The uncommitted working tree of `~/Code/ymp4` is an earlier draft of commit aa9c26e and adds nothing. Harvested in sections 2 to 8. |

Historical design documents of attempt 4 stay in the tree, marked historical:
`ymp-docs/architecture.md`, `ymp-docs/bounded-execution-contract.md`,
`ymp-docs/durable-journal-contract.md`. They describe the old vocabulary and are
not a specification for the new code.

What attempt 4 did not have, so it cannot inform: a team, a Treasury, an Arbiter,
several execution profiles, contribution kinds beyond a single task run, a
self-organizing loop. Everything below is mechanics under that loop, not the loop.

## 2. Codex App Server facts (owner: W1-0018)

Source unless stated otherwise: `legacy-foundation:crates/ymp-runtime/src/codex.rs`,
`.../codex/rpc.rs`, `.../codex/settings.rs`, `.../codex/usage.rs`, and the fake
server `legacy-foundation:crates/ymp-runtime/tests/fixtures/codex_app_server.py`.
Falsifier for the whole section: a Codex release whose `app-server` protocol
differs; check against the installed binary before relying on any line here.

- **Transport.** `codex app-server --stdio`: JSON-RPC over newline-delimited
  JSON on stdin/stdout. One protocol line is bounded at 16 MiB; a longer line is a
  protocol failure, not data.
- **Handshake order.** `initialize` request, then the `initialized` notification,
  then `model/list`, then `thread/start` (or `thread/resume`), then `turn/start`.
  The handshake has a bound (15 s in attempt 4); a stalled `initialize` is a typed
  readiness failure.
- **Model list.** `model/list` is paginated (`cursor` / `nextCursor`, `data`,
  `includeHidden`). Attempt 4 rejected a repeated cursor, a repeated model ID, a
  missing ID, an empty list, and unbounded metadata. Each model advertises
  `supportedReasoningEfforts`; an effort not advertised for the chosen model is
  rejected before `turn/start`, and the acknowledged model and effort must equal the
  ones sent (drift is a failure, not a warning).
- **Thread parameters.** `cwd`, `model`, `approvalPolicy`, `sandbox`, `ephemeral`.
  Only an allowlisted set of CLI arguments (`--ephemeral`, `--sandbox`) maps onto
  them; arbitrary arguments are rejected before spawn. An ephemeral thread cannot
  be resumed.
- **Prompt separation.** The goal text travels only inside `turn/start`
  (`input`), never as a CLI argument, so a flag-shaped prompt cannot change process
  options (commit 14bc534).
- **Workspace access.** Exactly one workspace-access mode per run. The
  workspace-write sandbox was configured with `writable_roots=[]`,
  `exclude_slash_tmp=true`, `exclude_tmpdir_env_var=true`.
- **Notifications.** `item/started`, `item/updated`, `item/completed`,
  `item/agentMessage/delta`, `thread/tokenUsage/updated`, `turn/completed`.
  Output deltas and the completed item describe the same text: count once.
- **Server requests.** `item/commandExecution/requestApproval` and
  `item/fileChange/requestApproval` arrive while `turn/start` is still pending and
  must be answered without waiting for the turn response.
- **Terminal and non-terminal errors.** Error classes seen: `usageLimitExceeded`,
  `internalServerError`, `sandboxError`, `threadRollbackFailed`,
  `activeTurnNotSteerable`, plus `codexErrorInfo` with `will_retry`. Only a literal
  boolean `true` in `will_retry` makes the error non-terminal; repeated usage
  reports during retries must not double count. Only allowlisted structured codes
  are kept; free-form error text from the provider is never persisted (7c64bf1).
- **What is not success.** A completed turn without a final agent message; a
  process exit without `turn/completed`; a failed turn is a typed termination.
- **Usage.** Reports carry `total` and `last`. On resume the pre-existing total
  is a baseline to subtract; a missing or reset baseline makes the usage partial
  rather than the whole history; equal first total and last prove a zero baseline.
  Cumulative usage advances across observation attempts (58c90ac).
- **Process ownership.** The child runs in its own process group; cancellation
  and drop signal the group (SIGTERM, then SIGKILL after a grace period of 3 s in
  attempt 4). The supervisor observes exit without waiting for stdout EOF first;
  stderr is drained separately and never becomes adapter output. Observations are
  delivered one at a time and can be left unread without loss.
- **Idempotent start.** A start that was already journaled returns the first
  outcome, successful or unknown; it never spawns a second turn (54b3676, 1c26ca9).

## 3. Durable journal facts (owner: W1-0016)

Source: `legacy-foundation:crates/ymp-storage/src/lib.rs`, `.../payload.rs`,
`legacy-foundation:crates/ymp-storage/tests/*.rs`,
`ymp-docs/durable-journal-contract.md`. Falsifier: a storage decision in W1-0016
that is not SQLite; then only the atomicity and indeterminate-commit rules survive.

- **Connection settings.** WAL journal mode, `synchronous=FULL`,
  `foreign_keys=ON`, busy timeout 5 s.
- **Identity header.** `application_id` = `0x594D504A` (ASCII `YMPJ`),
  `user_version` = schema version (1). On open: an empty file (both zero, no
  objects) is initialised; a foreign application id is corruption; an unknown
  schema version fails as unsupported without mutation; a non-database file fails
  on open without mutation; `PRAGMA quick_check` runs before use.
- **Revision representation.** A u64 revision is stored as two integer columns
  (high and low halves) because SQLite integers are i64. Ordering is by the column
  pair, not by insertion order; the value crosses the 2^32 boundary correctly; a
  head at u64::MAX reads exactly; an append at u64::MAX fails with overflow before
  any write.
- **Append.** One `BEGIN IMMEDIATE` transaction per batch; a batch lands as
  contiguous revisions or not at all; an empty batch is rejected without touching
  storage; an injected pre-commit failure leaves no partial result. Limits in
  attempt 4: at most 256 events per batch, payload at most 2^20 bytes, oversized
  strings rejected before encoding.
- **Row integrity.** A CRC-32 (ISO HDLC, as in `crc32fast`) covers every
  protected value of a row, so a flipped payload byte, altered-but-valid JSON, or a
  wrong storage class is detected as corruption. Malformed JSON with a valid
  checksum is still corruption. A revision gap or a head that disagrees with the
  entries is corruption reported with the exact revisions. Orphan entries without a
  stream row are not an empty history.
- **Isolation.** Corruption in one stream leaves other streams readable.
- **Indeterminate commit.** When the process cannot learn whether a commit
  landed: if the commit is visible, do not resubmit; if absent, retry once at the
  same expected revision; if absent and the head advanced, the write is stale.
  The kernel must recognise a committed indeterminate settlement without duplicate
  accounting.
- **Concurrency.** Two OS processes on one database, and in-process threads on
  separate handles, serialize correctly under the busy timeout.
- **Payload codec.** Encoding is byte-stable and key-order independent on
  decode; nested duplicate fields, stray error classes, and inconsistent facts are
  rejected on decode (82b5f79, ea51785). The new event vocabulary comes from model
  section 9, not from the old payload forms.

## 4. Check execution facts (owner: W1-0009)

Source: `legacy-foundation:crates/ymp-runtime/src/check.rs`,
`legacy-foundation:crates/ymp-cli/tests/cli.rs`. Falsifier: an OS release that
changes `sandbox-exec` (macOS) or user namespaces (Linux); re-run the checklist
in section 7 on the target OS.

- **macOS.** `sandbox-exec` with a profile of the form `(version 1)
  (allow default) (deny file-write ...) (deny file-write-unlink ...)` that denies
  writes to the journal path while allowing workspace writes.
- **Linux.** `unshare` with new user, PID and mount namespaces; the mount tree
  made private and recursive; bind mounts of the workspace; the rest remounted
  read-only with `nosuid`, `nodev`, `noexec`; capabilities dropped from all five
  sets; `prctl` to keep them dropped. The test proved workspace writes allowed and
  journal plus host `/proc` protected.
- **Process discipline.** The check runs in its own process group and the group
  is killed on completion, so a finished shell leaves no background processes.
- **Timeout.** 120 s in attempt 4. A timed-out check stops without an invented
  exit code: "timed out" is its own outcome.
- **Sandbox start versus check failure.** A start marker separates "the sandbox
  failed to start" from "the check ran and failed"; the former is not evidence.
- **Verifier pinning.** A verifier file is digested (SHA-256) when declared; a
  changed verifier is rejected before execution.
- **Evidence.** Exact bytes of named files are captured with SHA-256, present or
  missing; captured output is bounded at 4 MiB; command checks inherit the
  environment and derive outcome from the exit code.

## 5. CLI and session facts (owner: W1-0015, W1-0017)

Source: `legacy-foundation:crates/ymp-cli/src/lib.rs`,
`legacy-foundation:crates/ymp-cli/tests/cli.rs`. Falsifier: the W1-0015 decision
on the command surface; the numbers below were defaults, never measurements.

- **Workspace lock.** One run per workspace across processes, via a lock file
  under the data directory named by the SHA-256 of the workspace path (2cb71c5).
  Concurrent runs restore an uncertain workspace hold across processes.
- **Allowance defaults.** 16 turns, 200,000 output characters, 30 minutes wall
  clock; cancellation starts 5 s before the deadline so the record shows a
  cancellation, not an overrun.
- **Interrupt.** SIGINT cancels the child, records the observed termination, and
  exits with code 130 (aa9c26e). An interrupt that does not terminate the child
  records "uncertain" at the shortened deadline.
- **Post-start writes.** Journal writes after the child started are retried;
  losing them would lose the hold (e6949fd).
- **Reading.** `show` reopens the database read-only; a missing journal creates
  no hidden state. The journal is outside the invocation's write set.
- **Arguments.** Non-Unicode arguments fail without a panic; help documents
  limits, storage location and lifetime.

## 6. Pitfalls found the hard way

Each entry is a bug attempt 4 shipped and then fixed. The new code avoids the bug
by design; the commit is where the reasoning lives.

| Commit | Lesson |
|---|---|
| 1c40982 | Start executes only the assignment that was journaled; never a reconstructed one. |
| 1c26ca9 | Journal the start attempt before spawning; a lost journal write after spawn must resolve idempotently. |
| 54b3676 | Duplicate starts return the first outcome, never a second provider turn. |
| c77b4df | Host-enforced limits accumulate across observation attempts; per-attempt counters let a run exceed them. |
| 58c90ac | Cumulative usage advances across attempts; the last report is not the total. |
| 6239005 | Observation accumulation for one invocation is serialized; concurrent observers must not interleave. |
| 9f3f244 | Effect evidence is recorded only from an uncertain state; recording it from a settled state duplicates accounting. |
| 82b5f79, ea51785 | Payload invariants are strict at decode; tolerant decoding hides corruption. |
| 7c64bf1 | Raw provider diagnostics are never persisted; keep classification, drop text. |
| e6949fd | Journal writes after the child started are retried with the same expected revision. |
| aa9c26e | Cancellation by SIGINT must leave an observable record before exit. |
| 2cb71c5 | Workspace coordination spans processes, not just threads. |
| 14bc534 | The prompt is transported separately from process options. |

## 7. Edge-case checklist

Legacy test names, grouped by the task that now owns the behaviour. They are
scenarios to reproduce against the new types, not tests to port.

- **W1-0016 (journal storage):** contiguous revisions per batch; empty batch
  rejected; pre-commit failure leaves nothing; two OS processes on one database;
  unknown schema version; foreign application id; malformed JSON with valid
  checksum; flipped byte; wrong storage class; revision gap; head disagreement;
  non-database file; other streams readable around corruption; orphan entries;
  indeterminate commit visible / absent / stale; 2^32 boundary; u64::MAX head and
  overflow; restart replay with real child processes.
- **W1-0001, W1-0006, W1-0017 (kernel journal and dispatcher):** revision gaps
  and invalid event order from an alternate journal; an event for another
  session; concurrent expected-revision admission has one winner; denied
  operations leave history unchanged; typed failures surface at the public
  boundary.
- **W1-0018 (Codex backend):** every scenario in section 2, plus: JSON-RPC error
  codes cannot carry arbitrary content; invalid JSON is a protocol failure; a
  server request is answered while the turn start waits; dropping the backend kills
  the process group; the real app-server test is manual only.
- **W1-0009 (checks):** every scenario in section 4, plus: exact-bytes capture of
  present and missing files; explicitly declared verifier is pinned.
- **W1-0015 (CLI):** every scenario in section 5, plus: no arguments and `--help`
  report commands and limits; version uses the application name and package
  version; invalid commands fail with a message, not a panic.

## 8. Not carried over

These constructs of attempt 4 have no place in the new model. They are also on
the denylist of `tools/legacy_scan.py`.

- `CriterionEvaluation` and the single-criterion `run`: the model evaluates
  acceptance through `AcceptanceAuthority` over `Criterion` sets with
  `Difficulty` (section 12.1).
- Blanket scope exclusivity per workspace: `WorkspaceGuard` grants scoped holds
  (R-4, R-5); the old "one run per workspace" lock is a CLI convenience at most.
- Shell-string checks: checks are typed `CheckSpec` values executed by a port.
- `EffectState`, `SessionCommand`, `SessionEvent`, `LifecycleEventKind`: the
  event vocabulary is model section 9.
- The whole session and invocation vocabulary: the unit of work is a
  `Contribution` under a `Commitment`.

## 9. Maintenance

- A fact that a task re-verified against the live tool is moved into that
  task's evidence and stays here with "confirmed by <task>".
- A fact whose falsifier fired is deleted in the same commit that records why.
- New harvests (attempts 1 to 3, if their code appears) add a row to section 1
  and entries with the same structure; they never add code.

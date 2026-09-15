# Legacy lessons

Status: reference, harvested 2026-09-16 by owner decision from three previous
iterations' code (tags `legacy-attempt-1`, `legacy-attempt-2`, `legacy-foundation`). This document replaces reading that code. `AGENTS.md` sets the
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

## 9. What to adapt with confidence (cross-attempt summary, 2026-09-16)

Three iterations were harvested (sections 2 to 8 for attempt 4, 10 for attempt 2,
11 for attempt 1). Ordered from most to least transferable:

1. **Provider wire facts: adopt as facts, verify against the installed binary.**
   Codex App Server (attempts 2 and 4 agree; sections 2 and 10.3), the Claude Agent
   SDK bridge and ACP for GLM (attempt 2 only; 10.3), Claude Code `--print`
   stream-json and Codex CLI JSON-lines (attempt 1 only; 11.7). Every line is covered
   by fixture tests that speak the real protocol. Owners: W1-0018, W6-0001, W6-0002,
   W1-0003. Falsifier: a provider release.
2. **Mechanics proven by tests in at least two attempts: adopt as requirements.**
   Durable before memory; idempotent start and settlement; atomic admission of
   assignment, grant, reservation and event; a grant bound to one invocation and
   revoked at its terminal state; process-group ownership with observed, never
   assumed, termination; unknown usage is not zero; a failed invocation carries more
   evidence than a successful one; recovery never replays effects without independent
   inspection; strategies propose and the kernel commits; accepted is not confirmed;
   failing applicable evidence overrides every approver; unconfirmed acceptance never
   credits. These are already invariants of the approved model; the attempts supply
   the edge-case scenarios (sections 7, 10.10, 11.10).
3. **Single-attempt mechanics worth adopting deliberately, one owner decision each.**
   From attempt 1: switchable-invariant counterexample tests; a separate allowance
   namespace for board traffic; an irreversible creation-authority dimension; the
   frozen launch snapshot as the comparability unit; the negative control first, its
   passing an infrastructure error; evidence unforgeable by type; a reserved terminal
   journal slot; typed child paths pinned by compile-fail doctests; the failed-
   invocation evidence record (11.11). From attempt 2: measurement validity separated
   from task outcome; blind identifiers independent of condition order; the three
   columns requested, sent, reported; a catalog snapshot with fingerprint and
   staleness; per-invocation grants over a local socket; memory idempotency by digest
   (10.2, 10.6, 10.7).
4. **Empirical results: adopt as priors, never as claims.** No controlled
   team-versus-solo comparison exists in any attempt. The only real-model team runs
   (10.1) show zero useful parallelism, proposals that changed no admitted work,
   verifiers without the permissions their task needed, and cache reads dominating
   token counts. Prompt policy alone changed input tokens by about 64% in one
   attempt-1 case (11.8). These shape the evaluation design (W3-0009, W6-0005), not
   the architecture.
5. **Not adaptable.** Any code, by rule (`AGENTS.md`); the fixed pipelines of
   attempts 2 and 4; the 24-command commitment algebra of attempt 1; the storage
   design of attempt 2; every "team beats solo" claim (10.11, 11.12, 8).

Gaps in the approved model that the harvest exposes, each also an open question in
10.13 or 11.14: no separate budget namespace for board traffic; no stated bound on
recursive solicitation chains; no frozen profile set per run for comparability;
whether `ToolDenied` is journaled; whether Evidence retains bounded output bytes;
negative controls per Criterion; the generic-memory control and the
single-agent-with-the-same-bytes arm in the evaluation design; distinct CLI exit
codes. Findings F1 and F4 of the external assessment of the v3 proposal are already
answered by the approved model (see the opening of section 10).

## 10. Attempt 2 (tag `legacy-attempt-2`)

Attempt 2 is the repository at tag `legacy-attempt-2`: ymp 0.4.6, 411 commits between 2026-09-12 and 2026-09-15, ~96k lines of Rust, 666 passing tests [verified
manually]. Sources are cited as `legacy-attempt-2:<path>:<line>` (paths relative to that repository root) or as one of its commits. Two evidence sets live outside the
tag and are cited by archive name: `ymp-research/fifteen-2026-09-14/…` (run evidence) and `ymp-research/architecture-v3-2026-09-14/assessment.md` (external
assessment). Path shorthands below: `P/` = `ymp-rust/crates/ymp-providers/src/`, `B/` = `ymp-bridges/claude/src/`, `S/` = `ymp-rust/crates/ymp-storage/src/`, `Rt/` =
`ymp-rust/crates/ymp-runtime/src/`, `T/` = `ymp-rust/crates/ymp-tui/src/`, `D/` = `ymp-docs/`.

What shipped across 0.4.0–0.4.6: a fixed session pipeline over eight crate directories (`ymp-cli`, `ymp-core`, `ymp-eval-driver`, `ymp-providers`, `ymp-runtime`,
`ymp-storage`, `ymp-tui`, `ymp-workspace`) plus a Node bridge to the Claude Agent SDK; three native backends (Codex app-server, Claude Agent SDK, ACP/GLM); one SQLite
store; a Ratatui interface; an evaluation harness [verified manually]. What was measured: 32 offline protocol runs on a scripted provider, 81 effort-policy cases, 12
reuse-guard cases, a journal audit of five uncontrolled sessions (79 invocations), five designed real-model runs of one fifteen-puzzle task, one owner poker session
(`legacy-attempt-2:D/research/research-program-findings.md:13-17`).

The v3 self-diagnosis (YMP-169): no dispatcher (a fixed pipeline), a 4.2k-line engine, validators buried in storage, hybrid tables-plus-events instead of a journal,
upfront decomposition, serial sessions with stale proposals, coarse three-value confirmation, commitments bound to task versions rather than leases, no stall
detection, tokens-only economics, empty retrieval, learning before delivery, unsupported report claims (`legacy-attempt-2:D/architecture/v3/migration-plan.md:14-31`)
[design]. The external assessment recorded its finding set as F1–F8; the research report names these among them: `Engine` is a fixed pipeline with no Dispatcher; the
trace call is a hybrid snapshot, not journal replay; the A8 likelihood-ratio acceptance lets a prior of 0.95 plus one producer check (LR 1.5) reach 0.966 > 0.9
without an independent check; lease expiry is not writer termination; 212 runtime tests protect invariants. Its recommendation was a new kernel inside the same
workspace, not a new repository (`ymp-research/architecture-v3-2026-09-14/assessment.md`, F1–F8 and Options table) [design]; the greenfield choice departs from it.
The approved model of this iteration already answers F1 and F4: `Gatekeeper.revoke` retains unresolved holds and finalization blocks on uncertain
effects, and A8 uses a neutral prior with required evidence-class coverage; F5 (hidden-check isolation) is deferred to W2-0001 and W4-0001 with
`IsolatedCopy` as a required capability.
The approved model of this iteration descends from that v3 proposal: kernel services, Dispatcher, ports, P1–P7 and the R-invariants were first written there,
design-only, never compiled (`legacy-attempt-2:D/self-organizing-team-domain-model.md:446-761`).

Falsifier for the whole section: an entry whose source line no longer exists at the tag, or whose claim the owning task re-measures differently against live tools.

### 10.1 Empirical findings (measured with real models)

Owner: W3-0009 (comparison runner), W6-0005 (evaluation and calibration). Falsifier: a controlled run in the new iteration measuring the same quantity; the number
here is then superseded.

- **No controlled team-vs-solo comparison ever ran**; the only pairwise numbers come from two single runs of different tasks.
  `D/research/fifteen-findings-action-plan.md:28-42`. [measured] [non-causal]
- **The 2026-09-12 research program made zero experimental model calls**; H1–H9 rest on offline runs and a journal audit.
  `D/research/research-program-findings.md:13-17`. [measured]
- **Three uncontrolled "simple html page" traces:** 20/13/15 invocations, 19.43/6.73/15.48 min, 838,568/259,994/1,268,960 reported input tokens, cache-read 89.8% of
  input in one; 54 of 79 audited invocations were proposals, plan reviews and bids. Same file, 99, 183–187, 229–237. [measured] [non-causal] (two projects, partial
  ACP accounting).
- **Baseline run acb9c4e3** (two-agent roster, 200 turns / 900 s / 3 attempts): 20 calls, 9,094,673 input (8,717,839 cache-read), 83,151 output, blocked after 27m00s;
  the unchanged game still passed 14 external criteria; execute phase 7,033,423 input, browser branch 52.5% of input.
  `ymp-research/fifteen-2026-09-14/baseline/summary.json`; `D/research/fifteen-puzzle-repeat-plan.md:81-99`. [measured]
- **A repeat failed pre-spawn after one paid planning call** (89,726 input, 80.77 s, no artifact) because the copied binary resolved its bridge path from
  `current_exe()` ancestors. Same file, 18–23. [measured]
- **Renewed repeat 9f78ce30** reached accepted/unconfirmed with zero reputation: 6 tasks, 21 calls, 6,287,877 input, 80,276 output, 1,591.7 s; 47 check executions (43
  pass, 4 fail); 8 board proposals, 6 stale; 15 waits, all dependency waits; zero overlap; 21 memory retrievals, all empty. `D/evidence/ymp-164/acceptance.md`.
  [measured] [non-causal] — the plan differed from the baseline, so the 30.9% input drop is descriptive.
- **Stronger-pair run c71d4eda:** 1 task, 10 calls, 1,799,631 input (1,611,367 cache-read), 45,540 output, 1,114.2 s, 0 failed checks, one plan rejection that
  strengthened shuffle checks; zero overlap; 10 empty retrievals. Against the baseline pair: −30.0% time, −71.4% input, −43.3% output, 10 vs 21 calls.
  `D/evidence/ymp-168/acceptance.md`. [measured] [non-causal] — models, decomposition, access pattern and verification all changed together.
- **Owner poker session 6e286a3a** blocked at 44:11 after 14 non-overlapping calls, 5 of 6 tasks accepted-unconfirmed; the primary agent reported 11,084,090 input
  (10,825,984 cache-read), 88,216 output, 93 native responses, 86 tool calls; one final-verification invocation alone 6,203,061 input.
  `D/evidence/ymp-155/final-audit.md`. [measured]
- **Useful parallelism was never observed:** zero overlapping invocations in every fifteen-puzzle run and in the poker session; ready-task selection took one task at
  a time; a reserved final reviewer in a two-agent team leaves one producer. `…/research-program-findings.md:57`. [measured] [non-causal] — never tested with
  genuinely independent tasks. Ceremony cost of the fixed workflow was N+3+T×(N+2) invocations, 2N+1 of them before the first execution (same file, 87–99).
- **Failure mode — stale proposals.** Responsibility proposals changed no admitted work in any real run (8 baseline, 6 repeat, 5 poker, 1 pair); the task version
  hashed lifecycle fields, so an executor's check-repair proposal went stale and the task blocked after 3 attempts on a broken browser wrapper (exit 126).
  `ymp-research/fifteen-2026-09-14/baseline/report.md`; `…/fifteen-puzzle-repeat-plan.md:115-128`. [measured]
- **Failure mode — read-only verifiers.** Verification assignments could not do the work assigned (sockets, `/tmp` writes, browser): a 635.128 s verification branch
  in the repeat; the ACP reviewer was denied browser commands in poker, so acceptance stayed unconfirmed although the artifact was externally correct. `…/report.md`
  §3; `D/evidence/ymp-155/final-audit.md`. [measured]
- **Failure mode — malformed JSON.** A valid review decision preceded by CSS braces parsed as malformed JSON and blocked the poker session; a plan-review transport
  error stopped session 3223c6a9 after 2 of 200 invocations because execution re-planned instead of resuming the saved proposal. `D/evidence/ymp-159/README.md`;
  `D/research/session-recovery-144.md:27-49`. [measured]
- **Failure mode — explanation drift.** A reviewer called a failing substitute "stale"; synthesis called a page-title check "renders"; a claimed CSS mobile fix was
  unsupported when pre- and post-files were opened at the same 390×844 viewport. `ymp-research/fifteen-2026-09-14/…/report.md` §3–4 and the pair report. [measured] —
  the structured reason is authoritative, the narrative is not.
- **Failure mode — accounting traps.** 33 of 48 invocations carried no top-level input field (absence ≠ zero); ACP reports only the last request; Codex `willRetry`
  errors were treated as terminal; a completed run stayed `running`; the memory query used the role instruction's first 12 words instead of task terms.
  `…/research-program-findings.md:79,155,211,219`. [measured] (0.3.0 baseline).
- **Integrity gaps at c443ad0** (source review): the final reviewer was drawn from the whole team, so in a two-agent team the executor self-accepted with probability
  ½; reviewer–arbiter agreement created reputation; an empty check list yielded a vacuous success that still raised reputation. `D/requests/02-*.md` Part II.
  [verified manually]
- **Independent review did find real defects in every run** (a harness bug, a plan rejection, an overbroad knowledge claim); rubber-stamping appeared only in a
  fixture where shell checks overrule an approving model; no escaped-defect rate was ever measured. `…/research-program-findings.md:74`. [measured]
- **The designed weak-vs-strong pilot never ran:** 6 conditions, 12 outcomes, proposed 80k/160k observed-token ceilings; the runner was accepted, the roster switched
  twice, one model prohibited, no quota approved. `D/research/weak-agent-pilot.md:209-277`. [design]
- **Absolute token counts are not cost:** 87–90% of input was cache-read, no billing basis was recorded, and no currency figure exists anywhere at the tag.
  `D/architecture/token-usage.md:7-9`. [measured]

### 10.2 Experiment-design rules that survived mistakes

Owner: W3-0009, W4-0002, W5-0002, W6-0005. Falsifier: a W3-0009 design that makes a rule inexpressible; record why rather than dropping it. All [verified manually]
from the fix commit.

- **Measurement validity is separate from task outcome** (75f60a6): a wrong answer, missing deliverable, negative review, fully accounted budget stop and under-used
  roster are valid negative outcomes that continue the matrix; unknown or partial usage, a broken envelope and contradictory reported settings stop the phase and mark
  later cells not-started.
- **Candidate code runs away from the oracle** (75f60a6, after a canary leaked a private marker into selector stderr); **usable time is equalized across arms** (same
  commit): the per-turn timeout equals the condition ceiling and each request gets the rounded-up remainder.
- **Blind identifiers must be independent of condition order** (6e65bed); **controls are bound to accepted product bytes** (94d97c5: the manifest pins runner,
  interpreter, provider and sandbox hashes); **frozen experiment guards are isolated from the working suite** (807c2e1, a24d125, after they blocked unrelated
  development); **the manifest is validated before approval is requested** (0ac1650).
- **Protocol rules with no fix commit** [design]: one criterion set from plan to analysis; freeze validator semantics before launch and adapt selectors after; run an
  independent-attempt control at equal aggregate allowance before crediting cooperation; attribute only at the level varied; keep every failed attempt in the ledger;
  unknown usage stays unknown; pre-register the budget unit; a fresh directory is not isolation (audit native transcripts); do not repair the environment to make a
  run pass; preparation is not execution approval. `D/research/goal-driven-trials.md:160-187`; `D/research/fifteen-puzzle-repeat-plan.md:212-258`.

### 10.3 Provider facts

Owner: W1-0018 (Codex), W1-0003 (Registry), W1-0004 (Treasury), W6-0001 (Claude), W6-0002 (Glm). Versions observed: Codex CLI 0.153.4–0.154.0, Claude Code 2.1.269
with Agent SDK 0.3.246, glm-acp-agent 1.3.0 (`D/research/evidence/effort-capabilities.json`). Falsifier for the section: a provider release whose field names,
defaults or counter behaviour differ; check the installed binary before relying on a line.

**Codex app-server — only what section 2 does not already say.**

- **Thread parameters, differs from attempt 4:** `approvalPolicy:"never"` with `sandbox:"read-only"` or `"danger-full-access"`, plus `developerInstructions`,
  `config.model_reasoning_effort` and `config.mcp_servers.<name>{command,args,env_vars,required,default_tools_approval_mode}`; attempt 4's workspace-write sandbox
  with `writable_roots=[]` does not appear. `P/lib.rs:256-426`. [tested: `settings.rs::codex_sends_assignment_model_and_open_catalog_effort_to_the_turn`]
- **Approval replies** are `{decision:"accept"|"decline"}` from the read-only flag; an unknown server request gets `-32601`. `approvalPolicy:"never"` can reject a
  custom MCP call needing approval, hence `default_tools_approval_mode:"approve"` for the team server only. `P/rpc.rs:231-265`. [design] — no fixture ever sent an
  approval reply.
- **`model/list` beyond attempt 4:** `includeHidden:true`, `nextCursor` paging bounded at 32 pages and 4096 models; a row carries `model` (wire id) and `id` (picker
  id) which may differ, plus `displayName`, `isDefault`, `defaultReasoningEffort`. A real scan saw 8 offerings and models advertising an `ultra` effort.
  `P/settings.rs:24-98`; `D/research/evidence/native-catalog-127.json`. [verified manually]
- **Native retry, differs from attempt 4.** The `error` notification is `{threadId, turnId, willRetry, error{codexErrorInfo, message, additionalDetails}}`;
  `codexErrorInfo` is a string code (`contextWindowExceeded`, `sessionBudgetExceeded`, `usageLimitExceeded`, `rateLimitExceeded`, `serverOverloaded`, `cyberPolicy`,
  `misalignmentPolicyViolation`, `internalServerError`, `unauthorized`, `badRequest`, `threadRollbackFailed`, `sandboxError`, `other`) or an object keyed
  `httpConnectionFailed`, `responseStreamConnectionFailed`, `responseStreamDisconnected`, `responseTooManyFailedAttempts`, `activeTurnNotSteerable`. A strict boolean
  `true` in `willRetry` means the native turn retries itself: keep waiting and record a retry event without incrementing attempts. Before the fix every error was
  terminal and a retried-then-successful turn blocked the session. Commit 0468057. [tested: 9 tests in `ymp-providers/tests/codex_retry.rs`]
- **Resume re-sends the whole configuration** and replaces the MCP block explicitly (attempt 4 only recorded that an ephemeral thread cannot be resumed).
  `Rt/engine.rs:1225-1229`. [tested: `authority.rs::codex_resume_replaces_saved_mcp_configuration_and_native_sandbox`]
- **Usage field names and the reset:** `tokenUsage.total`/`.last` carry `inputTokens`, `outputTokens`, `cachedInputTokens`, `cacheWriteInputTokens`,
  `reasoningOutputTokens`, cumulative per thread; a real probe confirmed a new app-server process resets cumulative counters on resume. `P/usage.rs:4-87`;
  `ymp-evals/reports/token-usage-verification.md`. [verified manually]

**Claude Agent SDK bridge (full set).**

- **Shape.** One Node process per turn; the host sends one request (`run` or `capabilities`) whose params are the whole turn request; the bridge emits notifications
  `execution{sent, reported, native_session_id, native_version}`, `capabilities`, `usage`, `delta{text}`, `tool{name}`, `session{id}` and a result `{text, session_id,
  usage{usage, modelUsage}}`; errors `-32700` / `-32000`; a second concurrent request is refused. `B/index.ts:8-144`. [tested: `B/protocol.test.ts`, the real SDK
  against a scripted CLI fixture]
- **SDK options used:** `query()` over an async-generator prompt with `pathToClaudeCodeExecutable`, `settingSources:["user","project","local"]`,
  `systemPrompt:{type:"preset",preset:"claude_code",append}`, `permissionMode:"default"` (read) / `"bypassPermissions"` (write), `tools:["Read","Glob","Grep"]` when
  read-only, `allowDangerouslySkipPermissions`, `includePartialMessages`, `maxTurns`, `persistSession`, `resume`, a stdio MCP entry with the token in `env`,
  `allowedTools` scoped to it, a `canUseTool` callback, `PreToolUse`/`Stop` hooks. Messages consumed: `system/init` (`session_id`, `model`, `effort`,
  `permissionMode`, `claude_code_version`), `assistant.message.model`, `stream_event content_block_delta text_delta`, `result` (`subtype`, `is_error`, `result`).
  Resume is `--resume=<id>` with a fresh MCP token each turn. Native plan mode was avoided because it blocks MCP team tools. `B/index.ts:36-125`;
  `D/architecture/providers.md:11`. [tested: protocol.test.ts "native resume receives a fresh MCP credential and read tool restriction each time"]
- **Catalog.** `supportedModels()` runs on the control channel before the user prompt is released and returns `{value, displayName, resolvedModel, supportsEffort,
  supportedEffortLevels}`; the list is incomplete (an unlisted explicit model is still allowed); `default` resolves through `resolvedModel`; a bracketed context
  suffix is stripped only for matching and always sent verbatim; effort ladder observed low, medium, high, xhigh, max. `B/settings.ts:16-70`. [tested:
  protocol.test.ts "metadata-only discovery never releases a prompt"]
- **Usage dedup.** Stream usage arrives on `message_start.message.usage`, `message_delta.usage` and `assistant.message.usage` with `input_tokens`,
  `cache_read_input_tokens`, `cache_creation_input_tokens`, `output_tokens`, `output_tokens_details.thinking_tokens`, keyed by the API message id per lane (session id
  plus parent tool-use id). The assistant block's `output_tokens` is a placeholder and is dropped; `result.modelUsage[model]` supersedes stream totals and covers
  subagents; a zeroed crash result must not erase observed usage. `B/usage.ts`. [tested: 4 tests in `usage.test.ts`] `total_cost_usd`/`costUSD` were deliberately
  ignored as client-side price-table estimates and never journaled.
- **Fragility:** the bridge must be built, and its path was resolved from `current_exe()` ancestors, so a copied binary failed after spending planning calls; an
  environment override was added. `Rt/engine.rs:160-165`. [measured]

**ACP / GLM (full set).**

- **Turn.** `initialize{protocolVersion, clientCapabilities, clientInfo}` → `agentInfo.version`, `agentCapabilities.loadSession`; `session/new{cwd, mcpServers[]}` or
  `session/load{sessionId}` (requires `loadSession`); the reply carries `sessionId`, `modes.availableModes[].id`, `models.currentModelId`/`availableModels`,
  `configOptions`. `session/set_model{modelId}` returns `{}` and triggers `session/update` with `sessionUpdate:"config_option_update"`;
  `session/set_config_option{configId, value}` returns `configOptions`; `session/set_mode{modeId}` runs every invocation. `session/prompt{prompt[]}` → `{stopReason,
  usage}`; text streams through `agent_message_chunk content.text`; `end_turn` is success, `max_tokens` a native output limit; `session/request_permission` is
  answered by option kind (`reject_once` / `allow_once`), else cancelled. The MCP token travels in plaintext params here, while Codex gets it only through the
  environment. `P/lib.rs:471-642`. [tested: `settings.rs::acp_*`, `authority.rs::acp_resume_resets_prior_bypass_mode_before_prompt`]
- **Catalog.** Models from `availableModels[].modelId`/`name`; controls exist only for the current model in `configOptions` (`type`, `id`, `options[].value`/`name`,
  `currentValue`); after a model change the refreshed control set must be used because the agent clamps the thought level. Effort is per model: one model offered
  none/high/max with default max, two others only none/on; low, medium and xhigh were invalid. `P/settings.rs:135-258`;
  `D/research/evidence/glm-effort-capabilities.json`. [tested: `settings.rs::acp_uses_refreshed_model_controls_and_checks_acknowledgement`]
- **"Read-only" is not enforcement:** the ACP backend's effective access is always full write even for a read-only request. `P/backend.rs:55-61`. [verified manually]
  Incidents: a native output limit during review killed a session; a binary thinking switch was mislabelled as effort; a connection error in plan review, before any
  task graph existed, blocked a whole session. `D/research/session-recovery-144.md`. [measured]

**Discovery, usage accounting, failures, cancellation, resume.**

- **Snapshot cache.** The catalog file records `source{method, observed_at}` and a fingerprint over kind, command, args and environment-reference names; a failed
  refresh keeps the previous snapshot with an explicit unavailability reason; older than 24 h is stale; a configuration claim never counts as native; scans are
  serialized by a file lock and rejected if the configuration changed meanwhile; the ACP agent is found in the package cache without running the package runner.
  `P/discovery/catalog.rs:40-168`. [tested: `discovery.rs::configured_metadata_cannot_claim_to_be_an_observed_native_response`]
- **Owner catalog rules:** agent names come only from native scans; no hand-maintained model tables; the raw model id is the label and captions are metadata;
  `default` is never displayed as a model; effort is shown only when a graded level was reported (a binary switch is not effort); the provider name is transport, not
  identity. `legacy-attempt-2:AGENTS.md:22-24`. [design]
- **Usage units and arithmetic:** input includes cache reads and writes, output includes reasoning, all components optional, nothing estimated from text, no currency;
  each invocation's latest snapshot replaces the previous, a session total sums distinct invocations, unknown is not zero.
  `ymp-rust/crates/ymp-core/src/usage.rs:7-13`; `D/architecture/token-usage.md:5-9`. [tested: `core/usage.rs::missing_usage_is_distinct_from_zero`]
- **Baselines:** zero for a fresh thread; on resume, the prior invocation's native total or a same-thread notification from another turn; a first report where total
  equals last proves a process reset; an unconfirmable baseline means counting the last report only and marking the record partial. `P/usage.rs:4-87`. [tested:
  `usage.rs::resumed_thread_history_is_subtracted_and_repeats_do_not_accumulate`, `missing_or_reset_baseline_is_partial_not_full_history`]
- **ACP usage covers the last request only**, so it is always finalized-and-partial. `P/usage.rs:89-107`. [tested: `acp_usage_is_not_assumed_to_cover_a_whole_turn`]
- **Failure classification is an allowlist:** eight classes (transient transport, authentication, quota exhausted, unsupported configuration, timeout, cancelled,
  malformed response, unknown); only allowlisted codes classify, including HTTP 401/403/429/503 and JSON-RPC -32000/-32602/-32603/-32700; free text never classifies;
  no application-level retry on rate limits. `P/failure.rs:19-50`. [tested: `failure.rs::generic_connection_error_stays_unknown_and_native_codes_are_allowlisted`]
- **Redaction:** provider stderr is never persisted; the team credential is scrubbed from the whole error chain while the typed failure survives; the endpoint's debug
  formatting hides the token. `P/redaction.rs`. [tested: `authority.rs::debug_output_never_contains_the_live_capability_secret`]
- **Cancellation and timeout are host-side only.** The wrapper validates model, effort and permission mode against the read-only flag, races the backend against a
  cancel token and a timeout, counts streamed characters against an output limit, drains queued events on every terminal path so usage survives, then closes stdin,
  waits 2 s and kills the process group; no native cancel RPC is sent. Defaults observed: 900 s turn timeout, 64,000 output characters, 16 native turns.
  `P/lib.rs:104-178`. [tested: `resource_limits.rs::output_limit_stops_the_provider_path_and_keeps_final_usage`]
- **One spawn path for all providers:** newline-delimited JSON-RPC 2.0 over piped stdio with a 16 MiB line cap, stderr drained to a sink and never logged, host
  agent-detection environment variables removed, named host variables copied by reference, a process group killed on drop, and a re-exec supervisor so that losing the
  host's stdin kills the group. `P/rpc.rs:23-111`. [tested by every fixture test; the supervisor itself is design]
- **Provenance of settings:** `sent` is recorded only from actual wire parameters, `reported` only from the provider's acknowledgement, and any drift fails before the
  prompt. `P/rpc.rs:286-319`. [tested: `codex_retry.rs::native_provenance_distinguishes_sent_reported_and_unknown_settings`]
- **Continuation reuse:** a stored native continuation (session id, config version, requested settings, usage baseline) is reused only when backend identity, version,
  configuration and requested settings all match. `Rt/engine.rs:1607-1612`. [tested: authority.rs, protocol.test.ts]

### 10.4 Team-protocol invariants proven by tests

Source root: `ymp-rust/crates/ymp-{core,runtime,storage}`. Falsifier: a new-model rule that contradicts the stated invariant; the entry is then deleted with the
decision that replaced it.

- **Atomic admission.** One IMMEDIATE transaction admitted assignment, invocation and grant together, validating team capture, membership and eligibility, the
  reserved final reviewer, one running assignment per agent, one executor per task attempt, the claim still running for this assignee, no write mode for a read-only
  task, budget, and the next unspent ordinal; a denial wrote an event and no invocation. [tested: `mcp_tests.rs::competing_runtime_admissions_commit_one_assignment`]
  → Gatekeeper `admit`, R-3/R-9. Owner W1-0006.
- **Grant lifecycle.** A grant bound session, agent, assignment and invocation with an explicit operation list; the live token existed only in server memory and the
  native process environment; any terminal state, server drop or restart revoked it; each request id committed once; arguments were allowlisted so no field could name
  a session, agent or permission. [tested: `mcp_tests.rs::server_drop_ends_grants_and_restart_requires_fresh_admission`] → Grant, R-2. Owner W1-0006, W3-0001.
- **Roles do not carry over:** a fresh assignment rotated scope without discarding native context — context travels, authority does not. [tested:
  `mcp_tests.rs::fresh_assignment_rotates_scope_without_discarding_native_context`] → R-2. Owner W1-0007.
- **Strategies never commit:** allocation, board, resource and recovery policies returned proposals the runtime revalidated. [tested:
  `board_coordination.rs::replacing_strategy_changes_choice_without_bypassing_runtime_validation`] → R-1. Owner W1-0001, W1-0006.
- **Unconfirmed acceptance never credits.** [tested: `engine.rs::accepting_without_confirmation_never_awards_reputation`] → R-11, CreditPolicy. Owner W5-0001.
- **Failing applicable evidence overrides every approver.** [tested:
  `confirmation_tests.rs::confirmation_failed_applicable_assertion_overrides_every_approving_agent`] → R-6. Owner W1-0010.
- **A reserved final reviewer is required and execution may not consume it**; without an eligible non-producing final reviewer the session blocked. [tested:
  `allocation_admission.rs::public_admission_must_not_consume_reserved_final_reviewer`] → R-5, P7. Owner W1-0006, W1-0013.
- **Write concurrency:** a writer excludes overlapping paths, readers overlap, unbounded native access serializes everything; a reservation is single-use; the
  executed request, backend and access may not widen after admission. [tested:
  `concurrency.rs::scoped_backend_overlaps_independent_read_but_serializes_conflicting_writers`, `native_unbounded_access_serializes_writers_and_readers`] →
  WorkspaceGuard `lock`, R-9. Owner W1-0005, W1-0006.
- **Budget denial creates nothing** — no grant, no invocation, no spend. [tested:
  `budget_authority_tests.rs::joint_budget_denial_creates_no_grant_or_live_capability_across_connections`] → Treasury `reserve`, R-8. Owner W1-0004, W1-0006.
- **Accepted results are immutable; drift invalidates rather than edits**; artifact drift during review rejected the attempt and required a new attempt and a new
  review. [tested: `confirmation_tests.rs::confirmation_stale_artifact_requires_a_new_attempt_and_new_review`] → R-16. Owner W1-0008, W1-0010. Storage also refused
  forged result, review and credit links, and could store a review only as an unconfirmed or rejected assessment [tested:
  `confirmation_tests.rs::confirmation_storage_rejects_forged_result_review_and_credit_links`] → R-17, owner W1-0019.
- **Recovery holds survive membership change; membership is not permission.** [tested: `session_recovery/rework.rs::r2_membership_preserves_explicit_owner_wait`] →
  R-19/R-20. Owner W1-0012.
- **Board claims are atomic on the task version and have one winner**; proposals were consumed only at wave boundaries with zero running assignments, and the runtime
  revalidated stale plan, membership, task state, additive revision and budget inside the commit transaction. [tested:
  `board_coordination.rs::actual_team_tools_commit_one_of_two_conflicting_claims_and_execute_the_winner`] → Arbiter, P2. Owner W3-0001, W3-0003.
- **Structured-response parsing.** Every agent verdict was one JSON object; the parser accepted direct or fenced JSON, otherwise scanned brace blocks, and rejected
  malformed, incomplete, multiple or prose-trailing decisions. A parse failure became a malformed-response failure with its own record kind, never a verdict; identity
  never came from the response, only from the assignment. [tested: 12 parser unit tests in `ymp-core/src/model.rs:518-716`;
  `session_recovery.rs::negative_verdict_requires_revision_but_malformed_response_is_not_a_verdict`; commit b57355f] → R-21, P4/P6. Owner W1-0017, W3-0006.
- **Effort pins:** a fixed pin was a singleton allowed set; a conflicting choice failed before invocation and an allocation policy could not override a captured pin.
  [tested: `allocation_tests.rs::allocation_policy_cannot_override_captured_effort_pins`] → R-3. Owner W1-0003, W1-0006.

### 10.5 Acceptance contracts and recovery rules

Owner: W1-0002 (criteria), W1-0009 (checks), W1-0010 (acceptance), W1-0012 (recovery). Falsifier: a W1-0010 design with more than three grades, or a W1-0012 decision
to replay effects without independent inspection.

- **Contracts were installed by the trusted client before the first invocation**, captured with input snapshots and verifier-file digests, immutable on resume, bound
  to exactly one planned task by title. `ymp-rust/crates/ymp-core/src/confirmation.rs:19-135`. [tested:
  `contract_ingress_tests.rs::configured_resume_preserves_capture_and_rejects_replacement_before_invocation`,
  `configured_input_and_verifier_drift_on_resume_is_not_recaptured_or_confirmed`]
- **Agents saw only criterion, artifact and input names**, never expected bytes. Check kinds were exact bytes, matches-input, and a command with pinned program and
  verifier digests; paths were confined to the work directory with 4 MiB caps. The runtime ran a check itself only if artifacts, inputs and verifier code were still
  byte-identical and the checker identity matched; the outcome was stored as pass, fail or inconclusive evidence. `Rt/checker.rs`. [tested:
  `confirmation_tests.rs::confirmation_pinned_validator_captures_actual_process_and_data`]
- **A task without a contract could never be confirmed.** Acceptance and confirmation were separate facts, and accepted-unconfirmed results still satisfied
  dependencies and completed sessions. `S/confirmation.rs:201-268`. [tested: `confirmation_tests.rs::confirmation_coverage_and_agreement_do_not_imply_quality`] Two
  documents at the tag disagree on whether a dependency may require a confirmed predecessor (`D/architecture/confirmation.md:5` vs
  `D/architecture/runtime-contract.md:34`); W1-0010 decides once.
- **Ordinary checks were free-text shell strings** run through a shell with a 300 s timeout, and a broken command could not be removed because revisions were additive
  and proposals were pinned to a whole-task digest that changed on submit and review. Commit e063494 added a replacement bound to a lifecycle-insensitive definition
  digest, restricted to the active executor, requiring old-fails and new-passes evidence plus one independent review, trivial commands forbidden. [tested:
  `board_coordination.rs::active_executor_can_replace_a_broken_ordinary_check_before_attempts_exhaust`,
  `trivial_check_replacement_is_refused_at_independent_review_boundary`] A patch on a weak primitive; W1-0009 owns typed checks.
- **Recovery rules (ymp146).** Admitted work is durable responsibility that departure cannot erase; only unadmitted ready commitments are released (900bde4). A
  membership change is never a permit (b982322). A failed owner's task is scheduled around, and an ended read-only transport failure goes to independent interruption
  review without restart (e9a5649). Replaying uncertain effects requires a compiled backend effect scope, read-only independent inspection, and revalidation at commit
  (ca43a5f). A completed-but-malformed invocation is inspectable only with its exact malformed record and never reclassifies a real verdict (f297c6a). Interrupted
  invocations without a completed message become unknown failures; nothing is replayed automatically. [tested: the `r1_`…`r4_` families under
  `ymp-runtime/tests/session_recovery/`]
- **Owner decisions were typed commands** with a command id and an expected revision; receipts keyed by a digest of session and command id made replay idempotent and
  content reuse a conflict. Actions: retry or continue (refused unless effects were proven safe), wait on a condition, pause, release hold. `S/recovery.rs:84-240`.
  [design plus runtime tests]
- **What could not be recovered.** File contents were never copied — only path, status and digest. The effects of a write-capable invocation without termination
  evidence and without a backend-declared local effect scope could not be resolved, and no native or ACP backend ever declared one. Replay therefore required an
  independent read-only inspector whose evidence (≤64 files, ≤16 MiB) was recomputed at commit and again at admission; failing that, a separate owner authorization
  admitted new work while leaving the uncertainty recorded. No rollback existed. `D/architecture/recorded-checks-and-recovery.md:52-64`. [design plus runtime tests]

### 10.6 Storage, workspace and concurrency lessons

Owner: W1-0016 (journal and payloads), W1-0005 and W1-0007 (WorkspaceGuard, commitments), W1-0017 (execution host). Falsifier: a W1-0016 decision deriving the
Registry from the Journal rather than dual-writing; several negative lessons then no longer apply.

- **Negative — dual writes.** Every mutation wrote the current-state row and an event row in one transaction, and recovery never replayed events: snapshot rows were
  authoritative, events only served export; two sources of truth kept consistent by discipline alone. `S/lib.rs:188-192,304-322`. [tested:
  `event_failure_rolls_back_invocation_and_usage_state`]
- **Negative — a key-value catch-all** became the home for 15+ record kinds (recovery stages, grants, board, team state, budget denials, receipts, turn counters)
  addressed by string prefixes and scanned by substring, with no index on the JSON and `json_extract` filters in the WHERE clause. `S/schema.sql:1-12`. [verified
  manually]
- **Negative — the recovery stage as a mutable blob** updated by compare-and-swap, with three revision counters (stage, owner policy, team) checked in one transaction
  and a new binding hook added by every later feature. `S/recovery.rs:10-79,243-250`. [verified manually]
- **Negative — durability thinner than attempt 4:** only WAL, `foreign_keys=ON` and a 5 s busy timeout; no `synchronous` setting and no row checksums; crash
  consistency beyond SQLite defaults untested. `S/lib.rs:86-91`. [verified manually]
- **Negative — full-tree hashing as change detection.** Opening a workspace hashed every file (symlink target for links) under a fixed skip list, and the change file
  written once at run end could not say which agent wrote what; 59bece8 was needed to stop hashing on mere listing. `ymp-rust/crates/ymp-workspace/src/lib.rs:14-182`.
  [tested: `listing_skips_regular_file_contents_but_change_detection_reads_them`]
- **Negative — flock inheritance.** A forked child inherits the open file description, so returning the raw handle and relying on close left a finished run's lock
  held and produced a false "already running in another process"; fixed by an explicit-unlock guard (43ba6cc), after which the defect recurred in the configuration
  and catalog-scan locks. [tested: `completed_project_owner_releases_lock_before_inherited_descriptor_closes`] Unix only.
- **Negative — git2 lock ordering** needed three fix commits (a5e6091, 68b481d, 1e34487) to reserve references before changing checkout files, resolve the target
  after acquiring its reference lock, and verify refusal on a foreign lock. [tested: `preexisting_git_locks_refuse_checkout_without_changing_files_index_or_head`]
- **Negative — storage became the invariant layer:** ~1000-line assertion chains, 40+ string decision kinds, validations reloading and filtering all session decisions
  in Rust, eight recovery fix commits in three days, pervasive legacy-shape branches. `S/confirmation.rs:273-879`. [verified manually]
- **Negative — start-time recovery proved nothing about liveness.** Start and resume marked every still-running invocation interrupted, revoked its grants, wrote a
  release for any acquisition without one, and verified the stored workspace was the same directory; an open invocation after a crash proves nothing about a live
  process (liveness was never observable). `Rt/engine.rs:436-525`. [tested: `cancellation_failure_and_recovery_preserve_partial_and_unknown_usage`] Related: a check
  killed or timed out was written nowhere, and a record with no success field read as unrecorded, never as a pass (`S/lib.rs:365-398`).
- **Positive — per-invocation grants over a Unix socket.** Background agents never touched SQLite; they called an in-process team server over a Unix socket with a
  per-invocation grant, and every call revalidated the grant, the running assignment/invocation and the task claim in one transaction with allowlisted arguments;
  persisted grant ids were not credentials. `S/authority.rs:93-161`. [verified manually]
- **Positive — ordinals as a monotonic counter:** an explicit admission had to take the next unspent ordinal; gaps were unknown historical calls never refunded; usage
  without reports was unknown, not zero. [tested: `explicit_admissions_require_the_next_unspent_ordinal`, `recorded_turns_without_events_are_unknown_not_zero`] The
  counter lived outside the event log; the new Journal should not repeat that.
- **Positive — memory idempotency by digest.** A knowledge entry id was a digest over acceptance id, proposal and applicability, so retention was idempotent and could
  not revive a retired entry; correction was an atomic supersession requiring a confirmed acceptance whose captured contract named the target version digest, an
  independent review that saw that relation, a cycle check, and a receipt idempotent by proposal digest. `S/knowledge.rs:298-306`,
  `S/knowledge_correction.rs:187-370`. [verified manually] Owner W5-0003.
- **Positive — access coordination rules.** Readers overlap, disjoint scoped paths overlap, an unrestricted writer owns the directory; a policy may broaden but never
  narrow the access the backend actually enforces; scoped paths reject traversal, symlinks and hard links; verification takes exclusive access because check commands
  may write; release is not acceptance. `Rt/workspace_access.rs:38-110`. [tested per `D/architecture/concurrency-implementation.md:25-58`] In-process only —
  cross-process was a single project flock.
- **Positive — freshness against the captured directory:** result freshness was judged against the immutable captured directory, never the mutable project path;
  relocation changed registration only, and a legacy result without a capture returned an explicit unknown location (50b442d). [tested:
  `legacy_outcome_without_captured_directory_does_not_invent_a_location`]
- **Workspace policy:** work directly in the user's directory, home holds metadata only; an earlier isolated-copy design was abandoned after it lost context, and the
  isolated-candidate task was paused for the MVP. `D/architecture/workspace-policy.md:3-21`. [design] W4-0001 revisits isolation deliberately.

### 10.7 Interface and CLI facts

Owner: W1-0015. Falsifier: a W1-0015 decision on the command surface or the library stack; the versions below were the state at 0.4.6, not a requirement.
The owner selected this interface as the practical visual and interaction basis for W1-0015 (model section 3.9, amendment 12.1 item 7);
`ymp-docs/tui-reference.md` is the self-contained reference, and this section records the mechanics and tests behind it.

- **Library stack:** ratatui 0.30.2 (the 0.29→0.30 migration was deliberate), crossterm 0.29, clap 4 derive, tokio with the full feature set, tokio-util cancellation
  tokens, unicode-width 0.2, git2 0.21 vendored with no network transports and static zlib; toolchain pinned to Rust 1.89. `legacy-attempt-2:Cargo.toml:30-50`.
  [verified manually]
- **Rejected alternatives with reasons:** gix (API scope); syntect-tui (pins the older ratatui); tree-sitter (needs Rust 1.90 and C crates); tui-file-explorer,
  ratatui-interact, ratatree, ratatui-async-explorer (source review); ratatui-code-editor 0.0.6 (white fallback breaks light themes, CJK cursor bug, CRLF on Enter,
  double insert, unbounded cache); an installed `git` binary (owner rule). `D/architecture/ratatui-code-editor-assessment.md:53-87`. [verified manually] Of two
  unstable ratatui features only widget-ref was used; rendered-line-info and crossterm's event stream were never called (input was an 80 ms poll on a dedicated thread
  feeding a channel).
- **Event loop:** one select over a runtime-event channel, the input channel and a 40 ms ticker; after any wake both queues are drained before drawing, so streamed
  deltas never delay keys; draw only when dirty and ≥16 ms since the last frame; a re-arm flag re-schedules a frame that deferred highlighting; background work is
  held as join handles, each with its own cancellation token, at most one of each kind. `T/lib.rs:103-235`. [tested:
  `streaming_text_does_not_move_the_reading_position`]
- **State and render separation:** one module decides what a key means and returns actions and never starts a provider; one module alone writes into a frame; views
  produce lines for a single width; pages present controller snapshots taken at named moments and never read the store, filesystem or PATH while painting; page build
  is cached by view, width and revision. [tested: `read_only_pages_never_produce_a_run_action`, `the_inspect_surface_is_built_for_its_own_width_and_not_the_terminal`]
- **UI language the owner insisted on:** an agent's label is the concrete model identifier, never an installation caption, provider or actor id; the `default` alias
  is hidden and an unresolved alias is not offered; effort appears only when a graded level was reported, and on/off switches stay in details; an explicitly reported
  "none" is shown; a message is named by the invocation linked to it, an unlinked one is "unknown model", never renamed by later turns or current configuration.
  `T/label.rs:1-12,124-135`. [tested: `every_message_names_the_model_and_effort_of_the_invocation_that_wrote_it`]
- **Three columns: requested, sent, reported.** Sent-but-unreported is "unconfirmed", never "applied". Acceptance is confirmed (evidence passed every criterion),
  unconfirmed (independent review alone) or unknown (never a pass); accepted is not confirmed. An unreported count is an em dash, never 0; a growable total carries a
  plus; cache and reasoning are never summed twice; figures belong to agents, never to providers. "Nothing here invents a value."
  `D/architecture/ui-language.md:8-31`. [tested: `an_assignment_shows_what_was_sent_and_never_claims_an_unreported_setting_applied`,
  `an_acceptance_on_review_alone_is_not_presented_as_confirmed`, `unknown_is_not_zero_and_a_growing_total_says_so`]
- **Terminal quirks:** raw mode, alternate screen, bracketed paste; escape-code disambiguation pushed when supported so Escape acts immediately; a panic hook restores
  the terminal; markers fall back to ASCII when the locale variables contain "ascii", "c" or "posix"; NO_COLOR is honoured by crossterm, not product code, and
  monochrome selection is marker-only; repeat and release key events are ignored, so a terminal reporting a held interrupt as repeats exits; colour must be redundant
  with a word or marker; SQLite metadata on a Linux host bind mount failed with EINVAL. `T/terminal.rs:46-64`; `D/releases/0.4.6.md:70-72`. [tested:
  `a_repeated_or_released_key_is_not_a_second_press`; rest verified manually]
- **CLI commands:** globals for home and working directory; subcommands `init`, `doctor` (probe, provider, team-tools options), `run PROMPT` (`--json`, `--no-memory`,
  `--no-adaptive`, a settings file), `catalog`, `capabilities`, `resume`, `ask`, `sessions`, `trace`, `config`, `reputation`, `memory`, `relocate`, `demo`, `mcp`, and
  a hidden supervisor command; no subcommand opened the interface. `ymp-rust/crates/ymp-cli/src/main.rs:12-128`. [verified manually]
- **`--json` shape and exit codes:** `run --json` printed exactly one JSON object with session, workspace and summary on stdout, events going to stderr only in
  non-JSON mode; exit 1 whenever the session status was not "completed"; every error printed one prefixed line and exited 1. There were no other exit codes, so the
  status did not distinguish failure classes. Same file, 130–135, 171–192. [tested:
  `executable_contract_confirms_and_supplies_supported_knowledge_to_a_later_session`]
- **Demo, doctor, catalog:** `demo` built a configuration with one internal mock provider and three deterministic agents, kept the real configuration's acceptance
  contracts and knowledge scope, and ran one fixed prompt headless or in the interface. `doctor` printed availability per provider from PATH inspection; its probe
  sent a fixed short read-only prompt at the configured effort and was explicitly not an unattended test (probing every profile in a 19-agent pool is dangerous); its
  team-tools option proved a real team call through a marker and failed if the call never happened; any failure exited non-zero. `catalog --refresh` asked providers
  one at a time under a per-provider timeout with no model prompt. [tested: `demo_keeps_explicit_contract_configuration`,
  `team_probe_charges_its_first_explicit_invocation_at_admission`, `failed_initial_and_bounded_scans_leave_explicit_unresolved_state`]

### 10.8 Evaluation harness facts

Owner: W3-0009 (comparison runner), W2-0004 (check discrimination), W4-0002 (sealed attempts), W6-0005 (evaluation suite). Falsifier: a W3-0009 decision to use a
lighter local-only gate instead of manifest-and-approval; the authorization entries then become history.

- **Scenario declaration.** Each workflow declared prompt, inputs, output, reference, criterion ids, check id and expected confirmation, plus a comparison block
  naming treatments (solo, independent attempts, cooperating team) with shared inputs, a budget including selection and review, a fresh non-repository directory and
  separate metadata per treatment, and null for missing cost. A separate file declared 13 scripted protocol cases as setup, script and expected events. The driver
  embedded source and fixture hashes at build time, refused to run on drift, and marked a run complete only when all 17 cases passed.
  `ymp-evals/scenarios/universal-workflows.json:61-104`; `ymp-rust/crates/ymp-eval-driver/src/main.rs:57-138`. [tested: 24 functions in
  `ymp-evals/tests/test_universal.py`]
- **Conditions:** six — strong solo, weak solo (a raw turn call), independent two and three (isolated candidates in ordinal order sharing one deadline and one
  admission account), cooperating two and three (the real engine with one member reserved as final reviewer, memory and adaptation off). The same manifest consumer
  ran a scripted and a native execution kind. Selection took the first candidate passing the visible public check, else the first ordinal — no model, no private
  oracle. `…/weak_pilot/consumer.rs:847-1006`; `ymp-evals/weak-pilot/public_select.py:21-51`. [tested:
  `all_six_conditions_consume_real_task_exports_and_freeze_one_blind_result`, `test_selection_has_no_hidden_oracle_even_when_later_answer_is_correct`]
- **Blinding:** a random 32-hex identifier per attempt; a seal step copied the frozen candidate into an opaque directory with a submission file carrying the blind id,
  task, variant and file digests and null placeholders for acceptance and quality; the scorer read only that file; the condition mapping stayed outside it; names must
  not encode models or conditions. `ymp-evals/weak-pilot/observer.py:75-89`. [tested: `test_input_tampering_and_stale_artifact_are_rejected`]
- **Validity versus outcome:** one inspection rule covered raw and engine traces alike — roster equals manifest, no duplicate invocation, an originating assignment, a
  unique native context per actor, terminal accounting closed, usage complete, requested matching sent for a fixed setting; 10.2 states which results continue the
  matrix and which stop the phase. `…/weak_pilot/measurement.rs:10-120`. [tested: `ordinary_negative_results_do_not_gate_calibration_or_later_conditions`,
  `unknown_usage_and_deadline_stop_the_same_consumer_without_losing_observations`]
- **Authorization gating:** a native run required an approval file outside every workspace with a schema version, an owner authority, a reference, an approval time,
  the exact manifest digest, the phase and a single-phase-once scope; an atomic marker under a state directory was claimed before any output and survived failure; the
  pilot additionally required a native calibration report that allowed it. [tested: `unapproved_native_manifest_stops_before_provider_work`]
- **Result schema rules.** Per attempt: status, blind id, artifact digest, selection, invocations (agent/assignment/invocation ids, requested/sent/reported model and
  effort, tokens, coverage), resources (elapsed, input, output, cached, reasoning, observed input plus output, coverage, a null reported cost, peak active
  invocations), runtime acceptance / runtime confirmation / external score kept separate, captured versus invoked agent ids, actual participant count, protocol
  deviations. Rules: unknown is null and never 0; cache ⊂ input and reasoning ⊂ output; usage snapshots replace and never add; never aggregate by provider. [tested:
  `test_manifest_and_result_templates_leave_actual_observations_null`]
- **Everything that actually ran was model-free:** 17 driver cases, an offline proof, a 12-cell scripted matrix, 643 Rust and 62 Python tests, and real-terminal
  checks against the demo mock provider; no native measurement ever executed. `D/research/evaluation.md:7-18`. [verified manually] This 643+62 count and the 666-test
  figure above come from different documents at the tag and are not reconciled.

### 10.9 Pitfalls with fixing commits

Each row is a bug attempt 2 shipped and then fixed; the commit is where the reasoning lives.

| Commit | Lesson |
|---|---|
| 0468057 | A provider's own retry notice is not a terminal failure; wait, and do not spend an attempt. |
| e063494 | A check must be replaceable without a whole-task digest change, under evidence and independent review. |
| b57355f | Parse a structured verdict strictly; a parse failure is a typed failure, never a verdict. |
| 900bde4 | Departure releases only unadmitted responsibility; admitted work is durable. |
| b982322 | A membership change is not a permit and does not release recovery holds. |
| e9a5649 | Schedule around a failed owner; an ended read-only failure is reviewed, not restarted. |
| ca43a5f | Replay only after independent read-only inspection with revalidation at commit. |
| f297c6a | A completed-but-malformed response is inspectable and never reclassifies a real verdict. |
| 43ba6cc | Release a lock explicitly; a forked child inherits the file description and holds it. |
| 59bece8 | Listing a directory must not hash file contents. |
| a5e6091, 68b481d, 1e34487 | Reserve references before touching the checkout, resolve the target after taking its lock, verify refusal on a foreign lock. |
| 50b442d | A result's location is the captured directory; without a capture, say unknown. |
| 75f60a6 | Separate measurement validity from task outcome; run candidate code away from the oracle; equalize usable time. |
| 6e65bed | Blind identifiers must not be derivable from condition order. |
| 94d97c5 | Bind experiment controls to accepted product bytes. |
| 807c2e1, a24d125 | Isolate frozen experiment guards from the working test suite. |
| 0ac1650 | Validate the manifest before asking for external authorization. |
| 54f3da8 | Interface state, sidebar and theme persistence are one design decision, not incremental patches. |

### 10.10 Edge-case checklist by owning task

Scenarios to reproduce against the new types, not tests to port.

- **W1-0016 (journal, durable state):** an event-write failure rolls back invocation and usage state; a migration is transactional, keeps legacy capture unknown and
  does not re-add cumulative history; three connections racing admission admit exactly two with distinct ordinals; recorded turns without events are unknown, not
  zero; a session save cannot rewind missing historical spend; recorded checks read back in order without completing missing fields; a completed run releases its lock
  before an inherited descriptor closes.
- **W1-0018 (Codex), W6-0001/W6-0002 (Claude, Glm):** every scenario in 10.3, plus: a native retry disables neither timeout nor cancellation and keeps only known error
  codes; provenance distinguishes requested, sent, reported and unknown; native defaults stay omitted and model drift is rejected before the prompt; resume replaces
  the saved MCP configuration and sandbox and resets a prior bypass mode; metadata-only discovery never releases a prompt; configured metadata cannot claim to be a
  native response; pool inspection never runs the provider; debug output never contains the live credential.
- **W1-0009 (checks):** a pinned validator captures the actual process and data; a failed applicable assertion overrides every approving agent; a stale artifact
  requires a new attempt and a new review; an active executor can replace a broken check before attempts exhaust; a trivial replacement is refused at the independent
  review boundary; coverage and agreement do not imply quality.
- **W1-0015 (CLI and interface):** every scenario in 10.7, plus: streaming text does not move the reading position; a late background reply is dropped and leaving the
  page starts nothing beside it; read-only pages produce no run action; help lists every command in the registry; every page renders at 40x12; a repeated or released
  key is not a second press; a file preview keeps its size while it scrolls to its last line.
- **W1-0006, W1-0007 (kernel services):** competing admissions commit one assignment; reassignment revokes the previous attempt in the same commit; a grant event
  rolls back when a later insert fails; admission cannot enlarge reviewed read-only authority nor consume the reserved final reviewer; terminal credentials reject
  completion, failure and cancellation; restart restores no credentials from native context; a replayed request has only one effect; a budget denial creates no grant
  or live capability.
- **W1-0005 (workspace):** a scoped backend overlaps an independent read but serializes conflicting writers; unbounded native access serializes writers and readers; a
  lease admission binds identity and a drop revokes before releasing access; listing skips file contents but change detection reads them; writes stay in the working
  directory and the home holds only metadata.
- **W3-0001, W3-0003 (team protocols):** real team tools commit one of two conflicting claims and execute the winner; racing public claims have one winner and issue
  no authority; malicious proposals cannot override identity, versions, pins or contracts; board proposals cannot self-grant or commit runtime state; a parallel
  commitment waits for a busy owner and keeps prior claims running; a memory proposal records a bound origin without activation.
- **W1-0012 (recovery):** the `r1_`…`r4_` families — membership preserves an explicit owner wait, an exhausted policy and an owner pause; an injected policy cannot
  replay an uncertain writer; an interrupted review has no invented termination evidence after restart; a negative verdict requires revision but a malformed response
  is not a verdict.
- **W3-0009 (comparison runner):** every scenario in 10.8, plus: all six conditions consume real task exports and freeze one blind result; ordinary negative results do
  not gate calibration or later conditions; unknown usage and a deadline stop the same consumer without losing observations; a solo condition uses the whole remaining
  time and independent members do not reset it.

### 10.11 Not carried over

- The fixed execution pipeline and string purposes; upfront decomposition with proposals consumed only at wave boundaries; complexity by prompt length and risk by
  keyword; the literal "where is the file?" hint list.
- The record-links catch-all (≈30 optional fields, string decision kinds) and the trace rebuild — a typed Journal replaces both; string decision kinds generally;
  free-text shell checks as acceptance and the replacement patch on top of them; a free-text task result field; binary task access and a string permission mode as the
  access model; agents proposing membership changes; the prompt-embedded "runtime-selected method"; success-only reputation with no diagnosed failure observations; a
  closed string enum for competence.
- The key-value catch-all with prefix scans; dual snapshot-and-event writes; the recovery stage as a mutable blob; full-tree hashing as change detection; the
  workspace and change JSON files beside SQLite; the turn counter outside the log; branch switching from the interface.
- The mock provider driven by instruction markers and purpose-string dispatch; generated native agent ids and legacy-placeholder migration; the supervisor keyed on
  the binary name; a bridge path field in the turn request; configuration capability claims; the appended "must be in English" instruction; the ACP mode-name
  candidate list.
- Unused terminal feature flags; deterministic digest-based blind ids; the permanently refusing v1 native command; the macOS-only sandbox wrapper as-is; ad-hoc popup
  height fixes (carry the rule, not the code).
- Every "team beats solo" or "weak team approximates strong solo" claim, the vote arithmetic and its citations; cost-penalised selection; effective-token-only
  display; a universal effort ladder; prefix-allowlist check gating; historical quota numbers proposed and never approved.

### 10.12 Denylist candidates for `tools/legacy_scan.py`

Verified present in `legacy-attempt-2:ymp-rust` by read-only grep (occurrences in parentheses) and absent from the approved model: `Engine` (136), `RecordLinks` (71),
`TeamServer` (50), `NativeContinuation` (6), `PlanVersion` (18), `RecoveryStage` (51), `StoreLock` (11), `AccessCoordinator` (6), `TurnRequest` (162), `FailureClass`
(51), `TokenCounts` (65), `TrustedCheck` (23), `GrantRecord` (33), `BoundedRecoveryPolicy` (7), `McpEndpoint` (10), `FileSnapshot` (33), `ConfirmationStatus` (127),
`TaskAccess` (35), `WorkspaceAccess` (123), `BoardProposal` (20).

Excluded on purpose because the approved model uses them: Journal, Registry, Treasury, Gatekeeper, Arbiter, Dispatcher, Assignment, Grant, Commitment, Snapshot,
Check, Criterion, ExecutionBackend, CostModel, ExperienceVault, AcceptanceAuthority, WorkspaceGuard, Offer, Award, Observation, Reputation, Knowledge. Note: `Engine`
and `WorkspaceAccess` are generic enough to produce false positives; add them scoped to whole-word type positions.

### 10.13 Open questions for the owner

1. The external assessment advised a new kernel in the same workspace, not a new repository — what changed the decision, and is data compatibility with attempt 2's
   home directory abandoned?
2. Which task class is the team supposed to win, given that no fixture with genuinely independent work has ever run?
3. Will any paid comparative pilot be authorized in this iteration, and does the earlier model prohibition still stand?
4. Should AcceptanceAuthority use belief only for prioritization, as the assessment recommends? The approved model still lets A8 move a criterion to
   Satisfied once belief passes the threshold with the required evidence classes present; is that the intended boundary?
5. Which confirmation grades earn experience — the CreditPolicy decision attempt 2 left open, with no confirmed outcome ever produced to calibrate it?
6. May a dependency require a confirmed predecessor? Two documents at the tag contradict each other.
7. How does Treasury settle when a backend reports partial or no usage and no hard token bound exists?
8. How does WorkspaceGuard learn a native backend's actually enforced access, which attempt 2 never solved?
9. Is the Registry derived from the Journal, or dual-written as attempt 2 did everything?
10. Should retrieval be gated on a measured control, given that no benefit was shown and every trial retrieval was empty?
11. Should the CLI define distinct exit codes for validation, run failure and cancellation, where attempt 2 had only 0 and 1?
12. Should a checker's identity and version be part of the Criterion so grade rules survive a checker upgrade?

## 11. Attempt 1 (tag `legacy-attempt-1`)

Attempt 1 ran 2026-08-11 to 2026-09-06: 722 commits, 19 crates under `ymp-rust/crates/`, 134,375 lines of Rust and 968 `#[test]` functions (counted at the tag; the harvest brief's "~850" is low). Product and planning documents are Russian, but `ymp-docs/ARCHITECTURE.md`, `INVARIANTS.md`, `SECURITY.md` and `PROJECT-CONTRACT.md` are English and `ymp-rust/SCHEMA.md` holds the concrete storage rules — the brief and the audit disagree here; the files decide. Entries below are wrapped one per line.

Confidence marks: [tested: <fn>] a named test exists; [measured] a number from a recorded run; [verified live] observed against a real model or host; [design] documented and never executed.

- **Scientific goal.** From 2026-09-06 the primary objective is cumulative knowledge with provenance: a bounded knowledge item carrying bytes, digest, source task, evidence, authoring procedure, intended scope and known limits; accumulation means a later frozen snapshot carries usable provenance from more than one earlier point. `legacy-attempt-1:ymp-docs/research/rdr-002-cumulative-knowledge-poc.md:22-30`, `legacy-attempt-1:ymp-docs/work/backlog/OWNER-DIRECTION-20260906.md:5-7` [design]
- **The equal-budget program was abandoned, not refuted.** Best-of-11 and the 1,992-task sample size were correctly derived (Hoeffding, familywise alpha 0.05, minimum useful effect 1/8, per-contrast power 0.98333); the frozen corpus held 4 tasks against `total_additional_required: 3980`, and a manifest split conflict made diagnosis and transfer jointly unidentifiable. `legacy-attempt-1:ymp-rust/tools/ymp-corpus/corpus/study/power-report-v1.json`, `legacy-attempt-1:ymp-docs/research/map-002-coordination-research-queue.md:125-137`. The owner cut it on 2026-09-06: "better results at the same budget are not a required product outcome". [measured] for the arithmetic, [design] for the study.
- **Two authorized live runs, both uninformative about capability.** run-002 (2026-09-01) ended after 47.65 s with a runtime process exit: no completion, no usage, no tool event, no attestation, and whether a provider request was sent recorded as unknown and explicitly not zero. run-003 stopped at its deterministic prelive gate with 0 model calls and 0 cost — zeros because nothing ran, kept distinct in the record from run-002's unknowns. `legacy-attempt-1:ymp-docs/research/run-002-first-authorized-live-tool-host-probe.md:5-12,60-78`, `.../run-003-w1-evl-04s-prelive-stop.md:69-110` [verified live]
- **The TUI prototype was archived, not refuted.** A separate binary of 48 files and 7,899 lines depending on no product crate, with its own simulation and state file, installed as the operator's `ymp`. The owner rejected the TUI design as based on the old implementation and closed the work; code and evidence were preserved. `legacy-attempt-1:ymp-rust/tools/ymp-tui-prototype/README.md:3-11`, commit 7cdb238 [design]
- **Every invariant shipped "Unenforced".** All eight entries of `legacy-attempt-1:ymp-docs/INVARIANTS.md:12,24,36,48,60,72,85,97` carry that status; hidden checks, object GC, fencing tokens, protected-query budgets, holdout retirement and reviewer blinding were documented and unbuilt. Read every attempt-1 document as a statement of intent, never of delivered behaviour. The gap analysis also records that the product never ran more than one participant.

### 11.1 Journal, object store and snapshot facts

Owner: W1-0001 (envelope and replay), W1-0016 (durable storage), W1-0005 (snapshot and capture). Source unless stated: `legacy-attempt-1:ymp-rust/crates/ymp-domain/src/lib.rs`, `.../ymp-storage/src/{journal,object_store,root,lib}.rs`, `.../ymp-application/src/lib.rs`, `.../ymp-artifacts/src/lib.rs`, `legacy-attempt-1:ymp-rust/SCHEMA.md`. Falsifier for the section: a W1-0016 decision that makes the journal a database rather than an append-only file; then only durable-before-memory, idempotency by content and the recovery rule survive.

- **Envelope.** One JSON record per line carrying `schema_version`, `run_id`, `sequence`, `command_id`, `command_digest`, `predecessor_digest`, `event`, `digest`; the digest is SHA-256 over exactly the preceding fields in that order. `.../ymp-domain/src/lib.rs:304-367`; `SCHEMA.md:361-370` [tested: journal_gap_duplicate_predecessor_and_digest_corruption_are_distinguished]. Falsifier: an independent canonicalization spec — attempt 1 had none, and the digest was canonical only because a struct fixed field order.
- **Four distinct chain-break errors, never one.** Sequence starts at 1 and must equal the physical line number; separate refusals for sequence gap or duplicate, mixed `run_id`, predecessor mismatch, self-digest mismatch. The reader never repairs and never skips. `.../ymp-storage/src/journal.rs:139-189` [tested: journal_gap_duplicate_predecessor_and_digest_corruption_are_distinguished]
- **A truncated tail is its own state.** A file not ending in a newline is an incomplete tail, not corruption; an oversized file or record is refused before parsing. `.../journal.rs:118-147` [tested: incomplete_journal_tail_is_rejected, oversized_records_and_journals_are_rejected_before_parsing]
- **Corruption terminates, and terminates carefully.** The store is refused, original bytes preserved, and the run summary flipped to an infrastructure error only after a successful compatibility check — so a store whose schema version is unreadable takes no lock and is not defaced. `.../ymp-application/src/lib.rs:602-624,2454-2461` [tested: assert_persisted_infrastructure_error]. Falsifier: a repair path; attempt 1 had none, so such a store was a dead end with no export-then-continue route.
- **Bounded journal with a reserved terminal slot.** 64 KiB per record, 16 MiB per journal, 64 KiB reserved so a dying run can always append its own terminal event; ordinary appends use the reduced ceiling, the terminal append the full one. `.../journal.rs:7-26,191-234` [tested: bounded_journal_preserves_space_for_one_terminal_event, journal_capacity_uses_terminal_reserve_and_recovers_as_infrastructure_error]. Falsifier: the numbers — keep the reserve, re-pick the ceiling.
- **Durable before memory, as an ordering law.** Write bytes, flush, sync, then advance the in-memory cursor; the same order one level up, journal append before projection. The summary file is a projection and never a source of truth. `.../journal.rs:236-248`, `.../lib.rs:1786-1830` [tested: an_interrupted_apply_is_recovered_and_the_command_after_it_is_committed]. Cost: one sync per event, no batching.
- **Atomic metadata protocol.** Every small file write is create-new temp, write, sync file, rename, then fsync of the containing directory, with the temp removed on failure; a failed metadata write does not lose the committed command — replay repairs the summary. Root and project markers needed the same staging because in-place writes were read truncated by a concurrent start. `.../lib.rs:2462-2496`, `.../ymp-storage/src/root.rs:402-415`, commit 9399844 [tested: metadata_write_failure_replays_committed_command_and_repairs_summary]. Falsifier: a non-Unix target — directory fsync is Unix-shaped.
- **Recovery after an interrupted commit.** Because the fact is durable before it is applied, a panic leaves memory exactly one fact behind. Recovery compares two integers at the head of every command path and, when they differ, folds only the missing records through the same absorption path a cold restart uses. It writes nothing and re-executes nothing; a repeat of the interrupted command is answered from the record. commit 6544e25; `.../lib.rs:666-771` [tested: an_interrupted_apply_is_recovered_and_the_command_after_it_is_committed, a_recovered_projection_holds_the_candidate_its_journal_records]. Both audits call this the best idea in the attempt and state no falsifier.
- **Idempotency by command id plus content digest.** The same pair returns the recorded outcome marked replayed; the same id with a different digest is a conflict. Checked on the live path and the recovery path alike. `.../lib.rs:670-677,1766-1785` [tested: duplicate_command_is_replayed_and_state_recovers_from_cursor, recovery_rejects_conflicting_command_digest_before_applying_the_event]
- **One trusted writer is one advisory file lock.** An exclusive lock on a writer lock file held for the object's lifetime with the pid written into it; the second process is refused and told which store it lost, never killed. Run-directory claiming is deliberately racy and the lock, not the layout, refuses the loser. `.../ymp-storage/src/lib.rs:28-56`, `root.rs:105-124` [tested: second_writer_is_rejected]. Falsifier: a network filesystem or multi-host deployment — advisory locks survive neither. The board plane held a separate, unrelated lock.
- **Object store naming and dedup.** Lowercase hex SHA-256, path split two characters then sixty-two; a non-canonical digest is refused before touching the filesystem. Writing an object whose target exists re-verifies that target instead of returning early, so a partially written object is reported as a digest mismatch and left untouched rather than silently completed. `.../ymp-storage/src/object_store.rs:32-82` [tested: object_paths_require_canonical_lowercase_sha256, partial_target_is_not_completed_or_accepted]. The audit flags the re-verification as the line most likely to be optimized away.
- **Reads rehash.** Every object read re-digests the bytes before returning them. `.../object_store.rs:55-67` [tested: altered_object_bytes_are_rejected_on_read]
- **The snapshot manifest is itself an object.** Path-sorted entries of relative normalized UTF-8 path, object digest and an executable bit; duplicates rejected; the manifest's own digest is the snapshot identity. `.../ymp-artifacts/src/lib.rs:21-31,341-348,497-525` [tested: forged_path_outside_workspace_is_rejected]. Falsifier for W1-0005: a Snapshot that must carry modes, mtimes, ownership, empty directories or symlinks — attempt 1 preserved none of them and refused symlinks outright.
- **Roots are derived, never typed, and the launch directory receives nothing.** A home root with a marker file, a project segment derived from the launch path, a numeric run ordinal; a directory containing a journal file is refused as a root; another layout version is refused, never migrated; earlier product data beside a project is ignored, never read, copied or deleted. `.../ymp-storage/src/root.rs:10-33,160-263`, commit b084579 [tested: a_store_is_never_opened_as_a_root, a_root_of_another_layout_version_is_refused, earlier_product_data_beside_the_project_is_ignored_unchanged_and_never_copied]. Adopt the launch-directory rule as-is; the path-hash project segment and the 9,999-run ordinal are in 11.12.
- **No generic storage-root accessor.** The generic accessors were deleted and replaced by two typed child paths that validate their identifier down to exactly one normal path component, with the boundary pinned by compile-fail doctests. commits 40315e2, 427c87a; `.../ymp-application/src/lib.rs:497-530` [tested: attempt_paths_are_concrete_children_and_refuse_storage_traversal]. This is the right shape for a WorkspaceGuard hold.

### 11.2 Isolated candidates and apply-back

Owner: W4-0001 (isolated copies and merge through WorkspaceGuard), W1-0008 (immutable result versions), W4-0003 (selection and recheck), W1-0013 (evidence-audited delivery). Source unless stated: `legacy-attempt-1:ymp-rust/crates/ymp-artifacts/src/lib.rs` and `.../ymp-application/src/{lib,answer,participant}.rs`. Falsifier for the section: a W4-0001 copy-on-write provider that shares bytes with the base; then the materialization costs below do not apply, but the exclusion, submission and apply-back rules still do.

- **The private copy is a full materialization from the content store.** Not a worktree, not an overlay: capture the source into the object store, then write each file into an empty destination with create-new, sync and directory fsync. Git is not a dependency anywhere on this path. `.../ymp-artifacts/src/lib.rs:111-180`, `.../ymp-application/src/participant.rs:452-466` [tested: private_workspace_produces_reproducible_immutable_candidate]. Cost: whole-file read and write per entry, no reflink or hardlink path.
- **Exclusions are two-layered and symlinks are refused.** Structural, at the source root only, so a product directory name nested under `fixtures/` stays ordinary content; plus declared derived subtrees from the contract. A symlink at capture is a hard error, never a skip. `.../ymp-artifacts/src/lib.rs:309-326` [tested: source_capture_omits_product_roots_without_omitting_nested_project_names, source_symlink_is_rejected]. Attempt 1 shipped two divergent exclusion lists for one concept; that is a defect, not a pattern.
- **Excluded subtrees are verified, not ignored.** Before a submission is formed, every base file under an exclusion must still match byte for byte and be no symlink. "Excluded" means "may not be touched", not "may be anything", so an agent cannot hide changes behind an exclusion. `.../ymp-artifacts/src/lib.rs:364-454` [tested: submission_rejects_changes_to_base_files_inside_excluded_subtrees, submission_rejects_new_derived_files_without_storing_them]. Cost: a full rescan per submission.
- **The submission is deterministic provenance.** A base snapshot digest plus an ordered change list of upserts and deletes, replayed over the exact base with every object re-verified; a wrong base is refused, a delete of an absent path is refused, and rebuilding from the same submission yields the identical candidate identity. `.../ymp-artifacts/src/lib.rs:182-297` [tested: stale_base_is_rejected_without_candidate_creation, forged_delete_and_unknown_schema_are_rejected]. There is no merge or rebase; a stale base is simply refused.
- **Candidate immutability is enforced on identity.** One run holds at most one candidate identity of base digest plus tree digest; a second, different submission is a conflict, refused identically on the live path and the recovery path. `.../ymp-application/src/lib.rs:679-691,2390-2404` [tested: conflicting_integration_cannot_replace_an_existing_candidate, identical_candidate_tree_from_a_different_base_is_rejected]. This is R-16 as code.
- **The capture/draft asymmetry is the defect to fix.** The negative-control draft copier refuses at 20,000 entries or 256 MiB and skips a wide directory list; the real capture path has no limit at all and reads each file whole into memory. `.../ymp-application/src/answer.rs:546-560` against `.../ymp-artifacts/src/lib.rs:330` [design]. Falsifier: a W4-0001 bound on capture with a stated refusal.
- **Apply-back examines the whole path first and reports partial application honestly.** Only an accepted result applies. Every target and every ancestor component is examined before the first write — a file where a directory is needed, a directory where a file is named, a symlink on the way — because the OS reports the same error for "missing" and "parent is not a directory". All conflicts are named at once and nothing is written; files are then staged and renamed in, and if a rename fails mid-way the failure names the files already in the project rather than claiming nothing happened. commit bf28efe; `.../ymp-application/src/lib.rs:1651-1765` [tested: a_file_where_the_candidate_needs_a_directory_stops_the_application_before_it_begins, a_symbolic_link_on_the_way_stops_the_application_and_nothing_leaves_the_project, an_application_stopped_part_way_names_the_files_it_already_wrote]. Honesty replaced atomicity and deletes were never applied. Falsifier: a W4-0003 decision for transactional merge with rollback.
- **The export bundle is a whole-directory rename.** Built under a temporary name, fsynced and renamed; it carries `state.json`, `events.jsonl`, `candidate-manifest.json`, `manifest.json` and the `candidate/`, `evidence/`, `environments/` and `runtime-evidence/` directories, with every evidence and environment object rehashed before inclusion and an existing destination refused. `.../ymp-application/src/lib.rs:1537-1649` [design]. Owner: W1-0013. Falsifier: a report format that signs the bundle — attempt 1 signed nothing.

### 11.3 Verification facts

Owner: W1-0009 (check runs against immutable snapshots), W1-0019 (evidence and reviews bound to results), W1-0010 (acceptance grades), W2-0001 (hidden checks), W2-0002 (execution environments), W2-0004 (discrimination). Source unless stated: `legacy-attempt-1:ymp-rust/crates/ymp-verifier/src/lib.rs` and `.../ymp-application/src/{verification,contract,answer}.rs`. Falsifier for the section: an OS release that changes Linux user namespaces, or a W2-0002 container runner replacing the process-level scheme; the control, evidence-binding and undecided rules survive either.

- **Checks are declared only in the approved contract object.** Program path, arguments, negative-control directory, oracle digest, wall-time and output limits all come from the stored contract; on use its bytes are re-read and re-digested against the run's recorded binding, and a contract naming a different oracle is refused. `.../ymp-application/src/verification.rs:101-164`, `contract.rs:59-86` [tested: only_bound_verifier_evidence_can_accept_a_candidate]. Limit: one program and one exit code, no per-criterion granularity — exactly what W1-0009 must exceed.
- **The negative control runs first and must fail.** Exit 1 proceeds; exit 0 or any other code is an infrastructure error, never a rejection, because a non-discriminating oracle decides nothing. A contract cannot be prepared without a negative-control directory. `.../ymp-verifier/src/lib.rs:397-409`, `legacy-attempt-1:ymp-docs/PROJECT-CONTRACT.md:114-117` [tested: bounded_command_requires_a_failing_negative_control]. Attempt 1 allowed exactly one control per run and it was directory-shaped.
- **Per-observation isolation roots.** Each observation gets its own temporary root and a freshly copied subject tree, and the oracle is re-copied read-only per observation, so the control cannot seed or modify the candidate's observation and two observations cannot exchange state through the outer root. `.../ymp-verifier/src/lib.rs:428-476` [tested: negative_control_cannot_seed_candidate_observation_through_working_directory, negative_control_cannot_modify_the_oracle_used_for_the_candidate, observations_cannot_exchange_state_through_the_outer_root]
- **Oracle integrity is checked three times.** Digested when the environment is bound, re-checked before and after copying into the isolation root, then made read-only on file and directory; a mismatch is a typed program-digest failure. `.../ymp-verifier/src/lib.rs:619-660` [tested: exact_digest_evidence_is_bound_to_candidate_contract_and_oracle]
- **Independence was structural, never a label.** Attempt 1 had no independence taxonomy in code. Independence was produced mechanically: a separate process; a cleared environment reduced to language, locale and search path plus a private home and temporary directory; on Linux new user, mount, pid, network, ipc and uts namespaces with a chroot, no-new-privileges and read-only binds; elsewhere a named copy-per-observation profile. The profile string is recorded in the environment object, so evidence states which regime produced it. `.../ymp-verifier/src/lib.rs:509-530,594-602,662-734` [tested: observations_use_disposable_mount_pid_and_network_namespaces, launcher_protocol_distinguishes_isolation_failures_from_oracle_exit_one, oracle_exec_failure_is_an_isolation_failure]. The code disclaims containment of hostile code on the outer host (`lib.rs:669-671`). The model's four-way independence enum is the improvement; W1-0019 owns recording the regime as a field, not an implication.
- **Evidence stores digests, not bytes.** Output streams are hashed with byte counts and never retained; the evidence object names candidate, contract, oracle and environment digests plus a profile, an observation digest and a decision. `.../ymp-verifier/src/lib.rs:123-169,287-295` [tested: explicit_environment_binding_survives_serialization_and_detects_substitution]. Both audits call this a loss: an operator cannot be shown why a check failed. W1-0019 should retain bounded output bytes, not only their digest.
- **Evidence is unforgeable by type, not by convention.** The verified-evidence value has private fields and is constructible only inside the verifier; the read-back type deliberately cannot be passed to the recording boundary, and a legacy unbound evidence object is an infrastructure error rather than a rejection. `.../ymp-verifier/src/lib.rs:28-54,89-96` [tested: stored_evidence_reader_preserves_typed_legacy_and_invalid_data_errors, ambiguous_version_one_evidence_blocks_recovery_as_infrastructure_error]. The audit calls this the strongest pattern in the codebase; carry the shape, not the names.
- **Undecided is never rejected.** A verifier that cannot be built, cannot run, times out, exceeds the output limit, or answers about a different candidate produces a typed infrastructure failure. `.../ymp-application/src/verification.rs:41-98,166-187` [tested: unavailable_namespace_launcher_is_not_an_oracle_rejection]. Attempt 1 then died rather than retrying; W1-0009 and W1-0012 own the retry question.
- **Discrimination was demonstrated before a contract existed.** The production executor ran the proposed program with the negative control as both control and subject, requiring rejection twice, and stored nothing. Substitution controls were auto-generated: a candidate that rewrote the test entry point, and one per filename the shell would read in preference to it. All had to be rejected or the proposal was not offered. `.../ymp-application/src/answer.rs:215-300,520-660` [tested in `tests/generated_check.rs`, `tests/fixed_test_entry_point.rs`]. The documents admit the positive half was missing: what the verifier accepts is stated as undemonstrated. Owner: W2-0004.
- **Human and agent judgement were never in the automatic path.** The oracle decision is the only thing the journal records; board assessments, interventions and verdicts live in a separate section the journal never names. The protocol required a reviewer to commit an assessment before seeing rationale, votes or reputation, because language-model errors can be strongly correlated, and stated that a hidden *requirement* is invalid even if approved — only instances of public requirements may be hidden. `.../ymp-application/src/lib.rs:369-372`, `legacy-attempt-1:ymp-docs/PROJECT-CONTRACT.md:26-30,81-93,120-143` [design; unenforced]. Owner: W2-0001 (visibility as a type), W1-0010 (R-6, R-13).
- **Adaptive-feedback erosion was recognised and unbuilt.** A frozen hidden suite "becomes less independent with each query"; the contract demanded a protected-query budget, minimal disclosure, a fresh-seed policy, holdout retirement and blinding, with the kernel never choosing what to test and a failed query neither revealing raw output nor funding a retry. `legacy-attempt-1:ymp-docs/PROJECT-CONTRACT.md:131-143` [design]. Owner: W2-0001.

### 11.4 Commitment, lease, recruitment and admission facts

Owner: W1-0006 (atomic admission with grants and reservations), W1-0007 (lease, release, delegation), W3-0003 (solicitation, offers, award), W3-0002 (team change), W1-0004 (Treasury). Source unless stated: `legacy-attempt-1:ymp-rust/crates/ymp-domain/src/commitment/{ledger,records,budget,protocol,invocations}.rs` and `.../ymp-domain/src/recruitment.rs`. Falsifier for the section: attempt 1 never ran more than one participant, so every claim here is about the kernel's algebra and none is about multi-agent behaviour.

- **Contract formation is one indivisible step.** A compatible live bid plus a sponsor award emits the contract, the obligation, the lease and the escrow transfer together, or nothing at all; deciding is a pure function and a refused command leaves the ledger byte for byte unchanged. `.../commitment/ledger.rs:1184` [tested: a_compatible_offer_and_bid_form_one_contract_escrow_lease_and_obligation_together, an_incompatible_or_expired_record_forms_nothing_at_all]. This is W1-0006's award-then-admit chain under other names.
- **State sets were richer, and one state was deliberately absent.** Offer advertised/withdrawn/settled; bid live/withdrawn/awarded; contract active/returned/cancelled; obligation active/terminal; attempt running/closed; invocation running/yielded/closed; terminal outcomes result, dead end, declined, exhausted, cancelled, infrastructure error. There is no expired *contract* state — expiry is a lease property. A failed or empty return still closes causal work and never makes the parent successful. `.../commitment/records.rs:45-103` [design]. The model does carry `Expired` as a commitment state; W1-0007 must decide what entering it authorizes.
- **A lease is a fencing token, not a deadline on the work.** It carries a holder and a generation, every state-changing command repeats the generation, and a stale one is refused by name. `.../commitment/records.rs:157-165`, `ledger.rs:2270` [tested: a_stale_fencing_generation_cannot_submit_renew_or_close_after_reassignment, removing_the_fencing_check_produces_a_counterexample]
- **What expiry proves.** Only that the wall time the escrow bought ran out — not that the holder died, the work failed, or that anyone else may act. Expiry emits no event and changes no state by itself: it blocks the holder and unlocks the sponsor's reassignment, which is refused while the lease is still live. `.../ledger.rs:1685,2286` [tested: a_stale_fencing_generation_cannot_submit_renew_or_close_after_reassignment]. The clock was an explicit monotone command, never a host clock read.
- **Renewal is holder-only, current-generation-only, and only while the lease is live** — a lapsed holder cannot renew itself back into authority, and renewal charges wall time from the contract's own escrow. `.../ledger.rs:1372` [tested: only_the_sponsor_awards_and_only_the_holder_advances]. Attempt 1 bounded renewal by escrow and a maximum lease length only; the model's renewal counter is a second, different limit, and W1-0007 must say which a refusal names.
- **Reassignment is re-award, never transfer.** It requires the sponsor, a lapsed lease and a fresh live bid on the same offer with a matching artifact class; it bumps the generation and issues a new lease. Release of capacity is settlement: unspent escrow returns to the funding account and a closed contract stops being an account at all. `.../ledger.rs:1685,2167` [tested: a_reservation_returns_to_a_contract_that_has_changed_hands, a_settlement_into_a_task_contract_that_has_closed_is_refused_and_changes_nothing]. No consent-free handover existed.
- **Closing runs leaves inward.** A return requires the lease holder — not merely the current generation — every descendant obligation terminal, every parented offer settled and every outstanding reservation back. `.../ledger.rs:1759,2197,2230` [tested: an_obligation_cannot_close_while_the_work_below_it_is_outstanding, a_participant_that_does_not_hold_the_lease_cannot_close_the_obligation, a_task_contract_cannot_close_while_a_reservation_is_still_due_back_to_it]. Obligation parentage was a tree while dependencies were a graph.
- **A ten-dimension integer budget in two kinds.** Capacity dimensions (money, tokens, wall time, verification queries, external actions) settle back; creation authority (participant, attempt, invocation, offer and obligation starts) is irreversible once spent, which is what makes recursive decomposition finite. No dimension ever pays for another and a refusal names the exact dimension. `.../commitment/budget.rs:14-95` [tested: a_dimension_is_never_paid_for_out_of_another, a_debit_the_accounts_cannot_cover_is_refused_and_named_in_the_result]. Owner: W1-0004. Falsifier: a W1-0004 decision that one cost unit with reserves suffices; then only "no dimension pays for another" survives.
- **Yield and wake were bounded three ways.** At most 16 typed wake conditions, a wake deadline that may not outlive the funded lease, at most 8 wakes per attempt. Resumption is the only transition that reads committed facts and it records the exact sequence number that authorized the wake; cursors move forward only and never past what is committed. `.../commitment/protocol.rs:20-30,215-246`, `ledger.rs:1934` [tested: a_yielded_slice_has_no_process_and_resumes_only_on_a_committed_fact, duplicate_coalesced_and_lost_notifications_change_nothing, a_cursor_only_moves_forward_into_facts_that_exist, an_expired_wake_stops_holding_the_run_open]
- **The switchable-invariant testing method.** Sixteen named invariant checks in the ledger can each be switched off in a test build only, and a generated-schedule sweep then produces the counterexample; eleven tests are named for the check they remove. The invariant is proved load-bearing instead of asserted in prose. `.../ledger.rs:56-110` [tested: removing_the_fencing_check_produces_a_counterexample and ten siblings]. Cost: it doubles the kernel's test surface. See 11.11.
- **The kernel never picks a winner.** No transition ranks, scores or compares two participants; an open offer is serialized to its funded count, and disagreement between candidates is recorded as evidence and becomes work to fund, not a decision. `.../ledger.rs:8-12`, `protocol.rs:177` [tested: the_kernel_never_chooses_between_two_live_bids, the_kernel_names_no_winner_between_two_results, a_disagreement_becomes_evidence_and_work_rather_than_a_decision, no_transition_reads_an_opaque_payload_beyond_its_identity]. This is R-1 as code.
- **Acceptance is never inferred.** Quiescence, a spent budget and an unverified result are not acceptance, and a verdict that passed below the root scope is not acceptance of the run. `.../commitment/invocations.rs`, `ledger.rs:535-560` [tested: quiescence_a_spent_budget_and_an_unverified_result_are_not_acceptance, a_query_that_passed_below_the_root_is_not_acceptance_of_the_run, overstating_the_scope_of_a_verdict_produces_acceptance_without_evidence]. Owner: W1-0010.
- **A recruitment request carries no preference field.** Three identifiers and nothing else — no role, rank, priority, reason or quantity — "because a command that could carry one would be a command a transition could read one out of". The admitted identity, principal and workspace are derived from the request, so a repeat names the same participant. `.../recruitment.rs:41-107` [tested: the_request_carries_no_field_a_decision_could_read_a_preference_out_of, a_repeated_request_is_refused_as_a_duplicate]. Owner: W3-0002.
- **Five gates in a declared order.** Frozen membership, participant starts, concurrency, runtime admission, offer-stage charge — preceded by validation, run-creates, proposer-is-live and not-repeated. A request failing several is refused by the first, so the constraint an operator reads is a property of the request and not of check ordering, and no refusal proposes a substitute entry. `.../recruitment.rs:145-170,490-530` [tested: a_request_failing_several_gates_is_refused_by_the_first_in_the_declared_order, every_gate_refuses_in_the_words_of_the_constraint_that_stopped_it, the_gate_names_no_entry_the_request_did_not]
- **Only a live participant proposes.** Live means holding a running invocation the kernel committed, or being the run's root while holding no slice at all. A participant admitted and never started exists in the accounts and runs nowhere; the earlier code read its empty slice set as "has not begun one" and let it recruit. The refusal now names the state. commit 57f8b9b; `.../recruitment.rs:580-620` [tested: a_proposer_that_is_not_running_proposes_nothing, a_participant_that_was_admitted_and_never_started_proposes_nothing, unbound_and_yielded_agent_sessions_recruit_nobody]
- **Admission is not a start.** The decision changes nothing; the admission is handed to a single start path — the same one the origin participant uses, so there is no second start path to keep in step. A failed start leaves the admission standing and the authority spent; it is not a refusal, and attempt 1 did not refund it. `.../recruitment.rs:373-400` [tested: a_failed_start_leaves_the_admission_it_was_given_standing, one_request_admits_one_participant_records_it_charges_it_and_starts_it]. Owner: W1-0006.
- **The proposer pays, and parentage confers no authority.** The newcomer's endowment came out of the proposer's own balance and was enough to read an offer and answer it, never to do work; money, tokens and time reach a participant only through the escrow of a contract it actually took on. "A proposer is not a manager." `.../recruitment.rs:110-137,343-360` [tested: an_admission_spends_the_authority_it_used_and_funds_the_participant_it_admitted]. The ceiling of six participants was an owner placeholder, never measured.
- **Affordability is checked before any write.** A run with no attempt left used to be refused by the domain at commit time — after the private copy was made, after the kernel opened and the participant registered — so the journal recorded a run that paid for a participant it never got. commit 4a6e8a0; `.../ymp-application/src/participant.rs:1-38` [tested: acceptance suite over the journal file]. The check is duplicated in the application and the domain by design.

### 11.5 Board facts

Owner: W3-0001 (grant-scoped team operations and addressed board projections), with W1-0019 and W5-0001 for the blinded-review half. Source unless stated: `legacy-attempt-1:ymp-rust/crates/ymp-board/src/{lib,records,budget,store,observatory}.rs`. Falsifier for the section: a W3-0001 board that carries executable proposals rather than references; then inertness must be re-established some other way.

- **Inert by construction, not by rule.** A payload never enters the board crate: the caller hashes bytes and the command carries a digest and bytes only, so there is no field that could hold a capability token, command, URL or instruction. The crate depends on no other product package, so it cannot form a contract, move escrow or record a verdict — adding that would be a visible manifest change. `.../ymp-board/src/lib.rs:1-40` [tested: a_payload_is_inert_whatever_it_states, the_collaboration_plane_depends_on_no_other_ymp_package, the_collaboration_kernel_reaches_no_file_no_address_and_no_process, every_fact_a_payload_can_produce_is_a_record_and_never_an_action]. This is R-21 as a dependency graph.
- **An uninhabited authority type.** The type that would let a board record keep a run alive has no variants, and that absence is the statement: granting the board such power would require adding a public variant. `.../ymp-board/src/records.rs:28-40` [tested: nothing_a_payload_states_keeps_a_run_alive]. The audit calls it the cleanest inertness proof in the attempt.
- **A typed audience that narrows and never widens.** Project-wide discovery is open to registered participants and bounded to 512 bytes and 4 references so it stays a summary; scoped and named audiences narrow, and a named recipient must already hold a grant. Read and publish rights are separate, and grants expire and are never renewed in place. `.../ymp-board/src/records.rs:80-150` [tested: a_detailed_message_reaches_the_admitted_and_no_one_else, a_named_audience_narrows_and_never_widens, a_grant_on_one_scope_exposes_no_other, an_expired_grant_admits_nothing, reading_a_scope_is_not_publishing_to_it]
- **A separate talk allowance.** Six dimensions — publications, salience refreshes, membership grants, active memberships, published bytes, delivered bytes — deliberately not the control-plane budget, so unspent conversation cannot pay for execution. Delivered bytes are charged to the reader, because a read is what makes bytes cross. `.../ymp-board/src/budget.rs:1-90` [tested: publishing_refreshing_admitting_and_receiving_each_spend_their_own_dimension, capacity_is_conserved_across_admission_release_and_expiry]. See 11.11.
- **History and attention are separate.** The message record is permanent, only its salience expires, and a refresh is a new attributed message at a new cost that rewrites nothing. Relations are standalone, reply, challenge, revision and refresh; a challenge settles nothing and an unresolved disagreement stays visible. `.../ymp-board/src/records.rs:200-260` [tested: the_audit_record_outlives_the_salience_it_was_published_with, a_refresh_buys_salience_again_and_rewrites_nothing]. The model's P4 resolves objections with counterexamples; that is a different design and W3-0001 owns the difference.
- **A single-writer durable section checked against the recorded chain.** Three ways the audit could be lost were closed at once: a directory holding a fact record but no opening terms is refused by name instead of being treated as a fresh section; the section is held by one writer through an exclusive lock taken at open, because two writers each continued the sequence from where they last looked; and an append is checked by recomputing the digest at the last recorded position, not merely against length, so another board's facts cannot continue this sequence. commit c094be0; `.../ymp-board/src/store.rs:1-35,91-160` [tested: a_fact_record_without_opening_terms_is_refused_and_left_as_it_was, a_board_section_is_written_by_one_holder, a_foreign_board_is_refused_at_the_recorded_position]
- **The operator projection takes part in nothing, and causal labels are gated.** The operator view is unbounded by audience because the operator is not a board participant; a participant view carries only what that participant may read. An edge is labelled causal only where a controlled intervention record supports it, replicated at least twice under a matched budget; order and citation alone never produce one, and a view claiming otherwise fails its own check. A projection bug that dropped the exact audience was fixed by carrying it verbatim. `.../ymp-board/src/observatory.rs:1-32,186`, commit aa0ef50 [tested: order_and_citation_alone_never_produce_a_causal_label, a_reading_that_labels_an_association_causal_fails_the_check, the_operator_reading_copies_each_exact_owned_audience_in_publication_order]
- **Agents reached the board through tools, not files.** A stdio tool binding to an invocation-bound private socket, bound by the controller to one principal, participant, attempt and generation. Arguments refuse unknown fields, so a model cannot smuggle a reader, author, payload digest, capability, URL or protected-oracle field; reads are cursor-based and byte-bounded so no agent can pull the whole board. `.../ymp-agent-api/src/lib.rs:9-110`, `legacy-attempt-1:ymp-docs/PROTOCOL.md` [tested: bound_tools_derive_identity_compute_payload_and_replay_no_effect]
- **Blinding was the implemented half of independence.** A reviewer commits an assessment digest while board content, producer rationale and prior votes are withheld; only that first commitment is primary, everything after disclosure is secondary and stays attributable, and a majority vote never accepts a candidate. `.../ymp-board/src/records.rs`, `legacy-attempt-1:ymp-docs/REPUTATION.md` [tested: nothing_reaches_a_reviewer_before_its_own_assessment_is_durable, only_the_first_assessment_under_blinding_is_primary, a_prior_vote_does_not_reach_a_reviewer_that_has_not_committed]. Owner: W1-0019 (R-4), W5-0001.

### 11.6 Registry and readiness facts

Owner: W1-0003 (Registry, execution profiles, readiness), with W3-0009 and W6-0005 for comparability. Source unless stated: `legacy-attempt-1:ymp-rust/crates/ymp-runtime-registry/src/{lib,provider,catalog,pool}.rs` and `.../ymp-domain/src/pool.rs`. Falsifier for the section: a provider whose installed build exposes a machine-readable model list with a stable identity; then measurement-by-asking is replaceable, but the staleness rule is not.

- **Four levels, each recording and none deciding above itself.** The engine record's enable flag and reason is the only field never measured — executable, release, credential source, bounds and model list are written from a measurement. The provider record is observed and never seeded, so a root with nothing enabled holds no provider record at all. The catalog is derived at read time and never stored, because a stored union goes stale while reading as current. The pool separates the operator's declaration from the controller's resolution, neither writer able to reach the other's half. `.../runtime-registry/src/lib.rs:1-45`, `provider.rs:1-58`, `catalog.rs:1-38`, `pool.rs:1-72` [design]
- **Absence is stated, never silently shortened.** An inadmissible entry stays in the catalog marked with the measured reason, "because a surface that showed only what is usable would answer *why is this model not offered* with silence", and an engine serving no models produces a route row marked as serving none, so a catalog that lost a whole engine cannot be read as complete. `.../runtime-registry/src/catalog.rs:16-30,82-100` [design]. This is W1-0003's "visible as unknown, distinct from requested and sent".
- **The frozen launch snapshot, held by value.** A run holds every permitted entry in declared order with what admission measured, the origin entry and the ordered set's digest; permission is read only from the snapshot. A provider disabled, an entry removed or a model discovered after creation belongs to the next run, and two runs naming the same digest were created against the same capability boundary — which is what makes a matched-budget comparison reproducible. A pool with no live entry is refused at creation, not discovered later, and validation re-derives the origin and refuses a record whose origin was written by anything but the rule. `.../ymp-domain/src/pool.rs:1-210` [tested: a_created_run_carries_the_pool_it_was_created_under, what_changes_after_the_freeze_belongs_to_the_next_run, a_root_that_offers_nothing_creates_no_run_and_says_so_in_plain_words]. Falsifier: a W3-0009 comparison that pins the profile per assignment instead; until then, reading a live Registry by reference can move an assignment onto another model mid-run.
- **Enable and disable are resolved in exactly one place.** Two escapes were closed: a disabled engine was startable by naming a store inside the root, so the decision was read from an empty registry and a real process started; and a root the operator named was passed through store derivation, writing the decision where nothing read it. A root and a store are now different arguments, the search starts at the named path and stops at the first root, and an enable or disable reply names the file it wrote. commits 7fedc7a, 5776b5d [tested in `ymp-rust/crates/ymp-cli/tests/engine_registry.rs`]. Lesson: a policy flag is only as strong as the single place that resolves which record it lives in.
- **Measurement, not declaration, decides what exists; the digest decides staleness.** A hand-written candidate filter silently discarded models the installed build serves (`claude-3-5-haiku-latest` among them); putting every identifier the executable carries to the build and letting the build refuse grew the measured catalog from 24 to 55 routes, and a measurement that left candidates unasked now says so rather than passing for the whole catalog. Separately, a recorded list declared itself current by naming a release and so suppressed its own re-measurement; staleness is now the digest of the installed executable recomputed from its bytes at every reading, and the release is stated for a reader and decides nothing. commit 6f89c17; `.../runtime-registry/src/catalog.rs:74-81` [measured: 24 to 55 routes]
- **Four readiness-measurement defects, each fixed.** (a) The engines page measured every admitted engine on every read, so a fresh root started a provider CLI before any account was enabled — the very thing the provider level exists to hold back; measurement now runs only for enabled accounts and says nothing was measured rather than reporting one that is not running (72b288b). (b) An operator decision was read as absence of measurement, printing "nothing has been measured" beside a four-minute-old observation; a record now answers four separate questions about when it was observed and age errs toward staleness (fb961cf). (c) A measurement landing after the operator held an account back restored that account's readiness, so routing could send the next run to an account whose disclosure permission had just been withdrawn; the landing now re-reads the engines of every account whose permission moved (4cd0bee). (d) Process membership of a measurement is recorded while it can still be read, never recomputed from parent and group links the OS keeps only while both ends live, because a reparented engine outlived the session and the quit reported an ending it had not established (4fff141). [tested: an_account_held_back_during_a_measurement_stays_held_back, quitting_during_a_long_measurement_leaves_no_engine]. Lesson: the operator's decision outranks any measurement taken before it, and "we ended it" must be observed.
- **An observed version is evidence, never an acceptance predicate.** Both drivers ran a version query on the admitted copy; the observed version was removed from the identity-equality check, leaving compatibility decided by contract digest, executable digest and transport digest, with the version retained in the trace where it may not authorize or refuse. commit 777dfd5; `.../ymp-application/src/tool_host_probe.rs:820-841` [tested: observed_runtime_version_is_evidence_not_attestation_authority, version_difference_is_evidence_and_never_an_acceptance_predicate]. Attempt 1 pinned a Claude Code version floor and deliberately refused to pin Codex; W1-0003 needs one rule, not two.
- **Executable admission by copy and digest.** The resolved binary was byte-copied into a private temporary directory, made read-and-execute only, and executed from the copy; its SHA-256 was the identity, and the whole launch chain was re-verified immediately before and after the child was created, so a substitution mid-launch terminates before any event is accepted. `.../ymp-runtime-codex/src/lib.rs:458-488`, `.../ymp-runtime-claude/src/lib.rs:424,1105,1162` [tested: every_admitted_launch_chain_program_executes_and_refuses_replacement, admitted_runtime_bytes_execute_after_source_path_replacement]

### 11.7 Runtime and supervision facts

Owner: W1-0017 (execution host), W1-0018 (Codex backend), W6-0001 (Claude backend), W3-0001 (team-operation transport, R-21), W1-0004 (receipts). Source unless stated: `legacy-attempt-1:ymp-rust/crates/ymp-runtime-api/src/lib.rs`, `.../ymp-runtime-supervisor/src/lib.rs`, `.../ymp-runtime-codex/src/lib.rs`, `.../ymp-runtime-claude/src/lib.rs`, `.../ymp-agent-{api,mcp,rpc}/src/lib.rs`. Falsifier for the section: a provider release whose CLI surface differs; check the installed binary before relying on any flag or field name here.

- **Codex ran as a CLI JSON-lines process, not an app server.** The launch was `exec --json --ignore-user-config --ignore-rules --sandbox workspace-write --model <m>` with reasoning effort, approval policy and environment inheritance as `-c` settings, a working-directory flag, 32 feature-disable flags, optional `resume <id>`, and the prompt on stdin. The app-server surface was touched only as a compatibility probe (`app-server --help`, `generate-json-schema`), never as transport; the stated reason is that one bounded non-interactive turn per process is the portable baseline. `.../ymp-runtime-codex/src/lib.rs:962-999,365-392`, `legacy-attempt-1:ymp-docs/ARCHITECTURE.md:290-297` [tested: prepared_launch_matches_actual_process_and_rejects_all_mutations, structured_process_stream_preserves_session_output_and_usage]. Attempts 2 and 4 both chose the app-server JSON-RPC path instead (section 2 holds attempt 4's protocol facts); attempt 1's only live run failed and was never localized, so nothing here decides between them. Owner: W1-0018, which picks the wire protocol from the installed build.
- **Codex event vocabulary, and an unknown event type was fatal.** Parsed: `thread.started` with `thread_id`; `item.completed` with item types `agent_message`, `mcp_tool_call` (`server`, `tool`, `status`, `arguments`, `result`, `error`) and `reasoning`; `turn.started`; `turn.completed`; `item.started`; `item.updated`; `error` and `turn.failed`. Any other type aborted the run as a protocol failure, and raw stderr never escaped — only a bounded summary of bytes, hash and truncation flag. `.../ymp-runtime-codex/src/lib.rs:1726-1861` [tested: changed_tool_event_and_usage_schemas_fail_for_their_own_reason]. The audit judges fatal-on-unknown a mistake: it couples run survival to the provider's release cadence for no safety gain.
- **Codex usage fields.** Four integers required per turn record: `input_tokens`, `cached_input_tokens`, `output_tokens`, `reasoning_output_tokens`. Optional: `cost_microusd`, `cost_by_model`, an in-flight excess object, `protected_queries`. A cost with no per-model breakdown was treated as unverified. `.../ymp-runtime-codex/src/lib.rs:1641-1650`, `.../ymp-runtime-api/src/lib.rs:869-958` [tested: terminal_outcomes_preserve_available_accounting]. Owner: W1-0004.
- **Codex continuation re-spawned a whole process.** Resume prepended `resume <thread_id>`; the session id came from `thread.started`, a changed id on resume was fatal, and resume was refused unless the session was yielded or had a recoverable process failure. `.../ymp-runtime-codex/src/lib.rs:1960-1985` [tested: recoverable_process_failure_resumes_the_same_managed_session, unknown_session_reported_by_native_resume_is_terminal]
- **Claude Code ran as a printing stream-json process.** The launch was `--print --input-format text --output-format stream-json --verbose --setting-sources "" --disable-slash-commands --strict-mcp-config --mcp-config <inline JSON> --model <m> --effort low --permission-mode acceptEdits --max-budget-usd <n> --tools <list>` with an optional allowed-tools list and resume id, the prompt on stdin and stdin then closed. No permission-skipping flag was ever passed. `.../ymp-runtime-claude/src/lib.rs:1017-1050` [tested: only_the_approved_environment_and_search_path_reach_the_child_and_its_record]. The reason for the CLI over an SDK bridge was the provider-enforced cost ceiling; the audit notes that ceiling is per-process and unreconcilable with a Treasury allowance spanning turns. Owner: W6-0001.
- **Init was validated as an isolation assertion.** The init line had to carry the measured version, the pinned model and permission mode, empty slash-command, plugin and skill lists, and one connected product MCP server; the final result subtype had to be success, and a non-null `parent_tool_use_id` — subagent output — was refused outright. `.../ymp-runtime-claude/src/lib.rs:1720-1808` [tested: an_ambient_or_delegating_session_is_rejected_before_model_use]
- **Claude usage fields.** Input plus cache-creation tokens as input; cache-read tokens as cached; output tokens; thinking tokens as reasoning; total cost converted to microUSD; per-model cost from the model-usage map. Turn count and duration were deliberately not read — wall time was measured by the host. `.../ymp-runtime-claude/src/lib.rs:1610-1643` [tested: structured_stream_preserves_session_output_usage_and_cost, a_cost_its_model_breakdown_does_not_support_is_refused]
- **Provider quirks recorded from live observation.** One model identifier was refused with HTTP 400 on a consumer account and the route was pinned to another; Claude Code abandons unfed stdin a few seconds after start, so the prompt had to be written before the post-launch digest recheck; the Claude model catalogue was probed by asking for a printed turn with an empty prompt and matching the "not a model this version recognizes" reply. `.../ymp-runtime-codex/src/lib.rs:44-48`, `.../ymp-runtime-claude/src/lib.rs:530-533,1139-1142` [verified live]
- **The fd-9 launch marker.** Children were not executed directly: a shell preamble opened a marker file on descriptor 9 and then executed the target through an environment-clearing helper. Descriptor 9 is inherited across every fork and preserved across every exec, so it survives session detachment, reparenting to init, a working-directory change and closed standard streams — identifying a run's descendants when nothing the OS reports still links them. `.../ymp-runtime-api/src/lib.rs:2087-2160` [tested in `descendant_termination.rs`; a_descendant_does_not_survive_a_failed_parent]. Unix only. Keep the fact, not the carrier.
- **The termination ladder, exactly.** SIGTERM to the process group and individually to every marker-attributed survivor; 20 polls of 10 ms, that is 200 ms of grace; re-read the holders; SIGKILL to both; 100 polls of 10 ms, that is 1 s of confirmation; otherwise a timeout error naming how many managed processes and which group survived SIGKILL. No SIGINT anywhere, and group signalling alone was explicitly judged insufficient. `.../ymp-runtime-api/src/lib.rs:2603-2731` [tested: timeout_terminates_the_runtime_process_group, cancellation_token_interrupts_a_blocked_runtime_tree]. Signals were sent by an admitted utility, so an unrunnable utility is an error and never read as "the group is gone".
- **Honest termination recording.** Sites that end a process tree while already reporting something else — cancellation, timeout, drop — have no caller to fail to, so the reason is parked and later drained by the supervisor worker and the CLI before the run is reported. A run may say it left nothing running only where absence was measured. `.../ymp-runtime-api/src/lib.rs:2632-2663`, `.../ymp-cli/src/internal.rs:206-219` [tested: what_the_record_would_not_take_is_reported_with_the_run]. The carrier was a process-global destructively drained list; the obligation is right, the carrier is not.
- **An unreadable answer is not an empty answer.** The descendant reader treats exit 1 with output, exit 2, exit 0 with no output, unparsable pids and death by signal as errors, not as "no holders", because reading any of them as an empty set would end a run on an answer nobody gave. `.../ymp-runtime-api/src/lib.rs:2311-2360` [tested: a_holder_reader_that_did_not_answer_is_not_read_as_an_empty_answer]
- **The agent tool catalogue was seven tools.** Read control; read events with a cursor and a limit of at most 128; read board bounded to at most 32,768 bytes; publish; request participant; yield; submit — exposed with a product prefix. All argument structures refuse unknown fields, so a forged author or payload digest is an invalid-parameters error. `.../ymp-agent-api/src/lib.rs:9-15,52-220` [tested: attempt_scope_is_controller_bound_and_invalid_pages_are_tool_errors]. There was no cancel or reply operation: the child could not answer a host-initiated request, so nothing in attempt 1 validates the model's operation-request round trip. Owner: W3-0001.
- **Transport was two hops.** Child to bridge over stdio newline-delimited MCP JSON-RPC, protocol version `2025-11-25`, methods `initialize`, `ping`, `tools/list`, `tools/call`; bridge to controller over a Unix stream socket, one request per connection, write-shutdown as the only framing, 64 KiB request and 1 MiB response caps, socket mode 0600 inside a 0700 directory. `.../ymp-agent-mcp/src/lib.rs:18,420-457`, `.../ymp-agent-rpc/src/lib.rs:253-262,415-430` [tested: mcp_rpc_application_chain_derives_collaboration_metadata]. No request id and no server-initiated direction.
- **The grant was a bare UUID: no digest, no expiry, no scope.** A binding of executable, socket path and token, with the token a random UUID, delivered to the child through `YMP_AGENT_SOCKET`, `YMP_AGENT_TOKEN`, `YMP_ATTEMPT_ID` and `YMP_INVOCATION_ID`, and verified by plain equality on token, attempt and invocation. `.../ymp-runtime-supervisor/src/lib.rs:2186-2226`, `.../ymp-agent-rpc/src/lib.rs:568-577` [tested: socket_capability_binds_calls_to_one_attempt]. The security document argues this is acceptable by design — "security comes from endpoint isolation and controller-side authorization, not an opaque token in a tool argument" (`legacy-attempt-1:ymp-docs/SECURITY.md:129-133`) — but bounding by lease or expiry was absent. The model's Grant carries an expiry and a token digest, which closes the gap; W1-0006 owns whether it is needed.
- **Issuance is computed server-side and never claimed.** The recruitment capability is derived from the bound endpoint; drivers read it before building the model-facing allowlist and drop the recruitment tool when it is absent, and the tool listing is filtered the same way. The documentation warns callers must never infer recruitment from runtime capacity — yet both drivers fall back to an empty capability set on a transport failure, so a broken endpoint looks exactly like a policy decision. commit ec9a792; `.../ymp-agent-rpc/src/lib.rs:273-277` [tested: generated_tool_config_tracks_endpoint_recruitment_capability, recruitment_crosses_mcp_rpc_and_the_bound_application_gate_once]. Fail-safe in the authority direction, fail-open in the diagnostic direction.
- **A denied call is a tool error, never a protocol error, and writes nothing.** Denials return JSON-RPC success carrying an error-flagged structured result with a code in invalid-arguments, rejected or internal, and every refusal leaves the journal byte identical. `.../ymp-agent-mcp/src/lib.rs:783-793` [tested: recruitment_refusals_are_fail_closed_through_the_private_endpoint, every_gate_refuses_in_plain_words_and_writes_nothing]. The model makes denial an event; attempt 1 proved by test that it is not, so a refused capability left no audit trace. Owner: W3-0001.
- **Correlation is not the idempotency key.** The socket hop carried no request id — correlation was the connection itself — and idempotency was an explicit command or request identifier in the tool arguments, with the effect and the reply mapping committed atomically before the response was returned. "A JSON-RPC request identifier is not treated as a durable idempotency key." `.../ymp-agent-rpc/src/lib.rs:22-29,74-99`, `legacy-attempt-1:ymp-docs/ARCHITECTURE.md:479-484` [tested: lost_mcp_reply_replays_submit_only_under_the_same_command_identifier, invocation_bound_yield_replays_one_authoritative_confirmation]
- **Supervisor lifecycle and its bounds.** Prepare; start the invocation — the kernel slice is admitted before the process exists, so no runtime ever runs under an unfunded slice; then yield or wake, submit, verify, terminate. Bounds: at most 8 wakes per attempt, a 24-hour maximum lease, a wake deadline no later than lease expiry, a 64 KiB prompt ceiling. `.../ymp-runtime-supervisor/src/lib.rs:2340-2353`, `.../kernel.rs:65-80` [tested: a_live_run_resumes_through_the_kernel_transitions, the_live_wakes_of_one_attempt_are_finite]
- **A timed-out wait reads the record, not a flag.** Closing waits five seconds polling every two milliseconds, and what the run owes is read from the committed state of its slice, never from the worker's finished flag — that flag is set after the terminal, after the submission and after the runtime is wound down, so a worker held in that window used to be recorded as an infrastructure fault over a completed slice and stripped the candidate of its verdict. Three outcomes: recorded; already committed (write nothing); unread — in which case the journal is failed on the reading that failed, because a poisoned lock stays poisoned and that state will never be judged later either. commits b34ceb9, 6aa404b [tested: a_controller_that_stopped_waiting_writes_nothing_over_a_committed_terminal, closing_a_controller_in_the_completion_window_keeps_the_finished_run_judgeable, a_poisoned_journal_still_records_the_terminal_of_the_run]
- **Cancellation must reach a runtime that is working.** Interruption used to be delivered only where a yielded slice waits for a wake, so a runtime that never read the token worked to completion regardless; supervision now reads the token wherever it holds the session. Separately, a close reply names an ended process tree only where the worker actually ended — before, every close claimed the tree was ended. commits 3bfd761, 61603f5 [tested: a_cancellation_reaches_a_runtime_that_is_working_rather_than_waiting, closing_a_controller_whose_runtime_stopped_answering_is_bounded]
- **A panicked worker still owes the kernel a terminal.** The worker body runs inside a panic boundary and supervision that died is recorded as an infrastructure failure; left to unwind, the slice stayed open, the run reached no terminal, and a later cancellation moved the journal alone while reporting success. `.../ymp-runtime-supervisor/src/lib.rs:2392-2404` [tested: a_worker_that_dies_of_a_panic_still_reaches_a_kernel_terminal]
- **The fake runtime was excluded by dependency closure, not by discipline.** The fixture crate is a dev-dependency of the CLI, the supervisor and the test kit and never a normal dependency; a test reads the manifest closure of the shipped binary and asserts the fixture is absent, with a sanity assertion that nine core crates are present so an empty closure cannot pass. commit fa3ca2c; `.../ymp-cli/tests/one_command_path.rs:584-609` [tested: the_fixture_runtime_stays_outside_the_shipped_binary]. It is manifest-level and does not stop a test-gated fixture inside a shipped crate.
- **A failed invocation must carry more evidence than a successful one.** run-002 could not be localized because the pre-repair supervisor discarded the runtime failure's intermediate usage and diagnostic summary, so elapsed time and a process exit were all that remained. The repair records a failure-evidence structure of phase, provider-request state, per-stage three-valued observations (true, false, unknown), runtime failure kind, last event id, sequence and type, process exit code and signal, duration, usage, cost and both diagnostic summaries, merged so an absent field never erases another component's observation. The provider-request state moves through not-started, turn-started-unconfirmed, responded and unknown, and neither runtime exit nor elapsed time may advance it. commit ee96f70; `.../ymp-runtime-api/src/lib.rs:1259-1345` [tested: terminal_failure_preserves_kind_usage_diagnostic_and_event_identity_without_attestation, probe_wrapper_reports_only_direct_provider_stage_observations]. The audit names this the single most transferable lesson of the attempt. Owner: W1-0017.
- **A shutdown bound is a sum of separately enforced deadlines.** The interface's measurement shutdown limit was the controller limit (5 s) plus a 500 ms SIGTERM request limit plus a 500 ms SIGKILL enforcement limit plus a 1 s worker-return limit — "every part is enforced by a deadline of its own, so the sum is a bound and not an expectation". Measurement processes lived in the interface's own group, so membership was recorded at the moment the measurement started and never recomputed. commit a19c480; `.../ymp-tui/src/app.rs:72-84` [tested: quitting_ends_an_engine_process_that_outlived_its_parent, quitting_ends_an_engine_that_starts_another_while_it_is_ending]

### 11.8 Research and decision facts

Owner: W3-0009 and W6-0005 (comparisons), W5-0001, W5-0002, W5-0003 (experience, calibration, knowledge), W1-0015 and W1-0013 (interface and report). Source unless stated: `legacy-attempt-1:ymp-docs/research/*.md`, `legacy-attempt-1:ymp-docs/DECISIONS.md`, `legacy-attempt-1:ymp-docs/REPUTATION.md`. Falsifier for the section: every measured number here is a single development run and is explicitly not a success rate; a repeated measurement under a declared arm supersedes it.

- **What ran with real models.** The 2026-08-12 development smoke ladder is the only substantial real-model measurement: three levels accepted by Codex — for example 54,138 input / 46,080 cached / 973 output / 193 reasoning tokens at level 1, and 101,272 and 89,082 input at levels 2 and 3 — with interface-driven cases at 90,944 to 155,599 input. Claude Code 2.1.232 on 2026-08-15: one accepted candidate at 118,783 microUSD, and three runs of one card at 46,442, 60,871 and 60,454 microUSD of which one was an infrastructure error. `legacy-attempt-1:ymp-docs/research/cal-001-calibration.md:92-199` [measured]. Promotion stayed blocked for want of currency accounting, strict containment and a frozen corpus.
- **Prompt policy was worth about 64% of input tokens.** On the same case, batching reads and checks and suppressing progress messages cut input usage from 152,288 to 54,138 while both candidates passed the same protected oracle. `cal-001-calibration.md:102-103,118-122` [measured, n=1 per arm]. Owner: W1-0013 (context composition), W1-0004 (cost).
- **The first communication pilot produced zero communication.** Three participants were to append to a shared board file and the third to infer a unique intersection; two calls were made, both refusals, 78,547 input / 57,344 cached / 983 output / 539 reasoning tokens in 61,708 ms with a null monetary cost, and the board file's hash was unchanged. Stopped by a frozen four-call ceiling; two participants never launched. `legacy-attempt-1:ymp-docs/research/run-001-manual-file-communication-pilot.md:46-66` [measured]. It is evidence about nothing except the ceiling.
- **Why the equal-budget program was dropped.** See the section preamble: sound arithmetic, a corpus of 4 against a stated requirement of 3,984 more, a split conflict that made diagnosis and transfer jointly unidentifiable, and an explicit owner cut on 2026-09-06 — "failed infrastructure does not count as a negative capability result". `legacy-attempt-1:ymp-docs/work/backlog/OWNER-DIRECTION-20260906.md:6-7` [design]
- **The cumulative-knowledge design is testable and unexecuted.** Transfer means a predeclared favourable observable on a target that did not contribute to selecting, writing or tuning the memory, judged by an independent verifier. The minimum contrast is the same receiver profile in fresh sessions across three arms — experience-derived memory, no memory, and a similarly sized generic memory — the last two being the required negative-transfer controls. A collective claim additionally requires one agent holding the exact same memory bytes. Cost is recorded everywhere and is never a victory threshold. Staged: executable preparation with no models, carrier feasibility, accumulation with new-task transfer, bounded conclusion. `legacy-attempt-1:ymp-docs/research/rdr-002-cumulative-knowledge-poc.md:34-52,72-104` [design]. Owner: W5-0003, W5-0004, W6-0005.
- **The literature minimum.** The smallest first probe is an inert text note at *subtask* granularity with provenance and digest frozen before the second task; deterministic delivery suffices and semantic search is a later, independent mechanism. The cited risk is negative transfer from coarse task-level skills. Its demand: proved delivery, a participant choosing not to read, and missing instrumentation are three different outcomes, and echoing bytes is not understanding. `legacy-attempt-1:ymp-docs/research/lit-001-cumulative-knowledge-minimum.md:10-27` [design]
- **Five mechanisms held necessary, each with a rejection condition.** Useful evidence transfer; decomposition and synthesis; critique with preserved dissent; adaptive local allocation; recovery after perturbation. Each pairs an observable prediction with a controlled intervention — absence, neutral replacement, shuffling, direct delivery of the same raw evidence — and a named rejection: no action change means no listening; action change without an accepted outcome, calibrated abstention, recovery or cost means no task value. `legacy-attempt-1:ymp-docs/research/map-001-mechanism-map.md:14-20,49-60` [design]
- **Mechanisms ruled out, with arguments.** Central semantic planning, fixed roles and cheapest-capable selection ("makes the system a deterministic solver"); a learned grade as a claim gate; one globally readable board; a shared writable repository; consensus or majority vote; rewarding influence, agreement, role formation or message activity, because the measurement becomes the target and selects theatre; MCP as the internal bus; and calling a model API from the trusted kernel writer. `legacy-attempt-1:ymp-docs/DECISIONS.md:275-311` [design]
- **What the approved model appears to lack.** The intervention vocabulary (message absence, neutral replacement, shuffling, direct-evidence delivery, delay, participant removal) has no visible counterpart; the generic-memory control — a similarly sized irrelevant note — is the one control separating "memory helped" from "any text helped"; and the expected-null stratum appears as an exclusion rather than a required control. `map-001:22-30`, `rdr-002:72-77` [design; the claim about the new model is inference from its text, not a measurement]. Owner: W6-0005.
- **The reputation negative result.** The persistent per-skill grade was removed, not deferred, for two reasons: a grade-based claim gate is central allocation even when learned, and the signal is too sparse — distinguishing a 60% from a 75% pass rate needs about 152 comparable outcomes per runtime profile, and plus or minus 5 points needs about 323, under assumptions deliberately chosen to be favourable. Nothing is a scalar: only immutable outcome evidence persists (passed, failed, infrastructure error, cancelled, abstained) with enough context to prevent silent pooling, and a query returns an evidence card that must show its filter, effective sample size, an estimate with an interval, recency, version coverage, cost distribution and correlation sources — never an eligibility verdict or a global order. Explicitly not scored: message volume, persuasion, agreeing with a later consensus, claiming a role or confidence level, peer citations. Seven preregistered preconditions gate any future adaptive allocation, and none may enter the trusted kernel. `legacy-attempt-1:ymp-docs/REPUTATION.md` [design]. Owner: W5-0001, W5-0002. Cheapest to lose, most expensive to rediscover.
- **Ten consequential owner decisions.** (a) 2026-08-10 remove the per-skill grade and claim gate. (b) 2026-08-13 the product goal governs the mechanics: a hardening decision that makes goal-to-verified-result impossible is a divergence to resolve. (c) 2026-08-13 every interface action exists as a command of the same executable with the same authority checks and the same journal path. (d) 2026-08-13 the full suite runs once before integration, not per card — measured at 7m14s times 8 cards. (e) 2026-08-13 drawn interface sources are references, not a frozen contract. (f) 2026-08-13 the interface is one framework bound to real state; a concept the domain cannot produce is shown unavailable, enforced by checks that must fail on violation. (g) 2026-08-15 no confirmation dialogs, no operator-typed identifiers, no acknowledgement steps, no setup wizard: the operator's only deliberate acts are enabling a provider and typing a goal. (h) 2026-08-15 the permitted model set is frozen per run and the operator names models, not roles. (i) 2026-09-06 cumulative knowledge becomes the primary scientific objective. (j) 2026-09-06 a fully English interface, but verbatim user input, project and file names and quoted source retain their language. `legacy-attempt-1:ymp-docs/DECISIONS.md:437-486`, `.../design/COLLECTIVE-OWNER-DECISIONS.md:8-12`, `.../work/backlog/OWNER-DIRECTION-20260906.md:5-21`, `.../OPTIMIZATION.md:47-54` [design; (d) measured]. Note the tension: (c) and (g) were reconciled by routing every command through one writer, not by removing the surface. A standing rule accompanies them: every product or agent run gets a fresh disposable root with separate project, home, data, temporary, build and export paths, and the operator's real data directory is never an execution root — a convention no harness enforced.
- **The user-journey rules for the interface.** The goal is stored verbatim with no translation and no required English keyword. At most one blocking question at a time, and the next only after an answer and only when a safe assumption would materially change the result. Preparation is a non-blocking dim collapsible trace, never an approval ceremony. Five rejection criteria, any one of which fails the interface: a mandatory confirmation, typed identifier or manual assignment before work starts; a main path organised around subsystems; delivery, order or message count passed off as reading, usefulness, coordination, leadership or causal influence; automatic checks mixed with human evaluation, the interface embedding the artefact instead of naming the exact external command or path, or cancellation, exhaustion, needs-clarification and infrastructure error looking like success; a new mandatory action or information category outside the path. Terminals are accepted, exhausted, abstained, cancelled and infrastructure error — "exhaustion does not mean a verified result". `legacy-attempt-1:ymp-docs/USER_JOURNEY.md:47-49,58-64,161-180,291-306` [design]. Owner: W1-0015, W1-0013. An earlier decision allowed three clarifications per run; the later journey is the binding one, and the typed-identifier confirmation the interface actually shipped contradicts decision (g) and was recorded as a defect to remove.
- **Interface structure worth knowing.** The event loop was a blocking channel with a timeout rather than a fixed tick, with a redraw flag and collapsed queued events; state, projection and drawing were three layers with exactly one module allowed to write into a frame; unavailability carried a reason instead of a zero; "no fabricated value" was a failing structural test that scanned sources for illustration identifiers and unsupported concepts, not a policy; interface and command surfaces were machine-checked against each other for mutual surplus; and budget values carried an enforced/observed/estimated class. `.../ymp-tui/src/app.rs:2831-2945`, `.../ymp-tui/tests/state_binding.rs:47`, `.../ymp-cli/src/surface.rs` [tested: no_screen_value_is_fixed_in_the_source, a_page_with_no_state_behind_it_is_unavailable_rather_than_populated, only_the_frame_layer_draws, every_interface_action_is_a_command_and_neither_side_holds_a_surplus, the_command_and_the_interface_commit_the_same_journal, the_public_command_surface_never_names_the_writer]. Owner: W1-0015, W1-0004 (the three budget classes).
- **Dependency and build facts.** Fourteen external crates for the whole workspace and no async runtime; line-tables-only debug info measured a cold build from 23.2 s to 14.0 s and 479 MB to 403 MB; a build cache was tried and refuted — three builds, fifty compilations, zero cache hits. `legacy-attempt-1:ymp-rust/Cargo.toml` [measured]. Falsifier: a different dependency set; the refuted build cache is host-specific.

### 11.9 Pitfalls with fixing commits

Each row is a defect attempt 1 shipped and then fixed; the commit is where the reasoning lives.

| Commit | Lesson |
|---|---|
| 6544e25 | A projection that fell behind the journal is repaired by folding the missing facts through the startup path, not by re-executing. |
| 9399844 | Every small file write is staged and renamed with a directory fsync; an in-place write is read truncated by a concurrent reader. |
| 61914f3 | Advance a projection only after its durable write; and reopen with the terms recorded at creation, never today's configuration. |
| bf28efe | A leaf-only path pre-check is always wrong; examine every ancestor before the first write and name every conflict at once. |
| b084579 | The directory the product was started in receives nothing; earlier product data beside a project is ignored, never read, copied or deleted. |
| 40315e2, 427c87a | Remove the generic storage-root accessor; expose only typed child paths that validate their identifier, pinned by compile-fail doctests. |
| 318016a | A detector that falls through on a near-miss names the wrong file; stop at the obstacle and name the missing permission. |
| 5571a07, 1a88a10 | Isolate verifier observation roots (landed twice — rebase churn, not two defects). |
| ac3db26, 38a1f4a | Enforce isolated candidate identity (landed twice). |
| 13531f1, 08c1833 | Isolate immutable candidate checks (landed twice). |
| 57f8b9b | An admitted participant that never started is not "has not begun one"; only a live participant proposes. |
| 4a6e8a0 | Check affordability among the refusals, before any write; otherwise the journal records a run that paid for a participant it never got. |
| c094be0 | A fact record without opening terms is not a fresh section; one writer holds the section and an append is checked against the recorded chain. |
| aa0ef50 | A projection carries the exact audience verbatim; dropping it silently widens what a reading appears to show. |
| 7fedc7a, 5776b5d | A policy flag is only as strong as the single place that resolves which record it lives in; a root and a store are different arguments. |
| 6f89c17 | Measurement decides what exists and the executable's digest decides staleness; a hand-written filter hid models the build serves. |
| 72b288b | Do not measure on every read; an unmeasured page says so instead of starting a provider process. |
| fb961cf | An operator decision is not an absence of measurement; age errs toward staleness. |
| 4cd0bee | A measurement landing after a permission moved must not restore what the operator held back. |
| 4fff141 | Process membership is recorded while it can still be read, never recomputed after reparenting. |
| 777dfd5 | An observed runtime version is evidence and never an acceptance predicate. |
| ec9a792 | Capability is computed server-side and never inferred from runtime capacity; an unreadable endpoint must not read as a policy decision. |
| ee96f70 | A failed invocation must carry more evidence than a successful one; three-valued stage observations, merged so absence never erases. |
| 3bfd761 | Cancellation must reach a runtime that is working, not only one waiting for a wake. |
| 61603f5 | A close reply names an ended process tree only where the worker actually ended. |
| b34ceb9, 6aa404b | Read what a run owes from its committed state, never from a worker flag; a state that cannot be read is not the state "still open". |
| fa3ca2c | Exclude a fixture runtime by dependency closure and assert the closure, not by discipline. |
| a19c480 | A shutdown bound is a sum of separately enforced deadlines, so it is a bound and not an expectation. |
| 7cdb238 | A standalone simulation interface drifts into its own domain and has to be closed rather than merged. |

### 11.10 Edge-case checklist by owning task

Legacy test names, grouped by the task that now owns the behaviour. They are scenarios to reproduce against the new types, not tests to port.

- **W1-0001, W1-0016 (journal and storage):** gap, duplicate, predecessor and self-digest corruption distinguished; incomplete tail; oversized record and journal refused before parsing; terminal reserve preserved and capacity exhaustion recovered as an infrastructure error; interrupted apply recovered and the next command committed; recovered projection holds what its journal records; duplicate command replayed and state recovered from the cursor; recovery rejects a conflicting command digest before applying; metadata write failure replays the committed command and repairs the summary; second writer rejected; persisted infrastructure error asserted; canonical lowercase object paths; partial target neither completed nor accepted; altered object bytes rejected on read; a store is never opened as a root; a root of another layout version is refused; earlier product data beside the project is ignored, unchanged and never copied; attempt paths are concrete children and refuse storage traversal.
- **W1-0005, W4-0001 (snapshots and isolated copies):** reproducible immutable private workspace; forged path outside the workspace rejected; source capture omits product roots without omitting nested project names; source symlink rejected; submission rejects changes to base files inside excluded subtrees; submission rejects new derived files without storing them; stale base rejected without candidate creation; forged delete and unknown schema rejected.
- **W1-0008, W4-0003 (result versions and merge):** conflicting integration cannot replace an existing candidate; an identical tree from a different base is rejected; a file where a directory is needed stops the application before it begins; a directory where a file is named stops it; a symlink on the way stops it and nothing leaves the project; an application stopped part way names the files it already wrote.
- **W1-0009, W1-0019, W2-0004 (checks, evidence, discrimination):** bounded command requires a failing negative control; the control cannot seed the candidate observation through the working directory; the control cannot modify the oracle used for the candidate; observations cannot exchange state through the outer root; disposable mount, pid and network namespaces used; an unavailable namespace launcher is not an oracle rejection; the launcher protocol distinguishes isolation failure from oracle exit 1; oracle exec failure is an isolation failure; exact digest evidence bound to candidate, contract and oracle; explicit environment binding survives serialization and detects substitution; only bound evidence can accept a candidate; ambiguous legacy evidence blocks recovery as an infrastructure error; altered, missing or digest-substituted environment is an infrastructure error; a test script that cannot be executed is named with the permission it lacks.
- **W1-0006, W1-0007, W3-0003 (admission, lease, award):** one compatible offer and bid form contract, escrow, lease and obligation together; an incompatible or expired record forms nothing; a stale fencing generation cannot submit, renew or close after reassignment; only the sponsor awards and only the holder advances; a reservation returns to a contract that changed hands; a settlement into a closed contract is refused and changes nothing; an obligation cannot close while work below it is outstanding; a non-holder cannot close the obligation; a contract cannot close while a reservation is still due back; a dimension is never paid out of another; an uncoverable debit is refused and named; a yielded slice resumes only on a committed fact; duplicate, coalesced and lost notifications change nothing; a cursor only moves forward into facts that exist; an expired wake stops holding the run open; the kernel never chooses between two live bids; quiescence, a spent budget and an unverified result are not acceptance; removing each named invariant check produces a counterexample.
- **W3-0002 (recruitment and team change):** the request carries no field a decision could read a preference out of; a repeated request is refused as a duplicate; a request failing several gates is refused by the first in the declared order; every gate refuses in the words of the constraint that stopped it; the gate names no entry the request did not; a proposer that is not running proposes nothing; a participant admitted and never started proposes nothing; unbound and yielded sessions recruit nobody; a failed start leaves the admission standing; an admission spends the authority it used and funds the participant it admitted.
- **W3-0001 (board and team operations):** a payload is inert whatever it states; nothing a payload states keeps a run alive; a detailed message reaches the admitted and no one else; a named audience narrows and never widens; a grant on one scope exposes no other; an expired grant admits nothing; reading a scope is not publishing to it; the audit record outlives the salience it was published with; a refresh buys salience again and rewrites nothing; a fact record without opening terms is refused and left as it was; a board section is written by one holder; a foreign board is refused at the recorded position; order and citation alone never produce a causal label; recruitment refusals are fail-closed through the private endpoint with zero journal growth; the socket capability binds calls to one attempt; a lost reply replays a submission only under the same command identifier.
- **W1-0003 (registry and readiness):** a created run carries the pool it was created under; what changes after the freeze belongs to the next run; a root that offers nothing creates no run and says so in plain words; an account held back during a measurement stays held back; quitting during a long measurement leaves no engine; quitting ends an engine process that outlived its parent; quitting ends an engine that starts another while it is ending; the observed runtime version is evidence, not attestation authority; every admitted launch-chain program executes and refuses replacement; admitted runtime bytes execute after the source path is replaced; the fixture runtime stays outside the shipped binary.
- **W1-0017, W1-0018, W6-0001 (execution host and backends):** the prepared launch matches the actual process and rejects all mutations; only the approved environment and search path reach the child and its record; an ambient or delegating session is rejected before model use; changed tool-event and usage schemas fail for their own reason; a cost its model breakdown does not support is refused; terminal outcomes preserve available accounting; a recoverable process failure resumes the same managed session; an unknown session reported by a native resume is terminal; a timeout terminates the runtime process group; a cancellation token interrupts a blocked runtime tree; a descendant does not survive a failed parent; a holder reader that did not answer is not read as an empty answer; what the record would not take is reported with the run; a controller that stopped waiting writes nothing over a committed terminal; closing a controller in the completion window keeps the finished run judgeable; a poisoned journal still records the terminal of the run; a cancellation reaches a runtime that is working rather than waiting; a worker that dies of a panic still reaches a kernel terminal; terminal failure preserves kind, usage, diagnostic and event identity without attestation; workspace-root substitution after measurement fails before start.
- **W1-0015 (interface):** no screen value is fixed in the source; a page with no state behind it is unavailable rather than populated; only the frame layer draws; every interface action is a command and neither side holds a surplus; every action a key can reach is an action a command performs; the command and the interface commit the same journal; the public command surface never names the writer.

### 11.11 Practices present in attempt 1 and in neither attempt 2 nor the approved model

Each entry states the practice, its source and the question it puts to the owner. Attempt 2 was surveyed only at the intent and architecture-index level; that is the limit of the claim.

- **Switchable-invariant counterexample tests.** Sixteen named checks disableable in a test build, each with a generated-schedule counterexample. `.../commitment/ledger.rs:56-110` [tested]. Is every new invariant required to ship a disable switch and a counterexample generator? It is far cheaper at greenfield than retrofitted.
- **Two non-convertible allowance namespaces.** A six-dimension talk allowance separate from the control budget, so unspent conversation cannot pay for execution. `.../ymp-board/src/budget.rs:1-90` [tested]. Does board traffic draw on the session budget, which attempt 1 treated as a correctness bug rather than a convenience?
- **Creation authority as an irreversible dimension.** Participant, attempt, invocation, offer and obligation starts are spent and never returned, which is what bounds recursive decomposition. `.../commitment/budget.rs:14-95` [tested]. Without such a dimension, what bounds a recursive solicitation-assignment-solicitation chain?
- **Uninhabited authority types.** A type with no variants states that a capability does not exist and cannot be granted without a visible public change. `.../ymp-board/src/records.rs:28-40` [tested]. Is "cannot" expressed in the type system anywhere in the new kernel, or only in prose and tests?
- **Intervention-gated causal labels.** An influence edge exists only where a controlled intervention is replicated at least twice under a matched budget, and a view claiming otherwise fails its own check. `.../ymp-board/src/observatory.rs:186` [tested]. What stops a future experience or reporting surface from printing "A caused B" from order and citation?
- **The frozen-pool digest as the comparability unit.** Two runs naming the same digest were created against the same capability boundary. `.../ymp-domain/src/pool.rs:1-210` [tested]. What plays this role for W3-0009 and W6-0005 matched comparisons if the Registry is read by reference?
- **Refusal ordering as a product property.** A request failing five gates is refused by the first in a declared order, so an operator reads a stable limiting constraint. `.../recruitment.rs:145-170` [tested]. Is gate order part of the Gatekeeper contract or an implementation detail?
- **Compile-fail doctests pinning storage boundaries.** After the generic storage accessor was deleted, compile-fail doctests kept it deleted. commits 40315e2, 427c87a [tested]. Cheap, and it held; is it adopted for the workspace and journal boundaries?
- **A reserved terminal journal slot.** Capacity is computed so a dying run can always append one terminal event. `.../ymp-storage/src/journal.rs:191-234` [tested]. Does the new Journal bound anything, and can a run that hits the bound still record why?
- **The failed-invocation evidence record.** A structured failure record with three-valued stage observations, merged so an absent field never erases another component's observation. commit ee96f70 [tested]. Is the amount of evidence a failure must carry specified anywhere, or left to each backend?

### 11.12 Not carried over

These constructs of attempt 1 have no place in the new model. Denylisted identifiers are in 11.13.

- The Linux namespace-and-chroot shell launcher with its multi-state descriptor handshake: about 200 lines of shell inside Rust, platform-forked, and by its own comment making no containment claim. Prefer an external sandbox, or a first-class weak-isolation evidence class.
- The non-Linux fallback isolation, a plain directory copy with a cleared environment reported through the same evidence shape as the namespaced profile.
- Discarding output bytes and keeping only their digests: zero diagnosability when a check fails.
- The run summary file as a projection: it bought nothing the journal did not, and cost an atomic-write protocol, a repair path and a class of failure modes.
- Deriving the project directory segment from a path hash; the four-digit run ordinal; the deliberately racy directory claim.
- Fixed constants as the bounds — a 16 MiB journal, 9,999 runs, a 20,000-entry copy cap. Keep bounds and the terminal reserve; re-pick the numbers.
- Two divergent exclusion lists for one concept.
- The self-clearing digest field (serialize with the field blanked, hash, write it back): it works and it is a trap; use a separate wrapper type.
- A schema version per crate with no migration anywhere: every mismatch is a dead store.
- One application object holding board, journal, objects, recruitment, commitments and the agent tool host; the sealing commits were fighting a structure that should not have existed.
- The 24-command commitment enum; six-variant outcomes the kernel never reads; two overlapping sealing transitions for one concept; a ten-label message taxonomy the kernel stores and never reads.
- Four parallel state vocabularies across registry levels; a dated vendor fact compiled into a constant; owner-decision default ceilings chosen for coherence and never measured, whose replacement task never ran.
- Fatal-on-unknown-event in the provider drivers: it couples run survival to the provider's release cadence for no safety gain.
- The process-global destructively drained termination list; the one-request-per-connection socket with write-shutdown framing and no request id; the three-and-a-half-thousand-line single-file drivers; the shipped demo-fixture test kit; a provider-enforced per-process cost ceiling as the Treasury's bound.
- The equal-budget program; a learned per-skill grade or any ranking of agents, models or candidates; a thirty-seven-surface console-shaped operator surface; the typed-identifier confirmation ceremony; Russian screen copy (Russian input semantics stay); a standalone simulation interface; a build cache measured at zero hits on this codebase.
- Documented-but-unimplemented machinery: object GC, fencing tokens in the documents, protected-query budgets, holdout retirement, reviewer blinding beyond the committed-assessment half, and all eight invariants.

### 11.13 Denylist candidates for `tools/legacy_scan.py`

Identifiers unique to attempt 1, each verified present under `legacy-attempt-1:ymp-rust` and absent from the approved model: `CommitmentCommand`, `LeaseRecord`, `EventEnvelope`, `EventDigestInput`, `SubmissionManifest`, `SnapshotManifest`, `CandidateRef`, `CandidateIdentity`, `VerifiedEvidence`, `StoredVerificationEvidence`, `RunKeepingAuthority`, `McpBinding`, `ToolHostProbeFailureEvidence`, `ProviderRequestState`, `ManagedShutdown`, `FrozenPool`, `CatalogRoute`, `RecruitmentPolicy`, `ParticipantStartPath`, `CreationAuthority`, `DiagnosticSummary`, `ProbeTransportIdentity`, `AgentToolCapabilities`, `ExcludedPathChanged`, `ApplyInterrupted`, `unestablished_terminations`, `absorb_committed`, `recover_projection`, `append_terminal`, `blocking_ancestors`.

Excluded because the approved model uses them: Grant, Lease, Snapshot, Evidence, Board, Notice, Registry, Solicitation, Offer, Commitment, Assignment, Invocation, Attempt, ResultVersion, Journal, Envelope, Discrimination, TeamOperation, CheckSpec.

### 11.14 Open questions for the owner

1. Where does a hidden check's content live, given the producer runs on the same host, and what enforces that it never enters producer context (R-10)?
2. Does a negative control become mandatory per Criterion, and is a passing control an infrastructure refusal (as in attempt 1) or a Criteria rejection?
3. Does Snapshot accept attempt 1's losses — no modes beyond an executable bit, no empty directories, symlinks refused — and is a symlink an error or a recorded value?
4. Does applying a ResultVersion delete what the result deleted, and is the contract transactional or attempt 1's honest partial report?
5. Should Snapshot capture carry entry and byte limits, and what is the refusal when a real project exceeds them?
6. Is a single-writer guarantee still required across processes, and does a scoped WorkspaceGuard hold refuse a second process or only a second scope?
7. Does the model's renewal counter replace escrow exhaustion as the renewal bound or sit beside it, and which does a refusal name?
8. Does entering the Expired commitment state authorize anything by itself, or does reassignment still require a fresh offer on the same solicitation?
9. Is there an equivalent of an irreversible creation-authority dimension, and does the Board draw on a separate allowance namespace?
10. What is the new equivalent of the frozen launch snapshot, so a mid-run provider change cannot silently break matched-budget comparability?
11. Does ExperienceVault ever write a scalar, and is ToolDenied journaled as an audit fact?
12. Does the evaluation design keep the generic-memory control, the expected-null stratum and the single-agent-with-the-same-bytes comparison, and does it distinguish proved delivery from voluntary non-reading from missing instrumentation?

## 12. Maintenance

- A fact that a task re-verified against the live tool is moved into that
  task's evidence and stays here with "confirmed by <task>".
- A fact whose falsifier fired is deleted in the same commit that records why.
- New harvests (attempts 1 to 3, if their code appears) add a row to section 1
  and entries with the same structure; they never add code.

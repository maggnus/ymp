# Agent runtime calibration

## Purpose and evidence boundary

The development calibration ladder checks whether an exact runtime profile can make increasingly
stateful repository changes, preserve public constraints, stop cleanly, expose usage, and pass an
oracle withheld from its workspace. It is an engineering instrument, not the frozen POC study
corpus. Its results do not support the coordination hypothesis and are never pooled with the
primary three-arm comparison.

The implementation is in `ymp-rust/tools/ymp-calibration`. `prepare` creates a disposable Git
repository containing only public requirements and visible tests. `verify` copies the exact
candidate, injects the protected oracle into the private copy, checks that `Cargo.toml` is
unchanged, runs formatting, tests and strict Clippy, and emits a candidate digest and JSON result.
`validate-cases` proves that each seeded defect is rejected before the ladder is used.

Every product-level calibration or evaluation receives a fresh disposable root with separate
project, `HOME`, `YMP_HOME`, `TMPDIR`, build, and export paths. The product is never evaluated from
the source worktree and never reads or writes the operator's real `~/.ymp`.

## Ladder

| Level | Case | Added difficulty | Protected observation |
|---|---|---|---|
| L1 | `L1-line-endings` | One local UTF-8 boundary function | Complete CRLF, final lone CR, embedded CR, multibyte boundaries |
| L2 | `L2-size-parser` | Strict grammar across two source files | Case, whitespace, Unicode, invalid suffixes, parse and multiplication overflow |
| L3 | `L3-command-ledger` | Result identity across intervening state changes | Rejected-result replay, conflicting key reuse, overflow replay, empty-key non-allocation |

The next level must add repository-scale decomposition and bounded coordination. It must not weaken
the first three levels or expose their protected tests to producing attempts.

## Weak-participant mechanism-visibility ladder

Before spending the frozen primary-comparison budget, development experiments deliberately use a
weaker admitted profile on elementary and then increasingly decomposable repository tasks. Each
level compares the same profile in three matched-budget conditions: one participant, independent
best-of-`n`, and local coordination. The purpose is to make a coordination effect, or its absence,
large enough to diagnose before stronger executors approach the oracle ceiling.

The exact first development protocol is recorded in
[WEAK_DIAGNOSTIC.md](WEAK_DIAGNOSTIC.md).

A coordinated result advances this diagnostic ladder only when it repeatedly exceeds both the
single-participant and independent-selection conditions, combines attributable non-redundant work,
and survives removal or replacement of the messages claimed to matter. Fluent transcripts, more
participants, or beating only one weak participant do not advance it.

This ladder is development evidence. It does not replace the preregistered primary comparison or
the required strong-single-agent baseline. A mechanism that helps weak participants but fails to
transfer to stronger profiles remains a bounded finding about weak-agent coordination rather than
the product's reliability claim.

The first provisional profile cohorts are evaluated separately rather than mixed inside one arm:

| Family | Diagnostic profile | Initial effort |
|---|---|---|
| Codex | `gpt-5.6-luna` | `low` |
| Claude | `claude-haiku-4-5` | provider default |
| GLM | `glm-4.5-air` | `none` |

Availability in the local catalog is not admission. Each exact runtime/profile pairing must still
pass lifecycle, isolation, cancellation, usage-evidence, and cost-accounting probes before it can
produce a comparable observation. A result is first interpreted within its own profile cohort;
cross-family agreement or transfer is reported separately.

On 2026-09-01 a read-only `gpt-5.6-luna` low-effort test runner executed `validate-cases` in an
isolated worktree. The command exited zero and reported `seeded_defect_rejected: true` for L1, L2,
and L3 with identical clean Git state before and after. This establishes only that the protected
oracles reject the planted defects; it is not an agent-solution result or profile admission.

## Pinned development profiles

| Runtime | Exact profile | Ambient state | Current readiness |
|---|---|---|---|
| Codex | `codex-cli 0.147.0`, `gpt-5.6-terra` (route measured against the account; `gpt-5.6-sol` is refused for ChatGPT accounts), low reasoning, `ymp-codex-low-v2` (v2 from 2026-08-15: the product appends its own publication instruction after the verbatim request; measurements before that commit carry v1) | Ephemeral session, user configuration and rules ignored, native multi-agent, plugins, skills, browser, applications and unrelated tools disabled | Ready and exercised through `ymp` |
| Claude Code | installed build admitted at or above the `2.1.227 (Claude Code)` floor by numeric comparison, measured and recorded per run, `claude-opus-5`, low effort, `ymp-claude-low-v2` (v2 from 2026-08-15, same boundary as codex), `acceptEdits`, USD 1.00 invocation limit | Configuration sources, slash commands and session persistence disabled; strict generated MCP configuration and repository-only built-in tools | Driver fixture passes; local OAuth authenticated and exercised on the product path |

The compiled probes reject missing authentication and exact-version mismatches. Both process
drivers bound structured output, impose a ten-minute wall-time limit, create a private process
group, terminate that group on timeout, interruption or drop, and reject a successful process that
omits its terminal structured event. A Codex fixture proves that a spawned descendant does not
survive timeout; live Claude termination is measured, and a descendant that creates its own session survived the supervisor, which W1-APP-02k closes.

## 12 August 2026 smoke results

All rows below are single development runs. They establish functionality, not a success rate.

| Harness | Runtime and profile | Case | Result | Input tokens | Cached input | Output | Reasoning output |
|---|---|---|---|---:|---:|---:|---:|
| Paseo 0.3.1 | Codex `gpt-5.6-sol`, low, `auto` | L1 | accepted | 27,020 | 22,272 | 232 | not reported |
| Paseo 0.3.1 | Codex `gpt-5.6-sol`, low, `auto` | L2 | accepted | 34,741 | 33,536 | 197 | not reported |
| Paseo 0.3.1 | Codex `gpt-5.6-sol`, low, `auto` | L3 | accepted | 37,061 | 36,608 | 178 | not reported |
| Paseo 0.3.1 | Claude `claude-opus-5`, low, `acceptEdits` | L1 | infrastructure error before model use | 0 | 0 | 0 | 0 |
| `ymp`, unoptimized prompt | Codex pinned profile | L1 | accepted | 152,288 | 132,864 | 2,118 | 448 |
| `ymp`, `ymp-codex-low-v1` | Codex pinned profile | L1 | accepted | 54,138 | 46,080 | 973 | 193 |
| `ymp`, `ymp-codex-low-v1` | Codex pinned profile | L2 | accepted | 101,272 | 77,056 | 1,893 | 356 |
| `ymp`, `ymp-codex-low-v1` | Codex pinned profile | L3 | accepted | 89,082 | 76,288 | 2,166 | 365 |
| `ymp`, `ymp-codex-low-v1` | Codex pinned profile | M0 attempt-scoped `read_control` | accepted | 29,925 | 14,080 | 248 | 104 |
| `ymp`, `ymp-codex-low-v1` | Codex pinned profile | M1 private L1 candidate | accepted | 105,977 | 84,480 | 1,656 | 341 |
| `ymp`, `ymp-codex-low-v1` | Codex pinned profile | M2 private L2 candidate | accepted | 170,278 | 142,336 | 2,201 | 405 |
| `ymp`, `ymp-codex-low-v1` | Codex pinned profile | M3 private L3 candidate | accepted | 155,599 | 127,232 | 2,879 | 352 |
| `ymp`, `ymp-codex-low-v1` | Codex pinned profile | M1b private L1 candidate | accepted | 104,795 | 81,664 | 1,422 | 370 |
| `ymp` TUI, `ymp-codex-low-v1` | Codex pinned profile | M1c complete TUI L1 path | accepted | 123,689 | 101,632 | 1,599 | 344 |
| `ymp` TUI, `ymp-codex-low-v1` | Codex pinned profile | M2b managed L2 repeat | accepted | 145,471 | 106,752 | 2,402 | 283 |
| `ymp` TUI, `ymp-codex-low-v1` | Codex pinned profile | M2c complete TUI L2 repeat | accepted | 90,944 | 71,424 | 1,668 | 278 |
| `ymp` TUI, `ymp-codex-low-v1` | Codex pinned profile | M3b complete TUI L3 repeat | accepted | 131,860 | 89,600 | 2,655 | 280 |
| `ymp` TUI, `ymp-codex-low-v1` | Codex pinned profile | M3c complete TUI L3 repeat | accepted | 110,513 | 88,576 | 2,379 | 367 |
| `ymp` TUI, `ymp-codex-low-v1` | Codex pinned profile | M3d agent-submitted L3 candidate | accepted | 132,829 | 109,824 | 2,504 | 305 |

Paseo's last-usage snapshot and Codex's `turn.completed` totals have different accounting
semantics and must not be compared as if they were the same metric. Codex did not expose currency
cost in these events. The prompt-policy calibration is internally comparable: on the same L1 case,
batching reads and checks and suppressing progress messages reduced input usage by approximately
64% while both exact candidates passed the same protected oracle.

The M0 run started the MCP process from the current absolute `ymp` executable, bound it to one
controller attempt through a private Unix socket, exposed only `read_control`, `read_events`, and
`submit`, and set this server's Codex tool-approval mode to `approve`. The completed MCP event,
model response, and final controller projection contained the same run identifier and `running`
status. The approval setting is scoped to this generated server configuration and does not change
the user's Codex configuration.

The M1-M3 runs captured each public source as an immutable base, materialized a private Git
workspace, completed the same attempt-scoped MCP read, let Codex modify and test only that
workspace, and waited for process completion before capture. The controller then produced a
bounded submission and a reproducible immutable candidate. For every level, the bounded verifier
first rejected the seeded negative control, then ran the protected oracle, formatting, all-target
tests, and strict Clippy against the exact candidate. Each verifier result was committed as opaque,
digest-bound evidence and moved the run to `accepted`. An earlier M1 trial was rejected because
build output entered the submission; explicit capture exclusions and a regression test now
preserve excluded base files while omitting derived subtrees.

M1b repeated the accepted L1 path after verifier-object persistence was added. Reopening the data
root validated the journal, candidate, and evidence objects; the TUI then exported the accepted
state, four-event journal, materialized candidate, and evidence object. The exported evidence
object's SHA-256 matched its manifest identifier.

M1c exercised the user-facing path at 120 × 40. The TUI loaded a versioned contract, selected the
ready Codex profile, launched the managed process, displayed its MCP and lifecycle events, captured
a two-change candidate, ran the contract-declared bounded verifier with its negative control, moved
the controller state to `accepted`, and exported all evidence. The digest-linked runtime transcript
retained usage and MCP status while replacing the opaque session identifier, response text, tool
arguments, and tool results with SHA-256 digests. The exported verifier object hash matched its
filename.

M2b-M3c repeated the production managed path until L2 and L3 each had three accepted Codex
candidates. Every new run completed the attempt-scoped `read_control` call, produced a distinct
immutable candidate, passed the same negative control and protected oracle, retained bounded usage
evidence, and exported an evidence object whose SHA-256 matched its manifest identifier. During
M2b, a screen-text matcher missed a TUI redraw after the candidate was already committed; the
controller-bound verification command completed that run. This exposed and fixed an unsafe helper
boundary: the command now materializes the exact candidate from controller storage instead of
accepting a caller-supplied candidate directory. M2c, M3b, and M3c then exercised start,
verification, and export entirely through the TUI with journal-based automation.

M3d replaced the unusable digest-supplied MCP submission with a controller-bound operation. The
agent received no source path, base digest, or object digest through the tool schema; after its
checks passed, it called `submit` with only an idempotency key. The controller captured the bound
workspace and committed `agent.submit` as the candidate command. The runtime transcript recorded
successful `read_control` and `submit` calls, and the exact agent-submitted candidate passed the L3
oracle and TUI export checks without invoking the supervisor's post-process fallback.

Accepted `ymp` candidate digests are recorded in
`ymp-rust/tools/ymp-calibration/results/2026-08-12-smoke.json`.

## Promotion rule

A profile advances beyond development smoke testing only after:

1. authentication and exact-version probes pass;
2. at least three fresh repetitions per case pass the same protected oracle;
3. timeout, direct-child and descendant-tree termination are demonstrated;
4. usage and any monetary cost are captured under one declared accounting rule;
5. the attempt-scoped MCP projection is connected without ambient MCP servers or native
   subagents; and
6. the primary corpus, arm budget and exclusion rule are frozen separately.

Claude Code is currently blocked at item 1. Codex has passed the single-run functional ladder, the
descendant-timeout fixture, attempt-scoped MCP reads, and controller-captured private candidates
with committed protected verification at L1-L3. L1, L2, and L3 now each have three accepted
managed repetitions and satisfy the functional repetition threshold. Codex has not yet satisfied
currency-cost accounting, strict Linux containment, or the separate primary-corpus freeze required
for promotion beyond development calibration. Agent-originated candidate submission is now proven
by M3d.

## Live measurements (2026-08-15)

| Profile | Policy | Request | Outcome | Cost (microusd) |
|---|---|---|---|---|
| Claude Code 2.1.232, `claude-opus-5` | `ymp-claude-low-v2` | one live candidate, request never mentioning publication | accepted, candidate published via the product's own submit instruction | 118 783 |
| Claude Code 2.1.232, `claude-opus-5` | `ymp-claude-low-v1` | "create an empty html file" through the public surface (W1-APP-02e), three runs | 1 infrastructure_error (no submit mentioned, no instruction yet), 2 accepted | 46 442 · 60 871 · 60 454 |

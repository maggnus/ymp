---
id: W1-PRD-05k
kind: task
wave: W1
card: W1-PRD-05
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-PRD-05i, W1-PRD-05j]
blocks: [W1-EVL-04a]
created_at: 2026-08-16T12:20:00+08:00
updated_at: 2026-08-16T15:10:00+08:00
started_at: 2026-08-16T12:25:00+08:00
accepted_at: 2026-08-16T15:10:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/b87dedc2ab93c74d7fc05aa85be2ef76930f57d0
closure_commit: https://github.com/maggnus/ymp/commit/30d8e1c39e96f1f9252554050d06c8b3dc36ef78
evidence: ["[b87dedc](https://github.com/maggnus/ymp/commit/b87dedc2ab93c74d7fc05aa85be2ef76930f57d0)"]
duration_minutes: 165
blocker:
pause_reason:
return_trigger:
deliberate_partial: true
---

# W1-PRD-05k — P11: the origin participant starts through the product surface

## Outcome

Authorizing a run in the real product (TUI authorize action, CLI command) starts the origin
participant through the accepted P9 machinery: a production `ParticipantRuntimes` implementation
drives the frozen route, the private workspace is materialized with its own Git initialization,
and the participant's start, yield and finish are visible in the transcript. This closes the gap
between the accepted domain/application mechanics (P9/P10) and the product a real operator runs.

## Scope

### In

- Production `ParticipantRuntimes` over the existing managed drivers (Codex / Claude Code paths
  already admitted by the host), reading profile and route from the frozen entry only.
- The authorize action (TUI) and its mirrored CLI command call `start_origin_participant` after
  run creation; private workspace materialization including its Git init (the supervisor's
  contract), reuse of the W1-APP-02b private-copy path where it fits.
- Budget-edge fix from the P9 review: decide the attempt budget BEFORE creating the private copy
  and charging `participant_starts` — no copy, no charge, no journal side effects when the budget
  refuses.
- Fold the reviewer's two-run route discrimination into the maintained suite (two freezes, one
  live pool, different routes — as a permanent test, not review-only evidence).
- Transcript surfaces the four participant facts already projected by P9 (started/yielded/resumed/
  finished) through existing pages; no new screens (COR-03e owns observatory surfaces).

### Out

- Recruitment through the surface (P10 wiring — separate child), board wiring, TUI observatory,
  live-profile confirmation on the owner host (follows acceptance), any second participant.

## Acceptance

- [x] Authorizing a run through the product surface starts exactly one origin participant on the
      frozen route; the private workspace exists with its own Git init; the start fact, charge and
      transcript line are all present. Fake runtime for the suite.
- [x] Budget-edge negative half: on a run with an exhausted attempt budget the authorization
      refuses BEFORE any copy is created or charge taken — journal byte-identical to a refusal
      from the start, contrasted with the pre-fix tree where CommitmentKernelOpened /
      ParticipantRegistered / RunExhausted appeared with the charge (the P9 review's measurement).
- [x] Two-run route discrimination is a permanent maintained test: two roots with different
      freezes under one live pool start on their respective frozen routes; the pre-discrimination
      suite (single constant) fails this test by construction or the test is shown to distinguish
      them.
- [x] The four participant facts render in the transcript at both sizes through existing
      projections.

## Current state

Accepted after one return round. Round-1 blocker (origin agent process outlived run cancel and
second authorization, continuing to spend the operator's account) fixed at b87dedc: the worker
holds the participant session while the participant lives; all three exit paths run through
`end_origin` with a bounded shutdown (`CONTROLLER_SHUTDOWN_LIMIT`); negative halves measured by
unreturned runtime-session counters (cancel: 1→0, re-auth: 2→1). Lean re-review ACCEPT: reviewer's
own scenarios on the corrected revision (cancel terminates a working participant under 5 s,
participant end precedes run cancel; second authorization leaves exactly one, zero after session
close; a cancel-deaf runtime is bounded and named); mutating away the two `end_origin` calls
reproduces the author's measurements; the four-fact check was not narrowed. Full workspace on the
reviewer's tree: exit 0, 117 suites, 765 tests. Model substitution, boundary-before-driver, and
authorization idempotency falsifiers all held from round 1.

## Next action

Residuals below; the next children are recruitment wiring and the MCP bridge.

## Guardrails

- The kernel never consults the live pool; only the frozen fact (P9 invariant, unchanged).
- Every start spends budget; the budget decision precedes the copy and the charge.
- No new screens; existing projections only.

## Findings

None blocking. Review findings recorded as residuals.

## Closure

### Accepted outcome

The authorize action (TUI) and the mirrored `ymp start` both run through `Session::start_run`:
the origin participant ignites by the accepted P9 mechanics, the route read only from the frozen
record — `ManagedRuntimes` matches a frozen entry to a managed profile by engine AND pinned
model, refusing a different model instead of substituting one. The private copy receives the
supervisor's `PrivateGit` base. The participant runs on its own worker holding the session while
it lives; interface close, run cancel and re-authorization all terminate it through one bounded
path. The attempt-budget decision precedes the copy and the charge: a refused budget writes a
journal byte-identical to a refusal from the start. The two-freeze/one-live-pool route
discrimination is a maintained test. Journal schema v7 unchanged.

### Residuals

1. (minor, outcome-defect, review) `ymp-cli/src/surface.rs:675` discards `OriginShutdown` while
   its comment describes the previous behaviour: slice stop is awaited and the participant is
   ended, but a worker that outlived the bound is not named to the operator. Return trigger: any
   touch of the CLI surface, or the recruitment-wiring child.
2. (additional-work) Igniting the origin consumes the run's single attempt: `/attempt` after a
   successful ignition ends the run as exhausted; the pre-POC-2 manual path remains only when
   ignition was refused. Return trigger: the attempt-budget reconciliation in the wiring child.
3. (additional-work) The origin participant receives no MCP bridge (`mcp: None`) and cannot submit
   a candidate; no resume surface exists. Return trigger: the MCP-bridge child — the immediate
   next node of the frontier.
4. (known limitation) A live-pool leak through a single authorization is undetectable at that
   moment by construction: the freeze and the live pool are identical then. Route retention is
   enforced and tested at the ymp-testkit level.

### Evidence

- [b87dedc](https://github.com/maggnus/ymp/commit/b87dedc2ab93c74d7fc05aa85be2ef76930f57d0) — candidate head, range 4a6e8a0..b87dedc on 78d3423
- [30d8e1c](https://github.com/maggnus/ymp/commit/30d8e1c39e96f1f9252554050d06c8b3dc36ef78) — integration merge — `cargo test -p ymp-application -p ymp-tui` 43 ok-blocks and `cargo check --workspace --all-targets` exit 0 on the merged tree (CTO)
- Reviewer full-suite run: exit 0, 117 suites, 765 tests on the candidate tree

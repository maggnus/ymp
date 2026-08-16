---
id: W1-PRD-05k
kind: task
wave: W1
card: W1-PRD-05
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-PRD-05i, W1-PRD-05j]
blocks: [W1-EVL-04a]
created_at: 2026-08-16T12:20:00+08:00
updated_at: 2026-08-16T12:20:00+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
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

- [ ] Authorizing a run through the product surface starts exactly one origin participant on the
      frozen route; the private workspace exists with its own Git init; the start fact, charge and
      transcript line are all present. Fake runtime for the suite.
- [ ] Budget-edge negative half: on a run with an exhausted attempt budget the authorization
      refuses BEFORE any copy is created or charge taken — journal byte-identical to a refusal
      from the start, contrasted with the pre-fix tree where CommitmentKernelOpened /
      ParticipantRegistered / RunExhausted appeared with the charge (the P9 review's measurement).
- [ ] Two-run route discrimination is a permanent maintained test: two roots with different
      freezes under one live pool start on their respective frozen routes; the pre-discrimination
      suite (single constant) fails this test by construction or the test is shown to distinguish
      them.
- [ ] The four participant facts render in the transcript at both sizes through existing
      projections.

## Current state

Ready. Prerequisites accepted: P9 (ee75419, merged 2726a91), P10 (57f8b9b, merged da1ad18).
Residuals 1–3 of W1-PRD-05i and residual 1 of W1-PRD-05j land here.

## Next action

Dispatch to a builder against the fake-runtime testkit; live-profile confirmation on the owner
host follows acceptance.

## Guardrails

- The kernel never consults the live pool; only the frozen fact (P9 invariant, unchanged).
- Every start spends budget; the budget decision precedes the copy and the charge.
- No new screens; existing projections only.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

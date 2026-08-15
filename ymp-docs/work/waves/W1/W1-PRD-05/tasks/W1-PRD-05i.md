---
id: W1-PRD-05i
kind: task
wave: W1
card: W1-PRD-05
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-PRD-05h, W1-APP-02c, W1-APP-02d, W1-COR-03a]
blocks: [W1-PRD-05j]
created_at: 2026-08-16T09:20:00+08:00
updated_at: 2026-08-16T09:20:00+08:00
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

# W1-PRD-05i — P9: the origin participant starts from the frozen pool

## Outcome

When a run starts, the origin participant (decision D2: the first admissible entry of the frozen
pool) is launched as a real managed attempt against its own private workspace, under the runtime
profile and model route the frozen entry names — never a live-pool lookup. The operator authorizes
the run and sees the participant start in the transcript; the kernel records the start as a
fact; the participant may yield and be resumed. No recruitment (that is P10).

## Scope

### In

- Run start path (`ymp-application`): after `PoolFrozen`, resolve the origin entry from the frozen
  fact, admit it against the frozen containment predicate (already in place), and drive one
  participant attempt through the existing managed-runtime driver interfaces (Codex / Claude Code).
- Kernel facts: participant started (entry, profile, route, workspace), participant yielded /
  resumed / finished; budget charge for the start (participant-starts dimension).
- The attempt runs in a private workspace materialized from the run base, exactly as the
  single-participant path does today (W1-APP-02b).
- Refusal: if the frozen origin entry's runtime is not admitted on this host at start time, the
  run refuses with plain words naming `/runtimes` — the frozen fact stays untouched.
- Screen-level: the transcript and existing pages show the participant state through existing
  projections; no new screens (COR-03e owns observatory surfaces).

### Out

- Recruitment of further participants (P10), board wiring into runs (separate child), any
  collective decision-making, TUI observatory, participant replacement beyond yield/resume.

## Acceptance

- [ ] Authorizing a run whose frozen pool has an admissible origin starts exactly one managed
      participant attempt with the profile/route/workspace from the frozen entry; the journal
      records the start fact; budget participant-starts is charged.
- [ ] Changing the live pool or disabling the provider after freeze does not change which
      profile/route the participant starts with (negative half: on the pre-change tree the start
      consults the live pool — construct the discriminating scenario with the fake runtime).
- [ ] A frozen origin whose runtime is unadmitted at start time refuses the run with plain words;
      the refusal is honest (no partial state, journal intact).
- [ ] Yield/resume of the participant works through the existing lifecycle and never starts a
      second attempt for the same start authorization (idempotency).

## Current state

Ready. Prerequisites accepted: PoolFrozen (W1-PRD-05h), managed drivers (W1-APP-02c/d), budget
accounting (W1-COR-03a). This is the first POC-2 mechanics card: it unblocks the coordinated arm
of the study together with P10.

## Next action

Dispatch to a builder against the fake-runtime testkit first; live-profile confirmation on the
owner host follows acceptance.

## Guardrails

- The kernel never consults the live pool for a run decision; only the frozen fact.
- Participant start is a budgeted, journaled fact — never free.
- No semantic selection: the origin is the recorded first admissible entry (D2), not a "best"
  choice.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

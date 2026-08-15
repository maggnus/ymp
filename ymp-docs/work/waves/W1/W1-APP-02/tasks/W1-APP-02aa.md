---
id: W1-APP-02aa
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02k, W1-PRD-05g]
blocks: []
created_at: 2026-08-16T09:25:00+08:00
updated_at: 2026-08-16T09:25:00+08:00
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

# W1-APP-02aa — Quitting kills a probe that outlives the 5 s bound

## Outcome

Quitting during a provider measurement never leaves the engine process alive, whatever the probe
duration: the worker is joined or the child killed even beyond the existing 5 s bound. The
acceptance test of W1-PRD-05g pinned 2 s because this case was out of its scope (W1-APP-02k
limit); this card closes that limit.

## Scope

### In

- The measurement worker path (`ymp-tui` Session): on shutdown, terminate the probe process tree
  when the join bound expires, using the existing private process-group mechanism; wait for the
  group termination before reporting managed shutdown.
- Acceptance test with a slow fake engine (> 5 s, e.g. 20 s) driven through the real quit path;
  probe by pid file that no process remains.

### Out

- Any change to what is measured or when; worker-pool redesign; runtime drivers.

## Acceptance

- [ ] Quitting during a 20 s probe leaves no engine process after the shutdown sequence completes;
      the shutdown stays bounded (state what bound now holds and how it is enforced).
- [ ] Negative half: on the pre-change tree the same 20 s scenario leaves the process alive
      (measured: «engine remains, exit took 5.7 s, stderr: did not end within 5000 ms» — the
      reviewer's live measurement from the W1-PRD-05g review).

## Current state

Ready. Spawned from the W1-PRD-05g review (major, additional-work) and recorded as its residual.

## Next action

Dispatch after or parallel to P9 with a disjoint write zone (ymp-tui worker path only).

## Guardrails

- Descendant cleanup rules of W1-APP-02k apply in full; no new process-spawning surface.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

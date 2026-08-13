---
id: W1-APP-02p
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02k]
blocks: []
created_at: 2026-08-13T14:57:22+08:00
updated_at: 2026-08-13T14:57:22+08:00
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

# W1-APP-02p — Every program in the launch chain is verified, not only the pinned runtime

## Outcome

The bytes that execute in a managed run are admitted bytes at every step of the chain, not only for
the pinned runtime executable. What the supervisor starts, and whatever that program starts on its
behalf, is verified by digest before it runs.

## Scope

### In

- The launch chain of a managed run, including the shell and environment helpers the supervisor now
  invokes.
- The digest verification already applied to the pinned runtime, extended to cover them.

### Out

- Containment of hostile code, which the POC does not claim.

## Acceptance

- [ ] Every program the supervisor executes is verified by digest before execution; the negative half
      replaces one of them after admission and the run refuses with a captured non-zero exit.
- [ ] The property W1-APP-02c established — executed bytes equal admitted bytes — holds for the whole
      chain, proved by the same substitution test extended to each program.

## Current state

Ready. The independent review of the descendant-ownership work found that a shell and an environment
helper were added to the launch chain without digest verification, while the pinned runtime keeps
its check. This narrows a property an earlier accepted card established.

## Next action

Enumerate the programs the supervisor executes, then verify each by digest on the same path.

## Guardrails

- A helper in the chain is part of the trusted path; leaving it unverified reopens the substitution
  the earlier card closed.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

---
id: W1-APP-02q
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02p]
blocks: []
created_at: 2026-08-13T16:54:11+08:00
updated_at: 2026-08-13T16:54:11+08:00
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

# W1-APP-02q — Admission failures refuse instead of degrading, and ownership is proved beyond file mode

## Outcome

A program that cannot be admitted stops the run instead of degrading it quietly, and the ownership
rule that admits a program cannot be satisfied by a location the running account can still write
through an access-control entry.

## Scope

### In

- The admission path for the process-inspection and signalling utilities, whose failure is currently
  swallowed.
- The ownership rule itself, which today reads the file mode only.
- The exclusion of the fake runtime from launch attestation.

### Out

- The identity rule accepted in W1-APP-02p, which stands; this card removes the ways around it.

## Acceptance

- [ ] A run whose observation utility cannot be admitted refuses to start and says which program and
      why; the negative half places that utility in an unsuitable location and today's build reports
      a clean termination instead, with a captured exit.
- [ ] The ownership rule rejects an ancestor directory that grants write access through an
      access-control entry while its mode looks safe; the negative half constructs exactly that
      directory outside the repository.
- [ ] The fake runtime cannot be selected on a path that produces a candidate in the shipped product,
      proved by a check rather than by convention.

## Current state

Ready. The independent review of the launch chain measured that admission failures for the process
utilities are swallowed — the holder lookup returns an empty list and the process table returns
nothing, so the absence of descendants reads as clean termination — and that the ownership rule
inspects the file mode without looking at access-control entries. Neither is reachable on this
machine; both are reachable on a machine configured differently.

## Next action

Turn each swallowed failure into a refusal, then extend the ownership proof to access-control
entries.

## Guardrails

- A safety check that cannot fail loudly is worse than no check, because its silence reads as
  health.

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

---
id: W1-APP-02q
kind: task
wave: W1
card: W1-APP-02
state: rework
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02p]
blocks: []
created_at: 2026-08-13T16:54:11+08:00
updated_at: 2026-08-13T16:54:11+08:00
started_at: 2026-08-13T18:53:00+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/6220c71dd993800063296cee87e6e70d845acd3d
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

Returned by independent review after one round. Refusal, the widened ownership rule and the reading
of access-control entries were measured working on the built product, including a refusal that names
the entry granting write access. The card does not hold because an internal command still assembles
a candidate outside the supervisor, with neither attestation nor utility admission, and the check
that was to make that unreachable inspects a name rather than the path.

## Next action

Turn each swallowed failure into a refusal, then extend the ownership proof to access-control
entries.

## Guardrails

- A safety check that cannot fail loudly is worse than no check, because its silence reads as
  health.

## Findings

- `blocker`, defect in the contracted outcome. An internal command assembles a candidate without the
  supervisor, and the reviewer produced one with the fake runtime at exit zero. The guarding check
  searches for a seam name and inspects only the attested start.
- `major`, defect in the contracted outcome. The refusal at start guards the attested start alone,
  while another internal command launches a driver directly.
- `minor`, independent product defect. The holder lookup reads the utility's exit code one as "no
  holders" whether or not it failed, and parses output regardless of the exit.
- `minor`, independent product defect. Twenty-eight call sites discard the termination helper's
  result, swallowing the refusal on the cancellation and timeout paths.
- `minor`, defect in the contracted outcome. The structural check excludes both definition files in
  full, which is how the first finding stayed invisible.
- Established and needing no work here: the access-control reader is incomplete rather than
  circular, because an entry high in the tree could replace the reader itself, while ownership and
  mode come from a system call.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

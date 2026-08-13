---
id: W1-APP-02m
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02e.3]
blocks: []
created_at: 2026-08-13T12:40:57+08:00
updated_at: 2026-08-13T12:40:57+08:00
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

# W1-APP-02m — A typed prompt becomes a contract and starts a run

## Outcome

An operator types a request in the interface, answers what the system cannot infer, and a run starts
against a contract that carries a checkable acceptance condition. The product goal's first half —
a person writes a prompt — is executable instead of assumed.

## Scope

### In

- Turning typed text into a managed contract: intent, source directory, and an acceptance condition
  that a verifier can decide.
- Refusing to start when no acceptance condition exists, and saying what is missing.
- The interface path and, under the recorded decision, the equivalent command.

### Out

- Automatic composition of the acceptance condition by a model; the operator supplies or confirms
  it. Multi-participant behaviour, which belongs to W1-COR-03.

## Acceptance

- [ ] Typing a request and confirming produces a stored contract and a started run whose journal
      records both. The negative half: a request with no acceptance condition does not start a run
      and reports exactly what is missing, with a captured non-zero exit.
- [ ] The stored contract carries a verifier reference, its digest and a negative control; a package
      without them is rejected at load.
- [ ] The same result is reachable from the command surface with identical authority checks, and both
      paths write through the kernel writer.

## Current state

Ready. An independent audit found that the path from a typed request to an approved contract belongs
to no stage of the roadmap and to no node of the work tree, while the product goal states it first.
The interface already has the input line and the authorization surface; what is missing is the step
that turns text into a contract with an acceptance condition.

## Next action

Define the smallest contract a run can accept, then make the interface produce it and refuse to
start without it.

## Guardrails

- A contract with no checkable acceptance condition is not a contract; the system says so instead of
  starting a run it cannot judge.
- The kernel does not invent the acceptance condition on the operator's behalf.

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

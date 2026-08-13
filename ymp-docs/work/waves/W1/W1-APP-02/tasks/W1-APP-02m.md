---
id: W1-APP-02m
kind: task
wave: W1
card: W1-APP-02
state: rework
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02e.3]
blocks: []
created_at: 2026-08-13T12:40:57+08:00
updated_at: 2026-08-13T12:40:57+08:00
started_at: 2026-08-13T12:45:00+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/3f420b4f1eea91a8c6ed563b76ca1bc359563130
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
- [ ] One implementation serves both the interface and the command surface that W1-APP-02n will add,
      with no second path into the kernel. The command itself belongs to that node; this card proves
      only that the scenario is shared and that every start path passes the approved-contract check.
- [ ] A store or package written under an incompatible schema version is refused without a single
      write. The negative half compares the bytes of the run summary before and after an attempted
      open.

## Current state

Returned by independent review after one round. The typed path works end to end — a request produced
a stored contract, the journal carried the approval record, and a request without an acceptance
condition started nothing — but opening a store of the previous version rewrites the run summary and
the version check is inverted against the schema document, so the new reader is unreachable. Two
further paths bypass the approved-contract requirement.

## Next action

Define the smallest contract a run can accept, then make the interface produce it and refuse to
start without it.

## Guardrails

- A contract with no checkable acceptance condition is not a contract; the system says so instead of
  starting a run it cannot judge.
- The kernel does not invent the acceptance condition on the operator's behalf.

## Findings

- `blocker`, defect in the contracted outcome. Opening a store written under the previous version
  rewrites the run summary: a running run becomes an infrastructure error and the interface then
  reports that no run was recorded. The journal and the objects survive; the projection does not.
- `blocker`, defect in the contracted outcome. The version check is inverted against the schema
  document, so a package at the new version is rejected and one at the old version accepted.
- `major`, defect in the contracted outcome. A contract supplied on the command line offers an
  enabled start action, but the start path sees only a draft and refuses.
- `major`, independent product defect. The internal smoke command starts a run under the new schema
  without an approved contract and exits zero.
- Defect in this card rather than in the work: the acceptance demanded a command surface that the
  dispatch had assigned to W1-APP-02n. The item has been rewritten to require one shared
  implementation and no second path into the kernel.
- The migration consequence is confirmed by independent measurement: stores of the previous version
  do not open, no migration tool exists, and a new data root is required.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

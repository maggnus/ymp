---
id: W1-APP-02m
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02e.3]
blocks: []
created_at: 2026-08-13T12:40:57+08:00
updated_at: 2026-08-13T12:40:57+08:00
started_at: 2026-08-13T12:45:00+08:00
accepted_at: 2026-08-13T15:06:49+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/690701be4ddbb73c0a74d75b2beae771f6e8954a
closure_commit: https://github.com/maggnus/ymp/commit/bcaabb3d0e6b014f82d5479b944e117f1e1849a1
evidence: [`690701b`](https://github.com/maggnus/ymp/commit/690701be4ddbb73c0a74d75b2beae771f6e8954a)
duration_minutes: 137
blocker:
pause_reason:
return_trigger: the test kit becomes reachable in the built product, observable as its symbol or demo string appearing in the binary or a command exposing it
deliberate_partial: true
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

- [x] Typing a request and confirming produces a stored contract and a started run whose journal
      records both. The negative half: a request with no acceptance condition does not start a run
      and reports exactly what is missing, with a captured non-zero exit.
- [x] The stored contract carries a verifier reference, its digest and a negative control; a package
      without them is rejected at load.
- [x] One implementation serves both the interface and the command surface that W1-APP-02n will add,
      with no second path into the kernel. The command itself belongs to that node; this card proves
      only that the scenario is shared and that every start path passes the approved-contract check.
- [x] A store or package written under an incompatible schema version is refused without a single
      write. The negative half compares the bytes of the run summary before and after an attempted
      open.

## Current state

Accepted with residue and integrated. A typed request produces a stored contract and starts a run,
a request without an acceptance condition starts nothing and names what is missing, a store written
under the previous version is refused without a single byte changed, and a package given on the
command line reaches the same run as a typed request through one scenario.

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
- All four returned defects are closed and independently re-measured, including a byte-for-byte
  comparison of the refused store across thirty-four files.
- `major`, defect in the proof rather than the product, continued as W1-APP-02o: the structural check
  that forbids starting a run outside the scenario truncates a file at the first test marker and
  ignores aliased calls, so two mutations it claims to catch pass unnoticed.
- Residue: the test kit remains an ordinary dependency of the command crate while being unreachable
  in the built product — no command, no symbol and no demo string in either profile. The return
  trigger is recorded in the front matter.

## Closure

Filled when the task is accepted.

### Accepted outcome

The first half of the product goal is executable: a person types a request, supplies the acceptance
condition the system cannot infer, and a run starts against a stored contract that a verifier can
decide. One scenario serves both the interface and the command surface that follows.

### Residuals

The structural guard is weaker than its name: it can be evaded by an aliased call or by placing the
call after the first test marker. The product behaviour it guards was verified independently, so the
gap is in the proof; W1-APP-02o closes it. The test kit stays a dependency of the command crate and
is unreachable in the built product, with the observable return trigger recorded above.

### Evidence

- [`690701b`](https://github.com/maggnus/ymp/commit/690701be4ddbb73c0a74d75b2beae771f6e8954a) —
  reviewed correction.
- [`bcaabb3`](https://github.com/maggnus/ymp/commit/bcaabb3d0e6b014f82d5479b944e117f1e1849a1) —
  integration into the release branch.

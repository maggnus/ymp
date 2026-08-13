---
id: W1-APP-02n
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02m]
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

# W1-APP-02n — Every interface action exists as a command of the same executable

## Outcome

Every action the interface offers — start a run from a prompt, authorize a contract, cancel, export
evidence, apply an accepted candidate — is also reachable as a command of the same `ymp` executable,
with the same authority checks and the same journal path.

## Scope

### In

- A public command surface covering the interface's actions, sharing one implementation with the
  interface rather than duplicating it.
- The authorization and confirmation semantics of irreversible actions on the command path.

### Out

- A daemon, an operator socket, a second state path, or any command that writes state without
  passing the kernel writer.

## Acceptance

- [ ] For every interface action there is a command producing the same journal records; the negative
      half is a command that bypasses the writer and is rejected by a check with a captured non-zero
      exit.
- [ ] An irreversible command requires the same explicit confirmation the interface requires, and
      refuses when it is absent.
- [ ] The command surface adds no capability the interface lacks, proved by an inventory the check
      compares in both directions.

## Current state

Ready. The owner decided that the interface is a convenient way to reach the core rather than the
only way, which supersedes the restriction recorded in accepted subtask W1-APP-02a.2 and resolves the
contradiction an audit found between that subtask and the visual concept.

## Next action

Enumerate the interface's actions, then expose each through one shared implementation.

## Guardrails

- One implementation serves both surfaces; a second code path into the kernel is the failure this
  card must not create.
- No command may weaken a confirmation the interface enforces.

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

---
id: W1-APP-02n
kind: task
wave: W1
card: W1-APP-02
state: rework
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02m]
blocks: []
created_at: 2026-08-13T12:40:57+08:00
updated_at: 2026-08-13T12:40:57+08:00
started_at: 2026-08-13T17:00:00+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/bca2c2627b70af49ab349cd8b557d6fe885fab75
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

Returned by independent review after one round. Both surfaces share one implementation but also one
input channel: argument values are delivered as keystrokes, so an answer beginning with a colon opens
the command palette and later answers land in windows the command never opened. The reviewer started
an irreversible run through a command documented as starting nothing, with no confirmation, while the
exit code reported failure.

## Next action

Enumerate the interface's actions, then expose each through one shared implementation.

## Guardrails

- One implementation serves both surfaces; a second code path into the kernel is the failure this
  card must not create.
- No command may weaken a confirmation the interface enforces.

## Findings

- `blocker`, defect in the contracted outcome. Sharing the interface's key handler as the input
  channel lets a value escape into a second surface: an authorization command with a colon in its
  source value produced a run start and a contract approval with no confirmation, and a request
  command with the same value exited non-zero while the run had started.
- `minor`, defect in the contracted outcome. The inventory compares four enumerations only, so
  actions bound to keys outside them are never listed, the internal namespace is excluded from the
  surplus check, and generated code is invisible to the scanner.
- Refuted: the verifier crate did not change in this range and its tests pass in 2.6 seconds. The
  five-second figure the author reported is a limit inside the test body, not an observed timeout.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

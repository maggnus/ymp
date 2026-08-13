---
id: W1-APP-02k
kind: task
wave: W1
card: W1-APP-02
state: rework
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02d]
blocks: [W1-EVL-04a]
created_at: 2026-08-13T12:09:18+08:00
updated_at: 2026-08-13T12:09:18+08:00
started_at: 2026-08-13T12:44:00+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/48bd5503253b950b8086aa506543281cab3dec80
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02k — No managed descendant survives the supervisor that started it

## Outcome

When a managed run ends for any reason, no process it started is still running, including a
descendant that detached itself into its own session. The comparison can then charge every arm for
exactly the work it did, and no agent process acts after the run that authorized it.

## Scope

### In

- Process-group and session handling in `ymp-rust/crates/ymp-runtime-supervisor` and the two
  runtime drivers it starts.
- A check that observes the real process table after a terminal outcome, for both profiles.

### Out

- Containment of hostile code; this card is about lifecycle, not about a security boundary.
- `ymp-rust/crates/ymp-tui`, which another writer owns.

## Acceptance

- [ ] After every terminal outcome — success, error, cancellation, timeout and budget stop — no
      descendant of the managed process remains, proved by reading the process table rather than by
      the supervisor's own report.
- [ ] The negative half is the measured defect: a child that calls `setsid` and outlives its parent
      is detected and the check fails with a captured non-zero exit on the accepted base.
- [ ] The same evidence is produced for the Codex profile, whose driver shares the supervisor.

## Current state

Returned by independent review after one round. Ownership of a descendant is established by watching
its live ancestors, so a child detached by a double fork, with both intermediate processes gone,
escapes the accounting and survives the run — reproduced three times out of three on the product
path. Bounded termination and the absence of foreign victims were confirmed and are not in doubt.

## Next action

Reproduce the escape on the accepted base, then close it in the supervisor for both drivers.

## Guardrails

- A surviving agent process can spend provider budget and write files after its run was closed, so
  this is not deferred behind a convenience trigger.
- Termination evidence comes from the operating system, never from the component being tested.

## Findings

- `blocker`, defect in the contracted outcome. Ordinary daemonization defeats ancestor-based
  ownership: a double-forked descendant whose intermediate processes exit immediately is never
  recorded, and the product reports success while the process keeps running. Ownership must rest on
  a property the descendant itself carries.
- `minor`, defect in the contracted outcome. The evidence record understates the negative half: the
  suite fails eleven of eleven with the watcher disabled, not zero.
- Confirmed and not in question: termination stays time-bounded, and a session leader the run did
  not start survives untouched.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

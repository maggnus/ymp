---
id: W1-APP-02k
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02d]
blocks: [W1-EVL-04a]
created_at: 2026-08-13T12:09:18+08:00
updated_at: 2026-08-13T12:09:18+08:00
started_at: 2026-08-13T12:44:00+08:00
accepted_at: 2026-08-13T14:57:22+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/9544db2cc851db7dcd25c0c1f50fd87de51201c1
closure_commit: https://github.com/maggnus/ymp/commit/78c767cab279dd35003cfe6dcdca0884559b08ae
evidence: [`9544db2`](https://github.com/maggnus/ymp/commit/9544db2cc851db7dcd25c0c1f50fd87de51201c1)
duration_minutes: 131
blocker:
pause_reason:
return_trigger: a program appears on the product path that closes descriptors above the second while daemonizing, including as ordinary hygiene
deliberate_partial: true
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

- [x] After every terminal outcome — success, error, cancellation, timeout and budget stop — no
      descendant of the managed process remains, proved by reading the process table rather than by
      the supervisor's own report.
- [x] The negative half is the measured defect: a child that calls `setsid` and outlives its parent
      is detected and the check fails with a captured non-zero exit on the accepted base.
- [x] The same evidence is produced for the Codex profile, whose driver shares the supervisor.

## Current state

Accepted with residue and integrated. Ownership of a descendant now rests on an inherited marker the
process carries itself, so the escape by double fork is closed; the reviewer verified on the built
product that the marker survives a shell wrapper, an exec chain and a full environment rewrite, that
termination stays time-bounded, and that a session leader the run did not start survives untouched.

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
- The returned blocker is closed and independently re-measured: a double-forked, immediately orphaned
  descendant is detected and terminated on the product path.
- `major`, independent product defect, carried as residue against the unenforced isolation invariant:
  a descendant that discards every inherited property is not detected. The POC makes no containment
  claim and this card excludes hostile code from its scope, so this is a boundary of the experiment
  rather than a defect of the contracted outcome. The return trigger is deliberately wider than
  evasion and is recorded in the front matter.
- `minor`, independent product defect: a shell and an environment helper entered the launch chain
  without digest verification while the pinned runtime keeps its check, narrowing a property an
  earlier accepted card established. Continued as W1-APP-02p.

## Closure

Filled when the task is accepted.

### Accepted outcome

After every terminal outcome, no process a managed run started is still running, including one that
detached itself by ordinary daemonization, proved by reading the process table rather than the
supervisor's own report, for both runtime profiles.

### Residuals

A descendant that deliberately discards every inherited property escapes ownership and is not
detected. This is the containment boundary the POC explicitly does not claim, and the reviewer found
no evidence against that classification. The return trigger is written wider than deliberate evasion,
because ordinary daemonization hygiene would produce the same effect.

### Evidence

- [`9544db2`](https://github.com/maggnus/ymp/commit/9544db2cc851db7dcd25c0c1f50fd87de51201c1) —
  reviewed correction.
- [`78c767c`](https://github.com/maggnus/ymp/commit/78c767cab279dd35003cfe6dcdca0884559b08ae) —
  integration into the release branch.

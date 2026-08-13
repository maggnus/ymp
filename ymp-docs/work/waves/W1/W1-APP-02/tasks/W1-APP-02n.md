---
id: W1-APP-02n
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02m]
blocks: []
created_at: 2026-08-13T12:40:57+08:00
updated_at: 2026-08-13T12:40:57+08:00
started_at: 2026-08-13T17:00:00+08:00
accepted_at: 2026-08-13T19:12:44+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/7edd3f6232ce4e9111dbc43d86e994ce7a6ec492
closure_commit: https://github.com/maggnus/ymp/commit/57dc051c4d24e3753766375711b7649d57a89ac3
evidence: [`7edd3f6`](https://github.com/maggnus/ymp/commit/7edd3f6232ce4e9111dbc43d86e994ce7a6ec492)
duration_minutes: 112
blocker:
pause_reason:
return_trigger: a re-export of the key channel under another name appears in the terminal crate, which the structural check would not reject
deliberate_partial: true
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

- [x] For every interface action there is a command producing the same journal records; the negative
      half is a command that bypasses the writer and is rejected by a check with a captured non-zero
      exit.
- [x] An irreversible command requires the same explicit confirmation the interface requires, and
      refuses when it is absent.
- [x] The command surface adds no capability the interface lacks, proved by an inventory the check
      compares in both directions.

## Current state

Accepted with a recorded limitation and integrated. Every interface action has a command, and an
answer no longer travels through the interface's input row: it is handed to the session as the same
local move the event loop performs for a completed line, no key is synthesised anywhere, and the
terminal key library is now only a development dependency. Twelve runs with a colon, a path
separator, an escape and a control character in every answer slot refuse without writing a journal.

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
- The blocker is closed and independently re-measured on the built product; the structural check
  falls when the key channel is returned either directly or under an alias.
- `minor`, defect in the contracted outcome, carried as a limitation: a call through a re-export
  under another name passes the structural check. This is the same class already recorded for the
  writer check, and the return trigger is in the front matter.
- `minor`, defect in the contracted outcome: the key enumeration is fixed — printable ASCII, two
  control codes, eighteen codes across four modifiers — so function keys beyond that range,
  non-ASCII input and compound modifiers are outside it. It grants no surplus, because the key
  handler is pure over the view state and the action variants are matched exhaustively.
- Of the three uncovered areas the author listed, one is reachable: the internal namespace holds six
  commands pinned by name and never matched against the interface. The other two are hypothetical on
  this revision.

## Closure

Filled when the task is accepted.

### Accepted outcome

Every action the interface offers is reachable as a command of the same executable, through one
implementation and one journal path, with the same confirmation for an irreversible action and no
capability the interface lacks.

### Residuals

A call made through a re-export under another name is invisible to the structural check, so the
guard is narrower than its name. The behaviour it guards was measured directly on the built product;
the gap is in the proof. The return trigger is recorded in the front matter, and the six internal
commands remain pinned by name rather than matched against the interface.

### Evidence

- [`7edd3f6`](https://github.com/maggnus/ymp/commit/7edd3f6232ce4e9111dbc43d86e994ce7a6ec492) —
  reviewed correction.
- [`57dc051`](https://github.com/maggnus/ymp/commit/57dc051c4d24e3753766375711b7649d57a89ac3) — integration into the release branch.

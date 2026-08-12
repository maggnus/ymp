---
id: W1-APP-02a.1
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02a
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-12T16:35:41+08:00
updated_at: 2026-08-12T19:38:22+08:00
started_at: 2026-08-12T19:03:00+08:00
accepted_at: 2026-08-12T19:38:22+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/df9ac0c621e937ea643829a2d406a889cf1be528
closure_commit: https://github.com/maggnus/ymp/commit/d5c89fd63bb75274232353f72757a572e46121df
evidence: ["[df9ac0c](https://github.com/maggnus/ymp/commit/df9ac0c621e937ea643829a2d406a889cf1be528)", "[d5c89fd](https://github.com/maggnus/ymp/commit/d5c89fd63bb75274232353f72757a572e46121df)"]
duration_minutes: 35
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02a.1 — Recovered command identifiers preserve one exact result

## Outcome

After application recovery, each command identifier remains bound to one command digest and one
exact recorded result; a conflicting reuse fails before state application, event creation, or
resource charging.

## Scope

### In

- Recovery of the command-result index from committed events.
- Digest comparison for repeated command identifiers before `state.apply`.
- Persistence and return of the exact original command result across reopening.
- Regression tests for later terminal-state changes and cryptographically consistent conflicting
  journal entries.

### Out

- Public CLI command shape, candidate isolation, runtime profiles, schema-version changes, and TUI
  behavior.

## Acceptance

- [x] A command repeated after reopening returns the exact result recorded for its first execution,
  even when later events changed the current run state.
- [x] A journal containing one `command_id` with two different command digests is rejected as a
  typed infrastructure error before the second command changes state or charges a budget.
- [x] The unchanged live-controller replay, cursor recovery, corruption, and single-writer tests
  continue to pass.

## Current state

Accepted. Recovery preserves the exact initial command result, rejects a conflicting digest before
the second effect, and terminalizes the run without retaining active attempt authority.

## Next action

Continue the parent closure process with independent review of W1-APP-02a.2.

## Guardrails

- Limit changes to `ymp-application` and package-local recovery tests unless a separately reviewed
  contract change is required.
- Preserve schema version 1 and all accepted one-writer and corruption behavior.

## Findings

- Candidate [acc5586](https://github.com/maggnus/ymp/commit/acc55860f53501c4b5d35b3a5f934f28e12ec998)
  retains `active_attempts` after terminal `InfrastructureError`, violating the terminal-authority
  invariant.
- The reviewer-owned external check incorrectly requires `active_attempts` to remain unchanged on
  terminalization; the reviewer must correct that assertion before re-review.

## Closure

### Accepted outcome

The reviewed candidate was integrated byte-equivalently. Two external scenarios with negative
controls confirmed exact-result recovery and valid conflict terminalization; all package recovery
tests passed on the reviewed revision.

### Residuals

None.

### Evidence

- [Reviewed candidate df9ac0c](https://github.com/maggnus/ymp/commit/df9ac0c621e937ea643829a2d406a889cf1be528)
- [Integrated revision d5c89fd](https://github.com/maggnus/ymp/commit/d5c89fd63bb75274232353f72757a572e46121df)

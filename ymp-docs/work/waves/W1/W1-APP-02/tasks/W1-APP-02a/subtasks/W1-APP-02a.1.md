---
id: W1-APP-02a.1
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02a
state: review
risk: critical
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-12T16:35:41+08:00
updated_at: 2026-08-12T19:15:11+08:00
started_at: 2026-08-12T19:03:00+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/acc55860f53501c4b5d35b3a5f934f28e12ec998
closure_commit:
evidence:
duration_minutes: 12
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

- [ ] A command repeated after reopening returns the exact result recorded for its first execution,
  even when later events changed the current run state.
- [ ] A journal containing one `command_id` with two different command digests is rejected as a
  typed infrastructure error before the second command changes state or charges a budget.
- [ ] The unchanged live-controller replay, cursor recovery, corruption, and single-writer tests
  continue to pass.

## Current state

Candidate [acc5586](https://github.com/maggnus/ymp/commit/acc55860f53501c4b5d35b3a5f934f28e12ec998)
passes the fixed external recovery check and package checks. Critical independent review is queued
against the exact candidate and the reviewer-owned falsifier.

## Next action

Review the exact candidate with the preserved external recovery check and an independently selected
conflict or replay falsifier.

## Guardrails

- Limit changes to `ymp-application` and package-local recovery tests unless a separately reviewed
  contract change is required.
- Preserve schema version 1 and all accepted one-writer and corruption behavior.

## Findings

- A repeated identifier currently returns current state rather than the original result.
- Recovery currently applies a second event with the same identifier and different command bytes.

## Closure

Filled when the subtask is accepted.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.

---
id: W1-COR-03f
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03a]
blocks: []
created_at: 2026-08-13T16:02:47+08:00
updated_at: 2026-08-13T16:02:47+08:00
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

# W1-COR-03f — Commitment facts are durable and visible, not only in memory

## Outcome

The facts a commitment produces — contract, escrow transfer, lease, obligation, return and terminal
outcome — survive a restart and appear on the operator's screen, so a run's commitments can be
audited from the record rather than from a live process.

## Scope

### In

- Journalling the commitment facts with their own durable records, and the schema version that
  requires.
- The terminal projection that shows commitments and obligations.

### Out

- The commitment rules themselves, which W1-COR-03a owns, and any change to who decides what.

## Acceptance

- [ ] Every fact the commitment kernel produces is written to the journal before the command result
      returns; the negative half removes one write and a restart then loses that fact, with a
      captured non-zero exit.
- [ ] A restart reconstructs the registry from the journal alone, with no live state carried over.
- [ ] The operator sees commitments and obligations on a page whose every value derives from an
      application projection, and the existing framework and state-binding checks still hold.

## Current state

Ready. The commitment kernel decides and records its facts in memory only, because journalling them
requires a schema version the card could not raise and a terminal projection in a crate another
writer owned. Both zones are free once the current cards land.

## Next action

Raise the durable record to carry commitment facts, then project them into the interface.

## Guardrails

- A fact that decides money or completion is written before its result is returned, never after.

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

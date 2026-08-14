---
id: W1-COR-03u
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03s]
blocks: []
created_at: 2026-08-15T01:55:01+08:00
updated_at: 2026-08-15T01:55:01+08:00
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

# W1-COR-03u — The application recovers its memory from the journal after an interrupted apply

## Outcome

After an interrupted apply, the application replays the journal and subsequent commands succeed; the negative half is the current all-commands-refused state.

## Scope

### In

- Recorded from the W1-COR-03s review. A panic between the whole-fact append and the in-memory apply desynchronizes the sequence: every later command of the run is refused until restart. The refusal now surfaces (03q), but recovery requires replaying the journal into memory.

### Out

- What the sibling terminal-edge nodes accepted.

## Acceptance

- [ ] After an interrupted apply, the application replays the journal and subsequent commands succeed; the negative half is the current all-commands-refused state.

## Current state

Ready.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

None recorded.

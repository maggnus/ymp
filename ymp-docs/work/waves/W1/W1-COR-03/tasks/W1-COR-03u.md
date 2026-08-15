---
id: W1-COR-03u
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03s]
blocks: []
created_at: 2026-08-15T01:55:01+08:00
updated_at: 2026-08-15T14:38:39+08:00
started_at: 2026-08-15T14:08:53+08:00
accepted_at: 2026-08-15T14:38:39+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/6544e25c2f671a50c5d87ef3d741b943173fd9a5
closure_commit: https://github.com/maggnus/ymp/commit/2916b21de0554019a7fc24aa2b6970860cc054ee
evidence: ["[2916b21](https://github.com/maggnus/ymp/commit/2916b21de0554019a7fc24aa2b6970860cc054ee)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger: a consumer that reads manifest.json without recomputing state from events.jsonl appears; or read-only paths (state, export_evidence, apply_candidate) must show the journal-current projection before the next command
deliberate_partial: true
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

- reviewer swept every interruption point including the first command and a three-fact gap: replay
  never re-executes, the journal stays byte-identical, in-run recovery equals a reopened store;
  residuals: an export during desync carries a stale manifest summary beside a complete journal, and
  read-only paths lag until the next command

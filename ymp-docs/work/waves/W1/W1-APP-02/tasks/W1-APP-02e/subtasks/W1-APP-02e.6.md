---
id: W1-APP-02e.6
kind: subtask
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02e]
blocks: []
created_at: 2026-08-15T01:52:53+08:00
updated_at: 2026-08-15T01:52:53+08:00
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

# W1-APP-02e.6 — Runtime engines are managed entities with properties and model lists

## Outcome

Engines live in a registry under the product root (runtimes/<engine>.json): enabled flag, measured properties (executable, version, credential origin, budget bounds) and the list of models the engine can serve, filled by probe. A disabled engine is not admitted and not offered. Enable/disable from /runtimes and the mirrored command. Admission semantics (tools, permission mode, no delegation, budget ceilings) stay hard-coded; WHICH models may be used stays undecided until the W1-EVL-04d research lands.

## Scope

### In

- See outcome; zones per the approved plan of 2026-08-15.

### Out

- Everything accepted by the parent and sibling nodes.

## Acceptance

- [ ] Engines live in a registry under the product root (runtimes/<engine>.json): enabled flag, measured properties (executable, version, credential origin, budget bounds) and the list of models the engine can serve, filled by probe. A disabled engine is not admitted and not offered. Enable/disable from /runtimes and the mirrored command. Admission semantics (tools, permission mode, no delegation, budget ceilings) stay hard-coded; WHICH models may be used stays undecided until the W1-EVL-04d research lands.

## Current state

Ready. Owner decision 2026-08-15: manage runtime engines like Paseo does — switchable, with properties and at least the model list they provide.

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

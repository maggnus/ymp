---
id: W1-EVL-04d
kind: task
wave: W1
card: W1-EVL-04
state: accepted
risk: significant
maturity: RESEARCH
relation: required
depends_on: []
blocks: []
created_at: 2026-08-15T01:52:53+08:00
updated_at: 2026-08-15T15:08:23+08:00
started_at: 2026-08-15T01:52:53+08:00
accepted_at: 2026-08-15T15:08:23+08:00
candidate_commit:
closure_commit: https://github.com/maggnus/ymp/commit/4b58ecda4c311a7fae25d006227efc09b6e5ca98
evidence: ["[4b58ecd](https://github.com/maggnus/ymp/commit/4b58ecda4c311a7fae25d006227efc09b6e5ca98)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-EVL-04d — The model-use policy is decided before pools form

## Outcome

A verified recommendation, presented to the owner before any model selection is implemented in the
draft: where the permitted model set for a run is declared (the contract at first request, the
engine registry, or above), and how it composes with participant-pool formation in POC-2 —
including the automatic-pool case the owner flagged as harder. The recommendation weighs cost,
reproducibility of the matched-budget comparison (W1-EVL-04a), and honest per-model spend
attribution (W1-APP-02y). Refuting a candidate mechanism is a result.

## Scope

### In

- Options analysis over the accepted surfaces: contract document, engine registry (W1-APP-02e.6),
  charter-level defaults; pool formation sketches for POC-2.

### Out

- Any implementation; arbitrary-model use, which the owner ruled unreasonable.

## Acceptance

- [x] A written recommendation with named options, their consequences and one preferred mechanism,
      traceable to the accepted evidence; the owner decides on it.

## Current state

Accepted by owner decision D1: permitted models are named per task, frozen as a run-start snapshot,
and the collective forms its team from that set; no semantic pool entity ranks models.

## Next action

No further action; implementation consumers use the accepted per-task snapshot rule.

## Guardrails

None recorded.

## Findings

None.

## Closure

### Accepted outcome

The operator declares the permitted models per task; the set freezes at run start, and participant
formation selects only from that snapshot without a model-ranking pool.

### Residuals

None.

### Evidence

- [4b58ecd](https://github.com/maggnus/ymp/commit/4b58ecda4c311a7fae25d006227efc09b6e5ca98)
  — owner decision D1 and its durable design record.

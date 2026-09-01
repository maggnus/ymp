---
id: W1-EVL-04
kind: card
wave: W1
state: ready
risk: critical
maturity: RESEARCH
relation: required
depends_on: [W1-COR-03z, W1-PRD-05j.1]
blocks: []
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-09-01T13:42:00+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
---

# W1-EVL-04 — Controlled evidence decides both POC hypotheses

## Outcome

Preregistered, matched-budget evidence supports an explicit decision about instrumental
reliability and a separate decision about causal signs of collective reasoning.

## Invariants

- The coordinated condition is compared with both a strong single participant and independent
  best-of-`n` under the same declared budget.
- Acceptance uses the exact prevalidated oracle and includes blinded human audit where declared.
- Communication claims require message interventions; transcript fluency is not an outcome.
- Primary outcomes, exclusions, and stopping rules do not change after results are visible.
- A negative or inconclusive result is retained and can stop the mechanism claim.
- L1–L3 calibrate profiles and oracles only; held-out weak diagnostic and strong-profile transfer
  are development gates, never primary observations.

## Scope

This card owns pre-primary weak diagnostic and transfer gates, then `POC-3`: execution of the frozen
instrumental arms, causal message interventions, analysis with uncertainty by task stratum,
false-acceptance audit, and the final POC decision record. Development evidence can stop the path
but cannot satisfy primary acceptance.

It does not implement MVP features, tune the protocol after seeing primary results, or generalize
from the selected corpus to arbitrary projects, cognition, consciousness, or a group mind.

## Aggregate acceptance

All six required tasks are accepted. The final report can be reproduced from frozen inputs and
records one of the permitted decisions without changing the preregistered comparison: proceed for
the supported stratum, reject the reliability mechanism, reject the collective-reasoning claim,
or invalidate the experiment because oracle or execution integrity failed.

## Tasks

- [W1-EVL-04a](tasks/W1-EVL-04a.md) — required
- [W1-EVL-04b](tasks/W1-EVL-04b.md) — required
- [W1-EVL-04c](tasks/W1-EVL-04c.md) — required
- [W1-EVL-04d](tasks/W1-EVL-04d.md) — required
- [W1-EVL-04e](tasks/W1-EVL-04e.md) — required
- [W1-EVL-04f](tasks/W1-EVL-04f.md) — required

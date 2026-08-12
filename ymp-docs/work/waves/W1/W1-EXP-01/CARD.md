---
id: W1-EXP-01
kind: card
wave: W1
state: blocked
risk: critical
maturity: DESIGN
relation: required
depends_on: []
blocks: [W1-APP-02]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-10T19:34:25+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker: Owner gate G2; task W1-EXP-01d also requires G3
pause_reason:
return_trigger:
---

# W1-EXP-01 — POC assumptions are falsifiable before product code

## Outcome

The task corpus, acceptance oracles, comparison design, protocol model, and runtime probes can
invalidate the POC before implementation cost is committed to an uninterpretable experiment.

## Invariants

- Public requirements and protected cases remain distinct and digest-addressed.
- A negative control that passes invalidates the oracle rather than accepting a candidate.
- All experimental arms use the same declared total resource budget and stopping rule.
- Protocol safety and termination claims are exercised against generated fault schedules.
- Runtime compatibility is measured for exact profiles; a product name is not evidence of
  lifecycle or accounting conformance.

## Scope

This card owns `POC-0`: curated contracts and oracles, preregistration, executable protocol
falsification, fake-runtime fixtures, and exact Codex and Claude Code capability probes. It does
not build the user-facing runtime or broaden the supported project class.

## Aggregate acceptance

All four required tasks are accepted. At least one oracle mutation, one protocol mutation, and one
runtime incompatibility are observed to fail through the same checks intended for the POC. The
comparison can be executed without changing its primary outcome or budget rule after results are
visible.

## Tasks

- [W1-EXP-01a](tasks/W1-EXP-01a.md) — required
- [W1-EXP-01b](tasks/W1-EXP-01b.md) — required
- [W1-EXP-01c](tasks/W1-EXP-01c.md) — required
- [W1-EXP-01d](tasks/W1-EXP-01d.md) — required

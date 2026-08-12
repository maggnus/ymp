---
id: W1-EXP-01b
kind: task
wave: W1
card: W1-EXP-01
state: accepted
risk: critical
maturity: RESEARCH
relation: required
depends_on: [W1-EXP-01a]
blocks: [W1-EXP-01d, W1-EVL-04a]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T17:44:58+08:00
started_at: 2026-08-12T15:44:08+08:00
accepted_at: 2026-08-12T17:44:58+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/d12eff31940f8ad124f5f8f566a0dd21de3d2ae6
closure_commit: https://github.com/maggnus/ymp/commit/d12eff31940f8ad124f5f8f566a0dd21de3d2ae6
evidence: ["[d12eff3](https://github.com/maggnus/ymp/commit/d12eff31940f8ad124f5f8f566a0dd21de3d2ae6)"]
duration_minutes: 102
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-EXP-01b — Matched-budget study has a frozen decision rule

## Outcome

A preregistered study specification fixes the three experimental conditions, task strata,
resource accounting, replication, exclusions, stopping rule, primary outcomes, audit sample, and
minimum practically useful effect before any ymp outcome is observed.

## Scope

### In

- One strong participant, independent best-of-`n` with blinded selection, and locally coordinated
  ymp conditions under the same total model-route and protected-query budget.
- Randomization unit, stochastic repetitions, decomposability strata, budget-overshoot policy,
  false-acceptance audit, uncertainty reporting, exclusions, and permitted terminal outcomes.
- Separate instrumental-reliability and communication-intervention analyses.

### Out

- Choosing a preferred protocol topology, optimizing on POC results, or treating a single run as
  evidence of general collective intelligence.
- Implementation of the runtime, coordination protocol, or analysis pipeline.

## Acceptance

- [x] A versioned study manifest and human-readable protocol completely determine arm assignment,
  equal resource opportunity, primary outcomes, exclusions, stopping, and the final decision rule.
  An optional currency ceiling may be configured only for the project as a whole; accounting and
  statistical treatment are fixed before primary outcomes are observed.
- [x] A dry run over synthetic records produces the declared analysis without consulting mutable
  defaults or post-outcome configuration.
- [x] Deliberately changing one arm's budget, exclusion rule, or primary outcome after the manifest
  is frozen causes the compliance check to fail rather than producing a comparable result.
- [x] The protocol permits a negative or inconclusive result to reject the mechanism claim without
  redefining success.

## Current state

Accepted. The frozen manifest binds every condition to the approved corpus, profile, model route,
protected-result sequence, deterministic seed, assignment order, and assignment position. The
statistical thresholds and resource budget are immutable inputs to the compliance check.

## Next action

Use the accepted manifest as the immutable accounting and comparison contract for W1-EXP-01d and
W1-EVL-04a.

## Guardrails

- Experimental outcomes cannot alter the primary contrast, minimum useful effect, or stopping
  rule.
- Communication metrics never affect resource allocation or candidate acceptance in the measured
  run.

## Findings

None. The four findings from the first review were corrected and independently rechecked through
the preserved external black-box test.

## Closure

### Accepted outcome

The preregistered study fixes three comparable conditions, separate task strata, five repetitions,
best-of-11 selection, the complete token/time/query/cost accounting vector, exclusions, stopping,
primary outcomes, and a minimum useful effect of 0.125. Synthetic analysis succeeds only when the
corpus, route, profile, assignment, disclosure order, and frozen decision data all agree.

### Residuals

None recorded.

### Evidence

- [Reviewed-equivalent integration](https://github.com/maggnus/ymp/commit/d12eff31940f8ad124f5f8f566a0dd21de3d2ae6).

---
id: W1-EXP-01b
kind: task
wave: W1
card: W1-EXP-01
state: rework
risk: critical
maturity: RESEARCH
relation: required
depends_on: [W1-EXP-01a]
blocks: [W1-EXP-01d, W1-EVL-04a]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T16:58:01+08:00
started_at: 2026-08-12T15:44:08+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/a410dc5d32a39880e3bd9c8110637d4a8c70af0a
closure_commit:
evidence:
duration_minutes: 0
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

- [ ] A versioned study manifest and human-readable protocol completely determine arm assignment,
  equal resource opportunity, primary outcomes, exclusions, stopping, and the final decision rule.
  An optional currency ceiling may be configured only for the project as a whole; accounting and
  statistical treatment are fixed before primary outcomes are observed.
- [ ] A dry run over synthetic records produces the declared analysis without consulting mutable
  defaults or post-outcome configuration.
- [ ] Deliberately changing one arm's budget, exclusion rule, or primary outcome after the manifest
  is frozen causes the compliance check to fail rather than producing a comparable result.
- [ ] The protocol permits a negative or inconclusive result to reject the mechanism claim without
  redefining success.

## Current state

The first independent review returned the candidate for bounded rework. The statistical design,
power calculation, resource accounting, and negative-result handling are present, but the
compliance command still admits four inconsistent or incompletely recorded study variants.

## Next action

Bind each condition to its frozen model route, record and verify protected-result timing, validate
the exact initial seed and order, and require an explicit corpus path with its digest. Rerun the
preserved external falsifier before re-review by the same reviewer.

## Guardrails

- Experimental outcomes cannot alter the primary contrast, minimum useful effect, or stopping
  rule.
- Communication metrics never affect resource allocation or candidate acceptance in the measured
  run.

## Findings

Review of
[`study.rs`](https://github.com/maggnus/ymp/blob/a410dc5d32a39880e3bd9c8110637d4a8c70af0a/ymp-rust/tools/ymp-corpus/src/study.rs)
and
[`main.rs`](https://github.com/maggnus/ymp/blob/a410dc5d32a39880e3bd9c8110637d4a8c70af0a/ymp-rust/tools/ymp-corpus/src/main.rs)
found:

- Different model routes can be recorded in one matched block without rejection.
- Early disclosure of a protected result is accepted because the event sequence is absent.
- Arbitrary initial seeds and reversed assignment order pass permutation-only validation.
- `study-dry-run` accepts an implicit corpus path that is not bound by digest.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

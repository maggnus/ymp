---
id: W1-EXP-01b
kind: task
wave: W1
card: W1-EXP-01
state: review
risk: critical
maturity: RESEARCH
relation: required
depends_on: [W1-EXP-01a]
blocks: [W1-EXP-01d, W1-EVL-04a]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T17:35:00+08:00
started_at: 2026-08-12T15:44:08+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/141cbf370a575736a8f7f7d21539ac5f121e7fcd
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

The corrected candidate is under repeat review. It binds every condition to the frozen corpus,
profile, model route, protected-result sequence, deterministic seed, assignment order, and
assignment position. The statistical thresholds and resource budget are unchanged.

## Next action

Complete the preserved external black-box review of the correction and integrate it only after an
independent `ACCEPT`.

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

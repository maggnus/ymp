---
id: W1-EXP-01b
kind: task
wave: W1
card: W1-EXP-01
state: blocked
risk: critical
maturity: RESEARCH
relation: required
depends_on: [W1-EXP-01a]
blocks: [W1-EXP-01d, W1-EVL-04a]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-10T19:34:25+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker: Owner gates G1 and G2, followed by acceptance of W1-EXP-01a
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
  total budget matching, primary outcomes, exclusions, stopping, and the final decision rule.
- [ ] A dry run over synthetic records produces the declared analysis without consulting mutable
  defaults or post-outcome configuration.
- [ ] Deliberately changing one arm's budget, exclusion rule, or primary outcome after the manifest
  is frozen causes the compliance check to fail rather than producing a comparable result.
- [ ] The protocol permits a negative or inconclusive result to reject the mechanism claim without
  redefining success.

## Current state

The required comparisons are described in `ROADMAP.md`, but no approved corpus, machine-readable
study manifest, effect threshold, or budget is frozen. Work is blocked by `G1` and `G2`.

## Next action

After `W1-EXP-01a` defines the admitted corpus, record the exact randomization and decision rule.

## Guardrails

- Experimental outcomes cannot alter the primary contrast, minimum useful effect, or stopping
  rule.
- Communication metrics never affect resource allocation or candidate acceptance in the measured
  run.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

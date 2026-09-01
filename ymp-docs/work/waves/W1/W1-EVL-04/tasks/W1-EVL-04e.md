---
id: W1-EVL-04e
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: OPERATIONALIZATION
relation: required
depends_on: [W1-EXP-01e, W1-COR-03z, W1-PRD-05j.1]
blocks: [W1-EVL-04f, W1-EVL-04a]
created_at: 2026-09-01T13:41:52+08:00
updated_at: 2026-09-01T13:41:52+08:00
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
review_rounds: 0
escalation_decision:
---

# W1-EVL-04e — Weak diagnostic runner stops before primary evidence

## Outcome

One admitted weak profile completes a development-only matched-budget comparison of single,
blinded best-of-2 and coordinated arms on the frozen L4+ ladder, producing a compliant stop/go
decision that is not primary evidence.

## Scope

### In

- Diagnostic manifest, hash-derived condition ordering, scheduler, blinded best-of-2 selector,
  exact resource accounting, compliance record, exclusions, and the frozen `weak-diagnostic-v1`
  decision rule.
- Deterministic fake-runtime rehearsal before any model call, followed by one exact admitted weak
  profile cohort across the held-out decomposable and sequential/null strata.
- Candidate, usage, message, delivery, selector commitment, verifier, cost, failure and abstention
  records under separate diagnostic seeds.

### Out

- L1–L3 calibration observations as experimental arms, primary tasks/seeds/outcomes, message
  interventions, strong-profile transfer, protocol tuning after results, or claims of reliability.

## Acceptance

- [ ] A zero-model fake-runtime run exercises every arm, budget field, selector commitment,
      protected query and stop branch; budget mismatch, early arm disclosure, missing usage,
      duplicate condition, unavailable oracle or undeclared exclusion is rejected.
- [ ] The real cohort runs every frozen task/repetition/arm exactly once in hash-derived order with
      the same profile, route, total resource vector and one protected query per condition.
- [ ] The selector commits before arm identity, messages or producer rationale are revealed; a
      deliberately early reveal makes the compliance check fail.
- [ ] The diagnostic decision compares coordination with both controls by stratum, requires the
      predefined minimum attributable non-redundant contributions, and deterministically records
      `advance`, `no diagnostic support`, or `invalid` without changing thresholds.
- [ ] No primary seed, task, protected-query reservation or result is consumed; all behavior runs
      under a fresh disposable evaluation root and exact admitted profile record.

## Current state

The protocol is specified, and `publish/read_board` is accepted, but recruitment, the L4+ corpus,
diagnostic scheduler, selector and compliance runner are absent. No weak model observation has been
admitted or paid for.

## Next action

After W1-EXP-01e and W1-PRD-05j.1, implement and prove the complete fake-runtime path before opening
any model or monetary gate.

## Guardrails

- A development diagnostic can stop primary work but can never count toward the primary estimate.
- Failure to beat both single and independent controls stops mechanism deepening.
- Infrastructure failure, candidate failure and oracle invalidity remain distinct outcomes.

## Findings

- Split from W1-EVL-04a because weak development evidence cannot close a frozen primary-comparison
  task.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

## Closure

Filled when the task is accepted. Until then this section stays as written.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger. Empty when there are none.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.

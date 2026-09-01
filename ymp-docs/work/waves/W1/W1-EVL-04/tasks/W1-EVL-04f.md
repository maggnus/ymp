---
id: W1-EVL-04f
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: RESEARCH
relation: required
depends_on: [W1-EVL-04e, W1-EXP-01e]
blocks: [W1-EVL-04a, W1-EVL-04b]
created_at: 2026-09-01T13:41:53+08:00
updated_at: 2026-09-01T13:41:53+08:00
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

# W1-EVL-04f — Strong-profile transfer gates causal intervention

## Outcome

A preregistered gate either reproduces a positive weak-profile coordination effect on the frozen
transfer split with a stronger admitted profile, or records the exact negative stop that prevents a
weak-only effect from authorizing primary or causal claims.

## Scope

### In

- The diagnostic decision as an immutable trigger, a distinct held-out transfer partition, one
  stronger admitted profile, matched single/best-of-2/coordinated arms, exact resource accounting,
  and the unchanged diagnostic minimum-effect rule.
- A no-run negative branch when W1-EVL-04e reports no support or invalid evidence.

### Out

- Primary corpus/seeds/outcomes, message interventions, profile shopping, task replacement after a
  weak result, and tuning budgets or thresholds to obtain transfer.

## Acceptance

- [ ] The gate consumes exactly the signed W1-EVL-04e decision: `no support` or `invalid` produces a
      reproducible stop with no model calls; only `advance` admits the transfer cohort.
- [ ] An admitted transfer cohort uses the frozen transfer split, one exact stronger profile and
      matched resources across all three arms; profile substitution or reused development tasks is
      rejected by compliance validation.
- [ ] Advancement requires the preregistered effect against both controls in the required strata;
      failure or uncertainty records a negative transfer result and blocks message interventions.
- [ ] A forced weak-only positive result cannot satisfy this task without strong-profile evidence;
      a negative result remains an accepted scientific outcome.

## Current state

No transfer gate or stronger-profile development evidence exists. The current mechanism map states
that weak-only improvement is bounded, but no task enforces that boundary before primary and causal
work.

## Next action

Freeze the transfer contract after W1-EVL-04e produces its signed decision; do not select a stronger
profile or spend model budget earlier.

## Guardrails

- Transfer is a gate, not another opportunity to tune the weak mechanism.
- A negative or inconclusive transfer completes this task and stops the downstream claim.
- No intervention episode is selected before this gate closes.

## Findings

- Created from the scientific plan audit because W1-EVL-04b otherwise permits causal work on a
  weak-profile-only effect.

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

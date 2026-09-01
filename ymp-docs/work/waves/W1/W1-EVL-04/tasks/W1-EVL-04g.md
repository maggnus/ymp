---
id: W1-EVL-04g
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: routine
maturity: OPERATIONALIZATION
relation: follow_up
depends_on: []
blocks: []
created_at: 2026-09-01T13:49:35+08:00
updated_at: 2026-09-01T13:49:35+08:00
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

# W1-EVL-04g — Legacy work records no longer mask plan validation

## Outcome

The project-wide work-tree validator returns zero because legacy metadata, links and checklist state
match durable Git evidence, so new plan defects cannot hide inside an accepted baseline of errors.

## Scope

### In

- Existing `ymp-docs/work/**` schema, lifecycle metadata, source links and generated indexes reported
  by the current `work.py check` failure inventory.
- Mechanical repairs grounded in immutable commits or explicit honest partial/deferred state.

### Out

- Product code, scientific thresholds or claims, accepted implementation outcomes, rewriting Git
  history, inventing evidence, and making this follow-up a prerequisite for POC execution.

## Acceptance

- [ ] A frozen baseline lists every validator error by node and defect class before repair.
- [ ] Every accepted-node repair cites the immutable closure evidence or changes the node to an
      honest non-accepted state; no open checklist is merely checked by convention.
- [ ] `work.py check --root ymp-docs/work` exits 0, regenerated STATUS/WAVES are stable, all links
      resolve, and `git diff --check` exits 0.
- [ ] Reintroducing one unknown relation, missing closure link, open accepted checklist, or stale
      generated row makes the same validator fail non-zero.

## Current state

The validator reports pre-existing APP/COR/PRD metadata and link debt, while the newly changed
evaluation nodes validate cleanly. Ledger automation remains unavailable until this follow-up is
completed, but product and experiment implementation can proceed independently.

## Next action

Freeze and classify the current error inventory; repair homogeneous metadata in reviewed batches
after the product critical path releases capacity.

## Guardrails

- Unknown historical truth is recorded as unknown or non-accepted, never inferred from a green test.
- Generated indexes are regenerated, never edited manually.
- This task cannot absorb product defects discovered while reading old records.

## Findings

- Created from evaluation-plan review R1 as non-blocking additional work; touched evaluation nodes
  themselves produced no validator errors.

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

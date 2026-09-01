---
id: W1-EXP-01e
kind: task
wave: W1
card: W1-EXP-01
state: ready
risk: critical
maturity: BUILD
relation: follow_up
depends_on: []
blocks: [W1-EVL-04e, W1-EVL-04f, W1-EVL-04a]
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

# W1-EXP-01e — Held-out L4+ strata have frozen tasks and oracles

## Outcome

A frozen development-only L4+ ladder supplies held-out decomposable and sequential/null task
packages whose exact oracles, mutations, strata and digests can distinguish complementary synthesis
from independent selection without consuming a primary-study task or seed.

## Scope

### In

- At least one decomposable task with two independently checkable branches plus an integration
  property, and one strongly sequential or expected-null control.
- Public requirements, visible checks, protected oracle inputs, seeded invalid candidates, task and
  oracle digests, decomposability labels, and a frozen development/transfer partition.
- Executable preparation, exact verification, and manifest compliance in a fresh disposable root.

### Out

- Existing L1–L3 calibration cases, frozen primary corpus/seeds/outcome, model calls, arm scheduling,
  participant communication, and causal interpretation.

## Acceptance

- [ ] Every task package has a stable identifier, source digest, public-contract digest, protected
      oracle digest, stratum, split, and a seed namespace disjoint from calibration and primary use.
- [ ] The decomposable package exposes at least two branch oracles and one integration oracle;
      removing either required branch or the integration step makes the protected verifier reject.
- [ ] The sequential/null package declares its dependency order and expected-null stratum before
      execution; a manifest that relabels it as decomposable is rejected by compliance validation.
- [ ] Correct reference candidates pass and every seeded invalid candidate fails through the exact
      verifier with captured exits; the report states each oracle's blind spot.
- [ ] Preparation, mutation validation, digest reproduction and split-integrity checks run without
      model calls in a fresh root with separate project, `HOME`, `YMP_HOME`, `TMPDIR`, build and
      export paths.

## Current state

L1–L3 validate profiles and oracles but provide no held-out decomposability stratum. No task owns
the L4+ corpus, protected mutations, split, or frozen digests required by the weak diagnostic and
strong-profile transfer gates.

## Next action

Have Sol max freeze the task hypotheses and Sol xhigh implement the smallest exact packages and
protected verifiers before any model call.

## Guardrails

- Task design cannot be changed after a model sees an assignment or an oracle result is revealed.
- A task that the strong single profile already solves at ceiling remains a labelled control rather
  than being replaced post hoc.
- No protected oracle path or expected value enters a participant workspace or public manifest.

## Findings

- Created from the scientific plan audit: without this owner, the project jumps from elementary
  calibration directly to an uninterpretable coordination comparison.

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

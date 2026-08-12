---
id: W1-EXP-01a
kind: task
wave: W1
card: W1-EXP-01
state: active
risk: critical
maturity: DESIGN
relation: required
depends_on: []
blocks: [W1-EXP-01b]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T11:59:58+08:00
started_at: 2026-08-12T11:59:58+08:00
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-EXP-01a — POC corpus rejects known invalid candidates

## Outcome

A curated corpus of reproducible software-repair and bounded code-change tasks has approved public
requirements, pinned environments, protected black-box oracles, and mutation evidence showing
that each major requirement can reject a known invalid candidate.

## Scope

### In

- Immutable source snapshots, public `PROJECT.md` files, environment manifests, visible checks,
  protected bundles, review protocols, budget and observation policies, and approval records.
- Requirement-to-evidence matrices, known-good candidates where available, invalid mutations,
  negative controls, nondeterminism bounds, and clean-room reproduction.
- Decomposable and strongly sequential task strata labelled without using ymp outcomes.

### Out

- Automated contract generation, taste-based work, open-ended research, production credentials,
  external side effects, and arbitrary-project support.
- Product implementation or runtime-driver code.

## Acceptance

- [ ] Every admitted task has an approved contract package and a repeatable clean-room command
  whose result matches the package's declared baseline or known-good expectation.
- [ ] Every major public requirement has at least one deliberately invalid mutation that the
  protected oracle rejects for the intended reason.
- [ ] If a negative control unexpectedly passes, the same harness exits unsuccessfully and labels
  the package unusable rather than accepting a candidate.
- [ ] Protected cases instantiate public requirements only; review finds no hidden requirement
  that could change the declared task.

## Current state

The owner permits an unrestricted number of openly distributable, nonsensitive external packages
and tasks. Work is active on the primary corpus, immutable source bindings, protected bundles, and
approval evidence; L1-L3 remain development calibration and cannot be reused as primary cases.

## Next action

Select the first statistically justified external task set, then freeze its source revisions,
public contracts, structural labels, protected oracles, and negative controls.

## Guardrails

- Protected oracle bytes remain outside every producing attempt and collaboration export.
- The disposable study environment contains no sensitive repository or ordinary host credential.
- A generated check never becomes authoritative without human requirement-to-evidence review.

## Findings

None. Owner decisions are recorded in the gate registry rather than as inferred findings.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

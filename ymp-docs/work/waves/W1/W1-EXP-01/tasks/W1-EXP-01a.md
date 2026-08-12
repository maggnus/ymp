---
id: W1-EXP-01a
kind: task
wave: W1
card: W1-EXP-01
state: blocked
risk: critical
maturity: DESIGN
relation: required
depends_on: []
blocks: [W1-EXP-01b]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T14:50:00+08:00
started_at: 2026-08-12T11:59:58+08:00
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 148
blocker: Owner must approve the exact immutable corpus root through a trusted external provenance channel that the corpus author cannot self-declare.
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

The first four external packages are reproducible, their public artifact bindings are complete,
and all declared invalid candidates are rejected. The exact candidate and both clean workspaces
are preserved. Progress is blocked only because a corpus author can fabricate the current JSON
approval record and make the tool report owner approval without trusted provenance.

## Next action

The owner selects and uses a trusted approval channel for the immutable corpus root. The next
implementation must remove acceptance based on self-declared JSON before this task is reviewed
again or integrated.

## Guardrails

- Protected oracle bytes remain outside every producing attempt and collaboration export.
- The disposable study environment contains no sensitive repository or ordinary host credential.
- A generated check never becomes authoritative without human requirement-to-evidence review.

## Findings

Two review rounds are complete. The second review independently confirmed the source archives,
58 public artifact bindings, protected checks, negative controls, exact root, toolchain checks,
and fail-closed behavior for malformed approval data. It also demonstrated that a corpus author
can fabricate the asserted owner role and authority in JSON, so the remaining authorization
defect cannot be accepted as residue. The convergence decision is to name the owner gate and stop;
an ordinary third correction round is not authorized.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

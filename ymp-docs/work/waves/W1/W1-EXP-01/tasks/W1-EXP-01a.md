---
id: W1-EXP-01a
kind: task
wave: W1
card: W1-EXP-01
state: accepted
risk: critical
maturity: DESIGN
relation: required
depends_on: []
blocks: [W1-EXP-01b]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T15:34:00+08:00
started_at: 2026-08-12T11:59:58+08:00
accepted_at: 2026-08-12T15:34:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/ff4a9383653a0f09b947ff24b569e760396a0502
closure_commit: https://github.com/maggnus/ymp/commit/ff4a9383653a0f09b947ff24b569e760396a0502
evidence: ["[ff4a938](https://github.com/maggnus/ymp/commit/ff4a9383653a0f09b947ff24b569e760396a0502)", "[417f2d9](https://github.com/maggnus/ymp/commit/417f2d9cba9fd6b5d72aab4f7f2768b3c70c3d30)"]
duration_minutes: 179
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

- [x] Every admitted task has an approved contract package and a repeatable clean-room command
  whose result matches the package's declared baseline or known-good expectation.
- [x] Every major public requirement has at least one deliberately invalid mutation that the
  protected oracle rejects for the intended reason.
- [x] If a negative control unexpectedly passes, the same harness exits unsuccessfully and labels
  the package unusable rather than accepting a candidate.
- [x] Protected cases instantiate public requirements only; review finds no hidden requirement
  that could change the declared task.

## Current state

Accepted. Four external Rust repair packages have immutable sources, public requirements, protected
oracles, negative controls, and clean-room reproduction. The owner approved their exact corpus root
through the project CTO channel, while `ymp-corpus` exposes technical verification only and cannot
assert owner authority from corpus-authored input.

## Next action

Use the accepted corpus root and package records to freeze the matched-budget study specification
in W1-EXP-01b.

## Guardrails

- Protected oracle bytes remain outside every producing attempt and collaboration export.
- The disposable study environment contains no sensitive repository or ordinary host credential.
- A generated check never becomes authoritative without human requirement-to-evidence review.

## Findings

Two review rounds are complete. The second review independently confirmed the source archives,
58 public artifact bindings, protected checks, negative controls, exact root, toolchain checks,
and fail-closed behavior for malformed approval data. It also demonstrated that a corpus author
can fabricate the asserted owner role and authority in JSON, so the remaining authorization
defect cannot be accepted as residue. The convergence decision named the owner gate and stopped the
ordinary rework loop. The owner resolved that gate through the project CTO channel. One final
focused correction and review is authorized because silent unauthorized approval fails both the
reversibility and detection tests; no additional scope may enter this iteration.

## Closure

### Accepted outcome

The first corpus edition contains four reproducible packages from independent public Rust
repositories. All seven deliberately invalid variants are rejected for their declared public
requirements, and unexpected negative-control success makes a package unusable. Public artifact
bindings cover 58 linked entities. Technical verification and owner authorization are separate:
the corpus tool cannot represent or infer owner approval, and the approved root remains recorded
in the project-controlled owner-decision registry.

### Residuals

The four-package edition is an initial reproducible corpus, not the statistically sufficient final
sample. W1-EXP-01b owns preregistered expansion thresholds and the final study size.

### Evidence

- [Reviewed and integrated corpus](https://github.com/maggnus/ymp/commit/ff4a9383653a0f09b947ff24b569e760396a0502).
- [Owner approval of the immutable root](https://github.com/maggnus/ymp/commit/417f2d9cba9fd6b5d72aab4f7f2768b3c70c3d30).

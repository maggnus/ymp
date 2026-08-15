---
id: W1-APP-02e.6
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02e
state: accepted
risk: significant
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02e]
blocks: []
created_at: 2026-08-15T01:52:53+08:00
updated_at: 2026-08-15T12:08:34+08:00
started_at: 2026-08-15T08:15:17+08:00
accepted_at: 2026-08-15T12:08:34+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/dd7c7d36bf40a1e04ced7b5d4401a781919a5554
closure_commit: https://github.com/maggnus/ymp/commit/60af562fddf16c283f5c67d10dd9882ed6d3cc20
evidence: ["[60af562](https://github.com/maggnus/ymp/commit/60af562fddf16c283f5c67d10dd9882ed6d3cc20)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger: the codex quota resets (2026-09-12) — measure the account routes into the registry instead of the pinned entry; a build carrying more than 512 identifiers exercises the filtered branch for the first time
deliberate_partial: true
---

# W1-APP-02e.6 — Runtime engines are managed entities with properties and model lists

## Outcome

Engines live in a registry under the product root (runtimes/<engine>.json): enabled flag, measured properties (executable, version, credential origin, budget bounds) and the list of models the engine can serve, filled by probe. A disabled engine is not admitted and not offered. Enable/disable from /runtimes and the mirrored command. Admission semantics (tools, permission mode, no delegation, budget ceilings) stay hard-coded; WHICH models may be used stays undecided until the W1-EVL-04d research lands.

## Scope

### In

- See outcome; zones per the approved plan of 2026-08-15.

### Out

- Everything accepted by the parent and sibling nodes.

## Acceptance

- [ ] Engines live in a registry under the product root (runtimes/<engine>.json): enabled flag, measured properties (executable, version, credential origin, budget bounds) and the list of models the engine can serve, filled by probe. A disabled engine is not admitted and not offered. Enable/disable from /runtimes and the mirrored command. Admission semantics (tools, permission mode, no delegation, budget ceilings) stay hard-coded; WHICH models may be used stays undecided until the W1-EVL-04d research lands.

## Current state

Ready. Owner decision 2026-08-15: manage runtime engines like Paseo does — switchable, with properties and at least the model list they provide.

## Next action

None recorded.

## Guardrails

None recorded.

## Findings

### Absorbed owner decision (2026-08-15)

The Codex provider is unavailable for now (account usage limit until 2026-09-12) — the registry's
disable state is its first live use. Experiments run on the Claude engine with the cheaper
claude-haiku / claude-sonnet routes in the permitted pool; the measured model catalog of the
installed claude build must therefore include them, and the permitted-pool selection applies to
live smokes and experiments so cost stays bounded.

### Absorbed review findings (02e.4, 2026-08-15)

- The executable-resolution branch reading the configuration directory never fires on the owner's
  host and adds a second selection channel; registry properties should own executable discovery.
- The unparsable-version refusal claims the release is older than the floor instead of naming the
  parse failure.
- The minimum_version is the one pin not checked against its constant.

### Rounds

2

### Convergence

A third bounded round was authorized under the gate's blocker exception: the round-two finding —
a nested root is never recognized as its own, so a disable lands in a foreign root and a disabled
engine silently launches — fails the detection test (a silent authority bypass), and the fix is
precisely named by the review (stop the root search at a path carrying root.json; never re-derive
an explicit --root through the store). The digest-guarantee wording is corrected in the same
round.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- three review rounds; the final round reproduced both nested-root scenarios and the address-kind
  matrix on the built product, confirmed the surface scanner, and verified the honest digest
  guarantee line by line; catalog of 55 routes measured from the installed build at no cost;
  integration edit merged the home-root and registry addressing with green composition checks; codex
  seeded disabled with its reason

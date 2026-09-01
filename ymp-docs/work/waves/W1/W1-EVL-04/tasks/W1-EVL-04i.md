---
id: W1-EVL-04i
kind: task
wave: W1
card: W1-EVL-04
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04h]
blocks: [W1-EVL-04k, W1-EVL-04j, W1-EVL-04e]
created_at: 2026-09-01T15:41:48+08:00
updated_at: 2026-09-01T16:33:04+08:00
started_at: 2026-09-01T15:54:18+08:00
accepted_at: 2026-09-01T16:33:04+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/0f4be38b521b1c991e65bbced5f265f403997bae
closure_commit: https://github.com/maggnus/ymp/commit/b93cd09b039eb9751f929ce7972d141ceebbe0e1
evidence: ["[b93cd09](https://github.com/maggnus/ymp/commit/b93cd09b039eb9751f929ce7972d141ceebbe0e1)"]
duration_minutes: 39
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 1
escalation_decision:
---

# W1-EVL-04i — Codex runtime proves exact 0.151 conformance

## Outcome

The Codex driver and supervisor accept the installed `codex-cli 0.151.0` only after exact tool,
event, usage, resume, interruption, cancellation and descendant-termination conformance is measured;
every unsupported or incomplete tuple is rejected with a precise machine-owned reason before an
accepted task can start.

## Scope

### In

- Exact `0.151.0` version/help/feature discovery, removed-flag handling, App Server/tool schema,
  streaming event and usage parsing, resume, interruption, cancellation and descendant termination.
- Migration of the pinned Codex runtime constant and the narrow supervisor projection only after
  their exact compatibility is demonstrated.
- Exclusive write zone: `ymp-rust/crates/ymp-runtime-codex/src/lib.rs`,
  `ymp-rust/crates/ymp-runtime-supervisor/src/lib.rs`, optional focused additions under
  `ymp-rust/crates/ymp-runtime-supervisor/tests/**`, and mechanically forced changes only in
  `ymp-rust/crates/ymp-runtime-codex/Cargo.toml`,
  `ymp-rust/crates/ymp-runtime-supervisor/Cargo.toml` and `ymp-rust/Cargo.lock`.

### Out

- No-touch: `ymp-rust/crates/ymp-cli/**`, `ymp-rust/tools/ymp-corpus/**`, Claude runtime,
  provider/model selection, task prompts, budgets, research records, calibration results, primary
  corpus, TUI, storage, deployment, real model/network/money calls, and exposing compatibility
  versions or project directories to the user.

## Acceptance

- [x] An exact fake `codex-cli 0.151.0` passes the driver and supervisor conformance matrix with only
      flags and wire shapes that 0.151 actually supports; the measured version is projected
      automatically into the managed attempt.
- [x] A fake 0.151 executable that retains a removed flag, changes the tool/event/usage schema, lies
      about its version, loses resume identity or ignores cancellation fails for its own typed reason
      before an accepted task can start.
- [x] The pinned driver constant and supervisor projection bind the same exact tuple. A stale 0.147
      projection, a future unmeasured version or loss of the projection fails closed rather than
      silently selecting a nearby version.
- [x] Negative fixtures independently distinguish removed flags, changed tool/event/usage schema,
      false version output, lost resume identity, ignored cancellation and surviving descendants;
      each reaches the production driver or supervisor path and fails for its typed reason.
- [x] No real model/provider/network call runs; focused runtime and supervisor tests, strict Clippy,
      formatting and `git diff --check` pass without warnings.

## Current state

Accepted and integrated as
[b93cd09](https://github.com/maggnus/ymp/commit/b93cd09b039eb9751f929ce7972d141ceebbe0e1).
Codex readiness now measures exact 0.151 help, capabilities and App Server schema without a model;
the supervisor accepts only the exact 0.151 projection and the managed process boundary proves
cancellation plus descendant termination. Product fixtures remain W1-EVL-04k work.

## Next action

Run W1-EVL-04k and W1-EVL-04j in parallel from the accepted 0.151 runtime boundary; their write zones
are disjoint.

## Guardrails

- Version pinning is internal reproducibility metadata, not user workflow.
- Compatibility is measured from executable behavior, never inferred from a larger version number.
- Historical `ymp-docs/research/cal-001-calibration.md` and
  `ymp-rust/tools/ymp-calibration/results/**` remain immutable evidence and are not rewritten to
  name 0.151.

## Findings

- R1 contract review split the original cross-surface task: this node owns behavioral runtime
  conformance; W1-EVL-04k owns product fixtures and admission metadata.
- The accepted implementation removed obsolete capability flags, added measured help/schema
  readiness and tightened tool/event/usage parsing without changing manifests or dependencies.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(9/10) ACCEPT 01/09 16:33 — the three-path runtime-only diff pins exact 0.151 behavior and
  preserves the No-touch boundary → author evidence covers focused conformance and typed negatives
  → an independent managed-process scenario proves launch, exact projection, cancellation and
  descendant termination without model or network use

## Closure

### Accepted outcome

`ymp-runtime-codex` admits exact `codex-cli 0.151.0` only after no-model discovery of `exec`/resume
help, capability state and App Server schema; obsolete flags are not passed and structured
tool/event/usage parsing is strict. The supervisor rejects missing, stale or future projections,
records the exact runtime profile and terminates a cancelled managed process together with its
descendants.

### Residuals

None.

### Evidence

- [b93cd09](https://github.com/maggnus/ymp/commit/b93cd09b039eb9751f929ce7972d141ceebbe0e1)
  — integrated tree, byte-identical for the reviewed runtime paths to candidate
  [0f4be38](https://github.com/maggnus/ymp/commit/0f4be38b521b1c991e65bbced5f265f403997bae).

---
id: W1-EVL-04i
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04h]
blocks: [W1-EVL-04j, W1-EVL-04e]
created_at: 2026-09-01T15:41:48+08:00
updated_at: 2026-09-01T15:41:48+08:00
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

# W1-EVL-04i — Installed weak runtime matches the supported CLI

## Outcome

The installed `codex-cli 0.151.0` is either supported by one exact production runtime/profile tuple
with measured tool, event, usage, resume and cancellation conformance, or rejected with a precise
machine-owned reason. A normal user never selects, manages or sees an internal compatibility
version.

## Scope

### In

- Exact `0.151.0` version/help/feature discovery, removed-flag handling, App Server/tool schema,
  streaming event and usage parsing, resume, interrupt, cancellation and descendant termination.
- Migration of the pinned Codex runtime constant, fake executable fixtures, supervisor metadata,
  product-path fixtures and the admission manifest/digest only after their exact compatibility is
  demonstrated.
- Candidate write zone to be finalized by Critical pre-dispatch review from these bounded surfaces:
  `ymp-runtime-codex/**`, the narrow Codex-version projection in `ymp-runtime-supervisor/**`, exact
  Codex fixtures in `ymp-cli/tests/**`, and
  `ymp-corpus/corpus/development/weak-diagnostic-admission-v1/manifest.{json,sha256}` plus forced
  manifests/lockfile.

### Out

- Claude runtime, provider/model selection, task prompts, budgets, research records, primary corpus,
  TUI, storage, deployment, real model/network/money calls, and exposing compatibility versions or
  project directories to the user.

## Acceptance

- [ ] An exact fake `codex-cli 0.151.0` passes readiness and the complete existing production-path
      conformance matrix with only flags and wire shapes that 0.151 actually supports; the measured
      version is recorded automatically.
- [ ] A fake 0.151 executable that retains a removed flag, changes the tool/event/usage schema, lies
      about its version, loses resume identity or ignores cancellation fails for its own typed reason
      before an accepted task can start.
- [ ] The admission manifest and runtime constant bind the same exact tuple; a stale 0.147 manifest
      or a future unmeasured version keeps `model_ready=false` rather than silently falling back.
- [ ] Public CLI/TUI behavior contains no version selector, project-directory ceremony or upgrade
      prompt; an installed compatible runtime is used automatically and an incompatible one produces
      one actionable product-owned error.
- [ ] No real model/provider/network call runs; focused runtime/supervisor/CLI checks, strict Clippy,
      formatting and `git diff --check` pass with a captured negative half.

## Current state

W1-EVL-04h measured installed `codex-cli 0.151.0` against the accepted `0.147.0` profile and closed
the gate. The repository contains many exact 0.147 runtime and product fixtures, so a blind constant
bump would be an unreviewable compatibility claim. No model call is authorized.

## Next action

Run a Critical contract check that either proves this cross-surface migration is one reviewable atom
or splits runtime conformance from product-fixture migration before dispatch.

## Guardrails

- Version pinning is internal reproducibility metadata, not user workflow.
- Compatibility is measured from executable behavior, never inferred from a larger version number.
- Historical calibration results remain immutable evidence and are not rewritten to name 0.151.

## Findings

- Created from the accepted W1-EVL-04h fail-closed report. The task may require decomposition because
  the current 0.147 tuple appears in the driver, supervisor and several public product fixtures.

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

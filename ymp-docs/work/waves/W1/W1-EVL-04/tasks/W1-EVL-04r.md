---
id: W1-EVL-04r
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04n]
blocks: [W1-EVL-04s]
created_at: 2026-09-01T21:14:37+08:00
updated_at: 2026-09-01T21:14:37+08:00
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

# W1-EVL-04r — Live probe failures are durably phase-localized

## Outcome

Every failed live tool-host probe leaves one immutable, sanitized and phase-localized failure record
under its isolated YMP data root, sufficient to distinguish pre-provider process/configuration,
provider/model, MCP transport/server and controller-attestation failures without creating an
attestation or exposing raw stderr.

## Scope

### In

- Add strict `ToolHostProbeFailurePhase` and `ProviderRequestState` types plus a canonical
  `ToolHostProbeFailureRecord` binding manifest/runtime/compatibility/executable/probe-transport,
  probe/invocation/reservation identities, last accepted event id/sequence/type, exit status or
  signal, duration, complete `Usage`, cost availability and bounded diagnostics for Codex and MCP.
- Runtime-codex records observable stages only: process spawned, `turn.started`, provider response or
  provider-typed failure. It must not infer a provider request from elapsed time or process exit.
- Supervisor adds observed probe stages: runtime started, MCP call events seen and terminal mapping.
  It preserves structured failure evidence for both terminal events and stream/process errors.
- Application writes `failure.json` with `create_new`+fsync beside the already spent reservation on
  any executor failure, verifies it on recovery and never writes `attestation.ref` or refunds/retries.
- Exclusive write zone: narrow changes in `ymp-runtime-api/src/lib.rs`,
  `ymp-runtime-codex/src/lib.rs`, `ymp-runtime-supervisor/src/lib.rs`,
  `ymp-application/src/tool_host_probe.rs`, focused tests and the existing live CLI harness only;
  mechanically forced manifests/lockfile are allowed.

### Out

- No-touch: launch/MCP capability semantics, admission/corpus manifests, product/TUI surfaces,
  experiments, prompts, budgets, research records and any real provider/model/network invocation.

## Acceptance

- [ ] Deterministic fixture failures before process spawn, before `turn.started`, after turn start,
      before MCP call, after MCP receipt/result and before controller read-back each persist exactly
      one distinct phase with last event identity and no attestation.
- [ ] Failure records preserve complete token classes, in-flight model request count, cost
      availability, exit/signal/duration and only `{sha256, bytes, truncated}` diagnostics. Missing
      observations are explicit `unknown`, never zero or inferred success.
- [ ] Provider states are exactly `not_started`, `turn_started_unconfirmed`, `provider_responded` or
      `unknown`; deterministic tests prove each transition and prevent ProcessExit alone from being
      classified as provider failure.
- [ ] Recovery rejects missing/corrupt/mismatched/replayed failure records, copied stores and any
      simultaneous failure+attestation. A spent failed reservation cannot run again or be refunded.
- [ ] Focused tests and a mutation discarding terminal Usage/diagnostic/phase fail as expected;
      strict Clippy, formatting and `git diff --check` pass with zero live calls and isolated state.

## Current state

The first authorized live attempt ended `ProcessExit` after 47.65 s. Before the R1 repair, supervisor
discarded its terminal `Usage` and diagnostic summary, so provider request and failure phase are
indeterminate. W1-EVL-04n is accepted only as deterministic implementation; the live gate remains
closed and its reservation spent.

## Next action

Run a Critical contract check of this diagnostic-only four-file seam, then dispatch one Sol xhigh
builder with all provider/model calls prohibited.

## Guardrails

- Diagnostics report observations, never causal guesses.
- Failure evidence grants no attestation, refund, retry or model readiness.
- Raw prompts, outputs, stderr, tokens and credentials are never persisted.

## Findings

- Scientific classification of the first attempt is
  `infrastructure-invalid / failure phase indeterminate`; the reservation is spent.

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

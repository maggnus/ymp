---
id: W1-EVL-04r
kind: task
wave: W1
card: W1-EVL-04
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04n]
blocks: [W1-EVL-04s]
created_at: 2026-09-01T21:14:37+08:00
updated_at: 2026-09-01T22:28:48+08:00
started_at: 2026-09-01T21:23:04+08:00
accepted_at: 2026-09-01T22:28:48+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/c7f410853862291f058ee851e7799658bdd7b1dc
closure_commit: https://github.com/maggnus/ymp/commit/baf31aa974535d56514bbb0948df5b77cc983b4c
evidence: ["[baf31aa](https://github.com/maggnus/ymp/commit/baf31aa974535d56514bbb0948df5b77cc983b4c)"]
duration_minutes: 66
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 2
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
- Add one probe-specific default method
  `RuntimeSession::tool_host_probe_failure_evidence() -> Option<ToolHostProbeFailureEvidence>`.
  Ordinary sessions inherit `None`; the private Codex probe-session wrapper returns only its tracked
  sanitized stages/process outcome/duration. No general `RuntimeEventKind` or agent protocol grows.
- Runtime-codex records observable stages only: process spawned, `turn.started`, provider response or
  provider-typed failure. It must not infer a provider request from elapsed time or process exit.
- Supervisor adds observed probe stages: runtime started, MCP call events seen and terminal mapping.
  It merges the session evidence with existing `RuntimeTerminalFailed` kind, `Usage`, diagnostic and
  event identity; absent driver observations remain explicit `unknown`.
- After `reservation.json` is fsynced, `Application::controller_tool_host_probe` routes executor,
  trace-validation, read-back and attestation-write errors through one private finalizer. It writes
  canonical `failure.json` via `create_new`+fsync before returning; the record binds store,
  reservation and replay identities and never precedes a spent reservation.
- `Application::tool_host_probe_failure(probe_id)` reloads and verifies reservation plus failure for
  the trusted harness. Recovery refuses failure+attestation, missing/corrupt/mismatched or copied
  records; no public constructor or raw deserialization grants authority.
- Exclusive write zone: narrow changes in `ymp-runtime-api/src/lib.rs`,
  `ymp-runtime-codex/src/lib.rs`, `ymp-runtime-supervisor/src/lib.rs`,
  `ymp-application/src/tool_host_probe.rs`, focused tests and the existing live CLI harness only;
  mechanically forced manifests/lockfile are allowed.

### Out

- No-touch: launch/MCP capability semantics, admission/corpus manifests, product/TUI surfaces,
  experiments, prompts, budgets, research records and any real provider/model/network invocation.

## Acceptance

- [x] Deterministic fixture failures before process spawn, before `turn.started`, after turn start,
      before MCP call, after MCP receipt/result and before controller read-back each persist exactly
      one distinct phase with last event identity and no attestation.
- [x] Ordinary non-probe RuntimeSession implementations compile unchanged through the default method;
      only Codex probe sessions return phase evidence, and supervisor never fabricates missing driver
      observations.
- [x] Failure records preserve complete token classes, in-flight model request count, cost
      availability, exit/signal/duration and only `{sha256, bytes, truncated}` diagnostics. Missing
      observations are explicit `unknown`, never zero or inferred success.
- [x] Provider states are exactly `not_started`, `turn_started_unconfirmed`, `provider_responded` or
      `unknown`; deterministic tests prove each transition and prevent ProcessExit alone from being
      classified as provider failure.
- [x] Recovery rejects missing/corrupt/mismatched/replayed failure records, copied stores and any
      simultaneous failure+attestation. A spent failed reservation cannot run again or be refunded.
- [x] Focused tests and a mutation discarding terminal Usage/diagnostic/phase fail as expected;
      strict Clippy, formatting and `git diff --check` pass with zero live calls and isolated state.

## Current state

Accepted on integrated main
[baf31aa](https://github.com/maggnus/ymp/commit/baf31aa974535d56514bbb0948df5b77cc983b4c).
Every failed probe now leaves verified phase-localized evidence after a spent reservation and cannot
produce an attestation, refund or retry.

## Next action

Repeat the read-only W1-EVL-04s gate against this accepted diagnostic schema, then authorize only its
already frozen single live command.

## Guardrails

- Diagnostics report observations, never causal guesses.
- Failure evidence grants no attestation, refund, retry or model readiness.
- Raw prompts, outputs, stderr, tokens and credentials are never persisted.

## Findings

- Scientific classification of the first attempt is
  `infrastructure-invalid / failure phase indeterminate`; the reservation is spent.
- R1 contract review required an explicit narrow carrier and persistence order. The corrected design
  uses one default probe-session method, the existing terminal failure type and one post-reservation
  Application finalizer; the general event protocol remains unchanged.
- R1 outcome review selected the correct corruption/recovery risk but its command executed zero
  tests; this is an evidence defect, not evidence of a product-code failure.
- R2 ran two exact recovery cases: byte corruption, copied storage, absent attestation and spent
  reservation were all distinguished without changing production behavior.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(4/10) RETURN 01/09 22:19 — independent corruption/recovery command executed zero tests → final
  candidate remained unchanged → author must provide one exact non-zero typed-rejection scenario
- R2(9/10) ACCEPT 01/09 22:28 — test-only delta selected two cases → typed corruption/copy rejection,
  absent attestation and spent reservation observed → final candidate integrated unchanged

## Closure

### Accepted outcome

Strict failure phases and provider states now travel from the private Codex probe session through
supervisor evidence into one canonical, synchronized `failure.json`. Application recovery verifies
reservation and store identity, rejects corruption, copying, replay and failure+attestation, and
never converts failed evidence into readiness.

### Residuals

None. A real live result belongs exclusively to W1-EVL-04s.

### Evidence

- [baf31aa](https://github.com/maggnus/ymp/commit/baf31aa974535d56514bbb0948df5b77cc983b4c)
  — integrated code and exact R1 test correction; byte-equivalent to candidate
  [c7f4108](https://github.com/maggnus/ymp/commit/c7f410853862291f058ee851e7799658bdd7b1dc).

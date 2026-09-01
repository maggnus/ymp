---
id: W1-EVL-04l
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04j]
blocks: [W1-EVL-04n]
created_at: 2026-09-01T15:57:45+08:00
updated_at: 2026-09-01T17:24:00+08:00
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

# W1-EVL-04l — Controller owns nonce read-back and probe attestation

## Outcome

The trusted foreground controller proves the destination absent, creates an unpredictable nonce and
scoped disposable path, durably spends one separate probe reservation, executes the accepted fake
W1-EVL-04j seam, independently reads back the file and persists one opaque
`AttestedToolHostProbe` below the configured YMP data root only when every binding matches.

## Scope

### In

- `ControllerToolHostProbeRequest` names only the accepted admission-manifest digest, expected
  runtime tuple, deadline and `ToolHostProbeResourceVector`; `Application` generates the unique
  `probe_id`, `invocation_id`, nonce and normalized relative destination and proves the target absent
  before any invocation.
- A narrow probe store below
  `runtime-evidence/tool-host-probes/<probe_id>/` writes and fsyncs canonical
  `reservation.json` with `create_new` before execution. It records store/run identity, manifest,
  probe/invocation, nonce digest, relative path, reservation and replay key; an existing marker is
  spent and cannot start another invocation. Failure may add an immutable typed failure record but
  never removes or refunds the reservation.
- After the supervisor returns its untrusted trace, the controller reads the workspace file itself.
  `AttestedToolHostProbe` binds schema version, store/run identity, manifest digest, probe and
  invocation identities, nonce/read-back digest and byte count, proven-prelaunch absence, relative
  path, the complete canonical `ToolHostProbeTrace` and trace digest, runtime tuple, reservation,
  charged vector, usage, wall time, cost availability, terminal and replay key.
- Canonical attestation bytes go first to the existing `ObjectStore`; a `create_new`+fsync
  `attestation.ref` then binds its object digest and record digest. A missing, corrupt or mismatched
  object/reference, or a reservation without an attestation, is recovered fail-closed. An
  unreferenced object left by interruption is permitted and grants nothing.
- The public `AttestedToolHostProbe` has private fields, read-only accessors and `Serialize` only;
  it has no public constructor or `Deserialize`. `AttestedToolHostProbeHandle` is a strict
  `Serialize`/`Deserialize` locator containing only schema version, store identity, `probe_id` and
  record digest; it is explicitly untrusted and grants no authority by itself.
- `Application::attested_tool_host_probe(&handle)` first proves that the handle names this
  application's configured store, then loads `attestation.ref`, verifies the object and record
  digests and returns the opaque attestation. Any forged/imported handle, wrong store, missing
  reference or raw JSON fails before an attestation value exists.
- `ymp internal tool-host-probe` uses the existing internal dispatch and writes the canonical handle
  with `create_new` only to the controller-owned export path after the private attestation is
  durable. It accepts no caller nonce, workspace path, object digest or attestation bytes.
- Exclusive write zone: optional new
  `ymp-rust/crates/ymp-application/src/tool_host_probe.rs`, narrow declarations and dispatch in
  `ymp-rust/crates/ymp-application/src/lib.rs`, narrow internal command wiring in
  `ymp-rust/crates/ymp-cli/src/internal.rs` (the existing `InternalCommand` dispatch in
  `ymp-cli/src/lib.rs` already routes it and is No-touch), and optional new
  `ymp-rust/crates/ymp-cli/tests/tool_host_probe.rs`.

### Out

- No-touch: runtime API/supervisor/driver code, `ymp-corpus/**`, Cargo manifests/lockfile,
  `ymp-cli/src/{lib.rs,main.rs}`, public
  CLI/TUI commands, arbitrary caller paths, real user HOME, task/board/recruitment/candidate state,
  deployment and live model/network/money execution.

## Acceptance

- [ ] In a disposable evaluation root the controller first proves the target absent, creates and
      fsyncs exactly one reservation marker, generates nonce/path/identity, invokes the accepted
      fake-runtime seam, reads the file itself and emits one immutable opaque attestation bound to
      the complete trace and read-back digest.
- [ ] Missing file, wrong nonce, runtime- or model-chosen path/nonce/digest, stale or replayed
      invocation, incomplete usage, mismatched route/profile/version/schema, ambiguous terminal,
      timeout/cancellation, budget overrun or extra effect produces no attestation and no persisted
      success record.
- [ ] A pre-existing destination, syntactically valid raw trace or model-authored evidence cannot
      construct, deserialize or reload an accepted attestation without the controller-owned
      reservation, independent read-back and private store reference.
- [ ] A round-trip serialized handle reloads the same opaque attestation only through the matching
      Application store. Mutated digest/store/probe fields, a copied handle in another isolated root
      and a fabricated handle each fail; handle JSON alone cannot satisfy W1-EVL-04m.
- [ ] The probe budget is distinct from every arm/task/candidate/communication allocation. Its
      immutable reservation is written before start; charged usage is no larger than it, and a
      failure, crash, replay or repeated terminal cannot refund it or start a second invocation.
- [ ] Recovery accepts only matching reservation, object and reference digests; missing/corrupt
      records and a reservation stranded before attestation remain spent and keep the gate closed.
- [ ] A product-path test with separate project, `HOME`, `YMP_HOME`, `TMPDIR`, build and export
      proves all durable probe state is below isolated `YMP_HOME`, with nothing in the repository,
      current directory or real `~/.ymp`.
- [ ] Focused Application/internal-CLI tests, strict affected-package Clippy, formatting and
      `git diff --check` pass; no real model/network/money call runs.

## Current state

W1-EVL-04j supplies only an untrusted fake-runtime trace. The missing authority is controller-owned
prelaunch absence, nonce/read-back, one durable reservation and one private immutable attestation.
This node validates that mechanism without a model; W1-EVL-04n separately owns the exact live Codex
bridge, so no live probe is authorized here.

## Next action

Repeat the Critical contract check against these exact narrow storage and accounting semantics; on
acceptance, dispatch one Sol xhigh builder without widening into a generic budget or capability
subsystem.

## Guardrails

- Only the foreground controller may turn a runtime trace into an attestation.
- Internal CLI wiring is not user ceremony and exposes no version or project-directory controls.
- A real probe remains a separate owner model/money decision.

## Findings

- Created by R1 decomposition of W1-EVL-04j to isolate the authority, accounting and persistence
  boundary from runtime execution and corpus policy.
- R1 contract review found the former storage format, recovery, replay and accounting semantics
  underspecified; the corrected contract uses one purpose-built reservation marker plus the existing
  object store rather than a new general ledger.
- R2 contract review found the first opaque-type design had no executable cross-process consumer;
  the corrected design serializes only an untrusted locator and requires verified reload from the
  matching private Application store.
- Scientific peer review added the mandatory prelaunch absence check and identified the accepted
  fake-only runtime as an unconditional live-gate stop; W1-EVL-04n owns that minimal bridge.

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

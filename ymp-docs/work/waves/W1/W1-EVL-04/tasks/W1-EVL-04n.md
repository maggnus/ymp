---
id: W1-EVL-04n
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04k, W1-EVL-04l, W1-EVL-04o]
blocks: [W1-EVL-04m]
created_at: 2026-09-01T17:17:30+08:00
updated_at: 2026-09-01T18:08:00+08:00
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

# W1-EVL-04n — Exact Codex profile performs one controller-bound tool-host probe

## Outcome

The exact manifest-bound Codex 0.151 profile can execute one no-task-output compatibility probe
through the W1-EVL-04o two-tool MCP transport and return an explicitly untrusted trace that only the
W1-EVL-04l Application controller can attest; no other live runtime, task output, collaboration
authority or repeated admitted invocation becomes reachable.

## Scope

### In

- Implement `RuntimeDriver::tool_host_probe_identity` and `start_tool_host_probe` for the exact
  accepted Codex 0.151 tuple in `ymp-rust/crates/ymp-runtime-codex/src/lib.rs`, using the fixed
  W1-EVL-04o server, `workspace_write` then `workspace_read`, the controller-supplied path and nonce,
  and a fixed compatibility directive that is not a user task or experimental arm. Nonempty ordinary
  output remains an error rather than hidden evidence.
- Replace the fake-only guard in `ymp-rust/crates/ymp-runtime-supervisor/src/lib.rs` with an exact
  allowlist of `Fake` for deterministic tests and the exact Codex runtime plus probe transport
  identity. A public direct caller may receive only the existing `UntrustedRuntimeTrace`; it gains no
  controller read-back, private attestation or `model_ready` authority. Trusted local code already
  owns ordinary runtime starts, so preventing it from spending its own call is not a new POC gate.
- Focused tests may be added only under
  `ymp-rust/crates/ymp-runtime-{codex,supervisor}/tests/**`; mechanically forced changes are allowed
  only in those two package manifests and `ymp-rust/Cargo.lock`.
- One admitted live probe uses the frozen W1-EVL-04k manifest and separate stage-two budget: exactly
  one model request, no task/arm output, no board/recruitment/candidate tool, and one ordered
  write/read exchange in a fresh disposable root. The W1-EVL-04l controller independently reads and
  attests it before the model gate can open.
- The W1-EVL-04k manifest continues to bind the collaboration MCP digest. The opaque attestation
  independently binds the probe tool-schema digest and the W1-EVL-04o `probe_transport_digest`; no
  manifest byte changes in this task.

### Out

- No-touch: `ymp-runtime-api/**`, `ymp-application/**`, `ymp-agent-mcp/**`, `ymp-cli/**`,
  `ymp-corpus/**`, admission
  manifest/digest, Claude runtime, TUI, research and calibration records, task prompts, experimental
  arms, primary/development seeds and budgets, real user HOME, arbitrary network tools and any model
  call beyond the single frozen probe.

## Acceptance

- [ ] Exact fake Codex 0.151 discovery plus the probe run exposes only W1-EVL-04o workspace write and
      read, carries the manifest route/profile/CLI/driver, probe schema and transport digests, writes
      then reads the supplied relative path, reports complete usage/cost/terminal evidence and yields
      a W1-EVL-04l attestation with `model_calls=1` and `model_ready=false` until W1-EVL-04m consumes it.
- [ ] A direct caller can obtain at most `UntrustedRuntimeTrace`; presenting it or a raw request to
      Application/04m creates no attestation. Stale 0.147, future 0.152, wrong route/profile/schema/
      transport, absolute/traversing path, missing/reordered tool event, output, extra effect,
      timeout/cancellation, incomplete usage or budget overflow returns a typed refusal.
- [ ] The live run starts only after zero-model readiness and all deterministic mutations pass. It
      uses one fresh short root with separate project, `HOME`, `YMP_HOME`, `TMPDIR`, build and export;
      it performs exactly one provider request within the frozen stage-two vector and is never
      continued or selectively retried after any failure.
- [ ] The controller proves the destination absent before launch and its independent read-back equals
      the nonce after the ordered tool events. A `Fake` trace, schema-identical substitute server or
      mismatched transport digest may test mechanics but can never satisfy the live admission
      evidence consumed by W1-EVL-04m.
- [ ] Focused Codex/supervisor/controller-bound tests, strict affected-package Clippy, formatting and
      `git diff --check` pass. The real repository, current directory and real `~/.ymp` remain
      byte-identical; the disposable root is removed after evidence capture.

## Current state

Codex 0.151 conformance, the admission tuple and private Application attestation are accepted. The
runtime still rejects every non-Fake probe, and W1-EVL-04o must first provide an exact two-tool MCP
server plus transport digest. Until both are accepted, the live gate is unconditionally closed.

## Next action

After W1-EVL-04o is accepted, repeat the Critical contract check of this two-package bridge, then
dispatch one Sol xhigh builder and one frozen live probe only after deterministic negatives pass.

## Guardrails

- One accepted profile, one purpose, one request. This is not a generic live-probe API.
- Infrastructure reachability is not communication, listening, task value or self-organization.
- The live probe is an admitted evaluation expense, not part of any matched-budget arm.

## Findings

- Scientific peer review found that W1-EVL-04j plus W1-EVL-04l could otherwise never produce a
  positive real-profile attestation. This task is the minimal bridge rather than a relaxation of the
  fake seam for arbitrary drivers.
- R1 contract review found no controller marker in the public trace API, no private two-tool MCP
  server, no dedicated probe launch mode and ambiguity between collaboration and probe schema
  digests. The corrected graph assigns the exact transport to W1-EVL-04o, keeps public traces
  untrusted and binds both schema domains without changing the manifest.

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

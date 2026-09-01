---
id: W1-EVL-04n
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04l, W1-EVL-04o, W1-EVL-04p, W1-EVL-04q]
blocks: [W1-EVL-04m]
created_at: 2026-09-01T17:17:30+08:00
updated_at: 2026-09-01T18:19:05+08:00
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

# W1-EVL-04n — Compatible Codex runtime performs one attested tool-host probe

## Outcome

An installed Codex runtime whose observed behavior satisfies the W1-EVL-04p compatibility contract
can execute one no-task-output probe through the W1-EVL-04o two-tool MCP transport and return an
explicitly untrusted trace that only the W1-EVL-04l Application controller can attest. Its exact
observed version and executable digest are evidence, not an acceptance selector.

## Scope

### In

- Implement `RuntimeDriver::tool_host_probe_identity` and `start_tool_host_probe` for the exact
  accepted behavioral compatibility contract in `ymp-rust/crates/ymp-runtime-codex/src/lib.rs`,
  using the fixed
  W1-EVL-04o server, `workspace_write` then `workspace_read`, the controller-supplied path and nonce,
  and a fixed compatibility directive that is not a user task or experimental arm. Nonempty ordinary
  output remains an error rather than hidden evidence.
- Replace the fake-only guard in `ymp-rust/crates/ymp-runtime-supervisor/src/lib.rs` with an exact
  allowlist of `Fake` for deterministic tests and a Codex runtime carrying the accepted behavioral
  contract plus exact observed runtime/probe transport identity. A public direct caller may receive
  only the existing `UntrustedRuntimeTrace`; it gains no controller read-back, private attestation or
  `model_ready` authority.
- Focused tests may be added only under
  `ymp-rust/crates/ymp-runtime-{codex,supervisor}/tests/**`; mechanically forced changes are allowed
  only in those two package manifests and `ymp-rust/Cargo.lock`.
- One admitted live probe uses the additive W1-EVL-04q v2 manifest and separate stage-two budget: exactly
  one model request, no task/arm output, no board/recruitment/candidate tool, and one ordered
  write/read exchange in a fresh disposable root. The W1-EVL-04l controller independently reads and
  attests it before the model gate can open.
- The W1-EVL-04q manifest binds the compatibility-contract and collaboration MCP digests. The opaque attestation
  independently binds the probe tool-schema digest and the W1-EVL-04o `probe_transport_digest`; no
  manifest byte changes in this task.

### Out

- No-touch: `ymp-runtime-api/**`, `ymp-application/**`, `ymp-agent-mcp/**`, `ymp-cli/**`,
  `ymp-corpus/**`, admission
  manifest/digest, Claude runtime, TUI, research and calibration records, task prompts, experimental
  arms, primary/development seeds and budgets, real user HOME, arbitrary network tools and any model
  call beyond the single frozen probe.

## Acceptance

- [ ] A behaviorally compatible fake Codex with any observed version plus the probe run exposes only
      W1-EVL-04o workspace write and read, carries compatibility-contract, observed
      version/executable, route/profile/driver, probe schema and transport digests, writes then reads
      the supplied path, reports complete usage/cost/terminal evidence and yields a W1-EVL-04l
      attestation with `model_calls=1` and `model_ready=false` until W1-EVL-04m consumes it.
- [ ] A direct caller can obtain at most `UntrustedRuntimeTrace`; presenting it or a raw request to
      Application/04m creates no attestation. Wrong/missing compatibility contract, behavior drift,
      route/profile/schema/transport mismatch, absolute/traversing path, missing/reordered tool event, output, extra effect,
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

Private Application attestation is accepted. The runtime still rejects every non-Fake probe;
W1-EVL-04o must provide an exact transport, W1-EVL-04p behavioral compatibility, and W1-EVL-04q its
v2 consumer. Until all three are accepted, the live gate is unconditionally closed.

## Next action

After W1-EVL-04o and W1-EVL-04q are accepted, repeat the Critical contract check of this two-package bridge, then
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
- Owner correction removed exact CLI version as product authority. W1-EVL-04p admits observed
  behavior and W1-EVL-04q preserves exact version/digest only as reproducibility evidence.

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

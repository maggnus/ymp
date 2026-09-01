---
id: W1-EVL-04o
kind: task
wave: W1
card: W1-EVL-04
state: active
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04l]
blocks: [W1-EVL-04p]
created_at: 2026-09-01T18:09:44+08:00
updated_at: 2026-09-01T18:26:00+08:00
started_at: 2026-09-01T18:15:48+08:00
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

# W1-EVL-04o — Probe MCP transport is exact and attestation-bound

## Outcome

The product has one purpose-built stdio MCP child named `ymp.workspace` that exposes exactly
`workspace_write` and `workspace_read`, and every probe trace/attestation binds a canonical
`probe_transport_digest` covering the exact executable, launch configuration, tool schema and
workspace root rather than trusting a server that merely advertises matching JSON.

## Scope

### In

- Add a strict probe transport identity to `ymp-runtime-api/src/lib.rs`. Its canonical digest binds
  MCP protocol version, server name/version, probe tool-schema digest, ordered allowed tools,
  server and launching `ymp` executable digests, internal subcommand/arguments, inherited-environment
  contract and canonical workspace-root digest. The exact identity is part of
  `ToolHostProbeRuntimeIdentity`, and therefore of the accepted trace and opaque Application
  attestation.
- The trusted foreground `ymp internal tool-host-probe` controller independently hashes its own
  admitted `current_exe` bytes and canonical probe child configuration before any child starts,
  using the same measured-executable rule as accepted `LaunchDescriptor` evidence. It passes this
  expected identity into `ControllerToolHostProbeRequest`; Application stores it in the immutable
  reservation before execution. The child/server cannot choose or rewrite the expected digest.
- Add a separate `WorkspaceProbeMcpServer` in `ymp-agent-mcp/src/lib.rs`; it is not an agent tool
  mode and advertises only the fixed `workspace_write(path, content)` and `workspace_read(path)`
  schemas from `TOOL_HOST_PROBE_TOOL_SCHEMA`. Unknown fields, wrong path/content, extra/reordered
  calls and every other tool fail closed.
- Add only `InternalCommand::ToolHostProbeMcp` and its handler in `ymp-cli/src/internal.rs`. It reads
  controller-set private environment, resolves the one canonical workspace root, refuses symlinks,
  absolute/traversing paths and pre-existing destination, writes exactly the nonce once and reads
  the same bytes once. It has no Application, board, task, recruitment, candidate or network access.
- Exclusive write zone: the three files above, plus the narrow expected-transport field,
  reservation/attestation comparison and fixtures in `ymp-application/src/tool_host_probe.rs`, and
  mechanically forced fixture updates only in
  `ymp-runtime-supervisor/tests/tool_host_probe.rs`. No other production behavior may change.

### Out

- No-touch: runtime-codex and runtime-supervisor production code, agent API/RPC, Application logic
  beyond the explicit expected-transport binding above, CLI lib/main/public surface, Cargo manifests/lockfile,
  corpus/manifest, TUI, research/calibration, real models/network/money and real user state.

## Acceptance

- [ ] Direct MCP initialization/list/call in a fresh root exposes exactly two ordered tools and one
      canonical server identity; write then read produces exact bytes only at the controller-bound
      relative path and no file elsewhere.
- [ ] The canonical `probe_transport_digest` changes when server/launcher executable bytes, internal
      command/configuration, protocol/name/version, tool schema/order or canonical workspace root
      changes. A schema-identical substitute child or path changed after the controller's prelaunch
      measurement therefore produces a different actual identity; W1-EVL-04n owns the mandatory
      immediate pre-spawn comparison and refusal.
- [ ] `ToolHostProbeTrace` and recovered `AttestedToolHostProbe` bind the exact transport identity;
      missing/default/altered transport identity, a copied root or mismatched executable/configuration
      fails before attestation or `model_ready`.
- [ ] The expected executable/configuration digest is measured and fsynced by the foreground
      controller before child creation, not self-reported by the MCP child. Replacing only the child
      while retaining its JSON schema fails; replacing the entire trusted foreground executable is
      explicitly the signed-release/TCB boundary, not a claim of this probe.
- [ ] Absolute/traversing/symlink/pre-existing paths, wrong nonce, duplicate/reordered/extra calls,
      unknown fields, additional tools and inherited unapproved environment each fail without an
      accepted read-back or file outside the disposable workspace.
- [ ] Focused runtime-api/MCP/internal-CLI/Application fixture tests, one mutation replacing the
      executable while retaining the same JSON schema and proving a different transport digest,
      strict affected-package Clippy, formatting
      and `git diff --check` pass with zero model/network/money calls and isolated
      project/HOME/YMP_HOME/TMPDIR/build/export.

## Current state

The accepted fake trace binds only `tool_host_probe_tool_schema_digest`; no real MCP server exists.
A schema-identical substitute could create expected bytes and produce a false reachability claim.
W1-EVL-04l already stores the full runtime identity, so adding one exact transport identity closes
the gap without changing its authority or persistence model.

## Next action

Run a Critical contract check of the three-file transport seam before dispatching one Sol xhigh
builder; W1-EVL-04n remains blocked until this identity and server are accepted.

## Guardrails

- This is a two-tool compatibility fixture, not a general filesystem or agent MCP API.
- The server implementation is evidence only when independently hashed by the launcher/controller;
  self-reported identity is not authority.
- The trust anchor is the already trusted foreground product process, matching existing measured
  launch evidence; this task does not invent signing or defend against replacement of the whole TCB.
- Transport reachability is not communication, listening, task value or self-organization.

## Findings

- Scientific peer review required implementation/configuration binding in addition to JSON schema,
  because a substituted server can otherwise manufacture expected bytes.
- R1 contract review required an independent expected digest. The corrected source is the foreground
  controller's prelaunch measurement persisted in the Application reservation, never the child
  server's self-report.
- R1 implementation review confirmed measurement and persistence but found final TOCTOU refusal
  impossible before a live start exists. The immediate byte/config/root recheck is now an explicit
  W1-EVL-04n responsibility; 04o claims no live-process refusal.

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

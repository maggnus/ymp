---
id: W1-EVL-04j
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04h, W1-EVL-04i]
blocks: [W1-EVL-04l]
created_at: 2026-09-01T15:41:48+08:00
updated_at: 2026-09-01T15:57:50+08:00
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

# W1-EVL-04j — Runtime executes a bounded no-task-output probe

## Outcome

The runtime API and supervisor execute one separately budgeted no-task-output workspace probe and
return a complete, explicitly untrusted trace of the scoped write/read attempt, route, usage, events
and terminal state. This seam cannot attest controller ownership, persist evidence or set
`model_ready=true`.

## Scope

### In

- Strict `ToolHostProbeRequest` and `ToolHostProbeTrace` types carrying probe/invocation identity,
  caller-supplied nonce and relative workspace path, deadline, exact resource reservation, route,
  tool/event/output digests, usage and terminal state.
- One minimal managed invocation with workspace read/write only: no network, board, task contract,
  recruitment, candidate, verifier query or experimental arm.
- Exclusive write zone: `ymp-rust/crates/ymp-runtime-api/src/lib.rs`,
  `ymp-rust/crates/ymp-runtime-supervisor/src/lib.rs` and optional new
  `ymp-rust/crates/ymp-runtime-supervisor/tests/tool_host_probe.rs`.

### Out

- No-touch: `ymp-application/**`, `ymp-cli/**`, `ymp-corpus/**`, all Cargo manifests/lockfile,
  runtime driver packages, real task prompts or output, collaboration messages,
  primary/development arms, TUI, persistence, deployment and live paid execution.

## Acceptance

- [ ] A fake managed runtime receives one opaque nonce and one normalized relative path rooted in
      its disposable workspace, is offered only read/write tools, terminates honestly and returns a
      trace with exact invocation, route/profile/CLI/driver/tool schema, usage, wall time,
      cost-availability and output/event digests.
- [ ] Missing tool event, absolute or traversing path, incomplete usage, ambiguous terminal,
      timeout, cancellation, extra tool use, extra output or any board/task/recruitment/candidate
      effect fails for a typed reason and yields no successful trace.
- [ ] Start and completion consume one separately reserved resource vector; arm budget, task result,
      candidate or communication records cannot pay for or satisfy the probe.
- [ ] A successful trace is explicitly untrusted: it exposes no constructor or flag that can claim
      controller read-back, persistence, attestation or model readiness.
- [ ] Focused API and supervisor tests, one fake-process boundary walk, strict affected-package
      Clippy, formatting and `git diff --check` pass; no real model/network/money call runs.

## Current state

W1-EVL-04h defines the admission evidence schema but correctly refuses it. Existing runtime types
carry managed requests, events and usage, but no bounded probe request/trace exists. R1 contract
review split execution from controller authority and corpus consumption. No live probe is authorized.

## Next action

After W1-EVL-04i is accepted, dispatch one Sol xhigh builder for this runtime-only seam.

## Guardrails

- The trace reports execution only; it is never an attestation or admission decision.
- The caller supplies the nonce and path; the runtime may neither widen authority nor choose them.
- A real probe remains a separate owner model/money gate after all fake-runtime nodes are accepted.

## Findings

- R1 contract review rejected the former five-subsystem atom. This node now owns only API and
  supervisor execution; W1-EVL-04l owns controller authority and W1-EVL-04m owns admission use.

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

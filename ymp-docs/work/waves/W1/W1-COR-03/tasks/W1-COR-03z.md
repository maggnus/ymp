---
id: W1-COR-03z
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03c, W1-COR-03e.1]
blocks: [W1-EVL-04a, W1-EVL-04b]
created_at: 2026-09-01T12:20:00+08:00
updated_at: 2026-09-01T12:20:00+08:00
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

# W1-COR-03z — Agent tools publish and read collaboration through Application

## Outcome

External Codex and Claude participants can publish bounded inert collaboration messages and read
their authorized board audiences through explicit agent tools backed by the Application API, while
caller identity, scope, budgets, payload storage, and authority remain controller-owned.

## Scope

### In

- Typed `publish` and bounded `read_board` commands in the agent API, RPC/MCP binding, and generated
  runtime tool allowlists.
- Application-owned payload persistence/resolution, controller-derived participant identity, and
  audience/byte accounting.
- Fake-runtime and binding tests for exact tool effects and fail-closed authorization.

### Out

- TUI rendering, new board semantics, causal analysis, automatic collaboration policy, and any
  generic command or arbitrary object/fetch interface.

## Acceptance

- [ ] An authorized participant publishes exact bytes, the board records their digest and
      attribution, and a permitted reader receives the same bounded message through explicit tools.
- [ ] Tool arguments contain no caller-selected principal, capability, filesystem path, protected
      oracle reference, generic command, or arbitrary URL.
- [ ] Unauthorized scope, forged identity, oversized/mismatched payload, duplicate command, and
      over-budget read fail without a second effect or collaboration/control-plane leakage.
- [ ] Codex and Claude generated configurations expose the new tools only for a contract that grants
      them; focused binding tests, strict affected-package Clippy, formatting, and diff-check pass.

## Current state

The board domain and persistent Application section exist, but agent tool schemas and both managed
runtime allowlists expose only `read_control`, `read_events`, `yield`, and `submit`. No external
participant can yet publish or read collaboration messages through `ymp`.

## Next action

Start after W1-COR-03e.1 is accepted; define the narrow tool schema before changing bindings.

## Guardrails

- The private endpoint supplies the acting identity; model-supplied arguments never do.
- Message bytes are inert object data and cannot invoke tools or mutate the control plane.
- One explicit tool maps to one domain/application operation; no generic execution proxy is added.

## Findings

- None.

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

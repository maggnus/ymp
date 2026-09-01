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
- `publish` accepts model-controlled `command_id`, existing tagged `Audience`, `MessageKind`, UTF-8
  `content`, `salience_ms`, references, relation, and claimed decision basis. The handler derives
  author, payload length, and digest; `command_id` is the durable message identity.
- `read_board` accepts only `limit_bytes` in `1..=MAX_DELIVERY_BYTES` (32 KiB); the private endpoint
  supplies the reader, and the result contains owned resolved messages the board admits for it.
- Application-owned payload persistence/resolution, controller-derived participant identity, and
  audience/byte accounting.
- Fake-runtime and binding tests for exact tool effects and fail-closed authorization.
- Write zone: `ymp-agent-api`, `ymp-agent-rpc`, `ymp-agent-mcp`, `ymp-runtime-codex`,
  `ymp-runtime-claude`, and the narrow agent-session dispatch in `ymp-application`.

### Out

- TUI rendering, new board semantics, causal analysis, automatic collaboration policy, and any
  generic command or arbitrary object/fetch interface.
- `ymp-domain`, research documents, and unrelated application control or verification paths.

## Acceptance

- [ ] An authorized participant publishes exact bytes, the board records their digest and
      attribution, and a bound live reader receives the same owned UTF-8 bytes only when
      `BoardLedger::may_read` admits its project-discovery, scope, or named audience.
- [ ] Tool arguments contain no caller-selected author/reader/principal, capability, payload digest
      or length, filesystem/workspace path, protected-oracle reference, generic command, or URL;
      unknown fields such as `author`, `reader`, `payload_digest`, or `payload_bytes` are rejected.
- [ ] A 513-byte project-discovery payload and an 8193-byte detailed payload are rejected against
      `MAX_DISCOVERY_PAYLOAD_BYTES` and `MAX_PAYLOAD_BYTES`; the handler, not the model, computes the
      digest and byte length from the accepted UTF-8 content.
- [ ] Publishing to a scope or named audience without publish rights, calling from an unbound or
      non-live endpoint, repeating one `command_id`, reading with 0 or 32769 bytes, and a delivery
      exceeding the reader's `DeliveredBytes` allowance fail or stop at the existing board limit
      without duplicate publication, cursor corruption, unpaid bytes, or control-plane disclosure.
- [ ] A participant outside a scope or named audience does not receive that message; a member with
      a valid read grant does, through the same `read_board { limit_bytes }` schema.
- [ ] Codex and Claude generated configurations expose the new tools only for a contract that grants
      them; focused binding tests, strict affected-package Clippy, formatting, and diff-check pass.

## Current state

The board domain and persistent Application section exist, but agent tool schemas and both managed
runtime allowlists expose only `read_control`, `read_events`, `yield`, and `submit`. No external
participant can yet publish or read collaboration messages through `ymp`.

## Next action

Repeat the contract check on the explicit schemas and refusal inputs, then start after
W1-COR-03e.1 is accepted.

## Guardrails

- The private endpoint supplies the acting identity; model-supplied arguments never do.
- Message bytes are inert object data and cannot invoke tools or mutate the control plane.
- One explicit tool maps to one domain/application operation; no generic execution proxy is added.
- No changes to TUI, `ymp-domain`, or `ymp-docs/research`; any missing board rule returns to the CTO
  instead of being added inside this task.

## Findings

- Pre-dispatch check R0 returned the former phrases "permitted reader" and
  "oversized/mismatched" as non-falsifiable; the contract now names exact schemas, bounds, and
  refusal inputs.

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

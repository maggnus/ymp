---
id: W1-COR-03e.4
kind: subtask
wave: W1
card: W1-COR-03
parent: W1-COR-03e
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03e.1, W1-COR-03z]
blocks: [W1-COR-03e.2]
created_at: 2026-09-01T21:48:42+08:00
updated_at: 2026-09-01T21:52:00+08:00
started_at: 2026-09-01T21:52:00+08:00
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

# W1-COR-03e.4 — Operator projection preserves exact message audience

## Outcome

The owned operator board projection preserves each persisted message's exact typed audience —
project discovery, scope, or named recipients — together with author, kind, order and verified
payload bytes, without exposing board/storage authority or letting a reader/TUI reconstruct scope.

## Scope

### In

- Add the existing owned `Audience` value to `ymp-board::observatory::MessageView` and copy it exactly
  in `view_of` from the committed `MessageRecord`.
- Preserve the same value through `ymp-application::ResolvedMessageView` and
  `Application::operator_board_projection()` while retaining object length/digest verification and
  owned-copy isolation.
- Focused board/application tests for all audience variants, reopen, mutation isolation and order;
  compile-fail/static checks keep ledger/store/root and mutation authority private.
- Exclusive write zone: `ymp-board/src/observatory.rs`, narrow `ymp-application/src/lib.rs` projection
  types/mapping, and focused tests under those two packages only; forced manifests/lockfile only if
  compilation requires them.

### Out

- No-touch: audience authorization/visibility rules, board records/protocol transitions, agent RPC/MCP,
  TUI, storage layout, runtime, interventions and causal interpretation. This task transports a fact;
  it neither grants delivery nor decides who may read.

## Acceptance

- [ ] Project-discovery, scope and named-recipient messages each emerge from the operator projection
      with an audience exactly equal to the committed record, exact author/kind/order and payload.
- [ ] Reopen returns the same audience; mutating any returned owned audience/recipient vector cannot
      affect later reads, audit order or durable state.
- [ ] Missing/corrupt/mismatched payload still fails before returning a partial projection; audience
      presence cannot mask object failure or fabricate placeholder content.
- [ ] A mutation omitting audience, deriving it from reader/text/relation or collapsing named/scope
      variants fails focused tests. No public ledger/store/root/mutable accessor appears.
- [ ] Focused board/application tests, strict Clippy for both packages, formatting and
      `git diff --check` pass with no model/network/TUI run.

## Current state

Committed `MessageRecord` already stores exact audience, but `MessageView/view_of` drops it and
Application consequently cannot expose it. Payload persistence and the model-facing publish/read
seam are accepted; only this owned read projection is incomplete.

## Next action

Run a Significant contract check of the two-package type propagation, then dispatch one Sol high
builder; resume the retained W1-COR-03e.2 author only after integration.

## Guardrails

- Audience is inert provenance in this projection, never a capability or delivery claim.
- Copy the committed typed value; never derive, normalize or summarize it.
- Projection additions remain owned and read-only across the model/operator boundary.

## Findings

- Created from the W1-COR-03e.2 preflight blocker; no TUI code was changed before this prerequisite.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

## Closure

Filled when the subtask is accepted.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.

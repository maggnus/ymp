---
id: W1-EXP-01d.1
kind: subtask
wave: W1
card: W1-EXP-01
parent: W1-EXP-01d
state: active
risk: significant
maturity: RESEARCH
relation: required
depends_on: []
blocks: [W1-APP-02c]
created_at: 2026-08-12T20:31:00+08:00
updated_at: 2026-08-12T21:31:00+08:00
started_at: 2026-08-12T20:42:23+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/4d7b1f59d9f8b5eca6e64e103a2cb532923cda33
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger: Re-review after exact route binding is durable, validator-enforced, and the admission probe is repeated
deliberate_partial: false
---

# W1-EXP-01d.1 — Codex profile satisfies the primary-comparison runtime contract

## Outcome

The pinned Codex runtime profile has accepted, reproducible evidence for lifecycle continuity,
isolated configuration, tool binding, usage accounting, bounded stopping, and rejection of
incompatible fixtures, so it can be admitted to the primary comparison independently of Claude.

## Scope

### In

- Exact `codex-cli 0.147.0` capability and authentication probe through `ymp`.
- Fake-runtime coverage for start, resume, interrupt, yield, wake, replay, malformed events, and
  duplicate replies without model-provider access.
- Synthetic-home isolation, agent-originated submission, structured usage evidence, cancellation,
  in-flight excess accounting, and complete descendant cleanup.
- Versioned capability-matrix and calibration evidence for the Codex profile only.

### Out

- Claude Code authentication, runtime evidence, and admission.
- Production runtime-driver features owned by W1-APP-02c.
- OpenCode, Nemotron, strict containment, and terminal-screen scraping.

## Acceptance

- [ ] The exact managed Codex probe records version, effective isolated configuration, lifecycle,
  invocation-scoped tool binding, session continuity, tokens, wall time, protected queries,
  available provider cost, cancellation, and in-flight excess under the frozen study rule.
- [ ] A generated synthetic home admits only the pinned profile and demonstrates one
  agent-originated idempotent candidate submission through the product-owned MCP path.
- [ ] Resume, yield, wake, interruption, timeout, and descendant cleanup preserve one session and
  command identity without leaving a process that can commit a later effect.
- [ ] Fixtures that omit a required event, leak ambient configuration, reapply a duplicate reply,
  cannot terminate descendants, or lack reproducible usage are rejected with non-zero exits.

## Current state

Candidate `4d7b1f59` honestly records a provider-credit failure, but independent review returned the
package because it does not bind that result to the exact provider, approved non-personal account
or quota scope, authentication mode, and scenario digest. A route-binding mutation with a
recomputed hash is accepted, so the incompatibility cannot yet be attributed to the contracted
route; existing lifecycle and negative-fixture evidence remains valid for bounded rework.

## Next action

Record and validate the exact non-secret route identity and scenario digest, add the route-binding
negative control, then repeat the admission probe and return the same candidate lineage to the
preserved reviewer.

## Guardrails

- Do not require, inspect, or modify Claude Code credentials or user configuration.
- Do not change production runtime behavior owned by W1-APP-02c.
- Real provider calls must use the approved public-code route and record all usage categories.
- Native subagents, remote execution, background sessions, and unrelated tool servers remain disabled.

## Findings

- Independent review reproduced the synthetic-home leak with its preselected external TUI
  falsifier and found that a route-binding mutation is accepted after its hash is recomputed.
- No communication-integrity signal was found in the author return or reviewer verdict.

## Closure

Not accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

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
updated_at: 2026-08-12T22:03:31+08:00
started_at: 2026-08-12T20:42:23+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/d2cf46e445648710b9ee9a0b7b601b66bec3df33
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger: Second independent RETURN reached the convergence rule; no third research rework
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

Candidate `d2cf46e` binds and validates the declared route policy, but repeat review proved that the
managed Codex process still inherits `OPENAI_ORGANIZATION` and `OPENAI_PROJECT`. The actual request
may therefore use a different scope from the preliminary route check. The profile is reproducibly
ineligible because ambient configuration reaches the managed process; the observed credit failure
is not accepted as evidence about the declared route.

## Next action

Apply the convergence rule: close this research subtask as a bounded non-admission result and move
environment isolation plus a new exact-route probe to W1-APP-02c. Do not start a third correction
of the research package.

## Guardrails

- Do not require, inspect, or modify Claude Code credentials or user configuration.
- Do not change production runtime behavior owned by W1-APP-02c.
- Real provider calls must use the approved public-code route and record all usage categories.
- Native subagents, remote execution, background sessions, and unrelated tool servers remain disabled.

## Findings

- The first review found unbound route identity; the correction made all 12 route-policy fields
  validator-enforced and repeated the exact scenario.
- Repeat review independently observed `OPENAI_ORGANIZATION` and `OPENAI_PROJECT` inside the
  managed process, so route scope remains affected by ambient configuration.
- No pressure, concealment, verdict negotiation, or author-reviewer coordination was observed in
  either return. The repeated defect is technical and triggers convergence, not a collusion finding.

## Closure

Pending the bounded convergence closure.

### Accepted outcome

The exact Codex profile is not admitted because the managed process receives ambient route
configuration. Lifecycle and route-policy checks remain useful, but the provider-credit result is
not attributed to the declared route.

### Residuals

- The provider-credit observation is excluded from route-specific conclusions.
- Production environment isolation and the post-fix exact-route probe belong to W1-APP-02c.

### Evidence

- [Reviewed candidate d2cf46e](https://github.com/maggnus/ymp/commit/d2cf46e445648710b9ee9a0b7b601b66bec3df33)
- Preserved external falsifier `/tmp/ymp-01d1-falsifier`, which rejected both reviewed candidates.

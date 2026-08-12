---
id: W1-APP-02c
kind: task
wave: W1
card: W1-APP-02
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02a, W1-APP-02b, W1-EXP-01d.1]
blocks: [W1-APP-02e]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T22:06:42+08:00
started_at: 2026-08-12T22:06:42+08:00
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02c — Codex profile completes one managed candidate attempt

## Outcome

The foreground application detects an approved Codex profile, launches and supervises it through a
compiled driver, exposes only the invocation-scoped coordination tools, and records enough
lifecycle and usage evidence to include the profile in the POC comparison.

## Scope

### In

- Pinned Codex executable and App Server protocol, start, structured events, resume, interrupt,
  cancellation, session capsule, usage evidence, and generated isolated configuration.
- Per-invocation `ymp internal agent-mcp` over stdio and private ymp RPC to the foreground core.
- Exact model-route and coordination-binding provenance, including declared observational limits.

### Out

- Terminal-screen scraping, Codex-native subagents, remote execution, ambient MCP servers, direct
  store access, Nemotron routing, and a generic provider adapter.

## Acceptance

- [ ] From a generated synthetic home, ymp detects the exact Codex version and authentication
  readiness, starts the process itself, completes one fake-project candidate, and records the
  runtime session and usage evidence.
- [ ] Resume, explicit yield and wake, interruption, and full process termination preserve one
  attempt and command identity without duplicating a shared effect.
- [ ] A lost MCP reply followed by a repeated command identifier returns the original committed
  result; an ambiguous call is never replayed under a fresh identifier automatically.
- [ ] A profile with ambient configuration, enabled native subagents, incompatible structured
  events, missing reproducible usage, or no bounded stop mechanism is rejected before a run.

## Current state

The compiled driver pins the intended executable, route, sandbox, process supervision, and
attempt-scoped MCP server, but the admission probe is rejected. Independent review observed
`OPENAI_ORGANIZATION` and `OPENAI_PROJECT` inside the managed process, so the actual provider scope
can differ from the preliminary route check. Existing L1-L3 and TUI evidence remains development
calibration and does not admit the profile to the primary comparison.

## Next action

Generate the complete isolated home and allowlisted process environment before launch, reject any
ambient route or MCP configuration, then repeat the exact-route probe. Preserve existing lifecycle,
idempotent submission, usage, cancellation, and descendant-cleanup behavior.

## Guardrails

- MCP is an agent-facing binding, not the kernel, event store, or runtime lifecycle protocol.
- Model-provider compatibility is recorded as a route property and is not inferred from Codex.
- The operator starts only `ymp`; no manual second-terminal process is part of acceptance.

## Findings

- W1-EXP-01d.1 closed after two independent returns with a bounded non-admission result: route
  policy is declared and validator-bound, but ambient organisation and project scope still reach
  the managed Codex process.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- `ymp-docs/CALIBRATION.md`
- `ymp-rust/tools/ymp-calibration/results/2026-08-12-smoke.json`

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
updated_at: 2026-08-12T22:32:10+08:00
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

The compiled driver pins the intended executable and coordination server, but the admission probe
is rejected because route and home configuration reaches the managed process. A 21-file reviewer
falsifier frozen before the candidate rejects baseline `f4bb3e7` for that exposure, lost cost and
excess accounting, and a surviving descendant; existing resume, idempotency, missing-usage,
timeout, and active-tree checks pass. This evidence does not yet admit the profile.

## Next action

Generate the complete isolated home and allowlisted process environment before launch, reject any
ambient route or MCP configuration, then repeat the exact-route probe. Preserve existing lifecycle,
idempotent submission, usage, cancellation, and descendant-cleanup behavior. When the author
returns a clean exact candidate, fast-forward the preserved reviewer and run the frozen falsifier
without modification.

## Guardrails

- MCP is an agent-facing binding, not the kernel, event store, or runtime lifecycle protocol.
- Model-provider compatibility is recorded as a route property and is not inferred from Codex.
- The operator starts only `ymp`; no manual second-terminal process is part of acceptance.

## Findings

- W1-EXP-01d.1 closed after two independent returns with a bounded non-admission result: route
  policy is declared and validator-bound, but ambient organisation and project scope still reach
  the managed Codex process.
- Reviewer package SHA-256 manifest `423a05212ab005047583c5d489905e3c604100c0c0ec8a804b10bc3ac03ea12d`
  is frozen, read-only, and has a reproducing negative control on the exact baseline.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- `ymp-docs/CALIBRATION.md`
- `ymp-rust/tools/ymp-calibration/results/2026-08-12-smoke.json`

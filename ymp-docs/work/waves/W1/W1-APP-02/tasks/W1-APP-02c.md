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
updated_at: 2026-08-13T00:22:02+08:00
started_at: 2026-08-12T22:06:42+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/99cd5d2ade9d8d5d2de04f20799c9c4d5c0e6358
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

Corrected [candidate `99cd5d2`](https://github.com/maggnus/ymp/commit/99cd5d2ade9d8d5d2de04f20799c9c4d5c0e6358)
binds launch evidence to the executed process, sanitizes durable failures, terminates descendants,
records terminal accounting, and carries managed yield/wake/resume through the product path. The
author reports all targeted, workspace, Clippy, formatting, and executable-model checks successful.
The candidate is awaiting repeat review; W1-APP-02d implementation remains paused.

## Next action

Review exact candidate `99cd5d2` with the same frozen falsifier and recheck every returned finding
against the corrected range. Accept only if no child diagnostic or secret reaches durable state,
descendants terminate after parent failure, all terminal outcomes retain reproducible accounting,
yield/wake/resume preserve command identity, and launch evidence is derived from the exact process.
Start the W1-APP-02d writer only from the accepted integrated runtime base.

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
- The first review found that an MCP token supplied through the child environment can be echoed to
  `stderr` and persisted as an infrastructure-error reason. This is a blocker because it can
  disclose a secret without a reliable detection point.
- The same review reproduced a descendant surviving parent exit and found that cost, non-zero
  in-flight excess, managed yield/wake/resume, and launch-bound evidence are absent or incomplete.
  These are defects in this task's contracted BUILD outcome, not adjacent refinements.
- The reviewer's `OPENAI_BASE_URL` subcheck distinguishes only presence and therefore incorrectly
  described the candidate's pinned value as inherited. The reviewer excluded that subcheck from
  the verdict; the code clears the ambient value before setting the pinned route.
- No pressure, concealment, verdict negotiation, author-reviewer contact, or weakening of reviewer
  independence was observed. Provider-family diversity could not be confirmed, so the preselected
  external falsifier remains the compensating independent check.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- `ymp-docs/CALIBRATION.md`
- `ymp-rust/tools/ymp-calibration/results/2026-08-12-smoke.json`

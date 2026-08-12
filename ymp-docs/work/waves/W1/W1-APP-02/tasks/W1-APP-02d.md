---
id: W1-APP-02d
kind: task
wave: W1
card: W1-APP-02
state: blocked
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02a, W1-APP-02b, W1-EXP-01d.2]
blocks: [W1-APP-02e]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T20:31:00+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker: W1-EXP-01d.2 Claude Code runtime admission
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02d — Claude Code profile completes one managed candidate attempt

## Outcome

The foreground application detects an approved Claude Code profile, launches and supervises it
through a compiled driver, exposes only invocation-scoped coordination tools, and records enough
lifecycle and usage evidence to include the profile in the POC comparison.

## Scope

### In

- Pinned Claude Code non-interactive structured interface, start, streaming input and output,
  explicit session identity, resume, interrupt, cancellation, usage evidence, and isolated
  generated configuration.
- Per-invocation stdio MCP projection and private ymp RPC to the foreground core.
- Exact harness, model-route, budget-limit, and binding provenance.

### Out

- Claude Code native subagents, background or remote work, ambient hooks and plugins, direct store
  access, Nemotron routing, and treating Anthropic Messages compatibility as conformance evidence.

## Acceptance

- [ ] From a generated synthetic home, ymp detects the exact Claude Code version and authentication
  readiness, starts it itself, completes one fake-project candidate, and records its session and
  usage evidence.
- [ ] Resume, explicit yield and wake, interruption, budget stop, and full descendant termination
  preserve one attempt and command identity.
- [ ] Duplicate or malformed structured events and MCP replies become typed runtime or binding
  failures without committing a second state change.
- [ ] A profile with ambient configuration, enabled native subagents, incompatible structured
  events, missing usage evidence, or unenforceable overshoot beyond `G3` is rejected before a run.

## Current state

The compiled driver pins Claude Code `2.1.227`, `claude-opus-5`, low effort,
`ymp-claude-low-v1`, disabled configuration sources, strict generated MCP configuration,
restricted tools, bounded structured output, and a USD 1.00 stop. Its fixture preserves session,
output, token and currency-cost evidence. The exact probe reports the local profile as
unauthenticated; a real Paseo L1 run ended before model use.

## Next action

Authenticate the local Claude Code installation, run the L1-L3 ladder through `ymp`, and validate
the observed stream against the fixture before exercising the configured attempt-scoped MCP
bridge.

## Guardrails

- Runtime configuration is generated per invocation and never mutates user configuration.
- A Messages-compatible endpoint does not become an approved route without exact conformance.
- The operator starts only `ymp`; no manual second-terminal process is part of acceptance.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

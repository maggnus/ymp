---
id: W1-EXP-01d
kind: task
wave: W1
card: W1-EXP-01
state: active
risk: significant
maturity: RESEARCH
relation: required
depends_on: [W1-EXP-01b]
blocks: []
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T21:18:00+08:00
started_at: 2026-08-12T20:31:00+08:00
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

# W1-EXP-01d — Runtime probes expose incompatible POC profiles

## Outcome

A fake deterministic runtime, malformed MCP client, and exact Codex and Claude Code probes produce
a versioned capability matrix that admits only profiles with the lifecycle, configuration
isolation, usage evidence, and stopping behavior required by the experiment.

## Scope

### In

- Fake start, resume, interrupt, yield, wake, malformed event, duplicate reply, and ambiguous MCP
  behaviors without model cost.
- Pinned Codex and Claude Code executable versions, structured event formats, session continuity,
  tool projection, configuration sources, native subagent suppression, cancellation, usage, and
  hard or observational cost limits.
- Separate runtime-driver, model-route, and coordination-binding identities.

### Out

- Production runtime drivers, OpenCode admission, Nemotron route quality, strict containment, and
  terminal-screen scraping as a control protocol.

## Acceptance

- [ ] The fake runtime and MCP client deterministically reproduce duplicate replies, malformed
  events, session resume, yield and wake, cancellation, and replay without model-provider access.
- [ ] Exact Codex and Claude Code probes record version, effective configuration, structured
  lifecycle, tool binding, usage evidence, cancellation, and subagent behavior.
- [ ] A fixture that omits a required event, leaks ambient configuration, cannot terminate its
  descendants, or lacks reproducible usage is rejected from the primary comparison.
- [ ] Each call records tokens, wall time, protected queries, provider cost when available, and
  in-flight excess under the preregistered accounting rule; missing reproducible accounting makes
  the profile ineligible.

## Current state

The runtime contract is split into independently acceptable Codex and Claude Code profiles.
W1-EXP-01d.1 is under independent review after recording a reproducible route incompatibility.
Claude Code OAuth authentication is now available, so W1-EXP-01d.2 can execute independently. The
parent closes only after both profiles have accepted evidence.

## Next action

Complete the independent review of W1-EXP-01d.1 while W1-EXP-01d.2 runs the exact authenticated
Claude Code admission probe and L1-L3 ladder.

## Guardrails

- Product names never substitute for measured capabilities of an exact version and route.
- Runtime-native subagents, remote execution, and background sessions remain disabled in POC.
- Real provider probes use only experiment-specific accounts and approved disclosure.

## Findings

- `ymp-docs/CALIBRATION.md`
- `ymp-rust/tools/ymp-calibration/results/2026-08-12-smoke.json`

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

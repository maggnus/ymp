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
updated_at: 2026-08-12T22:04:20+08:00
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

The Codex subtask is accepted with a bounded non-admission result: ambient organisation and project
scope reaches the managed process, so the declared route cannot be attributed to the actual request.
The production correction now belongs to ready task W1-APP-02c. Claude evidence remains under
bounded rework after review proved that its self-certified report permits false attribution. The
parent closes only after the Claude subtask has accepted evidence.

## Next action

Complete the bounded primary-observation correction and repeated independent review for
W1-EXP-01d.2. W1-APP-02c may proceed independently with the accepted Codex non-admission evidence.

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

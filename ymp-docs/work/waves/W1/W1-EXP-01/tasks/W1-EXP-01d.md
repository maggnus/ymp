---
id: W1-EXP-01d
kind: task
wave: W1
card: W1-EXP-01
state: blocked
risk: significant
maturity: RESEARCH
relation: required
depends_on: [W1-EXP-01b]
blocks: [W1-APP-02c, W1-APP-02d]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T06:38:24+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker: W1-EXP-01b, Claude authentication, complete managed submission, and remaining G3 budget decisions
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
- [ ] Each unavoidable cost overshoot is measured and either falls within the approved tolerance
  or makes the profile ineligible.

## Current state

Exact-version and authentication probes distinguish ready, unauthenticated, incompatible,
unavailable, and missing profiles. Both drivers isolate configuration, parse bounded events,
terminate process groups, record usage, and pass fixtures. All L1-L3 levels have three accepted
managed Codex repetitions; later L2/L3 runs exercised TUI verification and evidence export.
Another L3 run submitted its bound workspace through MCP and passed the oracle; Claude OAuth is absent.

## Next action

Authenticate Claude Code and repeat the L1-L3 ladder through `ymp`; then add its timeout and
descendant-cleanup fixtures before primary admission.

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

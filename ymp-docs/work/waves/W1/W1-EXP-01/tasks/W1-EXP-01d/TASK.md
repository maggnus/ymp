---
id: W1-EXP-01d
kind: task
wave: W1
card: W1-EXP-01
state: accepted
risk: significant
maturity: RESEARCH
relation: required
depends_on: [W1-EXP-01b]
blocks: []
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T22:44:30+08:00
started_at: 2026-08-12T20:31:00+08:00
accepted_at: 2026-08-12T22:44:30+08:00
candidate_commit:
closure_commit: https://github.com/maggnus/ymp/commit/ca221c9a4b81775bf158cf915d0d7613e9daae92
evidence: ["[Codex bounded result](subtasks/W1-EXP-01d.1.md)", "[Claude bounded result](subtasks/W1-EXP-01d.2.md)", "[Convergence decision ca221c9](https://github.com/maggnus/ymp/commit/ca221c9a4b81775bf158cf915d0d7613e9daae92)"]
duration_minutes: 133
blocker:
pause_reason:
return_trigger:
deliberate_partial: true
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

Both exact profiles are reproducibly ineligible for the primary comparison. Codex receives ambient
route scope; Claude receives ambient configuration, loses error-path accounting, and leaves a
descendant after its parent exits. Both research evidence packages reached the two-return
convergence limit; only independently confirmed non-admission findings are accepted.

## Next action

Apply the independently confirmed production corrections in W1-APP-02c and W1-APP-02d, then run
new exact managed probes through the product. Do not reopen either research evidence package.

## Guardrails

- Product names never substitute for measured capabilities of an exact version and route.
- Runtime-native subagents, remote execution, and background sessions remain disabled in POC.
- Real provider probes use only experiment-specific accounts and approved disclosure.

## Findings

- `ymp-docs/research/RES-001-calibration.md`
- `ymp-rust/tools/ymp-calibration/results/2026-08-12-smoke.json`
- Both provider-specific subtasks ended with independent non-admission evidence and no observed
  pressure, concealment, verdict negotiation, or author-reviewer coordination.

## Closure

### Accepted outcome

The deterministic fixtures and independent falsifiers identify why neither exact profile is
eligible. This deliberately partial result is sufficient to define production corrections, but
neither profile is admitted and no provider-quality comparison may start from these packages.

### Residuals

- Codex production isolation and a post-fix exact probe remain in W1-APP-02c.
- Claude production isolation, complete accounting and process cleanup, and a post-fix exact probe
  remain in W1-APP-02d.

### Evidence

- [Codex bounded result](subtasks/W1-EXP-01d.1.md)
- [Claude bounded result](subtasks/W1-EXP-01d.2.md)
- [Convergence decision ca221c9](https://github.com/maggnus/ymp/commit/ca221c9a4b81775bf158cf915d0d7613e9daae92)

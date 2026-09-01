---
id: W1-PRD-05j.1
kind: subtask
wave: W1
card: W1-PRD-05
parent: W1-PRD-05j
state: ready
risk: significant
maturity: BUILD
relation: follow_up
depends_on: [W1-PRD-05j]
blocks: [W1-EVL-04a]
created_at: 2026-09-01T12:38:00+08:00
updated_at: 2026-09-01T12:38:00+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 0
escalation_decision:
---

# W1-PRD-05j.1 — Agent tool recruits through the accepted Application gate

## Outcome

A managed participant can request one frozen-pool participant through its private agent endpoint;
the request reaches the accepted application gate and managed start path with caller identity bound
by the endpoint, not supplied by the model.

## Scope

### In

- A typed `request_participant` agent call carrying only a fresh request identifier and frozen entry
  identifier.
- Agent API parsing, RPC/MCP exposure, invocation-bound application dispatch, and the minimum
  runtime allowlist or capability wiring required to expose the call to an admitted participant.
- Focused proof that the private endpoint supplies the proposer identity and that the accepted
  `Application::request_participant` gate performs the admission and start exactly once.

### Out

- Semantic selection, ranking, role assignment, replacement policy, board publication or reading,
  TUI changes, and any new recruitment gate.
- A general application proxy, caller-supplied principal, filesystem path, workspace authority, or
  capability token in model-controlled arguments.

## Acceptance

- [ ] An admitted participant invokes the agent call with `{request_id, entry}`; its bound identity
      is used as proposer, the accepted mechanical gate admits one participant, and the existing
      managed start path is observed.
- [ ] Repeating the call or its delivery admits and starts no second participant and records no
      second admission.
- [ ] An entry outside the frozen pool, an unbound or no-longer-running caller, exhausted budget,
      concurrency refusal, runtime refusal, and unaffordable communication charge fail through the
      existing application reasons without admission or start side effects.
- [ ] The public tool schema exposes no proposer, principal, path, workspace, profile, route,
      capability, score, rank, or role-selection field.
- [ ] Focused agent API, RPC/MCP, application and touched runtime checks pass; strict Clippy for
      touched packages, formatting, and `git diff --check` report no warnings or errors.

## Current state

The accepted kernel and application gate exist, but the participant-facing catalogue exposes only
control reading, event reading, submission, and yielding. No model-callable recruitment path exists,
so a coordinated arm cannot form itself through the product.

## Next action

Verify the contract against current API/RPC/MCP and invocation-binding names, then dispatch one
cross-component implementation after the overlapping board-tool slice releases those write zones.

## Guardrails

- The model names the desired frozen entry only; endpoint binding supplies identity and application
  code retains every mechanical decision.
- Reuse the accepted gate and managed start path. Any parallel gate, semantic chooser, or generic
  command forwarding surface is a contract defect.
- No behavioral evaluation runs in the repository or real home directory; any surface walk uses a
  fresh disposable root with isolated project, `HOME`, `YMP_HOME`, `TMPDIR`, build and export paths.

## Findings

None open; the pre-dispatch contract check must confirm the exact invocation-bound handler before
implementation.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

## Closure

Filled when the subtask is accepted.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.

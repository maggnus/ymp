---
id: W1-EVL-04s
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: OPERATIONALIZATION
relation: required
depends_on: [W1-EVL-04n, W1-EVL-04r]
blocks: [W1-EVL-04m]
created_at: 2026-09-01T21:14:37+08:00
updated_at: 2026-09-01T21:34:00+08:00
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

# W1-EVL-04s — Second authorized live probe resolves infrastructure gate

## Outcome

A second, separately budgeted and explicitly authorized live probe runs exactly once after the
diagnostic gate is proven, and yields either a complete controller attestation or a phase-localized
infrastructure STOP. In both cases the infrastructure gate becomes decidable without selective retry
or contamination of any experimental arm.

## Scope

### In

- Freeze a new probe id, nonce namespace, disposable root and stage-two resource vector distinct from
  the spent W1-EVL-04n attempt and every task/arm budget.
- Exact fresh vector: `model_calls=1`, input tokens `<=32768`, cached input `<=32768`, output
  `<=1024`, reasoning output `<=1024`, wall time `<=120000 ms`, workspace writes `=1`, workspace
  reads `=1`, invocation starts `=1`, and zero protected/external/participant/attempt/offer/
  obligation/board/task/recruitment/candidate/communication actions. Cost may be reported or
  explicitly unavailable; it may not be omitted silently.
- Use the separate namespace `ymp-live-probe-diagnostic-v2`; no identifier, reservation or result
  from the spent W1-EVL-04n attempt may be imported.
- Run all W1-EVL-04r deterministic phase/diagnostic negatives and exact runtime/transport/admission
  readiness before any provider request. Failure closes the task without a live call.
- Execute the existing ignored `live_tool_host_probe` consumer exactly once with the current
  behaviorally compatible installed Codex; no production source change is allowed.
- Capture manifest/runtime/route/compatibility/executable/transport digests, ordered events,
  controller read-back, handle/attestation or failure record, complete usage/cost/wall time,
  provider state, process cleanup and root cleanup.
- The scientific curator records the attempt as the next free `run-*` document from current main;
  the run record is evidence, not permission to alter protocol or repeat.

### Out

- No-touch: production/test code, manifests, budgets of experiments, corpus tasks/oracles, user
  configuration, real project/current directory/`~/.ymp`, TUI and any second provider request. A code
  defect discovered here becomes a new task and this run stops.

## Acceptance

- [ ] Preflight proves exact diagnostic schema, current compatible runtime, transport identity,
      v2 manifest, clean repository and fresh isolated project/HOME/YMP_HOME/TMPDIR/build/export;
      `model_calls=0` before the explicit live command.
- [ ] Exactly one live command is recorded with start/end, process tree, observed version/digests and
      frozen budget. No retry occurs after success, failure, timeout, cancellation or missing usage.
- [ ] Success requires ordered write/read events, exact controller bytes, complete usage/cost state,
      honest terminal, immutable handle/attestation and no extra effect; only then may W1-EVL-04m
      consider the evidence.
- [ ] Failure requires one immutable W1-EVL-04r record with determinate phase/provider/MCP/controller
      state, no attestation and `model_ready=false`; an unlocalized failure invalidates the run and
      still forbids retry.
- [ ] Temporary root/processes are removed, repository and real state remain byte-identical, and the
      run record states explicitly that reachability is not communication, listening, task value or
      self-organization.

## Current state

The first 04n attempt is spent and unlocalized; it cannot be retried. Deterministic launch code is
integrated, but W1-EVL-04r must first make every future failure phase and expenditure durable. No
second budget or permission exists until that diagnostic node is accepted.

## Next action

After W1-EVL-04r acceptance, perform one final read-only gate review and owner budget authorization;
then run the existing consumer once.

## Guardrails

- One budget, one process start, one terminal outcome, zero selective retries.
- A successful attestation opens only infrastructure admission, never a scientific claim.
- Any need to change code stops execution and returns to development.

## Findings

- Proposed after the first authorized attempt produced `ProcessExit` without enough evidence to
  localize whether a provider request occurred.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

## Closure

Filled when the task is accepted. Until then this section stays as written.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger. Empty when there are none.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.

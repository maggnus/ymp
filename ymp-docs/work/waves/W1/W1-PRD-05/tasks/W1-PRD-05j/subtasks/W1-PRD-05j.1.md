---
id: W1-PRD-05j.1
kind: subtask
wave: W1
card: W1-PRD-05
parent: W1-PRD-05j
state: accepted
risk: significant
maturity: BUILD
relation: follow_up
depends_on: [W1-PRD-05j]
blocks: [W1-EVL-04a]
created_at: 2026-09-01T12:38:00+08:00
updated_at: 2026-09-01T14:43:52+08:00
started_at: 2026-09-01T14:00:00+08:00
accepted_at: 2026-09-01T14:43:52+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/7c086eb60f09916df1f32d45342378f51ad2255b
closure_commit: https://github.com/maggnus/ymp/commit/ec9a792888782fe1cd43a70bbae6361733a37f56
evidence: ["[ec9a792](https://github.com/maggnus/ymp/commit/ec9a792888782fe1cd43a70bbae6361733a37f56)"]
duration_minutes: 44
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 3
escalation_decision: bounded_retry
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
- New exact surface names: `RequestParticipantArguments { request_id, entry }`,
  `AgentToolCall::RequestParticipant`, MCP tool `request_participant`, and an invocation-bound
  `AgentSession::request_participant` dispatcher that supplies the proposer before calling the
  accepted `Application::request_participant` operation.
- Agent API parsing, RPC/MCP exposure, invocation-bound application dispatch, and the minimum
  runtime allowlist or capability wiring required to expose the call to an admitted participant.
- Focused proof that the private endpoint supplies the proposer identity and that the accepted
  `Application::request_participant` gate performs the admission and start exactly once.
- Write zone: `ymp-agent-api`, `ymp-agent-rpc`, `ymp-agent-mcp`, `ymp-runtime-codex`,
  `ymp-runtime-claude`, and the narrow AgentSession dispatch in `ymp-application`, plus mechanically
  forced touched manifests/lockfile entries.

### Out

- Semantic selection, ranking, role assignment, replacement policy, board publication or reading,
  TUI changes, and any new recruitment gate.
- A general application proxy, caller-supplied principal, filesystem path, workspace authority, or
  capability token in model-controlled arguments.
- All other crates and application operations, including `ymp-domain`, board semantics, TUI,
  research/evaluation tools, work records, primary corpus, and provider/model selection.

## Acceptance

- [x] An admitted participant invokes the agent call with `{request_id, entry}`; its bound identity
      is used as proposer, the accepted mechanical gate admits one participant, and the existing
      managed start path is observed.
- [x] Repeating the call or its delivery admits and starts no second participant and records no
      second admission.
- [x] An entry outside the frozen pool, an unbound or no-longer-running caller, exhausted budget,
      concurrency refusal, runtime refusal, and unaffordable communication charge fail through the
      existing application reasons without admission or start side effects.
- [x] The public tool schema exposes no proposer, principal, path, workspace, profile, route,
      capability, score, rank, or role-selection field.
- [x] Unknown fields including `proposer`, `participant`, `profile`, `route`, `workspace`, or
      `capability` fail strict parsing before any application call or durable effect.
- [x] Focused agent API, RPC/MCP, application and touched runtime checks pass; strict Clippy for
      touched packages, formatting, and `git diff --check` report no warnings or errors.

## Current state

Accepted and integrated as
[ec9a792](https://github.com/maggnus/ymp/commit/ec9a792888782fe1cd43a70bbae6361733a37f56).
The private endpoint binds proposer identity, the accepted Application gate admits and starts once,
and Codex or Claude advertises the recruitment tool only when the bound endpoint grants it.

## Next action

Use the accepted tool in the bounded two-participant fake-runtime path of W1-EVL-04e.

## Guardrails

- The model names the desired frozen entry only; endpoint binding supplies identity and application
  code retains every mechanical decision.
- Reuse the accepted gate and managed start path. Any parallel gate, semantic chooser, or generic
  command forwarding surface is a contract defect.
- No behavioral evaluation runs in the repository or real home directory; any surface walk uses a
  fresh disposable root with isolated project, `HOME`, `YMP_HOME`, `TMPDIR`, build and export paths.
- Runtime allowlists expose the new tool only when the invocation's accepted contract grants
  recruitment; availability is not inferred from spare budget or model capability.

## Findings

- Pre-dispatch check returned the formerly implicit target names. The contract now names the new
  argument type, enum variant, MCP tool, AgentSession dispatcher, write zone and no-touch boundary.
- R1 found that runtime configurations still advertised the tool without a recruitment grant; the
  author corrected both Codex and Claude to derive their tool sets from the bound endpoint.
- R2 reached compilation but filtered out every test; the bounded proof-only R3 executed one Codex
  and one Claude boundary test and observed granted/ungranted tool sets.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(6/10) RETURN 01/09 14:29 — Codex advertised `request_participant` without the accepted
  capability → the author agreed and found the same defect in Claude → both runtimes now derive
  their tool set from the bound endpoint
- R2(7/10) ESCALATE 01/09 14:41 — static closure was sound but the exact test filter executed zero
  tests → candidate unchanged → proof requires a nonzero runtime-boundary observation
- CTO bounded_retry 01/09 14:41 — one proof-only R3 authorized; acceptance requires an executed
  granted/ungranted runtime test, not compilation or an empty filter
- R3(9/10) ACCEPT 01/09 14:43 — one Codex and one Claude boundary test executed and passed → tool
  presence followed the endpoint capability → R1 closed without another code change

## Closure

### Accepted outcome

Managed participants receive strict `request_participant { request_id, entry }` through the private
agent API, RPC and MCP path. The endpoint supplies proposer identity; the existing Application gate
and managed start path retain every admission decision and exactly-once effect. Runtime-generated
Codex and Claude tool sets expose recruitment only when the invocation-bound endpoint grants it and
fall back to the base tool set when capability discovery fails.

### Residuals

None.

### Evidence

- [ec9a792](https://github.com/maggnus/ymp/commit/ec9a792888782fe1cd43a70bbae6361733a37f56)
  — integrated tree, byte-identical for the reviewed recruitment paths to candidate
  [7c086eb](https://github.com/maggnus/ymp/commit/7c086eb60f09916df1f32d45342378f51ad2255b).

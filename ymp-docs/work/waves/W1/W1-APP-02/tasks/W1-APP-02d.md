---
id: W1-APP-02d
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02a, W1-APP-02b, W1-EXP-01d.2]
blocks: [W1-APP-02e]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-13T12:11:03+08:00
started_at: 2026-08-12T22:44:30+08:00
accepted_at: 2026-08-13T12:11:03+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/05e9bf1d6eb0b367cab73c4cba1f79703641696f
closure_commit: https://github.com/maggnus/ymp/commit/00e25cb95d65bba6056380d7b3fa51536999e5d5
evidence: [`05e9bf1`](https://github.com/maggnus/ymp/commit/05e9bf1d6eb0b367cab73c4cba1f79703641696f)
duration_minutes: 795
blocker:
pause_reason:
return_trigger: W1-APP-02k proves that no descendant survives the supervisor, and W1-APP-02l records per-model spend and a deterministic live check
deliberate_partial: true
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

- [x] From a generated synthetic home, ymp detects the exact Claude Code version and authentication
  readiness, starts it itself, completes one fake-project candidate, and records its session and
  usage evidence.
- [ ] Resume, explicit yield and wake, interruption, budget stop, and full descendant termination
  preserve one attempt and command identity.
- [x] Duplicate or malformed structured events and MCP replies become typed runtime or binding
  failures without committing a second state change.
- [x] A profile with ambient configuration, enabled native subagents, incompatible structured
  events, missing usage evidence, or unenforceable overshoot beyond `G3` is rejected before a run.

## Current state

Accepted with residue and integrated. A real pinned Claude Code build completes a candidate through
the product's own coordination bridge, with session and usage recorded and no credential in any
durable record. Independent review measured three defects that do not undo that outcome: the
per-model breakdown of spend is discarded, the live check cannot distinguish a product failure from
a model declining to act, and a descendant that creates its own session survives the supervisor.

## Next action

None for this task. W1-APP-02k closes the process escape before the comparison runs; W1-APP-02l
restores spend attribution and a deterministic live check.

## Guardrails

- Runtime configuration is generated per invocation and never mutates user configuration.
- A Messages-compatible endpoint does not become an approved route without exact conformance.
- The operator starts only `ymp`; no manual second-terminal process is part of acceptance.

## Findings

- W1-EXP-01d.2 closed after two independent returns with a bounded non-admission result. Candidate
  `4f27fe0` is not integrated; only independently reproduced runtime defects are authoritative.
- The independent review reproduced the product path outside the author's harness: the child's
  environment equalled the allow list, credentials were delegated by digest, and the executed files
  were the admitted copies. Terminal accounting and process-group termination each failed on a
  deliberately broken build, so the checks can fail.
- `major`, defect in the contracted outcome: only the total cost and aggregate usage are read, and
  the provider's per-model breakdown is discarded, so a record cannot show which model spent the
  sum. Continued as W1-APP-02l.
- `major`, defect in the contracted outcome: the live acceptance check is non-deterministic — a
  product failure is indistinguishable from a model that declines to submit. Continued as
  W1-APP-02l.
- `major`, independent product defect: a descendant that calls for its own session survived the
  supervisor's exit by 37 seconds and was reparented to init. Process-group termination does not
  reach it, and the supervisor is shared with the Codex profile. Continued as W1-APP-02k, which
  blocks the comparison because a surviving agent process can spend budget after its run closed.
- `minor`, independent product defect: the operator's full search path reaches the child and its
  profile record. Continued as W1-APP-02l.
- The reviewer and the author exchanged facts directly under the new rule: the exact command, its
  exit and the model and cost fields. The exchange is recorded in the review report.

## Closure

### Accepted outcome

The application detects an approved Claude Code profile, starts and supervises it through a compiled
driver, exposes only invocation-scoped coordination tools, and records lifecycle, route and usage
evidence from a real provider run. Both POC profiles now exist under one lifecycle model.

### Residuals

Full descendant termination is not proved: a child that creates its own session outlives the
supervisor. This is not carried as a tolerated limitation but as the blocking node W1-APP-02k,
because a surviving agent process spends money and acts after its authority ended, which the
residue tests forbid. Spend attribution and the determinism of the live check are carried as
W1-APP-02l. The card's front matter records the return trigger.

### Evidence

- [`05e9bf1`](https://github.com/maggnus/ymp/commit/05e9bf1d6eb0b367cab73c4cba1f79703641696f) —
  reviewed candidate; the integrated tree is byte-identical to it.
- Independent review measured a real provider run at 67060 microUSD with the session and usage
  recorded, and reproduced the product path outside the author's harness.

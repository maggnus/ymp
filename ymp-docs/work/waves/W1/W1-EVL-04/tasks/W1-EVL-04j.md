---
id: W1-EVL-04j
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04h, W1-EVL-04i, W1-EVL-04k]
blocks: [W1-EVL-04e]
created_at: 2026-09-01T15:41:48+08:00
updated_at: 2026-09-01T15:50:30+08:00
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

# W1-EVL-04j — Controller attests the no-task-output tool-host probe

## Outcome

The trusted foreground controller can run one separately budgeted no-task-output workspace-tool
probe, verify an unpredictable nonce write/read and complete route/usage/event evidence, and persist
an attested record that the admission gate accepts. Model-authored or incomplete evidence can never
set `model_ready=true`.

## Scope

### In

- New strict `ToolHostProbeRequest`, `ToolHostProbeEvidence` and `AttestedToolHostProbe` types with
  controller-owned nonce, path, deadline, invocation identity and digest binding.
- One minimal managed invocation with workspace read/write only, no network, board, task contract,
  recruitment, candidate, verification query or experimental arm; exact usage and terminal evidence
  is charged to a separate admission budget.
- Controller read-back of the nonce from the disposable workspace, comparison with the generated
  value, binding to runtime/profile/CLI/driver/tool schema and durable storage under `~/.ymp`.
- Proposed write zone for Critical contract review: narrow additions in `ymp-runtime-api`,
  `ymp-runtime-supervisor`, `ymp-application`, internal `ymp-cli` wiring, the admission evidence
  consumer in `ymp-corpus/src/admission.rs`, required manifests and `Cargo.lock`.

### Out

- Real task prompts or output, collaboration messages, primary/development arms, semantic grading,
  model/provider selection, TUI ceremony, arbitrary file paths from the model, real user HOME,
  deployment or live paid execution during implementation.

## Acceptance

- [ ] A fake managed runtime receives only a controller-generated nonce and scoped disposable path,
      writes and reads it once, terminates honestly and yields a controller-attested record whose
      invocation, route, profile, CLI/driver/tool schema, usage, wall time, currency/cost availability
      and output/event digests are complete.
- [ ] Missing write/read, wrong nonce, model-chosen path or digest, stale/replayed invocation,
      incomplete usage, ambiguous terminal, timeout, cancellation, extra tool use or any board/task
      effect fails closed without an attestation or model-ready transition.
- [ ] The probe consumes one separately reserved start and complete resource vector; no spare arm
      budget, task result, candidate or communication record can pay for or satisfy it.
- [ ] Only controller read-back plus controller-held nonce material creates the attestation. Raw
      model output, self-authored tests or a syntactically valid evidence file are insufficient.
- [ ] Application persists the record only below the configured ymp data root; a disposable-root
      product-path test proves no file appears in the repository, current directory or real `~/.ymp`.
- [ ] Fake-runtime positive and fault matrix, one consumer-boundary negative, strict affected-package
      Clippy, formatting and `git diff --check` pass; implementation performs no real model/network
      or money call.

## Current state

W1-EVL-04h defines and validates the evidence schema but correctly refuses every record because no
production component can attest nonce ownership, read-back and usage/route binding. The manual pilot
showed that model readiness without this tool-host proof is insufficient. No live probe is authorized.

## Next action

Run a Critical contract review to confirm the smallest production seam and split it if runtime
execution, controller attestation and corpus consumption cannot land under one acceptance story.

## Guardrails

- The probe is admission evidence only and never enters any experimental arm or task outcome.
- The model never chooses the nonce, authority, destination path, evidence digest or success rule.
- A real probe remains a separate owner model/money gate after fake-runtime implementation.

## Findings

- Created from the accepted W1-EVL-04h additional-work finding. Cross-crate breadth requires a
  pre-dispatch decomposition decision before any writer receives authority.

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

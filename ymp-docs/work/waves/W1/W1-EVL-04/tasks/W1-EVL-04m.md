---
id: W1-EVL-04m
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04l, W1-EVL-04q, W1-EVL-04s]
blocks: [W1-EVL-04e]
created_at: 2026-09-01T15:57:45+08:00
updated_at: 2026-09-01T21:14:37+08:00
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

# W1-EVL-04m — Admission consumes only controller-attested probe evidence

## Outcome

The admission command sets `model_ready=true` only from one strict `AttestedToolHostProbe` produced
by the W1-EVL-04l controller from the compatible non-Fake Codex invocation admitted by W1-EVL-04n
and bound to the W1-EVL-04q v2 manifest; raw, Fake, model-authored, stale, replayed or incomplete
evidence remains fail-closed.

## Scope

### In

- Replace the current specified-only stage-two probe acceptance in
  `ymp-rust/tools/ymp-corpus/src/admission.rs` with strict consumption of the controller-attested
  Application record and its exact manifest, runtime, route, usage, budget and digest bindings.
- Read only the strict serialized `AttestedToolHostProbeHandle` from the controller-owned export
  outside the candidate project; derive the matching probe Application store from the prepared
  evaluation `YMP_HOME`, open it through the existing Application/root API and call
  `Application::attested_tool_host_probe(&handle)`. The handle is a locator, not evidence.
- Preserve the accepted zero-model rehearsal, S1-S3 schedules, read cap, limits and report schema
  except for the minimal attestation fields required to open the gate.
- Exclusive write zone: `ymp-rust/tools/ymp-corpus/src/admission.rs` only.

### Out

- No-touch: all runtime, Application and CLI crates; admission manifest/digest owned by
  W1-EVL-04k; accepted v1/v2 tasks and protected oracles; research/work records; primary seeds,
  budgets and outcomes; real model/network/money calls.

## Acceptance

- [ ] A controller-attested non-Fake record with matching manifest digest, runtime tuple,
      invocation/probe identity, observed version/executable digest, behavioral compatibility
      contract, nonce read-back digest, collaboration schema digest, probe schema digest, exact
      W1-EVL-04o `probe_transport_digest`, complete usage, separate budget, event/output digests and
      honest terminal makes stage two pass and `model_ready=true` exactly once.
- [ ] A raw or Fake `ToolHostProbeTrace`, the former schema-valid `ProbeEvidence`, model-authored JSON,
      missing controller binding, stale/replayed identity, mismatched manifest/runtime/route/schema,
      incomplete usage, extra effect or budget overflow each leaves `model_ready=false` with its own
      typed reason.
- [ ] Re-running with the same attestation cannot consume it twice or create a second readiness
      transition; an attestation for another isolated root cannot be imported.
- [ ] Mutated or fabricated handle JSON, a valid handle copied from another `YMP_HOME`, a missing
      private reference/object and direct deserialization of attestation bytes each fail before
      stage two; the single-file consumer needs no new Application, CLI or Cargo change.
- [ ] One `ymp-corpus admission` consumer walk runs in a fresh root with separate project, `HOME`,
      `YMP_HOME`, `TMPDIR`, build and export; the positive record is supplied only through the
      trusted controller export outside the candidate project.
- [ ] Focused admission tests and one mutation removing the controller/read-back check discriminate
      the authority boundary; strict Clippy, formatting and `git diff --check` pass with
      `model_calls=0` and no real provider/network/money call.

## Current state

W1-EVL-04j owns the untrusted trace, W1-EVL-04l owns controller authority, and W1-EVL-04q freezes the
behavioral consumer. The first W1-EVL-04n live attempt failed without attestation; only the separately
budgeted W1-EVL-04s outcome may provide consumable evidence. Raw, Fake or failure records cannot open
this consumer.

## Next action

After W1-EVL-04s produces a complete attestation, run a Critical contract check of the single-file
consumer boundary. A failure result keeps this node blocked.

## Guardrails

- Admission consumes authority; it never creates, repairs or infers it.
- A syntactically correct evidence file is not proof of controller nonce ownership or read-back.
- A handle is an untrusted locator; only verified reload from the matching Application store returns
  the opaque attestation consumed here.
- A schema-identical substitute probe server is not equivalent transport; its executable or launch
  digest mismatch keeps the gate closed.
- This node opens only the development model gate and changes no experimental arm or metric.

## Findings

- Created by R1 decomposition of W1-EVL-04j to isolate the policy decision from execution,
  controller authority and persistence.

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

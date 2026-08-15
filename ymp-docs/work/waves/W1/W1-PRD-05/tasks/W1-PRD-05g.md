---
id: W1-PRD-05g
kind: task
wave: W1
card: W1-PRD-05
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-PRD-05d]
blocks: []
created_at: 2026-08-15T23:35:00+08:00
updated_at: 2026-08-16T02:05:00+08:00
started_at: 2026-08-15T23:35:00+08:00
accepted_at: 2026-08-16T02:05:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/fb961cf28017c3f36689efe2619188facb9815f4
closure_commit: https://github.com/maggnus/ymp/commit/028e7b7e9a8c4e739c0a20006e4cdf78af4958c2
evidence: ["[fb961cf](https://github.com/maggnus/ymp/commit/fb961cf28017c3f36689efe2619188facb9815f4)"]
duration_minutes: 150
blocker:
pause_reason:
return_trigger:
deliberate_partial: true
---

# W1-PRD-05g — Provider measurement runs off the interface thread

## Outcome

Enabling or refreshing a provider no longer freezes the interface: the measurement (an engine
probe of 15–40 s on the owner's host, measured in P2's review) runs on a worker thread; the
provider row and card show a measuring state and keep repainting; keys are handled meanwhile; the
observation lands when the probe returns and the pages re-read the records. No new operator act,
no confirmation.

## Scope

### In

- `ymp-tui` Session: `measure_enabled_providers` / `set_provider_enabled` / `refresh_provider_models`
  spawn the probe on a worker and deliver its result through the existing event channel; a
  `measuring…` state on `/providers`, the provider card and `/runtimes` while it runs; the input
  thread's buffered keys are processed, not replayed after the wait; a second Enable/Refresh while
  one is running is coalesced, not queued (stated on the row).
- CLI commands stay synchronous (they are one-shot).
- Bounded shutdown: quitting during a measurement does not leave an engine process behind
  (W1-APP-02k rule) — the worker is joined or the child killed within the existing 5 s bound.

### Out

- Any change to what is measured or when (P2's rules stand: only on Enable/Refresh).

## Acceptance

- [x] A deterministic test with a slow fake engine (sleep ≥ 2 s) shows the interface repainting
      and accepting a key during the measurement, and the observation appearing afterwards;
      negative half: on the current tree the same test observes no repaint until the probe ends.
- [x] Quitting during a measurement leaves no engine process (probe by pid file of the fake).
- [x] Screen tests for the measuring state at both sizes.

## Current state

Accepted after one return round. Reviewer blocker (landing overwrote an operator revocation made
during the probe) was accepted and fixed at fb961cf: `finish_measurement` re-applies
`withdraw_routing_of` for families withdrawn while the probe ran (`withdrawn_during`). Lean
re-review ACCEPT: original scenario now lands at «0 ready», reviewer's own mutation of the fix
fails exit 101, no new races (all landing steps sequential on the drawing thread). Merged into
main as the integration merge; `cargo test -p ymp-tui` on the merged tree: 162 tests ok, 0 failed.

## Next action

Residual below stays with W1-APP-02aa.

## Guardrails

- Measurement starts no process and reaches no network outside Enable/Refresh.
- The landing must never contradict an operator decision taken while the probe ran.

## Findings

None blocking. Review findings recorded as residuals/known properties.

## Closure

### Accepted outcome

Measurement runs on a worker thread through the event channel; measuring… state on both sizes;
keys processed during the probe; coalesced repeated Enable/Refresh; quit during measurement leaves
no engine within 5 s for probes under the bound; landing re-applies operator revocations made
during the probe. Verified on the built binary in a pseudo-terminal with a fake engine, plus a
reviewer-owned mutation proving the check distinguishes the defect.

### Residuals

1. (major, additional-work) Probes longer than the 5 s shutdown bound are not killed on quit; the
   engine process survives the session (pre-existing W1-APP-02k limit; the card's acceptance test
   pins 2 s). Carried to card W1-APP-02aa. Return trigger: any release before that card closes.
2. (minor, hypothesis-refinement) Withdraw-then-re-enable inside a single probe window leaves the
   pre-withdrawal routing installed; readiness matches the operator's standing decision, so no
   revocation is owed. Known property, no action.

### Evidence

- [fb961cf](https://github.com/maggnus/ymp/commit/fb961cf28017c3f36689efe2619188facb9815f4) — candidate head, full reviewed range 75f212a..fb961cf
- [028e7b7](https://github.com/maggnus/ymp/commit/028e7b7e9a8c4e739c0a20006e4cdf78af4958c2) — integration merge into main — `cargo test -p ymp-tui` 162 ok / 0 failed on the merged tree (run by CTO)

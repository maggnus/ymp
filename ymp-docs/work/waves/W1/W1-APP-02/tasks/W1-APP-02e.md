---
id: W1-APP-02e
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W0-UX-01c, W1-APP-02b, W1-APP-02c, W1-APP-02d]
blocks: [W1-COR-03a]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-10T21:23:34+08:00
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
---

# W1-APP-02e — TUI completes and exports a single-participant run

## Outcome

The ratatui interface exposes runtime readiness, run start, participant and obligation state,
candidate and verifier events, budgets, cancellation, terminal reason, and evidence export for one
complete participant attempt.

## Scope

### In

- Foreground startup, profile readiness, contract selection, run initiation, event-driven views,
  cancellation, terminal states, bounded logs, and evidence export.
- Views consume application projections and issue typed commands through the in-process adapter.
- The accepted `W0-UX-01c` screen and state contract for the POC-1 journey, including constrained,
  normal, failure, and high-volume terminal variants.
- Automated TUI-state tests against the fake runtime plus one controlled run for each real profile.

### Out

- Multi-participant negotiation views, communication observatory analysis, interface polish,
  background attachment, web access, and remote control.

## Acceptance

- [ ] A user starting only `ymp` can select an approved contract and ready Codex or Claude Code
  profile, observe the attempt, and export exact candidate and verifier evidence.
- [ ] The same TUI path exposes an unavailable or unauthenticated profile before start and refuses
  to launch it rather than falling back to another route.
- [ ] Cancellation stops the managed runtime tree, records `cancelled`, and leaves no live attempt
  endpoint that can commit a later command.
- [ ] Corrupt evidence, runtime failure, verifier failure, or foreground-controller loss is shown
  as its typed terminal condition and never as candidate rejection or acceptance.
- [ ] Deterministic Ratatui state tests cover the accepted 80 × 24 and 120 × 40 contracts for first
  launch, mixed runtime readiness, running, cancellation, acceptance, candidate failure, budget
  exhaustion, and infrastructure failure without relying on color or mouse input.

## Current state

The Claude Design requirements and `W0` review path now exist, but no accepted screen package,
crate, view model, fake-runtime test, or real run exists. This task follows the candidate path,
both runtime drivers, and the frozen `W0-UX-01c` contract.

## Next action

After `W0-UX-01c` is accepted, implement its POC-1 screen state model against fake application
events before connecting widgets to either real runtime.

## Guardrails

- Widgets do not own domain transition rules.
- The first TUI remains usable without a daemon, database service, or network listener.
- Human messages or interventions are explicitly attributed and exclude a run from autonomous
  experimental comparisons.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

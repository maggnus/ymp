---
id: W1-PRD-05d
kind: task
wave: W1
card: W1-PRD-05
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-PRD-05b, W1-PRD-05c]
blocks: []
created_at: 2026-08-15T18:55:00+08:00
updated_at: 2026-08-15T23:30:00+08:00
started_at: 2026-08-15T18:55:00+08:00
accepted_at: 2026-08-15T23:30:00+08:00
candidate_commit: 72b288b
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger: a live-session disable that leaves the just-disabled account routable (in-session staleness) reaching a run start; the frozen interface during measurement reaching an operator-facing release
deliberate_partial: false
---

# W1-PRD-05d — Provider and model surfaces; probing gated on Enable (P2)

## Outcome

Migration unit P2 of [COLLECTIVE-MIGRATION.md](../../../../design/COLLECTIVE-MIGRATION.md): the
operator sees the fixed list of supported providers as a table (`/providers`), opens a provider's
properties, and enables it there; probing/autodetect happens only on the enable transition or an
explicit refresh, never before; the model catalog is a table (`/models`) derived from the accepted
P1 records; the interface stops constructing drivers. Surfaces per
[COLLECTIVE-TUI.md](../../../../design/COLLECTIVE-TUI.md) (S03–S05 family) and mirrored commands.

Carries P1's two review residues: the stored provider state states the age of its observation (or
the surface re-observes before reading), and a deleted engine record reads as a route with no
models rather than shortening the catalog silently.

## Scope

### In

- `/providers` table + provider properties view with Enable / Disable / Refresh models as keys on
  the selected row (consequence stated above the key: enabling permits repository content of any
  workspace to reach that provider — decision D4); `/models` table; mirrored CLI commands (every
  interface action exists as a command — W1-APP-02n).
- Observation timestamp on provider records (SCHEMA additive); the deleted-engine-record case.
- Startup screen unchanged (W1-APP-02e.5); the "no provider enabled" state on goal entry names
  `/providers` in plain words (brief v2 Part A §6) — no technical error text.

### Out

- AgentPool (P3), pool freeze (P5), any run start. Route overrides (P2a). No autodetect before
  Enable; no wizard; no confirmations; no typed identifiers.

## Acceptance

- [x] On a fresh root `/providers` lists the supported providers all disabled with no probe having
      run (negative half: any probe before Enable is a failing test); Enable on a row probes,
      records the observation with its time, and `/models` shows the catalog; Refresh re-observes.
- [x] A deleted engine record shows as a route with no models, not as a shorter catalog; a stale
      observation is stated with its age.
- [x] Deterministic screen tests (80×24, 120×40) for the three surfaces; each key is a command.

## Current state

Accepted 2026-08-15 (candidate 72b288b after one RETURN pass; review ACCEPT WITH RESIDUE; merged at b25420d).

## Findings

- Product measures nothing before Enable: TUI start, `show providers|models|provider|runtimes` → 0
  engine invocations on a fresh root (fake-binary log); `provider enable` → exactly one probe.
- P1 residues closed: observation time (`observed_at_ms`, additive; Never/Undated/Ahead/Age),
  deleted engine record shows as a route with no models.
- Round-1 RETURN closed: `show runtimes` no longer probes unconditionally; a disabled-but-measured
  provider states its measurements and age beside the reason. Columns per S03 (STATE, OBSERVED).
- Pre-existing `engine_registry` assertion fixed deterministically (`0 ready · 2 unusable`).
- Residues: in-session staleness — the disable arm of `set_provider_enabled` re-reads but does not
  re-measure, so a live session may still route to a just-disabled account (one line; P4 takes it);
  ~29–42 s frozen interface with no repaint/cancel during measurement (own node: worker thread);
  the ↳ reason line is clipped at 80 columns; provider→engine binding at run start is P5.

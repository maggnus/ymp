---
id: W1-APP-02w.1
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02w
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02w]
blocks: []
created_at: 2026-08-15T01:52:53+08:00
updated_at: 2026-08-15T08:55:35+08:00
started_at: 2026-08-15T08:15:17+08:00
accepted_at: 2026-08-15T08:55:35+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/a10ea9e10d90547a6586dac56696d55faf5c7f3d
closure_commit: https://github.com/maggnus/ymp/commit/d457519cf888b8dd120b2ed3a241e5a827ebfac2
evidence: ["[d457519](https://github.com/maggnus/ymp/commit/d457519cf888b8dd120b2ed3a241e5a827ebfac2)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02w.1 — Product state lives under ~/.ymp; the launch directory stays untouched

## Outcome

The default root moves from ./.ymp to ~/.ymp (override --root kept; YMP_HOME honoured), the
accepted layout is unchanged (root.json, projects/<name>-<path-hash>/runs/<seq>), draft assembly
directories live under the root too, and nothing but an explicitly exported accepted result ever
appears in the launch directory. An old ./.ymp or ./.ymp-data is read only via an explicit
--root/--data-root; the default beside one refuses with named exits, as accepted in W1-APP-02w.

## Scope

### In

- The default root in ymp-cli (lib.rs data-root default) and the draft assembly location.
- The exported result applied into the project directory on explicit export (owner decision
  2026-08-15) — a delivery, not storage.

### Out

- The layout itself and the store invariants, accepted in W1-APP-02w.

## Acceptance

- [ ] A run started from a clean directory leaves it untouched; ls ~/.ymp shows root.json and
      projects/…/runs/0001; the negative half is the current ./.ymp appearing beside the project.
- [ ] Export applies the accepted candidate into the project directory; nothing else is written
      there.

## Current state

Ready. From the owner decision of 2026-08-15: nothing is stored in the launch directory;
neighbouring systems (~/.claude 1.0G, ~/.codex 2.3G) keep all state at home.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- sandboxed-HOME product runs left the launch directory byte-identical while the home root carried
  the store; both earlier-state forms refuse with named exits; the application route is forced by
  the surface scanner; reviewer mutation reproduced

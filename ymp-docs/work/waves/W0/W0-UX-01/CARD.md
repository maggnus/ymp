---
id: W0-UX-01
kind: card
wave: W0
state: ready
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: [W1-APP-02e, W1-COR-03e]
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-08-12T02:47:53+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
---

# W0-UX-01 — Reviewed terminal screen contract precedes implementation

## Outcome

The POC has an approved terminal-native screen system and state matrix that preserves ymp's domain
and trust semantics, remains feasible in Rust/ratatui, and binds implementation to one reviewed
revision of the existing HTML artifact.

## Invariants

- Visual design cannot introduce central semantic allocation, agent ranking, leadership labels,
  candidate selection, or causal claims from a live trace.
- Control, untrusted collaboration, and independent verification retain distinct meanings.
- Every material normal, empty, degraded, stale, and failure state has a design path.
- The result remains keyboard-first, terminal-native, and implementable in one installed Rust
  executable without a web or service runtime.
- Screen implementation begins only from an independently reviewed design contract.

## Scope

This card owns `ymp-docs/design/ymp_k9s_tui.dc.html`, its traceability to the project documents,
and an independent semantic, accessibility, and implementation-feasibility review. No other design
artifact or handoff document is expected.

It excludes implementation, changes to the domain protocol, acceptance of the POC product claim,
and visual requirements for optional server, web, or cluster modes.

## Aggregate acceptance

All three required tasks are accepted. The reviewed design package covers every POC-1 and POC-2
use case and material failure variant, is traceable to the project requirements, is feasible at the
declared terminal sizes with Ratatui and Crossterm, and contains no visual semantics that turn ymp
into a dispatcher or overstate acceptance, security, causation, or collective reasoning.

## Tasks

- [W0-UX-01a](tasks/W0-UX-01a.md) — required
- [W0-UX-01b](tasks/W0-UX-01b.md) — required
- [W0-UX-01c](tasks/W0-UX-01c.md) — required

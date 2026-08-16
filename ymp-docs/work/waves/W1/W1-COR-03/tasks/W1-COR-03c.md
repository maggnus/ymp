---
id: W1-COR-03c
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-COR-03a, W1-COR-03b]
blocks: [W1-COR-03e]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-16T02:45:00+08:00
started_at: 2026-08-16T01:03:00+08:00
accepted_at: 2026-08-16T02:45:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/00a70e6fe483c31f9e3aa646e1d7571780d0c3ba
closure_commit: https://github.com/maggnus/ymp/commit/7f0f01411db2f20ff4eba7a66a373d73d7fd065e
evidence: ["[00a70e6](https://github.com/maggnus/ymp/commit/00a70e6fe483c31f9e3aa646e1d7571780d0c3ba)"]
duration_minutes: 100
blocker:
pause_reason:
return_trigger:
deliberate_partial: true
---

# W1-COR-03c — Scoped board preserves attribution without carrying authority

## Outcome

Participants exchange bounded, attributed messages in explicit task scopes while authoritative
control and protected verification remain separate; application projections can reconstruct
declared provenance without claiming that message order or fluent dialogue proves causal reasoning.

## Scope

### In

- Task and candidate-review audiences, bounded project-discovery summaries, expiring membership,
  message kinds, immutable payload digests, references, salience expiry, communication charges,
  delivery cursors, challenges, revisions, and typed read-only observatory projections.
- Message-to-decision-to-artifact-to-verification provenance and independently committed first
  assessments where the approved oracle requires agent review.

### Out

- Global free-form visibility, semantic ranking, automatic action from message content, capability
  delegation through text, private chain-of-thought collection, and causal inference from one
  transcript.
- Ratatui widgets, screen composition, terminal interaction, and responsive visual states, which
  are owned by `W1-COR-03e` after these projections are accepted.

## Acceptance

- [x] Only explicit, unexpired audience grants expose detailed task messages; project discovery
      exposes bounded summaries and references only.
- [x] A payload containing a capability-shaped value, command, URL, consent statement, protected
      reference, or tool instruction cannot grant authority, form a contract, fetch data, invoke a
      tool, verify a candidate, or keep a run active.
- [x] Publication, refresh, membership, and delivered bytes consume finite resources, while the
      immutable audit record remains available after active salience expires.
- [x] The observatory distinguishes publication, delivery, citation, revision, artifact ancestry,
      and verifier result and labels none of them causal without a controlled intervention record.
- [x] A deliberately malformed or cross-scope message is rejected without changing control or
      verifier state.
- [x] When an assessment participates in blinded selection, it is committed before arm identity,
      producer rationale, other assessments, reputation, or communication evidence is revealed; an
      early-disclosure attempt is rejected and cannot enter the primary comparison.

## Current state

Accepted. Self-contained crate `ymp-board` (zero ymp-* dependencies; control, verifier and store
planes unreachable by construction). Independent critical-depth review: ACCEPT with an
out-of-package falsifier using only the public API — self-authored payloads byte-compared against
neutral payloads of equal length; self-issued grants, forged controller and foreign publication
rejected; blinded-first-assessment order enforced; author suite 39 passed on the merged tree.

## Next action

W1-COR-03e (TUI observatory) can start; persistence child below is its prerequisite for evidence
export only.

## Guardrails

- Storage co-location does not permit shared schemas, writers, capability namespaces, or exports.
- Influence, agreement, citations, verbosity, and centrality are never rewards or admission
  signals.
- Protected oracle data, credentials, capability material, and session capsules never enter board
  storage.

## Findings

None blocking. Review findings recorded as residuals.

## Closure

### Accepted outcome

In-memory collaboration board with expiring audience grants and separate read/publish rights,
bounded project-discovery, ten message kinds with references/replies/challenges/revisions/salience,
delivery cursors and receipts, a communication quota in its own namespace, blinded first
assessments with enforced disclosure order, and a read-only observatory projection. Payload bytes
never enter the board: commands carry identity and length only. Verified by builder negative
halves (rule-removal) and a reviewer-owned external falsifier of a different shape.

### Residuals

1. Closed 2026-08-16 by BOARD-PERSIST (merged 3bab764): the board crate carries its own record
   section (`board.json` opening conditions + `facts.jsonl` digest-chained facts, OS-exclusive
   writer lock, chain verification on append), and evidence exports as one reproducible file.
   One minor limitation carried forward: prefix verification confirms the last written position,
   not every position — full per-append chain comparison returns when the board gains a calling
   side (review finding, measured).
2. (minor, additional-work) `ymp-rust/README.md` package table lacks the ymp-board row. Return
   trigger: next documentation sweep.

### Evidence

- [00a70e6](https://github.com/maggnus/ymp/commit/00a70e6fe483c31f9e3aa646e1d7571780d0c3ba) — candidate, range f15712c..00a70e6
- [7f0f014](https://github.com/maggnus/ymp/commit/7f0f01411db2f20ff4eba7a66a373d73d7fd065e) — integration merge; `cargo test -p ymp-board` 39 ok / 0 failed on the merged tree (CTO)

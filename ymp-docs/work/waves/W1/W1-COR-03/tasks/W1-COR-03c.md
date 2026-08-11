---
id: W1-COR-03c
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-COR-03a, W1-COR-03b]
blocks: [W1-COR-03e]
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

- [ ] Only explicit, unexpired audience grants expose detailed task messages; project discovery
  exposes bounded summaries and references only.
- [ ] A payload containing a capability-shaped value, command, URL, consent statement, protected
  reference, or tool instruction cannot grant authority, form a contract, fetch data, invoke a
  tool, verify a candidate, or keep a run active.
- [ ] Publication, refresh, membership, and delivered bytes consume finite resources, while the
  immutable audit record remains available after active salience expires.
- [ ] The observatory distinguishes publication, delivery, citation, revision, artifact ancestry,
  and verifier result and labels none of them causal without a controlled intervention record.
- [ ] A deliberately malformed or cross-scope message is rejected without changing control or
  verifier state.

## Current state

The three-plane boundary and board schema exist only in documentation. Implementation follows the
accepted local-contract and cursor lifecycle.

## Next action

Implement separate collaboration records, authorization tests, and typed observatory projections
before `W1-COR-03e` renders them in the TUI.

## Guardrails

- Storage co-location does not permit shared schemas, writers, capability namespaces, or exports.
- Influence, agreement, citations, verbosity, and centrality are never rewards or admission
  signals.
- Protected oracle data, credentials, capability material, and session capsules never enter board
  storage.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

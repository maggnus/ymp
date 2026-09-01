---
id: W1-COR-03e
kind: task
wave: W1
card: W1-COR-03
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W0-UX-01c, W1-APP-02e, W1-COR-03c, W1-COR-03d]
blocks: [W1-EVL-04b]
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-09-01T22:12:00+08:00
started_at: 2026-09-01T21:41:03+08:00
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

# W1-COR-03e — TUI exposes local commitments and communication evidence

## Outcome

The foreground ratatui interface makes multi-participant commitments, scoped untrusted
conversation, competing candidate ancestry, and communication-study provenance inspectable without
acting as a dispatcher or labelling temporal association as collective reasoning.

## Scope

### In

- The accepted `W0-UX-01c` POC-2 screen contract implemented over the accepted application
  projections from `W1-COR-03c` and immutable candidate graph from `W1-COR-03d`.
- Participants, attempts, invocations, offers, bids, task contracts, obligations, leases, audience
  grants, messages, active salience, submissions, candidates, conflicts, synthesis, verification,
  budgets, and typed terminal outcomes.
- Read-only communication observatory views that distinguish publication, delivery, citation,
  revision, declared provenance, independent positions, interventions, and causal study results.
- Deterministic terminal-state tests at constrained, normal, and wide high-volume sizes.

### Out

- Collaboration records or authorization rules owned by `W1-COR-03c`, candidate integration rules
  owned by `W1-COR-03d`, the intervention analysis owned by `W1-EVL-04b`, and any semantic
  dispatcher, grade, agent or candidate rank, causal estimator, private chain-of-thought, or MVP
  interface feature.

## Acceptance

- [ ] From the TUI, an operator can trace an offer through bid, mutual task contract, obligation,
  attempt, submission, candidate, and verifier result while seeing that sponsor and contractor are
  temporary participant-local relationships rather than kernel assignments.
- [ ] Detailed messages remain visibly audience-scoped and untrusted; publication, delivery,
  citation, challenge, revision, and salience expiry are distinct, and no message is rendered as a
  capability or automatic action.
- [ ] Competing candidates, stale or conflicting integration, and participant-sponsored synthesis
  remain separate immutable branches without a visually preferred canonical winner.
- [ ] The observatory separates live provenance from offline intervention evidence and never labels
  message order, fluency, agreement, verbosity, or graph centrality as listening, task value,
  leadership, intelligence, or acceptance.
- [ ] Ratatui state tests cover the accepted 80 × 24, 120 × 40, and wide high-volume contracts;
  a negative-control fixture that marks delivery as causal reasoning or centrality as leadership
  fails the semantic view-state check.

## Current state

The rejected scaffold remains deleted and 03e.1/03e.4 now provide exact resolved payload and audience
facts. The owner replaced the old content hierarchy before 03e.2 wrote code: new required 03e.5
freezes the chat/slash/modal surface, then 03e.2 and 03e.3 implement it sequentially.

## Next action

Accept the compact 03e.5 design contract, resume and accept 03e.2 against it, then implement 03e.3;
close the parent only when all five observatory acceptance criteria are demonstrated together.

## Guardrails

- Widgets consume projections and issue typed application commands; they never implement protocol
  transitions or infer semantics from message text.
- Untrusted content is escaped before rendering and cannot create terminal control sequences,
  links, tool invocations, or state-changing actions.
- Interface metrics do not affect admission, allocation, budget, verification, or acceptance.

## Findings

- The discarded scaffold cannot seed the next implementation: its direct ledger reads violated the
  application-projection boundary, and its semantic label scan did not cover the data it claimed to
  protect.
- The owner's continuing POC objective and explicit parallelization instruction satisfied the return
  trigger; fresh work began from current main with disjoint runtime/TUI write zones.
- Fresh 03e.2 preflight found the typed projection, not TUI, omits audience; a new narrow prerequisite
  prevents the interface from inventing scope or recipients.
- Owner direction on 01/09 retains the chat-first visual idea but replaces the old screen contents;
  03e.5 now owns the exact slash-command and modal fixtures before implementation resumes.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

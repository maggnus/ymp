---
id: W1-COR-03e.2
kind: subtask
wave: W1
card: W1-COR-03
parent: W1-COR-03e
state: deferred
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03e.1]
blocks: []
created_at: 2026-09-01T12:16:00+08:00
updated_at: 2026-09-01T21:48:42+08:00
started_at: 2026-09-01T21:41:03+08:00
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason: operator projection omits the persisted message audience; TUI cannot reconstruct it honestly
return_trigger: W1-COR-03e.4 accepted
deliberate_partial: false
review_rounds: 0
escalation_decision:
---

# W1-COR-03e.2 — TUI renders resolved participant messages through Application

## Outcome

The foreground TUI reads resolved operator messages only through the typed Application API and
renders their exact bounded text, author, message kind, audience/untrusted status, and publication
order in the shared transcript without gaining board, object-store, or control authority.

## Scope

### In

- Session/application projection wiring for resolved board messages.
- Mapping to existing `Entry::BoardMessage` and honest empty/error states.
- One deterministic visual scenario at the normal terminal size and a PNG when the changed screen
  is ready for owner review.

### Out

- Agent-facing publication tools, interventions, causal analysis, new board semantics, and the full
  commitments/candidate observatory required by the parent.

## Acceptance

- [ ] A persisted message reaches the transcript with exact resolved bytes, author, kind, scope,
      and an explicit untrusted marker through Application APIs only.
- [ ] Missing or corrupt payload bytes produce an honest visible failure and never a placeholder,
      executable control, link, or inferred semantic label.
- [ ] Publication, delivery, citation, revision, challenge, and causal intervention remain distinct;
      ordinary ordering or delivery is never labelled listening, influence, leadership, or value.
- [ ] One focused TUI scenario, strict `ymp-tui` Clippy, formatting, and `git diff --check` pass; a
      PNG is produced for the visually changed normal-size screen.

## Current state

The transcript has an inert `BoardMessage` form and Application resolves exact payload bytes, but
`MessageView` drops the persisted audience before `operator_board_projection()`. Implementation
stopped without changes because TUI cannot reconstruct scope or recipients honestly.

## Next action

After W1-COR-03e.4 preserves exact audience in the owned projection, fast-forward the retained clean
workspace and implement the original vertical slice.

## Guardrails

- TUI code depends on the typed Application projection, never `BoardLedger`, `BoardStore`, object
  paths, or raw collaboration storage.
- Untrusted text is escaped by the existing terminal text boundary and cannot create actions.
- Visual ordering is provenance, not causation or ranking.

## Findings

- Builder preflight found `MessageRecord.audience` is discarded by `MessageView/view_of`; deriving it
  from reader, order, relations or payload text would invent state, so work paused before editing.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

## Closure

Filled when the subtask is accepted.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.

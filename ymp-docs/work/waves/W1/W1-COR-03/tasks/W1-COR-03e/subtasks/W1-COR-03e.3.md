---
id: W1-COR-03e.3
kind: subtask
wave: W1
card: W1-COR-03
parent: W1-COR-03e
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03c, W1-COR-03d, W1-COR-03e.2]
blocks: []
created_at: 2026-09-01T21:43:34+08:00
updated_at: 2026-09-01T21:43:34+08:00
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

# W1-COR-03e.3 — TUI traces commitments and candidate provenance without causation

## Outcome

Existing reachable TUI pages let the operator follow one local commitment chain from offer and bid
through mutual contract, obligation, attempt and submission to immutable candidate and verifier
result, while competing branches and offline intervention evidence remain visibly separate and no
layout or metric claims assignment, winner, influence or causation.

## Scope

### In

- Map accepted Application/domain projections into the existing events, agents, candidates and
  describe pages; reuse current routes, tables and transcript rather than adding a dashboard.
- Show exact identifiers and typed links for offer, bid, task contract, obligation, lease,
  participant/attempt/invocation, submission, candidate ancestry/synthesis and verification result.
- Keep sponsor/contractor visibly temporary and participant-local; competing/stale/conflicting
  candidates remain separate immutable branches with no preferred canonical winner.
- Add an observatory section that labels live declared provenance separately from offline message
  intervention evidence; missing intervention evidence is `not studied`, never `no influence`.
- Deterministic constrained 80×24, normal 120×40 and wide high-volume fixtures, using only TUI
  projection/state/render/test paths. One normal-size PNG is created externally if the page changes
  materially.

### Out

- No-touch: Application/domain/board/storage/runtime/protocol, allocation/admission/verification
  decisions, message-content analysis, causal estimators, ranking, full parent redesign and MVP
  features. W1-COR-03e.2 owns resolved message transcript rendering.

## Acceptance

- [ ] Through existing reachable routes, one fixture traces every exact id in
      offer→bid→contract→obligation→attempt/invocation→submission→candidate→verification order;
      missing/stale links render typed gaps without inventing ancestry.
- [ ] Two competing candidates, stale/conflicting integration and participant-sponsored synthesis
      render as distinct branches; ordering, centrality, message volume and verification presence do
      not select or visually prefer a winner.
- [ ] Live declared provenance and offline intervention result occupy different labelled fields;
      absence/invalidity is explicit, and temporal order or delivery never becomes listening/value.
- [ ] A semantic mutation adding assignment/rank/winner/listening/influence/leadership/value/causal
      labels fails the view-state/source invariant before rendering.
- [ ] 80×24, 120×40 and wide high-volume TestBackend checks, strict TUI Clippy, formatting and
      `git diff --check` pass in an isolated root; one external PNG is visually inspected when useful.

## Current state

Commitment and candidate mechanics are accepted, and current TUI already has events, agents,
candidates and describe routes. W1-COR-03e.2 is adding the resolved-message transcript seam. The
remaining parent POC-2 criterion is a minimal provenance reconstruction across existing pages.

## Next action

After W1-COR-03e.2 is accepted, run a Significant contract check of exact current projections and
page files, then dispatch one Sol high builder.

## Guardrails

- The interface observes typed facts; it never decides work, truth, quality or causation.
- Colour and ordering carry no authority or semantic conclusion.
- Every cross-page link names a committed identifier, never message-derived text.

## Findings

- Created to close the remaining parent W1-COR-03e/POC-2 criterion after the transcript slice; it is
  sequential, not concurrent, because both tasks may touch the same TUI projection/render paths.

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

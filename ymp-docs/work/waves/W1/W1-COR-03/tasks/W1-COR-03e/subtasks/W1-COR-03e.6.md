---
id: W1-COR-03e.6
kind: subtask
wave: W1
card: W1-COR-03
parent: W1-COR-03e
state: ready
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: [W1-COR-03e.2]
created_at: 2026-09-01T22:52:28+08:00
updated_at: 2026-09-01T22:52:28+08:00
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

# W1-COR-03e.6 — Primary user journey turns an ordinary goal into a working result

## Outcome

An owner-approved user-journey contract tells one complete, ordinary-language story from launching
`ymp` with a goal such as “make a Battleship game” through clarification, autonomous work, playable
result, revision, acceptance and honest failure recovery; only after this story is accepted may its
interaction model be translated into screens.

## Scope

### In

- Create `ymp-docs/USER_JOURNEY.md` as the authority for the primary POC user, job-to-be-done,
  chronological journey, user stories, failure/return paths and experience invariants; add it to the
  document index.
- Primary user: a person who wants a working project result and does not operate runtimes,
  multi-agent protocols or experiments. The concrete reference story is creating and revising a
  playable Battleship game from a plain-language request.
- For every journey step state: trigger, what the person sees, the minimum decision they make, what
  the system handles itself, the visible progress/error, the observable success and the next move.
- Cover first launch, initial goal, only necessary clarification, work in progress, preview/run,
  requested revision, acceptance/export, cancellation, recoverable failure and infrastructure STOP.
- Define the interaction grammar without drawing it: normal conversation, optional expert command,
  inspectable detail, and consequence-bearing decision. A UI surface is chosen later from this
  grammar rather than from system components.
- Name the two or three unavoidable decisions that require the person: material spend beyond an
  accepted default, external publication/access, and an irreversible action. Phrase each in user
  consequences, never internal authority terms.
- Define the visibility boundary: contracts, budgets, routes, agents, oracles, attestations,
  digests and experimental apparatus are hidden from the primary journey and available only through
  explicit diagnostic details when they help resolve a failure.
- Map each promised step to current accepted product behavior or mark it explicitly as an
  implementation gap; separate the production-foundation POC journey from later MVP breadth.
- Exclusive write zone: new `ymp-docs/USER_JOURNEY.md` and its single index entry in
  `ymp-docs/README.md`; no visual or implementation files.

### Out

- Screen layouts, HTML/PDF, colors, typography, popup or slash-command catalogues, Ratatui code,
  protocol/runtime/Application changes, marketing personas and speculative MVP automation.

## Acceptance

- [ ] The document opens with the ordinary user goal and tells one continuous Battleship story from
      empty project to runnable result, revision and acceptance without requiring knowledge of ymp
      internals.
- [ ] Six to ten “when / I want / so that” user stories each name observable success, refusal or
      recovery, evidence shown to the person and the next available action.
- [ ] Success, clarification, cancellation, recoverable failure and infrastructure STOP paths are
      complete; none ends at a diagnostic label without a human next step.
- [ ] The primary journey asks for no contract approval, runtime/model selection, agent management,
      oracle inspection or budget-vector editing. An automated check scoped only to the primary
      journey fails if those internal duties are inserted there.
- [ ] Conversation, optional command, detail inspection and consequence-bearing decision have one
      clear use rule each. Slash commands and modals are possible implementations, not mandatory
      steps in the story.
- [ ] Every promised capability is linked to accepted current behavior or labelled a gap; POC and
      MVP are separated without presenting future behavior as implemented.
- [ ] The scientific researcher confirms that visible progress/evidence does not claim listening,
      communication value, coordination or self-organization; the owner approves the complete story
      before any new screen contract is dispatched.

## Current state

The owner rejected candidate 893a7d1 because it preserved the old system-oriented information
architecture under improved formatting. Scientific research and a cross-provider committee are now
deriving the primary user story before another interface is drawn.

## Next action

Converge the researcher and committee reports into one user-journey draft, then present that story
for owner approval before writing any UI contract.

## Guardrails

- Primary-path language describes the person’s goal, result and choices, not ymp’s internal nouns.
- The system asks only for information that changes the desired result or an unavoidable external
  consequence; implementation convenience never becomes a user decision.
- No screen is designed and no current visual artifact is treated as authority in this subtask.

## Findings

- Owner feedback established that the actual product job is “generate a working result such as a
  Battleship game”, not “operate a multi-agent experiment”.

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

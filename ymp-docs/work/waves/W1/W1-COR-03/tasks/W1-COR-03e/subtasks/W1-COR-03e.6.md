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
- Cover provider and pool preparation as a separate occasional setup journey, not a tax on every
  task: inspect installed/authenticated availability, enable or disable a provider with plain-language
  data-disclosure consequences, choose which provider/model profiles are eligible to populate the
  pool, and set participant/spend ceilings. Normal task execution uses the accepted pool automatically.
- State what a provider/pool change affects according to accepted product behavior — future
  participant starts, current work, stored evidence and data access — or label the behavior a gap;
  never imply live revocation or migration that the protocol does not provide.
- Keep the distinctive multi-agent system legible without making it the user’s job: the primary
  path always carries a compact factual team summary — participant count, provider/model identity,
  current declared task/obligation and status — while detailed team, task, board and artifact-flow
  views open only when requested.
- Let the person read the shared board and communication flow. A user-authored board message is an
  explicit, attributed intervention with visible scope; it cannot silently become a task assignment,
  capability or proof that another participant listened.
- Define the interaction grammar without drawing it: normal conversation, optional expert command,
  inspectable detail, and consequence-bearing decision. A UI surface is chosen later from this
  grammar rather than from system components.
- Name the two or three unavoidable decisions that require the person: material spend beyond an
  accepted default, external publication/access, and an irreversible action. Phrase each in user
  consequences, never internal authority terms.
- Define the visibility boundary: team composition, provider/model identity, declared work, board
  communication and artifact movement are product-level facts; contracts, budget vectors, routes,
  oracles, attestations, digests, leases and experimental apparatus remain diagnostic details unless
  their consequence directly requires a user decision.
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
- [ ] One story covers automatic team formation and progress: the person can see how many agents are
      active, which providers/models they use and their declared tasks/statuses without choosing or
      manually assigning them. Detailed team/task/board/artifact flow is readable on demand.
- [ ] One setup story covers provider enable/disable, authentication/error states, disclosure
      consent and the eligible pool. The person chooses the allowed resources and ceilings, not the
      semantic assignment of every task; effects on active versus future work are stated honestly.
- [ ] One story covers a user message to the shared board as an attributed intervention; audience,
      delivery and later participant action remain distinct and no automatic effect is implied.
- [ ] Success, clarification, cancellation, recoverable failure and infrastructure STOP paths are
      complete; none ends at a diagnostic label without a human next step.
- [ ] The primary journey asks for no contract approval, runtime/model selection, manual agent
      assignment, oracle inspection or budget-vector editing. It may show agent/provider facts but an
      automated check fails if viewing the team is turned into required configuration work.
- [ ] Conversation, optional command, detail inspection and consequence-bearing decision have one
      clear use rule each. Slash commands and modals are possible implementations, not mandatory
      steps in the story.
- [ ] Every promised capability is linked to accepted current behavior or labelled a gap; POC and
      MVP are separated without presenting future behavior as implemented.
- [ ] A protocol-derived control inventory classifies provider/pool eligibility, participant and
      recruitment ceilings, spend, cancellation, board reading/writing, export and diagnostics as
      primary, occasional setup or advanced detail; no internal mechanism is promoted without a
      user consequence.
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

- Primary-path language describes the person’s goal, result and choices. Team facts are visible in
  ordinary language; protocol mechanics are not promoted into required steps.
- The system asks only for information that changes the desired result or an unavoidable external
  consequence; implementation convenience never becomes a user decision.
- No screen is designed and no current visual artifact is treated as authority in this subtask.
- External UI/UX and TUI skills are not installed or treated as process authority. They may be read
  only as references after the owner accepts this journey; layout, interaction and Ratatui guidance
  cannot introduce a user step or information category absent from the accepted story.

## Findings

- Owner feedback established that the actual product job is “generate a working result such as a
  Battleship game”, not “operate a multi-agent experiment”.
- Owner decision: external design skills remain reference-only; a correct user story is a mandatory
  prerequisite and cannot be reconstructed from layout patterns.

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

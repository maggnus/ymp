---
id: W1-COR-03e.6
kind: subtask
wave: W1
card: W1-COR-03
parent: W1-COR-03e
state: active
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: [W1-COR-03e.2]
created_at: 2026-09-01T22:52:28+08:00
updated_at: 2026-09-01T23:30:29+08:00
started_at: 2026-09-01T22:52:28+08:00
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
- Begin `USER_JOURNEY.md` with fixed machine-readable front matter:
  `journey_status: draft|owner_approved`, `owner_approved_at`,
  `scientific_review: pending|confirmed`, `scientific_reviewed_at`, `journey_body_sha256`,
  `owner_approval_source` and `scientific_review_source`. The body hash excludes front matter so
  approval metadata can be added without changing the reviewed story. The author returns a draft;
  neither the author nor a UI reviewer may infer approval.
- Bind both approvals to immutable prior Git records in this task file. After the draft is stable,
  the standing researcher reviews its exact `journey_body_sha256`; the CTO records
  `SCIENTIFIC CONFIRMED <hash>` in W1-COR-03e.6 and commits it. The owner then explicitly approves the
  same hash; the CTO records `OWNER APPROVED <hash>` in a later task commit. Only afterwards may the
  journey front matter cite those two full 40-character commit URLs and become accepted.
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
- Keep internal requirement/check preparation visible in the main conversation as dim, collapsible
  system trace — project analysis, clarification, prepared checks and work start — without a blocking
  contract-authorization ceremony or typed internal identifier.
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
- Treat provider enablement as the only persistent permission state. Enabling starts a background
  available-model catalogue request; while it runs the provider remains enabled and loading. Success
  supplies eligible models. Failure leaves the provider enabled with an empty model list, exact error
  and retry action. There is no separate `sign-in verified` state.
- Catch later participant-start and invocation errors at the operation that failed instead of
  rewriting provider enablement. Disabling removes that provider's models from future pool snapshots;
  a current run continues on its frozen snapshot.
- Give `/agents`, `/pool` and `/board` distinct jobs: current participants and declared work; future
  eligible provider/profile/model resources and ceilings; full scoped communication and attributed
  human intervention. The main conversation carries only a compact team summary and latest material
  board fact.
- Treat Russian free-form intent normalization as a product prerequisite, not user ceremony. The
  current ASCII-English check derivation is a declared gap; a Russian goal must become a structured
  artifact/result contract internally, asking one outcome question only when platform or behavior is
  materially ambiguous.
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
- [ ] Contract/check preparation appears only as non-blocking system trace; no primary-path action
      asks the person to authorize a contract or type an internal identifier.
- [ ] `/agents`, `/pool` and `/board` are not aliases: the first observes the current team, the second
      configures future eligibility and ceilings, and the third reads scoped communication or posts
      an attributed human intervention.
- [ ] Provider state is mechanically honest: off or enabled; enabled has catalogue loading,
      models-present or models-empty-with-error detail. A failed catalogue request keeps the provider
      enabled but contributes no models to a future pool. Retry is explicit, and later operation
      errors remain attached to their own starts/invocations.
- [ ] The Battleship story begins with the Russian phrase unchanged. It records structured intent
      normalization/check derivation as an implementation prerequisite instead of requiring English
      artifact keywords from the person.
- [ ] Every promised capability is linked to accepted current behavior or labelled a gap; POC and
      MVP are separated without presenting future behavior as implemented.
- [ ] A protocol-derived control inventory classifies provider/pool eligibility, participant and
      recruitment ceilings, spend, cancellation, board reading/writing, export and diagnostics as
      primary, occasional setup or advanced detail; no internal mechanism is promoted without a
      user consequence.
- [ ] The scientific researcher confirms that visible progress/evidence does not claim listening,
      communication value, coordination or self-organization; the owner approves the complete story
      before any new screen contract is dispatched.
- [ ] The accepted document itself records `journey_status: owner_approved` with a non-empty
      `owner_approved_at`, and `scientific_review: confirmed` with a non-empty
      `scientific_reviewed_at`. It also records a computed `journey_body_sha256` and full immutable
      commit URLs in `owner_approval_source` and `scientific_review_source`.
- [ ] A readiness command recomputes the body hash, extracts both source SHAs, and uses `git show` to
      verify that the cited historical W1-COR-03e.6 contains respectively exact markers
      `OWNER APPROVED <hash>` and `SCIENTIFIC CONFIRMED <hash>`. Both cited commits must precede the
      final journey-approval commit. A temporary `draft`/`pending`, altered body, missing/wrong source,
      short SHA or mismatched marker exits non-zero. Chat history and reviewer summaries do not count.

## Current state

The owner rejected candidate 893a7d1 because it preserved the old system-oriented information
architecture under improved formatting. Scientific research, Codex MAX analysis and a cross-provider
comparison now agree on “result first, team transparently nearby” and are resolving exact POC gaps,
provider/pool setup and owner decisions before a document is drafted.

## Next action

Produce the draft and body hash; obtain and commit the standing researcher marker; present the same
hash to the owner and commit the explicit owner marker; then add the two immutable source URLs to
the journey, run the provenance-aware readiness check and accept this task before any UI contract.

## Guardrails

- Primary-path language describes the person’s goal, result and choices. Team facts are visible in
  ordinary language; protocol mechanics are not promoted into required steps.
- The system asks only for information that changes the desired result or an unavoidable external
  consequence; implementation convenience never becomes a user decision.
- No screen is designed and no current visual artifact is treated as authority in this subtask.
- External UI/UX and TUI skills are not installed or treated as process authority. They may be read
  only as references after the owner accepts this journey; layout, interaction and Ratatui guidance
  cannot introduce a user step or information category absent from the accepted story.
- Draft metadata is fail-closed. Only the owner supplies owner approval and only the standing
  scientific researcher supplies scientific confirmation; all other roles may inspect but cannot
  set those states.
- Approval sources are prior commits, never the final document's own commit, a branch, a short SHA,
  an agent report or conversation prose. Both approvals bind the same computed story-body hash.

## Findings

- Owner feedback established that the actual product job is “generate a working result such as a
  Battleship game”, not “operate a multi-agent experiment”.
- Owner decision: external design skills remain reference-only; a correct user story is a mandatory
  prerequisite and cannot be reconstructed from layout patterns.
- Owner decisions: contract preparation is a dim system trace, not a ceremony; `/agents` and `/pool`
  are separate current-versus-future views; `/board` is a separate communication view; Russian intent
  normalization is a backend prerequisite rather than a wording burden on the user.
- Owner decision: provider enable/disable is the permission; enablement fetches available models in
  the background. Catalogue failure leaves the provider enabled with no models, while all later
  failures are reported at their actual operation boundary.

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

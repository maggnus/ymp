---
id: W1-PRD-05j
kind: task
wave: W1
card: W1-PRD-05
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-PRD-05i, W1-COR-03a, W1-COR-03b]
blocks: [W1-EVL-04a]
created_at: 2026-08-16T09:45:00+08:00
updated_at: 2026-08-16T12:10:00+08:00
started_at: 2026-08-16T09:50:00+08:00
accepted_at: 2026-08-16T12:10:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/57f8b9b0c49bd5086f82a0f949c1746eb03a3a9a
closure_commit: https://github.com/maggnus/ymp/commit/da1ad180f94965d304082c0fc4a4735d6b56ef14
evidence: ["[57f8b9b](https://github.com/maggnus/ymp/commit/57f8b9b0c49bd5086f82a0f949c1746eb03a3a9a)"]
duration_minutes: 140
blocker:
pause_reason:
return_trigger:
deliberate_partial: true
---

# W1-PRD-05j — P10: recruitment through the kernel's mechanical gate

## Outcome

A running participant can propose recruiting another participant from the frozen pool; the kernel
checks ONLY mechanical constraints (frozen-entry membership, participant-starts budget,
concurrency, runtime admission, communication charges) and either admits the start as a journaled
fact or refuses with a plain-words reason. The semantic decision — whom to recruit, whether at
all — belongs to the proposing participant; the kernel never ranks, scores, or assigns. This
completes the minimum POC-2 mechanics: the coordinated arm of the study becomes constructible.

## Scope

### In

- A typed kernel command `request_participant { proposer, entry }` carrying only identifiers; the
  containment check against the frozen fact (EntryNotPermitted for entries outside it).
- Mechanical gates in order: frozen membership → participant-starts budget remaining →
  concurrency ceiling → runtime admission at this moment → offer-stage communication charge.
  Each refusal names its limiting constraint in plain words.
- Admission commits a `ParticipantAdmitted` journal fact (proposer, entry, profile, route,
  workspace) and charges the start budget; the new participant starts through the same managed
  path P9 built (shared start machinery, no second implementation).
- Idempotency: a repeated identical request is refused as duplicate; concurrent identical
  requests admit exactly one (serialized under the single kernel writer).
- Refusal of a proposer that is not a live participant of this run.

### Out

- Board wiring (who may see whom's request — separate child), semantic selection or scoring of
  candidates, participant replacement, ceiling negotiation, TUI surfaces (COR-03e).

## Acceptance

- [x] A live participant's request for a frozen admissible entry under remaining budget admits
      exactly one new participant, journals the fact, charges the start; the new participant runs
      through the P9 start path.
- [x] Each mechanical gate refuses with its named reason: entry outside the frozen set
      (EntryNotPermitted), participant-starts exhausted, concurrency ceiling, runtime unadmitted,
      offer-stage charge unaffordable. Negative half: a scenario per gate on the tree without
      that gate check shows the violation happening (budget overspent / non-frozen entry started).
- [x] Duplicate and concurrent identical requests admit exactly one participant; the journal
      records one admission fact; a repeated replay of the same command id is refused.
- [x] A request from a non-participant (or a yielded proposer) is refused without journal side
      effects on admission.
- [x] The kernel performs no semantic selection anywhere in the path: no model scoring, no
      "best entry" choice, no priority among frozen entries beyond declared order of the entry
      named by the proposer.

## Current state

Accepted after one return round. The reviewer's round-1 falsifier (120 generated command series
with randomized gate states plus a two-writer race on one store) held: no admission passed with
any unsatisfied gate, one request_id yielded exactly one admission, and three product mutations
broke the property. Round 2 (ACCEPT) confirmed the tui fold, the live-proposer gate
(slice holder or root participant; a failed start yields ProposerNotRunning/NotStarted with a
byte-identical journal), and a green workspace check. Merged with P9 in one pass; full suite on
the merged tree: 755 passed, 0 failed.

## Next action

Residual below — the P9 wiring child also carries this card's connection to the real start path.

## Guardrails

- The kernel checks mechanics only; any semantic wording ("better model", "top entry") in this
  path is a defect against INV-1.
- Every admission spends budget; no free starts, no unaccounted offers.
- All refusals are honest and named; an exhausted budget is never presented as a participant
  failure.

## Findings

None blocking. Review findings recorded as residuals.

## Closure

### Accepted outcome

The command `request_participant { request_id, proposer, entry }` runs five gates in the declared
order, each refusal naming its limiting constraint; admission journals `ParticipantAdmitted`
(schema v7) with the charge facts in the same record; the newcomer's identity derives from
`request_id`, so any number of deliveries admit one participant; the single kernel writer makes
concurrent duplicates safe; the command carries no field from which a preference between entries
could be read. The tui journal folds the admission record and renders it as one transcript line;
no new screen appeared.

### Residuals

1. (minor, additional-work) `ParticipantStartPath` exists only in ymp-testkit; the production
   start path arrives with the P9 wiring child. Return trigger: that child's acceptance.
2. (minor, hypothesis-refinement) With a single endowment a newly admitted participant cannot
   itself recruit — one more unit of entitlement than the endowment grants is required; a second
   tier opens through stake transfer in the contract pledge. Known property of the budget model,
   recorded for the study design. Return trigger: any coordinated-arm scenario needing chained
   recruitment within one endowment.

### Evidence

- [57f8b9b](https://github.com/maggnus/ymp/commit/57f8b9b0c49bd5086f82a0f949c1746eb03a3a9a) — candidate head, range 6c9bf19..57f8b9b
- [da1ad18](https://github.com/maggnus/ymp/commit/da1ad18) — combined P9+P10 integration merge; `cargo test --workspace` 755 passed / 0 failed (CTO on the merged tree)
- Reviewer falsifier evidence preserved at `~/.paseo/reviewer-evidence/W1-PRD-05j-reviewer-falsifier.rs`

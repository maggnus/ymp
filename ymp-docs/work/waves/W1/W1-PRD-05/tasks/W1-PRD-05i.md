---
id: W1-PRD-05i
kind: task
wave: W1
card: W1-PRD-05
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-PRD-05h, W1-APP-02c, W1-APP-02d, W1-COR-03a]
blocks: [W1-PRD-05j]
created_at: 2026-08-16T09:20:00+08:00
updated_at: 2026-08-16T12:10:00+08:00
started_at: 2026-08-16T09:22:00+08:00
accepted_at: 2026-08-16T12:10:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/ee7541976e767e237c0c05d264fbf8b6810281a4
closure_commit: https://github.com/maggnus/ymp/commit/da1ad180f94965d304082c0fc4a4735d6b56ef14
evidence: ["[ee75419](https://github.com/maggnus/ymp/commit/ee7541976e767e237c0c05d264fbf8b6810281a4)"]
duration_minutes: 168
blocker:
pause_reason:
return_trigger:
deliberate_partial: true
---

# W1-PRD-05i — P9: the origin participant starts from the frozen pool

## Outcome

When a run starts, the origin participant (decision D2: the first admissible entry of the frozen
pool) is launched as a real managed attempt against its own private workspace, under the runtime
profile and model route the frozen entry names — never a live-pool lookup. The operator authorizes
the run and sees the participant start in the transcript; the kernel records the start as a
fact; the participant may yield and be resumed. No recruitment (that is P10).

## Scope

### In

- Run start path (`ymp-application`): after `PoolFrozen`, resolve the origin entry from the frozen
  fact, admit it against the frozen containment predicate (already in place), and drive one
  participant attempt through the existing managed-runtime driver interfaces (Codex / Claude Code).
- Kernel facts: participant started (entry, profile, route, workspace), participant yielded /
  resumed / finished; budget charge for the start (participant-starts dimension).
- The attempt runs in a private workspace materialized from the run base, exactly as the
  single-participant path does today (W1-APP-02b).
- Refusal: if the frozen origin entry's runtime is not admitted on this host at start time, the
  run refuses with plain words naming `/runtimes` — the frozen fact stays untouched.
- Screen-level: the transcript and existing pages show the participant state through existing
  projections; no new screens (COR-03e owns observatory surfaces).

### Out

- Recruitment of further participants (P10), board wiring into runs (separate child), any
  collective decision-making, TUI observatory, participant replacement beyond yield/resume.

## Acceptance

- [x] Authorizing a run whose frozen pool has an admissible origin starts exactly one managed
      participant attempt with the profile/route/workspace from the frozen entry; the journal
      records the start fact; budget participant-starts is charged.
- [x] Changing the live pool or disabling the provider after freeze does not change which
      profile/route the participant starts with (negative half: on the pre-change tree the start
      consults the live pool — construct the discriminating scenario with the fake runtime).
- [x] A frozen origin whose runtime is unadmitted at start time refuses the run with plain words;
      the refusal is honest (no partial state, journal intact).
- [x] Yield/resume of the participant works through the existing lifecycle and never starts a
      second attempt for the same start authorization (idempotency).

## Current state

Accepted. Critical-depth independent review: ACCEPT. The reviewer's own falsifier was two-sided —
two runs with different freezes under one live pool started on different routes (opus vs sonnet),
proving the start follows the freeze, not the live pool; exactly-one-start held under store
reopen, direct command, and id retry; the freeze origin object and the start entry object are
byte-identical in events.jsonl. Merged with P10 in one calculated pass (schema-v7 and journal
conflicts resolved additively); full workspace suite on the merged tree: 755 passed, 0 failed.

## Next action

Residuals below; the wiring child connects `start_origin_participant` to CLI/TUI authorization.

## Guardrails

- The kernel never consults the live pool for a run decision; only the frozen fact.
- Participant start is a budgeted, journaled fact — never free.
- No semantic selection: the origin is the recorded first admissible entry (D2), not a "best"
  choice.

## Findings

None blocking. Review findings recorded as residuals.

## Closure

### Accepted outcome

The run start command carries no entry of its own: the transition reads `origin` from
`pool_frozen`, checks it with the frozen record's own admissibility predicate, and copies it into
the start fact — the route cannot enter the journal from anywhere but the snapshot. The origin
start is simultaneously its attempt start; one unit of `participant_starts` is charged; refusals
happen before any record, in plain words naming `/runtimes`. Journal schema v7.

### Residuals

1. (minor, independent-defect) On an exhausted attempt budget the private copy is created and one
   `participant_starts` unit is charged before the domain decision; the refusal text says
   «nothing started» while the journal holds CommitmentKernelOpened, ParticipantRegistered,
   RunExhausted with the charge. Return trigger: any budget-edge card or the wiring child must
   decide the attempt budget before the copy and the charge.
2. (minor, additional-work) `ParticipantRuntimes` has no production implementation and
   `start_origin_participant` has no caller: application-layer and accounting behaviour is proven,
   but no real process has been launched on a frozen route. Return trigger: the wiring child
   (CLI/TUI authorization → start, including private Git init).
3. (minor, additional-work) All four route checks expect the same constant, so the maintained
   suite does not itself distinguish a route constant from a record read; the reviewer's
   two-run discrimination exists only in review evidence. Return trigger: fold the two-run
   discrimination into the maintained suite with the wiring child.

### Evidence

- [ee75419](https://github.com/maggnus/ymp/commit/ee7541976e767e237c0c05d264fbf8b6810281a4) — candidate head, range f2c29e8..ee75419
- [2726a91](https://github.com/maggnus/ymp/commit/2726a91) — integration merge (P9)
- [da1ad18](https://github.com/maggnus/ymp/commit/da1ad18) — combined P9+P10 integration merge; `cargo test --workspace` 755 passed / 0 failed (CTO on the merged tree)

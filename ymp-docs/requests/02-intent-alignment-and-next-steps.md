# Intent alignment and proposed next steps

This note compares [intent.md](../../intent.md) with the implementation at
revision **c443ad0** and proposes an order of work. It is written against the
intent as revised on 2026-09-12, which added explicit goals, a glossary, and a
definition of confirmation. Earlier drafts of this note are superseded.

That revision closed most of the contradictions an earlier draft raised. Naming
the runtime as the holder of assignments, authority, and final acceptance
resolved the question of who allocates work. Defining confirmation as evidence
independent of agent judgment, and stating that mutual agreement between agents
is not confirmation, resolved both the knowledge-supersession standard and the
acceptance-strength question. Part I is what remains open. Part II is where the
intent is now unambiguous and the code disagrees.

Every code claim below was read at c443ad0 and is cited with a line reference.
Where a claim concerns a branch condition, check it rather than inherit it.

## Part I — What the intent still leaves open

### I1 — The non-negotiable core is named but not enumerated

The intent says roles, coordination, and the method are chosen per task, and
separately that the runtime fixes assignments, authority, and final acceptance.
That settles who holds authority. It does not yet say which parts of the
protocol the team may not change.

The implementation already encodes candidate invariants that look sound: a
reviewer is never the executor, a task cannot be accepted by its executor, and a
failing deterministic check cannot be overruled by an approving model
([engine.rs](../../ymp-rust/crates/ymp-runtime/src/engine.rs), line 1011;
[team protocol](../protocols/team.md)).

**Proposal.** Enumerate the negotiable surface: how many plan proposals are
collected, whether bidding happens for a given task, how many reviewers a result
requires, decomposition depth, and what triggers replanning. Everything outside
that list is fixed. This boundary is a prerequisite for the capability in Part
III, stage 3.

### I2 — Standing is earned but never said to be revisable

The intent says roles and authority do not carry between assignments, and that
reputation does not rise on self-assessment or unconfirmed claims. It does not
say that accumulated standing can fall.

The implementation has an undocumented answer. The statistic reads the most
recent hundred observations for a key
([storage/lib.rs](../../ymp-rust/crates/ymp-storage/src/lib.rs), line 283),
which is a decay window nobody chose deliberately.

**Proposal.** State whether standing is revisable and on what evidence, then
make the window an explicit decision rather than a query limit.

### I3 — Superseded knowledge has no retention rule

Confirmation is now defined, so supersession has a standard: a newer entry needs
evidence independent of agent judgment. What remains unstated is what happens to
the displaced record. The intent also requires that knowledge keeps provenance
and links to confirming results, which argues for retaining and marking the old
entry rather than deleting it.

Current state: the `supersedes` field exists on the entry model
([model.rs](../../ymp-rust/crates/ymp-core/src/model.rs), line 211) and is
written as `None` at every one of its four call sites. Knowledge removal is
reachable only from the CLI and the TUI, never from the agent tool surface.
Nothing in the product can correct a stored entry today.

### I4 — One session per task versus continuing conversations

The intent says each user task runs in its own session. The product supports a
follow-up message that continues the same session and, when the user explicitly
requests more implementation, starts further work linked to the parent
conversation rather than in a new session
([engine.rs](../../ymp-rust/crates/ymp-runtime/src/engine.rs), `follow_up`).

That behavior is deliberate and useful: it preserves context and file paths.
**Proposal.** Either state that a follow-up requesting new work opens a new
session, or soften the principle to say that work on a task is conducted within
a session.

## Part II — Gaps between intent and implementation

### The defect worth fixing first

**A failing task discards its siblings' verification.** In `Engine::execute`,
parallel task results are joined into `finished` while any error is captured in
`error`. The error is returned at lines 662–664, before the loop at lines
665–669 that verifies everything in `finished`. One failing task therefore
discards the pending review of every sibling that completed in the same batch.
Their files remain on disk and their task records persist, but they are never
reviewed, never accepted, and the run reports failure. The same `?` in the
verification loop means one failed review also skips the remaining siblings.
Only the last error is retained; earlier ones are overwritten.

Default parallelism is three, so this is reachable whenever a plan produces
independent tasks. It directly contradicts the intent line that an agent's
failure must not destroy work already done and verified, and it is more severe
than the synthesis case recorded as YMP-104 because it can discard several tasks
at once. It is not currently in the register.

### Confirmation is defined but not enforced

The intent now says the basis of verification is recorded, and that a result
without confirmation is accepted but not confirmed and does not raise
reputation. The code satisfies neither half.

With an empty acceptance-command list, `checks` returns success with the text
that independent inspection is required
([engine.rs](../../ymp-rust/crates/ymp-runtime/src/engine.rs), lines
1067–1069). The deterministic veto at line 1011 then has nothing to veto with,
and acceptance rests entirely on one model's approval. The executor observation
is recorded from `review.approved` regardless (lines 1029–1038), so an
unconfirmed acceptance raises reputation exactly as much as a confirmed one.

There is also no way to tell a discriminating check from a vacuous one. Checks
run only after execution and in the final combined pass (lines 989 and 678);
they are never run against the working directory before the work begins, so a
command that already passed beforehand is indistinguishable from one the work
made pass.

### The rest

| Intent statement | Implementation at c443ad0 |
| --- | --- |
| An initial team is formed from the pool for the task and may change within limits | The team is the whole enabled profile set, captured at session creation and fixed thereafter (engine.rs, lines 210–219). |
| The number of concurrently working agents follows useful work and available resources | A fixed parallelism limit of three (config.rs, lines 58–63). |
| Agents may propose subtasks and revise assignment proposals | `task_propose` records a message of kind `proposal` and answers that it is queued "for the next planning boundary" (mcp.rs, lines 150–157). No code reads that kind, `plan` is called once (line 571), and no such boundary exists. |
| Executors, models and effort are chosen per assignment | Only the executor is chosen. Models are fixed per profile; effort does not exist on the agent profile or on `TurnRequest` (providers/lib.rs, lines 22–35). |
| Settings permitted by provider, model and user constraints are chosen per assignment | No capability check exists, because no setting is sent. ADR 0002 and YMP-111 cover this. |
| YMP controls a common budget including coordination and verification | Limits are turn count, parallelism, timeout and attempts. There is no resource budget. |
| Initial work gets a bounded resource; part of the budget is reserved for verification | No reservation of any kind. |
| An agent failure must not destroy verified work | The sibling-discard defect above; synthesis failure blocks an accepted result (YMP-104); turn-limit exhaustion ends the run as `paused` with no synthesis, leaving verified work unreported. |
| Wrong or stale knowledge is corrected or superseded by newer confirmed data | `supersedes` is never set; removal is not available to agents. |
| Session history allows reconstructing how decisions were made | `assignment_choice` records candidates, samples and per-candidate counts, but no task id (engine.rs, line 543). YMP-101 covers this. |

One behavior is consistent with the code but unstated in the intent. A unanimous
decline stops the run: `bid` forwards only willing agents, and `choose` fails on
an empty candidate list with "No available candidates" (engine.rs, lines
527–529). A team that organizes itself has no way to renegotiate, relax the
task, or escalate when everyone declines.

### What is already aligned

Roles are per-invocation and carry no standing between assignments. The runtime,
not an agent, assigns work and records acceptance, enforced by a per-process
capability token that tool arguments cannot override. Agents address the whole
team or one peer through the shared board. Reputation, data, events and
knowledge persist across sessions. The run is observable without intervention.

One point deserves explicit credit because it is easy to get wrong. Reputation
rests on verified results rather than self-assessment, and the implementation is
deliberate about it: a bid returns a willingness flag and a description of the
intended approach, and the description is discarded entirely (engine.rs, lines
934–948). Only willingness filters the candidate set; ranking comes from past
outcomes.

## Part III — Proposed order

**Stage 0, owner decisions, no code.** Resolve I1 through I4. I1 blocks stage 3.

**Stage 1, intent compliance, offline and quota-free.** Every item repairs a
behavior that contradicts an unambiguous intent line.

1. Verify siblings before propagating a task failure, and report the failed task
   without discarding accepted peers.
2. Record the basis of acceptance, and stop raising reputation on an acceptance
   that had no confirmation. Run the plan's acceptance commands against the
   working directory before execution so a vacuous check is distinguishable from
   a discriminating one.
3. Preserve a verified result when final synthesis fails (YMP-104).
4. Make turn-limit and, later, budget exhaustion graceful stops that report what
   was verified.
5. Handle a unanimous decline explicitly instead of failing with "No available
   candidates".
6. Honor Codex nonterminal retry notices (YMP-103).

**Stage 2, make the stated variables exist.** Agent-centric configuration
(YMP-109). Effort with the reputation key extended (YMP-111); [ADR 0002](../adr/0002-agent-identity-and-reasoning.md) rules
that applied reasoning defines a new execution variant, so the key must change in
the same commit. Budget policy, units, and the verification reserve (YMP-102).
Evaluation provenance (YMP-101). Task-term memory retrieval (YMP-106).
Names-only enumeration (YMP-105) is cheap and independent; take it whenever
convenient.

**Stage 3, the missing capability.** A team formed per task from the pool, a
concurrency level that follows available work, and a replanning boundary that
agent proposals actually reach. All bounded by the invariants fixed in I1.

**Stage 4, measurement.** Only now does a paired solo/team pilot measure the
design rather than the gaps. Run YMP-201 after stage 3. Measure the time goal
separately: it is the one goal the existing journal argues against, since three
recorded runs producing a verified simple page took 19.4, 6.7 and 15.5 minutes.
When the diversity question is posed, put it on the axis ADR 0002 names, which is
agent configuration. Two agents on one installation with different effort are
different participants, and grouping outcomes by provider is against the
project's own attribution rule.

## Part IV — Effect on the existing register

- **YMP-301 is largely answered by the intent's goals section.** The success
  class is now stated: exceed one strong agent at a comparable budget, with
  secondary goals for resource use, time, intervention and transfer. What the
  task still owns is the audience decision.
- **A new task is needed for the sibling-discard defect.** It is not covered by
  YMP-104, and it is the more severe of the two.
- **A new task is needed for acceptance basis and confirmation-gated
  reputation.** The intent now states the rule; nothing in the register
  implements it.
- **YMP-104 and YMP-102 are intent-compliance work** rather than proposed
  improvements, and their records should say so. YMP-102 additionally owns the
  verification reserve the intent now requires.
- **YMP-111 is correctly no longer parked.** ADR 0002 gives the reason: basic
  configuration must not wait on an unevaluated automatic policy.
- **YMP-201 should stay parked until stage 3,** and its reactivation criterion
  should be the stage-3 capability rather than a budget approval.
- **No task covers team formation per task or a replanning boundary.** That is
  the largest remaining gap between the intent and the product. YMP-203 tests a
  fast path chosen in advance by a developer, which is a different thing, and
  ADR 0002 leaves automatic phase policy outside its scope.

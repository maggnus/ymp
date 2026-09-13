# Session recovery after agent failures

Research only, completed on 2026-09-13. This document records findings and product
options; it does not authorize implementation, policy amendments or resuming the
reported session. The authoritative intent remains unchanged.

## Evidence and scope

Two independent Paseo assignments used the `paseo-cto:paseo-researcher` role:

- `332e0b99-79a7-4255-82ae-be642ff7707e`: `claude-opus-5`, high.
- `52111c72-73e2-4ddc-b677-5573306ba1a9`: `gpt-6-astra`, xhigh.

Both examined source at `4e68c55172b41a4f8dca9bb7103ab3d7b0bf70ef`
and narrow SQLite evidence through a read-only connection. Parent commit
`0b95e4a` only registered this research and the separate popup task. Both
researchers reported identical initial/final Git porcelain, containing only the
pre-existing dirty `ymp-docs/requests/02-intent-alignment-and-next-steps.md`.
The maintainer independently checked the cited execution and budget paths.
No application tests, builds, provider probes, live-session retries or workspace
recovery actions were performed. Researcher inference is separate from probing
the application's providers.

## Confirmed incident

Session `3223c6a9-9cd9-46c3-a275-72384504dad7` stopped after two of its
200 allowed invocations. Luna returned a seven-task plan. GLM's `review_plan`
invocation failed with a reported connection error before any accepted plan or
task graph existed.

- Proposal `243f5a16-bd2c-4db3-81c7-b2ef4625e580`, revision 1, survives in
  `plan_proposed`, with the completed author assignment and invocation links.
  There are zero task records and no `plan_committed` decision.
- With no tasks, `execute()` calls `plan()` again. The saved proposal is not
  used as a resumable planning-stage checkpoint. See
  [engine.rs](../../ymp-rust/crates/ymp-runtime/src/engine.rs), lines 1699–1703,
  and [provenance.rs](../../ymp-rust/crates/ymp-storage/src/provenance.rs),
  lines 199–215.
- A failed provider invocation records an allocation reconsideration with
  `ready_work=0`, then returns the error. Event 1930 has `executor=null` and
  retains GLM as the reserved reviewer; this does not dispatch recovery work.
  See `engine.rs`, lines 1536–1566.
- The plan-review call and response parser propagate errors immediately.
  `attempts=3` supports revision after a negative review, not general transport
  recovery. See `engine.rs`, lines 2145–2182.
- Unknown GLM usage does not independently block this incident: numerical token
  limits are absent. With the relevant token limits enabled, the captured
  `unknown_usage=stop` policy would refuse further admission. See
  [budget.rs](../../ymp-rust/crates/ymp-storage/src/budget.rs), lines 267–285.

Recovery is not universally absent. Existing interrupted execution tasks enter
review without automatic replay, and already accepted neighboring work is
retained (`engine.rs`, lines 1705–1726 and 2266–2268). This incident exposes a
gap before task creation, as well as the need to account for uncertain effects
outside task-bound execution.

The session has no fixed size, pinned roster or explicit eligible-agent subset;
its maximum membership is four. Its captured pool has 19 entries, which does
not establish current availability. Unresolved aliases are not confirmed model
choices. GLM requested read-only work but had effective `write_all` access;
released access and an empty directory do not prove an absence of side effects.

Luna reported 119,979 input and 2,040 output tokens, including 101,120 cached
input tokens. The displayed 122,019 is not evidence that all tokens were newly
billed. GLM usage is unknown.

## Product alternatives and shared recommendation

| Approach | Benefit | Tradeoff |
| --- | --- | --- |
| Manual continuation from a saved stage after every failure | Direct owner control over retry, replacement and waiting | Every failure interrupts autonomous work |
| Bounded automatic recovery with explicit manual controls | Retains the session and useful results while handling routine failures | Requires clear admission, failure classification and effect-inspection rules |
| An undifferentiated retry/replacement policy | Simpler initial behavior | Repeats permanent failures, can duplicate effects and can confuse rejected work with missing review |

Both researchers recommend the second approach. This recommendation is not an
approved implementation contract. Proposed behavior:

1. Preserve the result, its version, author, reviews and unfinished stage. Resume
   the existing plan review instead of planning again unless the proposal is
   invalid, stale or requires revision after substantive feedback.
2. Permit bounded retries for suitable transient failures. Authentication,
   exhausted quota and unsupported models require a changed condition or an
   admissible replacement. Shared-provider outages must not create independent
   retry storms for every affected agent.
3. Reassign unfinished work within captured constraints and the eligible pool,
   after validating current capability and availability. Provider unavailability
   is distinct from agent competence. Preserve usage and originating identities.
4. Continue unrelated work only while its dependencies, access, resource reserve
   and independent verification requirements can be satisfied. Missing review
   never becomes acceptance.
5. Establish that previous execution has stopped and inspect uncertain effects
   before repeating side-effecting work. Preserve MVP write serialization; do
   not imply isolation or rollback that the application does not provide.
6. Distinguish recovery in progress, waiting for an external condition, required
   owner action and an owner-requested pause. Display the saved stage, cause,
   continuation condition and available actions. Preserve explicit pauses.

For the incident, the proposed route is to assess uncertain GLM effects, restore
the review obligation for the saved proposal, choose an admissible independent
reviewer and create the graph only after acceptance. Actual replacement
availability has not been established.

## Reconciled wording and policy boundaries

Both researchers explicitly agreed to these corrections in their final replies:

- Existing execution-task recovery must not be described as absent. The missing
  transition is recovery of pre-task `review_plan` from its saved proposal.
- A genuine new participant ID is not prohibited. Creating or renaming an agent
  to fake independence or erase history is prohibited. A distinct eligible
  non-author may replace the failed reviewer within applicable constraints.
- Fixed size and pinned roster differ. Fixed size can permit replacement;
  a pinned roster does not authorize an undeclared replacement. See
  [team policy](../architecture/team-and-effort-policy.md), lines 22–31 and 98.
- A negative review requires revision or recorded disagreement handling. A
  failed invocation or malformed response is not a verdict. Previously recorded
  objections survive replacement, and retries cannot become reviewer shopping.

Allowing the owner to explicitly amend a pinned roster, captured pool or resource
limits in the same session, with versioned history, is a separate product option.
It is not approved here and is not necessary for this unpinned incident. Ordinary
resume preserves the captured policy; see
[resource budgets](../architecture/session-resource-budgets.md).

## Proposed acceptance scenarios

- A plan-review transport failure preserves the proposal version; continuation
  does not invoke its author again. A negative review instead retains objections
  and requests a justified revision.
- Multiple failures and a process restart retain accepted neighboring results.
  Uncertain writes are inspected before replay and expired authority is not
  restored automatically.
- A shared provider outage has bounded combined recovery activity. Permanent
  failures are not retried without an applicable changed condition.
- Loss of a pinned reviewer and unknown usage under a token cap remain distinct
  reasons for waiting or refusing admission. Neither self-acceptance, fabricated
  independence nor resetting past usage can bypass them.
- A waiting session shows its saved result and specific continuation condition;
  explicit owner pauses survive restart.

## Unknowns and next step

The underlying cause of the GLM connection error, current replacement
availability and actual side effects remain unknown. Runtime behavior was
inspected, not exercised through a new run. Numerical retry/wait limits and any
owner-controlled policy-amendment mechanism remain product decisions.

Next: discuss the product behavior with the owner and record a separately
authorized implementation contract. Do not start recovery implementation or
resume the real session merely because this research is complete.

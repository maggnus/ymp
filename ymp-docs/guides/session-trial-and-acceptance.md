# Session trial and acceptance protocol

This is the common protocol for investigating a real ymp session and accepting
its outcome. A study fills in the same criteria before execution and adds its
observations, explanations and verdicts afterward. It does not maintain separate
test and acceptance plans with different definitions of success.

Freeze behavioral validation semantics before launch, while allowing selectors
and other nonsemantic adapters to fit the delivered artifact. An optional control
or wording in a previous implementation must not become a hidden requirement.

The authoritative objectives are in [intent.md](../../intent.md). The
[goal-driven trials](../research/goal-driven-trials.md) define the wider research
questions. The historical [comparative protocol](../research/experiment-protocol.md)
and its numerical quotas remain historical; they do not authorize another run.
This protocol applies to software and non-software work. Browser tests are one
domain-specific evidence source, not the definition of product success.

## Study card

Before a run, record:

- The product question and decision that the result will inform.
- The exact goal, inputs, required deliverables and acceptance conditions.
- Study mode: diagnostic case, regression replication or controlled comparison.
- Baseline artifacts and known limitations; no retrospective conversion of a
  failed baseline into success because its artifact was usable.
- The allowed implementation change and variables held constant.
- Source and executable hashes, model/profile/catalog bindings, actual team
  constraints, requested/sent/reported settings, policy IDs and versions.
- Fresh output directory, application memory/reputation baseline and native
  continuation policy. Separate contexts do not prove absence of native memory.
- Existing execution and publication authority, resource settings and stop
  conditions. Preserve owner choices; do not invent new ceilings or infer that a
  default means unlimited execution. A protocol is not additional authority.
- Primary endpoint, required criteria and optional exploratory observations.

Use the existing runtime, trace/export readers, validators and terminal/browser
tools. Add a measurement only when it answers a named question not covered by
retained evidence. Do not create a new general-purpose runner for each case.

## One criterion record

Each criterion has these fields, in a table or structured record:

| Field | Purpose |
| --- | --- |
| ID and intent link | Which goal or principle is being evaluated |
| Requirement | Observable behavior required of the artifact or system |
| Hypothesis and competing explanation | Proposed mechanism and a plausible alternative |
| Expected transition or result | What must happen, including relevant prohibitions |
| Measurement and evidence | Exact event, source, output, hash or user action |
| Verdict | pass, fail, unknown or not exercised |
| Analysis | Why the evidence supports the verdict; where it is inconclusive |
| Proposal and tradeoff | Product change, expected benefit, possible downside |
| Next discriminating check | The smallest observation that could disprove the proposal |

An unexplored feature is not a pass. Missing required evidence leaves acceptance
pending or unsuccessful. Keep the distinction between a requirement and an
exploratory hypothesis: a useful finding may reject a hypothesis without failing
an otherwise valid experiment, but cannot excuse a failed primary endpoint.

## Acceptance layers

Keep separate recorded verdicts even when they live in one report:

1. **Artifact:** externally observed satisfaction of the original task.
2. **Native completion:** real session outcome, task completion, final review,
   outstanding assignments and failed or unmet checks.
3. **Protocol:** assignment authority, independent review, version transitions,
   proposal decisions, persistence, recovery and preserved history.
4. **Policy compliance:** decisions follow the captured policy and constraints.
5. **Policy effectiveness:** the policy helped deliver useful work for its cost.
6. **Product experience:** the user can understand, use and recover the result
   with the intervention expected by the original request.

A policy can be compliant yet ineffective. A valid game can coexist with a
failed session. A completed session can produce an inadequate game. Record these
outcomes without collapsing them into a single favorable score.

Map each requirement to what native acceptance actually established: a
discriminating executable check, inspection only, a non-discriminating check or
no evidence. A replacement command can preserve the old oracle while both remain
unable to detect the claimed defect. Keep native evidence coverage separate from
later external validation and from the narrow fact that a command executed.

Runtime acceptance and trusted confirmation remain distinct. External research
validation must not rewrite runtime state, establish a missing trusted contract
retroactively, or award reputation. If runtime reports accepted/unconfirmed,
retain that label alongside the separately scoped external validation.

Bind native evidence, final review and external validation to the same artifact
version. A final hash alone does not establish the freshness of an earlier check.
Missing required bindings remain unknown. Assess available artifacts after a
failed session too; secondary artifact success does not erase native failure.

## Intent coverage and measures

| Intent goal | Observe in a diagnostic run | Additional evidence needed for a comparative claim |
| --- | --- | --- |
| Higher success on difficult tasks | Artifact verdict, native completion, escaped defects, preserved requirements | Matched task set and solo/independent-attempt controls |
| Lower resources at comparable quality | Per-agent and per-phase calls, input/output, cache subsets, completeness, repeated work, observer effort separately | Same quality conditions and defensible resource matching across treatments |
| Less time through useful concurrency | Time to first artifact and verified completion, overlaps, runnable work, access conflicts and actual wait reasons | Suitable independent work and an appropriate sequential comparison |
| Less human intervention | Requests for help, manual changes, restarts, diagnosis, approvals and delivery work | Comparable tasks and identical intervention rules |
| Improvement from verified experience | Which knowledge was retrieved, supplied and referenced; origin, confirmation, corrections and reputation effects | Controlled history, held-out tasks and memory-off/stale-memory alternatives |

Do not interpret agent count, successful tool exits, token volume or message
count as quality. Do not infer harmful waiting from the count of wait records:
repeated records can describe the same interval. Do not claim a knowledge hit
helped merely because it appeared in context.

## Protocol and policy observations

Use the actual consumer to connect a decision to an effect:

| Responsibility | Compliance question | Effectiveness question |
| --- | --- | --- |
| Allocation | Were selected IDs/settings eligible and independently admissible? | Did each assignment address an unmet requirement or evidence gap? |
| Resource policy | Were captured limits/reservations and unknown usage handled correctly? | Did coordination or verification dominate useful work without adding evidence? |
| Coordination and board | Did a proposal receive a timely, version-bound decision? | Did substantive criticism change the next action? |
| Recovery | Were prior results, holds and uncertain effects preserved? | Was the diagnosed cause changed before another attempt? |
| Workspace access | Did actual permissions match the granted scope and serialize conflicts? | Could necessary verification be performed within the available scope? |
| Confirmation | Were checks current and tied to the reviewed result/criterion? | Did the evidence actually distinguish correct and incorrect outcomes? |
| Knowledge | Were provenance, applicability and confirmation retained? | Did supplied knowledge affect a later decision, and is that effect identifiable? |

Record policy implementation ID/version from the captured session, not the
current build's label. A planner's task difficulty and a reviewer's confidence
are claims, not calibrated measures. A reviewer who reads code but cannot run it
may still add value; record which evidence was independently inspected and which
was merely reported.

## Observation and intervention

Retain the source manifest, process boundaries, timestamped events, task graph,
assignment provenance, proposal-to-decision links, check source and output,
artifact hashes and independent validation. Use one final consistent trace where
possible. Interim captures must identify their sequence boundary.

Before a repeat, inventory accessible prior outputs, temporary checks, browser
profiles and analysis artifacts. Preserve baseline evidence in durable storage
with checked hashes. Where prior artifacts could satisfy checks or leak solutions,
record a scoped reversible relocation or an explicit contamination audit; do not
delete unrelated data or claim that a fresh directory is a sandbox. Keep relevant
failed environment conditions observable instead of repairing them invisibly.

Audit actual native transcripts for use of prior artifacts and out-of-project
check inputs when the application's own trace lacks command detail. Use bounded
relevant excerpts, source provenance and hashes; do not copy authentication homes.
Missing transcript coverage means the claim is limited. A stale screenshot or
check file cannot silently become new evidence merely because it exists.

Progress updates identify a meaningful change, its evidence and what it implies.
Do not relay every log line or label a model's speculation as a finding. Preserve
corrections to earlier interpretations, including who suggested an action versus
who submitted the structured proposal.

The observer must not repair the artifact, supply a solution, alter the team or
convert a blocked session to completed outside normal product actions. Explicit
interventions are separately recorded. Browser validation and publication after
the session are observer work, not autonomous team achievements.

Preserve the configured resource policy. No observer deadline is added unless
authorized. Silence alone is not proof of a hang: distinguish an active native
call, local command, required dependency, lock wait, crashed worker, repeating
unchanged action, and absent observability. A demonstrated non-progressing cycle
is recorded and diagnosed; do not silently terminate it and claim completion.

## Causal interpretation

For each material failure or inefficiency, reconstruct:

`requirement -> proposal/decision -> admission -> action -> evidence -> outcome`.

Locate the first divergence and check a competing explanation. Where needed,
use one targeted deterministic reproduction and one meaningful negative control.
Do not add an exhaustive matrix merely to make a report appear scientific.

A recorded first failing guard may conceal additional failing conditions. For
recovery, reproduce the actual last-attempt/state boundary and retain counters;
do not demonstrate only an easier first-attempt case. Limit a repair to its named
operation or declare broader decision changes as part of the intervention.

For a repaired product, establish the mechanism on the controlled reproduction
before the native repeat. A before/after native pair is an ecological replication,
not a randomized estimate of speedup: plans, native contexts and model behavior
may differ. Report that uncertainty even if the second run finishes much faster.

Count all attempts. Freeze criteria before execution and record amendments with
their cause and timing. Retrying until success demonstrates an eventual outcome;
it does not establish a high first-attempt success rate. Retain unsuccessful and
incomplete outcomes, their costs, and any changed implementation or procedure.

## Product evaluation and recommendations

Assess whether the user received a usable result, an accurate completion claim,
a comprehensible history and an actionable remedy when something failed.
Highlight useful cooperation as well as defects: a review that finds a real
error is valuable even when it takes time. An unchanged artifact after a task
does not alone prove waste; the task may have supplied necessary evidence.

Prioritize proposals by the observed user impact, mechanism confidence and scope
of correction, not by a fabricated numerical score. For each proposal name:

- The concrete trigger and before/after behavior.
- The evidence supporting the proposed mechanism.
- The existing subsystem responsible for the change.
- What could become worse or be incorrectly permitted.
- A small acceptance check and the claims it cannot establish.

List rejected alternatives and why they do not solve the observed problem. Keep
future comparisons separate from current authorized repairs and runs. Do not
turn every exploratory observation into an implementation task.

## Completion and reusable output

A study is complete only after its declared acceptance layers have evidence and
the final report contains causal analysis and product recommendations. A published
artifact alone is not experiment success. A progress message is not final proof.

Store the plan, final criterion records, run ledger and evidence index in durable
project documentation; retain bulky raw logs separately with hashes and locations.
Do not overwrite the baseline report. If publication is in scope, use a distinct
URL/version per run and verify the uploaded bytes without publishing private logs
or configuration.

After an actual use, update the project trial skill with reusable lessons that
changed decisions. Keep case-specific IDs, models, budgets and browser commands
in the study card, not in universal skill requirements.

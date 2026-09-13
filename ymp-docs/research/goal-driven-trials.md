# Goal-driven product trials

Product proposal, 2026-09-13. The owner requested a product-led recommendation
grounded in the approved goals, rather than further discussion of policy knobs.
This document defines the proposed next trial program. It does not authorize live
provider expenditure, claim measured strategy benefits, or start a new UI feature.
YMP-146 failed-agent recovery remains P0; YMP-145 remains independently in flight.

The owner explicitly requires separate approval before comparative trials,
including calibration. A completed preparation package is not execution approval.
The concrete protocol, treatment configurations and resource envelope must be
approved before YMP-201–204 measured runs; no current approval is recorded.

## Product decision

The next useful experiment is whether a coordination strategy can select the
next valuable contribution toward the user's acceptance conditions, within the
captured constraints. Team width and temporary roles follow that decision.
Do not make the owner select internal roles or tune eight policies for ordinary
use. Keep alternative implementations selectable by the experiment runner first;
a public strategy selector should follow demonstrated, understandable tradeoffs.

The first candidate should use unmet acceptance conditions and available evidence
to choose among execution, targeted research, independent verification, a bounded
alternative approach, revision and waiting. Every proposed new invocation should
identify its intended contribution, evidence or result dependency, resource
allowance and termination condition. Do not invent calibrated probabilities or
an expected-value score before evidence supports one.

Roles are assignment-scoped responsibilities. Parallelism is useful only where
both task dependencies and actual backend access permit it. A disagreement may
justify a targeted check or alternate hypothesis; it does not automatically
justify another discussion round, an extra agent or higher effort.

## Goals and measurements

The authoritative [intent](../../intent.md) has five goals. Do not replace them
with a single opaque score, agent count, invocation count or token throughput.

| Goal | Primary observation | Required countercheck |
| --- | --- | --- |
| Solve difficult tasks more often | Externally confirmed completion within the declared resource/time envelope | Same acceptance conditions; report false acceptance and budget overshoot separately |
| Use fewer resources at comparable quality | Total measured resources across all attempts divided by confirmed outcomes | Include failed work, coordination, selection, review and learning; unknown use stays unknown |
| Reach a checked result sooner | Time to confirmed result and fraction failing to finish by the deadline | Report resource increases and failed/censored runs, not only times of successful cases |
| Require less human intervention | Manual actions and active human time needed to unblock or repair a run | Separate legitimate requirement clarification from intervention caused by system failure |
| Improve subsequent tasks | Amortized resources/time and held-out success after acquiring verified experience | Charge acquisition and correction; include stale, irrelevant and changed-requirement controls |

Each study declares one primary endpoint before execution. Others remain
constraints or secondary observations. A favorable secondary metric cannot
retroactively replace a failed primary hypothesis.

## What already exists

- [YMP-evals](../../ymp-evals/README.md) has four universal artifact workflows,
  thirteen protocol cases, external validators, negative controls and a trusted
  runtime driver. Accepted scripted runs establish runtime behavior, not model
  quality or cooperation benefits.
- The original [experiment protocol](experiment-protocol.md) and YMP-201–203
  already cover solo/team comparisons, experience and reasoning settings. Their
  live studies are paused and their historical numerical envelopes are unapproved.
- Allocation, resource, board, knowledge, execution and confirmation interfaces
  have actual replacement consumers. Board ordering does not replace the whole
  workflow; initial planning breadth, phase transitions and arbitration remain
  partly embedded in the engine.
- No controlled result currently establishes a product advantage over a strong
  native solo agent. Existing small deterministic fixtures are insufficient to
  measure success on difficult tasks.

## Trial order

### 1. Establish recoverability without native inference

Accept the P0 recovery slice using the real runtime with scripted failures:
failure before plan review, failure after a candidate exists, uncertain writes,
multiple failures from one provider, restart, explicit owner pause and owner
replacement of a busy participant. Require preservation of accepted results,
bounded recovery, correct access/accounting, and truthful waiting where completion
is infeasible. A finite failed run is not a success simply because it avoided a
crash. These controls extend the current driver rather than replacing it.

### 2. Materialize the minimum product-comparison package

Prepare four diagnostic pilot tasks with independent acceptance, spanning:

1. A small precise task, exposing needless planning and communication overhead.
2. A task with genuinely independent contributions, exposing useful concurrency.
3. A difficult task with plausible competing hypotheses and a checkable result,
   exposing the value of alternative approaches and targeted criticism.
4. A sequential, constraint-heavy task, exposing harmful over-parallelization and
   loss of requirements during delegation.

Use software and non-software artifacts. The existing SW/DA/DO/PL/CT catalog is the
source of concrete fixture candidates: regression repair, cross-table data
reconciliation, source-backed document synthesis and constrained scheduling.
The owner's poker request is a useful recovery case, but one open-ended game
request cannot establish general team superiority. Freeze exact inputs, required
outputs and hidden external check data before measured runs. Do not call this
catalog materialized until fixture files and discriminating validators exist.

Keep separate calibration instances and matched held-out variants. Expand the
catalog only after the diagnostic pilot demonstrates valid measurements and
identifies which task structures warrant further evaluation. A trivial task on
which both variants always succeed tests overhead, not difficult-task success.

### 3. Compare three controls before adding many strategies

| Treatment | Purpose |
| --- | --- |
| Strong native solo agent | Full native tools, planning and self-correction within its entire allowance |
| Independent attempts with budgeted output selection | Distinguish multiple attempts from a benefit caused by communication |
| Current cooperating ymp team | Establish the actual product baseline, including all its coordination and review work |

Use one resolved model and fixed supported effort for the first controlled
comparison where viable. Distinct team participants must have genuine identities
and separate native contexts with verified bindings. Do not label copied profile
names as evidence of independent execution. Multi-model allocation and effort
changes belong in later controlled comparisons.

The first diagnostic pilot proposal is four tasks, three treatments and two
repetitions: 24 complete outcome attempts. Repeated attempts do not constitute
24 independent task types or establish a small quality margin. Native model,
effort, measured resource unit, per-attempt/aggregate ceilings, cancellation and
in-flight overshoot handling must be fixed in a separate executable quota sheet
before any live run. This document does not reuse or approve historical quotas.

When native usage is incomplete, do not claim equal expenditure. Homogeneous
input/output token volume is still distinct from monetary cost; retain cache
composition and accounting coverage. If an envelope cannot be bounded as claimed,
report that limitation instead of silently treating the attempt as budget-equal.

### 4. Test one goal-directed coordination candidate

Implement only the missing workflow decision boundary needed to contrast the
candidate with current cooperation. Reuse AllocationPolicy for participants and
settings, ResourceAllocationPolicy for allowances, existing board records for
communication and the shared runtime for validation. Do not fold recovery, memory,
model choice and a new collaboration protocol into one uninterpretable experiment.

Compare candidate versus current coordination on identical starting conditions,
with the same criteria and owner envelope. Hold model/effort and initial memory
conditions fixed. The candidate's mechanism hypothesis is that linking each
contribution to missing evidence or an unmet criterion reduces redundant work
while preserving useful independent checking.

Alternate-policy tests must alter real decisions and calls through the common
consumer. Evaluate both policies on an immutable copy of the same input before
applying either. Record event/state versions, policy identity/configuration,
decision chain and any strategy state/randomness. Input replay compares decisions
on that state; complete executions measure their downstream effects.

### 5. Test experience separately

Retain YMP-202's factorial comparison of memory and adaptive assignment after a
useful task class is identified. Keep adaptation instances separate from held-out
tasks and isolate application and native state across treatments. Include
irrelevant/stale memory and changed requirements; correct outputs on exact repeats
alone do not establish learning. Charge acquisition, retrieval, review and
correction when judging amortized value.

## Common execution and reporting rules

Randomize treatment order within each task and retain the seed. Use fresh input
directories and controlled native/application state; expected outputs and evaluator
implementation stay outside agent-visible material. All treatments receive the
same declared requirements and allowed tools. If a clarification is permitted,
provide the same answer under the same rule and record it.

Record the run manifest, input/source/model/strategy versions, requested/sent/
reported settings, goal/criterion links, invocation purpose, communication and
review activity, waits, retries, human intervention, actual artifacts and external
acceptance outcomes. Reuse existing journals and exporters; add only missing
observations required by a named hypothesis. Count resources by originating agent
and include failed and cancelled attempts. Separate externally confirmed outcomes
from accepted-but-unconfirmed qualitative judgments.

The pilot determines feasibility, obvious regressions, task-family differences
and sample/budget needs for a confirmatory study. It does not declare a general
winner. Predeclare a useful effect and task-level uncertainty analysis before a
confirmatory study. Do not infer equal quality from a non-significant difference,
or count repeated versions of one task as independent evidence.

If cooperation does not outperform budget-matched independent attempts, the
observed benefit cannot be attributed to interaction. If speed gains disappear
under actual write serialization, do not advertise coding speedup. If extra
verification merely consumes budget without reducing escaped defects, revise that
coordination mechanism. Repeated false acceptance, resource-accounting corruption
or lost accepted work blocks adoption of the experimental candidate.

## Immediate decisions and remaining work

1. Complete and independently accept P0 recovery; keep existing popup work
   separate. Do not expand its backend assignment into a general coordinator.
2. Prepare the four-task comparison package, execution manifest and quota sheet
   under YMP-201. Reuse current fixtures, validators and exporter contracts.
3. Run a bounded measurement calibration only after explicit quota authorization,
   then the separately authorized three-treatment pilot.
4. Introduce and compare one goal-directed coordination candidate if the baseline
   measurements justify that mechanism; evaluate experience in its own study.

No public policy selector is required for these trials. The owner should normally
provide a goal, acceptance conditions and limits. Experimental implementation
selection belongs in the research configuration until a meaningful user-facing
choice has been established. This proposal preserves all five approved goals;
it does not narrow the universal product to its initial trial cases.

## Owner-directed first pilot (2026-09-14)

The owner now requests a small direct comparison of two or three weaker agents
with one stronger solo agent, with scientific grounding. YMP-201 is preparing
[this narrowed pilot](../research/weak-agent-pilot.md), reusing the existing runner
where adequate; it no longer depends on completing all of YMP-148/YMP-151.
The earlier broad protocol remains historical, and concrete native settings and
quota approval are still required before measured calls. YMP-202--204 remain
paused. This scheduling change does not delay the executing P0 recovery fork.

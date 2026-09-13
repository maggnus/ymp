# Coordination strategies and /team delivery

Owner-approved direction, consolidated on 2026-09-14. Implement interchangeable
session coordination for goal-driven experiments, expose the discussed choices
only on `/team`, and remove the catalog/team responsibility overlap. P0 failed
agent recovery remains first. This plan records work; none of the new outcomes
below is delivered merely by this document.

## Work packages

| Task | Outcome | Dependencies | Ownership |
| --- | --- | --- | --- |
| YMP-146 | Recover saved stages and provide trusted live-team commands | YMP-144 | Existing P0 backend fork; separate minimal UI integration |
| YMP-148 | Replaceable CoordinationPolicy and versioned strategy selection API | YMP-146 | Backend fork with independent review |
| YMP-150 | `/team` strategy/membership controls and distinct `/agents` catalog | YMP-148, YMP-149 | Claude Code claude-opus-5 high |
| YMP-151 | Reproducible comparison fixtures, runner integration and result export | YMP-148, YMP-147 | Evaluation implementation fork with independent review |
| YMP-201 | Calibrated, explicitly budgeted native comparisons | YMP-121, YMP-151; separate quota authorization | Experiment execution after preparation |

YMP-149 remains the concise-language pass after the accepted YMP-145 popup
component. YMP-150 consumes that wording and component baseline. After YMP-148,
UI and evaluation work may proceed independently in isolated worktrees; no
conflicting backend writes are started while YMP-146 is active.

## YMP-148: actual coordination replacement

The input contract includes the goal and unchanged acceptance conditions, event,
versioned task/stage/board state, eligible/current/occupied participants, results,
review objections, evidence coverage, available resources and policy revision.
The output proposes bounded work, temporary responsibilities, information
exchange, independent approaches, review/revision/dispute handling and the next
stage or waiting condition. Record why the contribution is needed and its links
to the goal, criterion, unresolved issue or dependency.

Reuse AllocationPolicy to resolve eligible participants/model/effort and
ResourceAllocationPolicy for invocation allowance. Keep recovery from YMP-146,
runtime admission, effective write access, accounting and acceptance evidence
outside replaceable authority. No strategy can make hidden model calls or grant
itself acceptance. Agent-assisted decision-making requests an ordinary recorded
assignment and consumes its returned evidence at a later decision event.

Extract the current workflow into a default implementation and demonstrate a
materially different bounded implementation through actual dispatch, not merely
a different label or board ordering. Preserve baseline behavior except for
explicitly recorded contract corrections. Register only usable implementations;
test-only substitutes do not become advertised public choices.

Information-sharing decisions must be honored by prompt construction, applicable
board/tool access and native continuation context. Do not claim blind independent
proposals if another channel, including shared artifacts, exposes peer outputs.
Record limits where the backend cannot enforce the proposed information scope.

Expose real typed read/selection APIs for the three discussed strategy families:
coordination, allocation and resources. Persist selected IDs, versions and actual
configuration on the session. A revision change applies at a compatible decision
boundary, retaining active assignments and earlier strategy attribution. Handle
stale requests, restart and a strategy unable to continue the saved stage with
an explicit result; do not silently reset the session.

## YMP-150: one place to operate the session team

Follow [team and agent surfaces](team-and-agent-surfaces.md) and the
[language guide](../guides/ui-language.md). `/team` owns current participants,
temporary work, pending departures and the coordination/allocation/resource
choices. `/agents` owns native discovery, capabilities and profile defaults.
Adding a member opens a chooser over the shared catalog; the full catalog is
not permanently duplicated in `/team`.

All membership actions use the accepted trusted session command API. Clearly
distinguish selected-session operations from the next-session draft. Include
unknown/unavailable state, pending strategy application and historical membership.
Provide no `/settings` selector, strategy-default panel, separate policy page,
recovery mode selector or arbitrary parameter editor.

## YMP-151: experiments exercise the same implementation

Reuse the existing evaluation driver, validators and exporter. Materialize the
four diagnostic task structures and separate calibration/held-out inputs from
[goal-driven trials](../research/goal-driven-trials.md), with independently
discriminating acceptance checks outside agent-visible material. Preserve both
software and non-software results.

The manifest selects strategy implementations through the same registry/API as
the application and records source/input/model/strategy versions, actual settings,
information exposure, decision chains, random seeds/state where used, resources,
elapsed time, interventions, actual artifacts and confirmation coverage. Replaying
two strategies on one immutable input must not apply the first before evaluating
the second; full executions assess downstream effects separately.

Prepare strong-native-solo, independent-attempt and cooperating-team treatments.
The solo benchmark is an evaluation treatment, not a production strategy allowed
to bypass ymp's mandatory independent acceptance. Keep its full native planning,
tools and self-correction; score all treatments using the same external criteria.
Charge selection/review costs to the independent-attempt treatment.

Run only scripted/offline controls in this implementation task. Prove that changed
strategies alter real work and that failures, false acceptance, unknown usage and
lost work remain visible. Provide a concrete native-run manifest and quota-sheet
template for YMP-201, but do not launch models or claim comparative quality.

## Acceptance and release discipline

Each implementation package needs a focused failing control, observable behavior
through its real consumer, required formatting/Clippy/workspace checks and an
independent review. UI additionally needs representative terminal verification.
Record unsupported or unverified behavior explicitly. Do not count the contract,
selector, scripted test result and native product advantage as the same outcome.

The task register is authoritative for status. This decomposition replaces the
previous broad YMP-148 scope; it does not add a second implementation of its work.

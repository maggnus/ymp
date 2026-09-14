# Session statistics research brief

Owner request on 2026-09-14: investigate session statistics as a separate research
outcome, then implement the resulting design. The purpose is to explain who did
what in a session, with which models, for how long, using which resources and
knowledge, and with what results or errors. This does not resume model comparisons
or the deferred token-headline change.

## Research outcome

Produce an implementable metric dictionary and source-coverage map. For each
metric state its unit, entity key, event/source, aggregation formula, completeness,
live/final behavior, restart/idempotency rule and unavailable-data semantics.
Separate existing observable facts from missing instrumentation and causal claims.
Propose a narrow typed read/export contract and the smallest required collection
changes. Reuse existing Store, provenance, events, usage and knowledge records;
no external telemetry service, generic analytics framework or new dependency.

The primary attribution is session -> agent -> assignment -> invocation. Record
actual captured/requested/sent/reported model and effort per invocation; models
and providers are execution bindings, not substitutes for actor identity. A model
summary is a secondary rollup with explicit attribution/coverage. Changing an
agent's profile or model must not rename historical work.

## Required dimensions

| Dimension | Questions to answer |
| --- | --- |
| Participants and work | Which agents actually ran, which models/settings were used, which temporary roles/tasks/results/communications they owned, and what changed in the team |
| Time | Session elapsed time, observed invocation duration, recorded waiting/retry/owner-hold time, active interval overlap, critical dependency limits and actual parallelism |
| Tokens | Per-invocation canonical input/output/cache/reasoning and completeness, all phases/failures included, no duplicate cumulative or subset accounting |
| Knowledge | Retrieval requests, candidates found, selected excerpts actually supplied, explicitly recorded references/use and validation; separate native-provider memory from ymp knowledge |
| Outcomes | Completed calls, task acceptance, objective confirmation, rejection/revision, final session result and incomplete work |
| Errors | Provider/transport/protocol/admission/check/cancellation events, recovered versus unresolved cases, known attribution and unknown legacy attribution |
| Communication | Board/team/directed exchanges linked to their participants and assignments, rejected/stale proposals and the cost/duration of coordination where observable |

Duration of an invocation includes tools, provider/network and native processing;
it is not automatically model-compute time. Summed participant durations can
exceed elapsed wall time when they overlap. Different invocations, native requests,
tool calls and task attempts are different counts. Treat live durations as
provisional and preserve resumed segments rather than resetting history.

A retrieval that finds rows is not proof of useful knowledge. Distinguish found,
selected, supplied and explicitly referenced facts. Do not call a record helpful
without evidence or claim a causal reduction in cost from an ordinary hit counter.
No retrievals makes a hit rate undefined, not 0 percent; missing historical
instrumentation is unknown, not an empty knowledge base.

A correctly negative review can be successful checking work. It is not a provider
error or proof of reviewer incompetence. A completed transport call can still have
an unusable structured response. Accepted-but-unconfirmed work must remain distinct
from confirmed outcomes and must not acquire reputation through an analytics view.

## Practical grounding and boundaries

Use the actual 6e286a3a session audit and clean existing fixtures to show one concrete
example and data gaps. Read only the already authorized session projections/native
usage evidence; no other sessions or credential files. No new native requests,
probes, scans, experiments, user-directory writes or changes to the running app.
The installed 0.4.6, accepted current backend and legacy records may have different
coverage; identify that difference instead of backfilling imaginary facts.

Research should recommend an existing session/detail/usage surface where possible,
a drilldown into assignments, and a machine-readable export. UI placement is a
proposal until the research is accepted. Any UI implementation goes to Claude
Code claude-opus-5 high; the parent owns the typed backend contract and acceptance.
Keep current token presentation unchanged while this separate design is researched.

## Acceptance cases for the later implementation

Include live and finished sessions, resume and duplicate events, a changed model
within one agent, overlapping intervals versus wall time, failed/retried work,
partial/missing usage, accepted-unconfirmed versus confirmed outcomes, negative
review versus technical error, empty/missing knowledge retrieval, repeated
excerpts, and explicitly linked knowledge without assuming causal benefit.
Show consistent totals between summary, drilldown and export. Collection must
remain bounded and must not alter scheduling, grants, budgets or acceptance.

YMP-157 owns the research and proposed acceptance plan. YMP-158 owns implementation
only after that design is accepted. A written dictionary is not collected data or
implemented statistics.

# Attributable session statistics

Accepted YMP-157 research result, 2026-09-14. Research was performed read-only
with the owner-selected `gpt-5.6-sol`. It used existing source contracts and the
sanitized YMP-155 projection of session `6e286a3a`; no model call, test session,
user-data mutation or implementation was performed.

## Decision

Build statistics as a reproducible projection of one consistent `SessionTrace`
snapshot. Storage already reads the trace in one transaction. Every snapshot
includes `session_id`, `as_of_seq`, `read_at`, schema version and explicit coverage.
Summary, agent detail, assignment detail and JSON export must derive from the
same source rows and agree exactly.

The primary attribution is:

`session -> agent_id -> assignment -> invocation`.

An agent is the historical actor. Model, effort and provider are invocation
settings and may change without renaming the actor's previous work. Model totals
are secondary rollups that retain contributing agent IDs and coverage. Never
replace agent attribution with provider grouping.

## Metric dictionary

| Metric | Unit and key | Existing source | Coverage and interpretation |
| --- | --- | --- | --- |
| Captured and actual participants | distinct `agent_id` per session; actual requires an assignment/invocation/message origin | session policy/team, assignments, invocation origins | Requested membership and actual participation remain separate |
| Assignment work | count and state by agent/purpose/task attempt | assignments and linked decisions | Temporary role belongs to the assignment, not permanently to the agent |
| Session elapsed | interval from recorded session start to terminal transition or `read_at` while live | session plus new typed status transitions | Live value is provisional; legacy terminal end may be unknown |
| Invocation duration | ended-started per invocation | invocation timestamps | Includes provider, tools and waits internal to the invocation; not model-compute time |
| Participant work sum | sum of invocation intervals by agent | invocations | May exceed session elapsed when work overlaps |
| Active union and peak | union of all intervals and maximum simultaneous invocations | invocations | Measures observed concurrency; separate from participant work sum |
| Waiting | paired duration by category and optional task/assignment/stage/holder | new `WaitSpan` | Existing point events permit counts, not reliable historical duration |
| Calls | invocation, native response, tool call and task-attempt counts | invocation is existing; native activity is optional adapter evidence | These are different units and stay separate; unsupported native activity is unknown |
| Tokens | canonical input/output/cache/reasoning per unique invocation | final `UsageSnapshot` for each invocation | Sum the latest snapshot once; cache and reasoning are subsets; never sum `native_total`; partial observations remain lower bounds |
| Transport result | completed/failed/cancelled/interrupted per invocation | invocation state | Completed says nothing about response usability or task success |
| Response usability | usable/malformed/incomplete/ambiguous/schema-invalid/not-evaluated | partially inferable today; new `ResponseEvaluation` required | Record every structured consumer result, including negative valid reviews as usable |
| Task outcome | submitted/review-pending/accepted/rejected-for-revision/blocked/unresolved by attempt | tasks, results, reviews and decisions | Do not infer from message prose |
| Verification | valid positive/negative review and rationale | linked review record | A correct negative review is successful checking work, not a technical error |
| Confirmation | confirmed/unconfirmed/unknown per result version and criterion | confirmation/check evidence | Only linked criteria count; acceptance without confirmation remains distinct |
| Technical errors | classified provider/protocol/admission/check/cancellation failures and resolution | invocation/failure/admission/check records | Separate from negative reviews, stale board proposals and incorrect artifacts |
| Communication | distinct messages by origin/recipient/kind plus board proposal states | messages, message origin and board decisions | Count is activity, not usefulness; unattributed legacy messages remain unattributed |
| Knowledge requests | request count and scope | automatic retrieval events; team search lacks complete links | Native-provider memory is a separate unknown source |
| Knowledge candidates | found count and references before policy selection | new bounded retrieval fields | Missing historical data is unknown, not zero |
| Knowledge selected/supplied | selected references, resolution outcome, and unique context references included per invocation | supplied context partly exists; selected stage needs fields | Repeated inclusion is counted per invocation |
| Knowledge referenced | explicit memory/version/context link from a result/decision | new `KnowledgeUseReference` | A reference proves use attribution, not causal benefit or savings |
| Retrieval rate | requests with at least one candidate / requests | new candidate observations | No requests means not applicable; incomplete observations mean unknown |

Reputation observations remain existing historical outcomes. Computing statistics
must never create, alter or infer reputation.

## Minimal collection additions

1. `SessionStatusTransition { session_id, run_id, from, to, reason, at }` closes
   session intervals and explains pause/block/finish across restart.
2. `WaitSpan { id, session_id, category, assignment_id?, task?, stage_id?,
   holder?, started_at, ended_at?, resolution? }` pairs start and end using one
   idempotent ID. Categories are dependency, workspace access, concurrency,
   budget, owner, participant, recovery and provider backoff.
3. `ResponseEvaluation { invocation_id, purpose, disposition, schema,
   decision_ids, result_ids, at }` is written after every structured parse and
   validation. A valid negative review is `usable`.
4. Extend knowledge retrieval with bounded candidate references/count,
   policy-selected references, resolution outcomes and actually supplied
   references. Team `memory_search` records request ID, query digest, scope,
   mode, returned references/count and truncation without duplicating content.
5. `KnowledgeUseReference { memory_id, version, context_digest,
   supporting_criterion? }` is accepted only when the referenced excerpt was in
   that assignment's context. It does not assert causal usefulness.
6. `InvocationActivityObservation` is optional per adapter for native response
   and tool IDs/timestamps. Adapters without support report unknown coverage.

Do not add a general telemetry service, analytics database or new dependency.
Reuse the event/provenance journal, existing storage transactions and bounded
lists. New collection must not change scheduling, authority, budgets, acceptance
or provider authentication.

## Typed read and export contract

```text
SessionStatisticsSnapshot {
  schema_version, session_id, status, as_of_seq, read_at,
  summary: SessionStatisticsSummary,
  agents: [AgentStatisticsSummary],
  coverage: StatisticsCoverage
}

AgentStatisticsDetail {
  session_id, agent_id, captured_identity,
  invocation_and_model_rollups, outcomes, communication, knowledge,
  assignments: [AssignmentStatisticsSummary], coverage
}

AssignmentStatisticsDetail {
  assignment, invocations: [InvocationStatistics],
  messages, board_decisions, result_review_confirmation_error_links,
  waits, knowledge_context_and_use, coverage
}
```

Expose bounded typed readers equivalent to:

- `Store::session_statistics(session_id)`;
- `Store::agent_statistics(session_id, agent_id)`;
- `Store::assignment_statistics(session_id, assignment_id)`.

Readers use one read transaction and return `as_of_seq`. Detailed collections
use existing bounded pagination. A machine-readable `ymp stats SESSION --json`
opens metadata read-only, does not load current profile configuration, scan
providers or inspect the workspace. Exact public names may follow repository
conventions, but the semantic distinctions above are fixed.

## Interface proposal

Add `/stats` without changing `/usage`. The first level presents session outcome,
time, participants, task results, resources, knowledge, communication and data
coverage. One row per stable `agent_id` shows actual invocation configurations,
not the current profile. Enter opens assignments for that agent; another Enter
opens invocations and their linked messages, decisions, waits, knowledge,
results, reviews, confirmations and errors.

Existing `/tasks`, `/usage`, `/assignments`, `/decisions` and `/checks` remain
specialized views. `/stats` connects them rather than duplicating raw tables.
UI implementation belongs to Claude Code `claude-opus-5` with `high` after the
backend contract is accepted.

## Grounding in the audited session

At sequence 2579, the sanitized poker session has 14 completed transport calls,
but only 13 usable structured responses. Five tasks are accepted and unconfirmed;
the sixth remains in review. Therefore completed invocation, usable response,
accepted task and confirmed outcome are four separate measures.

Observed session time is 2651.091 seconds and summed invocation duration is
2643.861 seconds, with no overlap. Fifteen dependency waits are countable but
their durations are not reconstructable. Luna usage is complete; every GLM
observation is partial. Fourteen knowledge-retrieval events exist, but historical
candidate/selection/use stages are missing, so knowledge usefulness is unknown.

## Acceptance cases for YMP-158

1. Live, finish and reopen: provisional duration/usage becomes final; summary,
   agent, assignment and JSON totals agree; duplicate events do not add twice.
2. One agent changes model/effort between calls: history retains the agent ID and
   both configurations; model rollups preserve contributing agents and coverage.
3. Overlapping calls: work sum exceeds interval union, peak is two, and waiting
   is not inferred inside an invocation without evidence.
4. Completed-malformed, failed-partial, cancelled, retry, admission denial,
   negative review, revision and unresolved task remain distinct.
5. Knowledge covers no requests, zero candidates, candidates to selected to
   supplied, repeated supply, explicit reference and legacy unknown fields.
6. Missing historical assignment/message links and invocation gaps stay
   unattributed or unknown rather than being assigned to the current team.
7. Native activity supported by one adapter and absent from another yields known
   versus unknown, never a fabricated zero.
8. Collection and projection stay bounded and do not affect scheduling, grants,
   budgets, acceptance, reputation or existing token presentation.

## Unmeasured claims

Current records do not establish exact model-compute, network or tool duration;
complete native-provider memory activity; causal usefulness of knowledge or
messages; or monetary cost. Do not add such fields as zero or infer them from
elapsed time, hit counts or token observations.

# Session provenance and structured export

YMP-101 adds typed records in `ymp-core::provenance` and additive SQLite schema 3 tables. Existing session/task IDs, usage ordinals and historical events survive migration. A legacy session has `policy: null`; its original limits and settings are not reconstructed from current configuration.

`ymp --home ~/.ymp2 trace SESSION_ID` exports `SessionTrace` JSON. The command opens the existing store read-only, requires schema 3, and does not discover providers, load configuration, inspect the working directory or invoke agents. Normal store opening transactionally migrates older schemas. Newer schemas are rejected. The export contains current session/task records, the captured policy, assignments, invocations, decisions, canonical usage by agent ID, and complete ordered history. `schema_version: 1` identifies the export format, separately from the database version.

## Runtime integration

The engine captures the full goal, selected directory, limits, eligible profiles and initial team when creating a session. Structured constraints and evaluation run/scenario references are optional API fields. Normal team runs preserve constraints in the complete goal and do not claim to have extracted them. Resumes and follow-up questions use captured limits. Legacy sessions retain the existing current-limit fallback without presenting those limits as historical facts.

Each outer engine request starts a fresh assignment and invocation. Planning, bidding, execution, review, learning and narration therefore share the session's usage accounting. Task execution, bids and candidate reviews carry an explicit task ID and attempt. Result submission, plan commitment, candidate review/arbitration and acceptance decisions link their source invocations. Task acceptance records remain `confirmation: unknown` until YMP-117 supplies evidence grading. Native completion alone does not imply task acceptance.

Every valid planner output has a `plan_proposed` decision, including unselected proposals and revisions after rejection. `RecordLinks.plan_proposal` carries a `PlanVersion`: stable proposal ID, revision number, structured plan snapshot, and producer assignment/invocation IDs. A revision links the rejected review of its predecessor. Reviews and commitment retain the exact version, so commitment identifies both production and review without using message order. Storage rejects reused revisions, foreign producers, altered proposal content and reviews of another version. This optional field defaults to null in historical decisions; the database and export versions remain unchanged.

Assignment records capture agent/configuration identity, provider ID, requested model/effort/restriction, purpose, directory, timeout and context references. Context references identify messages, memory, task records, native continuation and prompt/instruction digests; they do not expose private reasoning. Providers report a separate `InvocationObservation`: transmitted settings come from the transport or initialized SDK options, reported settings come from allowlisted native response fields, and unavailable values remain null. Codex native turn IDs, Claude initialization versions and ACP agent versions are retained when observed. Usage snapshots retain existing canonical counts and complete/partial distinctions.

## Storage API

| API | Contract |
| --- | --- |
| `create_session(session, policy)` | Atomically inserts the session, immutable policy and events. An existing identity cannot be replaced. |
| `begin_invocation(assignment, invocation)` | Atomically inserts fresh identities, running accounting, durable turn ordinal and the start event. Scope and current task attempt must match. This does not issue permissions. |
| `observe_invocation(session, id, observation)` | Atomically updates allowlisted observations/accounting and appends their event. Requested settings are immutable; finalized usage cannot be replaced by a different snapshot. |
| `finish_invocation(session, id, state, reason)` | Atomically ends invocation, assignment and usage. Terminal records cannot be reopened or overwritten. Reasons should be runtime classifications, not raw SDK diagnostics. |
| `interrupt_open_invocations(session)` | Closes uncertain running records as interrupted. The runtime must own the session lock and end prior processes before calling it. |
| `record_decision(decision)` | Appends an immutable decision and event with task/assignment/invocation/review scope checks. |
| `save_task_with_decision`, `save_plan_with_decision` | Commit task state or the complete plan with its linked decision and events. Task submission/review requires the persisted attempt and assignee to match, with `running → review` or `review → accepted/ready/blocked`. These checks run before updating. Invalid state or links leave state and events unchanged. |
| `trace(session)` | Reads one consistent snapshot including complete ordered history. `open_read_only` also prevents writes through the connection. |

The engine closes cancellation and native failures explicitly; dropping an unfinished future records an interruption when storage is available. Process crashes are closed on recovery. Partial reports remain partial, missing reports remain unknown, and completed sibling tasks survive. Context continuation never reconstructs a live permission grant.

YMP-120 owns live grants and permission enforcement; YMP-117 owns confirmation/evidence validation and reputation qualification; YMP-102 owns resource admission. `RecordLinks` supplies grant, confirmation and evidence references for those implementations, and `DecisionOutcome` can preserve unknown, unconfirmed, confirmed and rejected outcomes without grading them. These APIs are for trusted runtime callers and are not an authorization boundary. Standalone diagnostic `ymp ask` and provider probes retain their existing accounting paths; the new complete invocation trace covers session-based engine runs.

## Validation

Offline regressions cover identity/scope collisions, stale task attempts, immutable captures, different requested/reported models, unknown native settings, excluded private payload fields, cancellation, dropped futures, partial usage, recovery, event-failure rollback, transactional migration, unsupported schemas, duplicate task titles, captured limits on resume, and read-only JSON round trips. The existing session-ID reassignment regression failed before the storage fix. Native fixtures and mock agents require no credentials or paid provider calls.

The independent-review corrections also have failing-before regressions: an attempt-one acceptance overwrote a running second attempt, and a normal two-planner trace contained no production decision referencing either planner invocation. The corrected tests verify unchanged state/history on stale writes and exact production/review links for initial, rejected and subsequently accepted revised plans.

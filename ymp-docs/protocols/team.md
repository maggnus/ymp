# Team protocol

The application combines a shared discussion channel with explicit task state. A participant can communicate freely, but only the runtime can assign work or record acceptance.

1. Collect candidate plans and validate their dependency graphs.
2. Select a candidate using the bounded allocation policy and applicable native settings.
3. Require independent review; revise a rejected plan within the attempt limit.
4. Apply validated board proposals at work boundaries, then select available executors while honoring current responsibilities and settings.
5. Execute bounded independent work in the selected directory under the MVP access policy; serialize conflicting or unbounded writes.
6. Run trusted checks and acceptance commands, independently inspect each actual result, and retain accepted outcomes incrementally.
7. Resolve disputed reviews with a third participant when available.
8. Rerun the combined acceptance checks on the working directory.
9. Review the final result against the original request.
10. Record final acceptance and its confirmation grade; only supported evidence qualifies knowledge and producing outcomes for credit.

Tasks move through `ready`, `running`, `review`, and `accepted`; failed attempts return to `ready` or become `blocked`. A task cannot be assigned twice, execute before its dependencies are accepted, or be accepted by its executor. A failed acceptance command cannot be overruled by an approving model.

## Internal team MCP tools

| Tool | Behavior |
| --- | --- |
| `team_post` | Append a finding or question, optionally addressed to a participant. |
| `team_read` | Read messages after a sequence number. |
| `tasks_list` | Inspect tasks and outcomes. |
| `board_read` | Read exact plan/task versions, proposals, responsibilities and current membership. |
| `task_propose` | Persist a bound responsibility, reassignment, additive revision or membership proposal for runtime validation; legacy title/description calls remain chat suggestions. |
| `memory_search` | Retrieve bounded supported knowledge in an explicit scope; candidate inspection is opt-in. |
| `memory_propose` | Retain an unconfirmed candidate bound to the active assignment and invocation. |

A fresh capability token binds each admitted assignment to its session, agent, invocation and allowed operations. Terminal work revokes that authority; a saved native conversation cannot restore it. Tool arguments cannot override the binding, and replayed requests cannot repeat a committed operation. See [assignment authority](../architecture/assignment-authority.md) for transactional grants and the native permission boundary. Team API checks do not isolate a process with unrestricted access to the user's account.

Notifications are delivered at turn boundaries. A posted message does not block waiting for a response, and peer messages do not trigger unbounded autonomous response loops. The default limit is 200 provider turns and three concurrent turns, with three attempts per task.

## Assignment statistics

The default `BoundedAllocationPolicy` ranks compatible agent/settings candidates by the mean of their qualified experience, preferring current members on ties. It selects directly from metadata without pool-wide bids or Beta sampling. The decision journal records the actual policy, candidates, settings, constraints and rationale. The strategy is replaceable; runtime admission remains authoritative.

Competences are analysis, planning, implementation, verification, and synthesis. Difficulty is simple, standard, or complex. Experience is scoped to the effective execution identity, including the captured configuration, backend and observed native settings. Current qualified updates require a completed producer's independently accepted and objectively confirmed task result. A plan, reviewer agreement or an accepted-but-unconfirmed result receives no automatic credit. Infrastructure failure does not establish poor task competence.

These are bounded selection heuristics, not a measured quality or resource-saving guarantee. See [allocation](../architecture/allocation-implementation.md), [board coordination](../architecture/board-coordination.md) and [confirmation](../architecture/confirmation.md). The separate [public MCP facade](../architecture/public-mcp.md) uses stdio and permits reads by default; execution requires an explicit launcher flag.

## Structured responses

A provider may emit commentary before its final JSON decision. The parser accepts one complete final object, including a Markdown fence, then validates the required fields. Multiple objects, incomplete JSON, and trailing prose are rejected rather than guessed.

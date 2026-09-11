# Team protocol

The application combines a shared discussion channel with explicit task state. A participant can communicate freely, but only the runtime can assign work or record acceptance.

1. Collect candidate plans and validate their dependency graphs.
2. Select a plan using the author's planning statistics.
3. Require independent review; revise a rejected plan within the attempt limit.
4. Collect bids for ready tasks, then select available executors by competence.
5. Execute in separate working copies.
6. Run acceptance commands and independently inspect the actual result.
7. Resolve disputed reviews with a third participant when available.
8. Integrate accepted changes and rerun the combined acceptance checks.
9. Review the integrated result against the original request.
10. Record resolved outcomes and independently reviewed reusable knowledge.

Tasks move through `ready`, `running`, `review`, and `accepted`; failed attempts return to `ready` or become `blocked`. A task cannot be assigned twice, execute before its dependencies are accepted, or be accepted by its executor. A failed acceptance command cannot be overruled by an approving model.

## MCP tools

| Tool | Behavior |
| --- | --- |
| `team_post` | Append a finding or question, optionally addressed to a participant. |
| `team_read` | Read messages after a sequence number. |
| `tasks_list` | Inspect tasks and outcomes. |
| `task_propose` | Record a proposed task or plan adjustment for team consideration. |
| `memory_search` | Retrieve verified knowledge in the current scope. |
| `memory_propose` | Record an unverified knowledge proposal. |

A per-process capability token binds tool calls to their actual participant and session. Tool arguments cannot override that identity. This prevents accidental cross-session attribution; it is not a security boundary against a process with unrestricted access to the user's account.

Notifications are delivered at turn boundaries. A posted message does not block waiting for a response, and peer messages do not trigger unbounded autonomous response loops. The default limit is 200 provider turns and three concurrent turns, with three attempts per task.

## Assignment statistics

For each profile-version × competence × difficulty category, maintain successes S and failures F. Sample from Beta(1+S, 1+F) for each eligible bidder and select the highest sample. The event journal records candidates, parameters, sampled scores, and the decision.

Competences are analysis, planning, implementation, verification, and synthesis. Difficulty is simple, standard, or complex. Profile versions include the provider configuration, model, and instructions. Resolved task outcomes update execution statistics. A plan receives credit after the integrated result succeeds. Reviewer observations require independently adjudicated outcomes; agreement alone is not evidence of correctness.

This is a practical bandit policy. Dependent reviews, subjective outcomes, model alias changes, and small samples violate idealized assumptions; no optimality guarantee is claimed.

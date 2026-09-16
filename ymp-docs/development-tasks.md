# Development task workflow

This is repository development tooling. A development task is not the product's
runtime `Task`, and claiming one does not start an agent or grant execution
permissions. Python is a development dependency only; the `ymp` executable does
not invoke this tool.

## Storage and authority

Store one authoritative JSON record per task:

```text
ymp-docs/tasks/
  README.md
  manage.py
  records/
    W1-0001.json
    W1-0002.json
    W2-0001.json
  tests/
```

All records live directly in `records/`. A development wave groups tasks toward a
checkable iteration outcome; its number is the `W1`, `W2`, or later prefix of each
task ID. The sequence starts at `0001` separately in each wave and grows beyond four
digits when needed. The wave is derived from the ID, not duplicated in a stored
field or directory. Area remains task metadata used for filtering.

Paths and IDs remain stable when work starts or finishes. Changing an ID's wave is
not supported because dependencies and evidence refer to that ID. A wave does not
implicitly block later waves: scheduling uses explicit dependencies, including
dependencies between waves. There is no combined backlog file or copied task
history. The roadmap describes outcomes and links to records as tasks are defined;
records are the only source of task status. Examples here are illustrative, not
existing tasks. An empty `records/` is valid; `.gitkeep` preserves it in Git.

Each record contains schema version, ID, area, title, type, priority, status,
dependencies, current owner, goal, scope, acceptance criteria, evidence, a positive
revision and timestamped history. Store history inside that task's record so one
atomic replacement commits the status and its note together. Display history only
when requested. Whole-record reads by the tool do not require the agent to load
the whole backlog into its context.

Use Python 3.11+ and the standard library. Record files and directories must stay
inside the configured task root; reject unsafe IDs, traversal, symlinked task
paths, nested directories and a record whose ID differs from its filename. Reject
creation of an existing ID even if its area differs. The default
root is the directory containing `manage.py`; `--root` selects an isolated task
root for checks or another checkout. No application-owned data is accessed.

## States and readiness

States are `new`, `planned`, `in_progress`, `owner_question`, `paused`, `done` and
`rejected`. Readiness is derived separately: a planned task is ready only when
every dependency is done. An unscheduled `new` task is not ready. Unknown or
rejected dependencies do not satisfy readiness. Detect unknown references,
self-dependencies and cycles, including graphs deeper than Python's recursion
limit. A task cannot start or complete with unfinished prerequisites.

`owner_question` and `paused` require a meaningful note; `done` requires at least
one evidence reference. Evidence references support review; their presence is not
proof that the work is correct. Completing research or documentation does not
count as implemented product functionality.

## Review, integration and completion

An independent `ACCEPT` verdict means that a result is ready to integrate; it does
not complete the development task. Keep the task `in_progress` until the accepted
result is committed to `main`. Set `done` only after verifying that the result's
commit is reachable from `main`, and record that commit as completion evidence. The
completion update and its evidence must then also be committed to `main`; until both
the result and the canonical completion record are present there, completion is not
recorded for the repository.

If a result is rejected, actually remove the rejected changes and task-owned
isolated work materials, then commit the rejection record to `main`. Limit cleanup
to files, branches, worktrees and other resources owned exclusively by that task;
do not discard or rewrite another task's or contributor's work. A review verdict or
a `rejected` status alone does not establish that cleanup occurred.

## Commands and bounded reading

- `next` returns one ready task by default, ordered by numeric priority, wave and
  task sequence. It supports wave and area filters and only recommends work; it
  does not claim or execute it.
- `list` supports wave, area, status and readiness filters. Its default excludes terminal
  tasks, shows at most 20 summaries and never includes full descriptions/history.
- `show ID` displays one current task, its acceptance and immediate dependency
  states, without its history. Bound the default output and give an explicit way
  to continue if the task itself is long; never silently hide remaining content.
- `deps ID` and `history ID` provide paginated detail. Dependency output contains
  summaries, not the bodies of all prerequisite tasks.
- `summary` groups counts by wave, area and task type. `render` emits a bounded Markdown
  overview to stdout, and `progress --write` regenerates the human-readable
  `PROGRESS.md` tables (write commands do this automatically); no generated status
  document is a second source of truth.
- Read commands support bounded limits and offsets, and machine-readable JSON
  where useful. Report total matches and how to retrieve the next page. Cap list,
  dependency and history page sizes at 100; default to 20. A long title or note
  must not silently turn a summary into an unbounded body dump.
- `create --wave W1 --file DRAFT.json` validates a new task and allocates the next ID
  in the specified wave when omitted. An explicit draft ID can supply the wave;
  if `--wave` is also given, it must match. Drafts contain the current task definition,
  not fabricated revision/history data.
- `update ID --file PATCH.json --expect-revision N --note TEXT` changes the task
  definition through the same validated writer. IDs and areas remain immutable.
- `claim ID --owner NAME --expect-revision N` atomically takes a ready planned
  task. A second claim fails, including one made by the same owner.
- `status ID STATE --expect-revision N --note TEXT` records an explicit state
  change; support adding evidence for completion and an actor for attribution.
- `check` validates canonical records, schema, history/revision consistency and
  dependencies. It does not infer completion from prose or launch model calls.

The task guide documents exact supported flags and examples. Invalid input,
missing tasks and stale revisions return readable errors and nonzero exit codes,
not Python tracebacks. Read queries do not modify canonical records.

## Cooperative concurrent updates

Serialize writers with an OS-managed lock in the task root on macOS/Linux. After
acquiring the lock, reread the records and validate the expected revision and
readiness before writing. Use a unique temporary file in the destination directory
and atomic replacement; a rejected update leaves the record and history unchanged.
Do not unlink the shared lock inode after each operation.

Every successful mutation advances the task revision and records its actor, time,
status and note. Claim ownership prevents accidental duplicate assignment but is
not an authentication or authorization system. Raw editor writes do not participate
in the locking protocol; use the update command for coordinated changes.

The lock coordinates one shared task root. Separate clones or worktrees have
separate record contents; they are not a distributed task service. Claim tasks in
the canonical checkout before creating implementation worktrees, and integrate
their status updates through that checkout. Git remains the source history.

## Acceptance

1. One task can be created, inspected, claimed, updated and completed with evidence
   through the actual CLI; each operation updates only that task file.
2. Invalid definitions, unknown/cyclic dependencies, premature start/completion,
   missing evidence and stale revisions are rejected without changing records.
3. Two processes claiming the same revision produce exactly one successful claim.
4. At least 1,500 synthetic tasks, including a dependency chain longer than 1,000,
   validate without recursion failure. `next`/filtered lists do not emit task bodies
   or history; default output remains bounded and pagination has no missing or
   duplicated records on an unchanged dataset.
5. A long single task or history page reports truncation/pagination explicitly.
6. A fresh iteration may start with no tasks. Wave-local allocation, numeric wave
   ordering, cross-wave dependencies and flat record paths pass through the actual
   CLI. Examples do not create records or claim delivered outcomes.
7. Agent instructions point to `next` then `show`, rather than asking the agent to
   read every task. The roadmap owns no duplicate manually maintained statuses.

The implementation may scan record files internally. This foundation promises
bounded agent-facing context, not a sublinear database index or distributed claims.
No native provider execution or product implementation beyond this development
workflow is part of the outcome.

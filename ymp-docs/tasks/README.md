# Development task register

This directory contains the repository's canonical development task records. It is
separate from the product's runtime `Task` model. A claim coordinates contributors;
it does not authenticate an owner, start an agent, or grant product execution
authority.

Use Python 3.11 or newer. `manage.py` uses only the standard library and does not
access application data, providers, models, credentials, or the network.

## Start and finish work

Read only the bounded queue and the selected record before claiming it:

```sh
python3 ymp-docs/tasks/manage.py next
python3 ymp-docs/tasks/manage.py show DEV-0004
python3 ymp-docs/tasks/manage.py claim DEV-0004 --owner maintainer --expect-revision 1
python3 ymp-docs/tasks/manage.py show DEV-0004
```

Use `update` for definition changes. Use `status` with the revision returned by the
previous write to record progress:

```sh
python3 ymp-docs/tasks/manage.py update DEV-0004 --file dev-0004-patch.json \
  --expect-revision 2 --note "Clarified the recovery acceptance cases."
```

Starting work uses `claim`; a task can reach `done` only from `in_progress`.
Repeating the current status with a new note records progress without changing its
meaning.

An `ACCEPT` review verdict means that the result is ready to integrate, not that the
task is complete. Keep the record `in_progress` while integrating the accepted
result. From the canonical checkout, first verify that the result commit is in
`main`; only then record completion with that commit as evidence:

```sh
git merge-base --is-ancestor <result-sha> main
python3 ymp-docs/tasks/manage.py status DEV-0004 done --expect-revision 3 \
  --actor maintainer --note "The accepted result is committed to main." \
  --evidence "Commit <result-sha> in main"
```

Commit the resulting task-record update to `main` as well. The repository does not
record completion until both the accepted result and the `done` record with its
evidence are present in `main`. `manage.py` validates the record and evidence field;
it does not inspect Git or prove that a referenced commit is reachable from `main`.

For a rejected result, first remove the rejected changes and isolated work materials
owned exclusively by that task, without changing another task's files, commits,
branches or worktrees. Then record `rejected` with a note describing the scoped
cleanup and commit that record to `main`. A review verdict or status change without
actual cleanup is insufficient.

Do not run future tasks merely because they exist. `new` means unscheduled. A
`planned` task is ready only when every dependency is `done`; `next` only recommends
such tasks and never claims or executes them.

## Read commands

Global `--root PATH` precedes the command and selects a task root containing
`records/`. It is intended for an isolated checkout or test data; the default is
this directory.

| Command | Purpose and principal flags |
| --- | --- |
| `next` | Ready planned tasks ordered by numeric priority then ID. Supports repeatable `--area`, `--limit`, `--offset`, and `--json`. |
| `list` | Summaries filtered by repeatable `--area`, `--status`, or `--readiness`. Terminal tasks are excluded unless `--all` or an explicit status is supplied. |
| `show ID` | Current task, acceptance, and immediate dependency states, but no history. `--limit` and `--offset` paginate characters. |
| `deps ID` | Transitive dependency summaries ordered by distance and ID. |
| `history ID` | History entries, newest first. |
| `summary` | Status counts grouped separately by area and task type. |
| `render` | A bounded Markdown overview on standard output. It does not write an index or cache. |
| `check` | Validate schemas, stable paths, revision/history consistency, and the complete dependency graph. |

`list`, `deps`, and `history` default to 20 results and accept at most 100.
`next` defaults to one result and also accepts at most 100. `summary` and `render`
use the same 20/100 bounds for groups or task summaries. Each page reports its total
and next offset. `list`, `next`, `deps`, `history`, `summary`, `show`, and `check`
support JSON where it is useful for automation.

`show` defaults to 12,000 characters and accepts at most 50,000. Its header and JSON
page metadata report the total character count and next offset, so a long single
task is never silently omitted. Summary titles and owners are shortened to a fixed
display size, and history notes are shortened to 1,000 display characters. These
display limits do not alter canonical records.

Read commands scan the JSON files but do not create the writer lock or modify
records. Examples:

```sh
python3 ymp-docs/tasks/manage.py list --area persistence --status planned --json
python3 ymp-docs/tasks/manage.py list --readiness unscheduled --limit 20
python3 ymp-docs/tasks/manage.py deps DEV-0008 --limit 20 --offset 0
python3 ymp-docs/tasks/manage.py history DEV-0003 --limit 20 --offset 0
python3 ymp-docs/tasks/manage.py summary
python3 ymp-docs/tasks/manage.py render --limit 20
python3 ymp-docs/tasks/manage.py check
```

## Write commands and files

`create --file DRAFT.json` accepts current definition fields and allocates the next
global ID when `id` is absent. A draft must not provide `revision` or `history`;
the writer creates revision 1 and its first history entry. These definition fields
are accepted:

```json
{
  "schema_version": 1,
  "id": "DEV-0010",
  "area": "persistence",
  "title": "Define a recovery scenario",
  "type": "design",
  "priority": 100,
  "status": "new",
  "depends_on": ["DEV-0004"],
  "owner": null,
  "goal": "State the recovery outcome.",
  "scope": ["Bound the scenario."],
  "acceptance": ["The scenario has observable outcomes."],
  "evidence": [],
  "context": ["This is unscheduled future work."]
}
```

The ID, schema, and area remain immutable. `update ID --file PATCH.json` accepts a
nonempty subset of `title`, `type`, `priority`, `depends_on`, `goal`, `scope`,
`acceptance`, `evidence`, and `context`. Status and ownership changes use their
dedicated commands. All updates require `--expect-revision`; stale writes fail
without changing the record.

Each canonical record adds a positive `revision` and an in-record `history` array.
Every history entry contains the resulting revision, UTC timestamp, actor, status,
and note. The history length equals the revision, revisions are contiguous from one,
and the latest history status equals the current status. A `done` task requires at
least one evidence reference. Evidence references aid review; their presence alone
does not prove correctness.

Task IDs have the canonical form `DEV-0001` and continue as `DEV-10000` and above.
Areas and types are safe lowercase slugs of at most 64 characters. Records stay at
`records/<area>/<ID>.json`; status changes never move them. Traversal, symlinked
record paths, path/record disagreement, duplicate IDs, unknown dependencies,
self-dependencies, and dependency cycles are rejected.

## Concurrency and limits

Writers use the persistent `.manage.lock` inode and an operating-system `fcntl`
lock. After acquiring it, a writer reloads all records, checks the expected revision
and readiness, validates the proposed register, writes a unique temporary file in
the destination directory, and atomically replaces one task record. Two processes
claiming the same revision therefore have one winner, including repeated claims by
the same owner.

The lock coordinates writers using one shared task root on macOS or Linux. It does
not coordinate separate clones, worktrees, machines, or editors that bypass the
tool. Claim in the canonical checkout before creating a worktree, and integrate
task updates through that checkout. Git remains the repository history. Owner and
actor strings are cooperative labels, not authentication or permissions.

Command exit codes are stable:

- `0`: success;
- `2`: command-line syntax error reported by `argparse`;
- `3`: invalid input, record, dependency graph, state transition, path, or missing task;
- `4`: stale revision or claim conflict.

Unexpected operating-system errors are reported without a Python traceback and use
code `3`; no command installs dependencies, publishes artifacts, or starts agents.

# Executable foundation contract

This contract began as the first implementation outcome and now owns the current
executable command scope. YMP validates tasks, records a trusted lifecycle, can
persist one admitted native invocation, and can replay that history after a
restart. An observed completed invocation does not establish task acceptance,
confirmation or a checked result.

## Domain values

Implement `TaskId`, `SessionId` and `CriterionId` as distinct types with private
storage. Identifiers and required text must be nonblank. Preserve valid goal and
criterion text; validation must not silently replace the user's content.

`Goal` contains the request. `Criterion` has an identifier and description.
`AcceptanceContract` contains a nonempty sequence of criteria with unique IDs.
`Constraints` retains a sequence of user-stated conditions as text. Each supplied
condition must be nonblank and is preserved exactly. An empty sequence means no
conditions were supplied; it does not authorize unrestricted execution. The
foundation validates and retains these conditions but does not interpret them as
executable resource or permission rules. That interpretation and enforcement belong
to the later admission outcome.

`Task` combines its ID, goal, acceptance contract and constraints. These values
perform no I/O and require no machine-specific configuration.

## Journal contract

`Revision::INITIAL` represents an empty stream. `JournalEntry` contains its stream
revision and a `SessionEvent`. Session lifecycle events and the execution events
defined by the [bounded native execution contract](bounded-execution-contract.md)
are implemented.

The `Journal` port provides coherent history reads and atomic append against an
expected revision. Each accepted event advances the revision by one. An empty
batch, stale revision, revision overflow or adapter failure returns a typed error
and leaves the stream unchanged. Unknown sessions have empty histories at the
journal boundary; the kernel distinguishes that from an existing session.

`MemoryJournal` stores records only in process memory; its clones share the same
store and atomic revision checks. `SqliteJournal` stores the same event streams in
`<data-dir>/journal.db`, validates its application and schema identifiers on open,
and uses SQLite WAL mode with full synchronization. Opening the durable adapter
does not read provider credentials or perform model discovery.

## Kernel lifecycle

`Dispatcher` receives a `Journal` implementation by injection. It exposes:

- Open a session for a validated task and caller-supplied session ID.
- Read its `SessionView` by replaying the journal.
- Cancel an open session against an expected revision.

Opening an existing ID fails without appending another event. Cancellation of an
unknown session, a stale view or an already cancelled session fails without
mutation. `Dispatcher` cancellation changes session metadata only; invocation
cancellation uses the execution orchestration and requires a termination
observation before it records `cancelled`. Library callers supply identifiers; the
`run` command generates identifiers for its one task and session.

A valid history begins with exactly one `SessionOpened`. Revisions are contiguous
from one. `SessionCancelled` can follow an open session once. Invalid event order,
duplicate opening, revision gaps and events after cancellation are rejected by
projection. An invalid history must not be exposed as a valid `SessionView`.

The view contains the session identity, task, status and revision. Returned values
are immutable snapshots; a later append does not rewrite a previously returned
view. Successful commands return the state at their committed revision.

## Command-line entry point

The binary is `ymp`. No arguments and `--help` print usage, the current capability
limit, the default storage location and its lifetime. `--version` prints the
application name and package version. Unsupported or incomplete arguments produce
a readable error and exit status 2; operational failures and observed outcomes
other than `completed` use exit status 1.

`ymp run "<task>" [--data-dir PATH]` constructs one `Task`: the argument is the
exact `Goal`, the sole criterion is `the agent completed the requested work`, and
`Constraints` contains no supplied conditions. It opens a durable session through
`Application<SqliteJournal>`, explicitly scans `CodexRegistry`, and admits one
assignment for the current working directory. If the data directory resolves
inside that workspace, including at the default location, the assignment and
Codex sandbox are read-only so the invocation cannot alter the authoritative
journal. A data directory outside the workspace permits read and write access.
The CLI clears additional writable roots, excludes `/tmp` and `$TMPDIR` from
workspace-write roots, disables approval prompts and requests an ephemeral Codex
run so Codex does not retain a separate rollout file. It then starts
`CodexBackend`, observes a termination and settles reported usage. The default
allowance is 16 observed turns, 200,000 accepted output characters and 30 minutes
of wall-clock time. The command requests cancellation five seconds before that
deadline. Without observed termination, the invocation remains `uncertain`; the
CLI never reports it as cancelled or timed out by inference.

`ymp show <session-id> [--data-dir PATH]` opens an existing durable journal and
prints the session state and every persisted invocation projection. It does not
create a missing data directory or journal. The default data directory is
`./.ymp/sessions`, relative to the working directory. Its `journal.db` and any
SQLite `journal.db-wal`/`journal.db-shm` side files belong together. History
persists across processes until the user deletes the complete data directory; the
files must not be copied or removed individually while YMP is running.

The command inherits native Codex authentication and does not read or copy
credentials. Its report contains only journaled outcome and usage observations,
the working directory, generated identifiers and the explicit statement that
acceptance was not evaluated. Interactive control and checked task delivery remain
separate outcomes.

## Acceptance evidence

1. Constructors reject blank identifiers, requests, criterion descriptions and condition strings. Acceptance contracts reject empty criterion lists and duplicate criterion IDs. An empty constraints sequence is represented explicitly and is valid.
2. Open/read/cancel passes through `Dispatcher` and `MemoryJournal`, preserving the exact goal, criteria, constraints and revisions.
3. Duplicate IDs and stale cancellation fail without partial history changes.
4. Two concurrent appends at the same expected revision cannot both succeed.
5. Kernel projection rejects malformed history from a deliberately different journal implementation.
6. Actual CLI help/version succeed; unsupported or incomplete commands fail
   honestly without creating application state.
7. Scripted CLI workflow tests cover completed, failed and not-ready outcomes; a
   SQLite reopen test confirms that `show` reads the persisted invocation history.
8. A real CLI invocation exists only as an ignored, explicitly selected test.
9. Workspace build, formatting, Clippy and tests succeed without fetching dependencies.

These checks establish the listed lifecycle, durable replay and one-invocation
command behavior. They establish neither crash recovery of an in-flight provider,
independent acceptance of generated artifacts, useful cooperation nor performance
improvement. Those require their own consumers and evidence when implemented.

# Executable foundation contract

This contract defines the first implementation outcome. It establishes task
validation, journal semantics and a small trusted lifecycle. It does not execute
agents or solve a task.

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
revision and a `SessionEvent`. The implemented events are `SessionOpened` and
`SessionCancelled`.

The `Journal` port provides coherent history reads and atomic append against an
expected revision. Each accepted event advances the revision by one. An empty
batch, stale revision, revision overflow or adapter failure returns a typed error
and leaves the stream unchanged. Unknown sessions have empty histories at the
journal boundary; the kernel distinguishes that from an existing session.

The first adapter, `MemoryJournal`, stores records only in process memory. Clones
share the same store and atomic revision checks. No directory, credential file,
database or model discovery is accessed.

## Kernel lifecycle

`Dispatcher` receives a `Journal` implementation by injection. It exposes:

- Open a session for a validated task and caller-supplied session ID.
- Read its `SessionView` by replaying the journal.
- Cancel an open session against an expected revision.

Opening an existing ID fails without appending another event. Cancellation of an
unknown session, a stale view or an already cancelled session fails without
mutation. Cancellation here changes metadata only: native work does not exist in
this foundation. The caller supplies IDs; automatic ID generation is not implied.

A valid history begins with exactly one `SessionOpened`. Revisions are contiguous
from one. `SessionCancelled` can follow an open session once. Invalid event order,
duplicate opening, revision gaps and events after cancellation are rejected by
projection. An invalid history must not be exposed as a valid `SessionView`.

The view contains the session identity, task, status and revision. Returned values
are immutable snapshots; a later append does not rewrite a previously returned
view. Successful commands return the state at their committed revision.

## Command-line entry point

The binary is `ymp`. No arguments and `--help` print concise usage and the current
capability limit. `--version` prints the application name and package version.
Unsupported or additional arguments produce a readable error and nonzero exit
status. The entry point uses the runtime's application metadata.

The CLI must not pretend to run a task. It performs no native discovery, inference,
filesystem mutation or network access. Agent execution and a terminal UI are
separate roadmap outcomes.

## Acceptance evidence

1. Constructors reject blank identifiers, requests, criterion descriptions and condition strings. Acceptance contracts reject empty criterion lists and duplicate criterion IDs. An empty constraints sequence is represented explicitly and is valid.
2. Open/read/cancel passes through `Dispatcher` and `MemoryJournal`, preserving the exact goal, criteria, constraints and revisions.
3. Duplicate IDs and stale cancellation fail without partial history changes.
4. Two concurrent appends at the same expected revision cannot both succeed.
5. Kernel projection rejects malformed history from a deliberately different journal implementation.
6. Actual CLI help/version succeed; an unsupported command fails honestly.
7. Workspace build, formatting, Clippy and tests succeed without fetching dependencies.

These checks establish the listed foundation behavior. They establish neither
durable recovery, provider control, independent acceptance of generated artifacts,
useful cooperation nor performance improvement. Those require their own consumers
and evidence when implemented.

# Durable Journal contract

Status: historical contract of the previous iteration. The code it describes was
removed from the working tree and is reachable only at git tag `legacy-foundation`;
it is not delivered scope or a baseline for the new implementation.

This contract records the accepted durable `Journal` adapter, implemented as `SqliteJournal` in
`crates/ymp-storage` and verified by that crate's acceptance tests through the kernel `Dispatcher`.
It fixes the port semantics, the typed failure mapping and the indeterminate-commit resolution rule;
later adapter changes amend this document rather than reinterpret it.

## Scope and relationship

The durable adapter is one `Journal` implementation injected into the same `Dispatcher` as
`MemoryJournal`, which remains valid for tests and in-memory use. The kernel port (`Journal`,
`JournalError`, history projection) is unchanged. Domain admission — valid event order, exactly one
`SessionOpened`, no events after cancellation — and replay stay with the kernel; the adapter stores
and retrieves events and never decides which histories are valid. Unknown sessions read as empty
histories at the journal boundary; the kernel distinguishes them from existing sessions.

## Typed failures and SQLite mapping

`JournalError` has seven variants. `EmptyBatch`, `StaleRevision { expected, actual }`,
`RevisionOverflow` and `AdapterFailure { message }` are the kernel's originals; the durable work
added `Corruption { message }`, `UnsupportedFormat { version }` and `IndeterminateCommit { expected,
attempted, message }`. Failures are matched structurally, never through message text. `attempted`
is the last revision of the submitted batch, not an acknowledgment of writing.

| Condition | Failure |
| --- | --- |
| Empty batch, batch over 256 events, payload or string over 2^20 bytes, busy timeout, poisoned lock, refused WAL mode, ordinary I/O error — any append failure before the commit attempt whose rollback is executed and confirmed | `AdapterFailure` |
| Expected revision differs from the durable head read inside the append transaction | `StaleRevision`, with the durable head as `actual` |
| The batch would pass `u64::MAX` under `Revision::checked_next` | `RevisionOverflow` |
| A commit attempt whose outcome is not proven, or a rollback that cannot be confirmed | `IndeterminateCommit` |
| SQLite `DatabaseCorrupt` or `NotADatabase` result, `InvalidColumnType` on a stored row, checksum mismatch, malformed payload, revision discontinuity, head disagreement, out-of-range stored columns, foreign application identifier, incomplete schema, failed `quick_check` | `Corruption` |
| A database schema version, or a stored payload version, this build does not support | `UnsupportedFormat`, naming the version |

`InvalidColumnType` maps to `Corruption`: a stored value whose storage class contradicts the
journal schema is altered storage, not an adapter malfunction. Decisions that need no storage
access — empty batch, batch and payload limits, revision overflow — are taken before anything is
written.

## Indeterminate-commit resolution

After an `IndeterminateCommit`, the caller reads the durable stream and compares the exact revision
range `expected+1..=attempted` and the event values with the submitted batch:

- A stored range that fully matches the batch is committed and is not resubmitted.
- The same batch may be retried only while the history still ends at the original `expected`; the
  retry uses that `expected` and never raises it to another writer's head.
- Any other advancement of the history is an explicit `StaleRevision`.

Exactly-once is not promised by caller identity; the rule decides by comparing stored content with
the submitted batch. A plain read is not a durability acknowledgment: only the observed durable
state establishes what happened, and only a returned commit under `synchronous=FULL` acknowledges
durability.

## Storage decisions

- Embedded SQLite through `rusqlite` with the bundled build: one executable, no installed database
  library or server. SQLite was selected over a custom file protocol because no demonstrated
  advantage justified replacing a proven transactional mechanism.
- One database file, `<root>/journal.db`, plus the side files SQLite itself manages (`-wal`,
  `-shm`, a transient `-journal`), with two owned tables (`journal_streams`, `journal_entries`)
  and enforced foreign keys.
- Identity: header application identifier `0x594D504A` (`YMPJ`) and the schema version in
  `user_version`; this build writes and reads version 1.
- An existing database is validated before any journal-mode change or DDL. An unsupported schema
  version fails `UnsupportedFormat` and is never rewritten, migrated or repaired; a fresh empty
  database is initialized as version 1 in one transaction; a foreign database fails `Corruption`.
  `PRAGMA quick_check` runs on open.
- The connection runs WAL, `synchronous=FULL` and a five-second busy timeout.
- A `u64` revision is two nonnegative 32-bit columns, `revision_hi` and `revision_lo`, composed and
  split with checked arithmetic; the full range survives without narrowing through `i64`,
  out-of-range stored columns are `Corruption`, and the stream head uses the same representation.
- An append is one `BEGIN IMMEDIATE` transaction: read the head, compare `expected`, write the
  whole batch as contiguous revisions in submitted order, set the new head, commit. A batch appears
  completely or not at all.
- A read is one deferred transaction — a stable WAL snapshot — that validates, in order: payload
  bounds, payload version, content checksum, strict payload decoding, revision continuity from
  one, and head agreement. The `payload_version` column is deliberately not CHECK-pinned so an
  unknown stored version stays readable and reportable as `UnsupportedFormat`.
- Fault injection exists only behind the non-default `fault-injection` feature: fail the next
  append after the rows are written but before the commit (confirmed rollback, `AdapterFailure`),
  or report an unproven commit with the batch actually committed or actually absent (both
  `IndeterminateCommit`); it fires once, never affects reads, and is absent from default and
  downstream builds.

## Payload encoding and checksum

The payload of one entry is UTF-8 JSON in one of two fixed forms, handled by a private strict DTO
codec:

```json
{"type":"session_opened","session_id":S,"task":{"id":T,"goal":{"request":G},"acceptance_contract":{"criteria":[{"id":C,"description":D}]},"constraints":{"conditions":[K]}}}
{"type":"session_cancelled","session_id":S}
```

Every field is required for its type. Unknown fields, duplicate fields, an unknown `type`, and a
`task` on `session_cancelled` are rejected; values are strings where the form requires strings.
Key order is insignificant on decode; the encoder produces one byte-stable form. Strings are
restored exactly, with no normalization, and domain constructors revalidate every restored value.
Limit violations are honest about direction: an oversized submitted batch or payload fails
`AdapterFailure` before storage; an oversized stored payload or string fails `Corruption`.

Every row carries a CRC-32 checksum (ISO-HDLC, computed with `crc32fast`) over the payload
version, the session identity, the full revision and the exact serialized payload bytes, each
32-bit part as a little-endian word and the session identity length-prefixed:

```text
checksum = CRC-32( le32(payload_version) ‖ le32(revision_hi) ‖ le32(revision_lo)
                   ‖ le32(len(session_id UTF-8)) ‖ session_id UTF-8 ‖ payload )
```

The checksum is recomputed on every read; any mismatch is `Corruption`, and no row is returned as
valid without a matching checksum, because structural checks alone do not detect every
altered-but-still-valid JSON value.

## Location, ownership, lifetime, copying

The journal root is selected explicitly by the caller; the adapter creates the root if missing and
writes nothing outside it. The files are plain documented files owned by the user running `ymp`;
there are no hidden application-owned files. They record the exact user task content — goal,
acceptance criteria, constraints — and may therefore contain sensitive text; they must be protected
accordingly. The adapter imports no native credentials or authentication state.

Copying, moving or backing up journal data is safe only when the storage is quiescent — every
process using the database is closed — or through SQLite's own coherent backup mechanism, which
remains safe while the database is in use; an arbitrary copy of a live database is not a consistent
backup. Concurrent external mutation or deletion of a live root is unsupported. Lifetime equals
the lifetime of these files: the application deletes nothing on its own and performs no migration,
compaction or vacuum, implicitly or explicitly; deletion is an explicit user action, after which
no hidden copies remain.

## Durability and filesystem assumptions

Each commit under WAL with `synchronous=FULL` flushes to the storage device before it is
acknowledged; an acknowledged append survives process restart. The claim rests on an honest local
filesystem whose `fsync` persists data: network filesystems and storage stacks that acknowledge
writes before they are durable void it. All processes using one database must run on the same
local host. Within one process the adapter serializes appends on its connection; across processes
writers wait on the bounded busy timeout, so of two appends at one expected revision exactly one
commits. Readers in WAL mode observe the old or the new coherent stream, never a mixed one.
Power-loss guarantees beyond this VFS contract are not claimed.

## Verified behavior

Each item names its evidence in `crates/ymp-storage/tests/`; every check runs through the kernel
consumer (`Dispatcher`), with direct row manipulation for damaged stores.

- `restart.rs` — real child processes: a session opened in one process that exits reads back in a
  fresh process with equal identity, exact task content, status and revision; an unknown
  identifier still yields `SessionNotFound`; a cancellation stays visible after another reopen.
- `atomicity.rs` — a multi-event batch lands as contiguous ordered revisions or not at all; no
  read exposes a strict prefix. An injected pre-commit failure returns `AdapterFailure` and the
  prior history reads identical; `EmptyBatch` touches no storage.
- `competing.rs` — two simultaneous appends at one expected revision in two real OS processes:
  exactly one commits; the other receives `StaleRevision` with the winner's revision; the
  winner's batch appears exactly once.
- `indeterminate.rs` — both injected branches: a visible unproven commit is not resubmitted; an
  absent one retries at the same `expected` and lands exactly once; any other advancement of the
  history is `StaleRevision`.
- `corruption.rs` — unknown schema and payload versions, malformed JSON (unknown or duplicate
  fields, unknown type), checksum mismatches including altered-but-valid JSON, revision gaps,
  head disagreement and a non-database file fail with typed failures, without mutation, and are
  never served as empty, shorter or valid histories; other sessions stay readable.
- `revision.rs` — checked high/low arithmetic carries across the 2^32 boundary into the high
  column, values above `i64::MAX` round-trip, ordering follows the column pair rather than
  insertion order, a head at `u64::MAX` reads as the exact value, and an append past `u64::MAX`
  fails `RevisionOverflow` before any write.

## Verified limits

The fault-injection points are a test seam compiled only with the non-default `fault-injection`
feature; the indeterminate branches are exercised through that seam, not through real power loss,
process kills inside SQLite's commit, or VFS-level I/O errors — that behavior is not
experimentally covered. `quick_check` runs at open; damage arising later is caught by the per-row
validations on read, not by continuous scanning. The acceptance evidence was gathered on macOS;
the same tests have not been run on Linux.

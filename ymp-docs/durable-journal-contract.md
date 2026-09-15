# Durable Journal contract

This contract extends the executable foundation's in-memory `Journal` port to
durable storage. It defines the invariants, failure behavior, storage layout and
storage approach a persistent adapter must satisfy. The implementation is
[DEV-0005](tasks/records/persistence/DEV-0005.json); this document starts no
implementation.

## Scope and relationship

The durable adapter is another `Journal` implementation injected into the same
`Dispatcher`. The kernel port (`Journal`, `JournalError`, `replay_session`), its
typed errors and the projection rules are unchanged; a durable adapter may extend
the error enum with new variants but must not weaken existing ones.
`MemoryJournal` remains valid for tests and in-memory use. Unknown sessions read
as empty histories at the journal boundary, and the kernel keeps distinguishing
them from existing sessions. The adapter's error additions are exactly the
dedicated, machine-matchable variants fixed in the next section — never
message-text-only distinctions.

## JournalError shape and failure mapping

The adapter's public `JournalError` shape is fixed. The kernel's existing
variants are retained unchanged: `EmptyBatch`,
`StaleRevision { expected: Revision, actual: Revision }`, `RevisionOverflow`
and `AdapterFailure { message: String }`. Exactly three variants are added,
and no others exist:

| Variant | Fields | Raised for |
| --- | --- | --- |
| `Corruption` | `message: String` | A corrupted or foreign database, or stored bytes that fail the adapter's own validation. |
| `UnsupportedFormat` | `version: u16` | An existing database, or a stored payload, carrying a version this build does not support. |
| `IndeterminateCommit` | `expected: Revision`, `attempted: Revision`, `message: String` | An append whose commit outcome is not proven. `attempted` is the last revision of the submitted batch, not an acknowledgment of writing. |

Failure conditions map to variants exactly, with no overlap. The mapping names
the actual SQLite operations:

| Condition | Failure |
| --- | --- |
| SQLite database-corrupt or not-a-database result, a failed structural check, a database that is not a journal of this application (foreign application identifier or schema), a payload checksum mismatch, a malformed payload, a revision discontinuity, or head disagreement | `Corruption` |
| An existing database whose schema version is not a supported schema version, or a stored entry whose payload version is unknown | `UnsupportedFormat` |
| An empty batch, an input-limit violation, a busy timeout, or an ordinary I/O error — any append failure before the commit attempt whose rollback is confirmed | `AdapterFailure` |
| A commit attempt whose outcome is not proven absent, or a rollback that cannot be confirmed | `IndeterminateCommit` |
| The expected revision does not equal the durable stream head | `StaleRevision` |
| The batch would push the revision past `u64::MAX` | `RevisionOverflow` |

## Durability and ordering invariants

Each session owns one append-only stream. Committed entries are never rewritten
in place; revisions are contiguous from one within a stream, and
`Revision::INITIAL` denotes an empty stream. An append compares the expected
revision against the durable head and extends the stream as one atomic step: a
batch either appears completely, as contiguous revisions in submitted order, or
not at all. Success is returned only after the transaction's `COMMIT` has
returned under `synchronous=FULL`; an acknowledged batch survives process
restart under the competing-writers and VFS assumptions below.

| Failure | Caller observation |
| --- | --- |
| `EmptyBatch` | Rejected before any storage access; no byte written. |
| `StaleRevision` | `actual` is the durable head; nothing appended; a read shows the winner's full history. |
| `RevisionOverflow` | Detected before any storage access; nothing appended. |
| `AdapterFailure` | Busy timeout, input limit or ordinary I/O failure before the commit attempt, with the rollback confirmed; the durable stream is unchanged. |
| `IndeterminateCommit` | The commit outcome is not proven; the batch may or may not be committed. The stream is not promised unchanged; resolution follows the indeterminate-commit resolution rule below. |

## Indeterminate-commit resolution

After an `IndeterminateCommit`, the caller reads the durable stream and
compares the exact revision range `expected+1..attempted` and the event values
with the submitted batch:

- A stored range that fully matches the submitted batch is committed and is
  not resubmitted.
- A retry of the same batch is allowed only if the history still ends at the
  original `expected` and the operation is still admissible. The retry uses
  the same `expected`; it never raises it to a foreign revision.
- Any other advancement of the history is an explicit `StaleRevision`.

Exactly-once is not promised by caller identity; the rule decides by
comparing the stored content with the submitted batch. A plain read does not
turn a sync error into confirmed durability: only the observed durable state
establishes what happened.

## Storage approach

DEV-0005 implements an embedded SQLite database through `rusqlite` with the
`bundled` SQLite build, so the application remains a single executable and
requires no installed database library or server. The adapter owns a narrow
journal schema inside that database; it reuses no other application's schema,
domain model or migration policy.

The durable store of a caller-selected root is exactly one database file,
`<root>/journal.db`, plus the SQLite side files SQLite itself creates and
manages there (`-wal`, `-shm`, and a transient `-journal` during checkpointing
edge cases). The adapter writes nothing outside the root.

Connection setup is fixed:

1. Open `<root>/journal.db` with a bounded busy timeout.
2. Validate an existing database's application identifier and schema version
   before changing the journal mode or executing any DDL; never rewrite,
   migrate or repair an unsupported schema — fail with `UnsupportedFormat`
   naming the version, leaving the database unchanged.
3. A new or empty database is initialized as schema version 1 in one
   transaction.
4. Only then set `journal_mode=WAL` and `synchronous=FULL` for the
   connection.
5. Run a structural check (`PRAGMA quick_check`) on open; failure is
   `Corruption`.

These settings do not replace application validation. Durability rests on the
VFS honoring the sync operations SQLite documents for WAL with
`synchronous=FULL`; all processes using one database must be on the same local
host, network filesystems are out of scope, and power-loss guarantees beyond
the VFS contract are not claimed.

## Schema and versioning

The schema is fixed as version 1 and identified by the database header's
application identifier (the journal's own constant) and schema version stored
in `user_version`. A database whose application identifier is absent but which
contains no user schema is an empty database and is initialized; a database
with a foreign application identifier or a non-journal schema fails as
`Corruption`; a journal application identifier with an unknown `user_version`
fails as `UnsupportedFormat`.

Two tables exist and no others are owned:

```sql
CREATE TABLE journal_streams (
    session_id TEXT PRIMARY KEY,
    head_hi    INTEGER NOT NULL CHECK (head_hi BETWEEN 0 AND 4294967295),
    head_lo    INTEGER NOT NULL CHECK (head_lo BETWEEN 0 AND 4294967295)
);

CREATE TABLE journal_entries (
    session_id      TEXT NOT NULL REFERENCES journal_streams (session_id),
    revision_hi     INTEGER NOT NULL CHECK (revision_hi BETWEEN 0 AND 4294967295),
    revision_lo     INTEGER NOT NULL CHECK (revision_lo BETWEEN 0 AND 4294967295),
    payload_version INTEGER NOT NULL CHECK (payload_version = 1),
    payload         BLOB NOT NULL,
    checksum        INTEGER NOT NULL CHECK (checksum BETWEEN 0 AND 4294967295),
    PRIMARY KEY (session_id, revision_hi, revision_lo)
);
```

A `u64` revision is stored as two nonnegative 32-bit columns, `revision_hi`
and `revision_lo`; the composite `(revision_hi, revision_lo)` preserves the
full `u64` range and its ordering. The adapter never casts a whole revision to
`i64` and never narrows above `i64::MAX`: columns are composed in Rust as
`(u64::from(hi) << 32) | u64::from(lo)` and decomposed inversely, and all
arithmetic goes through `Revision::checked_next` before anything is written.
The stream head is stored in the same representation. Bound violations in
stored columns are `Corruption`.

## Append protocol

An append performs exactly this sequence:

1. Reject an empty batch before any storage access.
2. Compute the batch's revision range with `checked_next` (`RevisionOverflow`
   before any storage access) and encode every event with the strict payload
   codec; an input-limit violation fails as `AdapterFailure` before any
   storage access.
3. Begin one `BEGIN IMMEDIATE` transaction, acquiring the write lock.
4. Read the session's stream head; an absent stream reads as
   `Revision::INITIAL`.
5. Compare `expected` with the head; on mismatch, roll back (confirmed) and
   fail `StaleRevision` with the durable head as `actual`.
6. Insert the whole batch as contiguous revisions in submitted order and set
   the stream head to the batch's last revision, all inside the same
   transaction.
7. Commit. A failure before the commit attempt with a confirmed rollback is
   `AdapterFailure`. A commit attempt whose outcome is not proven absent — or
   a rollback that cannot be confirmed — is `IndeterminateCommit` carrying
   `expected` and `attempted`, resolved by the indeterminate-commit resolution
   rule.

## Read protocol

A read takes one coherent read transaction — in WAL mode it observes a stable
snapshot — and returns the head and all entries of one stream ordered by
`(revision_hi, revision_lo)`. Before returning a history the adapter validates,
in order: the schema and payload bounds; the payload version; the content
checksum; strict payload decoding; revision continuity from one; and agreement
of the stream head with the last entry. An absent stream is an empty history
at the journal boundary. Any violation is a typed failure — `Corruption` or
`UnsupportedFormat` — and is never presented as an empty, shorter or valid
history; kernel projection through `replay_session` remains the final
authority on domain admission and replay order.

## Payload encoding

The payload of one entry is UTF-8 JSON in one of two fixed forms. Session
opening:

```json
{"type":"session_opened","session_id":S,"task":{"id":T,"goal":{"request":G},"acceptance_contract":{"criteria":[{"id":C,"description":D}]},"constraints":{"conditions":[K]}}}
```

Session cancellation:

```json
{"type":"session_cancelled","session_id":S}
```

Here `S`, `T`, `G`, `C`, `D` and `K` denote strings, and the arrays follow the
current domain constraints. The adapter keeps these forms as a private DTO
codec: every field is required for its type; unknown fields, duplicate fields
and an unknown `type` are rejected; a `task` on `session_cancelled` is
rejected; values must be strings where the form requires strings. Key order is
insignificant on decode; the encoder produces one byte-stable form. Strings
are restored exactly, with no normalization, and domain constructors revalidate
every restored value — a payload whose values fail constructor validation is
malformed and fails as `Corruption`. Each stored row records its payload
version; version 1 is the current version, and an unknown stored payload
version fails as `UnsupportedFormat`.

Limits are fixed: a record payload is at most 2^20 bytes, any single payload
string at most 2^20 bytes, and a batch at most 256 events. A submitted batch
or payload that exceeds a limit is an input-limit failure before any storage
access and fails as `AdapterFailure`; a stored payload or string that exceeds
a limit fails as `Corruption`.

## Content checksum

Every row carries a CRC-32 checksum (ISO-HDLC: reflected polynomial
0xEDB88320, initial value 0xFFFFFFFF, final XOR 0xFFFFFFFF) computed over the
following input, where `le32` is a four-byte little-endian unsigned integer,
` ‖ ` is concatenation, and the session identity is the exact UTF-8 bytes of
the `SessionId`:

```text
checksum = CRC-32( le32(payload_version) ‖ le32(revision_hi) ‖ le32(revision_lo)
                   ‖ le32(len(session_id UTF-8)) ‖ session_id UTF-8 ‖ payload bytes )
```

The checksum covers the payload version, the session identity, the full
revision and the exact serialized payload bytes. Database structural checks
alone do not detect every altered-but-still-valid JSON value, so this
application-level content verification is mandatory on every read: any
mismatch is `Corruption`, and no row is returned as valid without a recomputed
matching checksum.

## Corruption handling

Detection combines SQLite's structural validation on open with the adapter's
per-row application checksum and read validation. Damage inside a stream — a
checksum mismatch, an undecodable or malformed payload, a revision
discontinuity, or a head that disagrees with the entries — fails that stream's
reads and appends with the typed `Corruption` variant, matched structurally
rather than through message text. The adapter never truncates a stream, never
serves a shorter history as valid, and never presents a damaged history as an
empty one. Because all streams share one database file, a structurally damaged
database can affect every stream; there is no per-session file isolation. An
unreadable root or database file fails honestly on open.

## Competing writers

SQLite serializes writers: at most one write transaction exists at a time, and
`BEGIN IMMEDIATE` acquires the write lock before the head is read. Within one
process, the adapter serializes appends on its connection; across processes,
the bounded busy timeout makes a contemporaneous writer wait rather than fail
immediately. After serialization, exactly one of two appends at the same
expected revision commits; the loser acquires the lock afterwards, reads the
winner's head, and receives `StaleRevision` with the winner's revision.
Readers in WAL mode take no write lock and observe either the old or the new
coherent stream, never a mixed one.

## Location, ownership, lifetime, copying, migration and deletion

The journal root is selected explicitly by the caller through runtime assembly
or a CLI option; the adapter writes only `journal.db` and SQLite's side files
inside that root and creates nothing elsewhere. There are no hidden
application-owned files. The files are plain documented files owned by the
user running `ymp`. They record the exact user `Task` content — goal,
acceptance criteria and constraints — and may therefore contain sensitive
text; they must be protected accordingly, while the adapter imports no native
credentials or authentication state.

Copying, moving or backing up journal data is safe only when the storage is
quiescent — every process using the database is closed — or through SQLite's
own coherent backup mechanism; an arbitrary copy of a live database is not a
consistent backup and is unsupported. Concurrent external mutation or deletion
of a live root is unsupported. Lifetime equals the lifetime of these files:
the application does not delete journal data on its own, and deletion is an
explicit user action — removing the root — after which no hidden copies
remain. The adapter performs no migration, compaction or vacuum, and never
runs them implicitly; each requires its own explicit future task.

## Test fault injection

The adapter exposes a documented public one-shot fault-injection seam for the
acceptance tests: it can fail the next append after the batch rows are
written but before the commit (confirmed rollback, `AdapterFailure`), or
report an unproven commit with the batch actually committed or actually
absent (both `IndeterminateCommit`). It fires once per arm, never affects
reads, and exists in no other code path.

## DEV-0005 acceptance criteria

Each check runs through the kernel consumer (`Dispatcher`) against the
adapter, using direct row manipulation for damaged stores:

1. Restart replay determinism: a session opened in one real process that then
   exits reads back in a fresh process on the same root with an equal
   `SessionView` — identity, exact task content, status and revision; an
   unknown identifier still yields `SessionNotFound`.
2. Cancellation visibility: a session cancelled in one process is read as
   cancelled in a fresh process after another reopen.
3. Batch atomicity: a multi-event batch appears as contiguous ordered
   revisions or not at all; no read exposes a strict prefix of it. An
   injected pre-commit failure returns `AdapterFailure`, and a subsequent
   read returns the identical prior history with the database unchanged.
   `EmptyBatch` is rejected without touching the filesystem.
4. Indeterminate outcomes: an injected unproven commit returns
   `IndeterminateCommit` carrying `expected` and `attempted`, the last
   revision of the submitted batch. The visible case — the batch is present —
   is not resubmitted; the absent case — the batch is absent — retries at the
   same `expected` and lands exactly once; any other advancement of the
   history is an explicit `StaleRevision`.
5. Competing appends: of two simultaneous appends at the same expected
   revision in two real OS processes on one database, exactly one returns the
   new revision; the other returns `StaleRevision` with the winner's revision;
   the winner's batch appears exactly once.
6. Version honesty: an existing database with an unknown schema version makes
   every operation fail with `UnsupportedFormat`, naming the version, with no
   rewrite, migration or journal-mode change applied; a stored entry with an
   unknown payload version likewise fails with `UnsupportedFormat`.
7. Corrupted content: malformed JSON — including unknown or duplicate fields
   or an unknown `type` — a checksum mismatch — including altered-but-valid
   JSON — and a revision gap fail with `Corruption`; the stream is never
   reported as empty, shorter or valid, and other sessions stay readable.
8. Non-database input: a root whose database file is not an SQLite database
   fails on open with a typed failure and without modifying the file.
9. Full revision range: revisions are stored as two nonnegative 32-bit
   columns; checked high/low arithmetic carries across the 2^32 boundary, the
   full `u64` range round-trips without `i64` narrowing, and an append past
   `u64::MAX` fails with `RevisionOverflow` before any write.
10. Honest storage documentation and gates: delivered user-facing
    documentation states the root location, ownership, lifetime, absence of
    migration and compaction, copy rules and user deletion; the required
    offline build, format, lint and test checks pass for the final code, with
    platform limits disclosed, especially if Linux has not been run.

This contract records durable semantics and one storage selection. It
delivers no persistence, changes no behavior of the current executable, and
remains subject to independent review; its completion in main is required
before DEV-0005's final integration.

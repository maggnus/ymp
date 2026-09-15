# Durable Journal and content

W1-0016 implements the existing Journal contract using embedded SQLite and adds
an immutable ContentStore port. Canonical task status remains in
`tasks/records/W1-0016.json`.

## Storage format and ownership

`SqliteJournal::open(path, schemas)` opens or initializes one explicitly chosen
local database. The SQLite library is bundled through rusqlite; no SQLite CLI or
external service is required. The file has application ID `0x594d504e` and format
version 1. Existing files are inspected read-only before persistent changes.
The complete table definitions, including columns, constraints, foreign keys and
STRICT mode, must match the version. Initialization rechecks the header under an
IMMEDIATE transaction so concurrent first openers do not initialize twice.

The four tables have explicit responsibilities:

- `content_values`: immutable SHA-256-addressed bytes;
- `journal_events`: session, sequence, body digest, previous chain digest and row
  chain digest;
- `journal_heads`: last committed sequence and chain digest per session;
- `event_content`: mandatory content references of each event.

Sequences use eight-byte big-endian blobs, preserving unsigned ordering without
converting through SQLite's signed integer range. Event bodies retain the
kernel's versioned canonical JSON. The chain commits session, sequence, body and
predecessor; it is a corruption check, not an authenticated defense against an
actor able to rewrite the whole database.

## Atomic write and coherent read

Every append opens its own connection and IMMEDIATE transaction, reconstructs the
current session, checks the expected revision and calls the kernel's complete
batch validator. Event bodies, attached policy parameters, mandatory links and
the new head commit together. Failure before commit leaves none of that batch.
The adapter contains no alternative domain acceptance rules.

`Event.contents` explicitly separates attached bytes from required content
addresses. It does not interpret arbitrary domain Ref versions as blob addresses.
Future snapshot/output event owners extend that kernel method; the SQLite adapter
continues to use the same contract. ContentStore.put can write bytes before an
event references them. An interrupted caller may leave an unreferenced immutable
object; it is retained, not treated as a partial event or automatically collected.

Every Journal read uses one transaction for the head, rows, links and bytes.
Reads verify content digests, canonical event identity, sequence continuity,
chain and head agreement, exact required links and absence of orphan links for
the session, then run kernel replay. A corrupt logical session does not prevent
reading an unrelated healthy session; structural SQLite corruption can affect the
whole database. Missing content and corruption are refused without repair.
Content reads always rehash, and putting an existing digest also verifies its
retained bytes instead of overwriting a damaged object.

## Commit resolution

Journal.append retains strict compare-and-append semantics. Journal.resolve_append
is a separate read-only operation over the original expected revision and exact
batch:

- `Committed(end)` requires a valid preceding prefix and byte-identical events at
  every requested sequence, including validated content. Later committed events
  do not invalidate this result.
- `Absent` requires that the valid journal still ends at the original expected
  revision and the proposed append validates there.
- `Conflict` means the journal moved incompatibly. Corruption, missing content,
  unreadable data and exceeded limits remain errors, never Absent.

If SQLite commit returns an error, the write connection is dropped before a new
read connection resolves the result. A committed batch returns its original end;
a verified absence allows a caller-controlled retry at the original revision;
failed inspection remains explicitly indeterminate. Resolution never resubmits.
Retrying an already committed raw append is stale, so it cannot duplicate events.
Content puts are independently idempotent by digest; after an uncertain put the
same bytes can be retried without changing an existing object.

## Bounds and durability assumptions

Stored blob lengths are checked before loading bytes. A blob is limited to 32 MiB,
a session to 32,768 events and 128 MiB of event bodies plus referenced content,
and content-link count is bounded. Limits produce explicit refusals, not truncated
histories. Append checks the resulting session's bounds before commit. Existing
handles use open-without-create, so removing a database cannot silently reset it.

Connections use WAL, synchronous FULL, foreign keys and a five-second busy timeout;
fullfsync is enabled, and initial directory entries are synchronized. These
assume a local filesystem and VFS that honor SQLite locking and synchronization.
Network filesystems, external replacement/rewriting of an active database and
hardware power-loss behavior have not been validated. Reopening can establish the
expected WAL mode; ordinary operations refuse unexpected mode drift.

Tests terminate actual child processes immediately before and after commit and
reopen the database. They establish atomic recovery after process termination,
not a hardware power-failure guarantee. Test-only fault checkpoints also exercise
errors before commit and acknowledgement loss after commit; those checkpoints
are not compiled into production builds.

## Evidence

- `tests/restart.rs`: the same kernel consumers produce equal MemoryJournal and
  reopened SqliteJournal histories/views, including exact intake floats, changed
  policy selections, original parameter bytes and binary content; a deleted
  database is not recreated by an existing handle.
- `tests/atomicity.rs`: simultaneous initialization, competing compare-and-append
  calls, two actual writer processes and whole-batch refusal.
- `tests/corruption.rs`: damaged/missing content, missing and orphan links, head
  and sequence inconsistencies, healthy-session isolation, incompatible schemas
  unchanged on open, immutable-put verification and bounded reads.
- `tests/indeterminate.rs`: acknowledgement loss followed by another append,
  exact replay resolution without duplication, and corrupt resolution that cannot
  become Absent.
- Journal unit tests: abrupt process exits at both commit boundaries, injected
  commit-path failures, unsigned sequence representation and an unsupported event
  version whose storage hashes are otherwise intact.

There is no migration from prior ymp iterations, no inference, and no claim that
storage recovery proves cessation of native execution. W1-0017 and W1-0014 own
those execution and session recovery conditions.

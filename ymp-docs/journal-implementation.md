# Journal implementation

This describes the W1-0001 Rust mapping of the approved model's base values,
Journal and decision attribution. Canonical task status remains in
`tasks/records/W1-0001.json`.
The [intake extension](intake-implementation.md) adds task/contract projections,
CriteriaCommitted, ClarificationRecorded and AssumptionRecorded through W1-0002.
The [Registry extension](registry-implementation.md) adds PoolRecorded through
W1-0003, preserving discovery facts and profile-bound readiness decisions.
The [durable adapter](storage-implementation.md) implements the same contract and
adds explicit immutable-content requirements and read-only append resolution.

## Boundaries

- `ymp-domain/src/lib.rs` supplies validated `Id<T>`, SHA-256 `Digest`, `Ref`,
  `PolicyRef`, `Proposal<T>` and `Denial`. IDs are opaque ASCII identifiers with a
  128-byte bound. The type marker does not change their serialized form.
- `ymp-domain/src/journal.rs` supplies the common `Envelope`, strict JSON codec,
  effective policy parameters and the `Method` value used by the first consumer.
  `Capability`, `MethodKind` and `EscalationStep` are representations needed by
  that value; their execution belongs to later owning tasks.
- `ymp-kernel/src/events.rs` defines the typed `SessionOpened` and `MethodChosen`
  families, currently at payload version 1. Unknown families or versions fail.
- `ymp-kernel/src/journal.rs` defines the adapter contract, parameter-schema
  bindings and complete-batch validation. `view.rs` defines pure replay.
- `ymp-kernel/src/decision.rs` opens a session journal and commits a proposed
  method after validation. It does not call a strategy or start any work.
- `ymp-runtime/src/memory_journal.rs` supplies the in-memory adapter. Existing
  sessions use separate locks; the shared map lock only locates or creates a
  session. A rejected first append creates no session entry.

The domain performs no I/O. The reference consumer composes the kernel with
MemoryJournal in `ymp-runtime/tests/foundation.rs`. No runnable task, provider,
durable storage, budget or complete session loop is implied by this foundation.

## Recorded decisions and references

Opening records the explicitly selected implementations and their actual parameter
objects. `PolicySelection` is the Rust representation of the recoverable selection
required by model section 5.8. Its `PolicyRef.params` must match the canonical
parameter bytes. `ParameterSchemas` binds a validator to a precise port,
implementation and version, with no replacement of an existing binding. The
application composition supplies this fixed set to its Journal adapter. The
built-in binding is `MethodRouter` / `FixedMethod` / `1`; its parameters contain
`kind` and `ladder`. Other implementations can register different typed schemas
without changing the decision consumer. Unknown bindings are denied.

A `MethodChosen` records the full `Proposal<Method>`, effective parameters,
input-view digest and applied outcome. It only records an actually committed
method. Denied proposals return `Denial` and append nothing. No denied or merely
requested change is represented as an applied method.

`SessionControl` is an in-process capability issued by the opening consumer and
bound to that consumer and session. A changed selection requires this capability
as a separate argument, the exact previous revision and a real method decision in
the same atomic append. A proposal cannot carry owner authority. Every boundary
currently has no executing work because no execution event is supported yet.
W1-0014 must connect safe boundaries and recovered user authority to the real
session lifecycle before allowing changes during execution.

`Decision` and `SelectionChange` are serialized containers for the attribution
and boundary facts required by sections 0, 5.8 and 9, not additional services.
References resolve only against earlier records in the same session. Event IDs
are derived from `(session, sequence)`; their versions digest the complete
Envelope. Method references digest the full Method. Earlier versions remain
resolvable after a later choice. A replayed view is owned, read-only data and
contains no journal writer or owner capability.

The input-view digest uses a fixed v1 commitment to the session, sequence and
complete set of prior versioned references. Each event reference covers its full
Envelope, so this identifies the exact journal prefix from which the view is
derived. Adding a derived view field does not invalidate past decision digests.

## Serialization and append contract

Canonical JSON v1 uses recursively sorted object keys and compact UTF-8 JSON.
Array order and number representation are significant. Decode rejects duplicate
keys at every depth, unknown typed fields, invalid IDs/digests and excessive JSON
nesting. Payload version changes require explicit decoding/replay support; the
frozen new-implementation version-1 opening in `ymp-kernel/tests/replay.rs` must
remain readable as further families are added.

An append has an expected revision and a nonempty batch. The kernel consumer
validates it before calling the adapter and checks the returned revision. The
adapter repeats validation while holding its session lock: it checks the existing head, replays the current state,
validates the complete batch and commits it. A stale revision, reference mismatch,
invalid decision or unsupported payload rejects the entire batch. The revision
is a per-session `u64`; checked increment refuses overflow. Explicit timestamps
are milliseconds since the Unix epoch. Replay does not read clocks, invoke
strategies or recreate stochastic responses: it restores the recorded proposal.

MemoryJournal holds all events in memory and reconstructs views by replay. The
implementation favors a transparent reference contract over an incremental
index; it makes no crash durability, restart, throughput or migration claim.
W1-0016 supplies persistent contents and atomic durable append in SqliteJournal.

## Focused evidence

- `ymp-domain/tests/validation.rs`: malformed values, SHA-256 known vector,
  canonical encoding, nested duplicate-key rejection and parameter digest binding.
- `ymp-kernel/tests/replay.rs`: foreign sessions, sequence/head disagreement,
  malformed schemas, unknown versions, atomic refusal and frozen version-1 bytes.
- `ymp-runtime/tests/foundation.rs`: two different strategies and parameter schemas
  through the real consumer, historical replay after an authorized change,
  unsupported/foreign inputs, a two-thread revision race, atomic batch refusal
  and rejection before a deliberately permissive adapter can claim success.
- `SessionView` compile-fail documentation: a strategy's read-only view cannot
  mutate its journal revision.

The demonstrations use local deterministic strategies. They do not establish
native-provider behavior or empirical benefits of self-organization. Full
MethodRouter inputs and scheduling remain W1-0011 and W1-0014 responsibilities.

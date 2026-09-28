# Workspace implementation notes

Canonical task status is in `tasks/records/W1-0005.json`. Direct capture, retained
snapshots, persistent physical-root binding and mediated file access are implemented.
Native execution and admission integration remain owned by W1-0017 and W1-0006;
these file-access guarantees do not claim that a native session is running.

## Capture and retained content

`ymp-domain/src/workspace.rs` validates portable relative WorkspacePath values,
workspace identity and a SnapshotTree containing file digests, byte counts, ordinary
Unix modes and directories (including empty ones). Root is `.`; absolute paths,
traversal, empty components, control characters and ambiguous backslash/colon syntax
are refused. File/directory collisions and missing parent directories are invalid.
Device/inode and canonical root identify a workspace, not a confinement mechanism.

`ymp-runtime/src/workspace/direct.rs` implements WorkspaceProvider over an opened
directory. rustix supplies safe descriptor-relative filesystem operations; this is
the new dependency's concrete purpose. Each descendant is opened with NOFOLLOW,
regular-file opens use NONBLOCK, and the opened object's type, device, inode and
link count are checked. Symlinks, hard-linked files, device boundaries, special
files, non-UTF-8 names and special Unix permission bits are refused. Capture preserves
ordinary permissions; ownership, timestamps, ACLs and extended attributes are not
included. Hidden task files are retained. Once a root binding is attached, capture
excludes exactly its validated application-owned marker; it does not apply an
ambient exclusion list or silently omit a user-owned file with that name.

Explicit limits bound entries, depth, per-file size and total bytes. The adapter
checks content and directory metadata around each read, compares directory listings,
and requires two equal scans of the complete bounded tree. A changed physical root
is refused. Matching scans detect ordinary concurrent changes, but cannot establish
an atomic filesystem snapshot against arbitrary external mutation or an ABA change.
WorkspaceGuard holds a root Read claim against managed writers in the same journal
throughout capture and publication. The adapter itself makes no OS isolation claim;
external mutation and independently configured stores are not thereby confined.

WorkspaceGuard reconstructs workspace identity and provider selection from the journal,
checks the returned tree and every stored file digest, and retains the manifest before
committing SnapshotTaken. Event.contents explicitly links the manifest and file bytes;
SQLite verifies those links on commit and read. A failed capture may leave unreferenced immutable content and records a capture
abort after its synchronous I/O ends; it does not record a completed snapshot. Application storage should
be outside the source tree so writes to storage cannot alter the tree being captured.

`retained` loads and verifies the stored manifest and its files without consulting the
live workspace. `read_artifact` returns the exact bytes named by a recorded snapshot.
No method here applies those bytes to the user's directory or claims to have run a check.

## Persistent root binding

`RootBinding` in `ymp-runtime/src/workspace/binding.rs` installs or verifies an
immutable `.ymp-workspace-owner` marker. It records format, physical workspace root,
and JournalIdentity (stored random identity, canonical database path and physical
file identity). The journal records the same value in WorkspaceBound through
WorkspaceGuard.bind_workspace. A marker alone does not grant execution access.

Markers are installed through a private staging file and a no-replace rename,
with file and parent-directory synchronization. Existing foreign or malformed files
are refused without overwrite. A recorded binding whose marker disappeared is not
silently recreated. Replaced markers and roots are refused by existing handles.
A copied database cannot join the original root merely by retaining its UUID.
Only the journal contains mutable access holds; there is no second mutable ledger
in the marker. Relocation, rebinding and arbitrary external state replacement are
not silently repaired by this interface.

Binding operations lock physical ancestor directories from top to bottom, shared
on ancestors and exclusive on the chosen root. They retain those descriptors until
validation and installation finish, then explicitly unlock them. Existing ancestor
markers and bounded descendant scans forbid nested bound roots. Use relative scopes
within one workspace instead. Independent sibling roots can bind concurrently.
Unavailable topology, aliases, links, device crossings and scan limits cause refusal.

Unbound Direct preview obtains an exclusive root lock plus shared ancestor locks
for all its synchronous I/O, checks ancestors and scans descendants for existing
bindings. Thus it cannot read a bound descendant from another journal or race a new
child binding. Bound capture uses shared coordination locks and verifies its marker.
Capture receives the calling journal's identity as a read-only observation and
checks it under the provider's binding mutex. A bound provider reused by another
Guard cannot capture through that Guard's unrelated journal.
This is cooperative filesystem coordination, not a native sandbox. The mediator prevents access to control metadata and exposes checked synchronous
file operations. Native execution must independently establish its actual enforcement.

## Capture read lifecycle

Before capture, WorkspaceGuard records LockChanged::CaptureStarted with a root
PathObservation and a random per-attempt owner from OS entropy (`getrandom`). The
owner is an identity, not a serialized authority token. It binds the local completion
capability and prevents identical requests from different Guards from sharing one
hold, including when an adapter resolves an indeterminate append by matching bytes.
CaptureRead participates in the same aggregate conflict checks as assignment holds.
The [result consumer](result-implementation.md) adds a protected baseline capture
under an admitted writer that has no invocation authority, and an atomic transfer
from ceased Write ownership to the after capture. Other writers retain the same
aggregate exclusion guarantees; these operations do not add agent file authority.

The WorkspaceProvider capture contract is synchronous: all its I/O must end before
it returns, including on failure. A local pending completion retains the exact start
reference, owner, workspace, time and either a retained manifest digest or a bounded
failure. The Guard keeps at most 64 pending entries. Once I/O has ended, resolve_capture
retries only publication or abort at a fresh revision; it never recaptures the tree.
A confirmed foreign/absent failed begin is removed locally without aborting another
owner, even when the journal first became unavailable and is resolved later.

SnapshotTaken version 2 requires the matching live Read hold and atomically publishes
the snapshot and ends the hold. Version 1 remains replayable but cannot be newly
appended. Commit resolution checks a valid preceding prefix before recognizing an
exact historical packet, preserving the atomic intake-refinement boundary.

If publication fails, local completion evidence remains available for retry or explicit
abandon_capture. Abandonment is refused during I/O; after completion it records Abort
without publishing a snapshot. A lost final acknowledgement resolves idempotently.
A fresh Guard cannot recreate completion authority for an unfinished capture from
stored data, so unresolved holds survive restart. With a readable, matching owner,
a lost begin acknowledgement can resolve only that Guard's own never-started attempt.
When storage is unavailable the hold and any local completion remain unresolved.
The journal reserves one control event and 64 KiB per active capture for abort; this
is a logical capacity guarantee, not a promise under disk exhaustion or I/O failure.

## Ownership bookkeeping and aggregate transactions

`workspace_locks.rs` validates LockChanged acquisition, single-use invocation
authorization, revocation and evidence-backed release. A lock's effective paths
come from an opaque AccessEvidence bound to the Guard, session, workspace reference
and profile. The requested paths cannot narrow the effective lock footprint or
exceed the required task/backend capabilities. In particular, an exclusive Write
lock is not permission to perform an otherwise forbidden ReadFiles operation.
Registry constraints and profile eligibility are rechecked at authorization.

Direct.observe_paths records physical ancestry and the missing suffix of each
requested path. Comparison uses shared physical objects to detect aliases and
ancestor workspace roots; ASCII case is conflated conservatively, and ambiguous
Unicode components cannot establish disjointness. Traversal and unsupported
filesystem objects are refused. These observations alone do not establish confinement.
Mediated operations compare the acquired physical scope with opened descriptors
before accessing bytes. An originally missing scoped leaf must be created exclusively by its handle;
subsequent access checks the identity returned by that successful creation. A missing
scope cannot become a directory. Existing scope anchors cannot silently change identity.

WorkspaceGuard uses replayed Journal.read data, reconstructs effective claims and
checks all current owners before appending. SQLite repeats the kernel check under
BEGIN IMMEDIATE; MemoryJournal uses one common append mutex. Thus a second session
cannot commit a conflicting lock between validation and commit in the same store.
SQLite enumerates sessions from heads, events and content links, so a missing head
cannot hide a retained owner. Other sessions are replayed one at a time; only their
active ownership is retained in the bounded aggregate projection. Unrelated historical
events do not exhaust an aggregate-history quota or prevent release.

Active ownership is bounded to 4,096 assignments and 16 MiB of projected claims.
SQLite also protects the remaining authorize/revoke/release event slots and 64 KiB
per control event from ordinary appends. Basis lists are bounded to 32 references.
This reserves control capacity under the existing per-session journal limits; it
does not promise writes when the filesystem itself is full or unavailable.

Revocation keeps all holds. NeverAuthorized evidence is issued only for an unreleased
assignment without invocation authorization; state changes invalidate old evidence.
Terminated/AccessWithdrawn evidence is non-deserializable, scoped to the current
assignment/workspace/invocation state and requires a recorded basis. The mediated factory produces AccessWithdrawn after closing and draining its actual
file capability. Native Terminated evidence remains unimplemented here. Synthetic
fixtures separately exercise those consumer inputs without claiming native execution,
confinement or cessation. No cost receipt or
financial settlement releases workspace access.

## Evidence

`ymp-storage/tests/workspace.rs` exercises Direct through the real WorkspaceGuard and
SQLite content/journal adapters. It checks exact binary bytes, executable permissions,
empty directories, changed/deleted files and reopening both snapshots after restart.
Negative cases cover traversal, symlink/hard-link aliases, root replacement, corruption
of stored content, limits, a deterministic write between capture scans and a forged
adapter view trying to attribute another directory to the recorded workspace. Failed
validation before acquiring a hold leaves the journal unchanged; failures after
acquisition retain their Begin/Abort history without publishing a snapshot.

`workspace_guard_tests.rs` exercises the actual consumer with private synthetic
access and cessation inputs: broad scopes, compatible reads, disjoint writes,
revoked holds, state-bound release, foreign evidence and missing read capability.
`workspace_locks.rs` integration tests use Direct observations and trusted synthetic
LockChanged facts to check same-store cross-session/thread/process conflicts,
physical ancestor roots and corruption of an owner's head. The SQLite capacity
test uses a small test-only event limit to demonstrate that an ordinary pool-refresh
append is refused while authorize/revoke/release can still complete.

`ymp-storage/tests/capture.rs` checks capture/writer exclusion, failure and CAS retry,
lost begin/final acknowledgements, immutable completion without repeated I/O,
abandonment, unresolved holds after restart, historical v1 replay, invalid partial
refinement resolution and races between two Guards with identical requests. Repeated
foreign begin losses, including temporary read failures, do not exhaust local capacity
or abort the foreign holds.

`ymp-storage/tests/identity.rs` checks v1 migration without changing retained content,
concurrent identity creation, replacement/hard-link refusal and malformed identity.
`ymp-storage/tests/binding.rs` exercises WorkspaceGuard binding, marker exclusion,
missing/foreign metadata, copied databases, nested roots in both orders, thread and
process races, sibling roots and unbound-preview exclusion of bound descendants.

## Mediated file access

WorkspaceGuard.mediate creates an opaque MediatedAccess and records a random
per-attempt owner in LockAcquisition. This prevents two otherwise identical creation
attempts from owning the same handle after ambiguous commit resolution. Missing
`mediated_owner` remains omitted from old serialized acquisitions. A journal record
cannot recreate a live handle. Failed creation may leave an unresolved conservative
hold; it never exposes file I/O without the matching recorded owner.

The factory requires a bound workspace and an eligible Scripted profile with only
ReadFiles/WriteFiles capabilities. It refuses native or process-capable environments,
rather than assuming a requested path list constrains their access. The host must
expose only this handle to the Scripted participant. WorkspaceProvider is trusted
adapter code with an explicit synchronous-I/O contract; an arbitrary plugin is not
made safe merely by implementing this trait.

Each operation checks current replayed ownership, authorization, revocation, release,
profile eligibility and binding. Read and Write are distinct permissions. FileAccess
binds the checked operation to its exact root/journal, acquired physical scope and
freshly observed full target. The kernel checks that target against other retained
physical owners; Direct checks the same chain again on opened descriptors.
Direct opens descendants relative to its root descriptor without following links,
checks original existing scope identities during traversal, and refuses marker names,
staging names and physical marker aliases. Another root in the same journal cannot
consume the operation. Regular-file type, device, link count, scope and marker checks
precede truncation; write size limits precede creation. Reads are bounded and checked
for concurrent changes. File errors can leave partial changes and never release a hold.

The API reads or replaces complete files and creates missing leaf files under existing
parents. Every prepared existing file receives a journaled physical hold before its
bytes are read or written. Read holds remain compatible; Write is retained as the
strongest mode for an object. The holds are keyed by physical identity, bounded to
4,096 objects per assignment and retained through revocation until validated release.
A renamed file therefore remains protected even after leaving the declared directory.
An originally missing scoped leaf uses its published identity for later operations;
it cannot silently adopt a foreign replacement. All read/write and creation-resolution
calls take an explicit timestamp from the trusted host for the events they produce. It does not create/remove/rename directories, expose descriptors or execute
processes. File operations within one handle are serialized; independent handles can
perform compatible reads or disjoint writes. ReadOnly is a constrained provider with
capture and read capabilities but no write method; the same kernel consumer denies
write ownership before appending, records its distinct policy and runs actual reads.

Withdrawal closes the handle before waiting for the operation mutex. Queued operations
recheck closure after acquiring that mutex, so they cannot start after withdrawal.
Only after admitted synchronous I/O returns can the issuing Guard produce state-bound
cessation evidence. A poisoned operation mutex retains uncertainty and cannot certify
withdrawal. Revocation and dropped handles retain holds. NeverAuthorized evidence
becomes invalid if authorization intervenes; release must validate the current state.
The release basis persists across restart independently of Treasury settlement.

Filesystem checks detect observed replacement of roots, scope anchors, leaf files and
metadata. They do not claim to stop a privileged external actor from moving already-open
objects or changing them after validation. Managed file operations cannot make those
topology changes. Native execution needs its own enforced boundary; cwd alone is
insufficient. W1-0006 must bind ownership to admitted assignments, and W1-0017 must
connect the admitted Scripted host and native execution lifecycle.

`ymp-storage/tests/mediation.rs` exercises actual file operations through WorkspaceGuard
and SQLite: authorization, separate read/write scopes, limits before mutation, link
and marker rejection, existing/missing directory and leaf replacement, wrong-root forwarding, compatible
cross-session reads/disjoint writes, retained revoked/disconnected ownership, unrelated
work, restart release basis, ReadOnly behavior, process-capability denial, blocked I/O
withdrawal and provider panic. These fixtures do not claim a completed native agent run.

### Durable file preparation and creation

All mediated I/O obtains WorkspaceCoordination before checking current pending
creations, resolving the complete target and preparing its descriptor. Direct uses
shared physical ancestor flocks and a blocking exclusive root flock for this short
section; separate handles and processes participate in the same coordinator. Existing
file preparation does not truncate data. WorkspaceGuard commits FileAccessPrepared
with the target's physical observation and permitted Read/Write mode before dropping
the coordinator. The returned WorkspaceFile retains that exact descriptor for data
I/O outside the coordination section. A failed publication exposes no bytes.

Missing-file writes first commit FileCreationStarted with a unique random owner and
target. Direct then opens the file exclusively, verifies its descriptor, synchronizes
the empty file and parent directory, and returns its physical identity. FileCreated
publishes that identity into the assignment's physical holds and clears the pending
creation before data I/O begins. The maximum target encoding is 48 KiB; pending
creation reserves 64 KiB of aggregate ownership capacity and one additional control
event/64 KiB of journal capacity for publication or proven non-attempt. New creation
is refused before filesystem mutation if its future publication cannot fit. Ordinary
appends still cannot consume the separate revoke/release capacity.

A pending creation prevents new file I/O in that physical root, even for a different
session, until its uncertainty is resolved. The live handle keeps only one opaque
local outcome: not attempted, prepared identity, or uncertain. resolve_creation
retries only a proven non-attempt abort or publication of its known physical identity;
it never repeats creation or writes data. Lost publication acknowledgements resolve
against the recorded identity. Preparation errors after filesystem mutation and loss
of local state retain the pending barrier. Restart or absence of the old pathname
cannot prove that no file exists. A validated handle withdrawal drains all admitted
I/O; its subsequent assignment release can end retained ownership and its barrier.
An I/O error after publication may leave partial bytes but has no unpublished creation.

These checks do not stop an external actor from moving or modifying already-open
objects. They keep managed operations and later admissions from treating a renamed
physical file as unowned while its assignment still holds it. They do not promise
OS confinement of arbitrary native processes or reconstruction of lost live authority.

`ymp-storage/tests/file_creation.rs` covers failures before/after Begin and publication,
post-open preparation failure, resolution without repeated data I/O, retained pending
state after restart and pathname movement, exclusion of another operation during
preparation, and partial data writes after physical ownership publication.
`workspace_capacity_tests.rs` uses real SQLite with a small event limit to show that
creation is refused before I/O without room for publication, and successful creation
still leaves enough capacity for revoke and release despite an unrelated denied append.

# ymp local schema policy

This document freezes the durable schema implemented by the initial production codebase. It is a
compatibility contract, not a promise that every later feature fits schema version 1.

## Event journal version 1

Every line in `events.jsonl` is one UTF-8 JSON `EventEnvelope` followed by `\n`. Version 1 fixes the
following envelope fields and their digest input order:

- `schema_version`;
- `run_id`;
- `sequence`;
- `command_id` and `command_digest`;
- `predecessor_digest`;
- the tagged `event` value; and
- `digest`, computed as SHA-256 over the canonical serialization of all preceding fields.

The first sequence is 1. Later records increment by exactly one, use one run identifier, and name
the preceding record digest. A repeated command identifier is valid only when its command digest is
identical. Readers reject a missing newline, a gap, a duplicate sequence, a mixed run identifier,
an unknown schema version, a predecessor mismatch, or a record-digest mismatch. They never repair
or skip a record.

Version 1 records are limited to 64 KiB each. A journal is limited to 16 MiB and keeps 64 KiB in
reserve for one terminal infrastructure-failure record. These constants are part of the local
storage profile; changing them requires an explicit profile or schema decision.

## Compatibility and migration

Schema version 1 is immutable. A change that alters field meaning, digest input, event tags,
required fields, ordering rules, or replay behavior requires a new schema version. The current
binary reads and writes only version 1 and fails closed on every other version.

Migration must be an explicit offline operation into a new data root. A future migration tool must:

1. acquire exclusive ownership of the source and destination;
2. validate the complete source digest chain without modification;
3. transform every record in sequence and build a new digest chain;
4. retain the source schema version, source head digest, destination head digest, tool version, and
   transformation identifier in migration evidence;
5. validate the destination by reopening it with the destination reader; and
6. leave the source bytes unchanged until the operator separately approves removal.

No current command performs migration. An unsupported journal therefore remains inspectable as
raw evidence but cannot be resumed or rewritten by this binary.

## Other durable data

Object identifiers are canonical lowercase SHA-256 digests. Object reads rehash stored bytes before
returning them. Candidate-submission events require their candidate snapshot object to exist.
Verification events require a verifier-evidence object under the recorded evidence digest.
Verifier-evidence schema version 2 requires `candidate_digest`, `contract_digest`,
`oracle_digest`, and `environment_digest`. The environment digest identifies a separate immutable
object in the same content-addressed store. The evidence object is the canonical verifier record
with its self-referential `evidence_digest` field set to the empty string; the event and export
manifest supply the verified object identifier. Before accepting a result, the application stores
the exact environment bytes passed to the verifier, supplies the resulting object path and digest
to the verifier, and checks the same object again before committing the verification event.
Recovery reopens both objects, rehashes them, restores the evidence record, and checks that its
candidate, contract, oracle, decision, and evidence digest agree with the journal event.

Verifier-evidence schema version 1 did not contain `environment_digest`. Such evidence is
deliberately not migrated or assigned a guessed environment: recovery returns a typed
infrastructure error and marks the `run.json` projection as `infrastructure_error`. New evidence is
written only as version 2. This evidence-object revision does not alter the version-1 journal
envelope or event shape because the journal already identifies the complete content-addressed
evidence object.

`run.json` is an atomically replaced projection of the journal, not an independent source of truth.
If projection replacement fails after a journal commit, retrying the same command replays the
committed result and repairs the projection without applying the command twice.

An evidence export is constructed in a fresh sibling directory and renamed into place only after
all files are written. Version 1 contains `manifest.json`, `state.json`, `events.jsonl`, the exact
candidate manifest and materialized tree, one content-addressed JSON object per recorded verifier
result, and every referenced immutable environment object under `environments/<digest>`. The
manifest lists both evidence and environment digests. When a managed runtime was used, the export
also contains its `runtime-evidence/<attempt_id>/profile.json` record and `events.jsonl` transcript.
Existing destinations are never overwritten.

## Managed contract and runtime evidence

A managed contract is a bounded JSON file containing one identifier, source directory, prompt,
capture exclusions, and an optional command-verifier configuration. Relative paths resolve against
the contract file; the SHA-256 digest of the exact input bytes identifies the contract. Runtime and
verifier selection never changes that file or falls back to another configured profile.

Runtime-evidence version 1 wrote only a separate bounded event transcript. Its records contain the
run, attempt, runtime kind, contract identifier and digest, runtime event sequence, predecessor
digest, privacy-reduced event, and record digest.

Runtime-evidence version 2 added `profile.json`. The profile record contains the successful probe,
runtime executable digest when the runtime is an external file, generated-environment policy,
invocation identifier, coordination transport, bridge executable digest, invocation-scoped
endpoint-path digest, and exact coordination-tool allowlist. It records that credential values are
omitted. Each version-2 event contains the same invocation identifier and `profile_digest`; both
fields participate in the event digest, so a transcript cannot be rebound to another executable,
route observation, invocation, or coordination endpoint without detection.

Runtime-evidence version 3 binds `profile.json` to the immutable launch descriptor used to create
the managed process. The descriptor contains the admitted runtime executable path and digest, the
admitted coordination executable path and digest when MCP is configured, ordered arguments,
working directory, attempt and invocation identifiers, and the complete cleared child environment.
Non-confidential environment entries contain their value and digest. Confidential entries contain
only their name, confidentiality marker, and value digest. Runtime and coordination executables are
copied from one already-open source file into private admitted files; probing, descriptor creation,
MCP configuration, and execution use those admitted files rather than reopening the source paths.
The managed launcher derives both the process command and saved profile from that descriptor,
rechecks both executable digests at launch, and rejects any executable, coordination executable,
argument, environment, attempt, or invocation substitution. Every initial or resumed process emits
a launch event derived from its actual descriptor; the supervisor rejects events that cannot be
bound to the profile's invocation and immutable launch fields.

Managed completion and yield authority never comes from runtime output. `submit` is authoritative
only after its idempotent command is committed to the application journal. `yield` is authoritative
only after the invocation-bound RPC controller records its command identifier and confirmation;
repeating that identifier replays the same confirmation. At a turn boundary the supervisor accepts
exactly one new controller action. No action, multiple fresh actions, or a submitted/yielded message
that exists only in child output produces a typed protocol failure and no candidate. A wake command
is likewise idempotent by command identifier and resumes the same attempt, invocation, and opaque
runtime session.

Version 3 records terminal usage for successful completion, failure, cancellation, and timeout.
Every terminal record includes total wall time, protected-query count, optional provider cost,
token counters, and the latest explicitly reported bounded in-flight excess counters; successful
completion does not replace a non-zero reported excess with zero. Failures contain a typed safe failure kind
and, when available, only a bounded diagnostic digest, byte count, and truncation marker. Raw child
standard error and diagnostic text are never written to runtime evidence or the control journal.
Older serialized usage values remain readable with zero defaults; new evidence is written only as
version 3. Existing version-1 and version-2 evidence remains immutable and is not rewritten.

In all versions, session identifiers, response text, MCP arguments, and MCP results are represented
only by SHA-256 digests; usage counters and MCP tool status remain explicit. The writer synchronizes
every record before the TUI displays it. The control journal does not derive authority from this
auxiliary transcript, and application recovery does not currently require it; export preserves it
for accounting and independent chain validation.

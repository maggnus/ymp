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

Runtime-evidence version 2 adds `profile.json`. The profile record contains the successful probe,
runtime executable digest when the runtime is an external file, generated-environment policy,
invocation identifier, coordination transport, bridge executable digest, invocation-scoped
endpoint-path digest, and exact coordination-tool allowlist. It records that credential values are
omitted. Each version-2 event contains the same invocation identifier and `profile_digest`; both
fields participate in the event digest, so a transcript cannot be rebound to another executable,
route observation, invocation, or coordination endpoint without detection.
Completion usage additionally records total wall time, protected-query count, optional provider
cost, and bounded in-flight excess counters. Older serialized usage values remain readable with
zero defaults, but new evidence is written only as version 2.

In both versions, session identifiers, response text, MCP arguments, and MCP results are represented
only by SHA-256 digests; usage counters and MCP tool status remain explicit. The writer synchronizes
every record before the TUI displays it. The control journal does not derive authority from this
auxiliary transcript, and application recovery does not currently require it; export preserves it
for accounting and independent chain validation.

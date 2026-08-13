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

## Event journal version 2

Version 2 keeps every envelope field, ordering rule, digest input and limit of version 1 and adds
one event tag: `contract_approved`, carrying `contract_id`, `contract_digest` and `oracle_digest`.
It binds a run to the contract it is judged against, so a result can be checked against the exact
contract and acceptance condition that were approved before the run started. Without it the
journal recorded a run whose contract was known only outside the ledger.

A run start is still the first record. The approval follows it, is idempotent by command
identifier like every other command, and is refused once the run already carries a binding: a run
is judged against one contract and no other. The `run.json` projection gains the optional
`contract` field carrying the same three values; a projection written without it reads back as an
unbound run rather than failing.

## Compatibility and migration

Schema version 1 is immutable, and version 2 is a new version rather than an extension of it. A
change that alters field meaning, digest input, event tags, required fields, ordering rules, or
replay behavior requires a new schema version. The current binary reads and writes only version 2
and fails closed on every other version, version 1 included.

The migration consequence is stated rather than worked around: a journal written by an earlier
binary is rejected at open with an unsupported-schema error, and no command migrates it, because
no migration tool exists. Such a store remains inspectable as raw evidence and cannot be resumed,
extended or exported by this binary; a new run needs a new data root. Evidence exported from a
version-1 store keeps its own bytes and is not rewritten.

Refusing such a store changes nothing in it. The version is read from the first journal record
before the store is opened for writing, so no writer lock is taken, the `run.json` projection is
not replaced, and the refusal is not recorded as an infrastructure failure of a run this binary
cannot read. A store whose journal is unreadable for any other reason keeps the earlier behaviour:
its projection is marked `infrastructure_error`, because there the failure is this binary's to
record.

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
capture exclusions, and a command-verifier configuration. Relative paths resolve against the
contract file; the SHA-256 digest of the exact input bytes identifies the contract. Runtime and
verifier selection never changes that file or falls back to another configured profile.

Contract-record version 1 made the verifier optional. Version 2 requires it, and requires the
verifier to name its program, its negative control and its oracle digest. A package that states
none of them describes work no mechanical check can decide, so it is rejected when it is read
rather than accepted and discovered at verification time, when budget has already been spent.

The migration consequence: a version-1 package is rejected with an unsupported-version error, and
a version-2 package without a verifier is rejected with the missing part named. Neither is
migrated automatically, because supplying an acceptance condition is exactly the decision the
kernel may not take for the operator. An existing package is amended by declaring its verifier and
raising its version, which changes its bytes and therefore its digest — the amended package is a
new contract identity, and a run started against the old one keeps naming the old digest.

A contract drafted from a typed request is stored as an immutable object under its own digest
before the run starts, and the run's `contract_approved` record names that digest. In version 2 the
oracle digest is the digest of the verifier program itself: a drafted contract reads it from the
program, and a package that declares one is held to it — a declared digest that names something
else is rejected rather than stored as a claim its own program contradicts.

A package read from a file is stored as the canonical document with its paths resolved, so the
digest that identifies a stored contract is the digest of what the run is judged against and not of
the file's formatting. The file itself remains the operator's input, unchanged.

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

The environment the descriptor carries is closed rather than merely reduced. A managed Claude
invocation names only the generated home, its configuration and temporary directories, the search
path, the two display and traffic settings, and — when coordination is configured — the endpoint,
its token, and the attempt and invocation identifiers. Any other name is refused before the process
is created, whatever placed it in the descriptor. The search path is the approved system
directories; the operator's own search path is never delegated, so what it names beyond those
directories reaches neither the child nor the profile record.

Launch attestation is required of every runtime that creates an external process, and such a
runtime may not emit any other event before it. A resumed launch keeps the executable, coordination
executable, environment and working directory of the initial launch and may add only its own resume
operand carrying the established opaque session: `resume <session>` before the prompt source for
Codex, and a trailing `--resume <session>` for Claude Code. The rule is stated per runtime rather
than inferred from the observed arguments, so a resumed process cannot introduce an argument the
initial descriptor did not contain.

The generated-environment policy names what the label actually provides. `synthetic_allowlist_v1`
starts from a cleared environment and a synthetic home that carries the runtime's own credential
file. `synthetic_allowlist_with_delegated_credential_v1` is the same construction for a runtime
whose subscription credential lives outside its home, so the driver copies the operator's
credential into the generated home for the invocation. That is deliberately the weaker profile
`SECURITY.md` requires to be labelled rather than presented as strict containment.

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
token counters, and bounded in-flight excess counters. Token and cost counters are recorded exactly
as the runtime reported them and are never invented. The in-flight excess counters state what the
runtime's own accounting has not yet covered, and both of them are computed by the product from
values the runtime reported rather than read from a counter the runtime sends. One is the count of
model requests observed on the transcript that no accounting record has closed. The other is the
monetary excess: for the Claude runtime the product obtains it by subtracting the profile's
enforced ceiling from the cost the runtime reported for the turn, so what the record states is a
difference between a reported cost and an approved bound and not a number the runtime supplied.
An accounting record settles the excess of the turn it closes — the counters that record states
replace the current ones, and a record that states none leaves none — so a runtime that reported
complete accounting records a zero excess rather than the request count the product held while the
turn ran. A run that ended before its accounting record keeps that unaccounted request count. Total
wall time is measured from process creation, not from the end of the launch checks that follow it.

A recorded cost is kept only while it is attributed to the models that produced it. When a Claude
result carries the per-model breakdown the runtime reports beside its total, the product checks
that the breakdown adds up to that total, allowing only the rounding of each share to whole
microdollars, and that no model outside the admitted profile spent anything. A total its own
breakdown does not support, and consumption by an unadmitted model, are typed protocol failures
rather than recorded costs, because a matched-budget comparison reads the recorded number as
evidence of one model route. A result that carries no breakdown is recorded as before and is not
attributed to the admitted model by assumption.

Failures contain a typed safe failure kind and, when available, only a bounded diagnostic digest,
byte count, and truncation marker. Raw child standard error and diagnostic text are never written
to runtime evidence or the control journal.
Older serialized usage values remain readable with zero defaults; new evidence is written only as
version 3. Existing version-1 and version-2 evidence remains immutable and is not rewritten.

In all versions, session identifiers, response text, MCP arguments, and MCP results are represented
only by SHA-256 digests; usage counters and MCP tool status remain explicit. The writer synchronizes
every record before the TUI displays it. The control journal does not derive authority from this
auxiliary transcript, and application recovery does not currently require it; export preserves it
for accounting and independent chain validation.

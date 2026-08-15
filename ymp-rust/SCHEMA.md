# ymp local schema policy

This document freezes the durable schema implemented by the initial production codebase. It is a
compatibility contract, not a promise that every later feature fits schema version 1.

## Product state layout version 1

Everything the product writes on a host lives under one root. The root is `.ymp` in the operator's
home directory — `YMP_HOME` names another one for a whole environment, and `--root` for a single
invocation — and it is addressed rather than named per run:

```text
~/.ymp/
  root.json                     layout marker and version
  providers/<provider>.json     one account: family, observed state, the engines that reach it
  runtimes/<engine>.json        one runtime engine: admission, properties, model list
  projects/<project>/
    project.json                the directory this project addresses
    runs/0001/                  one store: one run, its objects and its evidence
    runs/0002/
```

The directory the product was started in receives nothing. It is the project the state is filed
under, not a place the state is kept, and the project segment is what keeps one project's runs
apart from another's under the one root they share.

The project segment is derived from the canonical launch directory: its own name, reduced to
characters every filesystem accepts, followed by the first twelve hexadecimal characters of the
SHA-256 of the whole path. Two projects whose directories share a name therefore never share a
segment. The run segment is a four-digit ordinal. Neither segment is supplied by the operator.

A store is what every earlier rule in this document describes, unchanged: one run, one journal,
one content-addressed object store, and the recovery and refusal behaviour stated below. What the
layout adds is that a second run and a second project are addressed, not named.

The engine registry stands beside the projects rather than inside a store, because which engines
this host admits is one decision and every run under the root reads it. Which registry an
invocation reaches is derived from the path it addresses rather than from the way it named it: a
store is walked up to the first ancestor carrying `root.json` that addresses that store under its
projects, so `--data-root` on a store inside a root reads that root's registry. A store standing
under no such root has no root decision to honour and keeps its registry beside it. The registry is
described under *Runtime engine registry version 1* below.

The provider records stand beside the engine records and are reached the same way, because which
accounts a host can send work to is the same kind of decision as which engines it admits: one per
root, read by every run under it. They are described under *Provider records version 1* below.

The provider directory is additive and the layout version is not raised for it. A root written by
an earlier build carries no `providers/` directory, and that is the state a root is in before
anything has been observed, not a version this build cannot read; a root written by this build
carries a directory an earlier build never looks at. Nothing is migrated either way.

An invocation that commits a run start is given a store holding no run; every other invocation is
given the store the project is already on. A store addressed but never started into holds no
journal and is where the next run belongs, so a refused start leaves no empty directory behind.

Creating a run directory is exclusive, so two invocations that both reach that step take different
ordinals. A directory holds no journal until its run is committed, though, and an invocation
arriving inside that window is given the directory another one just took. The layout therefore
does not promise that concurrent starts each receive a store; it promises that a store holds one
run. The writer lock gives such a store to one invocation and refuses the others with `data root
is already owned by another foreground process`, and no run of theirs is committed anywhere.

Both intents materialize the project's own directories, so a project directory that exists under a
root always carries `project.json` naming the directory it stands for.

`root.json` carries the layout version. A root of any other version is refused when it is opened,
and no command migrates a root. A directory holding a journal is refused as a root, because it is
a store.

`--data-root` addresses one exact store instead of one under a root. That is how a store written
before this layout is read where it stands. When state an earlier build left beside the project
stands in the launch directory — a `.ymp` root, or a `.ymp-data` store older still — the default
is refused and names both ways to proceed: reading that directory where it stands with `--root` or
`--data-root`, or declaring the root this build addresses and leaving it untouched. Nothing is
copied out of it and nothing is written into it. A root named on the command line answers that
question itself and is not asked again.

Three kinds of path are deliberately outside the root, each for a reason that does not apply to
durable state. The coordination socket, the generated runtime home and the private copies of
admitted executables live in the operating system's temporary directory: they exist only while one
process does, they are removed with it, and a unix socket path is length-limited in a way a
project-relative path cannot honour. An evidence export is written where the operator names it,
and beside the project under the run's own name when they name none, because an export exists to
leave the root.

## Run identity version 2

A run is identified by `run-<contract>-<store>`: twelve hexadecimal characters of the digest of the
contract the run is judged against, and eight of the digest of the absolute path of the store that
holds it. The store path is taken as the invocation states it, made absolute and rejoined from its
components; nothing is read from the filesystem, because the store directory of a run being
authorized does not exist yet and the identifier a decision surface shows before the start must be
the one the start records.

Version 1 derived the identifier from the contract alone. A contract names what a run is judged
against and not which run it is, so authorizing one contract twice wrote two runs — two journals,
two run projections, two exports — that all named themselves `run-<contract>`, and nothing reading
one of them could say which of the two it held. Because a store holds exactly one run, the store is
what tells them apart, and the identifier now states both.

The consequence for evidence is stated rather than worked around. Two runs of one contract no
longer commit the same bytes: the identifier is part of every envelope's digest input, so the
digest chains of two stores differ from the first record. What remains identical is what was
committed — the same events, in the same order, under the same command identifiers — and what
identifies the contract, which is the contract digest each run's own approval records. A journal
still carries one run identifier throughout, and a reader still rejects a mixed one.

This is not a journal schema version: no field, ordering rule or digest input changed, and a
journal written under version 1 identity reads unchanged. What changed is the value the field is
derived from, for runs started by this build onwards.

## Runtime engine registry version 1

One record per runtime engine lives at `<root>/runtimes/<engine>.json`, where `<engine>` is the
name the runtimes page spells: `claude-code` or `codex`. The record states three separable things.

The **admission decision** is the operator's: `enabled`, and `disabled_reason` when it is false. A
disabled engine is refused before anything is started, and the refusal repeats the recorded reason.
It is also not probed, not offered as a route and not counted ready, so nothing about it is
measured while it is held back. Where the registry holds no record for an engine, the engine takes
its seeded state; the Codex engine is seeded disabled, because the account this build authenticates
with is over its usage limit until 2026-09-12.

The **measured properties** are written from measurements of this host and never from a
declaration: the executable discovery resolved, the release that executable reported, the digest of
that executable computed from its bytes, the origin of the credential a managed invocation would
carry — named, never the credential itself — and the bounds that invocation is held to. Discovery
has one channel and the record names its result, so an engine cannot be selected by a path no
record states.

The **model list** is what the engine can serve, with the provenance of the list. `source` is
`measured` when the installed build was asked about every candidate its own executable carries and
named the ones it serves, `filtered` when some candidate was not put to it and the list is
therefore part of what it serves rather than all of it, `pinned` when the build publishes no
catalog and the record states the route the managed profile pins instead, and `unmeasured` when
nothing has been measured. Which candidates are asked about is not decided by any rule over model
names: every identifier the executable carries is put to the build, and the build refuses what it
does not serve.

Whether a recorded list belongs to the build that is installed now is decided by
`measured_for_digest` against the digest of that executable, computed at every reading.
`measured_for_version` states the release the build reported and decides nothing, because a record
states it and a record can say anything: a record naming the installed release while holding
another build's list would otherwise suppress the re-measurement that replaces it.

The guarantee is that and no more. The digest is of the executable, not of the measurement, so a
record whose digest is honest and whose names were rewritten by hand is read as current and no
check here notices. Nothing authenticates a record.

That limit is bounded by what a model list is allowed to decide, which is nothing about admission.
Whether an engine may be started is answered by `enabled` alone, so a rewritten list cannot admit
an engine the operator held back and cannot widen what a run may start. Which of an engine's models
a run may use is not decided by this layout at all.

The record states its own `schema_version`. A record of any other version is refused when it is
read, and no command migrates a record. A record that exists and cannot be read refuses the engine
rather than falling back to the seeded state, because an unreadable record of a disabled engine
must never admit it.

The registry holds no run's state and no credential, and it decides nothing about which models a
run may use. Which of an engine's models are permitted is a separate question this layout does not
answer.

## Provider records version 1

One record per provider lives at `<root>/providers/<provider>.json`, where `<provider>` is the
account's family as a record and a command spell it: `anthropic` or `openai`. A provider is an
account with an authentication state that exposes models; it is not a runtime and not a model, and
every provider is reached through an installed engine. The record therefore states the account, and
the engines that reach it stand beneath it as its `routes`.

The record holds its own name and `family`, the operator's `enabled` decision with the
`disabled_reason` it carries, the observed `state` with the `reason` that state was observed from,
the moment of that observation in `observed_at_ms`, and one route per engine. A route names the
engine, its admission decision, the executable and release measured for it, the digest of that
executable, and where its credential is read from — named, never the credential itself. Those
fields are copies of what the engine record held when the observation was made, and they are held
for one purpose: they say which engine build the provider state was observed from, so an
observation older than the installed build is visible as older rather than standing for it. Every
decision is taken from the engine record, never from the copy.

`enabled`, `disabled_reason` and `observed_at_ms` were added to the layout after it was first
written, as optional fields with defaults, and the version is **not** raised for them: a record an
earlier build wrote reads back as a provider nobody has enabled whose observation carries no
moment, which is what it is, and a record this build writes is read by an earlier one as the record
it already understood. An addition no reader has to understand is not a new layout.

An absent moment is not an absent measurement, and the two are answered apart. A record holds an
observation when it holds routes, because an observation is what writes them, so a record an
earlier build wrote states that it was measured and that the moment is unknown rather than that
nothing has been measured. The same distinction holds for the operator's decision: **disabling
keeps every measurement the record holds**, so a surface reads what was measured from the
observation and reads whether it is offered from `enabled`, never one from the other.

`enabled` is the operator's decision and the only field of this record nothing measures. **A
provider is not measured before it is true**: no engine that reaches it is started, no network is
reached, and reading the supported list or the catalog measures nothing. It is also the disclosure
consent — enabling permits repository content of any workspace to be sent to that account — which
is why it is stored beside the measurements rather than derived from them. Disabling keeps every
measurement the record holds, because the measured reason is the answer to "why is this model not
offered".

`observed_at_ms` is the moment the observation was taken, in milliseconds since the Unix epoch, so
a surface states the age of what it shows instead of presenting a measurement of any age as
current. A record stating a moment the reading host has not reached is stated as one that cannot
be dated rather than as one taken a moment ago, because an age errs towards staleness and never
towards freshness. It is the moment the engine records were read, which is also the moment they
were measured, because measuring the engines and observing the providers are one act. An engine measured
again afterwards by another surface makes the provider observation older than the measurement it
states, never newer: the age errs towards staleness and never towards freshness.

A provider record's measured half is observed and never seeded. It is written by one operation,
which reads the engine records under the same root and writes what they state; it starts nothing,
spends nothing and reaches no network. A root on which nothing has been enabled or observed
therefore holds no provider record at all, and a read answers that the record is absent rather than
inventing a state for it; a surface reads such a provider as disabled and unmeasured. An engine
record is seeded because its enabled flag is an operator decision with a product default; a
provider's state is measured throughout and has no default.

The state is the first of these that applies, in the order an operator can act in: `unavailable`
when no engine that reaches the provider is both admitted and installed here — including when an
engine record could not be read at all, which is recorded as that route's reason and never as a
working route; `not configured` when an admitted engine reported a release and nothing states where
a credential would be read from; and `ready` when an admitted engine reported a release and states
its credential origin. The design's fourth row state, `needs authentication`, is not written by
this build: nothing here authenticates against a provider, so no measurement separates a credential
the account rejects from one it accepts, and a record claiming that state would state the result of
a check no code performs.

`ready` means that a spend could reach the account, not that the account accepted anything. The
record states where a credential is read from and never whether the provider honours it.

The model catalog — the triples of provider, engine and model the design calls catalog entries — is
derived and is not a fourth stored object. The model names live in the engine records and the
attribution lives in the provider records, and the two are joined when the catalog is read; a
stored union would be a copy of both, and a copy is what goes stale while reading as current. An
entry that is not admissible is present and carries the measured reason rather than being dropped.
An engine record that cannot be read fails the whole reading, because a catalog silently missing
one engine's models is indistinguishable from a complete one.

The reading also carries **one row per engine that reaches an observed provider**, whether or not
it serves a model. An engine record that is removed from the root is answered by the registry's
seeded record, whose model list is empty, so its entries would otherwise stop appearing and a
caller counting them would read a catalog missing everything that engine served as a complete one.
The route stays, states that it serves nothing and states why — no record under this root, an
engine held back, or a list nothing has measured — and every surface draws those rows beside the
entries.

The record states its own `schema_version`. A record of any other version is refused when it is
read, and no command migrates a record. A record standing under another provider's name is refused
rather than read as that provider's.

This level decides two things and no more: whether a provider may be measured at all, and whether
its entries are offered by the catalog. Which engines are **admitted** is still answered by the
engine record's `enabled` flag alone, so a provider observed unavailable does not hold back an
engine the operator enabled and an observed provider does not admit one they disabled. Nothing is
instantiated by being recorded here, and which of the catalog's entries a run may use is not
decided under this root.

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

## Event journal version 3

Version 3 keeps every envelope field, ordering rule, digest input and limit of version 2 and adds
two event tags, which are what makes the facts of the commitment kernel durable:

- `commitment_kernel_opened`, carrying `root_participant`, `root_principal`, `root_obligation` and
  `budget` — the state a ledger is built from. None of it is derivable from the facts that follow,
  so a ledger without this record could not be rebuilt at all;
- `commitment_facts_recorded`, carrying `facts`: every fact one commitment command committed, in
  the order the kernel committed them.

One command is one record. A commitment command commits all of its facts or none of them, and a
record holding the whole command is what keeps that true across a restart: a reader either has the
command entire or does not have the record, so no recovery rebuilds a ledger holding half an award.
The record is written before the command's result is returned, so a caller told that escrow moved,
that a contract was formed or that an obligation was returned is holding something the journal
already states.

A ledger is reconstructed from these records and from nothing else. The genesis record builds it
and each `commitment_facts_recorded` record is replayed into it in journal order, by the same fold
the live run uses, so the ledger a restart reaches and the ledger the run held are the same reading
of the same records. No live state is carried across, and no projection beside the journal is
consulted. A fact that cannot be replayed refuses the recovery rather than producing a ledger the
record does not support.

The facts themselves are the commitment kernel's own, unchanged: identifiers it compares, integer
quantities it subtracts, deadlines it compares against its clock, and digests of inert content it
stores without opening. Nothing in the record states what work means or who deserves it. A run that
opens no kernel writes neither tag and reads back exactly as it did under version 2.

## Event journal version 4

Version 4 keeps every envelope field, ordering rule, digest input and limit of version 3. What it
adds is inside `commitment_facts_recorded`: four further commitment facts, which are what makes the
ancestry of a result durable.

- `object_recorded`, carrying `object_digest` — an immutable object is stored whole and may be named
  by a bundle. An attempt that stopped part-way through writing one never commits this fact, so
  nothing half-written can enter a result;
- `bundle_recorded`, carrying `bundle_digest`, `base_digest`, `parents` and `changes` — the
  immutable content of one submission bundle. The identifier is the digest of that content, so the
  same changes against the same base are one bundle whoever published them;
- `candidate_formed`, carrying `candidate_digest`, `content_digest`, `contract_id`,
  `obligation_id`, `participant`, `generation`, `base_digest`, `bundle_digest`, `contributions` and
  `changes` — one immutable result and everything needed to reproduce its construction. The two
  digests are computed by the kernel from those fields, so a record that does not reach its own
  identifier is detectable from the record alone;
- `conflict_recorded`, carrying `conflict_digest`, `base_digest`, `candidates` and `paths` — where
  named results put different bytes at the same path. It is evidence and resolves nothing.

A result's identity is a function of its construction and of nothing beside it: no workspace
directory, no process, no branch name and no row in any store takes part in it. The same
construction stated on another host reaches the same identifier.

Both submission paths seal the task contract they are recorded against. A contract records one
result; the same result submitted again states the same fact, and a different one is refused with
both identifiers named.

The record limit is unchanged and is what the kernel's own bounds are set against: one submission
commits its bundle and its result together, so a bundle carries at most 32 path changes, a result at
most 48, a path at most 256 bytes, and a bundle at most four contributions. The largest submission
those bounds admit is about 56 KiB, inside the 64 KiB a record holds. They bound what one submission
states at once and not how large a tree may be, since a tree of any size is named by the digest of
its base.

The version is raised rather than treated as an extension because a version-3 reader given these
tags fails on an unknown fact rather than on a stated version, which is the failure this policy
exists to prevent. A run that opens no commitment kernel writes none of them and reads back exactly
as it did under version 3.

## Compatibility and migration

Schema version 1 is immutable, and versions 2, 3 and 4 are new versions rather than extensions of
what came before. A change that alters field meaning, digest input, event tags, required fields,
ordering rules, or replay behavior requires a new schema version. The current binary reads and
writes only version 4 and fails closed on every other version, versions 1 to 3 included.

The migration consequence is stated rather than worked around: a journal written by an earlier
binary is rejected at open with an unsupported-schema error, and no command migrates it, because
no migration tool exists. Such a store remains inspectable as raw evidence and cannot be resumed,
extended or exported by this binary; a new run needs a new data root. Evidence exported from a
store of an earlier version keeps its own bytes and is not rewritten.

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

A recorded cost is kept only while it is attributed to the models that produced it. A Claude result
that states a cost must carry the per-model breakdown the runtime reports beside its total; the
product checks that the breakdown adds up to that total, allowing only the rounding of each share
to whole microdollars, and that no model outside the admitted profile spent anything. A cost with
no breakdown at all, a total its own breakdown does not support, and consumption by an unadmitted
model are typed protocol failures rather than recorded costs, because a matched-budget comparison
reads the recorded number as evidence of one model route and an unattributed number is not that
evidence. A turn that reports no cost has nothing to attribute and is recorded as before.

The attribution is carried by the record and not only by the rule that admits it. Every terminal
record states, beside the total, the models that produced the cost and what each of them spent,
sorted by model name and holding one entry per model across all of a run's turns. A reader tells an
attributed cost from an unverified one from the record alone: a recorded cost whose named shares add
up to it — allowing only the rounding of each share to whole microdollars — is attributed, and a
recorded cost with no named share is not. The breakdown is filled only from what the runtime
reported. It is never completed from the admitted profile, because a model name the runtime did not
state is not evidence of the route that spent the money, so a runtime that reports a total without
naming a model produces a record that states the total and names nothing. A record written before
this field reads back as naming nothing, which is what such evidence in fact does.

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

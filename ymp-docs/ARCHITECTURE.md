# Architecture

## Boundary: semantic organization versus mechanical constitution

| Participants and local task sponsors decide | Trusted kernel enforces |
|---|---|
| whether to work alone, recruit, bid, delegate, compete, or stop | authenticated and valid state transitions |
| temporary roles, task meaning, decomposition, and method | atomic formation and expiry of local commitments |
| which bid to accept and which findings to trust or challenge | escrow, budget conservation, attribution, and bounded payloads |
| what runtime profile to request | non-semantic capacity admission and capability limits |
| what patch or artifact to submit | private workspaces, base-digest matching, immutable candidates |
| which candidate merits a scarce verification query | exact-digest verification and protected-oracle isolation |
| how to react to rejection or conflict | obligation accounting, finite resources, and honest terminal states |

The kernel is deterministic where races, accounting, containment, and reproducibility require
it. It has no project planner, semantic priority function, role catalogue, skill matcher,
escalation tree, candidate ranker, or answer synthesizer.

## Stack and deployment boundary

- **Distribution:** one Rust executable named `ymp`; no separate ymp service, helper binary,
  database server, ymp-owned language runtime, container runtime, or external orchestrator is
  required.
- **Current application:** running `ymp` starts the foreground ratatui interface and constructs the
  trusted application core, kernel writer, local store, and runtime supervisor in the same process.
  This is the only user-facing mode in POC, MVP, and Alpha.
- **Private child modes:** the same installed executable may run an attempt tool bridge, native
  runtime worker, isolation initializer, model relay, or verifier worker. External Claude Code,
  Codex, and later OpenCode processes are launched and supervised by the foreground application.
- **State:** the POC keeps authoritative live state in memory under one writer and records a bounded
  JSONL event journal plus atomically replaced run metadata. An embedded database is an optional
  later store implementation, not a POC dependency or protocol identity.
- **Artifacts:** a local content-addressed directory for source, submission, candidate,
  environment, oracle, and bounded log objects.
- **Agent runtimes:** user-installed Claude Code, Codex, and OpenCode programs are untrusted
  external harnesses, not ymp dependencies in the sense of linked libraries or orchestration
  services. A later direct-model runtime is an `ymp` worker mode, not code in the kernel writer.
- **Strict execution:** supported Linux kernels only in the initial release. Native macOS is a
  separately labelled best-effort profile.
- **Future interfaces:** the application core exposes typed commands and events through an internal
  port. The TUI calls that port in process. A later `ymp server`, remote TUI connection, or web
  gateway may adapt it, but no external operator transport is implemented or required now.

One executable is not one process and is not one security domain. The trusted TUI and application
core deliberately share the foreground process, but untrusted runtimes, model-response parsing in
a future native harness, attempt tool bridges, and independent verification retain explicit
process, filesystem, namespace, and capability boundaries. An unexpected foreground-process exit
ends the POC run as `infrastructure_error`; the POC does not pretend to provide transparent crash
continuation.

## Single-executable local topology

```text
                         one installed file: ymp

 operator ──► ymp ratatui application
                    │
                    ├── in-process application core and kernel writer
                    │       ├── live run state
                    │       ├── event journal and atomic run metadata
                    │       └── content-addressed object store
                    │
                    ├── spawn and supervise ──► Codex App Server
                    │                         └► Claude Code
                    │                         └► later OpenCode
                    │
                    ├── private attempt boundary
                    │       external runtime ── stdio MCP ──►
                    │       ymp internal agent-mcp
                    │                 │
                    │                 └── attempt-scoped local endpoint ──► core
                    │
                    └── ymp internal verifier in a separate,
                        stricter verification boundary
```

The operator starts only `ymp`. The TUI invokes typed application commands directly and subscribes
to an in-process event stream. It presents start, inspect, message, pause, resume where supported,
cancel, archive, and evidence-export actions without an operator socket or background lifecycle.
The foreground process owns all live local runs and supervises every child it creates.

At startup the runtime catalog probes the installed Codex and Claude Code executables, their
versions, authentication readiness, models, and required capabilities. The TUI reports unavailable
or unauthenticated profiles before a run starts. When the operator selects a ready profile, ymp
creates and supervises the required child process itself: Codex through `codex app-server` over its
default stdio JSONL transport and Claude Code through its supported programmatic interface. The
operator is never asked to launch an agent runtime in another terminal. Installation and provider
authentication remain explicit prerequisites rather than binaries or credentials bundled into ymp.

`send` appends an attributed human collaboration message to an authorized audience. It cannot
alter the approved contract, grant authority, choose a bid, or forge an agent message; a semantic
change requires a new approved contract lineage. Any unscheduled human message marks the run as
intervened and excludes it from autonomous POC comparisons. `archive` removes a terminal run from
default views but does not rewrite its ledger or bypass retention policy.

The local data root has a single-writer lock. A second `ymp` process may inspect no live state and
must either choose a different data root or report that another foreground application owns it. It
never kills or replaces that process. Multi-client attachment and detached background execution are
future server-mode concerns, not implicit behavior of the local TUI.

Names below `ymp internal` are private implementation details. An attempt cannot reach the
TUI command port, authoritative store, data directory, or another attempt's endpoint. Its tool
bridge receives only an attempt-scoped endpoint and can exercise no authority beyond the
controller-held grants for that attempt. The launcher exposes the exact running `ymp` executable to
the external runtime at a read-only absolute path and records its digest; the workspace cannot
substitute another bridge.
The bridge is a translator, not a trusted store client.

No ymp process loads Paseo libraries, invokes the Paseo executable, connects to its daemon, or
reads its state. Similarities in lifecycle shape are independent implementations of general
process-supervision patterns.

The POC uses this foreground topology directly. The researcher-provided disposable harness supplies
outer process and oracle separation. The POC uses the same executable, domain commands, event
journal, immutable objects, private attempt directories, and stdio MCP tool binding, but it does
not implement or claim the strict isolation initializer, credential relay, hardened verifier, or
crash continuation. Those become MVP work only after the coordination hypothesis passes.

## Local state and storage seam

The main application core serializes accepted state-changing commands under one writer. All
authoritative identifiers are stable random or time-sortable identifiers; a PID, local path, file
offset, database row number, or current process is never a protocol identity. Monetary and token
amounts use integer smallest units.

For POC, live projections are in memory. Before returning a successful state-changing tool result,
the core appends the bounded control event and command-result association to `events.jsonl` and
synchronizes the record according to the experiment profile. `run.json` is an atomically replaced
summary and index, not a competing source of truth. Duplicate delivery during the live run returns
the recorded result from the command table. After a controller crash the outer study harness marks
the run `infrastructure_error`; a new process may inspect or export the journal but does not resume
the run as if no authority interval was lost.

Every journal envelope carries a schema version, run identifier, monotonically increasing sequence,
event identifier, optional command identifier, predecessor digest, payload digest, and record
digest over a bounded canonical encoding. `run.json` records the last synchronized sequence and
head digest, and the evidence export repeats that head digest. Readers reject gaps, duplicate
sequences, digest mismatches, and an incomplete final record; they preserve the original bytes for
diagnosis and report `infrastructure_error` rather than repairing experimental history.

Large payloads do not pass through event records. The object store writes a temporary object,
verifies its digest, synchronizes it to durable storage, and atomically renames it before a control
event may reference it. A crash can therefore leave an unreferenced object for later garbage
collection, but never an authoritative record pointing to partially written bytes. Control,
collaboration, and verification records have separate schemas, access modules, projections, and
export rules. Neither an agent process nor an MCP bridge opens the journal or object directory
directly.

The platform-specific data root contains only external data, conceptually:

```text
data/
  objects/<algorithm>/<digest>
  runs/<run_id>/
    run.json
    events.jsonl
    workspaces/<attempt_id>/
    logs/

runtime/
  runs/<run_id>/attempts/<attempt_id>.sock
```

Socket paths live in a user-private runtime directory and are never treated as durable records.
Workspaces and logs have recorded retention policies. Object garbage collection considers only
objects unreachable from durable records and uses a grace period so it cannot race an in-flight
write. Applying an accepted candidate back to the user's working tree is a separate explicit
operator export; the application never treats that working tree as its state store.

The store is a typed port rather than a promise that JSONL is permanent. If MVP requirements show
that foreground restart must resume live authority, or that history queries and backup cannot be
implemented safely with bounded files, an embedded SQLite implementation may replace the POC store.
It must preserve the same command, event, idempotency, and object-ordering tests and must not require
a daemon. A future server mode may use the same local store implementation; a future cluster uses a
different transactional store.

The source may be organized as several internal Rust library crates for auditability, but the
release build produces and installs only one ymp-owned executable. Store migrations if an embedded
database is later adopted, agent-runtime drivers, coordination-tool schemas, isolation policies,
and verifier code are embedded into that artifact; there
is no runtime-loaded ymp module directory.

## Three-plane trust topology

ymp separates three planes even if the first implementation stores their records under one local
run directory:

1. The **trusted control plane** contains contracts, budgets, capability grants, task contracts,
   obligations, leases, candidates, and authoritative transitions. Only the kernel writes it.
2. The **untrusted collaboration plane** contains task-scoped natural-language messages,
   findings, challenges, and inert projections or references for offers and bids. Storage
   integrity and attribution are trusted; payload truth and intent are not.
3. The **verification and security plane** contains protected oracle material, isolation and
   broker policy, boundary evidence, and verifier results. Participants cannot write it, and an
   independent first assessment cannot read collaboration content.

The planes use distinct schemas, capability namespaces, readers, writers, and export policies.
A board message can refer to a control object but cannot mutate it. No payload is executable, no
URL is fetched merely because it is mentioned, and no capability can be delegated through text.
This keeps useful communication observable without making the board a command-and-control path.

## Participants, attempts, and bootstrap

A **participant** is one resumable agent-runtime session with a principal identifier, a versioned
runtime profile, and a locally owned budget account. An **attempt** is one bounded period in which a
participant works on a named task and base snapshot. An **invocation** is one supervised process
slice that starts or resumes that participant inside an attempt. For an external harness this is
the harness process tree; for a direct-model runtime it is an `ymp internal runtime-worker`
process. A participant may complete several sequential attempts, and an attempt may contain
several invocations separated by explicit yields. Concurrent attempts never share a writable
workspace.

These identities are deliberately independent. Participant parentage records recruitment;
placement records which execution backend ran an invocation; workspace ownership belongs to an
attempt; and a runtime session identifier is driver-private continuity data. Moving an attempt
or replacing a process never changes its obligation lineage. This distinction is required for
crash diagnosis, any later recovery implementation, and future worker placement in Kubernetes.

Every run needs an initial condition. Configuration names an **origin participant** and gives it
the root contract and a finite root branch budget. This is not a permanent orchestrator role:
the participant may do the work itself, advertise parts, recruit peers, fund competing branches,
transfer sponsorship, or expire. The kernel does not infer which organization should follow.

Starting a dormant agent runtime merely to ask whether it wants a task is not free. Recruitment therefore
has two explicit reservations:

1. a small sponsor-funded proposal budget lets a new participant inspect an offer and bid or
   decline; and
2. an execution escrow is transferred only when sponsor and bidder form a task contract.

Already active participants may bid using their own communication allowance. Agent count is an
outcome of local resource decisions, bounded by the global runtime-start and concurrency
budgets.

## Execution model

ymp separates four concepts that a single “provider adapter” would incorrectly conflate:

| Concept | Responsibility | Examples |
|---|---|---|
| **Agent runtime** | Owns one agent harness, its reasoning and project-tool loop, session continuity, events, and cancellation. | External Claude Code, Codex, or OpenCode; a future ymp-native direct-model harness. |
| **Runtime driver** | A compiled `ymp` implementation that probes and supervises one runtime protocol. | Process-per-turn CLI, Codex App Server client, OpenCode HTTP/SSE client, native API worker. |
| **Model route** | Names the actual provider, deployment and endpoint class, wire protocol, account or quota scope, model identifier or snapshot, authentication mode, and approved data-disclosure class. | NVIDIA Nemotron through hosted NVIDIA API or local NIM using OpenAI Responses, Chat Completions, or Anthropic Messages; an OpenAI Responses route. |
| **Coordination-tool projection** | Maps the versioned ymp command schema to tools understood by the runtime without owning authority. | stdio MCP or provider-native typed function calls. |

A versioned **runtime profile** composes an exact driver and external harness digest where
applicable, harness and prompt policy, one model route, one coordination-tool projection, and an
execution-assurance profile. This profile is provenance and a mechanical admissibility boundary,
not an agent role. A participant or local sponsor may request an approved profile in an offer or
bid; the kernel checks availability, capabilities, disclosure policy, and reserved resources but
does not rank profiles, match them to task meaning, or choose a “best” model.

External runtimes retain their own internal reasoning loops. A future direct-model runtime would
instead implement a minimal harness and tool loop inside a restricted `ymp internal runtime-worker`
process. It must never parse model output or execute model-requested tools inside the trusted
kernel-writer process. Direct API access is therefore another runtime implementation, not a
shortcut around attempt isolation, command authorization, provenance, or matched-budget accounting.

For each invocation the driver:

1. probes the exact runtime and model-route versions and records lifecycle, event, tool,
   cancellation, authentication, usage, and cost capabilities separately;
2. materializes or reopens the attempt's private writable workspace from its immutable base digest;
3. attaches the task contract, attempt budget, event cursor, and controller-held capability set;
4. constructs an explicit process environment, synthetic home, generated runtime configuration,
   and coordination-tool projection that exclude ambient hooks, plugins, user settings, and
   unrelated tool servers in strict mode;
5. enters the operating-system isolation boundary before untrusted runtime or project code starts;
6. starts or resumes the runtime session and delivers a bounded event manifest;
7. supervises descendants, structured runtime events, model requests, and externally measurable
   resource use;
8. ends the invocation on return, explicit yield, cancellation, limit, process exit, or
   infrastructure failure; and
9. revokes invocation-scoped endpoints. The workspace survives only while the attempt remains
   funded and resumable, then is destroyed.

Session resumption, cancellation, prompt delivery, configuration isolation, authentication, and
trustworthy usage telemetry remain runtime- and route-specific. A profile that cannot prevent
ambient credential inheritance or enforce a declared hard cost ceiling is not eligible for that
strict claim.

The portable external-runtime baseline is a sequence of bounded non-interactive turns, not a
permanently running terminal. Codex exposes structured `exec` events and explicit session resume;
Claude Code exposes non-interactive `stream-json`, explicit session identifiers, resume, and a
bidirectional input mode; OpenCode exposes `run --format json`, explicit sessions, selectable
`provider/model` routes, and local MCP configuration. Richer runtime protocols may be used only
behind a capability-gated driver. OpenCode's headless HTTP/SSE server and Codex App Server are
optional, attempt-scoped integrations rather than shared local services. Terminal text and ANSI
screen scraping are diagnostics, never the authoritative completion protocol.

An invocation can call `yield` with mechanically defined wake conditions such as direct message
delivery, a child obligation return, a requested verification result, cancellation, or a deadline.
Yield ends the process slice but not the attempt or task contract. The foreground controller
coalesces matching events and may resume the runtime session only while its lease, start budget, and wake deadline
remain valid. This avoids token-consuming polling without letting a dormant participant keep a run
alive indefinitely. The participant chooses what events matter; the kernel does not interpret
message content or invent a next task.

A message arriving during an active model turn is committed and queued after the participant's
event cursor. The portable baseline does not inject text into runtime stdin or interrupt an
in-flight inference. The participant sees the event on an explicit read or at its next invocation.
A runtime-native mid-turn delivery feature may be tested as a separate driver capability, but
the run records it and no correctness property depends on it.

## System components and trust separation

### Control kernel

The kernel validates proposed commands, reserves budget vectors, forms mutually agreed task
contracts atomically, issues leases and fencing tokens, tracks work obligations, and records
terminal outcomes. It accepts or rejects transitions solely against mechanical preconditions.

Only the kernel may mutate authoritative projections, issue capability tokens, reserve or
transfer budget, close an obligation, or record verifier evidence. This exclusive write
authority is about consistency, not project judgment.

### Contract and oracle store

The store holds:

- the human-approved public `PROJECT.md` digest;
- the protected acceptance bundle and approval record;
- visible development checks;
- the immutable source and environment digests; and
- the diagnostic-disclosure and verification-query policies.

Agents can read public semantics and visible checks. They cannot mount protected checks or
write oracle state. Hidden cases may instantiate only public requirements; they may not add
secret requirements. Approval and oracle validation are specified in
[PROJECT-CONTRACT.md](PROJECT-CONTRACT.md).

### Trusted control ledger

The append-only control ledger is the source of truth for shared effects. The first implementation
has one kernel writer and a total order per run. Cross-process delivery is at least once; commands
therefore carry idempotency keys and produce exactly-once effects during a live run. Derived control
projections can be rebuilt without reading conversation payloads.

Control records include reservations, task contracts, obligations, leases, capability grants and
uses, candidate construction, verification attestations, and terminal states. A participant may
propose a transition but cannot append an authoritative record directly.

### Scoped collaboration board

The board is an append-only untrusted publication module. It exposes attributed natural-language
messages and typed summaries alongside read-only references to selected control objects. The
kernel validates authorship, audience, size, lifetime, reference syntax, and communication cost,
but not truth, importance, intent, or technical validity.

Detailed messages are visible only to a task, a candidate-review group, or another explicitly
admitted audience. A bounded project-discovery scope may announce that work or help exists, but it
does not disclose full findings by default. Sponsors and participants choose whom to invite within
their authority; active participants may request admission after seeing a discovery notice. A task
contract supplies minimum sponsor-contractor membership automatically. Audience grants expire and
consume finite membership capacity. The kernel checks scope and authority but does not choose
collaborators.

Board payloads are inert bytes. They cannot carry a capability token, cause a tool invocation,
grant membership, form a task contract, close an obligation, or trigger integration or
verification. Those effects require separate authenticated control commands. A recorded delivery
receipt proves only that bytes were made available, not that a model read or understood them.

Audit history and payload digests never evaporate. Active salience does: offers and finding
projections have a finite lifetime and must be refreshed at a communication cost. This separates
durable evidence from stale coordination cues without making repetition free.

### Local-contract module

Any participant with an obligation and budget may sponsor a task offer. The module supports:

- open or targeted offers;
- bids and counter-proposals;
- sponsor-selected awards;
- pre-authorized first-accept offers where the sponsor deliberately chooses that policy;
- transfer of sponsorship; and
- expiry, withdrawal, return, or failure.

A contract forms only from compatible offer and bid records. The module does not score bids.
The sponsor's selection is an agent action, recorded for later analysis. “Manager” and
“contractor” are temporary relationships scoped to one task.

### Obligation tracker

Delegation diffuses work through the system. Each accepted child contract creates a unique
obligation linked to its parent. A parent obligation cannot close successfully until every child
has returned a result, been incorporated by an explicit synthesis action, or been cancelled.

The causal obligation lineage remains a tree even when task dependencies and candidate ancestry
form directed acyclic graphs. This permits unambiguous termination accounting without dictating
the semantic shape of decomposition. Each created obligation consumes one finite authority unit,
so infinite zero-cost decomposition is impossible.

### Agent-runtime driver and model route

The runtime driver launches and supervises invocations. It reports capabilities instead of
pretending every harness or route has the same lifecycle. It must:

- clear inherited environment variables and close inherited file descriptors;
- provide an empty attempt-specific home and no host agent sockets or credential stores;
- generate an invocation-specific runtime configuration instead of mutating user configuration;
- disable runtime-native subagents, background agents, remote execution, and unrelated tool
  servers unless the driver can account for them as explicit ymp participants and invocations;
- parse structured runtime events and capture the runtime session identifier without treating
  terminal prose as control state;
- inject only explicit, revocable capabilities;
- attach process, memory, CPU, disk, output, and network limits before execution;
- capture boundary actions and resource measurements;
- terminate the full descendant process tree; and
- distinguish model, infrastructure, cancellation, and policy failures.

Capabilities are reported in two matrices. Runtime capabilities include `start`, `resume`,
`interrupt`, `structured_events`, `session_capsule`, `mcp_stdio`, `native_typed_tools`,
`subagents_disabled`, `ambient_config_isolated`, `full_descendant_cancel`, and any runtime-server
transport. Model-route capabilities include `provider_and_model_pinned`, `usage_evidence`,
`hard_cost_limit`, `brokered_auth`, `approved_disclosure`, and `request_cancel`. Evidence records
the exact driver version, external executable digest and version where applicable, model route,
and probe result. A missing optional capability selects a declared fallback; a missing required
security or experimental capability makes that profile ineligible rather than silently weaker.

For the POC and MVP, runtime-native subagent or cloud-agent execution is disabled rather than
mapped. Otherwise one apparent participant could create an unobserved team, share a writable
workspace, spend outside participant-start accounting, and confound both the self-organization
experiment and the isolation claim. If a pinned runtime version cannot demonstrate that these
features are unavailable, it is not eligible for the primary POC comparison or strict mode.

Runtime drivers are compiled into `ymp`; the first release has no dynamic runtime-driver plugin
ABI and does not execute user-supplied driver code in the trusted application process. Declarative
command templates may select documented flags, but a new lifecycle parser or authority mapping
requires a new ymp build and the runtime conformance suite.

On strict Linux, the launcher places the child in its final cgroup and namespaces before any
untrusted runtime or project code executes. Supervision uses a pidfd where available and cgroup
membership for descendant accounting; cancellation sends a graceful signal for a bounded interval,
then kills the entire cgroup. A process group alone is not considered complete descendant cleanup.

### Runtime session capsule

Some runtimes persist resumable conversation state under their configuration directory; others
need only a session identifier. The driver captures the minimum runtime-specific state required
for resume as an opaque **session capsule** scoped to one participant and attempt. The capsule is
untrusted execution state, never authoritative control state. Its digest and runtime profile are
recorded, but its contents are unavailable to the board, verifier, operator evidence export, or
communication observatory.

Before suspension the driver closes the runtime process, removes invocation endpoints and
ephemeral credentials, then snapshots the remaining capsule. Local resume restores it only into
that attempt's synthetic home. A corrupt or incompatible capsule causes a typed runtime error; it
cannot alter a lease or obligation. MVP stores capsules with restrictive permissions and deletes
them by policy after the run. A later cluster backend may encrypt and move the same opaque object,
but must not turn runtime transcripts or hidden reasoning into research telemetry.

### Agent-facing coordination-tool projection

Claude Code, Codex, and OpenCode can launch a local MCP server over standard input and output. For
an external-runtime invocation, the generated runtime configuration points to the installed
executable itself:

```text
ymp internal agent-mcp --attempt-endpoint <private-path>
```

The bridge negotiates a supported MCP revision, exposes a small attempt-specific
coordination-tool set, and translates each call into a versioned ymp command. It writes only valid
MCP JSON-RPC messages to standard output and diagnostics to standard error. It has no authoritative
store, object-store, TUI command port, verifier, or cross-attempt access.

MCP is not the kernel protocol and does not own session, lease, obligation, budget, or candidate
state. A JSON-RPC request identifier is not treated as a durable idempotency key. The bridge first
registers an invocation-scoped command identifier with the foreground controller; the resulting
effect and reply mapping are committed atomically before the MCP response is returned. After an ambiguous bridge
failure, the driver reads authoritative state and never replays a state-changing operation under
a fresh identifier automatically.

The initial bridge uses only stdio. It opens no TCP or HTTP listener, accepts no principal or
capability secret in model-supplied arguments, and does not rely on a secret hidden from the agent
process. The foreground controller derives the principal and attempt from the private endpoint and
authorizes every command from controller state. An agent that imitates its own bridge therefore gains no
authority beyond the tools already granted to that attempt.

A direct-model runtime exposes the same versioned tool schemas through the selected provider's
typed function or tool-call format and translates accepted calls to the same attempt endpoint. It
does not emulate an MCP server for a provider that already supports typed calls. Binding-specific
request identifiers remain transport data in both cases; the controller's command identifier provides
idempotency. Project shell and file tools remain inside the attempt boundary and are distinct from
the coordination command set.

NVIDIA Nemotron is therefore a model route, not an agent-runtime implementation. The route may be
paired with OpenCode's NVIDIA integration, Claude Code through an Anthropic-compatible Messages
endpoint, Codex through an OpenAI Responses-compatible endpoint, or a future native runtime. Each
pairing is a separate conformance profile because nominal API compatibility does not establish
identical tool calling, streaming, reasoning fields, session state, cancellation, or usage
accounting.

### Workspace and snapshot manager

The user's working tree is an input, never a shared execution directory. The manager captures it
as source snapshot `S0` and creates one private writable workspace per concurrent attempt.
Workspaces share no writable Git metadata, dependency cache, home directory, temporary directory,
or inter-process socket. An explicitly content-addressed cache may be mounted read-only.

### Mechanical integrator

An agent submits an immutable bundle containing its base digest, patch or declared artifact set,
object hashes, task and attempt identifiers, fencing token, and producer provenance. The
integrator applies it in disposable staging and creates a new immutable candidate digest.

The integrator checks format, path authority, object hashes, and base identity. It does not
resolve semantic merge conflicts. A stale base or conflict produces typed evidence; a participant
may then sponsor rebasing, synthesis, or an alternative branch.

Candidate history is a directed acyclic graph. There is no mutable canonical branch during a
run, so two agents cannot silently undo one another's work.

### Verifier

An authorized participant requests verification of a candidate and spends one protected query
reservation. The kernel does not choose or rank the candidate.

The verifier executes the approved oracle against the exact candidate, contract, environment,
and oracle digests in a separate disposable isolation boundary. It has no agent credentials, no
collaboration-board capability, and no external network unless verification explicitly requires a
brokered capability. Its controller reads the minimum exact inputs named by the verification
request rather than the producer's conversation. Project code and project tests are untrusted
inputs.

The result contains machine evidence and a bounded diagnostic projection. Infrastructure failure
does not count as rejection. Protected checks are not a perfect secret when arbitrary candidate
code must execute near them; strict projects should prefer black-box randomized checks whose
generator and expected results remain outside the candidate process.

The verifier controller signs or otherwise authenticates the evidence in its own plane; the kernel
records only the attestation and digest as the authoritative control transition. Neither component
can substitute participant conversation for an oracle observation.

Agent review, if required, is a separate blinded attempt. The reviewer first records a digest of
its independent assessment without candidate rationales, board history, or prior votes. Only after
that commitment may the review policy reveal selected challenges for a second phase. Consensus
never substitutes for machine evidence or a human criterion named in the contract.

### Capability and network broker

The broker mediates actions whose authority must be counted and revoked: coordination-tool
commands, runtime starts, verification queries, credential use, dependency retrieval,
model-provider access, and
delivery to external systems. Direct attempt network access is denied in strict mode.

Every grant is bound to a subject, task contract, action class, object scope, expiry, and resource
reservation. Capability material is not accepted from collaboration payloads and is not
transferable between participants. A boundary request records its initiating subject and causal
control command; citing another participant's message never changes the requester's authority.
This prevents textual delegation. A participant can still misuse authority that it legitimately
holds after following malicious advice, so the first cut denies irreversible external actions and
keeps broker scopes narrow enough to review mechanically.

Allowing a model-provider endpoint necessarily permits project data to be sent to that provider;
host allowlisting is not a confidentiality control. The contract must authorize that disclosure.
Long-lived provider credentials stay outside attempts where a runtime profile can use a broker;
otherwise
only a short-lived attempt-specific credential is injected.

### Non-semantic admission

Finite host slots are admitted using per-principal round robin among mechanically valid requests,
subject to reservations and rate limits. Admission does not inspect task text, skill tags, model
quality, bid content, or anticipated value. This supplies an explicit fairness assumption without
becoming a dispatcher.

### Operator interface

The ratatui interface is the current product surface and calls the application core through an
in-process adapter. It presents obligation and candidate graphs, local contracts, budgets, boundary
events, verifier outcomes, task-scoped conversation, and terminal reasons. Human actions include
pause, cancellation, contract-version approval, explicit budget changes, and evidence export. TUI
widgets consume projections and issue typed commands; they do not implement kernel transition
rules. A future external client must use another adapter to the same port rather than duplicating
those rules.

### Communication observatory

The observatory is a read-only analytical component, not part of admission, allocation, or
acceptance. It can present:

- task-scoped conversation with an explicit untrusted-data label;
- a message-to-decision-to-artifact-to-verification provenance graph;
- independently committed positions and later revisions;
- challenges, unresolved disagreements, and evidence references;
- changes in locally chosen roles and communication topology over time; and
- offline results from message-removal, replacement, shuffling, and participant-loss experiments.

Explicit references and temporal order establish provenance, not causal influence. Causal claims
come only from repeated controlled interventions under matched budgets. The observatory stores
published summaries and externally visible actions; it does not request hidden chain-of-thought or
reward an agent for producing a persuasive narrative.

## Immutable candidate flow

```text
approved public contract ───────┐
protected oracle bundle ────────┼─────────────────────► verifier
source snapshot S0 ─────────────┘                          ▲
       │                                                   │ exact digest
       ├─► private attempt A ─► bundle A ─► integrator ─► S1
       │
       └─► private attempt B ─► bundle B ─► integrator ─► S2
                                                            │
                                      local synthesis task ─┤
                                                            ▼
                                                            S3
```

`S0`, `S1`, `S2`, and `S3` are immutable. Verification evidence identifies all four relevant
digests: contract, candidate, environment, and oracle.

## Coordination flow

```text
                    trusted control plane
human-approved root ─► root obligation ─────────────────────────────┐
                              ▲                                    │
                              │ authenticated offer / award         │
                              │                                    ▼
origin participant ───────────┼───────────────► contractor attempt
        │                     │                       │
        │ inert messages      │ control command       │ submission
        ▼                     │                       ▼
  scoped collaboration board │                immutable candidate
        ▲                     │                       │ exact digest
        │ bid / challenge     │                       ▼
  invited participants ──────┘              verification/security plane
```

Only arrows entering the control or verification plane can change authoritative state, and the
kernel validates each of them. Board arrows carry untrusted information only. The kernel does not
decide why an offer, bid, message, submission, or verification request should exist.

## Consistency, crash termination, and quiescence

- Durable append precedes acknowledgement.
- Commands are idempotent; resource charges and returns occur exactly once during a live run.
- Every working lease has an expiry and monotonic fencing token.
- Work under an old token may be preserved as a stale artifact but cannot close an obligation or
  create a current candidate.
- A POC controller crash ends the run as `infrastructure_error`. The event journal may be inspected
  or exported after restart, but the process does not reissue leases or continue budget transfers.
- If MVP later claims crash continuation, its store must rebuild authoritative projections before
  accepting commands, fence every pre-crash lease, and rebuild collaboration projections
  separately so they cannot repair or alter control state.
- Quiescence requires no live attempt, admitted runtime, unexpired task contract, pending
  verifier operation, or open descendant obligation under the root.
- Quiescence plus no funded valid transition yields `exhausted`; it never implies acceptance.

These consistency claims assume an intact kernel, append-only control records, content-addressed
store, isolation runtime, and monotonic clock. Byzantine corruption of those trusted components is
outside the first fault model and must be reported as loss of assurance, not automatically repaired.

## Initial platform boundary

Strict mode initially targets one supported Linux host. The single executable calls Linux kernel
facilities directly to create per-attempt user, mount, PID, IPC, and network namespaces, delegated
cgroups, seccomp filters, and an additional filesystem policy such as Landlock. A startup
preflight proves that the required facilities and privileges are available; strict mode refuses
to start rather than silently weaken the profile. The host kernel remains in the trusted computing
base, and the measured residual shared-kernel risk is explicit.

Native macOS subprocess confinement is best-effort because the general command-line sandbox
interface is deprecated and App Sandbox is built around signed applications, entitlements, and
declared helper tools. The first one-executable release therefore makes no strict macOS claim and
does not bundle a virtual-machine image or require a virtual-machine manager. A later externally
configured execution backend may provide a Linux or remote boundary, but that is a separate
profile with its own evidence.

## Future client/server and web seam

The application port separates domain commands and events from ratatui widgets, but it is an
in-process boundary in POC, MVP, and Alpha. Preserving that separation is the only current work for
future interfaces. There is no network serialization, operator authentication protocol, browser
session, or multi-client event-recovery implementation in the current milestones.

If later evidence justifies detached or remote operation, `ymp server` may host the same application
core and store while the ratatui application selects either embedded or connected mode. A web
gateway may adapt the same commands and projections. Embedded TUI mode remains the default and must
not depend on the server. The external mode requires its own compatibility, authentication,
authorization, origin, replay, flow-control, and multi-client race tests; an in-process trait alone
does not prove those properties.

## Future cluster seam, not first-version scope

Cluster execution is deliberately absent from the POC, MVP, and Alpha unless local
evidence justifies it. The local implementation nevertheless defines four narrow ports around the
same domain protocol:

- a transactional control store with compare-and-swap, idempotency, ordered per-run events, and
  fencing;
- a digest-addressed immutable object store;
- an execution backend that starts, observes, interrupts, and cleans one invocation; and
- an authenticated command and event transport.

The in-memory/file-backed POC store, local object directory, Linux subprocess launcher, in-process
TUI adapter, and attempt-scoped Unix sockets implement those ports locally. An embedded SQLite
store may replace the POC store during MVP without changing domain commands. A separately approved
`ymp server` mode may add an authenticated operator transport without changing embedded TUI mode.
A later Kubernetes deployment can run the same `ymp` executable in controller, worker, MCP-bridge,
and verifier modes, replace the local transactional store with PostgreSQL, replace local objects
with an object store, and place attempts in pods. SQLite on a shared volume is explicitly not a
cluster design.

The first clustered control model should use one elected active controller with fencing while
allowing multiple stateless API replicas and worker replicas. Active-active side-effect execution
is deferred until serializable transactions and failover tests justify it. Workers never decide
lease validity from their local clocks; the authoritative store issues fencing generations and
the controller rechecks them at commit. Network transport in that mode requires authenticated,
encrypted ymp RPC and workload identities. It is not MCP, and it does not change command
semantics or grant the cluster a semantic scheduler.

## Open implementation experiments

- Select and attack-test the smallest auditable Linux isolation stack.
- Determine which runtime authentication modes can keep long-lived credentials outside attempts and
  which require an explicitly weaker profile.
- Validate lifecycle, configuration isolation, structured-event, tool-binding, cost, and
  hard-cancellation capabilities against pinned Claude Code, Codex, and OpenCode versions.
- Probe NVIDIA Nemotron routes separately through OpenCode/NVIDIA, Claude Code/Anthropic Messages,
  Codex/OpenAI Responses, and a minimal direct Rust client. Promote only pairings that preserve
  required streaming, tool-call, cancellation, usage, and model-identity evidence.
- Compare bounded process-per-turn integration with richer runtime-native protocols without
  exposing runtime-specific lifecycle semantics to the domain protocol.
- Crash-test event-journal/object ordering, foreground-controller failure, and MCP reply loss;
  repeat the same suite for an embedded database if MVP adopts one.
- Measure the cost of suspending and resuming sponsor and contractor sessions.
- Determine how little protected-check feedback can preserve debugging utility without enabling
  adaptive oracle extraction.
- Compare open claim, local bidding, and direct mutually accepted offers under matched budgets;
  the protocol enables all three but does not assume which is best.
- Measure whether task-scoped boards preserve useful discovery while limiting false-message and
  authority-laundering blast radius.
- Validate that the observatory can reconstruct provenance without exposing protected oracle data
  or being mistaken for a causal estimator.
- Keep cluster ports under contract tests, but do not implement PostgreSQL, object storage, or
  Kubernetes placement before the local Alpha decision.

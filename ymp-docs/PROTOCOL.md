# Coordination protocol

The protocol is a constitutional interface for local self-organization. Participants propose
offers, bids, commitments, delegations, findings, submissions, and verification requests. The
trusted kernel accepts a command only when it preserves mechanical invariants. It never judges
the technical value of the command.

## Principles

1. **Local commitments, not global assignment.** A task relationship is created by a sponsor's
   offer and a participant's consent. The kernel does not match skills to tasks.
2. **Separated planes.** Authoritative control, untrusted collaboration, and independent
   verification have different records, capabilities, and readers even when one local store holds
   them.
3. **Typed shared effects.** Changes to authoritative state are explicit commands with validated
   preconditions; conversation is inert evidence, not state mutation or executable input.
4. **Non-delegable authority.** A message cannot grant, transfer, or amplify a capability. Every
   consequential request is checked against the requester's own subject-bound grant.
5. **Conserved resource vectors.** Delegation transfers existing capacity into escrow. It cannot
   mint money, tokens, time, authority, or verification queries.
6. **Tracked obligations.** Every propagation of work has a causal parent and a terminal return.
7. **Immutable evidence.** Records, messages, submissions, candidates, contracts, environments,
   and oracle results are content-addressed or append-only.
8. **At-least-once delivery, exactly-once effect.** Commands are safely retryable by idempotency
   key within a live controller authority interval. A controller crash ends a POC run rather than
   opening a new interval under ambiguous state.
9. **Finite activity.** Creating a task, message, attempt, participant, external action, or
   verification consumes a finite budget dimension.
10. **Visible assumptions.** Fairness, expiry, feedback disclosure, and the fault model are
   protocol properties rather than informal expectations.

## Protocol layers and transports

The authoritative protocol is the set of typed domain commands, events, preconditions, and
outcomes in this document. It is independent of a provider, process, storage engine, or wire
transport. The current ratatui application invokes the command interface in process. When an
attempt tool bridge or private worker crosses a process boundary, the local implementation uses
**ymp RPC**: length-prefixed UTF-8 JSON over an attempt-scoped Unix-domain socket, with integer
quantities, explicit schema versions, bounded payloads, and references to large content-addressed
objects.

There is one local endpoint class in the current product: each attempt receives a different private
endpoint mounted only into its isolation boundary. The foreground controller binds that endpoint to
one principal, participant, attempt, lease generation, and allowed command set. A caller does not
select its own subject by putting an identifier in a request. TUI actions need no operator endpoint
because the TUI and application core share a process.

The coordination-tool schema is a compatibility projection in front of the attempt endpoint. An
external Claude Code, Codex, or OpenCode runtime may launch `ymp internal agent-mcp`, which speaks
the negotiated MCP revision over stdio and translates explicit tools to ymp commands. A direct
model runtime may expose the same schema through provider-native typed function calls. Neither MCP
nor a native tool-call identifier becomes an authoritative record. These bindings do not supply
durable identity, exactly-once effects, budget conservation, lease fencing, session persistence,
or termination detection; the kernel protocol supplies those properties.

Agent-runtime lifecycle is a third, separate boundary. A compiled runtime driver may use a
structured CLI event stream, resume command, Codex App Server, OpenCode HTTP/SSE server, or a
direct provider API, but converts those events to one common invocation lifecycle. Neither terminal
output, model output, nor an agent's final prose can mutate control state without an explicit
accepted ymp command.

The model route is separate again: it identifies the provider and deployment, endpoint and wire
protocol, account or quota scope, model identifier or snapshot, authentication mode, and approved
data-disclosure class. One OpenCode runtime can therefore create different profiles for NVIDIA,
Anthropic, OpenAI, or a local model. NVIDIA Nemotron may be reached through Anthropic Messages,
OpenAI Responses, or OpenAI Chat Completions only when the exact runtime-route pairing passes its
declared conformance probes.

This separation also preserves optional client/server and cluster paths. A future `ymp server` may
adapt the same domain commands to an authenticated operator transport, and a protected cluster
transport plus PostgreSQL may replace local adapters without changing command semantics. These
transports are not implemented in POC, MVP, or Alpha. MCP remains local to an agent invocation and
is never used for operator sessions, controller replication, worker placement, leader election, or
store synchronization.

The ymp domain schema, local or cluster transport envelope, coordination-tool schema, and binding
revision are recorded independently. The binding revision is an MCP revision for stdio MCP or a
named provider tool-call dialect for native tools. Runtime-driver and harness versions, harness and
prompt policy, and model-route identity are provenance, not substitutes for protocol versions. A
private child-process handshake advertises a minimum and maximum compatible domain and transport
range; an incompatible bridge or worker fails before issuing a command. A future external client
handshake must define its own authenticated compatibility exchange. Upgrading a tool binding or
runtime therefore cannot silently reinterpret a durable ymp record.

## Shared objects

### Participant

A participant identifies a resumable agent-runtime session and its versioned runtime profile. The
profile composes the runtime driver and harness policy, model route, coordination-tool binding, and
execution-assurance profile. It owns a local budget account and may sponsor or perform work.
Runtime descriptions and skill hints are claims for discovery, not permissions.

### Collaboration audience membership

Board access is an explicit, expiring control grant with separate read and publish rights. Active
participants may read bounded `project_discovery` notices. A participant can request admission to
a detailed task or candidate-review audience; the current local scope sponsor may grant or decline
it within membership and communication limits. Formation of a task contract automatically grants
the sponsor and contractor the minimum task audience needed for their work.

An audience grant exposes information but does not assign a role, create an obligation, or compel
the recipient to read or respond. A participant may leave at any time. Agent-review audiences obey
the stricter reveal order in the approved review policy.

### Task offer

A participant sponsors an offer using work it already owns. An offer contains:

- `offer_id`, sponsor, parent obligation, and causation references;
- task-scope identifier, named base snapshot, and digest of an inert intent message;
- mechanically declared dependency identifiers and path/capability scope;
- execution escrow and a proposal-stage allowance;
- an offer policy: `negotiated`, `open_accept`, or `targeted`;
- bid and offer expiry;
- the maximum number of awards funded by separate escrow reservations; and
- active projection lifetime.

The sponsor chooses the offer policy. The kernel validates resources and structure, not whether
the policy is wise. Skill, method, diversity, and runtime hints belong to the inert intent message
and never become kernel matching fields.

### Bid

A bid is a participant's consent to a possible local contract. Its control record contains a
resource counter-offer, expected artifact class, expiry, and an optional digest of a collaboration
message containing the proposed method, caveats, and evidence references. It can be withdrawn
before award.

The bid is not a proof of competence. No scalar score, grade, or self-declared level can make it
eligible or ineligible at the kernel boundary.

### Task contract

A task contract forms atomically when a sponsor awards a compatible live bid. For an
`open_accept` offer, the sponsor has pre-authorized the first mechanically valid acceptance;
concurrent acceptance is serialized and fenced. A contract names sponsor, contractor, obligation,
task scope, base digest, escrow, lease policy, and both consenting records.

“Sponsor” and “contractor” are temporary roles for one contract. A contractor may sponsor other
offers from its own escrow.

### Work obligation

An obligation represents outstanding causal work. It has one parent, zero or more children, an
authority charge, and one terminal return. A child is created only when a funded award or direct
locally accepted delegation forms a task contract. It terminates as `result`, `dead_end`,
`declined`, `exhausted`, `cancelled`, or `infrastructure_error`.

Task and candidate dependencies may form directed acyclic graphs; obligation parentage remains a
tree for termination accounting. An obligation returning a failed or empty result still closes
causal work. It does not make the parent successful.

### Attempt and lease

An attempt is one bounded execution of a participant against a task contract and base snapshot.
It has a private workspace, capability set, budget reservation, and terminal record. A lease is
the temporary right to advance that task contract and contains an expiry plus a monotonically
increasing fencing token.

### Invocation, event cursor, and wake condition

An invocation is one supervised runtime process slice that starts or resumes a participant within
an attempt. It records `invocation_id`, runtime kind and driver version, external harness version
and digest where applicable, harness and prompt-policy versions, model provider and deployment,
wire protocol, model identifier or snapshot, account or quota scope, coordination-tool binding and
schema, runtime session reference, workspace identifier, input event cursor, usage and resource
evidence, and one terminal reason. Runtime session references are restricted control data, not
board identifiers.

An invocation may yield without returning its task contract. `yield` records a new event cursor,
a bounded set of typed wake conditions, a wake deadline, and an optional inert explanation digest.
Allowed wake conditions refer only to mechanically observable events: a direct delivery, audience
change, named offer or bid transition, descendant obligation return, integration result,
verification result, cancellation, or deadline. They never match natural-language meaning.

A yielded attempt retains its lease only until the already funded expiry. A matching event merely
makes another invocation eligible for non-semantic admission; it does not guarantee a process
slot, assign work, or prove that the participant read the event. Wakes are coalesced, each resumed
invocation consumes a runtime-start unit, and the per-attempt wake count is finite. If no wake is
funded before the deadline, normal expiry or return rules apply.

### Budget vector

The root contract provides independently enforced dimensions such as:

- model-route cost and reported model tokens;
- wall time, CPU, memory, process count, disk, and output bytes;
- participant and attempt starts;
- active and total collaboration-audience grants;
- outstanding and total work obligations;
- control-command count, collaboration-message count, and published or delivered payload bytes;
- protected verification queries;
- network and dependency-broker requests;
- credential grants; and
- externally consequential actions.

Dimensions are not silently exchangeable. A budget transfer changes ownership of existing
capacity. Authority for one class of action cannot be inferred from spare capacity in another.

### Collaboration message and active projection

A collaboration message is an attributed, bounded, inert payload with kind `proposal`, `question`,
`hypothesis`, `observation`, `constraint`, `dead_end`, `challenge`, `confirmation`, `decision`, or
`help_request`. It may cite files, artifacts, candidates, control records, or other message
digests. Optional fields state what response is requested and which earlier published evidence a
decision claims to use.

Messages have an explicit audience: `project_discovery`, a task scope, a candidate-review group,
or named admitted participants. Project discovery carries only a bounded summary and references;
detailed findings are not globally readable by default. Audience membership is a control grant,
not something a message can confer.

The message record is permanent. Its active board projection expires unless refreshed. Refresh
creates a new attributed message and consumes communication budget; it never rewrites history.
The kernel does not determine whether a confirmation is correct. A delivery receipt means that the
payload became available to a participant; it does not prove reading, belief, or understanding.

A message alone never creates an obligation, launches a runtime, fetches a URL, invokes a tool,
spends escrow, or changes a lease. A participant that chooses to act must issue a separate control
command under its own capability.

### Submission and candidate

A submission is an immutable patch or artifact bundle against a named base snapshot. The
mechanical integrator creates a new immutable candidate or a typed integration failure. Candidate
ancestry, task ownership, and work obligations are separate structures.

### Verification request and evidence

A verification request names one candidate, one contract scope, and one protected-query
reservation. A verifier result identifies the candidate, contract, environment, and oracle
digests. It is `passed`, `failed`, or `infrastructure_error`; the last state consumes resources
according to policy but never counts as a candidate rejection.

### Independent assessment commitment

When the approved oracle includes agent review, the reviewer first commits the digest of a bounded
assessment while board history, producer rationale, reputation views, and prior votes remain
unavailable. After the commitment is durable, the review policy may reveal selected collaboration
records for a second assessment. Both versions remain attributable. This preserves an independent
starting point without forbidding later criticism or synthesis.

## Versioned commands

The command set is versioned and may evolve. It is not a slogan about a permanently fixed number
of verbs.

| Command | Plane | Mechanical effect |
|---|---|---|
| `read_control` | control | Read a bounded authorized projection of authoritative state |
| `read_events` | control | Read typed authorized events after an invocation cursor and advance only the caller's delivery cursor |
| `read_board` | collaboration | Read and charge bounded delivered bytes from the caller's explicit audiences |
| `publish` | collaboration | Append an inert message or salience refresh within communication budget |
| `request_audience` | control | Request an expiring task or candidate-review audience grant |
| `grant_audience` | control | Let the local scope sponsor grant bounded read or publish access |
| `leave_audience` | control | Relinquish an audience grant without changing task obligations |
| `advertise` | control | Reserve sponsor-owned capacity and create a funded task offer |
| `bid` | control | Record expiring consent and a digest link to an optional proposal message |
| `award` | control | Select compatible consent and atomically form a task contract and child obligation |
| `accept_open` | control | Form a task contract and child obligation under sponsor-pre-authorized contention rules |
| `withdraw` | control | Withdraw an unawarded offer or bid and settle its reservations |
| `request_participant` | control | Spend proposal-stage budget to start a possible contractor on a named offer |
| `renew` | control | Extend a current task lease within existing wall-time and renewal allowances |
| `yield` | control | End the current invocation and register bounded typed wake conditions without returning the task contract |
| `return` | control | End a task contract and return a typed result or non-success outcome to its parent |
| `submit` | control | Record an immutable bundle under the current fencing token and request mechanical integration |
| `request_verification` | verification | Spend a protected-query reservation to verify an exact candidate digest |
| `commit_assessment` | verification | Commit a blinded reviewer assessment digest before social information is available |
| `reveal_assessment_context` | verification | Reveal only the later context allowed by the approved review policy |

No command forces another participant to take a role. A targeted offer still requires a bid or
acceptance. No command lets the kernel choose a bid, infer task complexity, or decide which
candidate deserves verification.

Offer intent and bid rationale may be published as collaboration messages, but the corresponding
control records contain only the fields and digests needed to form a valid local contract. Free
text is never parsed as consent or authority.

## Agent-facing coordination-tool projection

The first tool schema exposes a small one-to-one projection rather than a generic `execute`
function. An invocation receives only tools allowed by its current task contract:

| Tool family | Domain commands |
|---|---|
| observe | `read_control`, `read_events`, `read_board` |
| communicate | `publish`, `request_audience`, `grant_audience`, `leave_audience` |
| contract | `advertise`, `bid`, `award`, `accept_open`, `withdraw`, `request_participant`, `renew` |
| lifecycle | `yield`, `return` |
| artifact | `submit` |
| verification | `request_verification`, `commit_assessment`, `reveal_assessment_context` when authorized |

Tool arguments never contain a database path, operator endpoint, protected-oracle reference,
principal selector, bearer capability, or arbitrary URL to fetch. Object and control identifiers
are ordinary references; the foreground controller supplies the acting principal and checks scope,
lease, budget, and fencing from trusted connection state. Read results are cursor-based and byte-bounded so an
agent cannot request the entire board or ledger in one tool result.

The binding registers a kernel command identifier before forwarding a state-changing call and
durably associates the binding request within that invocation with the result. A repeated delivery
of that same command identifier returns the recorded result without a second effect. A disconnected
runtime may issue a new semantic request after reading current state, but the driver never guesses
that a fresh MCP or provider-native identifier is a retry and never blindly replays an ambiguous
write.

The POC binding is stdio MCP. It exposes no MCP resources, prompts, server-initiated sampling, or
long-running MCP tasks. Resources would make client-controlled context injection and read
accounting runtime-dependent; MCP tasks are experimental in protocol revision `2025-11-25` and do
not model ymp's causal obligations. Server notifications may announce that tool definitions
changed, but correctness does not depend on a runtime inserting an unsolicited message into an
active model turn. `yield` plus controller-driven session resumption is the portable wake mechanism.

A future direct-model runtime may render the same schema as OpenAI Responses tools, Anthropic
Messages tools, or another explicitly versioned typed-call dialect. That binding still calls the
attempt endpoint and has no direct store authority. Differences in parallel tool calls, streaming
arguments, cancellation, or usage fields are route capabilities and conformance cases, not changes
to domain command semantics.

Strict mode generates the complete tool and runtime configuration for an invocation and ignores
ambient user, project, and plugin-provided tool servers. A runtime that cannot enforce that
configuration may be used only in a labelled weaker profile.

## Plane and authority semantics

Control and collaboration identifiers occupy different namespaces. A collaboration payload may
quote a control identifier for discussion, but only an authenticated control command can exercise
it. Capability tokens are subject-bound, audience-bound where applicable, short-lived, and
rejected if presented by another participant or embedded in a message.

Every brokered boundary action records:

- the initiating principal and attempt;
- its task contract and current fencing token;
- action class and exact object or destination scope;
- the capability grant and resource reservation used; and
- causation and provenance references supplied by the caller.

Causation references support audit; they do not expand authority. If participant B acts after a
request from participant A, B can use only B's own grant for that task and object. A malicious
message can still persuade B to misuse legitimate authority, so strict initial contracts provide
no irreversible external action and minimize broad fetch or credential capabilities. Security
does not depend on the model correctly recognizing an instruction as malicious.

Protected oracle content, raw verifier output, credentials, capability material, and isolation
policy never enter collaboration storage. A bounded diagnostic may appear in a control projection;
a participant may publish its own interpretation, which remains an untrusted claim.

## State transitions

### Offer and local contract

```text
funded offer ──► advertised ──► awarded ──► contracted
                     │   │          │
                     │   └─ bids ───┘
                     ├──────────────► expired
                     └──────────────► withdrawn

contracted ──► active ──► submitted ──► returned(result)
                  ├──────────────────► returned(dead_end | exhausted | declined)
                  ├──────────────────► expired
                  └──────────────────► cancelled | infrastructure_error
```

For a multi-award offer, each award has its own escrow, obligation, lease, and attempt lineage.
Competing work is deliberate and budgeted rather than an accidental consequence of a lease race.
Expiry or withdrawal of an unawarded slot settles its reservation atomically and creates no child
obligation. Work remains with the sponsor's parent obligation.

### Candidate

```text
recorded ──► integrating ──► integrated ──► verification_requested
                    │                             │
                    └─► integration_failed        ├─► passed
                                                  ├─► failed
                                                  └─► infrastructure_error
```

A candidate remains immutable in every state. A root-scope `passed` result can terminate the run
as accepted; a subtask-scope pass is only evidence for its parent.

### Obligation

```text
open ──► delegated ──► active ──► returned
  │                         │
  ├─► locally_active        ├─► expired | cancelled
  └─► cancelled             └─► infrastructure_error
```

An obligation can return only after all descendant obligations are terminal. The return signal
therefore propagates toward the root after delegated work becomes passive. This is termination
bookkeeping, not a prescribed order of semantic execution.

### Invocation

```text
created ──► running ──► completed
                ├─────► yielded ──► admitted ──► running
                │          ├──────► wake_deadline_expired
                │          └──────► cancelled
                ├─────► interrupted | limit_exceeded
                └─────► runtime_error | model_route_error | infrastructure_error
```

`completed` means only that the runtime process ended after an accepted explicit task action; it
is not candidate acceptance. An ordinary zero exit without `yield`, `return`, or another terminal
protocol action is `runtime_error` or a driver-defined incomplete outcome. A model API rejection,
incompatible response, or upstream inference failure is `model_route_error`; it is not evidence
that the task is unsolvable. A yielded invocation has no running process. Its task contract remains
active only under its finite lease and wake
budget.

## Lease and race semantics

Formation of a task contract and issue of its first lease are transactional. A successful result
returns:

- `task_contract_id`, `obligation_id`, and `attempt_id`;
- `lease_id`, fencing token, and expiry;
- base snapshot digest;
- capability identifiers; and
- resource reservation identifiers.

Every state-changing command repeats the lease and fencing token. Once a newer token exists, an
older attempt cannot submit a current candidate or close the obligation. It may preserve a stale
bundle or publish a message within its remaining budget.

Expiry can duplicate computation but cannot permit a stale write. The expiry event, new lease,
old result, and all charges remain visible. Timeout is not evidence that the runtime lacked the
task skill.

## Resource conservation and finite decomposition

`advertise` atomically reserves sponsor capacity and creates offer escrow. `award` transfers one
reserved execution escrow to the contractor and creates the corresponding child obligation.
`request_participant` reserves its proposal-stage cost before launch. `request_verification`
consumes a distinct query token reserved from the root contract.

Unused capacity may return according to the recorded settlement policy; consumed capacity never
does. A creation authority unit for an offer, obligation, participant, or attempt is not refunded,
so a branch cannot create an infinite chain by repeatedly returning the same nominal budget.

No arbitrary maximum semantic depth is required for finiteness. Concurrency limits protect the
host; total creation budgets protect the run. A project may still choose a depth limit in its
authority policy, but depth is not used as a proxy for task quality.

Model-route token or monetary ceilings are strict only when the runtime driver, route broker, or
provider can enforce them. Otherwise the run must use an enforceable upstream limit or declare
that dimension observational rather
than claim a hard ceiling.

## Fairness, congestion, and stale information

When valid requests exceed host capacity, admission uses per-principal round robin. Per-principal
rate and outstanding-request limits prevent one participant from manufacturing an advantage by
flooding. Within one principal, durable control-record order breaks ties. These rules inspect identifiers
and reservations, not task meaning.

The board exposes raw task age, bid count, failed-attempt count, offer escrow, time to deadline,
queue pressure, and recent publication rate. Participants may use those signals as they choose.
The kernel does not combine them into “complexity”, priority, or a response threshold.

Offer expiry, projection expiry, bounded reads, rate limits, and finite communication bytes damp
reaction storms. A participant may refresh an important finding or re-advertise a returned task,
but must spend local resources to keep doing so.

## Verification feedback

Visible development checks may run inside attempts. Protected verification uses separately
budgeted queries. The oracle policy determines whether a failed query discloses only pass/fail, a
failure class, or bounded diagnostics. Raw protected output never enters collaboration storage.

This distinction prevents unlimited adaptive probing of one fixed holdout. A rejection does not
automatically fund a retry, reopen work, lower a grade, or increase task complexity. Participants
must decide locally whether to spend remaining capacity on another attempt.

## Record envelopes

### Authoritative control record

Every control record carries:

- run sequence, record identifier, schema version, and trusted timestamp;
- command idempotency key, causation identifier, and correlation identifier;
- authenticated principal, participant, attempt, offer, task contract, obligation, and lease
  identifiers as applicable;
- fencing token and expiry where applicable;
- base, artifact, candidate, contract, environment, and oracle digests where applicable;
- typed transition and its mechanically checked preconditions; and
- budget reservation, transfer, charge, settlement, and capability references.

It contains no grade and no free text that can be interpreted as authority.

### Collaboration message record

Every message carries:

- board-local sequence, message identifier, schema version, publication time, author, and audience;
- message kind, bounded inert payload, payload digest, and projection expiry;
- reply, artifact, evidence, candidate, and claimed decision-causation references;
- task, offer, bid, or candidate-review scope as applicable; and
- communication-byte and publication charges.

Message order is reliable publication order, not proof of causal order or truth. Delivery receipts
are separate observations. Control records refer to a message only by digest and never import its
semantics.

### Verification and outcome record

Verifier records contain the exact contract, candidate, environment, and oracle digests; isolation
and toolchain identity; machine observations; approved diagnostic projection; and terminal status.
Outcome evidence always retains sample size, context, and verification regime. It does not import
board content into the initial verdict. See [REPUTATION.md](REPUTATION.md).

## Communication observability and causal audit

The live trace can show signaling, replies, citations, revisions, artifact ancestry, and later
actions. Those associations are useful for diagnosis but do not establish that one participant
listened to another. The experimental analysis therefore distinguishes:

- **positive signaling:** a message carries information about the sender's observation or action;
- **positive listening:** the receiver's later action distribution changes when message content
  changes; and
- **task value:** the change improves accepted outcome, calibration, recovery, or cost rather than
  merely increasing influence.

For sampled episodes, preregistered replay or fresh-run conditions compare the original message
with: no message; a neutral payload of comparable size; a message shuffled from another task or
sender; and the same underlying evidence delivered directly to the receiver. The last condition
tests whether a separate participant added more than context duplication. Multiple stochastic
replications use the same total model-route, runtime, execution, communication, and verification
budget.

Further analyses test whether the group exceeds the strongest single agent and independent
best-of-`n`, whether accepted artifacts combine non-redundant contributions, whether correct
independent dissent survives social pressure, and whether locally formed roles adapt after a
participant, message, or dependency is removed. Results are reported by task class and cannot be
collapsed into a claim of general intelligence from one run.

These measures are observational study outputs. They do not award budget, rank participants,
force communication, or affect candidate acceptance. Influence, agreement, verbosity, and
self-reported confidence are never intrinsic rewards. The protocol records only deliberately
published summaries and external actions, not private chain-of-thought.

## Quiescence and terminal outcomes

The kernel emits `run_quiescent` only when all of the following hold:

- no attempt or participant launch is running;
- no yielded invocation retains a live lease, funded wake, or unexpired wake deadline;
- no unexpired task contract, lease, or funded offer can advance;
- no integration or verification operation is pending;
- all descendant obligations have terminal returns; and
- the transactional command queue is empty.

Unread messages and unrefreshed help requests do not keep a run alive. Only a funded control object
such as an offer, task contract, attempt, obligation, integration, or verification request
participates in termination accounting.

If no funded valid transition remains, quiescence becomes `run_exhausted`. A human may authorize a
new contract version or budget and begin a new run lineage; the old run is not rewritten.

Root terminal states are:

- `accepted` — exact root candidate passed the approved oracle;
- `exhausted` — no funded work remains;
- `abstained` — participants returned insufficient evidence under an allowed stopping policy;
- `cancelled` — an authorized human stopped the run; or
- `infrastructure_error` — trusted execution or evidence integrity was lost.

Budget exhaustion, consensus, a contractor's `result`, and quiescence are never aliases for
acceptance.

## Recovery assumptions

The POC single-machine protocol assumes an intact in-process kernel, append-only event journal,
object store, monotonic host clock, and outer disposable experiment environment. Duplicate bridge
delivery, MCP reply loss, delayed child-process events, and lease expiry are handled during a live
run through idempotency and fencing. A foreground-controller crash ends the run as
`infrastructure_error`; the replacement process may inspect evidence but does not resume authority.
The controller does not automatically replay an ambiguous runtime tool call under a new command
identifier.

If MVP later claims crash continuation, the selected store must reconstruct authoritative state
before accepting new commands, fence every pre-crash lease, and rebuild collaboration projections
from separate records. Embedded SQLite is one possible implementation, not a protocol assumption.

Corruption of the trusted control ledger, message attribution or integrity record, kernel,
verifier, or isolation boundary is Byzantine with respect to this model. The correct response is
`infrastructure_error` and loss of assurance, not a claim of self-repair.

## Deliberately absent

- No predefined project work graph.
- No global semantic scheduler or permanent manager role.
- No required-level, grade, cheapest-capable, or strongest-first rule.
- No automatic escalation after failure.
- No global scalar task complexity that rises when work remains unsolved.
- No majority-vote acceptance.
- No shared writable project tree.
- No globally readable free-form board carrying authority or protected verification data.
- No dependency on Paseo or another external orchestrator.
- No use of MCP or a provider-native tool dialect as the authoritative state, agent-lifecycle,
  application-core, external operator, or future cluster protocol.
- No assumption that a PID, workspace path, file offset, database row, or runtime session is a
  durable principal.
- No assumption that more messages, participants, or attempts imply progress.

Recurring roles and effective allocation policies may emerge in recorded traces. A narrowly
mechanical mechanism is promoted into the protocol only after a preregistered ablation shows that
it improves outcomes without centralizing semantic choice.

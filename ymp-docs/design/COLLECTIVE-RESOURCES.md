# The resource model

Part B of [`PRODUCT-BRIEF-collective-v2.md`](PRODUCT-BRIEF-collective-v2.md): the product domain
stated as a declarative resource model with `spec` and `status`, so that the local `ymp` executable
is the *first* controller of that model and a future Kubernetes control plane is another
implementation of the same semantics rather than a different product.

Read [`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) first. This document is the per-resource
definition that item 6 of Part A §24 and the whole of Part B ask for; the design document states
the boundaries and the mechanisms these resources carry.

Every resource below is stated in the same seven parts, in the order Part B lists them:

1. **spec** — desired or configured state;
2. **status** — observed state;
3. **references and ownership** — what points at what, and what owns whose lifetime;
4. **mutable and immutable fields**;
5. **lifecycle and terminal states**;
6. **reconciliation responsibility** — which controller writes `status`, and what it may not do;
7. **events and conditions**.

Where this design does not decide something, the part says **open** and names the decision in
[`COLLECTIVE-OWNER-DECISIONS.md`](COLLECTIVE-OWNER-DECISIONS.md). Nothing is filled in for the sake
of a complete table (Part A §24, closing rule).

Source links are pinned to the current head
[f0376be](https://github.com/maggnus/ymp/commit/f0376be).

## Three tiers, and why the distinction is the whole model

Part B's main criterion is that operator-facing configuration, runtime objects and trusted kernel
state are three different things. The tiers decide who may create a resource, not merely where it
is stored.

| Tier | Resources | Created by | May the operator create one? |
|---|---|---|---|
| **Operator configuration** | Provider, AgentPool, Task | the operator, or a mechanical reconciler on their behalf | yes — and only Provider and Task are ever necessary |
| **Derived catalog** | Model | joined on read from provider and engine records; stored nowhere | no; it has no spec at all |
| **Runtime objects** | Run, Collective, Agent, Candidate, Verification, Result | the kernel and the local controller, from committed facts | **no** — Part B forbids it for Agent, Candidate and Verification, and this design extends the rule to Run, Collective and Result |
| **Trusted kernel state** | the internal contract, obligations, leases, escrow, offers, budget vectors, the journal | the kernel alone | no; reachable only as evidence and diagnostics |

The internal contract is deliberately *not* a resource of this model. It is trusted kernel state
referenced by digest from `Run.spec`, exactly as an approved package is today
([`ymp-domain/src/contract.rs`](https://github.com/maggnus/ymp/blob/f0376be/ymp-rust/crates/ymp-domain/src/contract.rs)).
Promoting it to a resource would put the definition of *done* in a place an operator is expected to
edit, which is the ownership leak the whole brief is about.

The engine and its runtime driver are also not resources. An engine is a measured record beneath a
Provider ([`ymp-runtime-registry`](https://github.com/maggnus/ymp/blob/f0376be/ymp-rust/crates/ymp-runtime-registry/src/lib.rs#L3-L37)),
and the driver is compiled code. Neither is operator-facing, and Part A §19 names runtime profiles
exposed as user-level roles as a leak.

**Every resource has a name, and no name is ever operator input.** Run identifiers, candidate
digests, participant identifiers and workspace hashes exist so that facts can refer to each other
and so that evidence can be read; they are shown, copied and followed. They are never typed back to
the product as a confirmation, and no surface asks for one (owner ruling, 2026-08-15). A command's
subject comes from a selected row or from the open run. This is a property of the model and not only
of the interface: a resource whose identity had to be re-entered to act on it would have made the
identity part of the workflow rather than part of the record.

## Resource map

    Provider ──── observed ────► Model (derived catalog entry: provider · engine · model)
       │                            │
       │ (enabled, ready)           │ admissible entries
       ▼                            ▼
    AgentPool  ──── capability boundary, never a team ────┐
       ▲                                                  │
       │ referenced by name                                │ frozen by digest at run creation
    Task ──── owns ────► Run ──── owns ────► Collective ───┴──► Agent
                          │                      │
                          │                      └──► Candidate ──► Verification
                          └──────────────────────────────────────► Result

Two edges in that picture carry the product's whole ownership claim. **AgentPool → Agent is not an
instantiation**: a pool of a hundred entries produces zero Agents. And **Collective → Agent is not
a controller loop**: no reconciler computes a desired participant count. An Agent exists because a
participant asked for one and paid for it, and the kernel admitted the request.

---

## Provider

An external account that exposes models, together with how this host reaches it. Operator
configuration.

**spec**

| Field | Meaning |
|---|---|
| `family` | the vendor the account belongs to, from the fixed supported list the product ships (`anthropic`, `openai`, `nvidia`, `google`, `local`). The list is shown in full whether or not anything is configured (Part A §7) |
| `enabled` | the operator's decision, and the only field the product never measures. A provider is **not probed and not autodetected until this is true** (Part A §7) |
| `credentialOrigin` | where the credential is read from, named and never carried. Where the engine owns the credential this is the engine's own store |
| `routes[]` *(optional)* | route overrides for reaching this provider through an engine of another family: endpoint class, wire protocol, account scope, authentication mode, disclosure class |

**status**

| Field | Meaning |
|---|---|
| `state` | `disabled` · `not configured` · `needs authentication` · `ready` · `unavailable` |
| `reason` | the measurement the state was read from, in the words the row shows |
| `routes[]` | one per engine that reaches this provider: the engine's admission decision, the executable and release measured for it, that executable's digest, and where its credential is read from |
| `models` | how many models the engines under this provider serve |
| `observedAt` | when the observation was taken |
| `observedFromBuild` | the engine build the observation came from, so an observation older than the installed build reads as older rather than standing for it |

The build accepted as P1 writes three of the five states. Nothing in it authenticates against a
provider, so no measurement separates a credential the account rejects from one it accepts, and
`needs authentication` is not written
([`provider.rs`](https://github.com/maggnus/ymp/blob/f0376be/ymp-rust/crates/ymp-runtime-registry/src/provider.rs#L34-L38)).
`observedAt` is **not yet written either**, which is review residue 1 of W1-PRD-05b and an
obligation of migration unit P2.

**references and ownership**

Cluster-scoped in Kubernetes terms: a Provider belongs to the product root, not to a workspace or a
Task. It references engine records by name; an engine record is owned by the root and referenced,
never owned, by the Provider. Nothing owns a Provider, and disabling is not deletion — the record
and its reason survive, because the reason is the answer to "why is this model not offered".

**mutable and immutable fields**

`family` and the record layout version are immutable; a record is not migrated in place. Everything
else in `spec` is mutable at any time. Every `status` field is written only by observation.

Changing `spec` never affects a Run already started: a Run holds a frozen pool snapshot, and
disabling a provider mid-run removes its entries from **later** pools only (Part A §18).

**lifecycle and terminal states**

    not configured ──enable──► probing ──► ready
                                   │  └──► needs authentication  (open: no measurement writes it yet)
                                   └────► unavailable
    any state ──disable──► disabled

A Provider has **no terminal state**. It is configuration with an indefinite life; the product
offers no delete, because a deleted account would take its measured reason with it.

**reconciliation responsibility**

The **provider observer**, a mechanical local controller in the registry crate. It reads the engine
records under the root and states what they measured; it opens no process and reaches no network of
its own
([`provider.rs`](https://github.com/maggnus/ymp/blob/f0376be/ymp-rust/crates/ymp-runtime-registry/src/provider.rs#L23-L32)).
It probes only providers whose `spec.enabled` is true. It never creates an Agent, never spends a
run's budget, and never decides which models a run may use.

**events and conditions**

Events: `ProviderEnabled`, `ProviderDisabled`, `ProviderProbed`, `ProviderProbeFailed`,
`CredentialOriginRecorded`, `RouteConformanceProbed`.

Conditions: `Ready` (an engine reaches it and answered), `Authenticated` (**open** — no measurement
writes it in this build), `Conformant` (per engine–provider pairing; absent while every pairing is
an engine reaching its own vendor).

---

## Model

One discovered capability of one provider, reached through one engine. This is the level Part A §1
insists must not be confused with an Agent.

**spec**

**None.** A Model has no desired state and no operator field. It is a derived catalog record, joined
on read from the provider records and the engine records rather than stored a third time
([`catalog.rs`](https://github.com/maggnus/ymp/blob/f0376be/ymp-rust/crates/ymp-runtime-registry/src/catalog.rs#L10-L14)).
Saying so plainly is part of the model: a resource with no spec cannot be misread as something the
operator configures, and a stored copy is what goes stale while reading as current.

**status**

| Field | Meaning |
|---|---|
| `provider`, `engine`, `model` | the identifying triple; this is what the design calls a catalog entry |
| `availability` | `offered`, or not offered with the measured reason |
| `labels[]` | resource groupings such as `reasoning`, `fast`, `cheap`. **Never a role**, never an input to a kernel decision (Part A §16) |
| `measuredAt`, `measuredAgainstBuild` | so a stale list reads as stale |
| `conformance` | per engine–provider pairing. Absent in this build, and stated as absent rather than assumed passing |

**references and ownership**

Owned by the Provider whose account serves it; references the engine that reaches it. A Model has
no independent lifetime: when the engine record disappears or the provider is disabled, the entry
leaves the catalog. That disappearance must be **visible**: a route with no models must read as a
route with no models, not as a shorter catalog. The current build answers a deleted engine record
with the seeded record, so its models leave silently — review residue 2 of W1-PRD-05b, and the
second obligation of migration unit P2.

**mutable and immutable fields**

The identifying triple is immutable; an entry with a different model identifier is a different
entry. Everything else is measured and replaced wholesale by the next probe.

**lifecycle and terminal states**

    discovered ──► offered ⇄ not offered (with reason) ──► withdrawn

`withdrawn` is not a terminal state in the accounting sense: the entry simply stops being joined.
A Run that froze the entry keeps its snapshot, and the entry's later disappearance reaches the
kernel as an availability fact rather than as a change to the frozen set.

**reconciliation responsibility**

The **catalog reader** derives the entries at the moment it is asked. The **engine prober** fills
each engine's model list. The **catalog watcher** in the supervisor measures liveness during a run
and commits `EntryAvailable` / `EntryUnavailable` facts, which is how the kernel decides without
performing any input or output of its own.

**events and conditions**

Events: `ModelDiscovered`, `ModelWithdrawn`, `EntryAvailable`, `EntryUnavailable`.

Conditions: `Admissible` (its provider is enabled and ready, its engine is enabled and ready, and
its pairing has passed whatever conformance probe applies).

---

## AgentPool

The set of capabilities the collective is permitted to create participants from, together with the
mechanical limits it may do so within. **It is a capability boundary and never a team**
(Part A §16, Part B), and owner decision D1 as refined by brief v2.

**spec**

| Field | Meaning |
|---|---|
| `models` | either `selector: allAdmissible` — the form the automatic `default` pool is created in — or an explicit **ordered** list of catalog entries |
| `capacity.maxAgents` | the ceiling on participants a run using this pool may create |
| `capacity.maxConcurrentAttempts` | the ceiling on attempts running at once |
| `resourceLimits` | the per-participant budget bounds a recruit is endowed within |
| `disclosureClasses` | which disclosure classes entries of this pool may carry |
| `assuranceProfile` | the execution assurance profile a run using this pool requires, named with its limit |
| `externalActions` | permitted external effects; zero by default |

**No `replicas` and no `desired` field.** Part A §2 is explicit that replicas must not mean "keep N
agents running", and Part A §22 fixes bootstrap at exactly one participant. A desired count would
be a mechanical actor deciding how large the collective should be, which is the semantic decision
Part A §11 reserves for the collective.

**`minAgents` is the one open question.** Part B mentions minimum, maximum, desired and autoscaling bounds; Part A
§2 says min/max are constraints rather than replica targets, and §22 starts one participant. A
floor above one therefore has no reconciler in this design and would have to be either ignored or
enforced by something starting participants nobody asked for. Whether the resource carries a floor
at all is the one question this design set leaves open —
[decision D11](COLLECTIVE-OWNER-DECISIONS.md#d11--does-agentpool-carry-a-floor).

**status**

| Field | Meaning |
|---|---|
| `entries[]` | the resolved, ordered entries with each one's admissibility and the reason for it |
| `admissible` | how many of them are admissible now |
| `digest` | the digest of the resolved ordered set, which is what a run freezes |
| `resolvedAt` | when the resolution was computed |
| `tracking` | whether this pool still follows the catalog (`selector`) or holds an explicit list |

**references and ownership**

References catalog entries weakly: an entry that becomes unavailable makes the pool degraded, never
deleted. Referenced by `Task.spec.agentPool` and, by value, by `Run.spec.poolSnapshot`. A pool
referenced by a live Run may be edited freely — the Run holds the snapshot, not the pool — and a
pool named by no Task may be deleted.

**The automatic `default` pool.** The pool reconciler creates one pool named `default`, with
`selector: allAdmissible`, the first time any Provider reaches `ready` with at least one admissible
entry. It is created rather than seeded, so a root that has observed nothing holds no pool and the
product's first-run state says *enable a provider*, never `No AgentPool configured` (Part A §4).
Editing `default` replaces the selector with the explicit list the operator left behind, and the
pool stops tracking newly discovered models; the surface states that at the moment of the edit,
because a tracking pool that silently ignored an edit and an edited pool that silently ignored a new
provider are both dishonest.

**mutable and immutable fields**

`metadata.name` is immutable. Every `spec` field is mutable. `status` is written only by the
reconciler. Nothing about a pool is immutable for the sake of reproducibility — reproducibility is
carried by the Run's frozen snapshot instead, which is what lets the operator edit pools without
disturbing running work (Part A §18).

**lifecycle and terminal states**

    absent ──(first ready provider)──► ready ⇄ degraded (some entries unavailable)
                                          └──► empty (no admissible entry)

No terminal state: a pool is configuration. `empty` is a state, not a failure, and the surfaces
state which provider would restore it.

**reconciliation responsibility**

The **pool reconciler**, mechanical. It resolves the selector or the explicit list against the
catalog, recomputes admissibility, orders the result and writes the digest. It creates the automatic
`default`.

It does **not** create an Agent, and there is no `AgentAutoscaler`. Part A §2 rules one out until
something needs it; in this model scaling is the collective's semantic decision inside
`capacity`.

**events and conditions**

Events: `PoolCreated`, `PoolResolved`, `PoolEdited`, `EntryAdmitted`, `EntryWithdrawn`, `PoolEmpty`.

Conditions: `Ready`, `Tracking`, `Degraded`, `Empty`.

---

## Task

The operator's requested work: one goal, in ordinary prose, against one workspace. This is the only
resource the operator has to create, and creating it is typing a sentence.

**spec**

| Field | Meaning |
|---|---|
| `goal` | the operator's own words. Provenance class A, and the product never rewrites it |
| `workspace` | the project the goal is about, identified from the launch directory |
| `agentPool` | the name of the pool this work may recruit from; defaults to `default` and is never asked for |
| `boundaries` *(optional)* | narrowings of the pool's capacity and of the run ceilings |
| `basedOn` *(optional)* | an accepted candidate this Task continues from |

There is no `acceptance`, no `verifier`, no `negativeControl`, no `roles`, no `participants`, no
`decomposition` and no `models` field. Their absence is the deliverable: the current build's
`RunRequest` carries three of them
([`contract.rs`](https://github.com/maggnus/ymp/blob/f0376be/ymp-rust/crates/ymp-application/src/contract.rs#L44)),
and the command line asks for two more
([`surface.rs`](https://github.com/maggnus/ymp/blob/f0376be/ymp-rust/crates/ymp-cli/src/surface.rs#L185-L193)).

**status**

| Field | Meaning |
|---|---|
| `phase` | `reading` · `deriving` · `awaitingClarification` · `active` · `terminal`. There is no `awaitingAuthorization`: decision D3 removed the authorization step, and a Task with a complete plan and no open question creates its Run at once |
| `workspaceFacts` | what the local reading found: files, languages, build and test entry points, version-control state. Free, and nothing left the host |
| `derivation` | what the derivation run spent from the derivation allowance and which provider it disclosed to |
| `clarifications[]` | the questions asked, their answers and their provenance |
| `contractDigest` | the internal contract bound when the Run is created |
| `runs[]`, `latestRun` | the executions of this Task |
| `semanticRemainder` | the part of the goal no mechanical check observes, named rather than dropped |

**references and ownership**

Owns its Runs: archiving a Task archives its Runs, and nothing is deleted. References an AgentPool
by name and a workspace by path. A Task whose pool has been deleted is blocked with the pool named,
not silently repointed at `default`.

**mutable and immutable fields**

Before the first Run is created, every `spec` field is mutable: amending the goal re-derives and
draws on the same allowance. **Once a Run exists, `goal`, `workspace`, `agentPool` and `basedOn` are
immutable**, because a Run is bound to one contract derived from exactly those fields. An amended
goal is a new Task lineage, which is what `Continue` opens, and it is the accepted amendment rule
kept rather than bent
([`PROJECT-CONTRACT.md`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/PROJECT-CONTRACT.md#L62)).

**lifecycle and terminal states**

    created ──► reading ──► deriving ──┬──► awaitingClarification ──┐
                                       │                            │
                                       │  answered, or safely assumed│
                                       └─────────────┬───────────────┘
                                                     ▼
                                          active ──► terminal

Terminal states: `completed` (a Run of this Task reached `accepted`) and `closed` (every Run reached
a terminal and none was `accepted`). A third state, `discarded`, existed while an authorization step
did: with decision D3 there is no moment at which a derived Task has no Run, so a Task that never
ran is only possible when derivation itself refused — and that is recorded as `closed` with the
refusal as its reason.

**reconciliation responsibility**

The **derivation sequencer**, mechanical: it advances the phase when a fact says the previous phase
finished, holds the phase at `awaitingClarification` while a material question is open, and creates
the Run as soon as the plan is complete and no material question is open.

The semantic content of derivation — reading the goal, producing observable requirements, generating
and validating the acceptance plan — is **not** performed by this controller. It is performed by a
derivation run of the collective, in a separate scope with its own budget, audience and
capabilities, which is the two-run boundary already accepted in
[`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/VISUAL_CONCEPT.md#L125-L141).
This placement is what keeps the product free of a central semantic component
([`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) item 16).

**events and conditions**

Events: `TaskCreated`, `WorkspaceRead`, `DerivationStarted`, `DerivationCompleted`,
`DerivationExhausted`, `ClarificationRequested`, `ClarificationAnswered`, `RunCreated`,
`TaskArchived`.

Conditions: `Derived`, `Discriminating` (the derived plan rejected the negative control and every
substituted entry point), `Started`, `Blocked` (with the obstacle named).

---

## Run

One execution of a Task, and the unit everything mechanical is accounted against. A runtime object:
a Run comes into existence because a goal was stated and a plan was derived, and the operator
composes none of it. There is no authorization act (decision D3): the workspace's standing ceiling
bounds it and the goal sentence states what it is for.

**spec** — every field is set once, when the Run is created, and never changes.

| Field | Meaning |
|---|---|
| `taskRef` | the Task this executes |
| `contractDigest` | the internal contract this run is judged against; a run names exactly one and the kernel refuses a second binding |
| `poolSnapshot` | every permitted entry with its identity, disclosure class, assurance profile, conformance result and measured readiness — plus the digest of the ordered set |
| `entryModel` | the catalog entry the run ignites on, recorded so evidence names it and replay reproduces it |
| `budget` | the budget vector drawn from the workspace's standing ceiling at creation |
| `policy` | disclosure classes, assurance profile, verification-query budget, clarification budget, external actions |

**status**

| Field | Meaning |
|---|---|
| `phase` | `starting` · `running` · `paused` · `terminating` · `terminal` |
| `terminal` | `accepted` · `exhausted` · `cancelled` · `abstained` · `infrastructure_error` · `needs_clarification` |
| `terminalReason` | which dimension ran out, what could not be established, what stopping policy was published |
| `spend` | per dimension and per model, never totalled across dimensions |
| `counts` | participants, attempts, candidates, verifications, messages |
| `intervened` | whether unsolicited operator content entered the collaboration plane |
| `journalHead` | the digest the evidence bundle repeats |
| `holds` | why the acceptance path is held, when it is — an unresolved divergence being the only such hold this design defines |

**references and ownership**

Owned by the Task. Owns the Collective, the Agents, the Candidates, the Verifications and the
Result. References the AgentPool by name for provenance only: every decision reads
`spec.poolSnapshot`, never the live pool. That indirection is the whole of the run-scoped freeze
(Part A §18): changing a pool cannot change what a running run may do, and the next run gets the
updated pool.

**mutable and immutable fields**

The entire `spec` is immutable after creation. Narrowing a boundary mid-run is not an edit of
`spec`: it is an operator command that commits a fact and lowers an enforced ceiling, recorded in
`status`. Raising the workspace's standing ceiling does not widen a Run already created; the wider
ceiling applies to the next one, and to a `Continue` after an exhausted terminal.

**lifecycle and terminal states**

    created ──► starting ──► running ⇄ paused ──► terminating ──► terminal

**Six** terminals, never substituted for one another, and quiescence is never acceptance. Five are
built ([`ymp-domain/src/lib.rs`](https://github.com/maggnus/ymp/blob/f0376be/ymp-rust/crates/ymp-domain/src/lib.rs#L31-L46));
the sixth, `needs_clarification`, is added because Part A §15 names the outcome and decision D6
states it is never `abstained` — five words cannot carry six distinct meanings without one standing
in for another ([`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) item 14, change C15).

**reconciliation responsibility**

The **run launcher**, mechanical and composition-level. As soon as the plan is complete it creates
the run against
the derived contract, commits the pool freeze, registers the origin participant on the entry model,
creates the root obligation and starts one invocation. After that it only reacts to committed facts:
a registration fact starts a process, a resume fact resumes one, a terminal fact winds processes
down. It selects no participant, holds no plan, reads no goal and ranks nothing.

**events and conditions**

Events: `RunCreated`, `PoolFrozen`, `OriginParticipantRegistered`, `RunPaused`, `RunResumed`,
`RunCancelled`, `RunTerminal`, `BoundaryNarrowed`.

Conditions: `Bootstrapped`, `AcceptancePathHeld`, `Verified`, `Intervened`.

---

## Collective

The self-organizing group of participants working on one Run. Part A §2 calls it the semantic
controller, and that is exactly what it is: the component that decides, and the only one.

**spec**

**None the operator writes.** A Collective's boundaries are its Run's: the frozen pool, the
capacity from the pool, the budget vector and the policy. The resource exists so that the model has
a place to attach collective-level status and so that a future control plane has an object to
reconcile — not so that anyone configures a collective.

**status**

| Field | Meaning |
|---|---|
| `phase` | `forming` · `working` · `revising` · `stopping` · `stopped` |
| `participants[]` | references, with recruitment parentage |
| `active`, `finished` | counts, never a ranking |
| `obligations` | open and returned |
| `candidates` | produced so far |
| `stoppingReason` | published by the collective when it stops of its own accord |

**references and ownership**

Owned by the Run, one per Run. Owns Agents in the sense that a terminal Run winds every Agent down;
parentage between Agents is recorded by the kernel at recruitment and is never re-parented.

**mutable and immutable fields**

`runRef` is immutable. Nothing else is written by anyone but the kernel and the projections over
its facts.

**lifecycle and terminal states**

    forming ──► working ⇄ revising ──► stopping ──► stopped

`stopped` is terminal and carries the Run's terminal. `revising` is a named state rather than a
surface, because Part A §21 asks for revision after failure to be visible and a failed verification
is not a terminal (Part A §9).

**reconciliation responsibility**

**None mechanical for size or composition.** This is the load-bearing sentence of the whole resource
model. No controller computes a desired participant count, chooses a model, assigns work,
decomposes a task or judges quality. The collective reconciles itself: participants recruit,
advertise, bid, award, delegate, review, revise and stop, and each of those acts is a kernel command
that is admitted or refused mechanically.

The local controller's only involvement is starting a process for a registration fact the kernel
already committed.

**events and conditions**

Events: `CollectiveFormed`, `ParticipantRecruited`, `ParticipantReturned`, `OfferAdvertised`,
`BidRecorded`, `Awarded`, `FindingPublished`, `ChallengeRaised`, `CandidatePublished`,
`RevisionStarted`, `StoppingPublished`.

Conditions: `AtCapacity` (a mechanical fact about `ParticipantStarts` and concurrency),
`Quiescent` (nothing is running — and never a synonym for accepted).

---

## Agent

One actual participant. Part B's Pod analogy holds here and nowhere else: an Agent is a running
thing with an identity, an account and a lifecycle, created dynamically because something needed it.

**spec** — set at admission, never edited.

| Field | Meaning |
|---|---|
| `runRef`, `collectiveRef` | what it belongs to |
| `catalogEntry` | provider · engine · model, checked for membership in `Run.spec.poolSnapshot` |
| `principal` | the identity its commands are authenticated as |
| `endowment` | the budget vector it was funded with, from its recruiter's own account |
| `recruitedBy` | the participant that requested it; empty only for the origin participant |
| `purposeDigest` | the digest of the intent it was recruited under. Stored and compared, never read ([`protocol.rs`](https://github.com/maggnus/ymp/blob/f0376be/ymp-rust/crates/ymp-domain/src/commitment/protocol.rs#L1-L6)) |

**status**

| Field | Meaning |
|---|---|
| `state` | `admitted` · `starting` · `working` · `yielded` · `returned` · `failed` · `cancelled` · `expired` |
| `attempts`, `invocations` | counts, each one a charged dimension |
| `spend` | by dimension, with what remains |
| `holds[]` | the obligations it took and has not returned |
| `published` | findings, challenges, confirmations — counts and references |
| `submissions[]` | the candidates it produced |
| `verificationRequests[]` | what it asked verification for and what came back |
| `recruited[]` | whom it recruited in turn |
| `lastActivityAt` | the raw mechanical column the agents table sorts by |

Never in `status`, and never stored anywhere: private chain-of-thought (Part A §14). What the
operator sees is published messages and externally visible actions.

**references and ownership**

Owned by the Collective and, through it, the Run. References its catalog entry and its recruiter.
An Agent is never re-parented and never migrated between Runs.

**mutable and immutable fields**

Everything in `spec` is immutable. An Agent that would run a different model is a different Agent:
`Reassign` moves work, it does not re-model a participant. This is what makes per-model spend
attributable
([`W1-APP-02y`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/work/waves/W1/W1-APP-02/tasks/W1-APP-02y.md)).

**lifecycle and terminal states**

    requested ──► admitted ──► starting ──► working ⇄ yielded
                     │                          │
                     └── refused (no Agent) ────┴──► returned · failed · cancelled · expired

Four terminal states. A refusal creates no Agent at all, moves no budget and starts no process —
which is test 7 of the design's acceptance list.

**reconciliation responsibility**

The **run launcher** starts a process for each `ParticipantRegistered` fact and winds it down at a
terminal fact. **Admission** measures whether the entry is live before the recruitment command is
issued and commits availability facts. The **kernel** decides by containment over committed facts:
entry ∈ frozen pool, no live `EntryUnavailable`, disclosure class ⊆ policy, assurance ≥ required,
`ParticipantStarts` ≥ 1, concurrency headroom.

No controller assigns an Agent its work.

**events and conditions**

Events: `ParticipantRegistered`, `EntryNotPermitted` (a refusal, recorded), `InvocationStarted`,
`InvocationResumed`, `InvocationEnded`, `Yielded`, `ObligationTaken`, `ObligationReturned`,
`Submitted`, `Recruited`, `Cancelled`, `Expired`.

Conditions: `Live`, `Funded`, `EntryAvailable`.

---

## Candidate

An immutable result of one branch of work. Part A §3 and the accepted architecture agree on the one
property that matters: it does not change after it exists.

**spec** — the whole resource, and immutable in full.

| Field | Meaning |
|---|---|
| `runRef` | the run it belongs to |
| `producedBy` | the participant that submitted it |
| `base` | the candidate or source snapshot it was built from — the ancestry Part A §9's revision cycle walks |
| `digest` | the exact digest a verdict is bound to |
| `manifest` | paths and sizes, with a path to the full difference |

**status**

| Field | Meaning |
|---|---|
| `integration` | `integrated` · `conflicted` (a conflict becomes a task, not a failure) |
| `verifications[]` | the verifications performed against this exact digest |
| `latestVerdict` | `passed` · `failed` · `infrastructure_error` · none |
| `exported` | whether the operator delivered it into the project directory, and where |

**references and ownership**

Owned by the Run. Referenced by Verification (by digest, exactly) and by Result. Never deleted:
archiving a run leaves its candidates readable.

**mutable and immutable fields**

Immutable in full. There is no operation that edits a candidate, and a verifier that finds a
candidate other than the recorded one refuses rather than judges
([`protocol.rs`](https://github.com/maggnus/ymp/blob/f0376be/ymp-rust/crates/ymp-domain/src/commitment/protocol.rs#L590-L598)).

**lifecycle and terminal states**

    submitted ──► integrated | conflicted

No terminal state, because a candidate is a fact rather than a process. Its verdict lives on
Verification, so that one candidate may carry several verdicts without any of them rewriting it.

**reconciliation responsibility**

The kernel records it; the integrator builds it from a submission. Nothing reconciles it afterwards.

**events and conditions**

Events: `CandidateSubmitted`, `CandidateIntegrated`, `IntegrationConflict`, `CandidateExported`.

Conditions: `Immutable` (stated permanently, because it is the property everything else rests on),
`Verified` (an exact-digest verdict of `passed` exists).

---

## Verification

One independent check of one exact candidate against the approved plan. The operator never creates
one (Part A §3, §20).

**spec** — immutable in full.

| Field | Meaning |
|---|---|
| `candidateDigest` | exact; a different candidate is refused, not judged |
| `contractDigest` | the plan it executes |
| `checks[]` | the acceptance-plan checks, each with its program digest, kind, protected flag and the requirements it observes |
| `requestedBy` | the participant that spent the query |
| `queryReservation` | the `VerificationQueries` unit reserved for it |
| `disclosurePolicy` | what a failure may disclose |
| `environmentDigest` | the environment the check runs in |

**status**

| Field | Meaning |
|---|---|
| `state` | `reserved` · `running` · `passed` · `failed` · `infrastructure_error` |
| `failureClass` | the class, not the protected material |
| `diagnostic` | bounded by the disclosure policy, and the same bounded text the collective receives |
| `observed[]` | which requirement each check observed, and which requirements no check observes |
| `digests` | contract, candidate, environment, oracle — the four an acceptance claim names ([`lib.rs`](https://github.com/maggnus/ymp/blob/f0376be/ymp-rust/crates/ymp-domain/src/lib.rs#L84-L92)) |

**references and ownership**

Owned by the Run; references a Candidate by digest and the contract by digest. It is never owned by
the participant that requested it, which is what independence means structurally.

**mutable and immutable fields**

`spec` is immutable. `status` is written once per transition by the verifier controller and never
by a participant.

**lifecycle and terminal states**

    reserved ──► running ──► passed | failed | infrastructure_error

Three terminals, and `infrastructure_error` **is not a rejection** — it is a failure of the
machinery, drawn as its own state and never counted against the candidate (Part A §10).

A `failed` verification is not a terminal of the Run: while budget and policy allow, the obligation
stays open and the collective reacts (Part A §9, §15).

**reconciliation responsibility**

The **verifier controller**, in its own boundary. It executes the approved plan, commits the verdict
as a kernel fact and discloses only what the policy allows. It does not consult the producers'
conversation before committing its independent verdict, and no participant writes verification
state.

**events and conditions**

Events: `VerificationReserved`, `VerificationStarted`, `VerdictCommitted`,
`VerifierInfrastructureFailure`, `QueryBudgetExhausted`.

Conditions: `Independent`, `Discriminating` (the plan rejected the negative control and every
substituted entry point before it could decide anything), `BudgetRemaining`.

---

## Result

What the operator came for. A projection, not a thing anyone composes.

**spec**

**None.** Result is status-only, derived from committed facts about one Run. Giving it a spec would
invite something to be configured about an outcome.

**status**

| Field | Meaning |
|---|---|
| `outcome` | `verified` · `budget exhausted` · `infrastructure error` · `cancelled` · `needs clarification` · `stopped`, one per kernel terminal, each with its own sentence and its own keys, and never substituted for another (Part A §15) |
| `summary` | one paragraph of what changed, published by the collective |
| `verification` | how many requirements were observed, and the remainder no mechanical check observes |
| `participants`, `attempts`, `candidates` | counts |
| `elapsed`, `spend` | wall clock, money, tokens per model |
| `candidateDigest` | the accepted candidate |
| `evidence` | the four digests, the requirement-to-observation map, per-model spend, the journal head digest |
| `intervened` | shown wherever the outcome is reported, because it changes what the evidence may be used for |

**references and ownership**

Owned by the Run, one per Run. References the accepted Candidate and its Verification.

**mutable and immutable fields**

Everything is immutable except the `exported` and `archived` markers, which record operator acts and
rewrite nothing.

**lifecycle and terminal states**

    published ──► exported? ──► archived?

The Result exists from the moment the Run reaches a terminal, including a terminal that is not a
success. `exhausted` is never drawn as success.

**reconciliation responsibility**

A projection over the journal. No controller writes a semantic judgement into it; the summary is
published by the collective and carried through unchanged.

**events and conditions**

Events: `ResultPublished`, `EvidenceExported`, `CandidateExported`, `RunArchived`.

Conditions: `Verified`, `Exportable`, `Continuable`.

---

## Controllers, in full

The model must not smuggle a central planner in as a controller, so every controller it defines is
listed here with what it may and may not do. If a component is not on this list, it does not exist
in this design.

| Controller | Reconciles | May never |
|---|---|---|
| provider observer | Provider `status` from engine records; probes only enabled providers | start a process for a run, spend a run budget, decide permitted models |
| catalog reader | Model entries, joined on read | store a copy, create anything |
| catalog watcher | `EntryAvailable` / `EntryUnavailable` facts during a run | refuse a command; it measures, the kernel decides |
| pool reconciler | AgentPool `status`, the automatic `default` | create an Agent, compute a desired count, prefer an entry |
| derivation sequencer | Task `phase` | read the goal, write a requirement, judge an answer |
| run launcher | Run bootstrap, then processes for committed facts | pick a participant, pick work, pick a model, rank anything |
| verifier controller | Verification `status` | consult the producers' conversation before its verdict, count infrastructure failure as rejection |
| kernel | every state transition, by containment and arithmetic over committed facts | choose an agent or model, assign a role, decompose, rank a bid, synthesize, read natural language |

The semantic work — interpreting the goal, deriving observable requirements, deciding the shape of
the collective, choosing what to try, judging quality, deciding to stop — belongs to collective runs
in both places it happens: the derivation run before the Run exists, and the task's own collective
after it. Neither is a controller, neither outlives its run, and neither can be addressed by a
participant as an authority.

## What Kubernetes would add, and what it must not change

A future control plane would represent Provider, AgentPool and Task as ordinary CRDs written by the
operator, and Run, Collective, Agent, Candidate, Verification and Result as CRDs written only by
controllers — the same split this document defines. An Agent would become a Pod-backed workload; the
pool's capacity would become the admission bound on that workload; the frozen snapshot would become
an immutable field of the Run object.

Three things must survive that translation unchanged, and they are the reason the model is written
this way rather than as a set of Kubernetes objects:

1. **No controller acquires semantic authority by being a controller.** A Kubernetes operator that
   decided how many Agents a Collective needs would be the central planner Part A §11 and Part B
   both forbid, whatever its name.
2. **The freeze is a value, not a reference.** `Run.spec.poolSnapshot` holds entries, not a pointer
   to an AgentPool, so editing a pool cannot change a running run in either implementation.
3. **The trusted kernel state stays kernel state.** Obligations, leases, escrow and the internal
   contract do not become CRDs an operator can edit; they remain the ledger, reachable as evidence.

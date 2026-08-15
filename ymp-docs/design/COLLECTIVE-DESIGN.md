# The collective design

The design deliverable for [`PRODUCT-BRIEF-collective-v2.md`](PRODUCT-BRIEF-collective-v2.md),
which refines [`PRODUCT-BRIEF-collective.md`](PRODUCT-BRIEF-collective.md) and its owner addendum
([7f3730f](https://github.com/maggnus/ymp/commit/7f3730f65171fe2290194f814bddeac0d7f9f3a8)) at the
product level. **Where v2 and v1 or an earlier revision of this design disagree, v2 wins.** It
answers the sixteen deliverables of v2 Part A §24 and the per-resource definitions of Part B, and it
walks the acceptance scenario of Part A §22 through the surfaces it designs.

Read in this order:

1. [`COLLECTIVE-GAP-ANALYSIS.md`](COLLECTIVE-GAP-ANALYSIS.md) — the repository inspection and the
   fifteen ownership leaks, cited below as `L-nn`.
2. This document — the coherent design behind the ownership move.
3. [`COLLECTIVE-RESOURCES.md`](COLLECTIVE-RESOURCES.md) — Part B in full: every resource with its
   declared, observed, references, mutability, lifecycle, reconciliation, events and states.
4. [`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md) — the terminal architecture, designed from the lifecycle
   list of Part A §21 and walked screen by screen against §22.
5. [`COLLECTIVE-MIGRATION.md`](COLLECTIVE-MIGRATION.md) — the migration path, unit by unit.
6. [`COLLECTIVE-OWNER-DECISIONS.md`](COLLECTIVE-OWNER-DECISIONS.md) — the decisions that genuinely
   need the owner.

Source links are pinned to the current head
[f0376be](https://github.com/maggnus/ymp/commit/f0376be), which carries the accepted provider and
catalog records of `W1-PRD-05b`. Older revisions of this design cited the baseline
[dfdac03](https://github.com/maggnus/ymp/commit/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf); those
links remain valid for the text they support.

## The sixteen deliverables of Part A §24

| # | Deliverable | Where it is answered |
|---|---|---|
| 1 | Analyse the current repository | [`COLLECTIVE-GAP-ANALYSIS.md`](COLLECTIVE-GAP-ANALYSIS.md) §1 |
| 2 | Show the current ownership leaks | [`COLLECTIVE-GAP-ANALYSIS.md`](COLLECTIVE-GAP-ANALYSIS.md) §2, leaks L-01 – L-15 |
| 3 | Define the target product mental model | item 1 below |
| 4 | Define the operator lifecycle from launch to Result | item 2 below |
| 5 | Design the TUI from scratch | [`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md), derived from §21 |
| 6 | Define the Provider / Model / AgentPool / Task / Collective / Agent lifecycle | [`COLLECTIVE-RESOURCES.md`](COLLECTIVE-RESOURCES.md); items 5–7 below |
| 7 | Define dynamic recruitment | item 9 below |
| 8 | Define automatic `default` AgentPool creation | item 7 below; AgentPool in the resource model |
| 9 | Define the run-scoped pool snapshot and freeze | item 7 and item 8 below; Run in the resource model |
| 10 | Preserve the strict contract / oracle / verification internals | item 16 below; gap analysis §5 |
| 11 | Move their ownership inside the system | item 16 below |
| 12 | Define the necessary domain, state and API changes | item 20 below |
| 13 | Define the Rust module and crate changes | item 21 below |
| 14 | Define the migration path | [`COLLECTIVE-MIGRATION.md`](COLLECTIVE-MIGRATION.md) |
| 15 | Define the end-to-end tests | item 22 below |
| 16 | State separately the decisions that genuinely need a human | [`COLLECTIVE-OWNER-DECISIONS.md`](COLLECTIVE-OWNER-DECISIONS.md) |

Part B's per-resource requirements — the declared half, the observed half, references and
ownership, mutable versus immutable fields, lifecycle and terminal states, reconciliation
responsibility, events and states — are answered for all ten resources in
[`COLLECTIVE-RESOURCES.md`](COLLECTIVE-RESOURCES.md). Where this design does not decide something it
says **open** and names the decision, per §24's closing rule: *do not invent missing details for the
sake of completeness.*

## What this design changes, in one page

The kernel, the verification plane, the accounting and the isolation boundary are kept as built.
The product boundary moves. Today it runs between the operator and the contract, so the operator
supplies the acceptance condition, the verifier program and the negative control, and nothing at
all starts a participant. After this design it runs between the operator's *goal* and everything
that makes a goal checkable.

    before   operator ── contract, oracle, verifier, engine ──► kernel ──► (nothing starts)
    after    operator ── goal ──►│ derivation · pool · bootstrap · collective │──► verified result

Five moves do the whole job. Each is the smallest change that closes a family of leaks without
weakening a mechanism.

**Move 1 — the contract is derived, and a collective derives it.** The internal contract, its
observable requirements and its acceptance plan are produced from the goal, the repository and the
run policy inside the product. The mechanisms — public requirements, protected cases, negative
controls, digest pinning, query budgets — survive unchanged; only their author changes. Closes
L-01, L-02, L-03.

**Move 2 — the approval moves off the path and becomes a standing bound.** There is no
authorization screen and no confirmation between a goal and a run. The operator's approval of *what*
is the goal sentence itself; their approval of *how much* is the workspace's **standing ceiling**,
set once by default and editable on `/budget` (decision D3). A goal typed against a ready pool
starts the collective at once, and what an authorization screen used to ask is stated in the
transcript as the run starts: what *done* means, what the run spends from, the frozen pool, the
disclosure, the assurance profile. The requirement-to-evidence coverage map survives in full as
diagnostics. Closes L-06 and L-16, and preserves constitutional constraint 1 of
[`CONCEPT.md`](../CONCEPT.md) — a human still approves the definition of done, in two acts that are
theirs rather than in a modal that interrupts them.

**Move 3 — the system bootstraps the collective.** As soon as the run exists a mechanical launcher
registers the origin participant, funds it from the root account and starts its first invocation.
It follows committed facts and takes no semantic decision. Every further participant exists because
a participant asked for one. Closes L-09, L-11, L-12.

**Move 4 — provider, catalog, AgentPool, participants.** Providers are enabled explicitly and only
then probed; the models each engine can reach are measured into a catalog; a **`default` AgentPool
appears automatically** as soon as one provider is ready, so the operator is never asked for a
permitted set and never meets `No AgentPool configured`; a Task references a pool by name and its
Run freezes that pool's resolved entries by digest; a participant names a catalog entry when it
recruits and the kernel checks containment over the frozen fact. One hundred catalog entries create
no participants at all. Closes L-04, L-05, L-07, L-08, L-13.

**Move 5 — no component outside a collective reads the goal.** Interpreting intent is a semantic
decision, so it belongs to a collective and not to a product module. Derivation therefore runs as a
**collective run of its own** — separate budget, audience and capabilities, which is the two-run
boundary already accepted — and the application layer keeps only a mechanical sequencer that
advances phases when facts say a phase finished. Closes L-15, and it is what makes the claim "no
hidden central semantic orchestrator" checkable rather than asserted.

Nothing else in the architecture moves. The board (`W1-COR-03c`), candidate ancestry
(`W1-COR-03d`), the commitment surfaces (`W1-COR-03e`) and durable commitment facts (`W1-COR-03f`)
are opened work this design consumes; the engine registry (`W1-APP-02e.6`), the product root
(`W1-APP-02w.1`), the generated check (`W1-APP-02z.3`) and the provider and catalog records
(`W1-PRD-05b`) are accepted work it builds on. `W1-COR-03f` is load-bearing rather than incidental:
the kernel decides recruitment from committed facts, so those facts must be durable rather than
held in memory (item 9).

---

## 1. Revised product mental model

> I give ymp a goal. A collective of agents figures out how to reach it. ymp keeps the process
> inside its rules and makes sure the result is actually verified.

Four parties, and nothing between them:

| Party | Holds | Never does |
|---|---|---|
| **Operator** | the goal, the boundaries, the decision to spend, the decision to deliver | assemble a team, write a check, choose a model, decompose work |
| **Collective** | every semantic judgement: what the goal means, how to split it, whom to recruit, what to try, what to trust, when it is done | mint authority, budget or capability; write control or verification state |
| **Kernel** | authority, resources, lifecycle, isolation, accounting, provenance, state integrity | choose agents or models, assign roles, decompose, rank bids, synthesize answers, read natural language as a command |
| **Verification** | whether one exact candidate satisfies the plan derived for the goal | consult the producers' conversation before its independent verdict is committed |

The interface is the operator's window onto this and nothing more. It shows the collective forming
and working, it carries the operator's four decisions, and it never becomes a control panel with
one row per participant.

The product's own words are: **collective, participant, goal, task, activity, result,
verification, evidence, provider, model, pool, workspace, budget**. The words *obligation, lease,
fencing token, escrow, oracle bundle, capability namespace, runtime driver, engine, invocation,
attempt sandbox* are internal; they appear only under advanced diagnostics, and always with the
plain-language fact beside them.

*Workspace* names the project the goal is about. The private writable copy of one attempt is renamed
**attempt sandbox**. This is a rename of an internal word; the isolation rule it names is unchanged.

The four distinctions v2 §17 insists on, said once in the operator's own terms:

| Question | Answered by |
|---|---|
| what models can I reach at all? | **Provider** and its measured model catalog |
| which of those may the collective use for this work? | **AgentPool** — a capability boundary with an automatic `default` |
| how many participants, of which models, does this work actually need? | **Collective** — and nothing else in the system holds that sentence |
| who is working right now? | **Agent**, one row per participant |

The resource model behind those four is [`COLLECTIVE-RESOURCES.md`](COLLECTIVE-RESOURCES.md), which
also states the rule the whole product turns on: an AgentPool never instantiates an Agent, and no
controller computes how large a collective should be.

## 2. Complete operator lifecycle

    ymp
      └─ startup: logo, workspace and version, the prompt — nothing probed        [S01]
      └─ if no provider is enabled: one sentence and /providers                   [S02]
           └─ enable a provider → autodetect → model catalog                      [S03 S04 S05]
                └─ a default AgentPool appears, automatically                     [S35 S36]
      └─ goal, in ordinary prose                                                  [S08]
           └─ reading: repository facts, locally, free, nothing disclosed         [S08]
           └─ derivation, in a collective run of its own:
              requirements, acceptance plan, verification strategy                (internal)
                — spends the derivation allowance and discloses to one provider   [S06 S08]
                └─ clarification, only for material ambiguity                     [S09]
           └─ the run starts on the sentence — no confirmation, no identifier     [S10]
                └─ bootstrap: pool frozen, origin participant registered, started [S11]
                     └─ collective: analysis, recruitment, delegation,
                        disagreement, implementation, review        [S12 S13 S14 S15 S16 S37]
                          └─ candidate                                            [S17 S18]
                               └─ verification                                    [S19]
                                    ├─ failed → the collective revises while
                                    │           budget and policy allow           [S12 S19 S20]
                                    └─ passed → result                            [S25]
      └─ inspect result and evidence                                              [S25 S26]
      └─ export, explicitly — the only write into the project directory           [S27]
      └─ continue, archive, history                                               [S28 S29]

The operator's deliberate acts are exactly two: **enable a provider** and **state a goal**.
Everything else is optional — an answer when ymp genuinely cannot determine intent, and pause,
cancel, inspect, export, continue and archive, each a key on a selected row. Nothing is confirmed
and no identifier is ever typed back to the product (owner ruling, 2026-08-15; decisions D3, D4,
D10).

**The lifecycle is interrupted in exactly one place**: a clarification question, and only when a
material assumption cannot be resolved safely (decision D6, at most three per run). It is bounded
and it is recorded. Everything else the operator sees while a run works is information, appended to
the transcript, that they may read or ignore without the run waiting on them.

**Where the human approval went.** Constitutional constraint 1 requires a human-approved definition
of done, and removing the authorization screen does not remove it — it splits it into the two acts
the operator was already taking. The **goal sentence** approves *what*: it is the statement the
contract is derived from, recorded as provenance class A, and no run exists without it. The
**standing ceiling** approves *how much*: a per-workspace bound with default values, visible and
editable on `/budget`, and no run may spend past it. Both are recorded, both are the operator's, and
neither is a modal.

One thing happens before the run exists and is stated rather than implied: **derivation spends**.
Turning prose into observable requirements needs a model, so the stage between the goal and the
collective is not free and is not silent. It draws on a small **derivation allowance**, a dimension
of the standing ceiling with its own disclosure class, visible on `/budget` and defaulted rather than
asked for (decisions D4, D8). Item 16 states the mechanism, the split between the free local half
and the funded half, and why the alternative — deriving without a model — would return authorship to
the operator.

## 3. Complete TUI information architecture

The full architecture — thirty-seven surfaces, each stated as *what the operator sees*, *what
actions exist*, *what each action does* — is [`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md). It is derived
from the twenty-nine lifecycle moments of v2 §21 rather than from the screens that exist: each
moment names its surface, three of them are drawn as states of a surface rather than as their own
screens, and six surfaces are added because the design needs them.

The composition rules that govern all of them:

1. **Tables first** (§8). Providers, models, pools, agents, tasks, activity, candidates, budgets,
   failures, engines and history are tables in the k9s style — `list → select → properties →
   action` — and a property view exists only where a single object's facts do not form rows.
2. The main surface is one scrolling conversation with an always-ready input line, a thin context
   header and a status line — the shape already accepted in
   [`VISUAL_CONCEPT.md`](../VISUAL_CONCEPT.md). Data lives on full-screen pages opened with `/` and
   closed with `Esc`. There is no setup wizard and no dialogue chain (§6).
3. Every surface leads with product language. An internal term appears only inside an advanced
   view, and never alone: `obligation ob-7 · the work participant B took on and has not returned`.
4. Nothing a participant says is executable in one keypress. Acting on a suggestion is a separate
   command issued under the operator's own authority (INV-4 as layout).
5. No ranking, scoring or grading of participants, bids or candidates anywhere. Sorting uses raw
   mechanical fields only (INV-1).
6. The control, collaboration and verification planes stay visually distinct wherever they meet.
7. Every view holds its own cursor and recovers from the journal; lag is a visible state, never a
   silent divergence (INV-6).
8. A terminal state is named exactly. `exhausted` is never drawn as success (INV-7).
9. A budget dimension is never traded against another in the interface any more than in the kernel
   (INV-2).
10. The interface is never a per-participant control panel: the operator is the principal, not the
    team manager (§13).

## 4. Startup and first run

**Startup** is the accepted start screen and nothing more: the logo, one line of basics — the
current directory and the application version — and the request invitation, shown once (node
`W1-APP-02e.5`, brief §5). The header carries the workspace and the assurance glyph; the full
assurance sentence lives in `?`, on the block that states what a run spends from and on `/budget`,
because the rule is
that the profile never appears without its limit, not that it appears everywhere.

**Nothing is probed at startup.** A provider is not autodetected before the operator enables it
(§7), so with nothing enabled, launching ymp opens the product root (`~/.ymp`, node
`W1-APP-02w.1`), identifies the workspace from the launch directory, reads the records that exist
and does not open a process or a network connection. Enabling a provider is what triggers the
autodetect, and an explicit refresh is what repeats it (leak L-13).

**First run** — the state where no provider is enabled — is the only moment the product asks the
operator to configure anything, and it asks for exactly one thing by naming a command. It states
what ymp is in two sentences and offers `/providers`. It never asks for a contract, a verifier, a
model, a pool or a team; it never says `no providers configured` (§6); and it can never say
`No AgentPool configured`, because a pool that has to be created before work can start is exactly
what §4 removes.

## 5. Provider configuration flow

Four levels, exactly as v2 §1 and §17 state them — provider, model, pool, participant — and the
product owns configuration of the first three while creating none of the fourth.

**Provider level.** A provider is an account with an authentication state: Anthropic, OpenAI,
NVIDIA, Google, a local endpoint. `/providers` shows the **whole supported list** whether or not
anything is configured (§7), each row `disabled`, `not configured`, `needs authentication`, `ready`
or `unavailable`, with the measured reason.

**A provider is enabled before it is probed.** §7 is explicit: no autodetect precedes an explicit
enable. Enabling is therefore the transition that triggers discovery, and `r` repeats it. This is
also what makes the first-run state honest: with nothing enabled there is nothing to report, rather
than a row of probe results for accounts the operator never asked about.

The accepted build already holds the correct half of this: a provider record is observed and never
seeded, so a root that has observed nothing holds no provider record at all
([`provider.rs:23-32`](../../ymp-rust/crates/ymp-runtime-registry/src/provider.rs)). What migration
unit P2 adds is the gate on the enabled flag and the age of the observation.

**Every provider is reached through an installed engine.** This is not a simplification, it is the
accepted architecture: NVIDIA Nemotron is a *model route*, not an agent runtime, and the route may
be paired with Claude Code through an Anthropic-compatible Messages endpoint or with Codex through
an OpenAI Responses-compatible endpoint
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L495-L500),
[`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROTOCOL.md#L62-L67)).
A provider row therefore names the engine that reaches it and the wire protocol it is reached by. A
native runtime that speaks to a provider without an engine is a future `ymp` worker mode
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L260-L265));
this design neither requires it nor pretends it exists, and no surface offers a provider that has
no engine able to reach it.

The consequence for configuration is small and exact: connecting a provider whose credential the
engine owns launches that engine's own authentication and re-probes; connecting one reached by a
route override records where the credential is read from and the endpoint it is used against, and
never copies the credential into ymp's own store. Each engine–route pairing carries its own
conformance probe, because nominal API compatibility does not establish identical tool calling,
cancellation or usage accounting.

ymp does not become a credential store. What it adds over today (leak L-05, L-13) is that the operator learns
what is ready, what is missing and what to do about it inside the product instead of guessing from
a refusal at run time.

**Disclosure is stated at this level, not buried.** Enabling a provider permits repository content
to be sent to it; host allowlisting is not a confidentiality control
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L562-L568)).
The provider row therefore carries its disclosure class, and the block that starts a run repeats the
list of providers that run may send content to. Decision D4 places the consent here and nowhere
else: enabling a provider **is** the disclosure decision, stated on its properties view above the
enable key, and there is no per-workspace question and no per-run acknowledgement.

## 6. Model catalog and availability

This level is **built and accepted** (`W1-PRD-05b`,
[09fc9c4](https://github.com/maggnus/ymp/commit/09fc9c4)). One record per engine under the product
root holds the enabled flag, the measured properties (executable, version, credential origin, budget
bounds, capability matrices) and the list of models that engine can serve, filled by probe; one
record per provider holds the account and the engines that reach it; and the catalog is **derived by
joining the two on read** rather than stored a third time, so a re-measured list is never read
through a stale copy ([`catalog.rs:10-14`](../../ymp-rust/crates/ymp-runtime-registry/src/catalog.rs)).

A **catalog entry** is one usable triple: provider, engine, model. It carries the properties the
kernel checks and the evidence the run records — engine version and digest, wire protocol, endpoint
class, account or quota scope, disclosure class, and whether the pairing passed its conformance
probes. A pairing that has not passed its probes is present and marked, never silently offered
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L494-L500)).

The provider of an entry need not be the engine's own vendor. `(nvidia, claude-code, nemotron-…)`
is an ordinary entry: the NVIDIA route reached through Claude Code's Anthropic-compatible endpoint,
probed as its own pairing. This is what lets the operator connect three providers on a host with
two installed engines, and it is why the design adds no engine crate to serve a third provider.

Three properties are load-bearing:

- **A catalog entry is not an agent.** Nothing is instantiated by being in the catalog. A hundred
  entries produce zero participants until a participant asks for one and pays a `ParticipantStarts`
  unit for it — the sentence §1 spends a page on.
- **A catalog entry has no role.** There is no architect model, no coder model, no reviewer model.
  Convenience groupings such as *reasoning*, *fast* or *cheap* may exist as resource labels a
  participant reads, and are never a semantic assignment (brief §15).
- **The catalog is measured, not declared.** The model list comes from probing the engine. An entry
  whose engine is disabled or unready is not admissible and is shown as such.

The version pinning that exists today survives as a property of an entry
([`runtime-claude/lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-claude/src/lib.rs#L24)):
it stops being the only reachable route and becomes the recorded identity of one route among
several (leak L-04).

## 7. AgentPool, the automatic `default`, and the run-scoped freeze

This is the item brief v2 changes most, and the change is an ownership move rather than a new
mechanism. **The operator is never asked which models a task may use.**

### The pool is a resource, not a question at request time

An **AgentPool** is the set of capabilities the collective may create participants from, together
with the mechanical limits it may do so within. It is a capability and resource boundary and never
a team, a roster or a role assignment (§16, §17, and owner decision D1 as refined by v2). Its full
definition — declared, observed, ownership, mutability, lifecycle, reconciliation, events and
states — is in [`COLLECTIVE-RESOURCES.md`](COLLECTIVE-RESOURCES.md).

    declared.models       the whole catalog  |  an explicit ordered list of catalog entries
    declared.capacity     maxAgents · maxConcurrentAttempts
    declared.resourceLimits                                    (deferred: disclosure classes,
                                                                assurance profile, external actions)

`declared.models` holds no count and no role. Permitting `opus-5`, `glm` and `fable` means only
that the collective may use those three; whether it runs one participant, or one of each, or two
of one and three of another, is the collective's decision inside `capacity` (§1, §2). There is no
`replicas` field and no `AgentAutoscaler`: a desired participant count would be a mechanical
component deciding how large a collective should be, which §11 reserves for the collective. There is no floor either:
[decision D11](COLLECTIVE-OWNER-DECISIONS.md#d11--does-agentpool-carry-a-floor) settles that
`capacity` carries `maxAgents` and the concurrency limit and nothing else, because a `minAgents`
would be the only field in the resource model with no reconciler.

### The `default` pool appears by itself

The pool reconciler creates one pool named `default`, tracking every admissible entry, the first
time any provider reaches `ready` with at least one admissible entry (§4). The consequences are the
ones §4 asks for exactly:

- the operator never sees `No AgentPool configured` and is never made to build a pool before
  working;
- after enabling one provider, a goal typed immediately starts a run;
- an advanced operator may later edit `default` or add a pool from `/pools`, which is a boundary
  edit and not a team assembly.

Editing `default` replaces the tracking selector with the explicit list the operator left behind,
and the pool stops following newly discovered models. The surface states that at the moment of the
edit, because a tracking pool that silently ignored an edit and an edited pool that silently ignored
a new provider are both dishonest.

### The run policy keeps the ceilings, and only the ceilings

| Field | Default | Enforced by |
|---|---|---|
| pool | `Task.declared.agentPool`, which defaults to `default` | kernel: containment in the frozen snapshot |
| spend ceiling | a stated default per run | `MoneyMicros` |
| wall clock | a stated default per run | `WallTimeMs` |
| participants ceiling | the pool's `capacity.maxAgents` | `ParticipantStarts` |
| concurrent attempts | the pool's `capacity.maxConcurrentAttempts` | admission and `AttemptStarts` |
| verification queries | a stated default | `VerificationQueries` |
| clarification questions | a stated default | `ClarificationRequests` (new dimension) |
| disclosure classes | the classes of the enabled providers | kernel: entry class ⊆ policy classes |
| assurance profile | the host's best available, named with its limit | preflight refusal, never silent weakening |
| external actions | none | `ExternalActions`, zero by default |

The policy is a **boundary, not a route**. It says what may be used and how much may be spent. It
never says which model suits which task; that sentence has no representation anywhere in the kernel,
and the whole no-semantic-kernel claim depends on it staying that way.

### The freeze is a value, not a reference

When the run is created the pool is frozen: `PoolFrozen` records every permitted entry with its
identity, disclosure class, assurance profile, conformance result and measured readiness, plus the
digest of the ordered set, and `Run.declared.poolSnapshot` holds those entries **by value**. Every
later kernel decision about recruitment reads that fact and never the live pool.

Three properties follow, and they are what §18 asks for:

- changing a provider or a pool after a run starts cannot change what that run may do, and the next
  run gets the updated pool;
- a matched-budget comparison (`W1-EVL-04a`) is reproducible, because both arms name the same
  snapshot digest, and per-model spend stays attributable (`W1-APP-02y`);
- **the operator never creates the snapshot** — §18's closing sentence — and never learns that it
  exists unless they open the evidence bundle, where it is named.

Node `W1-EVL-04d`, which asked where the permitted model set is declared, is closed by owner decision
D1; the pool resource is that decision as refined by brief v2.

## 8. Collective bootstrap

Bootstrap is the piece with no implementation today (leak L-09), and it is deliberately the dullest
component in the design.

**The run launcher** is a composition-level component that follows committed facts and starts
processes. It has no opinion. As soon as derivation is complete and no material question is open, it
performs exactly this sequence, each step a kernel command that can be refused:

1. Create the run and bind it to the derived internal contract — the path that exists today
   ([`contract.rs`](../../ymp-rust/crates/ymp-application/src/contract.rs)).
2. Freeze the pool: commit `PoolFrozen`, recording every permitted entry with its identity,
   disclosure class, assurance profile, conformance result and readiness as admission measured it,
   plus the digest of the set. Every later kernel decision about recruitment reads this fact.
3. Register the **origin participant**, endowing it with the root budget vector, on the **entry
   catalog entry**: the first entry of the frozen pool, in declared order, that admission finds
   live. The chosen entry is part of the `PoolFrozen` fact, so which entry ignited the run is
   recorded rather than reconstructed.
4. Create the root obligation for the goal.
5. Start one invocation for that participant, delivering the internal contract, its own budget, the
   tool set its contract allows, and an event cursor.

Then the launcher stops deciding anything for the rest of the run. From that point it only reacts:
a `ParticipantRegistered` fact makes it start a process for the new participant; an
`InvocationResumed` fact makes it resume one; a terminal fact makes it wind processes down. It
never picks a participant, never picks work and never picks a model.

### How the first participant is created without anything reading the goal

This is the sharpest point in the design, and §11 makes it worth stating in full rather than in a
clause. Something has to run first. Three sources could name it and the brief closes two of them:
the collective cannot choose, because it does not exist yet, and the operator must not be asked,
because that is the model assignment §1 forbids. What is left is a rule, and the rule has to be
mechanical in the strict sense — computable from committed facts, reading no property of the goal,
the task or the entry beyond position and readiness.

The rule is: **the first entry of the frozen pool, in the pool's declared order, that admission found
live.** It is defensible mechanically on four counts.

- The order exists before the goal does. `declared.models` is an ordered list — for `default`, the
  order the catalog produced; for an edited pool, the order the operator left. Nothing about a goal
  contributes to it.
- The rule reads two things and no more: position, and the availability facts admission committed.
  Both are already in the ledger, so the decision is a pure function of the journal and a replay
  reaches the same participant.
- It is the same kind of rule the product already relies on: the per-principal round robin that
  governs admission is an ordering over a list, not a preference over its contents
  ([`PROTOCOL.md`](../PROTOCOL.md)).
- The chosen entry is committed as part of the `PoolFrozen` fact, so a run's evidence always names
  what it ignited on. A rule that could not be reconstructed from evidence would be exactly the kind
  of hidden decision §11 is about.

What it is **not** is neutral about cost, and the decision records that rather than hiding it: the
first entry of `default` decides which model every run of that pool ignites on, which matters
directly to the owner's recorded position that experiments run on the cheaper Claude routes.
Reordering a capability list is not assigning a role — it names an ignition point and nothing else —
and the operator may reorder a pool on `/pools` when they want a different ignition. This rule is
[decision D2](COLLECTIVE-OWNER-DECISIONS.md#d2--the-entry-rule-for-the-origin-participant), decided
on 2026-08-15: no operator setting and no question.

One further property of step 3 is stated rather than assumed. **One, not many.** Bootstrap starts
exactly one participant, which is what §22 requires. The collective's size is an outcome of its own
recruitment decisions, bounded by the vector and the pool's capacity — never a target a controller
drives towards.

## 9. Dynamic recruitment

Recruitment already has its primitives in the ledger. What it lacks is a subject for the mechanical
checks v2 §12 lists (leak L-04) and a projection to the participant (leak L-10). The shape §12 draws
— *"мне нужен ещё один независимый взгляд" → recruitment request → kernel checks → Agent B joins* —
is exactly this pipeline.

    participant A            admission (supervisor)         kernel (ymp-domain)      launcher
    ──────────────────────────────────────────────────────────────────────────────────────────
    request_participant ──►  is the entry live now?
      names a catalog        probes / watches the engine
      entry, funds a         └─ not live → refuse here, and
      proposal allowance        commit EntryUnavailable{entry, reason}
      from its own account   └─ live → forward the command ──►  check: entry ∈ frozen pool
                                                                check: no live EntryUnavailable
                                                                check: disclosure class ⊆ policy
                                                                check: assurance ≥ required
                                                                check: ParticipantStarts ≥ 1
                                                                check: concurrency headroom
                                                                ├─ refuse EntryNotPermitted
                                                                └─ commit ParticipantRegistered ──► start B
    advertise ─────────────────────────────────────────────►  reserve escrow, create funded offer
                                                              (B sees the offer after its cursor)
    B: bid / decline ──────────────────────────────────────►  record consent, or nothing
    A: award ─────────────────────────────────────────────►   contract + obligation + lease, atomically
                                                              ─────────────────────────────► B works

The split in that diagram is the point, and it is what makes the claim "the kernel gains one new
kind of check" true rather than convenient (a claim an earlier revision of this design made too
loosely).

**Liveness is measured outside the kernel and enters it as a fact.** Whether an engine answers,
whether a credential is still valid, whether a version still matches — all of it is I/O, and
`ymp-domain` performs none. The catalog watcher in the supervisor measures it and commits
`EntryUnavailable { entry, reason }` and `EntryAvailable { entry }` facts. There is consequently no
`EntryNotReady` refusal in the kernel: an entry that is not live is refused at admission, before
the command is issued, and the refusal is itself recorded so the operator and the collective can
both see why.

**The kernel decides only by containment over committed facts.** Its inputs are the pool snapshot
committed at run creation — each entry with its identity, disclosure class, assurance profile and
conformance result — the availability facts committed since, the contract's policy, and the
requester's account. Every one of the six checks above is therefore a comparison of identifiers,
sets and integers over data already in the ledger. None of them reads the goal, the offer text, the
bid text, or any property that could stand in for quality; the offer's intent and the bid's
rationale reach the kernel only as digests
([`protocol.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/commitment/protocol.rs#L1-L6)).

**Reproducibility follows from the same arrangement.** A decision taken from committed facts is a
pure function of the journal, so replay reaches the same decision without re-probing anything, and
a run's evidence states exactly which entries were permitted and which had been marked unavailable
when. This is why node `W1-COR-03f` — commitment facts durable rather than only in memory — is a
dependency of this design and not a nicety.

The precise claim, then: **the kernel gains one new refusal (`EntryNotPermitted`) and one new kind
of check (containment over committed facts), instantiated for pool membership, availability,
disclosure class and assurance profile.** Nothing in it orders, ranks or prefers.

The economics the brief asks to preserve (§5) are already built and stay: a proposal allowance so a
new participant can read an offer and decline; an execution escrow transferred only when a contract
forms; a separate `InvocationStarts` charge so waking a participant is never free; irreversible
creation authority so decomposition terminates.

A participant may decline, stop, yield, expire, be cancelled, be replaced (`Reassign`) or recruit
in turn. The kernel records each transition and interprets none of them.

## 10. The `/agents` experience

`/agents` answers one question: **who is working on this, and on what?** It lists active
participants and, on a second tab, those that have finished, with raw mechanical columns only:
identifier, catalog entry (provider · engine · model), lifecycle state, the task it holds, spend so
far, and its last activity.

Participant detail adds: what it was recruited for and by whom, its budget and what it has spent by
dimension, the work it took on and returned, its published findings and challenges, its
submissions, the verification results it requested, whom it recruited, and its failures, yields and
cancellations.

Two rules bound this surface. There is **no private chain-of-thought**: the operator sees published
messages and externally visible actions, which is what the observatory records
([`CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/CONCEPT.md#L74-L78)).
And there is **no per-participant control panel**: the operator cannot assign it work, change its
model, promote it or rank it. The only participant-directed actions are the operator's own
authority — send a message, which marks intervention, and cancel the run.

## 11. Task, activity and result views

**Tasks** shows the work the collective created: what each task is, who sponsors it, who holds it,
what it depends on, and whether it has returned. Indentation expresses recorded parentage, never
priority. The internal words *obligation* and *escrow* appear only in the advanced rows.

**Activity** is the collaboration plane, labelled untrusted. Attributed messages with their kind —
proposal, question, hypothesis, observation, constraint, dead end, challenge, confirmation,
decision, help request — their audience and their evidence references. This is the surface that
lets the operator understand disagreement without reading reasoning, and it is where the brief's
§18 "they investigate, disagree, resolve" becomes visible. Its content is inert: no message is a
button.

**Result** is described in item 15.

## 12. Intervention semantics

Three different things are distinguished, because collapsing them would either make normal
operation look like interference or hide real interference:

| Act | Marks the run intervened | Why |
|---|---|---|
| Answering a question ymp asked | **no** | It is provenance class E. The product asked because it could not determine intent, and the question was funded from the run's own clarification budget, so the answer is solicited rather than unscheduled. Asking and answering is the designed path, not a deviation. |
| Sending an unsolicited message to the collective | **yes** | Unscheduled human content enters the collaboration plane and may change what participants do, which excludes the run from autonomous comparison ([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L90-L94)). |
| Pause, resume, cancel, narrowing a boundary | **no**, recorded as operator authority | These are resource and authority acts. They change what is permitted, never what a result means. |

An intervened run states so on every surface that reports its outcome, and the observation policy
decides whether it may enter an experiment. A message can never grant authority, form a contract,
choose a bid or spend escrow — the same rule that protects the collective from a malicious
participant protects it from the operator's own message (INV-4).

Two boundaries keep this distinction honest rather than convenient. A solicited answer is recorded
as solicited, with the question it answers, so an observation policy that wants to treat any human
content as intervening still can — the design does not hide the fact, it classifies it. And a
mid-run answer is inert collaboration data: it may change how the collective works and never what
the candidate is judged against, because the contract is bound once (item 16).

## 13. Verification UX

The operator sees three things about verification and never the protected material.

**While it runs.** `verifying candidate cd-3 · query 2 of 4 · requested by participant B`. The
count is the real reservation, because verification queries are scarce by design and the operator
should see them being spent.

**When it fails.** The verdict, the failure class, and whatever bounded diagnostic the run's
disclosure policy allows — nothing more, because unlimited feedback turns a fixed check into
something a candidate can be fitted to
([`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROTOCOL.md#L471-L478)).
Beside it, the operator sees that the collective has the same bounded diagnostic and what it is
doing with it. A failed verification is a normal event, presented as one.

**When it passes.** A checklist of the derived requirements, each with its provenance mark and what
observed it, plus the four digests that identify the claim: contract, candidate, environment,
oracle. The checklist also names any requirement no mechanical check observes, which is the honest
half of the same screen.

An infrastructure failure of the verifier is never a rejection, and is drawn as its own state.

## 14. Failure and terminal-state UX

Five terminals, five different sentences, five different sets of next actions. None of them is
drawn as success, and none is substituted for another.

| Terminal | What the operator reads | What is offered |
|---|---|---|
| `accepted` | the result, with its evidence | inspect, evidence, export, continue, archive |
| `exhausted` | which dimension ran out, what was produced, what was never verified | raise the boundary and continue as a new run, inspect what exists, archive |
| `cancelled` | that the operator ended it, what was interrupted, that spend is not returned | inspect, archive |
| `abstained` | that the collective stopped under an allowed stopping policy and why it published as its reason | inspect, continue, archive |
| `infrastructure_error` | what could not be established, and that this is a failure of the machinery rather than of the work | inspect the journal, export evidence, archive |

Brief v2 §15 names the operator-facing words: `✓ VERIFIED`, `BUDGET EXHAUSTED`,
`INFRASTRUCTURE ERROR`, `CANCELLED` and `NEEDS CLARIFICATION`, and requires that they never be mixed
and that an exhausted run is never reported as a success. The first four map onto the kernel's
terminals directly.

**`NEEDS CLARIFICATION` becomes a sixth kernel terminal** (change C15). Decision D6 requires the
outcome and states that it is never `abstained`, and five terminals cannot express six distinct
outcomes without one standing in for another — which is exactly what the terminals exist to prevent.
The sixth is therefore added rather than borrowed: it is distinct, it is never success, its published
reason is the unanswered question, and `Continue` resumes the work once the question is answered
instead of starting a new lineage. `abstained` keeps its own meaning — the collective stopped under
an allowed stopping policy — and the two are never substituted.

| Terminal | Operator-facing word |
|---|---|
| `accepted` | `✓ VERIFIED` |
| `exhausted` | `BUDGET EXHAUSTED`, with the dimension named |
| `cancelled` | `CANCELLED` |
| `infrastructure_error` | `INFRASTRUCTURE ERROR` |
| `needs_clarification` *(new)* | `NEEDS CLARIFICATION`, with the question published |
| `abstained` | the collective stopped, with its published reason |

Failed verification is **not** a terminal while budget and policy allow: the collective keeps the
work obligation open and reacts (§9, §15). The interface says so explicitly, so a red verdict is not
read as the end.

Two honesty rules from the accepted composition survive, one of them in a new place. A run the
operator ends is recorded as `cancelled` and never as `infrastructure_error`, unchanged. And the
operator still learns beforehand what a cancellation interrupts and that consumed budget does not
return — but on the run's own row, above the key that ends it, rather than in the typed confirmation
[`VISUAL_CONCEPT.md`](../VISUAL_CONCEPT.md) required. The sentence survives; the ceremony does not
(decision D3, leak L-16, surface S23).

## 15. Result and export flow

The result surface answers "what did I get" before "how was it made":

    completed · verified
    <one-paragraph summary of what changed, published by the collective>

    verification   6 of 6 requirements observed · 1 requirement is not mechanically checkable
    participants   3 · 5 attempts · 2 candidates
    time / spend   41 min · $2.18 · 214k tokens
    candidate      cd-3  sha256:9f2c…
    actions        [Inspect result] [Inspect evidence] [Export] [Continue] [Archive]

`Export` is the only thing that writes into the project directory, it is explicit, and it states
exactly which paths it will touch before it touches them. A candidate is never presented as ready
to apply — the operator applies it (node `W1-APP-02w.1`,
[`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L166-L167)).

`Continue` opens a new run in the same workspace, carrying the accepted candidate as its base and
the previous goal as context. It is a new run lineage against a new definition of done, and it
starts at once while the standing ceiling has headroom (decision D10).

The evidence export contains the four digests, the requirement-to-observation map, the per-model
spend, the run journal head digest and the participant transcripts as observational records. It
contains no protected oracle material and no capability material.

## 16. Internal ownership of contracts, oracles and verifiers

This is the item the whole brief turns on — items 10 and 11 of §24 together — so it is stated as a
pipeline with an owner per stage. §10 draws the same pipeline and asks only that its ownership move
inside; nothing in it is removed.

    goal (A)
      └─► repository reading ───────────────────────────► facts (B)     local · free
      └─► observable requirements ──────────────────────► requirements (A, B, C)   funded
            └─ material assumption that cannot be resolved
                 └─► question ──► operator answer ──────► requirement (E)
      └─► internal task contract  (requirements + policy + environment + budget)
            └─► acceptance plan ──────────────────────────► checks (D)              funded
                  └─ validation: negative control, substituted entry points,
                     mutation controls — a plan that does not discriminate is refused
      └─► verification strategy (which checks are protected, query budget, disclosure)

### Who performs it, and why that is not a product module

§11 forbids a hidden component that in fact takes the semantic decisions, and reading a goal to
decide what *done* means is a semantic decision — arguably the most consequential one in the
product. Placing it in an application module would therefore create precisely the thing §11 rules
out, whatever the module was called (leak L-15).

**Derivation is a collective run of its own.** It runs against a built-in package that ships with
the product and is approved once, because it describes the procedure rather than the operator's
project; it draws participants from the same AgentPool by the same mechanical bootstrap; and it has
its own budget, its own audience and its own capabilities. That is the two-run boundary already
accepted in [`VISUAL_CONCEPT.md`](../VISUAL_CONCEPT.md), kept for the reason it was built — it is
what keeps a protected plan out of reach of the participants judged by it — and no longer described
as a step the operator takes.

What remains in the application layer is a **derivation sequencer**: it starts the derivation run
when a goal is typed, advances the Task's phase when a fact says a phase finished, holds the phase
while a material question is open, and creates the Run as soon as neither holds. It reads no
goal text, writes no requirement and judges no answer. Every controller in the product is listed
with the same discipline in [`COLLECTIVE-RESOURCES.md`](COLLECTIVE-RESOURCES.md).

Two consequences are worth stating because they are checkable. The derivation run appears in
diagnostics like any other run, with its participants, its spend and what it disclosed — so the
claim "no hidden component" is inspectable rather than asserted. And a participant of the task's own
collective cannot address the derivation run as an authority, because it is a different run with a
different audience: the boundary that protects the plan is the same one that prevents a manager from
existing.

### What derivation costs, and why it is not free

The stage marked *funded* needs a model: turning "implement feature X so that it works properly"
into observable requirements is a reading of intent, and no amount of local analysis performs it.
Three honest options existed and two were rejected.

- *Derive locally, without a model.* Requirements would come from pattern matching over the
  repository, which brief §16 forbids — "do not invent details to fill gaps" — or the operator
  would have to supply them, which is the authorship the brief removes. Rejected.
- *Derive after the run starts.* The transcript could then state only the goal and a ceiling, never
  what *done* will mean, and the operator would learn what their money bought after it was spent.
  Rejected — not because it needs a gate, but because the statement is the only thing that makes the
  run legible.
- *Fund derivation separately, and say so.* Chosen. It is also the accepted position rather than a
  new concession: drafting is already specified as a run under a built-in package approved once,
  with its own budget, audience and capabilities
  ([`VISUAL_CONCEPT.md`](../VISUAL_CONCEPT.md)).

Derivation therefore has two halves with different costs:

| Half | Does | Costs | Discloses |
|---|---|---|---|
| **Reading** | walks the workspace: files, languages, build entry points, test entry points, version-control state, documentation paths | nothing; it is local file reading | nothing leaves the host |
| **Deriving** | goal → observable requirements → acceptance plan → verification strategy | the **derivation allowance**: a small ceiling in money, tokens and wall clock, separate from the run's budget | repository excerpts and the goal reach exactly one provider — the one the allowance names |

The allowance is one dimension of the workspace's standing ceiling: defaulted rather than asked for
(decision D8: $0.10 per goal), visible and editable on `/budget`, and enforced by the same machinery
as any budget dimension. It is charged per goal, not per keystroke: amending a goal re-derives and
draws on the same allowance, and the remainder is visible while it does. Exhausting it stops
derivation with what it has and says so in the transcript, rather than silently producing a thinner
plan.

The consent for what derivation discloses is given once, when the provider is enabled: decision D4
places it on the provider's properties view, above the enable key, stating that enabling sends
repository content — including the bounded excerpts used to work out what *done* means before a run
starts — to that provider. There is no second question.

One consequence is stated plainly because it is the price of the choice: a goal typed in a workspace
the operator does not want disclosed will have disclosed a bounded part of it before the run exists.
The mitigations are real but partial — the consequence is stated at the point of enabling, the
allowance is small, the excerpts are bounded, a workspace may exclude a provider as an explicit
policy, and the operator sees the provider named on S08 while derivation runs and on S10 afterwards.
D4 records this as the one decision with a confidentiality consequence, flagged to the owner.

**Provenance classes** travel with every requirement and every check, from derivation to verdict:

| Class | Source | Where the operator sees it |
|---|---|---|
| **A** | the operator's goal text | the run-start block: the restated goal; result: the checklist mark |
| **B** | the repository — tests, build entry points, documentation, configuration | the run-start block; diagnostics show the file each fact came from |
| **C** | an assumption inferred rather than established — by derivation before the run, or by a participant during it | stated in the run-start block if it existed then; published on the activity surface if it arose later; marked in the result checklist either way |
| **D** | a verification mechanism ymp generated | the result checklist column "observed by"; diagnostics show the check and its digest |
| **E** | an explicit operator clarification | the question and its answer in the transcript; marked in the result checklist |

The rule that separates class C from class E is the brief's own (§1, §8): an assumption that
materially affects the result and cannot be safely resolved from the request, the repository, the
documentation, the visible checks, the policy or what the collective discovers becomes a question.
Anything else stays a recorded assumption and is shown, never hidden. The number of questions one
run may ask is a budget dimension, so "ask more" is not a way to avoid deciding.

Assumptions arise at two moments, and the two are treated differently because the contract is
immutable once bound.

**Before the run**, derivation raises them. A material question holds the run from starting until it
is answered or the operator explicitly proceeds under the stated assumption; an unattended run whose
question is never answered ends `needs_clarification` (decision D6). Its answer becomes a
class E requirement, and the contract that is then bound already contains it.

**During the run**, a participant that meets a material ambiguity publishes it and may spend a
clarification unit to raise it to the operator. The answer is delivered as attributed collaboration
data of class E. A run names exactly one contract, the kernel refuses a second binding
([`lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/lib.rs#L283-L288)),
and INV-5 requires acceptance to name one approved package — so a mid-run answer that genuinely
changes what *done* means is a new definition of done. The product says so and offers `Continue`
with the amended goal, which starts a new run lineage against a new contract. That is the accepted
amendment rule
([`PROJECT-CONTRACT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROJECT-CONTRACT.md#L53-L62)),
kept rather than bent.

### Who decides that an answer changed the definition

"Inert" and "changes what *done* means" cannot be left as a distinction the design asserts and
nobody makes. There are two deciders, in this order, and the second one is authoritative.

**The derivation run classifies, conservatively.** It is the scope that already holds
the requirement set, so it is the one that can compare an answer against it. On every mid-run
answer it emits a typed verdict over the *recorded* requirements — not over the goal prose — with
the identifiers it names:

| Verdict | Observable trigger | Effect |
|---|---|---|
| `method_only` | the answer names no recorded requirement, negates none, and adds no observable statement | delivered as inert collaboration data; the run continues unchanged |
| `divergent` | the answer contradicts the observable statement of a recorded requirement, or states an observable condition no recorded requirement covers | a divergence fact is committed, naming the requirements, and surface S09 states it to the operator |
| `undecided` | the classification is not clearly one of the above | **treated as `divergent`** |

The threshold is that third row, and it is the whole of the safety argument: the classifier fails
towards telling the operator. A run may be interrupted by a divergence that was really only a
method note; a run may not quietly continue against a definition the operator has contradicted.

The division of labour matters as much as the threshold. The classification is a semantic judgement
and is therefore made inside a collective run; its output is a **committed fact**. The **hold** it
triggers is mechanical and is therefore enforced by the kernel, which refuses a verification-query
reservation against a diverged requirement set. Nothing about the guarantee lives in a screen.

**The operator decides, in one act.** The divergence surface offers exactly two:

- *"this does not change what done means"* — the answer stays inert, the run continues, and the
  operator's judgement is recorded beside the divergence fact.
- *"this changes what done means"* — the run stops. It records `cancelled`, which is the honest
  terminal for an operator decision
  ([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L248-L260)),
  and `Continue` opens a new run against the amended goal. Nothing about the work is lost: the
  candidates, the journal and the participants' findings stay readable and the new run may take an
  accepted candidate as its base.

**The enforcement path.** A run must not reach `accepted` against a definition the operator has
contradicted, so the guarantee is placed where it cannot be drawn around rather than in the
presentation layer. Once the operator takes the second option, the divergence is an operator act
and the run is stopped by the same command an operator cancellation uses — before any further
verification query can be spent. A verdict that arrives for a candidate submitted earlier is
recorded as evidence and does not resurrect the run. Where the operator has not yet answered the
divergence, the run continues but the acceptance path is held: no verification query may be spent
against the diverged requirement set until the operator has taken one of the two options, and the
surface says which one it is waiting for.

**The negative half.** The check that proves this is not decoration: an answer that plainly negates
a recorded requirement — R2 says the token endpoint uses Authorization Code, the operator answers
"no, it must be Client Credentials" — must be classified `divergent`, must hold the acceptance
path, and must be capable of ending the run as `cancelled`. A build in which that answer is
classified `method_only`, or in which a verification query is spent after it, fails tests 27 and 28
of item 22.

The practical consequence is a design constraint on derivation rather than a limitation the
operator feels: a question that would change the requirement set should be found *before*
the run starts, which is why the reading half runs first and why the clarification budget is spent
mostly there.

**What does not change.** Protected checks may instantiate only public requirements and may never
add secret ones; negative controls must discriminate or the run ends as `infrastructure_error`
rather than accepted; the exact digests are recorded; the adaptive query budget is bounded; the
verifier's first assessment is independent of the producers' conversation
([`PROJECT-CONTRACT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROJECT-CONTRACT.md#L73-L112)).
The eleven package contents survive as the internal contract's contents. Only the author changes.

**Where the derived plan lives.** It is protected material, so it is produced in a separate scope
from the participants judged by it — the two-run boundary already accepted in
[`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L126-L141),
with different budgets, audiences and capabilities. The design keeps that boundary and stops
describing the derivation as a step the operator takes (leak L-15). It is invisible unless diagnostics
are opened.

**What ymp still refuses.** A plan it cannot show to discriminate does not decide a run. The
refusal is stated as ymp's own limit and offers the operator a narrower goal, never a request to
write a check: *"the goal asks for a faster parser; ymp can check that the parser still accepts the
declared grammar and that the benchmark improves by a stated margin, but not that the code is
'clean'. Start with the checkable part, or state the margin."*

## 17. Provider, model, runtime, participant

The distinctions the brief asks to preserve and make explicit (§3), each with where it lives:

| Concept | Is | Is not | Lives in |
|---|---|---|---|
| **Provider** | an account with an authentication state that exposes models | a runtime | provider record under the product root |
| **Model** | a capability source named by identifier or snapshot | an agent | catalog entry field |
| **Engine (agent runtime)** | a harness that owns a reasoning and tool loop | a model | catalog record, node `W1-APP-02e.6` |
| **Runtime driver** | the compiled ymp implementation that supervises one engine protocol | a user-level role | `ymp-runtime-*` crates, internal |
| **Model route** | the provider, endpoint class, wire protocol, account scope, model identifier, authentication mode and disclosure class an engine reaches a model by | an engine, and not a role | engine profile's route override; one conformance profile per pairing |
| **Catalog entry** | one admissible provider · engine · model triple with measured properties | a role, a rank or a team slot | catalog |
| **Participant** | an actual autonomous member of the collective, with a principal, a budget account and a lifecycle | a model | ledger `ParticipantRecord` |
| **Attempt** | one bounded period in which a participant works on a task from a base | a participant | ledger |
| **Invocation** | one supervised process slice that starts or resumes a participant | an attempt | ledger |

These identities are already independent in the code
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L210-L226));
what the redesign adds is the provider and catalog levels above them, and the operator-facing
vocabulary that keeps them apart on screen: the operator sees providers and models on
configuration surfaces and participants on activity surfaces, and never a screen that mixes the
two as if they were the same list.

## 18. AgentPool and recruitment together

    providers ──► model catalog ──► AgentPool ──► frozen snapshot ──► recruited participants
    (enabled)     (measured)        (declared,     (per run, by       (created one at a time,
                                     default        digest)            each paid for)
                                     automatic)

- **Availability** is measured: an entry is admissible only if its provider is enabled and ready,
  its engine is enabled and ready, and its pairing passed whatever conformance probe applies.
- **Permission** is declared, but not by a question at request time: the pool is a resource with an
  automatic `default`, and the Task references it by name.
- **Reproducibility** is frozen: the run holds the resolved entries by value with their digest.
- **Instantiation** is requested: only `request_participant` creates a participant, only a
  participant issues it, and it costs a `ParticipantStarts` unit from the requester's own account.

The kernel's whole involvement is containment over committed facts — pool membership, availability,
disclosure class, assurance profile — plus the resource and capability checks it already performs.
Whether an entry is live is measured outside it and arrives as a fact (item 9). It holds no preference order over the pool beyond the declared one, no notion that one
entry suits one task, and no fallback that silently substitutes another entry. A refusal names the
mechanical reason and creates nothing.

Convenience labels such as *reasoning*, *fast* or *cheap* are optional groupings on catalog entries
that a participant may read when deciding what to ask for. They are resource policies. They are
never inputs to a kernel decision, and no code path may turn a label into a selection.

The same rule bounds pools themselves. A `cheap` pool holding the cheaper Claude routes is a
resource boundary and a legitimate thing to create — it is how the owner's recorded position on
experiment cost is expressed. An `architect-pool`, a `coder-pool` and a `reviewer-pool` are not:
they would encode a role system the product does not have, and §16 rules them out as a mandatory
structure. If a collective uses one model as an architect and another as a reviewer, that is its own
semantic decision, taken per task and visible on the activity surface — never a property of a
capability list.

## 19. Exact boundaries

**Operator may**: state a goal; enable providers; edit or create a pool; narrow the boundaries;
answer a question; observe every surface; pause, resume, cancel; send a message (marking
intervention); inspect result and evidence; export; continue; archive.
**Operator must never be required to**: author a contract, acceptance criteria, an oracle, a
verifier or a negative control; create or name a pool; state a permitted model set; create a task
inside the work; create or name an agent; assign a model; set the number of agents; write a
decomposition; assemble a team; start review or verification by hand; approve a plan item by item.

**Collective may**: read the repository; investigate; publish and challenge; identify ambiguity;
decompose; sponsor offers; recruit from the permitted pool; choose which entry to request; bid,
decline, counter; delegate and take delegation; implement; review; request verification; react to
failure; revise; abandon; synthesize; stop when demonstrated.
**Collective must never**: create authority, budget or capability; write control or verification
state; read protected oracle material before the disclosure point; obtain authority from another
participant's message.

**Kernel may**: authenticate and validate transitions; reserve, transfer and consume budget;
form contracts atomically; issue leases and fencing tokens; track obligations; enforce isolation;
check catalog-entry containment against committed facts, capabilities, disclosure classes and the
assurance profile; admit
by per-principal round robin; record candidates, verdicts and terminals.
**Kernel must never**: choose an agent or a model; assign a role; decompose; rank or score a bid, a
participant or a candidate; synthesize an answer; read natural language as a command; judge
semantic quality; prefer one pool entry over another for any reason but the declared order.

**Verification may**: execute the approved plan against one exact candidate in its own boundary;
record evidence; disclose a bounded diagnostic.
**Verification must never**: consult the producers' conversation before committing an independent
verdict; accept a candidate that has altered the pinned check; count an infrastructure failure as a
rejection; be written by a participant.

**TUI may**: present projections; carry the operator's four decisions; open diagnostics.
**TUI must never**: implement a kernel rule; rank anything; make a participant's suggestion
actionable in one keypress; present a terminal state as another; hide the path to a fact.

## 20. Required domain, API and state changes

Numbered `C-nn` so they are never confused with the owner decisions `D-nn`.

| # | Change | Closes | Note |
|---|---|---|---|
| C1 | `ContractDocument` gains `requirements: Vec<Requirement { id, statement, provenance, observed_by }>` | L-01 | provenance is `A|B|C|D|E` |
| C2 | `ContractDocument` gains `acceptance_plan: Vec<Check { id, program_digest, kind, protected, observes }>`; the single `verifier` becomes one check of that plan | L-02, L-03 | validation of each check is unchanged |
| C3 | `ContractDocument` gains `policy: RunPolicy { pool_digest, ceilings, disclosure_classes, assurance_profile }` | L-08 | the frozen pool travels with the definition of done |
| C4 | **New stored resource `AgentPool`**: a declared half — `models` (the whole catalog or an explicit ordered list), `capacity`, resource limits — and a resolved half with the ordered entries and their digest | L-07, L-08 | the automatic `default` is created by its reconciler, never seeded |
| C5 | `Task` as a stored resource: goal, workspace, `agentPool` (defaults to `default`), optional boundaries and base; **no acceptance, verifier, model, role or count field** | L-02, L-04, L-07 | `RunRequest.acceptance` retires with it |
| C6 | `Run` as a stored resource whose `declared` is immutable after creation: contract digest, `poolSnapshot` by value, `entryModel`, budget, policy | L-08, L-12 | reference by value is what makes the freeze real |
| C7 | New typed object `Clarification { id, question, options, answer, asked_at }`, and budget dimension `ClarificationRequests` | L-15 | answers become class E |
| C8 | `RegisterParticipant` gains `requested_entry: CatalogEntryRef` | L-04, L-10 | the subject §12's mechanical checks need |
| C9 | New kernel refusal `EntryNotPermitted { participant_id, entry, reason }` | L-04 | containment only; creates nothing. `EntryNotReady` is **not** a kernel refusal: liveness is measured in admission (item 9) |
| C10 | New facts `PoolFrozen { entries, entry_model, digest }`, `EntryUnavailable { entry, reason }`, `EntryAvailable { entry }` | L-08, L-12 | committed by the launcher, admission and the catalog watcher, so the kernel decides without I/O and replay reproduces the decision |
| C11 | New fact `RequirementsDiverged { answer, requirements }`, its operator resolution, and the kernel's refusal of a verification reservation while it is unresolved | item 16 | the hold is a kernel rule, not a screen |
| C12 | `RunState.budget` becomes a projection of the root participant's `BudgetVector`; the two-dimension `Budget` retires | — | one accounting; enforcement unchanged |
| C13 | The single-active-attempt refusal becomes a policy ceiling checked from the vector and the concurrency limit | L-11 | the isolation rule it protected is unchanged |
| C14 | Participant tool projection extended to the accepted families: observe, communicate, contract, lifecycle, artifact, verification | L-10 | schemas already in [`PROTOCOL.md`](../PROTOCOL.md) |
| C15 | A sixth terminal `needs_clarification`, distinct from `abstained`, never success, resumable by `Continue` | item 14 | required by decision D6; INV-7's wording follows |
| C16 | The workspace's **standing ceiling** as a stored object: the budget vector, the derivation allowance and the disclosure classes, defaulted from decision D8 and edited in place on `/budget`. `Run.declared.budget` is drawn from it at run creation | L-16 | replaces per-run authorization; the goal sentence and this ceiling are the human approval |
| C17 | Every public command takes its subject from the open run or a named row; the `--confirm <ID>` argument retires | L-16 | node `W1-APP-02n` is restated, not weakened: parity stays, the ceremony goes |

Two changes are already **built and accepted** and are consumed rather than proposed: the provider
record with its observed state and credential origin, and the derived model catalog
(`W1-PRD-05b`). Two obligations attach to them and are carried by migration unit P2: the stored
provider state must carry the age of its observation, and a deleted engine record must read as a
route with no models rather than shortening the catalog silently.

Two more state changes follow from item 16: a **derivation allowance** as a policy field with its own
disclosure class and its own ceilings, and the `Result` projection with the outcome vocabulary of
§15.

No change removes a precondition, a reservation, a digest or a refusal. C8–C11 and C15 are the only
kernel-facing additions: C9 is a containment refusal, C10 is the fact stream that lets it decide
without input or output, C11 is a refusal that holds an acceptance path, and C15 is a sixth distinct
terminal. `ymp-domain` performs no I/O before or after this design.

C16 and C17 remove ceremony and add no permission: the standing ceiling is enforced by the same
budget dimensions as before, and a command whose subject is a selection reaches exactly the same
kernel commands as one whose subject was typed. What disappears is the typing, not a check.

## 21. Required Rust module and crate changes

| Crate | Change |
|---|---|
| `ymp-domain` | C1–C3, C6–C13: contract document, provenance, clarification object, catalog entry reference, the containment refusal, the pool-snapshot, availability and divergence facts, budget projection. No file, network or process access is added: every new decision reads committed facts |
| `ymp-application` | the **derivation sequencer** — phases, not semantics: it starts the derivation run, advances the Task's phase on committed facts, holds it while a question is open and creates the Run as soon as neither holds; `prepare_contract` stops requiring an operator acceptance condition and consumes the derived plan; clarification service; task and run listing under a workspace |
| `ymp-runtime-registry` *(exists)* | provider records, engine records and the derived catalog are **already accepted**; P2 adds the observation age, the enabled-flag gate on probing, and a route with no models reading as such |
| **`ymp-pool`** *(new)* | the AgentPool resource and its reconciler: the automatic `default`, selector resolution, admissibility, ordering, the digest. Creates nothing and starts nothing |
| **`ymp-conduct`** *(new, composition)* | the run launcher and the catalog watcher: freezes the pool, registers the origin participant, commits availability facts, starts and resumes invocations by following committed facts. Depends on `ymp-application` and `ymp-runtime-supervisor`; nothing depends on it but `ymp-cli` |
| `ymp-runtime-api` | catalog entry as the profile's model source; the pinned model becomes a property of an entry rather than a constant the profile refuses to deviate from; a **route override** on an engine profile — endpoint class, wire protocol, account scope, authentication mode, disclosure class — so a provider is reached through an installed engine without a new engine crate |
| `ymp-runtime-supervisor` | the one-attempt constant becomes a policy ceiling; invocation start accepts the entry the ledger recorded; admission measures entry liveness and refuses before the recruitment command is issued |
| `ymp-agent-api`, `ymp-agent-mcp` | C14: the extended tool projection, one tool per accepted domain command, gated by the invocation's contract |
| `ymp-tui` | the surfaces of [`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md); startup reduced to the accepted start screen; the authorization surface, the setup question and every typed-identifier confirmation removed; drops `ymp-runtime-claude` and `ymp-runtime-codex` and reads providers, catalog and pools through application ports |
| `ymp-cli` | injects the launcher into the session; mirrors every new interface action as a command; the public `--runtime` argument retires, since which agent works is not the operator's decision |
| `ymp-verifier` | unchanged in mechanism; executes plan checks rather than one program |

The dependency direction is deliberate: `ymp-runtime-supervisor` already depends on
`ymp-application`, so the launcher cannot live in either. Putting it in a composition crate keeps the
interface free of runtime dependencies, which is what finally removes driver construction from
`ymp-tui` (leak L-05) — node `W1-APP-02s` already removed the fixture runtime from that list.

## 22. Required tests and acceptance scenarios

Each scenario states its negative half — the condition under which the check fails — because a
check that cannot fail proves nothing.

**Ownership**

1. A goal in a project with no tests starts a run against a derived plan. *Negative half:* today's
   refusal demanding a hand-written verifier.
2. No path from goal to spend exists that asks the operator for a verifier, a negative control, a
   task, an agent or a model. *Negative half:* a build in which any such prompt is reachable.
3. The internal contract is complete without operator input: every field of the accepted package
   has a value and a provenance. *Negative half:* a field left `None` by derivation.

**Bootstrap and recruitment**

4. A goal that starts a run starts exactly one participant. *Negative half:* zero (today) or more
   than one.
5. A catalog of one hundred entries with no recruitment produces one participant and one hundred
   uninstantiated entries. *Negative half:* any participant created by catalog presence.
6. A second participant exists only after a committed `request_participant`, and its parentage names
   the requester. *Negative half:* a participant with no recruiting fact.
7. `request_participant` naming an entry outside the pool is refused with `EntryNotPermitted`, and
   no participant, budget movement or process results. *Negative half:* admission, or a fallback to
   another entry.
8. Participant starts are bounded by `ParticipantStarts`; the ceiling is reached and the refusal
   names the dimension. *Negative half:* a start funded from another dimension.

**The kernel decides nothing semantic**

9. Mutation: a kernel that orders pool entries by any property other than declared order fails the
   suite. *Negative half:* the mutant survives.
10. Mutation: a kernel that reads the intent digest, the offer text or the bid text as anything but
    an opaque value fails. *Negative half:* the mutant survives.
11. Two identical recruitment requests differing only in goal text receive identical kernel
    decisions. *Negative half:* the decision changes with the text.

**Derivation and provenance**

12. Every acceptance-plan check names at least one requirement, and every requirement carries a
    class. *Negative half:* an orphan check or a classless requirement.
13. A material assumption with no safe resolution produces a question; the run does not proceed past
    from starting until it is answered or the operator explicitly proceeds under the stated
    assumption. *Negative half:* a silently invented requirement.
14. The clarification budget bounds the number of questions; exceeding it stops the run honestly
    rather than asking again. *Negative half:* unbounded questioning.
15. A derived plan that accepts the negative control never starts a run, and neither does a plan
    that accepts a substituted entry point (nodes `W1-APP-02u`, `W1-APP-02z`). *Negative half:* the measured false accept those nodes recorded.

**Verification and terminals**

16. A failed verification with budget remaining leaves the run running, the obligation open and the
    candidate immutable. *Negative half:* automatic termination.
17. Each of the six terminals is reachable and is reported with its own text; `exhausted` is never
    reported as a result. *Negative half:* substitution of one terminal for another.
18. Verifier infrastructure failure is recorded as such and is not a rejection. *Negative half:*
    counted as a failed candidate.
19. Acceptance evidence names contract, candidate, environment and oracle digests, and the exported
    evidence repeats the journal head digest. *Negative half:* any missing digest.

**Interface and delivery**

20. Every new interface action exists as a command of the same executable and cannot skip a
    confirmation (node `W1-APP-02n` extended). *Negative half:* an action without a command.
21. A run started from a clean directory leaves it untouched; only export writes into it, and it
    states its paths first (node `W1-APP-02w.1`). *Negative half:* any other write.
22. The shipped binary links no fake runtime and offers no profile it cannot start (node
    `W1-APP-02s`). *Negative half:* the fake appearing in `/models`.
23. An operator answer to a question does not mark the run intervened; an unsolicited message does.
    *Negative half:* either mark applied to the other act.

**Derivation cost and disclosure**

24. Nothing leaves the host before a provider is connected and its disclosure class is set: the
    reading half runs entirely on local files and opens no network connection. *Negative half:* a
    build in which a goal typed at first run reaches a provider.
25. Derivation charges the derivation allowance and never the run's budget, the two are enforced
    separately, and exhausting the allowance stops derivation with a stated result rather than a
    thinner plan. *Negative half:* derivation drawing on the run's ceiling, or continuing past it.
26. The run-start block states what derivation spent and which provider it disclosed to, and the
    figures match the recorded charges. *Negative half:* a transcript claiming nothing was spent.

**The divergence judge**

27. An answer that negates a recorded requirement is classified `divergent`, holds the acceptance
    path, and can end the run as `cancelled` on the operator's act. *Negative half:* the same answer
    classified `method_only`, or a verification query spent while the divergence is unresolved.
28. An unclassifiable answer is treated as `divergent`. *Negative half:* an `undecided` verdict that
    lets the run continue silently.

**The kernel decides from facts alone**

29. `ymp-domain` opens no file, socket or process: the recruitment decision is a pure function of
    committed facts, and replaying a journal reproduces every admission and refusal without
    re-probing. *Negative half:* a domain path that measures liveness, or a replay that diverges.
30. An entry that went unavailable after the pool was frozen is refused at admission with the
    reason recorded as a fact, and the kernel's refusal names containment rather than readiness.
    *Negative half:* a kernel refusal that required a live probe.

**Providers and routes**

31. Three providers are configurable on a host with two installed engines, because a provider is
    reached through an engine route; each pairing carries its own conformance result. *Negative
    half:* a provider offered with no engine able to reach it, or a pairing admitted without its
    own probe.

**The pool is a boundary and appears by itself**

32. Enabling one provider with at least one admissible entry creates a pool named `default`
    tracking the catalog, without an operator act. *Negative half:* a build in which `/pools` is
    empty after a provider is ready, or in which any surface says `No AgentPool configured`.
33. From an unconfigured product, enabling one provider and typing a goal starts a run against a
    non-empty pool with no question about models, counts or roles. *Negative half:* any
    prompt for a permitted set, a model, a count or a role between the goal and the spend
    decision — which is what the current design set does (leak L-08).
34. A pool of three entries and a run that recruits twice produces two participants, not three, and
    not one per entry. *Negative half:* one participant instantiated per pool entry, or a
    controller creating participants to reach a count.
35. Editing `default` replaces tracking with the explicit list, the surface says so before it
    applies, and a provider enabled afterwards does not silently rejoin the pool. *Negative half:*
    an edit silently overwritten by the reconciler, or a tracking pool that ignored the edit.
36. A pool edited while a run is live does not change that run: its snapshot digest and its
    admissible entries are unchanged, and the next run gets the edited pool. *Negative half:* a
    running run whose permitted set moved under it.

**Startup and provider discovery**

37. With no provider enabled, launching ymp opens no process and no network connection, and the
    start screen is the logo, one line of basics and the invitation. *Negative half:* a probe at
    launch (leak L-13), or startup text beyond the accepted screen contract (leak L-14).
38. Enabling a provider is what triggers autodetect; disabling one stops it being probed and marks
    its entries not offered with the recorded reason. *Negative half:* an autodetect that precedes
    an enable.

**No component outside a collective reads the goal**

39. The goal text reaches exactly two places: the derivation run and the task's own collective.
    No controller, reconciler, sequencer or kernel path reads it. *Negative half:* any mechanical
    component branching on goal content.
40. The derivation run is visible in diagnostics as an ordinary run with its participants, its spend
    and its disclosures. *Negative half:* derivation performed by a component with no run, no budget
    and no record.

**No ceremony, and no identifier typed back**

41. No input field in the product accepts a run identifier, a candidate digest, a participant
    identifier or a workspace hash; every act takes its subject from a selection or from the open
    run. *Negative half:* any surface or command that asks the operator to type back a value the
    product already holds — which is what three public commands do today
    ([`surface.rs:78-101`](../../ymp-rust/crates/ymp-cli/src/surface.rs)).
42. A goal typed against a ready pool starts a run with no intervening screen, no confirmation and
    no acknowledgement, and the standing ceiling is what bounds it. *Negative half:* any dialogue
    between the sentence and the collective.
43. Pause, cancel, export and archive are each one key on a selected object whose consequences are
    stated on that object beforehand; each remains recorded, and none destroys a journal, a
    candidate or evidence. *Negative half:* a modal, a typed identifier, or an act that leaves
    nothing readable behind.
44. Each of the six terminals is reachable and reported with its own word, `needs_clarification`
    among them and never as `abstained`. *Negative half:* five terminals expressing six outcomes.

**End to end**

45. The acceptance scenario of v2 §22, driven on the built product: three providers configured, one
    sentence typed, a default pool that already exists, one participant started, a second recruited
    by the collective, collaboration, a candidate, a failed verification, a revision, a passing
    verification, and `COMPLETED · VERIFIED` with result, evidence, cost, time and participants —
    then Inspect, Export, Continue or Archive. *Negative half:* any step that requires the operator
    to create a contract, oracle, verifier, team, agent, role, model assignment or decomposition.
    §22 states the consequence itself: if the operator still has to, the ownership boundary is still
    in the wrong place.


## 23. Migration path

Unit by unit, in [`COLLECTIVE-MIGRATION.md`](COLLECTIVE-MIGRATION.md): what survives as built, what
changes owner, what closes, and what the correction proposes as new work. Summary:

- **Survives unchanged:** the commitment kernel and its accounting (`W1-COR-03a`, `03b`,
  `03g`–`03s`), verification exactness and pinning (`W1-APP-02b`, `02g`, `02u`, `02z`,
  `W1-EXP-01a`), launch admission (`W1-APP-02p`, `02q`, `02r`, `02k`), journal and recovery
  (`W1-APP-02a`, `02f`), per-model spend (`W1-APP-02l`, `02y`), command parity (`W1-APP-02n`),
  chat-first composition (`W1-APP-02e.2`, `02e.3`), the start screen (`W1-APP-02e.5`), the product
  root (`W1-APP-02w`, `02w.1`, `02w.3`), the engine registry (`W1-APP-02e.6`), the fixture-runtime
  unlink (`W1-APP-02s`), and the provider and catalog records (`W1-PRD-05b`).
- **Changes owner:** the contract dialogue (`W1-APP-02v`) and the verifier answer (`W1-APP-02u`)
  become the derivation run; the generated verifier (`W1-APP-02z.3`) becomes the acceptance plan;
  the pinned profile probe (`W1-APP-02e.4`) becomes catalog readiness.
- **Closes:** the operator-facing acceptance condition; the public `--runtime` argument and the
  "which agent does the work is yours to decide" refusal; the second operator act that starts work;
  the single-run store; the two-dimension run budget; the single-active-attempt constant; the
  roster sentence in the document of record.

## 24. Decisions that genuinely need the owner

Item 16 of §24, and the honest answer is: **none, as of 2026-08-15.** The owner's instruction —
*resolve as many questions as possible by the application's logic* — was applied to every decision
this design set carried, and all eleven are settled in
[`COLLECTIVE-OWNER-DECISIONS.md`](COLLECTIVE-OWNER-DECISIONS.md), where the reasoning behind each
stays readable and the owner may overturn any of them. This design is written against them as
inputs, not as assumptions.

| # | Decision | State |
|---|---|---|
| D1 | Where the permitted model set is declared | **decided** — and refined by brief v2 into the AgentPool resource with an automatic `default` |
| D2 | The entry rule for the origin participant | **decided** — first ready entry of the frozen pool in declared order, recorded in the freeze |
| D3 | Whether the single authorization stays mandatory | **decided** — no: a standing per-workspace ceiling, and a goal starts a run at once |
| D4 | The disclosure default | **decided** — enabling a provider is the consent, stated on its properties view before the enable |
| D5 | The semantic remainder | **decided** — `VERIFIED` covers what was observed; the remainder is named on the same line |
| D6 | The clarification budget | **decided** — assume where safe, ask only genuine intent ambiguity, at most three, then `NEEDS CLARIFICATION` |
| D7 | Diagnostic disclosure for a derived contract | **decided** — one conservative default, not a setting |
| D8 | Default ceilings | **decided** — setting *working* as the standing ceiling placeholder, editable on `/budget` |
| D9 | Naming | **decided** — *workspace* is the project, *attempt sandbox* the private copy |
| D10 | Whether `Continue` re-authorizes | **decided** — no: it starts at once while the ceiling has headroom |
| D11 | Does `AgentPool` carry a floor? | **decided** — no: `maxAgents` and concurrency only, because a `minAgents` would be the only field with no reconciler |

**Nothing on that page is outstanding.** The eleventh entry was the last one open — Part B lists
minimum, desired and autoscaling bounds while Part A §2 makes min and max constraints rather than
targets and §22 fixes bootstrap at one participant — and it is settled the way the other ten were,
by asking what the application could actually reconcile.

Two further questions are resolved here by application logic rather than referred, in the spirit of
the same instruction, and are recorded as design decisions rather than as questions for the owner:

- **`NEEDS CLARIFICATION` becomes a sixth kernel terminal.** D6 requires the outcome and states it is
  never `abstained`; five terminals cannot express six distinct outcomes, and folding it into
  `abstained` would break the rule that terminals are never substituted. Change C15 adds it, and it
  is distinct, never success, and resumable by `Continue` (item 14, item 20).
- **An edited pool stops tracking the catalog.** A tracking pool that overwrote an operator's edit
  and an edited pool that silently absorbed a new provider are both dishonest; writing an explicit
  list is therefore what "stop tracking" means, and the surface says so at the moment of the edit
  (item 7).

---

## The sixteen acceptance criteria of brief v1 §17

Kept for traceability: `W1-PRD-05a` was accepted against them, and v2 refines rather than repeals
them. Each is still satisfied, now by the corrected items.

| # | Criterion | Where satisfied |
|---|---|---|
| 17.1 | Launch without authoring a contract or verifier | Items 4, 16; C1–C2, C5; tests 1, 2 |
| 17.2 | Providers configurable and visible | Items 5, 6; accepted in `W1-PRD-05b`; tests 37, 38 |
| 17.3 | 100 models never become 100 agents | Items 6, 7, 18; tests 5, 34 |
| 17.4 | No manual team construction | Items 7, 8, 19; tests 4, 33 |
| 17.5 | The collective recruits from the permitted pool | Items 7, 9, 18; tests 6, 32 |
| 17.6 | Recruitment bounded by kernel resources and authority | Item 9; C8–C10; tests 7, 8 |
| 17.7 | The collective, not the kernel, makes semantic decisions | Items 16, 18, 19; tests 9, 10, 11, 39 |
| 17.8 | Internal contracts and verification stay strict, not prerequisites | Item 16; test 15 |
| 17.9 | Questions only on genuine intent ambiguity | Item 16; tests 13, 14 |
| 17.10 | The TUI exposes the full lifecycle | Item 3; [`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md) |
| 17.11 | Activity understandable without chain-of-thought | Items 10, 11 |
| 17.12 | Results carry verification evidence | Items 13, 15; test 19 |
| 17.13 | Failed verification does not auto-terminate | Item 14; test 16 |
| 17.14 | Terminals stay distinct | Item 14; test 17; C15 adds the sixth |
| 17.15 | Kernel trust boundaries intact | Item 19; gap analysis §5; tests 9–11, 15, 18 |
| 17.16 | "Give the collective a goal, get a verified result" | Items 1, 2; test 45 |

None of the sixteen now waits on an owner answer. 17.1 is settled by D3 (no authorization step),
17.5 by D1 and D2, 17.12 by D5 and 17.14 by D6 with change C15.

## The acceptance test of v2 §22

The scenario is walked **screen by screen** in
[`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md#the-acceptance-scenario-of-22-screen-by-screen), which is
where it belongs: §22 is a test of what the operator has to do, and that is a question about
surfaces. The design-level result of that walk is one line long.

**Five operator acts before the result:** launch, enable three providers, type one sentence, press
one option key for the clarification. Then, optionally, one key to inspect, export or continue. **No
confirmation is given and no identifier is typed at any point.** The operator creates no contract, no
oracle, no verifier, no team, no agent, no role, no model assignment and no decomposition — and is
not asked for a permitted model set, a participant count, a pool or a runtime profile.

Two things the walk states rather than hides. Enabling a provider is what starts autodetect, so the
three enables are the only configuration the product ever requires, and the disclosure they permit is
stated above the key that performs them. And repository excerpts reach one named provider between the
goal and the run, because working out what *done* means needs a model; the operator has consented to
that at the enable, and the run-start block restates what it cost.

## Required updates to the documents of record

Recorded here rather than applied, per the scope of this card. Each is a wording change the design
implies; none changes a mechanism.

| Document | Change |
|---|---|
| [`PROJECT-CONTRACT.md:55-58`](../PROJECT-CONTRACT.md), [`:64`](../PROJECT-CONTRACT.md) | "Draft, approval, and amendment": the draft is produced by a derivation run, not by a structured interview with the operator; the human approves a spend against a restated goal. The eleven package contents and all validation stay. |
| [`PROJECT-CONTRACT.md:12-46`](../PROJECT-CONTRACT.md) | Package contents gain requirement provenance (A–E) as a recorded field. |
| [`VISUAL_CONCEPT.md:117-123`](../VISUAL_CONCEPT.md) | "Roster and recruitment": the sentence "the operator sets the starting roster of participants and a ceiling on how many more may be recruited" is replaced. The ceiling survives as pool capacity and a budget dimension; the roster is the collective's own outcome (leak L-07). |
| [`VISUAL_CONCEPT.md:87-99`](../VISUAL_CONCEPT.md) | The contract-authorization screen is removed. The goal sentence and the workspace's standing ceiling are the human approval (decision D3); the coverage map is retained as diagnostics. |
| [`VISUAL_CONCEPT.md:257-260`](../VISUAL_CONCEPT.md) | The typed cancel identifier is removed. A cancellation is a key on the run's own row with its consequences stated above the key (owner ruling, 2026-08-15; leak L-16). |
| [`CONCEPT.md:163-171`](../CONCEPT.md) | Constitutional constraint 1 is restated: the human-approved definition of done is the operator's goal sentence together with the workspace's standing ceiling, both recorded, rather than a per-run approval act. |
| [`VISUAL_CONCEPT.md:112-117`](../VISUAL_CONCEPT.md) | The naming constraint on *workspace* is superseded at product level. |
| [`ARCHITECTURE.md:227-228`](../ARCHITECTURE.md), [`CONCEPT.md:245-246`](../CONCEPT.md) | The origin participant is derived from the frozen pool by a mechanical rule, not named by configuration (leak L-12). The ignition-point guard at [`CONCEPT.md:122`](../CONCEPT.md) stays as written. |
| [`ARCHITECTURE.md:82-88`](../ARCHITECTURE.md) | Provider enablement and authentication become product surfaces that route to the engine's own authentication, rather than prerequisites arranged elsewhere; and the operator does not select a ready profile, because which engine works is not their decision. |
| [`ARCHITECTURE.md`](../ARCHITECTURE.md) | Add the provider, catalog and AgentPool levels above the runtime profile, and the run launcher as a composition component with no semantic authority. |
| [`PROTOCOL.md`](../PROTOCOL.md) | `request_participant` names a catalog entry; the containment refusal is added; a clarification command and its budget dimension are added. |
| [`CONCEPT.md`](../CONCEPT.md) | The product goal is restated as goal in, verified result out; the approved contract becomes an internal object. |
| [`INVARIANTS.md`](../INVARIANTS.md) | INV-1 gains the explicit sentence that catalog-entry membership is a set operation and that the kernel holds no preference order over the pool beyond the declared one. |

## Self-check against v2 §19 and §11

### The ten ownership leaks §19 names

| Named by §19 | Where it is closed | Leak |
|---|---|---|
| `PROJECT-CONTRACT.md` required from the user | Item 16: the package survives as an internal object filled by a derivation run | L-01 |
| Verifier creation required from the user | Item 16: the plan is derived and demonstrated; the refusals that asked for one retire | L-02, L-03 |
| Manual agent creation | Item 8: the launcher starts participant one; the collective recruits the rest | L-09 |
| Manual team assembly | Items 7, 8: no roster exists; the collective's size is its own outcome within capacity | L-07 |
| Manual model assignment | Items 7, 18: a participant names a catalog entry; nothing assigns one and no surface asks | L-04, L-08 |
| Manual decomposition | Item 9: the collective sponsors, bids, awards and delegates | L-10 |
| Provider configuration mixed with participant creation | Items 5, 6, 18: a catalog entry instantiates nothing, and configuration surfaces create nothing | L-05 |
| Runtime profiles exposed as user-level roles | Items 17, 21: the driver is internal, the engine level is unadvertised, the public `--runtime` argument retires | L-04, L-05 |
| Acceptance machinery in the operator workflow | Item 2 and surface S10: one spend decision; the coverage map is diagnostics | L-06 |
| A hidden manager/orchestrator taking semantic decisions | Item 16 and [`COLLECTIVE-RESOURCES.md`](COLLECTIVE-RESOURCES.md): every controller is listed with what it may not do, and the two semantic stages are collective runs | L-15 |

### The three questions §11 makes it fair to ask

**Is there a hidden manager?** Every mechanical component in the design is enumerated in
[`COLLECTIVE-RESOURCES.md`](COLLECTIVE-RESOURCES.md) with what it may never do: a provider observer,
a catalog reader, a catalog watcher, a pool reconciler, a derivation sequencer, a run launcher and a
verifier controller. None of them reads the goal; none creates a participant of its own accord; none
holds a plan. The two stages that are genuinely semantic — deriving what *done* means, and doing the
work — are collective runs with their own budgets, audiences and capabilities, and neither can be
addressed by a participant as an authority. Tests 39 and 40 exist to kill a build in which that
stops being true.

**Is the entry rule a model assignment in disguise?** Item 8 answers it in full rather than in a
clause. Something must run first; the collective cannot choose because it does not exist yet, and
the operator must not be asked because that is the assignment §1 forbids. What is left is an
ordering, and the ordering this design uses reads position and availability and nothing else, is
computable from committed facts, and is recorded in the freeze so evidence names it. What it is not
is free of consequence — the first entry of `default` decides which model every run of that pool
ignites on — which [decision D2](COLLECTIVE-OWNER-DECISIONS.md#d2--the-entry-rule-for-the-origin-participant)
records rather than hides, and which the operator changes by reordering the pool rather than by
answering a question. Tests 9 and 11 kill a build in which the rule starts reading anything else.

**Did the kernel become a model router?** The kernel gained one refusal and one kind of operation:
containment of a requested catalog entry against facts already committed. It holds the frozen pool
as a set plus a declared order, and the order is used in exactly one place — the ignition entry at
bootstrap — where some mechanical rule is unavoidable. No kernel path reads any property of the
goal, the task or the entry other than membership, availability, disclosure class, assurance profile
and reserved resources. §1's list of what the kernel checks is exactly that list, and §1's sentence
about what it must never do — decide which model is "better" — has no representation anywhere in it.

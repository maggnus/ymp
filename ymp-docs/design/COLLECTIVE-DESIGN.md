# The collective design

The design deliverable for [`PRODUCT-BRIEF-collective.md`](PRODUCT-BRIEF-collective.md) and its
owner addendum
([7f3730f](https://github.com/maggnus/ymp/commit/7f3730f65171fe2290194f814bddeac0d7f9f3a8)). It
answers the twenty-four items of brief §16, satisfies or escalates the sixteen criteria of §17, and
walks the final test of §18 through the surfaces it designs.

Read in this order:

1. [`COLLECTIVE-GAP-ANALYSIS.md`](COLLECTIVE-GAP-ANALYSIS.md) — what conforms, what contradicts,
   what is merely unbuilt. Findings are cited below as `G-nn`.
2. This document — the minimal coherent design, items 1–24.
3. [`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md) — item 3 in full: every surface as what the operator
   sees, what actions exist, and what each action does.
4. [`COLLECTIVE-MIGRATION.md`](COLLECTIVE-MIGRATION.md) — item 23, node by node.
5. [`COLLECTIVE-OWNER-DECISIONS.md`](COLLECTIVE-OWNER-DECISIONS.md) — item 24.

Source links are pinned to the baseline revision
[dfdac03](https://github.com/maggnus/ymp/commit/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf).

## What this design changes, in one page

The kernel, the verification plane, the accounting and the isolation boundary are kept as built.
The product boundary moves. Today it runs between the operator and the contract, so the operator
supplies the acceptance condition, the verifier program and the negative control, and nothing at
all starts a participant. After this design it runs between the operator's *goal* and everything
that makes a goal checkable.

    before   operator ── contract, oracle, verifier, engine ──► kernel ──► (nothing starts)
    after    operator ── goal ──►│ derivation · pool · bootstrap · collective │──► verified result

Four moves do the whole job. Each is the smallest change that closes a family of findings without
weakening a mechanism.

**Move 1 — derivation owns the contract.** The internal contract, its observable requirements and
its acceptance plan are derived from the goal, the repository and the run policy, inside the
product. The mechanisms — public requirements, protected cases, negative controls, digest pinning,
query budgets — survive unchanged; only their author changes. Derivation is itself funded and
disclosed, which item 16 states rather than hides. Closes G-01, G-02, G-14, G-20, G-21.

**Move 2 — authorization is a spend decision, not an authoring step.** One confirmation stands
between a goal and the **run's** budget. It shows the restated goal, what *done* will be judged by,
the boundaries, the disclosure and what derivation already cost. It never asks the operator to
write, choose or fix a check. The requirement-to-evidence coverage map survives in full as
diagnostics. Closes G-07 and preserves constitutional constraint 1 of
[`CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/CONCEPT.md#L163-L171).

**Move 3 — the system bootstraps the collective.** On authorization a mechanical run launcher
registers the origin participant, funds it from the root account and starts its first invocation.
It follows committed facts and takes no semantic decision. Every further participant exists because
a participant asked for one. Closes G-03, G-13.

**Move 4 — provider, catalog, pool, participants.** Providers are connected at the provider level;
the models each engine can reach are measured into a catalog; a run's policy declares the permitted
pool, frozen into the journal as a fact at authorization; a participant names a catalog entry when
it recruits and the kernel checks containment over that recorded fact. One hundred catalog entries
create no participants at all. Closes G-04, G-05, G-06, G-12, G-19.

Nothing else in the architecture moves. The board (`W1-COR-03c`), candidate ancestry
(`W1-COR-03d`), the commitment surfaces (`W1-COR-03e`), durable commitment facts (`W1-COR-03f`),
the engine registry (`W1-APP-02e.6`), the product root (`W1-APP-02w.1`) and the generated check
(`W1-APP-02z.3`) are already-accepted or already-opened work this design consumes rather than
replaces. `W1-COR-03f` is load-bearing rather than incidental: the kernel decides recruitment from
committed facts, so those facts must be durable rather than held in memory (item 9).

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
verification, evidence, provider, model, workspace, budget**. The words *obligation, lease, fencing
token, escrow, oracle bundle, capability namespace, runtime driver, invocation, attempt sandbox*
are internal; they appear only under advanced diagnostics, and always with the plain-language fact
beside them.

*Workspace* names the project the goal is about (brief §6, §13). The private writable copy of one
attempt is renamed **attempt sandbox** (G-08). This is a rename of an internal word; the isolation
rule it names is unchanged.

## 2. Complete operator lifecycle

    ymp
      └─ startup: product root, catalog, provider readiness, workspace           [S01 S02 S03 S31]
      └─ goal, in ordinary prose                                                 [S08]
           └─ reading: repository facts, locally, free, nothing disclosed        [S08]
           └─ derivation: requirements, acceptance plan, policy                  (internal)
                — spends the derivation allowance and discloses to one provider  [S06 S08]
                └─ clarification, only for material ambiguity                    [S09]
           └─ authorization: one confirmation, the run's spend and disclosure    [S10]
                └─ bootstrap: origin participant registered, funded, started     [S11]
                     └─ collective: analysis, recruitment, delegation,
                        disagreement, implementation, review                     [S12 S13 S14 S15 S16]
                          └─ candidate                                           [S17 S18]
                               └─ verification                                   [S19]
                                    ├─ failed → the collective reacts while
                                    │           budget and policy allow          [S19 S20]
                                    └─ passed → result                           [S25]
      └─ inspect result and evidence                                             [S25 S26]
      └─ export, explicitly                                                      [S27]
      └─ continue, archive, history                                              [S28 S29]

The operator's decisions are exactly four: **what the goal is**, **whether to spend**, **an answer
when ymp genuinely cannot determine intent**, and **whether to deliver**. Pause, cancel and
intervention are authority the operator holds but does not have to use. Everything else is
observation.

The lifecycle is interrupted in exactly two places: authorization (once, before the run's budget is
committed) and a clarification question (only when a material assumption cannot be resolved
safely). Both interrupts are bounded and both are recorded.

One thing happens before the first interrupt and is stated rather than implied: **derivation
spends**. Turning prose into observable requirements needs a model, so the stage between the goal
and the authorization screen is not free and is not silent. It draws on a separate, small
**derivation allowance** with its own disclosure class, set once when the first provider is
connected and visible thereafter as a policy row (surface S06). Item 16 states the mechanism, the
split between the free local half and the funded half, and why the alternative — deriving without a
model — would return authorship to the operator.

## 3. Complete TUI information architecture

The full architecture — thirty-four surfaces, each stated as *what the operator sees*, *what
actions exist*, *what each action does* — is [`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md). Brief §6
requires at least thirty and enumerates twenty-seven items; those expand into thirty surfaces, and
four more are added because the design needs them.

The composition rules that govern all of them:

1. The main surface is one scrolling conversation with an always-ready input line, a thin context
   header and a status line — the shape already accepted in
   [`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L46-L64).
   Data lives on full-screen pages opened with `/` and closed with `Esc`.
2. Every surface leads with product language. An internal term appears only inside an advanced
   view, and never alone: `obligation ob-7 · the work participant B took on and has not returned`.
3. Nothing a participant says is executable in one keypress. Acting on a suggestion is a separate
   command issued under the operator's own authority (INV-4 as layout).
4. No ranking, scoring or grading of participants, bids or candidates anywhere. Sorting uses raw
   mechanical fields only (INV-1).
5. The control, collaboration and verification planes stay visually distinct wherever they meet.
6. Every view holds its own cursor and recovers from the journal; lag is a visible state, never a
   silent divergence (INV-6).
7. A terminal state is named exactly. `exhausted` is never drawn as success (INV-7).
8. A budget dimension is never traded against another in the interface any more than in the kernel
   (INV-2).

## 4. Startup and first run

**Startup** does four mechanical things and reports them in one line each: opens the product root
(`~/.ymp`, node `W1-APP-02w.1`), reads the catalog, probes provider readiness in the background,
and identifies the workspace from the launch directory. None of them blocks the input line: the
operator may state a goal while probes are still running, and authorization waits for readiness
rather than the prompt doing so.

**First run** — the state where no provider is ready — is the only moment the product asks the
operator to configure anything, and it asks for exactly one thing: connect a provider. It states
what ymp is in two sentences, lists the providers it can detect with their state, and offers
`/providers`. It never asks for a contract, a verifier, a model or a team, and it does not present
an empty catalog as a failure.

The startup line states the assurance profile with its limit in the same breath, never the name
alone: `poc_process_isolation · attempts run as separate processes with your own permissions; this
is not hostile-code containment`. That rule is already accepted in
[`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L228-L246).

## 5. Provider configuration flow

Three levels, exactly as brief §4 states them, and the product owns only the first two.

**Provider level.** A provider is an account with an authentication state: Anthropic, OpenAI,
NVIDIA, a local endpoint. `/providers` shows each as `ready`, `not configured`,
`needs authentication` or `unavailable`, with the measured reason.

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

ymp does not become a credential store. What it adds over today (G-19) is that the operator learns
what is ready, what is missing and what to do about it inside the product instead of guessing from
a refusal at run time.

**Disclosure is stated at this level, not buried.** Enabling a provider permits repository content
to be sent to it; host allowlisting is not a confidentiality control
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L562-L568)).
The provider row therefore carries its disclosure class, and the authorization surface repeats the
list of providers a run may send content to.

## 6. Model catalog and availability

The catalog is the registry of node `W1-APP-02e.6`, read as the level the brief calls the model
catalog. One record per engine under the product root, holding: enabled flag, measured properties
(executable, version, credential origin, budget bounds, capability matrices) and **the list of
models that engine can serve**, filled by probe.

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
  unit for it (criterion 17.3).
- **A catalog entry has no role.** There is no architect model, no coder model, no reviewer model.
  Convenience groupings such as *reasoning*, *fast* or *cheap* may exist as resource labels a
  participant reads, and are never a semantic assignment (brief §15).
- **The catalog is measured, not declared.** The model list comes from probing the engine. An entry
  whose engine is disabled or unready is not admissible and is shown as such.

The version pinning that exists today survives as a property of an entry
([`runtime-claude/lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-claude/src/lib.rs#L24)):
it stops being the only reachable route and becomes the recorded identity of one route among
several (G-04).

## 7. Project and task policy

A **run policy** is the boundary object. It is derived with sensible defaults and the operator may
narrow it; it is never a prerequisite. It declares:

| Field | Default | Enforced by |
|---|---|---|
| permitted pool | every enabled, ready, conformant catalog entry, in registry order | kernel: set membership at recruitment |
| spend ceiling | a stated default per run | `MoneyMicros` |
| wall clock | a stated default per run | `WallTimeMs` |
| participants ceiling | a stated default | `ParticipantStarts` |
| concurrent attempts | a stated default | admission and `AttemptStarts` |
| verification queries | a stated default | `VerificationQueries` |
| clarification questions | a stated default | `ClarificationRequests` (new dimension) |
| disclosure classes | the classes of the enabled providers | kernel: entry class ⊆ policy classes |
| assurance profile | the host's best available, named with its limit | preflight refusal, never silent weakening |
| external actions | none | `ExternalActions`, zero by default |

The policy is a **boundary, not a route**. It says what may be used and how much may be spent. It
never says which model suits which task; that sentence has no representation anywhere in the
kernel, and criterion 17.7 depends on it staying that way.

The pool is frozen into the internal contract at authorization and recorded by digest, so a
matched-budget comparison (`W1-EVL-04a`) is reproducible and per-model spend stays attributable
(`W1-APP-02y`). Where the permitted set is finally declared is the owner's decision, deferred as
node `W1-EVL-04d`; the recommendation and its alternatives are
[owner decision 1](COLLECTIVE-OWNER-DECISIONS.md#d1--where-the-permitted-model-set-is-declared).

## 8. Collective bootstrap

Bootstrap is the piece with no implementation today (G-03), and it is deliberately the dullest
component in the design.

**The run launcher** is a composition-level component that follows committed facts and starts
processes. It has no opinion. On authorization it performs exactly this sequence, each step a
kernel command that can be refused:

1. Create the run and bind it to the derived internal contract — the path that exists today
   ([`contract.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-application/src/contract.rs#L294-L318)).
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

Two properties of step 3 deserve to be stated rather than assumed:

- **Order, not preference.** "First ready entry in declared order" is a mechanical rule over a list
  the operator's configuration produced, of the same kind as the per-principal round robin that
  already governs admission
  ([`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROTOCOL.md#L455-L460)).
  It reads no property of the goal. A rule that read the goal would be the semantic router the
  brief forbids.
- **One, not many.** Bootstrap starts exactly one participant. The collective's size is an outcome
  of its own recruitment decisions, bounded by the vector (criterion 17.4, 17.5).

Who declares the order, and whether the operator may set a default entry, is
[owner decision 2](COLLECTIVE-OWNER-DECISIONS.md#d2--the-entry-rule-for-the-origin-participant).

## 9. Dynamic recruitment

Recruitment already has its primitives in the ledger. What it lacks is a subject for the checks
brief §3 lists (G-12) and a projection to the participant (G-10).

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
committed at authorization — each entry with its identity, disclosure class, assurance profile and
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

Failed verification is **not** a terminal while budget and policy allow: the collective keeps the
work obligation open and reacts, which is criterion 17.13. The interface says so explicitly, so a
red verdict is not read as the end.

Two honesty rules from the accepted composition survive verbatim: a run the operator ends is
recorded as `cancelled` and never as `infrastructure_error`, and the cancel confirmation states
beforehand what is interrupted and that consumed budget does not return
([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L248-L260)).

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
the previous goal as context. It is a new run lineage with its own authorization, because it spends
new money against a new definition of done.

The evidence export contains the four digests, the requirement-to-observation map, the per-model
spend, the run journal head digest and the participant transcripts as observational records. It
contains no protected oracle material and no capability material.

## 16. Internal ownership of contracts, oracles and verifiers

This is the item the whole brief turns on, so it is stated as a pipeline with an owner per stage.

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

### What derivation costs, and why it is not free

The stage marked *funded* needs a model: turning "implement feature X so that it works properly"
into observable requirements is a reading of intent, and no amount of local analysis performs it.
Three honest options existed and two were rejected.

- *Derive locally, without a model.* Requirements would come from pattern matching over the
  repository, which brief §16 forbids — "do not invent details to fill gaps" — or the operator
  would have to supply them, which is the authorship the brief removes. Rejected.
- *Derive after authorization.* The authorization surface could then show only the goal and a
  ceiling, never what *done* will mean, which removes the one thing that makes the spend decision
  informed. Rejected.
- *Fund derivation separately, and say so.* Chosen. It is also the accepted position rather than a
  new concession: drafting is already specified as a run under a built-in package approved once,
  with its own budget, audience and capabilities
  ([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L126-L141)).

Derivation therefore has two halves with different costs:

| Half | Does | Costs | Discloses |
|---|---|---|---|
| **Reading** | walks the workspace: files, languages, build entry points, test entry points, version-control state, documentation paths | nothing; it is local file reading | nothing leaves the host |
| **Deriving** | goal → observable requirements → acceptance plan → verification strategy | the **derivation allowance**: a small ceiling in money, tokens and wall clock, separate from the run's budget | repository excerpts and the goal reach exactly one provider — the one the allowance names |

The allowance is a policy, set once when the first provider is connected (surface S02), shown on
the policy surface (S06) and enforced by the same dimensions as any budget. It is charged per goal,
not per keystroke: amending a goal re-derives and draws on the same allowance, and the remaining
allowance is visible while it does. Exhausting it stops derivation with what it has and says so on
the authorization surface, rather than silently producing a thinner plan.

The authorization surface reports what derivation already spent and to whom it disclosed, because
by the time the operator reads it the disclosure has happened. Whether that pre-authorization
disclosure needs its own consent, and how large the allowance should be, are
[owner decision 4](COLLECTIVE-OWNER-DECISIONS.md#d4--the-disclosure-default) and
[owner decision 8](COLLECTIVE-OWNER-DECISIONS.md#d8--default-ceilings).

One consequence is stated plainly because it is the price of the choice: a goal typed in a
workspace the operator does not want disclosed will have disclosed a bounded part of it before any
run is authorized. The mitigations are real but partial — the disclosure class is per workspace
(decision 4), the allowance is small, the excerpts are bounded, and the operator sees the provider
named on S08 while derivation runs and on S10 afterwards.

**Provenance classes** travel with every requirement and every check, from derivation to verdict:

| Class | Source | Where the operator sees it |
|---|---|---|
| **A** | the operator's goal text | authorization: the restated goal; result: the checklist mark |
| **B** | the repository — tests, build entry points, documentation, configuration | authorization summary; diagnostics show the file each fact came from |
| **C** | an assumption inferred rather than established — by derivation before the run, or by a participant during it | listed at authorization if it existed then; published on the activity surface if it arose later; marked in the result checklist either way |
| **D** | a verification mechanism ymp generated | the result checklist column "observed by"; diagnostics show the check and its digest |
| **E** | an explicit operator clarification | the question and its answer in the transcript; marked in the result checklist |

The rule that separates class C from class E is the brief's own (§1, §8): an assumption that
materially affects the result and cannot be safely resolved from the request, the repository, the
documentation, the visible checks, the policy or what the collective discovers becomes a question.
Anything else stays a recorded assumption and is shown, never hidden. The number of questions one
run may ask is a budget dimension, so "ask more" is not a way to avoid deciding.

Assumptions arise at two moments, and the two are treated differently because the contract is
immutable once bound.

**Before the run**, derivation raises them. A material question blocks authorization until it is
answered or the operator explicitly proceeds under the stated assumption. Its answer becomes a
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

**Derivation classifies, mechanically and conservatively.** It is the component that already holds
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
authorization, which is why the reading half runs first and why the clarification budget is spent
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
describing the derivation as a step the operator takes (G-21). It is invisible unless diagnostics
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

## 18. Model pool and recruitment design

    providers ──► model catalog ──► permitted pool ──► recruited participants
    (connected)   (measured)        (declared)         (created one at a time, each paid for)

- **Availability** is measured: an entry is admissible only if its engine is enabled and ready and
  its pairing passed its conformance probes.
- **Permission** is declared: the pool is the policy's subset, frozen at authorization and recorded
  by digest.
- **Instantiation** is requested: only `request_participant` creates a participant, only a
  participant issues it, and it costs a `ParticipantStarts` unit from the requester's own account.

The kernel's whole involvement is containment over committed facts — pool membership, availability,
disclosure class, assurance profile — plus the resource and capability checks it already performs.
Whether an entry is live is measured outside it and arrives as a fact (item 9). It holds no preference order over the pool beyond the declared one, no notion that one
entry suits one task, and no fallback that silently substitutes another entry. A refusal names the
mechanical reason and creates nothing.

Convenience pools such as *reasoning*, *fast* or *cheap* are optional labels on catalog entries
that a participant may read when deciding what to ask for. They are resource policies. They are
never inputs to a kernel decision, and no code path may turn a label into a selection.

## 19. Exact boundaries

**Operator may**: state a goal; connect and enable providers; narrow the policy; answer a question;
observe every surface; pause, resume, cancel; send a message (marking intervention); inspect result
and evidence; export; continue; archive.
**Operator must never be required to**: author a contract, acceptance criteria, an oracle, a
verifier or a negative control; create a task; create or name an agent; assign a model; write a
decomposition; assemble a team; approve a plan item by item.

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

| # | Change | Closes | Note |
|---|---|---|---|
| D1 | `ContractDocument` gains `requirements: Vec<Requirement { id, statement, provenance, observed_by }>` | G-14 | provenance is `A|B|C|D|E` |
| D2 | `ContractDocument` gains `acceptance_plan: Vec<Check { id, program_digest, kind, protected, observes }>`; the single `verifier` becomes one check of that plan | G-01, G-14 | validation of each check is unchanged |
| D3 | `ContractDocument` gains `policy: RunPolicy { pool_digest, ceilings, disclosure_classes, assurance_profile }` | G-05 | pool frozen by digest for reproducibility |
| D4 | New stored object: catalog record per engine (enabled, measured properties, model list) | G-05 | node `W1-APP-02e.6` |
| D5 | New typed object: `Clarification { id, question, options, answer, asked_at }`, and budget dimension `ClarificationRequests` | G-15 | answers become class E |
| D6 | `RegisterParticipant` gains `requested_entry: CatalogEntryRef` | G-12 | the subject the brief's §3 checks need |
| D7 | New kernel refusal `EntryNotPermitted { participant_id, entry, reason }` | G-12 | containment only; creates nothing. `EntryNotReady` is **not** a kernel refusal: liveness is measured in admission (item 9) |
| D7a | New facts `PoolFrozen { entries, digest }`, `EntryUnavailable { entry, reason }`, `EntryAvailable { entry }` | G-12 | committed by admission and the catalog watcher, so the kernel decides without I/O and replay reproduces the decision |
| D7b | New fact `RequirementsDiverged { answer, requirements }` and its operator resolution | item 16 | holds the acceptance path until the operator resolves it |
| D8 | `RunState.budget` becomes a projection of the root participant's `BudgetVector`; the two-dimension `Budget` retires | G-09 | one accounting; enforcement unchanged |
| D9 | The single-active-attempt refusal becomes a policy ceiling checked from the vector and the concurrency limit | G-13 | the isolation rule it protected is unchanged |
| D10 | A project holds many runs addressed by sequence; a session opens, creates and lists them | G-16 | layout accepted in `W1-APP-02w` |
| D11 | Provider record with authentication state and disclosure class | G-19 | ymp records where a credential is read from, never the credential |
| D12 | Participant tool projection extended to the accepted families: observe, communicate, contract, lifecycle, artifact, verification | G-10 | schemas already in [`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROTOCOL.md#L280-L292) |

Two more state changes follow from item 16: a **derivation allowance** as a policy field with its
own disclosure class and its own ceilings, and the divergence fact D7b with its operator
resolution.

No change removes a precondition, a reservation, a digest or a refusal. D6, D7 and D7a are the only
kernel-facing additions; D7 is a containment refusal and D7a is the fact stream that lets it decide
without input or output. `ymp-domain` performs no I/O before or after this design.

## 21. Required Rust module and crate changes

| Crate | Change |
|---|---|
| `ymp-domain` | D1–D3, D5–D9: contract document, provenance, clarification object, catalog entry reference, the containment refusal, the pool-snapshot, availability and divergence facts, budget projection. No file, network or process access is added: every new decision reads committed facts |
| `ymp-application` | derivation module in two halves — local reading, then the funded derivation with its allowance; `prepare_contract` stops requiring an operator acceptance condition and starts consuming the derivation; the divergence classifier of item 16; clarification service; run listing under a project |
| **`ymp-catalog`** *(new)* | catalog records under the product root: read, probe, enable/disable, model lists, engine–route pairings and their conformance results; no process starting |
| **`ymp-conduct`** *(new, composition)* | the run launcher and the catalog watcher: registers the origin participant, freezes the pool, commits availability facts, starts and resumes invocations by following committed facts. Depends on `ymp-application` and `ymp-runtime-supervisor`; nothing depends on it but `ymp-cli` |
| `ymp-runtime-api` | catalog entry as the profile's model source; the pinned model becomes a property of an entry rather than a constant the profile refuses to deviate from; a **route override** on an engine profile — endpoint class, wire protocol, account scope, authentication mode, disclosure class — so a provider is reached through an installed engine without a new engine crate |
| `ymp-runtime-supervisor` | the one-attempt constant becomes a policy ceiling; invocation start accepts the entry the ledger recorded; admission measures entry liveness and refuses before the recruitment command is issued |
| `ymp-agent-api`, `ymp-agent-mcp` | D12: the extended tool projection, one tool per accepted domain command, gated by the invocation's contract |
| `ymp-tui` | new surfaces (providers, models, pool, agents, tasks, activity, result, evidence, export, history, archive, questions); authorization restated as a spend decision; drops `ymp-runtime-fake`, `ymp-runtime-claude`, `ymp-runtime-codex` and reads the catalog through the application port |
| `ymp-cli` | injects the launcher into the session; mirrors every new interface action as a command; the internal engine argument stops being the only way to start a run |
| `ymp-verifier` | unchanged in mechanism; executes plan checks rather than one program |

The dependency direction is deliberate: `ymp-runtime-supervisor` already depends on
`ymp-application`, so the launcher cannot live in either. Putting it in a composition crate keeps
the interface free of runtime dependencies, which is also what closes G-06 and node `W1-APP-02s`.

## 22. Required tests and acceptance scenarios

Each scenario states its negative half — the condition under which the check fails — because a
check that cannot fail proves nothing.

**Ownership**

1. A goal in a project with no tests reaches an authorization surface with a derived plan. *Negative
   half:* today's refusal demanding a hand-written verifier.
2. No path from goal to spend exists that asks the operator for a verifier, a negative control, a
   task, an agent or a model. *Negative half:* a build in which any such prompt is reachable.
3. The internal contract is complete without operator input: every field of the accepted package
   has a value and a provenance. *Negative half:* a field left `None` by derivation.

**Bootstrap and recruitment**

4. Authorization starts exactly one participant. *Negative half:* zero (today) or more than one.
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
    authorization until it is answered or the operator explicitly proceeds under the stated
    assumption. *Negative half:* a silently invented requirement.
14. The clarification budget bounds the number of questions; exceeding it stops the run honestly
    rather than asking again. *Negative half:* unbounded questioning.
15. A derived plan that accepts the negative control never reaches authorization, and a plan that
    accepts a substituted entry point never reaches authorization (nodes `W1-APP-02u`,
    `W1-APP-02z`). *Negative half:* the measured false accept those nodes recorded.

**Verification and terminals**

16. A failed verification with budget remaining leaves the run running, the obligation open and the
    candidate immutable. *Negative half:* automatic termination.
17. Each of the five terminals is reachable and is reported with its own text; `exhausted` is never
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
26. The authorization surface states what derivation spent and which provider it disclosed to, and
    the figures match the recorded charges. *Negative half:* a surface claiming nothing was spent.

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

**End to end**

32. The scenario of brief §18, driven on the built product, with no operator-authored machinery.
    *Negative half:* any step that requires the operator to supply a contract, oracle, verifier,
    team, role, model assignment or decomposition.

## 23. Migration path

By node, in [`COLLECTIVE-MIGRATION.md`](COLLECTIVE-MIGRATION.md): what survives as built, what
changes owner, what closes, and what the redesign proposes as new work. Summary:

- **Survives unchanged:** the commitment kernel and its accounting (`W1-COR-03a`, `03b`,
  `03g`–`03s`), verification exactness and pinning (`W1-APP-02b`, `02g`, `02u`, `02z`,
  `W1-EXP-01a`), launch admission (`W1-APP-02p`, `02q`, `02r`, `02k`), journal and recovery
  (`W1-APP-02a`, `02f`), per-model spend (`W1-APP-02l`, `02y`), command parity (`W1-APP-02n`),
  chat-first composition (`W1-APP-02e.2`, `02e.3`), the product root (`W1-APP-02w`,
  `W1-APP-02w.1`), the engine registry as the catalog level (`W1-APP-02e.6`).
- **Changes owner:** the contract dialogue (`W1-APP-02v`) and the verifier answer (`W1-APP-02u`)
  become internal derivation; the generated verifier (`W1-APP-02z.3`) becomes the acceptance plan;
  the pinned profile probe (`W1-APP-02e.4`) becomes catalog readiness; the model-use policy
  (`W1-EVL-04d`) becomes the permitted pool and remains an owner decision.
- **Closes:** the operator-facing acceptance-condition requirement; the per-run engine argument;
  the single-run store; the two-dimension run budget; the fake runtime as an offered profile.

## 24. Unresolved decisions

Ten decisions that genuinely need the owner, each with options and consequences, are in
[`COLLECTIVE-OWNER-DECISIONS.md`](COLLECTIVE-OWNER-DECISIONS.md). Their headings:

1. Where the permitted model set is declared (deferred node `W1-EVL-04d`).
2. The entry rule for the origin participant.
3. Whether the single authorization stays mandatory per run.
4. The disclosure default across enabled providers.
5. The semantic remainder: may a run be `accepted` when part of the goal is not mechanically
   checkable?
6. The clarification budget and what happens when it runs out.
7. Who sets the diagnostic-disclosure policy when the contract is derived.
8. Concurrency and participant ceilings by default.
9. Naming: *workspace* for the project scope and *attempt sandbox* for the private copy.
10. Whether `Continue` re-authorizes.

---

## Acceptance criteria of brief §17

| # | Criterion | Where satisfied |
|---|---|---|
| 17.1 | Launch without authoring a contract or verifier | Items 4, 16; D1–D3; test 1, 2 |
| 17.2 | Providers configurable and visible | Item 5; surfaces S03, S04; D11 |
| 17.3 | 100 models never become 100 agents | Items 6, 18; test 5 |
| 17.4 | No manual team construction | Items 8, 19; test 4 |
| 17.5 | The collective recruits from the permitted pool | Items 9, 18; test 6 |
| 17.6 | Recruitment bounded by kernel resources and authority | Item 9; D6, D7; tests 7, 8 |
| 17.7 | The collective, not the kernel, makes semantic decisions | Items 18, 19; tests 9, 10, 11 |
| 17.8 | Internal contracts and verification stay strict, not prerequisites | Item 16; test 15 |
| 17.9 | Questions only on genuine intent ambiguity | Item 16; D5; tests 13, 14 |
| 17.10 | The TUI exposes the full lifecycle | Item 2; [`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md) |
| 17.11 | Activity understandable without chain-of-thought | Items 10, 11 |
| 17.12 | Results carry verification evidence | Items 13, 15; test 19 |
| 17.13 | Failed verification does not auto-terminate | Item 14; test 16 |
| 17.14 | Four terminals stay distinct | Item 14; test 17 |
| 17.15 | Kernel trust boundaries intact | Item 19; gap analysis §5; tests 9–11, 15, 18 |
| 17.16 | "Give the collective a goal, get a verified result" | Items 1, 2; test 32; the §18 walk below |

Three criteria carry an owner decision that this design names rather than settles: 17.1 depends on
[decision 3](COLLECTIVE-OWNER-DECISIONS.md#d3--whether-the-single-authorization-stays-mandatory)
(whether the one authorization remains); 17.5 and 17.6 depend on
[decision 1](COLLECTIVE-OWNER-DECISIONS.md#d1--where-the-permitted-model-set-is-declared) (where
the pool is declared); 17.12 depends on
[decision 5](COLLECTIVE-OWNER-DECISIONS.md#d5--the-semantic-remainder) (whether an unobservable
remainder blocks acceptance).

## The final design test of brief §18

Driven through the designed surfaces. Each step names the layer that performs it. The operator's
acts are marked ▶: six before the result and one optional act after it, and none of them is
authorship. One of the six is the one-time workspace setup that fixes the disclosure class and the
derivation allowance, which the design states rather than hides.

| # | Step | Layer | Surface |
|---|---|---|---|
| 1 | ▶ `$ ymp` | — | S01 startup: root opened, catalog read, providers probing, workspace identified |
| 2 | ▶ connect Anthropic, OpenAI, NVIDIA | TUI → provider records | S03/S04. Two engines are installed. Anthropic is reached by Claude Code and OpenAI by Codex, each authenticating through its own engine; NVIDIA is reached as a model route through Claude Code's Anthropic-compatible endpoint, and its pairing is probed for conformance in its own right. Each provider reaches `ready` with its disclosure class stated |
| 3 | ▶ set the disclosure class and the derivation allowance for this workspace | operator | S02/S06: one setup decision, taken once; it states that repository excerpts will reach one named provider before any run is authorized |
| 4 | catalog fills from probes | catalog | S05: 3 providers · 2 engines · 11 entries, the NVIDIA ones marked with their pairing; the pool defaults to every enabled, ready, conformant entry |
| 5 | ▶ "Implement feature X in this repository." | TUI → application | S08 goal entry; the run's budget is untouched |
| 6 | the workspace is read locally: files, languages, build and test entry points, version-control state | derivation, reading half | S08 states the facts it found; nothing left the host |
| 7 | observable requirements derived (A, B); acceptance plan generated (D) and validated against the negative control and the substituted entry points | derivation, funded half | S08 states the provider it is disclosing to and the allowance it is drawing on; the plan itself is visible in diagnostics |
| 8 | one material assumption cannot be resolved | derivation | S09: a question with concrete options — the brief's "Authorization Code or Client Credentials?" shape |
| 9 | ▶ answer | operator | recorded as class E; the run is **not** marked intervened |
| 10 | ▶ authorize the spend | operator | S10: restated goal, what *done* means, ceiling, pool, disclosure, assurance with its limit; one confirmation |
| 11 | origin participant registered on the first ready pool entry, endowed from the root vector, root obligation created, one invocation started | run launcher | S11 collective startup, then S12 live view |
| 12 | participant A analyses the repository in its own attempt sandbox | collective | S12 activity; S13 shows one participant |
| 13 | A determines more expertise is useful and issues `request_participant` naming a catalog entry, funding a proposal allowance from its own account | collective | S16 activity records the request |
| 14 | admission finds the entry live and forwards; the kernel checks containment against the frozen pool, the availability facts, the disclosure class, the assurance profile, `ParticipantStarts` and concurrency — and admits | admission + kernel | S13 shows the admission and the mechanical reason it was permitted |
| 15 | a suitable model becomes participant B; its process starts | run launcher | S13 now shows two participants with their catalog entries |
| 16 | A advertises a scoped offer; B bids; A awards; a task contract, obligation and lease form atomically | collective + kernel | S15 tasks |
| 17 | they investigate, publish findings, disagree, challenge, and resolve on the board | collective | S16 activity, attributed and labelled untrusted |
| 18 | one implements in its sandbox and submits; the integrator builds an immutable candidate | collective + kernel | S17 candidates |
| 19 | the other reviews as a separate blinded assessment | collective | S16, S18 |
| 20 | a participant spends a verification query on the exact candidate digest | collective + verification | S19: `verifying cd-1 · query 1 of 4` |
| 21 | verification fails | verification | S19 shows the verdict, the failure class and the bounded diagnostic the policy allows; the run stays alive, the obligation stays open |
| 22 | the collective sees the same bounded diagnostic and revises | collective | S12, S16, S20 |
| 23 | a new candidate is submitted and verified; it passes | collective + verification | S19, then S25 |
| 24 | ▶ the operator reads *completed · verified* and inspects, or exports | operator | S25 result, S26 evidence, S27 export |

Operator acts: start the program, connect providers, set the workspace's disclosure class and
derivation allowance once, state the goal, answer one question, authorize the spend — plus the
optional inspection and export at the end. No contract, oracle, verifier, team, role, model
assignment or decomposition was created by the operator, which is the test brief §18 sets.

Two things this walk states that an earlier revision of it did not. Step 3 is a real decision and
appears as one: repository excerpts reach a provider between the goal and the authorization, and
the operator learns that before it happens rather than after. And step 2 is honest about the host:
three providers are configured on two installed engines, because NVIDIA enters as a model route
through an existing engine rather than through a native runtime this design does not build.

## Required updates to the documents of record

Recorded here rather than applied, per the scope of this card. Each is a wording change the design
implies; none changes a mechanism.

| Document | Change |
|---|---|
| [`PROJECT-CONTRACT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROJECT-CONTRACT.md#L53-L71) | "Draft, approval, and amendment": the draft is produced by internal derivation, not by an operator interview; the human approves a spend against a restated goal. The eleven package contents and all validation stay. |
| [`PROJECT-CONTRACT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROJECT-CONTRACT.md#L14-L46) | Package contents gain requirement provenance (A–E) as a recorded field. |
| [`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L84-L88) | Provider authentication becomes a product surface that routes to the engine's own authentication, rather than a prerequisite arranged elsewhere. |
| [`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L502-L508) | The per-attempt private copy is renamed *attempt sandbox*; *workspace* becomes the operator-facing name of the project. |
| [`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L244-L259) | Add the provider and catalog levels above the runtime profile, and the run launcher as a composition component with no semantic authority. |
| [`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROTOCOL.md#L244-L278) | `request_participant` names a catalog entry; two mechanical refusals are added; a clarification command and its budget dimension are added. |
| [`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L88-L99) | Contract authorization becomes run authorization: a spend decision. The coverage map is retained as diagnostics. |
| [`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L112-L117) | The naming constraint on *workspace* is superseded by brief §13. |
| [`CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/CONCEPT.md#L8-L13) | The product goal is restated as goal in, verified result out; the approved contract becomes an internal object. |
| [`INVARIANTS.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/INVARIANTS.md#L7-L14) | INV-1 gains the explicit sentence that catalog-entry membership is a set operation and that the kernel holds no preference order over the pool. |

## Self-check against brief §9 and §14

### The eight ownership leaks of §14

| Leak named by the brief | Where it is closed | Finding |
|---|---|---|
| `PROJECT-CONTRACT.md` as a mandatory input | Item 16: the package survives as an internal object | G-20 |
| Verifier creation as user responsibility | Item 16: the plan is derived and demonstrated | G-01, G-02 |
| Manual agent creation | Item 8: the system starts participant one; the collective recruits the rest | G-03 |
| Manual model assignment | Items 6, 18: a participant names a catalog entry; nothing assigns one | G-04 |
| Manual decomposition | Item 9: the collective sponsors, bids, awards and delegates | — |
| Manual team assembly | Items 8, 9: the collective's size is an outcome of its own recruitment | G-03 |
| Provider configuration conflated with participant creation | Item 6: a catalog entry is not an agent and instantiates nothing | G-05 |
| Runtime profiles exposed as user-level roles | Item 17: the runtime driver is internal; the operator sees providers and models | G-04, G-06 |
| Acceptance mechanics as mandatory workflow steps | Item 2 and surface S10: one spend decision; the coverage map is diagnostics | G-07 |

### The two failure modes of §9

**Is there a hidden manager?** The only component added between the operator and the collective is
the run launcher. It registers one participant, starts and resumes processes for facts the ledger
already committed, and stops. It reads no goal, holds no plan, selects no participant, assigns no
work and ranks nothing. Derivation produces a contract and then has no further part in the run; it
cannot start, stop, fund or direct a participant. Neither component can be reached by a participant
as an authority.

**Is the entry rule a model assignment in disguise?** This is the sharpest point in the design and
it deserves the direct answer. Something must run first, and the brief closes two of the three
possible sources: the collective cannot choose, because it does not exist yet, and the operator
must not be asked, because that is the model assignment §1 forbids. What remains is a rule, and the
rule this design uses reads nothing about the goal, the task or the entry beyond readiness and
declared position. It is not new authority either: the accepted architecture already states that
configuration names an origin participant and gives it the root contract and budget
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L227-L231)),
and [`CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/CONCEPT.md#L121-L124)
already calls that participant an ignition point rather than a permanent orchestrator. The design
changes only where the name comes from: derived from the declared pool instead of typed by the
operator. Test 9 and test 11 of item 22 exist to kill a build in which the rule starts reading
anything else.

**Did the kernel become a model router?** The kernel gained one refusal and one kind of operation:
containment of a requested catalog entry against facts already committed. It holds the pool as a set plus a declared order, and
the declared order is used in exactly one place — the entry participant at bootstrap — where some
mechanical rule is unavoidable and where the alternative is asking the operator, which the brief
forbids. No kernel path reads any property of the goal, the task or the entry other than
membership, readiness, disclosure class and reserved resources. Test 9 and test 11 exist to kill a
build in which that stops being true.

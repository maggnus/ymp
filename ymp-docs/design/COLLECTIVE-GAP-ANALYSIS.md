# Collective redesign — gap analysis

This is the first half of the deliverable the owner addendum asks for: a walk of the existing
repository against [`PRODUCT-BRIEF-collective.md`](PRODUCT-BRIEF-collective.md), stating what
already conforms, what contradicts it, and which ownership, state, API and interface changes
follow. The second half — the smallest coherent design that closes the gaps — is
[`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md).

The brief is the product-level source of truth. The addendum recorded at
[7f3730f](https://github.com/maggnus/ymp/commit/7f3730f65171fe2290194f814bddeac0d7f9f3a8) adds
three obligations this document is written to: analyse before designing, move ownership rather
than remove mechanisms, and never hand responsibility back to the operator.

Every claim below is pinned to source. File links resolve at the baseline revision
[dfdac03](https://github.com/maggnus/ymp/commit/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf); node
identifiers name accepted or open work units in
[`work/STATUS.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/work/STATUS.md).

## Summary

The kernel is not the problem. Almost everything the brief demands of a kernel — finite budget
vectors, obligation accounting, leases with fencing, candidate immutability, exact-digest
verification, honest terminals, non-semantic admission — is already built and accepted, and the
code is explicit that no transition may read a skill, a model or a rank
([`commitment/protocol.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/commitment/protocol.rs#L1-L6)).

The gap is concentrated in one place: **the product boundary is drawn at the contract**. Everything
on the operator's side of that boundary — the acceptance condition, the verifier program, the
negative control, the choice of engine and model, the decision that a run consists of one attempt —
is work the brief assigns to ymp and to the collective. Everything on the far side of it is either
built or specified. Twenty-one contradictions are recorded below; nineteen of them are consequences
of that single boundary being drawn in the wrong place, and the remaining two are naming and
accounting duplication.

The second consequence is quieter and more serious: **the operator surface cannot start a
participant at all**. `ymp-tui` does not depend on the runtime supervisor, and the authorization
action records the approved contract and stops. A managed run exists only behind
`ymp internal managed-candidate-smoke`. The brief's central sentence — goal in, collective works,
result out — has no implementation between "goal in" and "result out"; what exists is the
machinery on either side of it.

## 1. What already conforms to the brief

### 1.1 The kernel enforces without deciding

| Brief clause | Mechanism in the repository |
|---|---|
| §9 the kernel must not rank bids or judge quality | Commands carry identifiers, digests, integers and deadlines only; an intent reaches the kernel as a digest that is compared and never read ([`protocol.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/commitment/protocol.rs#L1-L6)) |
| §2 the sponsor chooses, not the kernel | `Award` names the consent it awards; the kernel never selects one ([`protocol.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/commitment/protocol.rs#L86-L96)) |
| §5 recruitment consumes finite resources | Ten independently enforced dimensions with no conversion between them ([`budget.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/commitment/budget.rs#L39-L50)) |
| §5 starting an agent to ask it whether it wants work is not free | `InvocationStarts` is charged per process slice, so waking a participant costs a unit of its own ([`budget.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/commitment/budget.rs#L29-L33)) |
| §5 decomposition must terminate | Creation authority is irreversible once spent, so a branch cannot mint an endless chain ([`budget.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/commitment/budget.rs#L54-L64)) |
| §5 the kernel records transitions without interpreting reasons | `Outcome` closes causal work without making the parent successful, and nothing else reads it ([`records.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/commitment/records.rs#L84-L95)) |

Accepted by nodes `W1-COR-03a`, `W1-COR-03b`, `W1-COR-03g`–`W1-COR-03j`, `W1-COR-03k`–`W1-COR-03s`.

### 1.2 Verification is already independent, exact and honest

- A verdict is a kernel fact bound to one exact candidate digest, and a candidate that is not the
  recorded one is refused rather than judged
  ([`protocol.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/commitment/protocol.rs#L590-L598)).
- Acceptance evidence names contract, candidate, environment and oracle digests together
  ([`lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/lib.rs#L84-L92)).
- A program that accepts a deliberately wrong candidate cannot enter a contract; the negative
  control is executed before anything is stored
  ([`answer.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-application/src/answer.rs#L9-L13), node `W1-APP-02u`).
- A verifier that delegates to a file inside the candidate is pinned to the bytes approved with the
  contract, so a candidate cannot decide its own run (node `W1-APP-02z`).
- Five terminal states exist and quiescence is never acceptance
  ([`lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/lib.rs#L31-L46)).

The brief's protected-verification principles (§8) therefore need no weakening and no
reinvention. They need a different author.

### 1.3 Runtime, model and process admission

- Every program in the launch chain is admitted by digest or by a location the run's own account
  cannot write, and its role is recorded beside it
  ([`runtime-api/lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-api/src/lib.rs#L52-L131), nodes `W1-APP-02p`, `W1-APP-02q`, `W1-APP-02r`).
- Ambient user, project and plugin configuration is excluded from a managed invocation, and
  runtime-native subagents are disabled rather than mapped
  ([`runtime-claude/lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-claude/src/lib.rs#L88-L98)).
- Spend is attributed per model rather than per run only
  ([`runtime-api/lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-api/src/lib.rs#L868-L871), nodes `W1-APP-02l`, `W1-APP-02y`).
- No managed descendant survives the supervisor that started it (node `W1-APP-02k`).

This is exactly the mechanical layer the brief's §3 recruitment check needs. What it lacks is the
set it should check membership against.

### 1.4 Interface composition and command parity

- The main surface is a scrolling conversation with a `/` command line and full-screen data pages,
  which is the feel the brief asks for in §6
  ([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L46-L64), nodes `W1-APP-02e.2`, `W1-APP-02e.3`, `W1-APP-02v`).
- Every action the interface offers exists as a command of the same executable, and a command
  cannot skip a confirmation the interface enforces
  ([`surface.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-cli/src/surface.rs#L1-L32), node `W1-APP-02n`).
- Nothing in the transcript is executable in one keypress, which is INV-4 expressed as layout
  ([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L80-L86)).

### 1.5 State home and journal

- One product root holds projects and their runs; the journal is digest-linked and recoverable, and
  a poisoned lock still records a terminal (nodes `W1-APP-02w`, `W1-APP-02a`, `W1-COR-03q`,
  `W1-COR-03s`).
- The user's working tree is an input, and applying an accepted candidate back into it is a
  separate explicit export
  ([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L170-L174)).

## 2. What contradicts the brief

Each finding names the brief clause it violates, the current behaviour with its evidence, and the
ownership move that closes it. No finding proposes removing a mechanism.

### G-01 · The operator must supply the acceptance condition

**Brief.** §1, §17.1, §18 — the operator must not be required to prepare acceptance criteria,
oracle bundles or verifier implementations.

**Now.** `RunRequest.acceptance` is `Option`, and a request that states none is refused with the
missing part named
([`contract.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-application/src/contract.rs#L38-L48),
[L162-L167](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-application/src/contract.rs#L162-L167)).
The doc comment states the rule plainly: "It never invents the acceptance condition: a request
without one is refused with the missing part named."

**Move.** The acceptance condition stops being an operator input and becomes a derived internal
object. The refusal survives — it moves from the operator's request to the internal derivation,
which must still refuse to run against a condition it cannot show to discriminate.

### G-02 · The refusal path tells the operator to write a verifier

**Brief.** §1 — "Please create an acceptance oracle / agent contract / verifier" is named as the
inappropriate question.

**Now.** Two different refusals return the work to the operator, and they are not the same refusal.
A project whose test entry point ymp will not propose from — npm, for the reason node `W1-APP-02z`
recorded — is refused with the instruction to "state a verifier of your own instead"
([`answer.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-application/src/answer.rs#L94-L99)).
A project with no detectable test entry point at all is refused more quietly, with the statement
that ymp proposes a verifier from the way a project already runs its tests and this directory names
none it can run
([`answer.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-application/src/answer.rs#L89-L93)) —
which leaves the operator with the same task and without naming it.

**Move.** Generation becomes the default source of the acceptance plan for every project rather
than the fallback for projects with tests; node `W1-APP-02z.3` already records this decision and is
open. What cannot be derived is named as the operator's own semantic remainder, not as an artifact
they must author.

### G-03 · No participant is started from any operator surface

**Brief.** §7 lifecycle; addendum — after the goal is entered the first participant is created by
the system.

**Now.** `ymp-tui` declares no dependency on `ymp-runtime-supervisor`
([`Cargo.toml`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/Cargo.toml#L8-L20)),
and the authorization action stores the contract, records the approval and returns
([`app.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/src/app.rs#L471-L517)).
A managed candidate run is reachable only from an internal command
([`internal.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-cli/src/internal.rs#L103-L105)).

**Move.** A mechanical bootstrap step is added above the application core: on authorization it
registers the origin participant, funds it from the root budget and starts one invocation. It makes
no semantic choice. This is the single largest missing piece of the brief.

### G-04 · The engine and its model are chosen outside the collective, one model each

**Brief.** §3, §15; addendum — no hard-coded model roles, the kernel is not a semantic router,
the collective chooses among approved models.

**Now.** The engine is an argument of an internal command
([`internal.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-cli/src/internal.rs#L141-L149)),
and each driver profile pins exactly one model and refuses any other
([`runtime-claude/lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-claude/src/lib.rs#L24),
[L141](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-claude/src/lib.rs#L141);
[`runtime-codex/lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-codex/src/lib.rs#L25),
[L119](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-codex/src/lib.rs#L119)).

**Move.** The pinned pair becomes one entry of a catalog rather than the only reachable route. A
participant names the entry it wants; the kernel checks set membership and resources. Version and
conformance pinning survive as properties of a catalog entry, which is what makes the pinning
meaningful rather than exclusive.

### G-05 · There is no provider level, no catalog and no pool

**Brief.** §4 three levels; §15 catalog/pool abstraction; addendum
`Provider → model catalog → permitted pool → participants`.

**Now.** The readiness page lists the three shipped drivers, one route each, with no provider, no
model list, no enabled flag and no pool
([`runtimes.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/src/runtimes.rs#L60-L75)).
Node `W1-APP-02e.6` opens the registry and is not started; node `W1-EVL-04d`, which decides where
the permitted set is declared, is deferred.

**Move.** The registry of `W1-APP-02e.6` becomes the catalog level exactly as written, and the pool
is a declared subset above it. Neither level creates a participant.

### G-06 · The fake runtime is offered to the operator as a profile

**Brief.** §4 — the operator connects real providers; a profile the product cannot start must not
be presented as one it can.

**Now.** The fake in-process runtime is probed and listed alongside Codex and Claude Code
([`runtimes.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/src/runtimes.rs#L62-L73)),
and `ymp-tui` links it
([`Cargo.toml`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/Cargo.toml#L19)).
Node `W1-APP-02s` records this and is open.

**Move.** The interface stops constructing drivers; it reads a catalog. The fake is then neither
linked nor listed, and `W1-APP-02s` closes as a consequence rather than as separate work.

### G-07 · Authorization is shaped as authorship

**Brief.** §14 — "acceptance mechanics as mandatory workflow steps" is named an ownership leak.

**Now.** The most consequential screen is a requirement-to-evidence coverage map the operator is
expected to review, with blocking items that disable authorization
([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L88-L99)),
modelled in state as requirements with coverage states
([`state.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/src/state.rs#L139-L170)).

**Move.** The decision the operator takes is authorizing a spend against a restated goal, not
reviewing an oracle. The coverage map survives in full as an on-demand diagnostics surface, because
it is genuinely the best evidence about a derived acceptance plan; it stops being the gate.

### G-08 · The product vocabulary collides on "workspace"

**Brief.** §6 lists workspace selection as a surface; §13 puts *workspace* in the product language.

**Now.** The word is reserved for the private writable copy of one attempt and is explicitly
forbidden from naming the enclosing scope
([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L112-L117)).

**Move.** The brief wins at product level: *workspace* names the project the goal is about, and the
per-attempt copy is renamed to *attempt sandbox*, a term the operator never sees. This is a rename
of an internal word, not a change of mechanism.

### G-09 · Two accountings of the same run

**Brief.** §5 — preserve the existing budget distinctions.

**Now.** The run state carries a two-dimension budget mutated by run events
([`lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/lib.rs#L16-L29),
[L390-L406](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/lib.rs#L390-L406)),
while the ledger carries ten
([`budget.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/commitment/budget.rs#L39-L50)).
Node `W1-COR-03n` exists because the two disagreed.

**Move.** One accounting. The run-level figure becomes a projection of the root account rather
than a separately mutated number. Nothing about enforcement changes; the duplicate disappears.

### G-10 · A participant cannot recruit, delegate, or speak

**Brief.** §2, §5, §18.

**Now.** The tool surface is four tools: read control, read events, submit, yield
([`agent-api/lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-agent-api/src/lib.rs#L7-L20)).
Every command the brief's scenario needs is already defined in the ledger and in
[`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROTOCOL.md#L280-L292);
none of it is projected to a participant.

**Move.** Project the accepted tool families that the brief's scenario requires. This is
implementing an accepted specification, not new design.

### G-11 · The collaboration plane does not exist in code

**Brief.** §11 activity view; §18 two participants disagree and resolve.

**Now.** There is no board module, message record or audience grant in the workspace; the
composition document states plainly that participants, messages and the board are absent from the
domain
([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L169-L178)).
Node `W1-COR-03c` is open.

**Move.** None at the design level: the plane is specified and the node exists. The redesign
depends on it and says so.

### G-12 · Recruitment carries nothing the kernel could check against a pool

**Brief.** §3 — the kernel checks provider, model and runtime availability, policy, budget,
concurrency, participant-start limits, capability restrictions, disclosure policy, assurance
profile.

**Now.** `RegisterParticipant` carries participant, principal, sponsor and endowment
([`protocol.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/commitment/protocol.rs#L29-L36));
there is no field naming what the new participant would run on, so the mechanical checks the brief
lists have no subject.

**Move.** One field and one refusal: the requested catalog entry, and a refusal when it is outside
the run's permitted pool. Set membership is a mechanical operation; it is not a ranking and cannot
become one.

### G-13 · A run admits one active attempt

**Brief.** §2, §18 — two participants investigate concurrently, one implements while another
reviews.

**Now.** The supervisor refuses a second active attempt by design of the POC profile
([`lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-supervisor/src/lib.rs#L683)).

**Move.** The ceiling becomes a policy number enforced from the budget vector and the concurrency
limit, not a constant in the supervisor. The isolation rule it protects — concurrent attempts never
share a writable copy — is unchanged and is what the ceiling must keep true.

### G-14 · Provenance is not represented anywhere

**Brief.** §8 — classes A–E must be distinguished; assumptions that materially affect the result
must be asked about rather than silently invented.

**Now.** The stored contract carries the prompt and one verifier record
([`contract.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/contract.rs#L112-L121)).
There is no requirement object, no class and no path from a requirement to the check that observes
it.

**Move.** The internal contract gains requirements, each with a provenance class and the plan item
that observes it. This is the state change that makes the result explainable without exposing
reasoning.

### G-15 · The product cannot ask a question

**Brief.** §1 — ask the operator when intent genuinely cannot be determined; §8 class E.

**Now.** A typed line either amends a draft field or replaces the work statement
([`draft.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/src/draft.rs#L67-L88)).
There is no question object, no answer record and no way for an answer to become part of what the
run is judged against.

**Move.** A clarification is a typed object with a bounded budget, and its answer is recorded as
provenance E. Without this, the only two available behaviours are inventing a requirement and
refusing — and the brief forbids both.

### G-16 · A store holds one run

**Brief.** §12 `[Continue]`; §6 history and archive surfaces.

**Now.** A second run is refused with the instruction to use another store
([`app.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/src/app.rs#L472-L476)).

**Move.** The accepted root layout already addresses runs by sequence under a project
(node `W1-APP-02w`); the session reads and creates runs within it instead of owning one.

### G-17 · The state root defaults to the launch directory

**Brief.** §6 startup and first run.

**Now.** The default root is `.ymp` beside the project
([`lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-cli/src/lib.rs#L26-L33)).
Node `W1-APP-02w.1` moves it to the product root under the home directory and is open.

**Move.** None beyond that node, which the redesign depends on: the catalog, the provider
credentials reference and the run history all live under the product root.

### G-18 · There is no result surface

**Brief.** §12 — not "the agents stopped" but "here is the result", with five named actions.

**Now.** The pages are runtimes, candidates, events, budgets, attempts and describe
([`state.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/src/state.rs#L74-L96)).
A candidate list is not a result, and none of the five actions exists.

**Move.** A result surface, and an export path. Export is the only write into the project
directory, which node `W1-APP-02w.1` already scopes.

### G-19 · Provider authentication is outside the product

**Brief.** §4 — the operator connects and authenticates providers from `/providers`, with ready and
not-configured states.

**Now.** Installation and provider authentication are explicit prerequisites the operator arranges
elsewhere
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L84-L88)).

**Move.** The product shows provider state and takes the operator to the engine's own
authentication where the engine owns the credential. ymp does not become a credential store; it
becomes the place where the operator learns what is ready and what is not.

### G-20 · The document of record makes the package an operator obligation

**Brief.** §14 — "PROJECT-CONTRACT.md as mandatory input" is the first named ownership leak.

**Now.** The approved package has eleven required contents, drafting is "repository analysis plus a
structured interview", and an amendment requires fresh human approval
([`PROJECT-CONTRACT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROJECT-CONTRACT.md#L14-L46),
[L53-L71](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROJECT-CONTRACT.md#L53-L71)).

**Move.** The package survives entirely as an internal object with the same eleven contents and the
same validation. What changes is who fills it and what the human approves. The design records the
required wording changes rather than editing the document.

### G-21 · Drafting is described as a privileged run outside the collective

**Brief.** §2 — no fixed role hierarchy; §9 — no manager standing behind the interface.

**Now.** Contract drafting is specified as a run under a built-in package approved once, whose
protection rests on the boundary between two runs
([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L126-L141)).
This is sound protection, but as written the drafting run is a fixed procedure standing before the
collective rather than part of it.

**Move.** Keep the two-run boundary — it is the mechanism that keeps a protected plan out of the
reach of the participants judged by it — and stop describing the drafting run as the operator's
step. It is internal derivation, invisible unless the operator opens diagnostics.

## 3. What is absent rather than contradictory

These are not violations; they are unbuilt parts of an accepted specification that the redesign
depends on. They are listed so the design does not appear to invent them.

| Item | Specified in | Node |
|---|---|---|
| Scoped collaboration board with attribution and no authority | [`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROTOCOL.md#L203-L222) | `W1-COR-03c` |
| Competing submissions with immutable candidate ancestry | [`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROTOCOL.md#L224-L228) | `W1-COR-03d` |
| Interface surfaces for local commitments and communication | [`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L576-L584) | `W1-COR-03e` |
| Commitment facts durable rather than in memory | — | `W1-COR-03f` |
| Engine registry with properties and model lists | — | `W1-APP-02e.6` |
| Product root under the home directory | — | `W1-APP-02w.1` |
| Generated acceptance check for a project without tests | — | `W1-APP-02z.3` |
| Where the permitted model set is declared | — | `W1-EVL-04d` |

## 4. Required changes by layer

### 4.1 Ownership

| Moves from | Moves to | Closes |
|---|---|---|
| Operator authors the acceptance condition | Internal derivation from goal, repository and policy | G-01, G-02 |
| Operator reviews an oracle coverage map to authorize | Operator authorizes a spend against a restated goal | G-07 |
| Operator selects an engine per run | Collective requests a catalog entry; kernel checks membership | G-04, G-12 |
| Operator arranges provider readiness elsewhere | Product shows provider state and routes to authentication | G-19 |
| Operator assembles or approves the team | System bootstraps participant one; participants recruit | G-03 |
| Document of record obliges the operator to a package | Same package, filled internally, approved as a spend | G-20, G-21 |

### 4.2 State and domain

- Internal contract gains requirements with provenance classes A–E and an acceptance plan that
  names which item observes which requirement (G-14).
- Clarification becomes a typed object with an answer of class E and its own budget dimension
  (G-15).
- Catalog entry (provider, engine, model, measured properties, enabled flag) and run policy
  (permitted pool, ceilings, disclosure class, assurance profile) become stored objects (G-05).
- `RegisterParticipant` gains the requested catalog entry; a new refusal names a placement outside
  the pool (G-12).
- The run-level budget becomes a projection of the root account rather than a second accounting
  (G-09).
- A project holds many runs, addressed by sequence under the product root (G-16, G-17).

### 4.3 API and crates

- The participant tool surface grows from four tools to the accepted tool families needed by the
  brief's scenario: observe, communicate, contract, lifecycle, artifact, verification (G-10).
- A composition-level conductor sits above the application core and the supervisor, so the
  interface can start a participant without linking a runtime (G-03, G-06).
- The single-active-attempt constant becomes a policy ceiling enforced from budget and concurrency
  (G-13).

### 4.4 Interface

- Provider, catalog and pool surfaces appear; the readiness page stops constructing drivers
  (G-05, G-06, G-19).
- Authorization is restated as a spend decision; the coverage map becomes diagnostics (G-07).
- Collective, participant, activity, result, evidence, export, history and archive surfaces appear
  (G-03, G-11, G-18).
- The operator-facing word *workspace* names the project; the per-attempt copy is renamed
  internally (G-08).

## 5. What must not change

Recorded here because the addendum's rule — move responsibility, do not remove mechanisms — is
easiest to violate while closing G-01 and G-07.

1. A run is still judged against an approved definition of done. The approval act changes shape; it
   does not disappear. Constitutional constraint 1 in
   [`CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/CONCEPT.md#L163-L171)
   stands.
2. A derived acceptance plan is still required to reject the negative control and every
   substituted-entry-point control before it can decide a run (nodes `W1-APP-02u`, `W1-APP-02z`).
3. Protected material never enters the collaboration plane, and the producing attempt never writes
   verifier state (INV-4, INV-5).
4. Every created participant, attempt, invocation, offer and obligation still consumes a reserved
   dimension of a finite vector (INV-2).
5. Exhausted, cancelled, abstained, infrastructure error and accepted remain five distinct
   terminals, and quiescence is never acceptance (INV-7).
6. The kernel gains one new refusal and one new kind of check — containment of a catalog entry
   against facts already committed — and gains no ordering, ranking or preference over that set
   (INV-1). Liveness is measured outside the kernel and reaches it as a fact, so `ymp-domain`
   performs no input or output and a replayed journal reaches the same decision.

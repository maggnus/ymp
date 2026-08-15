# Migration

Item 14 of [`PRODUCT-BRIEF-collective-v2.md`](PRODUCT-BRIEF-collective-v2.md) Part A §24: what the
correction preserves, what changes owner, what closes, and what work it proposes — honestly,
including the work it costs. Read [`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) first; leaks cited
as `L-nn` are in [`COLLECTIVE-GAP-ANALYSIS.md`](COLLECTIVE-GAP-ANALYSIS.md).

Node identifiers are those of [`work/STATUS.md`](../work/STATUS.md) at
[f0376be](https://github.com/maggnus/ymp/commit/f0376be).

**This is a rebase, not a restart.** The migration plan the previous design proposed had one unit
built and accepted since — P1, the provider and catalog records — and six of the nodes it was
waiting on have been accepted. Brief v2 changes the *shape* of three units and the *ownership* of
one, and leaves the rest as they were. The correction is a boundary move; the plan is a plan for
moving a boundary.

## 1. Survives as built

Consumed exactly as accepted. No mechanism they established is weakened and no evidence they
produced is invalidated.

| Node | What it established | Why it survives |
|---|---|---|
| `W1-COR-03a` | Local commitments conserve budgets and close obligations | The accounting v2 §12 demands is this |
| `W1-COR-03b` | Yielded participants resume finitely; runs terminate honestly | Bounded wakes are what make recruitment affordable |
| `W1-COR-03g`–`03j` | Settlement, conservation, emitted facts proved against their commands | Untouched by a boundary move |
| `W1-COR-03k`–`03m` | The live controller drives the kernel lifecycle; verification is a kernel fact | The launcher of design item 8 follows exactly these facts |
| `W1-COR-03n`–`03s` | Cancel, panic, poisoned lock and limit-expiry paths reach a terminal | The distinct terminals of §15 rest on them |
| `W1-APP-02a`, `02a.1`, `02a.2` | Recoverable event history; recovered command identifiers; no public headless mode | Unchanged; the launcher is not a headless mode |
| `W1-APP-02b` | A private attempt produces an independently verified immutable candidate | The core v2 §10 preserves explicitly |
| `W1-APP-02c`, `02d` | Codex and Claude Code profiles complete a managed attempt | Each becomes a catalog record rather than the only route |
| `W1-APP-02f` | Duplicate or out-of-order runtime events terminate without a candidate | Unchanged |
| `W1-APP-02g` | Verification evidence binds the exact runtime environment | The evidence §15 asks the Result to carry |
| `W1-APP-02h` | Managed sessions resume after interruption | Needed more, not less, once several participants run |
| `W1-APP-02i`, `02l`, `02y` | Accounting reports only counters the runtime sent; evidence names the models that spent; the run record carries per-model spend | Per-model spend is what makes a pool measurable |
| `W1-APP-02k` | No managed descendant survives its supervisor | Unchanged |
| `W1-APP-02n` | Every interface action exists as a command of the same executable | Extended to the new actions, not replaced |
| `W1-APP-02o`, `02o.1`, `02o.2` | No path starts a run without a contract; the guards parse rather than truncate | Becomes "no path starts a run without a derived internal contract" |
| `W1-APP-02p`, `02q`, `02r` | Every program in the launch chain is verified; admission refuses rather than degrades; an unreadable marker is not an absent holder | The mechanical half of §12's admission |
| `W1-APP-02s` | The fixture runtime is neither linked nor offered | Already closed one of the leaks the previous design was going to close |
| `W1-APP-02t` | Lifecycle tests isolated from other test binaries | Unchanged |
| `W1-APP-02w`, `02w.1`, `02w.2`, `02w.3` | Product state under one root, under `~/.ymp`; a run identifier names the run; export applies in place | The layout the catalog, the pools, the history and export need |
| `W1-APP-02e.1` | Deterministic TUI buffers enforce the screen contract | The method by which the new surfaces are proved |
| `W1-APP-02e.2`, `02e.3` | The chat-first visual contract and its ratatui implementation | The shape of every surface in [`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md) |
| `W1-APP-02e.5` | The start screen carries the logo and one line of basics | **Now load-bearing:** v2 §5 asks for exactly this, and the previous design's startup contradicted it (leak L-14) |
| `W1-APP-02e.6` | Engines as managed entities with an enabled flag, measured properties and model lists | The engine level beneath providers |
| `W1-APP-02x` | Entry validation cannot hold the interface | Applies to derivation, which must not block the input line |
| `W1-PRD-05b` | Provider records above the engine records; the catalog derived from both | The `Provider → Model catalog` half of §1, built |
| `W1-EXP-01a` | The corpus rejects known invalid candidates | The discrimination requirement a derived plan must still meet |
| `W1-EXP-01b` | The matched-budget study has a frozen decision rule | The reason the pool is frozen by digest |
| `W1-EXP-01c` | The protocol model terminates under declared fault schedules | Unchanged |
| `W1-EXP-01d`, `01d.1`, `01d.2` | Runtime probes expose incompatible profiles | Becomes catalog readiness measurement |
| `W0-UX-01a`–`01c` | The reviewed screen contract that precedes implementation | The method this design follows |

## 2. Changes owner

The mechanism stays; the party responsible for it moves inside the product.

### `W1-APP-02v` — the contract dialogue becomes a derivation run

**Was.** A typed request produces an automatically assembled draft contract, presented for one
approval, amendable by further typed lines.

**Becomes.** The assembly becomes the derivation run of design item 16 and stops being a dialogue
about a contract. The `/` prefix, the single approval and the amendment grammar all survive: the
prefix as accepted, the approval reinterpreted as the spend decision of surface S10, and the
amendment grammar as an advanced override that is never required, offered or implied.

### `W1-APP-02m` — "a typed prompt becomes a contract and starts a run" is completed

**Was.** Accepted: a typed prompt becomes a contract and starts a run, where "starts a run" meant
the run object exists in the journal bound to its approved contract.

**Becomes.** The same sentence, finished. After bootstrap the typed goal becomes a derived contract
*and starts a participant working on it* (leak L-09). Recorded here so the completion is an
extension of an accepted node rather than new ground.

### `W1-APP-02u` — the discriminating-verifier gate moves behind the boundary

**Was.** The dialogue accepts as a verifier only a program that exists, can be executed and is shown
to reject the negative control before the contract is stored.

**Becomes.** The same gate applied to every derived check before the run is created. A plan that
accepts the negative control never starts one. What disappears is the operator-typed answer the gate used
to validate, not the gate — which is the clearest case of §19's closing rule.

### `W1-APP-02z` and `W1-APP-02z.3` — the pinned generated check becomes the acceptance plan

**Was.** `02z`: a verifier that delegates to a file inside the candidate is pinned to the approved
bytes. `02z.3` (now accepted): a project without tests receives a generated verifier proposal for
approval.

**Becomes.** `02z`'s pinning is a property of every plan check and is untouched. `02z.3`'s generation
becomes the default source of the plan everywhere; detected project tests are the strongest
generator, not the precondition; and the operator's act is not approval of a program but
the goal sentence itself. Its semantic-remainder line becomes design item 16's rule and surface S19.

**Open sub-node.** `02z.1` (the npm entry point is pinned or stays refused) survives as written: it
is about the honesty of a refusal, which the correction needs more of, not less.

### `W1-APP-02e.4` — profile admission becomes catalog readiness

**Was.** The pinned runtime profiles admit the owner's host.

**Becomes.** The same measurement, reported as the readiness of catalog entries rather than of the
only available profiles. A pinned version stays the recorded identity of an entry; it stops being
the reason no other route exists (leak L-04).

### `W1-EVL-04d` — the model-use policy became the AgentPool

**Was.** Deferred: where the permitted model set for a run is declared, and how it composes with
pool formation.

**Becomes.** Closed by owner decision D1 and then refined by brief v2: the permitted set is the
`AgentPool` resource — a capability boundary with an automatic `default` — and the per-run freeze is
an internal snapshot. The node is recorded as decided; the *shape* of the answer changed after it
closed, which is why the decision carries a v2 refinement block rather than a new node.

## 3. Closes, or is superseded

| What closes | Replaced by | Proof required first |
|---|---|---|
| The operator-facing acceptance condition: `RunRequest.acceptance` as a required input and its refusal ([`contract.rs:44`](../../ymp-rust/crates/ymp-application/src/contract.rs), [`:183-194`](../../ymp-rust/crates/ymp-application/src/contract.rs)) | The derivation run (design item 16) | Tests 1–3, 12, 15: a derived plan is complete, classified and discriminating |
| The four refusals that leave the acceptance condition with the operator ([`answer.rs:108-125`](../../ymp-rust/crates/ymp-application/src/answer.rs), [`:133-143`](../../ymp-rust/crates/ymp-application/src/answer.rs)) | Generation everywhere, plus the honest semantic-remainder statement | Node `W1-APP-02z.3` and test 15 |
| **"which agent does the work is yours to decide"** ([`attempt.rs:143-151`](../../ymp-rust/crates/ymp-tui/src/attempt.rs)) and the public `--runtime` argument ([`surface.rs:88-96`](../../ymp-rust/crates/ymp-cli/src/surface.rs), [`:191-193`](../../ymp-rust/crates/ymp-cli/src/surface.rs)) | The collective names a catalog entry; the kernel checks containment | Tests 6–8 |
| **The second operator act that starts work**: `/attempt` as a public step and the "nothing is being done yet" reply ([`app.rs:800-820`](../../ymp-rust/crates/ymp-tui/src/app.rs)) | Bootstrap as soon as the run exists | Test 4 |
| One model per driver profile, enforced by refusing any other | Model as a catalog-entry field; pinning as recorded identity | Conformance evidence per pairing |
| The single-active-attempt constant ([`supervisor/lib.rs:935`](../../ymp-rust/crates/ymp-runtime-supervisor/src/lib.rs)) | A policy ceiling from pool capacity and the budget vector | The isolation property it protected, proved for concurrent sandboxes (node `W1-COR-03d`) |
| One run per store ([`app.rs:863-871`](../../ymp-rust/crates/ymp-tui/src/app.rs)) | Runs addressed by sequence under a workspace | Node `W1-APP-02w` layout, already accepted |
| The two-dimension run budget as an independent accounting | A projection of the root account's vector | What `W1-COR-03n` had to check becomes structural |
| Contract authorization as a requirement-coverage review ([`state.rs:154-171`](../../ymp-rust/crates/ymp-tui/src/state.rs), [`decisions.rs:15-21`](../../ymp-rust/crates/ymp-tui/src/decisions.rs)) | Nothing: decision D3 removes the step. The run-start block states what *done* means; the coverage map moves to diagnostics (S34) | That the map is still reachable and still complete |
| **Typed identifiers as the mechanism of every irreversible act** ([`surface.rs:23-24`](../../ymp-rust/crates/ymp-cli/src/surface.rs), [`:78-101`](../../ymp-rust/crates/ymp-cli/src/surface.rs), [`VISUAL_CONCEPT.md:257-260`](../VISUAL_CONCEPT.md)) | A key on a selected row, with the consequence stated above it | Tests 41–43 |
| **The roster sentence** in the document of record ([`VISUAL_CONCEPT.md:117-123`](../VISUAL_CONCEPT.md)) | Pool capacity and `ParticipantStarts`; the collective decides its own size | Tests 4, 33, 34 |
| **Probing before enabling** ([`runtimes.rs:13-17`](../../ymp-rust/crates/ymp-tui/src/runtimes.rs)) | Probe triggered by the enable transition and by an explicit refresh | Tests 37, 38 |

### 3.1 Accepted text the brief supersedes

Two lines of accepted scope do not survive, recorded here rather than left to be discovered.

`W1-APP-02v` places out of scope "inventing an acceptance condition on the user's behalf, which
stays forbidden". Brief §1 and §10 assign exactly that machinery to the product, and node
`W1-APP-02z.3` already moved in this direction by owner decision. What the superseded line was
protecting is kept by other means: the derived plan is validated against the negative control and
the substituted-entry-point controls before it can decide anything, its provenance is recorded per
requirement, its checks and digests are one keystroke from the run-start block, and the part
of the goal it cannot observe is named rather than dropped. The condition is derived and
demonstrated; it is not invented.

`W1-APP-02e.6` records that "*which* models may be used stays undecided until the `W1-EVL-04d`
research lands". That is now decided twice over — by owner decision D1 and by brief v2's AgentPool —
and the sentence is superseded rather than outstanding.

One change to an accepted node's acceptance is also proposed. `W1-APP-02z.3` requires the generated
program to be "shown in the draft before authorization". There is no authorization step after
decision D3: the run-start block states what *done* means in plain sentences and the program text
and its digest sit one keystroke away under diagnostics. The node's intent — the operator can see
what will judge the work — is met; the moment it named no longer exists.

## 4. Unaffected, and stated so

`W1-APP-02j` (runtime admission stops duplicating the pinned executable per instance, deferred) is
untouched. `W1-COR-03t` (a cancel interrupts a runtime that is still working) and `W1-COR-03u` (the
application recovers its memory from the journal after an interrupted apply) become more important
with several participants but need no change of scope.

`W1-COR-03f` — commitment facts durable rather than only in memory — is **accepted** since this
design set was last written ([3f9e604](https://github.com/maggnus/ymp/commit/3f9e604)), and with it
the precondition P5 and P10 depend on. The kernel decides recruitment by containment over committed
facts and performs no probing of its own, so a fact stream held only in memory would have meant a
decision a replayed journal could not reproduce, and evidence that could not state which entries
were permitted when. `W1-COR-03x` (the run's commitment kernel journals through the durable path) is
open and carries the remainder of that work; `W1-COR-03d` (competing submissions with immutable
candidate ancestry) is active and is the precondition of P12.

`W1-EVL-04a`–`04c` — the matched-budget arms, the message interventions and the reproducible POC
decision — are unaffected in method and better served in substance, because a frozen pool and
per-model spend are what make matched budgets comparable across arms. The owner's recorded position
that experiments run on the cheaper Claude routes is expressed as a pool, which is exactly what a
pool is for.

## 5. The rebased plan

Proposed for the CTO to record as nodes; this card creates none. Ordered by dependency, since the
order is itself a design statement: nothing that spends money is built before the thing that bounds
it. Units are kept small enough to be accepted independently.

| # | Unit | Depends on | Closes | State |
|---|---|---|---|---|
| **P1** | Product root carries provider records and a derived model catalog; engines are managed entities beneath providers | `W1-APP-02e.6`, `W1-APP-02w.1` | part of L-05 | **accepted** `W1-PRD-05b` at [09fc9c4](https://github.com/maggnus/ymp/commit/09fc9c4) |
| **P2** | Provider and model surfaces; probing gated on the enabled flag; the interface stops constructing drivers. **Carries P1's two review residues:** the stored provider state must state the age of its observation, and a deleted engine record must read as a route with no models rather than shortening the catalog silently | P1 | L-05, L-13, L-14 | proposed |
| P2a | A provider reached through another vendor's engine: route override on an engine profile — endpoint class, wire protocol, account scope, authentication mode, disclosure class — with a conformance probe per pairing | P1 | §22 step 4 without a native runtime | proposed |
| **P3** | **The `AgentPool` resource and its reconciler**: selector or explicit ordered list, capacity, limits, resolved entries and digest — and the **automatic `default`** created on the first ready provider | P1 | L-07, L-08 | proposed |
| P4 | `/pools` list and properties; editing `default` replaces tracking with an explicit list and says so | P3 | L-08 | proposed |
| **P5** | Run policy and the pool freeze: `PoolFrozen { entries, entry_model, digest }` committed when the run is created; `Run.declared.poolSnapshot` by value | P3, `W1-COR-03f` (accepted) | L-08, L-12 | proposed |
| P6 | Derivation in two halves — local reading, then the **derivation run** with its own budget, audience and capabilities — producing requirements with provenance, the acceptance plan and the verification strategy, with the discrimination gate applied to every check | `W1-APP-02u`, `W1-APP-02z`, `W1-APP-02z.3` | L-01, L-02, L-03, L-15 | proposed |
| P6a | The derivation allowance as a dimension of the workspace's standing ceiling: defaulted from D8, enforced as its own dimension, reported in the run-start block | P6 | spend before the run exists | proposed |
| P7 | Clarification as a typed object with its own budget dimension; the divergence classifier, its operator resolution, and the **kernel's refusal** of a verification reservation while a divergence is unresolved | P6 | L-15 | proposed |
| **P8** | **The standing ceiling and the end of ceremony**: the workspace's ceiling as a stored object edited in place on `/budget`; the authorization surface, the setup question and every `--confirm <ID>` retire; pause, cancel, export and archive become keys on a selected row with their consequences stated above them; the coverage map moves to diagnostics | P5, P6, P7 | L-06, L-16 | proposed |
| **P9** | The run launcher: pool frozen, origin participant registered and started; processes follow committed facts | P8 | L-09, L-12 | proposed |
| P10 | `request_participant` names a catalog entry; admission measures liveness and commits availability facts; the kernel checks containment only | P5, P9, `W1-COR-03f` | L-04, L-10 | proposed |
| P11 | The participant tool projection extended to the accepted families | P9, `W1-COR-03c` | L-10 | proposed |
| P12 | Concurrency ceiling from pool capacity; the single-attempt constant retires | P9, `W1-COR-03d` | L-11 | proposed |
| P13 | Agents, tasks, activity and recruitment surfaces | P9, P11, `W1-COR-03c`, `W1-COR-03e` | L-10 | proposed |
| P14 | Result, evidence and export surfaces with the outcome vocabulary of §15; runs addressed by sequence | P9, `W1-APP-02w.1` | L-06 | proposed |
| P15 | One accounting: the run budget becomes a projection of the root account | P9 | — | proposed |
| P16 | The *attempt sandbox* rename across documents and internal identifiers | D9 | — | proposed |
| P17 | The end-to-end scenario of §22 driven on the built product | P1–P15 | test 45 | proposed |

**What changed in the rebase.** P3 is new in substance: it was "run policy and permitted pool,
waiting on owner decision D1", and it is now the AgentPool resource with an automatic `default` and
no dependency on an open decision. P5 is what remains of the old P3 — the freeze alone. P6 changes
owner rather than shape: derivation is a run, not a module. P8 inverts: it was "restate the
authorization surface", and it is now "remove it, and every other confirmation with it", which the
owner's ruling of 2026-08-15 and decisions D3, D4 and D10 require. P2 grows two obligations from
P1's review. Everything else keeps its old position with one number shifted; P1 is done and
`W1-COR-03f` is accepted, so two of the plan's preconditions are already met.

**Reading the order.** P1–P9 (with P2a and P6a) are the minimal path to the brief's central sentence
with one participant: goal in, collective starts, verified result out. P10–P13 are what make it a
collective. P14–P16 are the product finish. P17 is the acceptance of the whole. Three already-open
nodes are preconditions rather than neighbours: `W1-COR-03f` for P5 and P10, `W1-COR-03c` for P11,
and `W1-COR-03d` for P12.

## 6. What the migration must not do

Repeated from the gap analysis because it is easiest to violate while executing P6 and P8.

1. A run is still judged against an approved definition of done; the approval act changes shape and
   does not disappear.
2. A derived plan still has to reject the negative control and every substituted-entry-point control
   before it can decide a run.
3. Protected material never enters the collaboration plane; the producing attempt never writes
   verifier state.
4. Every created participant, attempt, invocation, offer and obligation still consumes a reserved
   dimension of a finite vector, and recruitment is never free.
5. The terminals stay distinct, quiescence is never acceptance, and `infrastructure_error` is never
   a rejection.
6. The kernel gains containment over committed facts and one acceptance-path hold, and no ordering,
   ranking or preference beyond the declared order used once at bootstrap. Liveness stays outside
   `ymp-domain`, which performs no input or output.
7. **No unit may introduce a component that reads the goal.** Semantic work belongs to a collective
   run; a controller that branched on goal content would reintroduce the leak the whole correction
   exists to close, however small the branch.
8. **No unit may reintroduce a confirmation or a typed identifier.** Removing them is not a licence
   to remove what they carried: every consequence a dialogue used to state must be stated on the
   object it applies to, before the key that performs the act. A unit that deletes the ceremony and
   the sentence together has made the product less honest, not simpler.

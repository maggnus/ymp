# Migration from the current design

Item 23 of the design deliverable: what the redesign preserves, what changes owner, what closes,
and what new work it proposes — node by node, honestly, including the nodes it costs. Read
[`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) first; findings cited as `G-nn` are in
[`COLLECTIVE-GAP-ANALYSIS.md`](COLLECTIVE-GAP-ANALYSIS.md).

Node identifiers are those of
[`work/STATUS.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/work/STATUS.md)
at the baseline revision
[dfdac03](https://github.com/maggnus/ymp/commit/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf).

The proportions matter more than any single row. `STATUS.md` records seventy-nine nodes, fifty-three
of them accepted. The redesign invalidates none of the fifty-three: forty-seven are consumed exactly
as accepted, four change owner without losing a mechanism, and two are the cards above them. Nine
nodes change owner in total — four accepted, five open, active or deferred. Two open nodes close as
consequences rather than as separate work. Nine behaviours of the current build close and are
replaced. One sentence of one accepted node is superseded by the brief, and is named as such in
§3.1 rather than left to be discovered.

The redesign is a boundary move, not a rewrite.

## 1. Survives as built

These nodes are consumed by the redesign exactly as accepted. No mechanism they established is
weakened, and no evidence they produced is invalidated.

| Node | What it established | Why it survives |
|---|---|---|
| `W1-COR-03a` | Local commitments conserve budgets and close obligations | The accounting the brief §5 demands is this |
| `W1-COR-03b` | Yielded participants resume finitely; runs terminate honestly | Bounded wakes are what makes recruitment affordable (brief §5) |
| `W1-COR-03g`–`03j` | Settlement, conservation, emitted facts proved against their commands | Untouched by a boundary move |
| `W1-COR-03k`–`03m` | The live controller drives the kernel lifecycle; verification is a kernel fact | The launcher of item 8 follows exactly these facts |
| `W1-COR-03n`–`03s` | Cancel, panic, poisoned lock and limit-expiry paths reach a terminal | Criterion 17.14 rests on them |
| `W1-APP-02a`, `02a.1` | Recoverable event history; recovered command identifiers preserve one result | Unchanged |
| `W1-APP-02a.2` | No public headless mode | Unchanged; the launcher is not a headless mode |
| `W1-APP-02b` | A private attempt produces an independently verified immutable candidate | The core the brief preserves explicitly |
| `W1-APP-02c`, `02d` | Codex and Claude Code profiles complete a managed attempt | Each becomes a catalog record rather than the only route |
| `W1-APP-02f` | Duplicate or out-of-order runtime events terminate without a candidate | Unchanged |
| `W1-APP-02g` | Verification evidence binds the exact runtime environment | Criterion 17.12 |
| `W1-APP-02h` | Managed sessions resume after interruption | Needed more, not less, once several participants run |
| `W1-APP-02i`, `02l`, `02y` | Accounting reports only counters the runtime sent; evidence names the models that spent; the run record carries per-model spend | Per-model spend is what makes a pool measurable (item 7) |
| `W1-APP-02k` | No managed descendant survives its supervisor | Unchanged |
| `W1-APP-02n` | Every interface action exists as a command of the same executable | Extended to the new actions, not replaced |
| `W1-APP-02p`, `02q`, `02r` | Every program in the launch chain is verified; admission refuses rather than degrades; an unreadable marker is not an absent holder | The mechanical half of brief §3 admission |
| `W1-APP-02t` | Lifecycle tests isolated from other test binaries | Unchanged |
| `W1-APP-02w` | Product state under one root supporting many projects | The layout the catalog, history and runs need |
| `W1-APP-02e.1` | Deterministic TUI buffers enforce the screen contract | The method by which the new surfaces are proved |
| `W1-APP-02e.2`, `02e.3` | The chat-first visual contract and its ratatui implementation | The shape of every surface in [`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md) |
| `W1-APP-02x` | Entry validation cannot hold the interface | Applies to the new derivation, which must not block the input line |
| `W1-EXP-01a` | The corpus rejects known invalid candidates | The discrimination requirement a derived plan must still meet |
| `W1-EXP-01b` | The matched-budget study has a frozen decision rule | The reason the pool is frozen by digest (item 7) |
| `W1-EXP-01c` | The protocol model terminates under declared fault schedules | Unchanged |
| `W1-EXP-01d`, `01d.1`, `01d.2` | Runtime probes expose incompatible profiles | Becomes catalog readiness measurement |
| `W0-UX-01a`–`01c` | The reviewed screen contract that precedes implementation | The method this design follows |

## 2. Changes owner

The mechanism stays; the party responsible for it moves inside the product. This is the addendum's
rule applied node by node.

### `W1-APP-02v` — the contract dialogue becomes internal derivation

**Was.** A typed request produces an automatically assembled draft contract, presented for one
approval, amendable by further typed lines.

**Becomes.** The assembly becomes derivation (item 16) and stops being a dialogue about a contract.
The `/` command prefix, the single approval and the amendment grammar all survive: the prefix as
accepted, the approval reinterpreted as the spend decision of S10, and the amendment grammar as an
advanced override that is never required, offered or implied (surface S08).

**Why not closed.** The node's real content — one approval instead of a question sequence — is
exactly what the brief wants. Only its subject changes.

### `W1-APP-02m` — "a typed prompt becomes a contract and starts a run" is completed

**Was.** Accepted: a typed prompt becomes a contract and starts a run. What "starts a run" meant at
the time is that the run object exists in the journal, bound to its approved contract.

**Becomes.** The same sentence, finished. After bootstrap (item 8) the typed goal becomes a derived
contract *and starts a participant working on it*. This node's title is close to the brief's central
sentence, and the redesign supplies the half of it the code does not yet have (finding G-03).

**Why it is listed here.** So that the completion is recorded as an extension of an accepted node
rather than presented as new ground.

### `W1-APP-02u` — the discriminating-verifier gate moves behind the boundary

**Was.** The dialogue accepts as a verifier only a program that exists, can be executed and is shown
to reject the negative control before the contract is stored; refusals arrive where the answer is
typed.

**Becomes.** The same gate applied to every derived check before authorization. A plan that accepts
the negative control never reaches S10. What disappears is the operator-typed answer the gate used
to validate, not the gate (finding G-01, G-02).

**Why this matters.** This is the clearest case of the addendum's rule: the check that would be
tempting to drop while "simplifying UX" is the one the brief explicitly preserves (§8).

### `W1-APP-02z` and `W1-APP-02z.3` — the pinned generated check becomes the acceptance plan

**Was.** `02z`: a verifier that delegates to a file inside the candidate is pinned to the bytes
approved with the contract, so a candidate cannot rewrite its own judge. `02z.3` (open): a project
without tests receives a generated verifier proposal for the operator's approval.

**Becomes.** `02z`'s pinning is a property of every plan check and is untouched. `02z.3` is the
derivation itself: generation is the default source of the plan everywhere, detected project tests
are merely the strongest generator, and the operator's action is not approval of a program but
authorization of a spend. Its third acceptance line — that a goal whose checkable part is a
produced artifact yields a check of the mechanical claim while the semantic remainder is named as
the operator's own — becomes item 16's semantic-remainder rule and surface S19.

**Open sub-nodes.** `02z.1` (the npm entry point is pinned or stays refused) and `02z.2` (the
refusal names the actual obstacle) survive as written; both are about honesty of a refusal, which
the redesign needs more of, not less.

### `W1-APP-02e.6` — the engine registry becomes the catalog level

**Was.** Engines as managed entities under the product root: enabled flag, measured properties, the
model list each engine can serve, enable and disable from `/runtimes` and the mirrored command;
admission semantics stay compiled in; *which* models may be used stays undecided until `W1-EVL-04d`.

**Becomes.** The same record, read as the model-catalog level of `provider → catalog → pool →
participants` (item 6). Three additions, none of which changes what the node accepted: a provider
level above it (surface S03), a catalog entry as the addressable provider · engine · model triple,
and the pool as a declared subset the kernel checks membership against.

**Note.** The node's own sentence — "*which* models may be used stays undecided" — is precisely the
question the redesign must answer to have a pool at all. It is
[owner decision 1](COLLECTIVE-OWNER-DECISIONS.md#d1--where-the-permitted-model-set-is-declared).

### `W1-APP-02e.4` — profile admission becomes catalog readiness

**Was.** The pinned runtime profiles admit the owner's host.

**Becomes.** The same measurement, reported as the readiness of catalog entries (surface S31)
rather than as the readiness of the only available profiles. A pinned version stays the recorded
identity of an entry; it stops being the reason no other route exists (finding G-04).

### `W1-EVL-04d` — the model-use policy becomes the permitted pool

**Was.** Deferred: a verified recommendation on where the permitted model set for a run is declared
and how it composes with pool formation, weighed against cost, reproducibility of the matched-budget
comparison and honest per-model spend attribution.

**Becomes.** The pool of item 7 is the object the node was about. The redesign supplies the
mechanism and the mechanical consequences of each option; the choice remains the owner's, with the
analysis in
[owner decision 1](COLLECTIVE-OWNER-DECISIONS.md#d1--where-the-permitted-model-set-is-declared).
This node is not closed by the design and must not be recorded as closed by it.

### `W1-APP-02w.1` — the product root gains three tenants

**Was.** Open: the default root moves to `~/.ymp`, drafts live under the root, and only an explicit
export writes into the project directory.

**Becomes.** Unchanged in outcome, larger in consequence: the catalog records, the provider records
and the run history of a workspace all live under that root. The redesign depends on this node
rather than modifying it (finding G-17).

## 3. Closes, or is superseded

Each row names what disappears, what replaces it, and what must be proved before it may be removed.

| What closes | Replaced by | Proof required first |
|---|---|---|
| The operator-facing acceptance condition: `RunRequest.acceptance` as a required input and `MissingPart::AcceptanceCondition` as an operator refusal ([`contract.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-application/src/contract.rs#L38-L48)) | Derivation (item 16) | Tests 1–3, 12, 15 of item 22: a derived plan is complete, classified and discriminating |
| The refusal that instructs the operator to write a verifier ([`answer.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-application/src/answer.rs#L88-L99)) | Generation everywhere, plus the honest semantic-remainder statement | Node `W1-APP-02z.3` and test 15 |
| The per-run engine argument as the way a run selects a runtime ([`internal.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-cli/src/internal.rs#L141-L149)) | Catalog entry recorded by the kernel at recruitment | Tests 6–8 |
| One model per driver profile, enforced by refusing any other ([`runtime-claude/lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-claude/src/lib.rs#L141)) | Model as a catalog-entry field; pinning as recorded identity | Conformance evidence per pairing, as `W1-EXP-01d` established |
| The single-active-attempt constant ([`supervisor/lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-runtime-supervisor/src/lib.rs#L683)) | A policy ceiling from the budget vector and the concurrency limit | The isolation property it protected, proved for concurrent sandboxes (node `W1-COR-03d`) |
| One run per store ([`app.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/src/app.rs#L472-L476)) | Runs addressed by sequence under a project | Node `W1-APP-02w` layout, already accepted |
| The two-dimension run budget as an independent accounting ([`lib.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-domain/src/lib.rs#L16-L29)) | A projection of the root account's vector | The agreement `W1-COR-03n` had to establish becomes structural rather than checked |
| The fake runtime as an offered profile, and its linkage into the interface ([`runtimes.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/src/runtimes.rs#L62-L73)) | The interface reads the catalog instead of constructing drivers | Node `W1-APP-02s`, which this closes as a consequence |
| Contract authorization as a requirement-coverage review ([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L88-L99)) | Run authorization (surface S10); the coverage map moves to diagnostics (S34) | That the map is still reachable and still complete |

### 3.1 Accepted text the brief supersedes

One line of accepted scope does not survive, and it is recorded here rather than left to be
discovered during implementation.

`W1-APP-02v` places out of scope "inventing an acceptance condition on the user's behalf, which
stays forbidden". Brief §1 assigns exactly that machinery to the product — "if such machinery is
required for correctness, ymp creates, derives, validates and manages it internally" — and §16 item
16 names internal ownership of contracts, oracles and verifiers as a deliverable. The brief wins at
product level, and node `W1-APP-02z.3` already moved in this direction by owner decision when it
made generation the default source of the proposal.

What the superseded line was protecting is kept by other means, and the distinction is worth
stating precisely. *Inventing* meant producing an acceptance condition nobody could see and nothing
had tested. After the redesign the derived plan is validated against the negative control and the
substituted-entry-point controls before it can decide anything, its provenance is recorded per
requirement, its checks and digests are reachable in one keystroke from the authorization surface,
and the part of the goal it cannot observe is named rather than quietly dropped. The condition is
derived and demonstrated; it is not invented.

One change to an **open** node's acceptance is also proposed. `W1-APP-02z.3` requires the generated
program to be "shown in the draft before authorization". In this design the authorization surface
states what *done* means in plain sentences and the program text and its digest sit one keystroke
away under diagnostics (surface S34). If the owner reads that line as requiring the program text in
the authorization body, the surface changes and nothing else does.

Two nodes already open are closed by the redesign as consequences rather than as separate work:
`W1-APP-02s` (the fake runtime neither linked nor offered) and `W1-APP-02o` (the shipped binary
carries no path that starts a run without a contract) — the latter because after the redesign every
path starts a run from a derived contract, and the check becomes "no path starts a run without an
authorized internal contract".

## 4. Unaffected, and stated so

`W1-APP-02j` (runtime admission stops duplicating the pinned executable per instance, deferred) and
`W1-APP-02e.5` (the start screen carries the logo and one line of basics) are untouched by the
redesign. `W1-COR-03t` (a cancel interrupts a runtime that is still working) and `W1-COR-03u` (the
application recovers its memory from the journal after an interrupted apply) become more important
with several participants but need no change of scope.

`W1-EVL-04a`–`04c` — the matched-budget arms, the message interventions and the reproducible POC
decision — are unaffected in method and better served in substance, because a frozen pool and
per-model spend are what make matched budgets comparable across arms.

## 5. Work the redesign proposes

Proposed here for the CTO to record as nodes; this card creates none. Ordered by dependency, since
the order is itself a design statement: nothing that spends money is built before the thing that
bounds it.

| # | Proposed unit | Depends on | Closes |
|---|---|---|---|
| P1 | Product root gains catalog and provider records; engines become managed entities | `W1-APP-02e.6`, `W1-APP-02w.1` | G-05, G-06, G-19 |
| P2 | Provider, model-catalog and policy surfaces; the interface stops constructing drivers | P1 | G-05, G-06, `W1-APP-02s` |
| P3 | Run policy and permitted pool as a stored object, frozen by digest at authorization | P1, owner decision 1 | G-05 |
| P4 | Derivation: goal → requirements with provenance → acceptance plan → verification strategy, with the discrimination gate applied to every check | `W1-APP-02u`, `W1-APP-02z`, `W1-APP-02z.3` | G-01, G-02, G-14, G-20 |
| P5 | Clarification as a typed object with its own budget dimension | P4 | G-15 |
| P6 | Run authorization restated as a spend decision; the coverage map moves to diagnostics | P3, P4, P5 | G-07 |
| P7 | The run launcher: origin participant registered and started; processes follow committed facts | P6 | G-03 |
| P8 | `request_participant` names a catalog entry; two mechanical refusals; pool membership checked | P3, P7 | G-12 |
| P9 | The participant tool projection extended to the accepted families | P7, `W1-COR-03c` | G-10 |
| P10 | Concurrency ceiling from policy; the single-attempt constant retires | P7, `W1-COR-03d` | G-13 |
| P11 | Agents, tasks, activity surfaces | P7, P9, `W1-COR-03c`, `W1-COR-03e` | G-11 |
| P12 | Result, evidence and export surfaces; runs addressed by sequence | P7, `W1-APP-02w.1` | G-16, G-18 |
| P13 | One accounting: the run budget becomes a projection of the root account | P7 | G-09 |
| P14 | The *attempt sandbox* rename across documents and internal identifiers | owner decision 9 | G-08 |
| P15 | The end-to-end scenario of brief §18 driven on the built product | P1–P13 | criterion 17.16 |

P1–P7 are the minimal path to the brief's central sentence with one participant. P8–P11 are what
make it a collective. P12–P14 are the product finish. P15 is the acceptance of the whole.

## 6. What the migration must not do

Repeated from the gap analysis because it is easiest to violate while executing P4 and P6.

1. A run is still judged against an approved definition of done; the approval act changes shape and
   does not disappear.
2. A derived plan still has to reject the negative control and every substituted-entry-point control
   before it can decide a run.
3. Protected material never enters the collaboration plane; the producing attempt never writes
   verifier state.
4. Every created participant, attempt, invocation, offer and obligation still consumes a reserved
   dimension of a finite vector.
5. The five terminals stay distinct and quiescence is never acceptance.
6. The kernel gains exactly one new check — set membership — and no ordering, ranking or preference
   over that set beyond the declared order used once at bootstrap.

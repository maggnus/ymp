# Ownership inspection

Items 1 and 2 of [`PRODUCT-BRIEF-collective-v2.md`](PRODUCT-BRIEF-collective-v2.md) Part A §24: a
walk of the repository against the brief, and the ownership leaks it finds. Part A §19 asks for
this to be an inspection of the code and the documents of record rather than a rewrite of the
interface, and for each leak to state **what stays, who takes ownership, the API or state
transition, the change in the terminal interface, and the tests**. Every leak below is written that
way.

Brief v2 is the product-level source of truth. Where it and
[`PRODUCT-BRIEF-collective.md`](PRODUCT-BRIEF-collective.md) or the current design set disagree, v2
wins; kernel trust boundaries and verification principles are preserved by v2 explicitly, and §19
closes with the rule this document is written to: *do not delete safety mechanisms because they
complicate the user experience.*

Evidence is pinned to the current head
[f0376be](https://github.com/maggnus/ymp/commit/f0376be), which includes the accepted provider and
catalog records of `W1-PRD-05b`
([09fc9c4](https://github.com/maggnus/ymp/commit/09fc9c4)). Node identifiers name work units in
[`work/STATUS.md`](../work/STATUS.md).

## Summary

The kernel is still not the problem, and the accounting, the verification plane and the isolation
boundary are still the parts of this product that need no redesign. Since the previous inspection
three of the leaks it found have been closed by accepted work: product state now lives under
`~/.ymp` (`W1-APP-02w.1`), the fixture runtime is neither linked nor offered (`W1-APP-02s`), and
providers, engines and a derived model catalog now exist as measured records under the root
(`W1-PRD-05b`).

What remains concentrates in one sentence: **the product still asks the operator for the things
brief v2 assigns to ymp, and still cannot start a collective.** The operator supplies the acceptance
condition and the verifier; the operator names which agent does the work; the operator sets the
roster in the document of record; the operator types a second command to start any work at all; and
after all of that exactly one participant can exist, with no way for it to recruit a second.

Sixteen leaks are recorded. Eleven are in the shipped code and the documents of record. Five are in
the **design set as it stood before this correction** — it asks the operator for the permitted model
set at request time, probes providers before they are enabled, draws a startup surface the owner had
already replaced, places the semantic work of derivation in a product module rather than in a
collective, and loads the operator with confirmations and typed identifiers. A design correction that
did not name its own leaks would be the least honest kind.

The sixteenth is of a different kind from the other fifteen and is stated separately for that
reason. The first fifteen are **ownership** leaks: work the brief assigns to ymp that the operator is
made to do. L-16 is an **operator-burden** leak: work that is genuinely the operator's, made heavier
than it needs to be by ceremony — a confirmation, an identifier typed back to the product, an
acknowledgement. The owner ruled on it directly on 2026-08-15, and the ten decisions of
[`COLLECTIVE-OWNER-DECISIONS.md`](COLLECTIVE-OWNER-DECISIONS.md) exist largely to remove it.

## 1. What already conforms

### 1.1 The kernel enforces without deciding

| Brief v2 clause | Mechanism in the repository |
|---|---|
| §1 the kernel checks mechanical constraints only | A command carries identifiers, digests, integers and deadlines; an intent reaches the kernel as a digest that is compared and never read ([`protocol.rs:1-6`](../../ymp-rust/crates/ymp-domain/src/commitment/protocol.rs)) |
| §11 the kernel does not decide semantic quality | There is no field a transition could consult to learn a skill, a model or a rank (same doc comment) |
| §12 recruitment consumes finite resources | Ten independently enforced dimensions with no conversion between them ([`budget.rs:39-50`](../../ymp-rust/crates/ymp-domain/src/commitment/budget.rs)) |
| §12 starting an agent to ask whether it wants work is not free | `InvocationStarts` is charged per process slice ([`budget.rs:29-33`](../../ymp-rust/crates/ymp-domain/src/commitment/budget.rs)) |
| §12 decomposition must terminate | Creation authority is irreversible once spent ([`budget.rs:54-64`](../../ymp-rust/crates/ymp-domain/src/commitment/budget.rs)) |

Accepted by nodes `W1-COR-03a`, `03b`, `03g`–`03s`.

### 1.2 Verification is already independent, exact and honest

- A verdict is bound to one exact candidate digest; a candidate that is not the recorded one is
  refused rather than judged — `CandidateMismatch`
  ([`protocol.rs:627-633`](../../ymp-rust/crates/ymp-domain/src/commitment/protocol.rs)).
- Acceptance evidence names contract, candidate, environment and oracle digests together
  ([`lib.rs:84-92`](../../ymp-rust/crates/ymp-domain/src/lib.rs)).
- A program that accepts a deliberately wrong candidate cannot enter a contract; the negative
  control is executed before anything is stored
  ([`answer.rs:9-13`](../../ymp-rust/crates/ymp-application/src/answer.rs), node `W1-APP-02u`).
- A verifier that delegates to a file inside the candidate is pinned to the approved bytes
  ([`answer.rs:15-21`](../../ymp-rust/crates/ymp-application/src/answer.rs), node `W1-APP-02z`).
- Five terminal states exist and quiescence is never acceptance
  ([`lib.rs:31-46`](../../ymp-rust/crates/ymp-domain/src/lib.rs)). A sixth,
  `needs_clarification`, is added by change C15 because decision D6 requires the outcome and
  forbids reusing `abstained`; the property that terminals are distinct and never substituted is
  what the addition preserves.

Brief v2 §10 lists exactly these as the mechanisms to preserve. They need a different author, not a
different design.

### 1.3 Provider, engine and catalog records exist

`W1-PRD-05b` stood the provider level above the engine records and derived the catalog from both:

- one record per provider holding the account, its observed state, the reason that state was read
  from, and one route per engine that reaches it
  ([`provider.rs:1-32`](../../ymp-rust/crates/ymp-runtime-registry/src/provider.rs));
- a catalog derived on read rather than stored a third time, so a re-measured list is never read
  through a stale copy ([`catalog.rs:10-14`](../../ymp-rust/crates/ymp-runtime-registry/src/catalog.rs));
- an entry that is not offered is present and carries its measured reason
  ([`catalog.rs:16-18`](../../ymp-rust/crates/ymp-runtime-registry/src/catalog.rs));
- nothing at this level instantiates anything or changes admission
  ([`catalog.rs:20-24`](../../ymp-rust/crates/ymp-runtime-registry/src/catalog.rs)).

This is precisely the `Provider → Model catalog` half of brief v2 Part A §1. Two review residues are
carried into migration unit P2 and are named in [`COLLECTIVE-MIGRATION.md`](COLLECTIVE-MIGRATION.md):
the stored provider state carries no observation age, and a deleted engine record is answered by the
seeded record so its models leave the catalog silently.

### 1.4 Launch admission, isolation and accounting

Every program in the launch chain is admitted by digest or by a location the run's own account
cannot write; ambient user, project and plugin configuration is excluded from a managed invocation;
runtime-native subagents are disabled rather than mapped; spend is attributed per model; no managed
descendant survives its supervisor (nodes `W1-APP-02p`, `02q`, `02r`, `02k`, `02l`, `02y`).

### 1.5 Composition, command parity and the product root

The main surface is a scrolling conversation with a `/` command line and full-screen pages; every
interface action exists as a command of the same executable; product state lives under `~/.ymp` and
the launch directory is written only by an explicit export (nodes `W1-APP-02e.2`, `02e.3`, `02n`,
`02w`, `02w.1`, `02w.3`).

---

## 2. Ownership leaks

Each leak names the brief clause it violates, the evidence with file and line, and the five things
Part A §19 requires. No leak is closed by removing a mechanism.

### L-01 · `PROJECT-CONTRACT.md` is an operator obligation

**Brief.** §19 names it first: *"PROJECT-CONTRACT.md требуется от пользователя"*. §1: the operator
must not create a contract or write acceptance criteria.

**Now.** The approved package has eleven required contents
([`PROJECT-CONTRACT.md:12-46`](../PROJECT-CONTRACT.md)); drafting is "repository analysis and a
structured interview" with a human reviewing "the semantics, environment, budget, private oracle
bundle, and observation policy"
([`PROJECT-CONTRACT.md:55-58`](../PROJECT-CONTRACT.md)), and the recommended path is repeated at
[`:64`](../PROJECT-CONTRACT.md). An amendment requires fresh human approval
([`:62`](../PROJECT-CONTRACT.md)).

**Stays.** All eleven contents, every digest, the approval record, the amendment rule, the
distinction between drafting, approval, execution and acceptance authorities.

**Owner.** The derivation run of the collective fills the package; the human approves a spend
against a restated goal. The interview disappears; the analysis does not.

**Transition.** `ContractDocument` gains `requirements[]` with a provenance class and
`acceptance_plan[]`; the package is stored as an internal object and referenced by digest from
`Run.spec.contractDigest` ([`COLLECTIVE-RESOURCES.md`](COLLECTIVE-RESOURCES.md), Run).

**Interface.** No surface asks for a package. The block that starts the run states the restated
goal, what *done* means in plain sentences, the ceiling it spends from and the disclosure; the eleven
contents live in diagnostics. It is information appended to the transcript, not a gate — decision D3
and leak L-16.

**Tests.** Design tests 1, 3, 12, 15: a contract complete without operator input, every requirement
classified, every check discriminating.

### L-02 · The acceptance condition is an operator input

**Brief.** §1: the operator must not create an oracle or a verifier; §10: the machinery stays, its
ownership moves inside.

**Now.** `RunRequest.acceptance` is an `Option`
([`contract.rs:44`](../../ymp-rust/crates/ymp-application/src/contract.rs)); the module comment
states the rule — "It never invents the acceptance condition: a request without one is refused with
the missing part named" ([`contract.rs:9`](../../ymp-rust/crates/ymp-application/src/contract.rs));
the refusal is the third thing `prepare_contract` does
([`contract.rs:183-194`](../../ymp-rust/crates/ymp-application/src/contract.rs)). The command line
takes the verifier and the negative control as arguments
([`surface.rs:185-190`](../../ymp-rust/crates/ymp-cli/src/surface.rs)).

**Stays.** The refusal itself, moved: a plan that cannot be shown to discriminate still cannot
decide a run. `MissingPart` survives for the fields the operator does supply.

**Owner.** Derivation, inside the product, funded and disclosed rather than free and silent.

**Transition.** `RunRequest` loses `acceptance`; `prepare_contract` consumes a derived acceptance
plan instead of validating a supplied one; `Task.spec` has no acceptance field at all.

**Interface.** The goal-entry surface never asks for a program, a control or a path. The advanced
amendment grammar survives as an override that is never required, offered or implied.

**Tests.** Design tests 1, 2, 15, and the negative half: a build in which any prompt for a verifier
is reachable from goal to spend.

### L-03 · The refusal path tells the operator to write a verifier

**Brief.** §1 names *"Создайте acceptance oracle"* as the question that must not be asked.

**Now.** Four refusals return the work to the operator by name:
"no test entry point was found … and this directory names none it can run"
([`answer.rs:108-112`](../../ymp-rust/crates/ymp-application/src/answer.rs)); "nothing is proposed —
state a verifier of your own instead"
([`answer.rs:113-118`](../../ymp-rust/crates/ymp-application/src/answer.rs)); the same instruction
for a script without execute permission
([`answer.rs:119-125`](../../ymp-rust/crates/ymp-application/src/answer.rs)); and again where the
request names no artifact ([`answer.rs:133-143`](../../ymp-rust/crates/ymp-application/src/answer.rs)).

**Stays.** Every refusal's *honesty*: the file that is the obstacle is still named, and a proposal
that could not hold the candidate to its own check is still not made.

**Owner.** Generation becomes the default source of the acceptance plan everywhere rather than the
fallback for projects with tests; node `W1-APP-02z.3` already moved in this direction and is
accepted. What cannot be derived is named as the operator's own semantic remainder, never as an
artifact to author.

**Transition.** `answer::generate` stops being a special case and becomes one generator among
several inside derivation; its refusals become statements of what the plan does not observe.

**Interface.** A goal stated in a project with no tests starts a run against a derived plan, with
the remainder named in the transcript — not a refusal.

**Tests.** Design test 1 with its negative half — today's refusal demanding a hand-written verifier —
and test 15.

### L-04 · The operator names which agent does the work

**Brief.** §19 names manual model assignment and runtime profiles exposed as user-level roles; §16
forbids hard-coded assumptions of the "Opus = architect" kind; §1 states that the kernel does not
decide which model is better and the operator does not assign one either.

**Now.** The routing module opens with the rule: "A run is done by one of the runtime profiles this
host can start … the operator names it, or exactly one managed profile is ready"
([`attempt.rs:1-7`](../../ymp-rust/crates/ymp-tui/src/attempt.rs)). Where two are ready the refusal
is explicit: "name the one this run uses with `runtime <name>`, because **which agent does the work
is yours to decide**" ([`attempt.rs:143-151`](../../ymp-rust/crates/ymp-tui/src/attempt.rs)). The
same argument exists on two commands
([`surface.rs:88-96`](../../ymp-rust/crates/ymp-cli/src/surface.rs),
[`surface.rs:191-193`](../../ymp-rust/crates/ymp-cli/src/surface.rs)). A driver profile still pins
one model as the only route it will serve
([`runtime-claude/lib.rs`](../../ymp-rust/crates/ymp-runtime-claude/src/lib.rs)).

**Stays.** Version and conformance pinning, which stop being exclusive and become the recorded
identity of one catalog entry among several. The refusal to substitute a different profile silently
stays: nothing is ever routed to a second entry in place of the one recorded.

**Owner.** The collective, which names a catalog entry when it recruits; the kernel checks
membership in the run's frozen pool and nothing else.

**Transition.** `RegisterParticipant` gains `requested_entry`; a new refusal `EntryNotPermitted`
names containment. The per-run `runtime` argument disappears from every public command.

**Interface.** `/models` shows entries and states permanently that nothing there is an agent;
`/agents` shows participants and their entries. No surface offers a profile picker before a run.

**Tests.** Design tests 5, 6, 7, 9, 11 — a hundred entries produce one participant; a request
outside the pool is refused and creates nothing; two requests differing only in goal text receive
identical kernel decisions.

### L-05 · Provider configuration is the same act as choosing who works

**Brief.** §19: *"provider configuration смешан с participant creation"*; §17 separates provider,
pool, collective and agent as four different questions.

**Now.** The terminal interface links the engine crates and constructs drivers itself
([`ymp-tui/Cargo.toml:17-21`](../../ymp-rust/crates/ymp-tui/Cargo.toml),
[`attempt.rs:23-30`](../../ymp-rust/crates/ymp-tui/src/attempt.rs)); readiness of a *profile* is
what decides whether work can start ([`runtimes.rs:75-79`](../../ymp-rust/crates/ymp-tui/src/runtimes.rs));
and authorization is blocked with "no runtime profile on this host would do this work"
([`decisions.rs:27-29`](../../ymp-rust/crates/ymp-tui/src/decisions.rs)). Configuring an engine and
deciding who does the work are one act on one page.

**Stays.** The registry's enabled flag as the single place an engine is refused; probing; the
refusal-rather-than-degradation rule of node `W1-APP-02q`.

**Owner.** The provider observer and the pool reconciler own configuration; the run launcher owns
process creation. The interface reads records through an application port and constructs no driver.

**Transition.** A catalog read port in `ymp-application`; driver construction moves to the
composition crate; `ymp-tui` drops its runtime dependencies.

**Interface.** `/providers`, `/models` and `/pools` are configuration surfaces and create nothing;
`/agents` is a runtime surface and configures nothing. A goal is never refused because of a
*profile* — it is refused by an empty pool, which is a different sentence with a different fix, and
which names `/providers` rather than a runtime.

**Tests.** Design test 22 extended: the shipped interface links no runtime crate and offers no
profile it cannot start.

### L-06 · The acceptance machinery is the operator's workflow step

**Brief.** §19: *"acceptance machinery вынесена в operator workflow"*; §1: no manual review, no
manual verification.

**Now.** The authorization decision is a requirement-to-evidence coverage map modelled in state
([`state.rs:154-171`](../../ymp-rust/crates/ymp-tui/src/state.rs)), built as such
([`decisions.rs:56-62`](../../ymp-rust/crates/ymp-tui/src/decisions.rs)), with blocking items that
disable the action ([`decisions.rs:15-21`](../../ymp-rust/crates/ymp-tui/src/decisions.rs)) and
described in the document of record as "the most consequential screen in the product"
([`VISUAL_CONCEPT.md:87-99`](../VISUAL_CONCEPT.md)).

**Stays.** The coverage map in full, as an on-demand diagnostic — it is genuinely the best evidence
about a derived plan — and the rule that a plan nothing could reject cannot start a run.

**Owner.** The product decides plan quality and refuses on its own account; the operator decides a
spend.

**Transition.** The `Authorize` modal retires entirely (decision D3). The restated goal, the
ceiling, the pool, the disclosure and the assurance profile are carried by the run-start block in the
transcript; `requirements[]` moves to the diagnostics projection.

**Interface.** No screen stands between the goal and the collective. The map is one keystroke away
under diagnostics and is never a gate — and, per L-16, there is nothing to confirm.

**Tests.** Design test 2 and the reachability check that the map is still complete.

### L-07 · The operator assembles the roster

**Brief.** §19 names manual team assembly; §1 forbids assigning the number of agents; §12: the
number of agents is decided by the collective within mechanical limits.

**Now.** The document of record states the opposite: "The operator sets the starting roster of
participants and a ceiling on how many more may be recruited, before authorization"
([`VISUAL_CONCEPT.md:117-123`](../VISUAL_CONCEPT.md)).

**Stays.** The ceiling, as a mechanical bound in the pool's capacity and the run's budget vector;
the visibility of remaining funded starts.

**Owner.** The collective decides the roster; the operator's only related act is editing a pool's
capacity, which is a boundary and not a team.

**Transition.** No domain change — the ceiling already exists as `ParticipantStarts`. The document
of record's sentence is replaced; the design records the required wording change.

**Interface.** No roster surface exists at any point between the goal and the collective.
`/agents` shows what the collective made.

**Tests.** Design test 4 — a goal that starts a run starts exactly one participant, and its negative
half is any build in which a number is asked for.

### L-08 · The permitted model set is asked for at request time

**Brief.** §4: the operator must never be forced to create a pool to start work; §16: a pool is a
capability boundary, not a team; §18: the effective pool is frozen internally at task creation and
the operator is not made to build that snapshot.

**Now — in the current design set, not in the code.** Owner decision D1 as recorded states that "for
a task the operator names the permitted set of concrete models/agents … and, when needed, their
count/limit" and that "the per-task permitted list is operator input at request time"
([`COLLECTIVE-OWNER-DECISIONS.md:22-24` at f0376be](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-OWNER-DECISIONS.md#L22-L24),
[`:40-41`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-OWNER-DECISIONS.md#L40-L41)). The design carried it into the run policy as a
field the operator may narrow ([`COLLECTIVE-DESIGN.md:255-278`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-DESIGN.md#L255-L278))
and the interface drew it as a policy row before a run
([`COLLECTIVE-TUI.md:239-273`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-TUI.md#L239-L273)).

**Stays.** The freeze, the digest, the reproducibility of a matched-budget comparison, and the fact
that a run states exactly what it could have used.

**Owner.** The `AgentPool` resource with an automatic `default`, created once a provider is enabled
and its models discovered. `Task.spec.agentPool` defaults to `default` and is never asked for. D1's
own sentence — "no separate operator-facing model pool entity" — is superseded by brief v2, which is
already annotated on the decision.

**Transition.** `AgentPool` as a stored resource with `spec.models` (selector or explicit ordered
list) and `spec.capacity`; `Run.spec.poolSnapshot` as the frozen value; `PoolFrozen` as the fact.

**Interface.** Nothing about models appears between the goal and the collective. `/pools` exists
for the advanced operator and is never a prerequisite.

**Tests.** Design tests 32 and 33: after enabling one provider, a goal typed immediately starts a run
against a non-empty pool with no question about models. Negative half: `No AgentPool configured`, or
any prompt for a permitted set.

### L-09 · No participant is started, and starting one is a second operator act

**Brief.** §9 and §22: after the goal, `Task created · Collective starting…`, and the operator never
creates an agent.

**Now.** Storing the contract is where the path stops: "nothing is being done yet · `/attempt
{run_id}` starts the {profile} profile on this run, which is where spending against your own account
begins" ([`app.rs:800-820`](../../ymp-rust/crates/ymp-tui/src/app.rs)). Starting work is a separate
public command with its own typed confirmation
([`surface.rs:88-96`](../../ymp-rust/crates/ymp-cli/src/surface.rs)).

**Stays.** The separation of authorities the two-step shape was protecting — creating a run and
spending against a provider account are different grants — expressed as two facts committed in one
sequence rather than as two operator acts.

**Owner.** The run launcher: as soon as the run exists it freezes the pool, registers the origin
participant, funds it and starts one invocation, following committed facts and deciding nothing.

**Transition.** A composition-level launcher above the application core and the supervisor; the
`Attempt` command becomes an internal consequence rather than a public step.

**Interface.** The collective-startup surface replaces the "nothing is being done yet" line with the
four facts of a collective being created.

**Tests.** Design test 4 and its negative half: zero participants, which is today's behaviour.

### L-10 · A participant cannot recruit, delegate or speak

**Brief.** §12 dynamic recruitment; §14 findings, submissions, recruitment visible per participant.

**Now.** The tool surface is four tools — read control, read events, submit, yield
([`agent-api/lib.rs:7-10`](../../ymp-rust/crates/ymp-agent-api/src/lib.rs)). `RegisterParticipant`
carries participant, principal, sponsor and endowment and no statement of what the new participant
would run on ([`protocol.rs:30-36`](../../ymp-rust/crates/ymp-domain/src/commitment/protocol.rs)),
so the mechanical checks §12 lists have no subject.

**Stays.** Everything: the commands are already specified in
[`PROTOCOL.md`](../PROTOCOL.md) and the ledger already carries them.

**Owner.** The collective. Projection is implementation of an accepted specification, not new
design.

**Transition.** `requested_entry` on `RegisterParticipant`; the tool projection extended to the
accepted families — observe, communicate, contract, lifecycle, artifact, verification.

**Interface.** `/activity` and `/agents` show recruitment as it happens, with the mechanical reason
the request was admitted.

**Tests.** Design tests 6, 7, 8.

### L-11 · A run admits one active attempt

**Brief.** §9 and §22: two participants work concurrently, one implements while another reviews.

**Now.** "the POC profile admits only one active attempt"
([`supervisor/lib.rs:935`](../../ymp-rust/crates/ymp-runtime-supervisor/src/lib.rs)).

**Stays.** The isolation rule the constant protects: concurrent attempts never share a writable
copy.

**Owner.** The pool's `capacity.maxConcurrentAttempts` and the run's budget vector.

**Transition.** The constant becomes a policy ceiling checked from the vector and the concurrency
limit.

**Interface.** The budgets surface shows the ceiling and what is left of it.

**Tests.** Concurrent sandboxes proved isolated (node `W1-COR-03d`), and a refusal that names the
dimension rather than a constant.

### L-12 · Configuration names the origin participant

**Brief.** §11: no hidden manager; §22: the collective starts one participant and the operator
assigns nothing.

**Now.** "Every run needs an initial condition. Configuration names an **origin participant** and
gives it the root contract and a finite root branch budget"
([`ARCHITECTURE.md:227-228`](../ARCHITECTURE.md)); "The kernel creates the root obligation and
starts the configured origin participant" ([`CONCEPT.md:245-246`](../CONCEPT.md)).

**Stays.** The concept and its guard — an ignition point, not a permanent orchestrator
([`CONCEPT.md:122`](../CONCEPT.md)) — and the fact that the kernel infers no organization from it.

**Owner.** The run launcher derives the entry from the frozen pool by a mechanical rule instead of
reading it from configuration: the first ready entry in the pool's declared order
([decision D2](COLLECTIVE-OWNER-DECISIONS.md#d2--the-entry-rule-for-the-origin-participant), decided
2026-08-15). No operator setting and no question.

**Transition.** `Run.spec.entryModel`, recorded in the `PoolFrozen` fact so evidence names it and a
replayed journal reproduces it.

**Interface.** The collective-startup surface states the rule in one line, so the entry is never
mistaken for a judgement about the goal.

**Tests.** Design tests 9 and 11 — a kernel that orders pool entries by any property other than the
declared order fails; two requests differing only in goal text decide identically.

### L-13 · Providers are probed before they are enabled

**Brief.** §7: *"Provider НЕ должен autodetect'иться до явного включения пользователем"*; the fixed
supported list is shown with every provider `disabled` until the operator enables one.

**Now — design and code.** The design has startup "probes provider readiness in the background"
([`COLLECTIVE-DESIGN.md:164-168` at f0376be](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-DESIGN.md#L164-L168)) and the startup
surface drew `providers probing… (3 configured)`
([`COLLECTIVE-TUI.md:92-97`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-TUI.md#L92-L97)). In the code the
engine probe runs the engine executable on a worker thread as a matter of course
([`runtimes.rs:13-17`](../../ymp-rust/crates/ymp-tui/src/runtimes.rs)).

**Stays.** Probing itself, and the rule that a measured property is never a declaration. The
accepted provider record is already observed rather than seeded
([`provider.rs:23-32`](../../ymp-rust/crates/ymp-runtime-registry/src/provider.rs)), which is the
correct half of this.

**Owner.** The provider observer, gated on `spec.enabled`.

**Transition.** Probing is triggered by the enable transition and by an explicit refresh, never by
startup.

**Interface.** `/providers` lists the fixed supported set with `disabled` and an em dash for models;
startup states no provider count at all.

**Tests.** A new design test: with no provider enabled, startup opens no process and no network
connection. Negative half: a probe that runs on launch.

### L-14 · The startup surface contradicts the accepted one

**Brief.** §5: logo, basic information, the current directory, the input line, and nothing else;
§6: no conversational setup wizard.

**Now.** The design's startup draws four lines including the full assurance sentence
([`COLLECTIVE-TUI.md:92-99` at f0376be](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-TUI.md#L92-L99)), while node `W1-APP-02e.5` is
accepted with exactly
the logo, one line of basics (current directory, application version) and the request invitation,
the full assurance text living in `?` and `/runtimes`.

**Stays.** The rule that the assurance profile never appears without its limit — it appears where it
is load-bearing, not at launch.

**Owner.** The accepted screen contract; the design conforms to it rather than replacing it.

**Transition.** None; this is an interface correction.

**Interface.** Startup is the logo, one line, and the prompt, shown once.

**Tests.** The deterministic 80×24 and 120×40 screen tests of node `W1-APP-02e.1`.

### L-15 · The search for a hidden manager, and what it found

**Brief.** §11: no hidden manager agent that becomes a central planner; §19: *"hidden
manager/orchestrator фактически принимает semantic decisions"*.

**Now — the repository is clean and the design is not.** The documents of record are explicit that
no planner exists ([`ARCHITECTURE.md:16`](../ARCHITECTURE.md),
[`README.md:36`](../README.md)), and nothing in the crates ranks participants or assigns work. Three
candidates exist and each is named rather than waved past:

1. **Derivation as a product module.** The current design puts the reading of the goal, the
   production of observable requirements and the generation of the acceptance plan in an
   application module ([`COLLECTIVE-DESIGN.md:511-560` at f0376be](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-DESIGN.md#L511-L560),
   [`:786-790`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-DESIGN.md#L786-L790)). That is a semantic decision taken by the product
   rather
   than by the collective, which §11 reserves.
2. **The divergence classifier.** The same design has derivation judge whether an operator's answer
   contradicts a recorded requirement
   ([`COLLECTIVE-DESIGN.md:602-620` at f0376be](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-DESIGN.md#L602-L620)) —
   again a semantic judgement in a product component.
3. **The generated check's fixed rule.** `answer::generate` reads a production verb and the artifact
   it names out of the request's own words
   ([`answer.rs:29-42`](../../ymp-rust/crates/ymp-application/src/answer.rs)). It is a stated,
   bounded lexical rule rather than a planner, but it does read the goal.

**Stays.** Every mechanism in all three: the two-run boundary that keeps a protected plan away from
the participants judged by it, the conservative `undecided → divergent` threshold, the discrimination
gate, and the honesty of the generated check about what it does not decide.

**Owner.** Candidates 1 and 2 move into a **derivation run of the collective** — a separate scope
with its own budget, audience and capabilities, which is the arrangement
[`VISUAL_CONCEPT.md:125-141`](../VISUAL_CONCEPT.md) already accepted. Candidate 3 stays a mechanical
generator inside that run's toolkit, and its rule stays stated.

**Transition.** The application layer keeps only a **derivation sequencer**: it advances phases when
facts say a phase finished and creates nothing semantic. The classifier's output is a fact; the hold
on the acceptance path it triggers is enforced by the kernel, not by a screen.

**Interface.** Nothing changes for the operator, which is the point: the ownership move is invisible
except in diagnostics, where the derivation run is readable like any other.

**Tests.** Design tests 27, 28 and a new one: no component outside a collective run reads the goal
text, and the sequencer's decisions are a pure function of committed facts.

### L-16 · Operator burden: confirmations, typed identifiers, acknowledgements

**Brief and owner.** v2 §23 draws the whole product as `$ ymp` → one sentence → `✓ Verified result`;
§6 forbids a conversational setup wizard. The owner ruled explicitly on 2026-08-15, after the
previous command-line release: *resolve as many questions as possible by the application's logic —
no confirmation dialogs, no identifiers typed by the operator, no acknowledgement steps, no setup
wizard.* Decisions D3, D4 and D10 are that ruling applied.

**Now — in the code.** The command surface makes a typed identifier the mechanism of every
irreversible act: "an irreversible action is committed only from the interface's own confirmation,
and only while that confirmation reports the typed identifier as exact"
([`surface.rs:23-24`](../../ymp-rust/crates/ymp-cli/src/surface.rs)). Three public commands carry
it — authorize ([`surface.rs:78-84`](../../ymp-rust/crates/ymp-cli/src/surface.rs)), start the agent
([`:88-92`](../../ymp-rust/crates/ymp-cli/src/surface.rs)) and cancel
([`:97-101`](../../ymp-rust/crates/ymp-cli/src/surface.rs)) — and the interface mirrors them
([`app.rs:95`](../../ymp-rust/crates/ymp-tui/src/app.rs),
[`:491`](../../ymp-rust/crates/ymp-tui/src/app.rs),
[`:1980`](../../ymp-rust/crates/ymp-tui/src/app.rs)). The document of record requires the typed
cancel identifier as a rule ([`VISUAL_CONCEPT.md:257-260`](../VISUAL_CONCEPT.md)).

**Now — in the design set before this correction.** A setup question after the first provider
([`COLLECTIVE-TUI.md:147` at f0376be](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-TUI.md#L147)),
an authorization surface with a typed run identifier
([`:415`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-TUI.md#L415),
[`:428`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-TUI.md#L428)), a typed
cancel identifier ([`:690`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-TUI.md#L690)),
and modal acknowledgements for pause and export
([`:661`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-TUI.md#L661),
[`:769`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-TUI.md#L769)).

**Stays.** Everything the ceremony was carrying: the consequences of a cancellation, the exact paths
an export writes, the disclosure a provider enable permits, what *done* will be judged by, and the
distinction between an act with a spend and an act without one. **What changes is where they are
said** — on the row, above the key, before the act — rather than in a dialogue after it.

**Owner.** The application, by decision. A standing per-workspace ceiling replaces the per-run
authorization (D3); enabling a provider *is* the disclosure consent, stated on its properties view
(D4); `Continue` starts at once (D10); the semantic remainder is named rather than signed off (D5);
and the diagnostic policy is one conservative default rather than a setting (D7).

**Transition.** The `--confirm <ID>` argument retires from every public command; the equivalent
command takes its subject from the open run or from a named row, and the parity rule of node
`W1-APP-02n` is restated as *every action exists as a command whose subject is a selection*. The
`Authorize` modal and its coverage map retire into diagnostics.

**Interface.** Four surfaces disappear and nothing replaces them: the setup question, the
authorization screen, the typed cancel confirmation and the continue confirmation. Pause, cancel,
export and archive become keys on a selected row with their consequences stated above the key
([`COLLECTIVE-TUI.md`](COLLECTIVE-TUI.md) rules 1–2, surfaces S21, S23, S27, S29).

**Tests.** New: no operator input field in the product accepts a run identifier, a candidate digest,
a participant identifier or a workspace hash; a goal typed with a ready pool starts a run with no
intervening screen; the §22 walk completes in five operator acts. *Negative half:* any dialogue
between the goal and the collective, or any act that requires typing back a value the product
already knows.

**One trade recorded rather than hidden.** Cancel and export stay irreversible and now take one
keypress. The design keeps them defensible by placing the key on the object's properties view rather
than on a list row, by stating the exact consequence above it, and by the fact that nothing is
destroyed — journals, candidates and evidence survive every act. An accidental keypress costs the
work in flight; that is the cost of the rule, and it is the owner's call, taken deliberately.

---

## 3. Absent rather than contradictory

Unbuilt parts of an accepted specification that the corrected design depends on, listed so it does
not appear to invent them.

| Item | Specified in | Node |
|---|---|---|
| Scoped collaboration board with attribution and no authority | [`PROTOCOL.md`](../PROTOCOL.md) | `W1-COR-03c` (open) |
| Competing submissions with immutable candidate ancestry | [`PROTOCOL.md`](../PROTOCOL.md) | `W1-COR-03d` (open) |
| Interface surfaces for local commitments and communication | [`ARCHITECTURE.md`](../ARCHITECTURE.md) | `W1-COR-03e` (open) |
| Commitment facts durable rather than in memory | — | `W1-COR-03f` (active, and a precondition) |
| The npm entry point pinned or refused | — | `W1-APP-02z.1` (open) |

Four nodes the previous inspection listed here are now accepted and are consumed rather than
awaited: `W1-APP-02e.6` (engine registry), `W1-APP-02w.1` (product root), `W1-APP-02z.3` (generated
check), `W1-EVL-04d` (closed by owner decision D1).

## 4. Required changes by layer

### 4.1 Ownership

| Moves from | Moves to | Closes |
|---|---|---|
| Operator authors the acceptance condition and the verifier | A derivation run of the collective, funded and disclosed | L-01, L-02, L-03 |
| Operator names which agent does the work | The collective names a catalog entry; the kernel checks membership | L-04 |
| Operator configures a profile and thereby chooses the worker | Provider and pool configuration; a separate launcher creates participants | L-05 |
| Operator reviews a coverage map to authorize | Nothing: the step retires. The goal sentence states what is wanted and the workspace's standing ceiling bounds it | L-06, L-16 |
| Operator sets the roster and the permitted set | An automatic `default` AgentPool; the collective decides its own size within capacity | L-07, L-08 |
| Operator issues a second command to start work | The launcher bootstraps as soon as the run exists | L-09 |
| Configuration names the origin participant | A mechanical rule over the frozen pool | L-12 |
| Operator authorizes each run by typing its identifier | A standing per-workspace ceiling; the goal sentence starts the run | L-16 |
| Operator confirms, acknowledges and re-types what the product knows | Consequences stated on the row above the key | L-16 |

### 4.2 State and domain

- `AgentPool` as a stored resource with a selector or explicit ordered list, capacity and limits;
  an automatic `default` created on the first ready provider (L-08).
- `Run.spec.poolSnapshot` and `Run.spec.entryModel`, committed as `PoolFrozen` (L-08, L-12).
- `ContractDocument` gains requirements with provenance classes A–E and an acceptance plan naming
  which check observes which requirement (L-01, L-02).
- `Clarification` as a typed object with its own budget dimension, and the divergence fact with its
  operator resolution (L-15).
- `RegisterParticipant` gains `requested_entry`; the kernel gains one refusal, `EntryNotPermitted`,
  and one kind of check — containment over committed facts (L-04, L-10).
- `RunState.budget` becomes a projection of the root account rather than a second accounting.
- The single-active-attempt constant becomes a policy ceiling (L-11).

### 4.3 API and crates

- The participant tool surface grows from four tools to the accepted families (L-10).
- A composition crate holds the run launcher and the catalog watcher, so the interface can start a
  collective without linking a runtime (L-05, L-09).
- Provider probing is gated on the enabled flag (L-13).

### 4.4 Interface

- `/providers`, `/models`, `/pools` as tables first, k9s style; `/agents`, `/tasks`, `/activity`,
  `/budget`, `/verify`, `/result`, `/help` as the operator's surfaces (brief §13).
- Startup is the logo, one line and the prompt (L-14).
- The authorization screen disappears; the coverage map becomes diagnostics; the standing ceiling
  lives on `/budget` (L-06, L-16).
- No surface between the goal and the collective mentions a model, a team, a check or a confirmation
  (L-04, L-07, L-08, L-16).
- No input field anywhere accepts an identifier the product already knows (L-16).

## 5. What must not change

Recorded here because §19's closing rule is easiest to violate while closing L-01, L-02 and L-06.
Each mechanism is named with where its ownership now sits.

| Mechanism | Kept as | Owned by |
|---|---|---|
| An approved definition of done | The internal contract, bound once per run | derivation run; approved as a spend by the operator |
| Discrimination gate | A plan that accepts the negative control or a substituted entry point cannot decide a run | derivation run, before the run is created |
| Exact digests | Contract, candidate, environment, oracle named together in every acceptance claim | kernel |
| Protected material | Never enters the collaboration plane; the producing attempt never writes verifier state | kernel and verifier boundary |
| Finite resources | Every participant, attempt, invocation, offer and obligation reserves a dimension | kernel |
| Escrow and proposal allowance | Recruitment is never free | kernel |
| Isolation | Concurrent attempts never share a writable copy | supervisor |
| Disclosure | Enabling a provider permits repository content to reach it, stated where it is decided | provider record and run policy |
| Candidate immutability | A candidate is a fact; a verdict binds one exact digest | kernel |
| `infrastructure_error` ≠ rejection | Its own state, never counted against a candidate | verifier controller |
| Bounded verification budget | `VerificationQueries` reserved per query | kernel |
| Six distinct terminals | Quiescence is never acceptance; `exhausted` is never drawn as success; `needs_clarification` is its own word and never `abstained` | kernel and every surface |

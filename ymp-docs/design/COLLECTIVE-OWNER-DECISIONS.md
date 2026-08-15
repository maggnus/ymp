# Decisions that require the owner

Item 24 of the design deliverable: the decisions this design deliberately does not take, because
each trades one thing the owner values against another and no evidence in the repository settles
it. Every entry states the question, why it cannot be decided from the brief or the code, the
options with their consequences, and **the assumption the design proceeds under while the answer is
outstanding** — so that no implementation node is blocked waiting for an answer.

Decisions that would merely be work are not here. These ten change what the product is.

Source links are pinned to
[dfdac03](https://github.com/maggnus/ymp/commit/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf).

---

## D1 · Where the permitted model set is declared

> **DECIDED by the owner, 2026-08-15 — option (b) with a product model refinement.**
> The operator does not manage a "model pool" and assigns no roles. For a task the operator names
> the permitted set of concrete models/agents (e.g. Opus 5, GLM, Fable) and, when needed, their
> count/limit. The provider only supplies the catalog of available models. When the run starts,
> that set is frozen for the run. From there the collective decides on its own: how many
> participants to create, which permitted model to recruit, when to recruit another, which models
> to use in parallel or in sequence, and when to stop.
>
>     Provider  -> many available models
>     Operator  -> the models permitted for this task
>     Collective -> forms the team from that set by itself
>
> 100 models at a provider never mean 100 agents; permitting Opus 5 + GLM + Fable means only that
> the collective may use those three. Actual participant count and distribution is the
> collective's, within budget and limits. Comparative experiments use the snapshot of the set taken
> at run start, so both arms have the same possibilities.
>
> Important: no separate operator-facing "model pool" entity. For the operator it is simply the
> task's list of permitted models; inside the system it may be a frozen snapshot.
>
> Consequence for the design: the registry (accepted W1-APP-02e.6) and the provider/catalog level
> (P1) supply what exists; the per-task permitted list is operator input at request time and is
> frozen into the internal contract by digest at authorization (P3); the deferred W1-EVL-04d is
> closed by this decision.
>
> **Refined by product brief v2, 2026-08-15** ([PRODUCT-BRIEF-collective-v2.md](PRODUCT-BRIEF-collective-v2.md),
> Part A §1, §4, §16–18 and Part B): the permitted set is now the `AgentPool` resource — a
> capability/resource boundary (allowed models, limits, min/max bounds), never a team or a role
> assignment. A useful `default` pool is created automatically once a provider is enabled and its
> models discovered, so the operator is never asked to create a pool before working; an advanced
> operator may edit `default` or add pools later via `/pools`. The run-scoped freeze stays an
> internal snapshot/digest in run provenance. The sentence "no separate operator-facing model pool
> entity" above is superseded to this extent; the rest of D1 stands.


**Question.** A run may recruit only from a permitted pool. Where is that pool declared: in the
engine registry, in the internal contract at the first request, or above both?

**Why the owner.** This is the deferred node `W1-EVL-04d`, whose own text records that engines are
probed for everything they can serve but what may be *used* must be declared somewhere, and that
the answer depends on how pools form. It trades reproducibility of the matched-budget comparison
(`W1-EVL-04a`) against operating convenience, and it decides whether per-model spend
(`W1-APP-02y`) is attributable to a stable set.

**Options.**

| | Mechanism | Consequence |
|---|---|---|
| **a** | Registry only: the enabled entries are the pool for every run | Simplest to hold in the head; a provider enabled mid-experiment silently changes what a later arm could recruit, which breaks matched-budget comparability |
| **b** | Frozen per run: the pool is the snapshot of enabled, ready, conformant entries taken at authorization and recorded in the internal contract by digest | Every run states exactly what it could have used; two arms are comparable by construction; costs one stored digest and the need to explain that enabling a provider affects the *next* run |
| **c** | A charter level above both, with per-run narrowing | Most expressive; adds a configuration concept the operator must learn, which the brief's §14 warns against |

**What the design assumes.** Option **b**, built on the registry of option **a**: the registry
declares what exists and is enabled, and the run freezes its pool at authorization. The pool digest
appears in the internal contract and in the evidence bundle. This is a recommendation with the
mechanism described in [`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) item 7, not a closure of
`W1-EVL-04d`, which must record its own decision.

## D2 · The entry rule for the origin participant

> **DECIDED 2026-08-15 — option (a).** Resolved by the CTO under the owner's instruction of the same
> day ("resolve as many as possible by the application's logic; the last CLI release was a
> nightmare of identifier entry and confirmations") and brief v2 Part A §9/§22 ("Collective starts
> one participant"). The origin participant runs on the first ready entry of the run's frozen
> AgentPool in the pool's declared order (for the automatic `default` pool: catalog order of the
> enabled providers). The kernel reads nothing about the goal; the ignition entry is recorded in the
> pool-freeze fact. No operator setting, no question. Option (b) is reserved for a later `/pools`
> edit if the surprise turns out to be real.

**Question.** Bootstrap starts exactly one participant. Something must decide which catalog entry
it runs on, at a moment when no collective exists to decide it. What is that rule?

**Why the owner.** Every mechanical rule here is defensible and none is neutral. A rule that read
the goal would be the semantic router brief §9 forbids; a rule that asks the operator would be the
model assignment brief §1 forbids. What is left is an ordering, and who owns the ordering is a
product decision.

**Options.**

| | Rule | Consequence |
|---|---|---|
| **a** | First ready entry of the pool in declared (registry) order | No new concept; the order is whatever enabling produced, which may surprise an operator who did not think of enabling as ranking |
| **b** | The operator sets a default entry per workspace, defaulting to **a** | Explicit and predictable; introduces one setting that looks like a model assignment even though it only names the ignition point |
| **c** | Ask once at first run and remember | Honest, but the first thing a new operator meets is a model question — precisely the shape brief §1 rules out |
| **d** | Round robin across ready entries | Balances usage across providers and varies the ignition entry between otherwise identical runs, which is a comparability question for the study design rather than a reproducibility one — see below |

**Whichever rule is chosen, the entry that ignited the run is recorded.** It is part of the
pool-freeze fact committed at authorization ([`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) item 8),
so a run's evidence always names what it started on and a replayed journal reaches the same
participant. That record removes the *reproducibility* objection to option **d**: a round-robin run
is reproducible from its own journal. What it does not remove is the *comparability* cost — two
runs of a matched-budget arm would ignite on different entries unless the arm pins the rule — so
option **d** remains a choice about how `W1-EVL-04a` is designed rather than a free one.

**What the design assumes.** Option **a**, with the rule stated on the bootstrap surface so it is
never mistaken for a judgement about the goal, and the chosen entry recorded either way. Option
**b** is a small addition if the surprise in **a** turns out to be real.

## D3 · Whether the single authorization stays mandatory

> **DECIDED 2026-08-15 — option (b).** A standing ceiling per workspace (defaults from D8, editable
> on `/budget`) is the human-approved bound; the goal statement itself is the approval of *what*.
> A goal starts immediately — "Task created · Collective starting…" — with no confirmation screen,
> no identifier to type, nothing to acknowledge. Re-authorization is asked only when the standing
> ceiling is reached (terminal BUDGET EXHAUSTED offers "raise ceiling and Continue"). Rationale:
> brief v2 Part A §6/§9/§23 and the owner's explicit rejection of confirmation-heavy CLI flows.

**Question.** The design keeps exactly one confirmation between a goal and the **run's** budget.
Does that confirmation stay mandatory for every run? (Derivation spends earlier, from its own
allowance; that is decision 4 and decision 8, not this one.)

**Why the owner.** It is the point where two of the owner's own rules meet. Constitutional
constraint 1 in
[`CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/CONCEPT.md#L163-L171)
requires a human-approved definition of done; brief §16 asks for "give the collective a goal, get a
verified result" and §14 names mandatory acceptance mechanics as an ownership leak. A confirmation
that shows a restated goal and a spend ceiling is not authorship — but it is still a step.

**Options.**

| | Rule | Consequence |
|---|---|---|
| **a** | One confirmation per run, always | The approval is unambiguous and the spend is never a surprise; the second and tenth goal of a session each cost a keystroke |
| **b** | One authorization per workspace establishes a standing ceiling; goals start immediately until it is reached, then re-authorize | Closest to "state a goal and get a result"; the human-approved definition of done becomes the goal statement itself, with the ceiling approved earlier |
| **c** | Threshold: below a stated estimated spend, start without confirmation | Reads well and depends on an estimate the product cannot make honestly before a run exists |

**What the design assumes.** Option **a**, because it is the only one that keeps the approval and
the spend in the same act and because it is what the accepted composition already implements. If
the owner prefers **b**, the surface changes and no mechanism does: the ceiling moves from the run
to the workspace and the goal statement carries the approval.

## D4 · The disclosure default

> **DECIDED 2026-08-15 — option (a) for the run and (ii) for derivation, with the consequence
> stated at the point of enabling.** Enabling a provider is the operator's one deliberate act
> (brief v2 Part A §7: no autodetect before explicit Enable); the provider detail view states, before
> the Enable action, that enabling permits repository content of any workspace to be sent to that
> provider, including bounded excerpts used to derive what "done" means before a run starts. No
> per-workspace and no per-run question. Optional narrowing stays available as an explicit policy
> (Part A: "при необходимости явные ограничения/policy") — a workspace may exclude a provider — but
> it is never asked. Flagged to the owner as the one decision with a confidentiality consequence.

**Question.** Enabling a provider permits repository content to be sent to it. Is that permission
given once at the provider level, per workspace, or confirmed per run?

**Why the owner.** Host allowlisting is not a confidentiality control
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L562-L568)),
so this is a real disclosure decision, and its default decides whether a private repository can
reach an external provider without a deliberate act.

**Options.**

| | Rule | Consequence |
|---|---|---|
| **a** | Provider level: enabling implies disclosure everywhere | One decision, easy to forget by the time a sensitive project is opened |
| **b** | Per workspace: enabling a provider is global, permitting disclosure is per project | One extra decision per project, at the moment the project is first used; matches how an engineer thinks about "may this code leave" |
| **c** | Per run: the authorization surface lists providers and requires acknowledgement | Most explicit; makes every run carry a decision the operator has already taken |

**The pre-authorization half of the same question.** Deriving what *done* means needs a model, so
bounded excerpts of the workspace reach one provider *before* any run is authorized
([`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) item 16). That is a disclosure the operator cannot
consent to per run, because it happens before the per-run decision exists. The design therefore
asks once, at the moment the first provider is connected, and records the answer as workspace
policy: which provider derivation may disclose to, or none.

| | Rule | Consequence |
|---|---|---|
| **i** | One question at first run, recorded as workspace policy, restated on the authorization surface as an accomplished fact | Consent precedes the disclosure and is asked once; the operator meets one setup question they did not ask for |
| **ii** | Derivation may disclose to any enabled provider without a separate answer | No setup question at all; repository content leaves the host on the strength of a decision the operator took about running agents, not about deriving requirements |
| **iii** | Derivation discloses nothing and requires the operator to state the acceptance condition | No pre-authorization disclosure; returns exactly the authorship the brief removes, so it is not a real option unless the owner accepts that trade |

**What the design assumes.** Option **b** for the run, and option **i** for derivation: one setup
question at first run (surface S02), the provider list repeated on the authorization surface as a
statement rather than a question, and the `already` row stating what derivation spent and disclosed
before the operator decided. Surface S03 states the consequence at the point of enabling either
way.

## D5 · The semantic remainder

> **DECIDED 2026-08-15 — option (a).** `VERIFIED` covers the mechanically observed requirements;
> the unobserved remainder is named on the same result line and in the evidence, never as a footnote
> and never as a question to the operator. No sign-off step (option c) — that would be a confirmation.

**Question.** Part of a goal is often not mechanically checkable — "clean", "idiomatic",
"beautiful". May a run be reported as `accepted` when the mechanically observable part passed and a
named part was never observed?

**Why the owner.** `accepted` is oracle-relative by definition
([`CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/CONCEPT.md#L36-L40)),
and node `W1-APP-02z.3` already records that the semantic remainder is named as the operator's own.
What is undecided is whether naming it is enough to call the run accepted.

**Options.**

| | Rule | Consequence |
|---|---|---|
| **a** | `accepted` covers the observed requirements; the remainder is named on the result and in the evidence | Honest and already the accepted meaning of the word; an operator who reads only the headline may over-read it |
| **b** | The remainder blocks `accepted`; the run ends as `abstained` pending the operator's judgement | Maximally cautious; makes almost every real goal end in a non-success terminal, which devalues the terminal |
| **c** | The operator's explicit sign-off on the remainder converts the outcome to `accepted` | Keeps the word strict and adds a decision at the end of every run |

**What the design assumes.** Option **a**, with the remainder shown on the same line as the
verification count, never in a footnote. Surface S19 and S25 are drawn that way.

## D6 · The clarification budget and what happens when it runs out

> **DECIDED 2026-08-15 — option (a) with the brief's terminal.** The collective proceeds under
> stated assumptions (class C, shown) wherever an assumption is safe; it asks only about genuine
> intent ambiguity (brief v2 Part A §10), at most three questions per run by default. If an
> essential question stays unanswered (unattended run), the run ends NEEDS CLARIFICATION (Part A
> §15), never `abstained`, and `Continue` resumes it once answered. Never "create an oracle"-type
> questions; never identifiers.

**Question.** A run may ask a bounded number of questions. What happens at the bound?

**Why the owner.** The three answers are three different products: one that guesses, one that stops,
and one that keeps asking.

**Options.**

| | Rule | Consequence |
|---|---|---|
| **a** | Proceed under stated assumptions, recorded as class C and shown | The run always produces something; a wrong assumption is discovered at the result |
| **b** | Stop as `abstained` with the unresolved question published | Never guesses; a goal with three genuine ambiguities cannot complete unattended |
| **c** | Ask the operator to raise the budget | Defers the decision to the moment it matters and interrupts an unattended run |

**What the design assumes.** Option **a** with a default of three questions, because brief §1 puts
the burden on ymp to determine intent from the request, the repository, the documentation, the
policy and discovery, and treats asking as the exception.

## D7 · Who sets the diagnostic-disclosure policy for a derived contract

> **DECIDED 2026-08-15 — option (a).** One conservative default for every derived contract: failure
> class plus which requirements were not observed, never protected inputs. Not an operator setting.

**Question.** A failed verification discloses pass/fail, a failure class, or bounded diagnostics.
When the contract is derived rather than written, who decides which?

**Why the owner.** Disclosure trades debugging usefulness against adaptive probing of a fixed check
— the open experiment recorded in
[`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L737-L739).
With a derived plan there is no author to make the call.

**Options.**

| | Rule | Consequence |
|---|---|---|
| **a** | One conservative default for every derived contract: failure class plus which requirements were not observed, never protected inputs | Predictable, and the same everywhere; some failures take more attempts to diagnose |
| **b** | Derivation chooses per check, based on whether the check is generated or discovered | More useful diagnostics where they are safe; the policy becomes something the product decides about itself, which is harder to reason about |
| **c** | An operator boundary in the run policy | Explicit; adds a setting most operators cannot evaluate |

**What the design assumes.** Option **a**. Surface S19 draws exactly that disclosure.

## D8 · Default ceilings

> **DECIDED 2026-08-15 — setting (b) *working* as the placeholder standing ceiling of a workspace
> (D3), replaced by measurement in `W1-EVL-04a`.** $5 · 2h · 6 participants · 3 concurrent
> attempts · 4 verification queries · 3 questions · $0.10 derivation. Visible and editable on
> `/budget`; never asked. Flagged to the owner as the one decision that spends money unattended.

**Question.** What are the default spend, wall clock, participant, concurrency, verification-query
and question ceilings for a run with no operator narrowing?

**Why the owner.** They decide what an unattended run costs and how hard the host is worked, and
they cannot be derived from the code — no measurement of a multi-participant run exists yet.

**Options.** Three coherent settings rather than an open field of numbers, because the dimensions
interact and choosing them one at a time produces a policy nobody intended.

| | Setting | Consequence |
|---|---|---|
| **a** *cautious* | $2 · 1h · 3 participants · 1 concurrent attempt · 2 queries · 2 questions · $0.05 derivation | Cheapest to be wrong with, and the likeliest to end as `exhausted` holding an unverified candidate — which reads as failure and is not. One attempt at a time also removes the competing-branch behaviour the POC hypothesis is about |
| **b** *working* | $5 · 2h · 6 participants · 3 concurrent attempts · 4 queries · 3 questions · $0.10 derivation | Enough headroom for the §18 scenario to complete, including one failed verification and a revision; three concurrent attempts is three sandboxes and three engine processes on the host |
| **c** *generous* | $20 · 6h · 12 participants · 6 concurrent attempts · 8 queries · 5 questions · $0.25 derivation | Room for competing branches and several verification cycles; multiplies host load and disk, and an unattended run can spend twenty dollars against a goal that was ambiguous |

Two consequences hold across all three. Participant starts multiply cost without bounding it, so
the money ceiling and not the participant ceiling is what actually stops a run. And a
verification-query budget that is too small converts a nearly-finished run into `exhausted`, which
is honest but wastes everything spent before it.

**What the design assumes.** Setting **b**, which is what surface S06 draws. These are placeholders
chosen for coherence, not measurements: no multi-participant run has been observed yet, and the
first evaluation (`W1-EVL-04a`) is what should replace them.

## D9 · Naming: *workspace* and *attempt sandbox*

> **DECIDED 2026-08-15 — option (a).** *workspace* is the operator's project; the private writable
> copy of one attempt is the *attempt sandbox* (unit P14).

**Question.** Brief §6 and §13 make *workspace* the operator's word for the project. Today it names
the private writable copy of one attempt, and
[`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L112-L117)
explicitly forbids the other use. Does the internal term change?

**Why the owner.** It is a rename across the documents of record and internal identifiers. The cost
is small and entirely in one direction; the decision is whose word wins.

**Options.**

| | Rule | Consequence |
|---|---|---|
| **a** | The internal copy becomes *attempt sandbox*; *workspace* is the project | The brief's product language holds; a rename touches documents, identifiers and comments, and no behaviour |
| **b** | The operator's word for the project becomes *project* | No rename; the brief's §13 vocabulary is contradicted on one word, and "project" is already used for the enclosing scope in the k9s naming table |

**What the design assumes.** Option **a**, recorded as proposed unit P14 in
[`COLLECTIVE-MIGRATION.md`](COLLECTIVE-MIGRATION.md).

## D10 · Whether `Continue` re-authorizes

> **DECIDED 2026-08-15 — option (c), following D3(b).** `Continue` starts the next run at once
> while the standing ceiling has headroom; the only prompt is the exhausted case.

**Question.** `Continue` opens a new run in the same workspace with the accepted candidate as its
base. Does it require a fresh authorization?

**Why the owner.** It is the same trade as D3, at the moment the operator is most likely to want
momentum and least likely to reread a ceiling.

**Options.**

| | Rule | Consequence |
|---|---|---|
| **a** | Full authorization, as for any run | Consistent; the operator confirms twice in a minute when continuing immediately |
| **b** | One confirmation, since the workspace policy and pool are unchanged | Matches the accepted rule that an unchanged re-authorization is one confirmation (node `W1-APP-02v`) |
| **c** | No confirmation while the standing ceiling has headroom | Only coherent if D3 resolves to option **b** |

**What the design assumes.** Option **b**, consistent with the re-authorization weight already
accepted.

---

## Summary

| # | Decision | Design assumes | Blocks |
|---|---|---|---|
| D1 | Where the pool is declared | frozen per run, over the registry | node `W1-EVL-04d`; proposed unit P3 |
| D2 | Entry rule at bootstrap | first ready entry in declared order, recorded in the pool-freeze fact | proposed unit P7 |
| D3 | Authorization mandatory per run | yes, one confirmation, before the run's budget | surface S10; criterion 17.1 |
| D4 | Disclosure default, including before authorization | per workspace; one setup question for derivation | surfaces S02, S03; proposed units P2, P4a |
| D5 | Semantic remainder | `accepted` covers the observed part, remainder named | criterion 17.12; surfaces S19, S25 |
| D6 | Clarification bound | proceed under stated assumptions, three questions | proposed unit P5 |
| D7 | Diagnostic disclosure | one conservative default | surface S19 |
| D8 | Default ceilings, including the derivation allowance | setting **b** | proposed units P3, P4a |
| D9 | Naming | *attempt sandbox* | proposed unit P14 |
| D10 | `Continue` re-authorizes | one confirmation | surface S25 |

**All ten are decided as of 2026-08-15** (D1 by the owner; D2–D10 by the CTO under the owner's
delegation "resolve the maximum by the application's logic", each marked in its block; the owner may
overturn any of them). Common thread: no confirmation screens, no identifiers to type, no setup
wizard — the operator's deliberate acts are enabling a provider and stating a goal.

Historical note — none of the ten blocked the start of implementation. D1 and D2 must be answered before proposed unit
P8 — recruitment against a pool — can be accepted, because a pool whose declaration is undecided
cannot be checked against, and an entry rule that is undecided cannot be proved not to be a
semantic router.

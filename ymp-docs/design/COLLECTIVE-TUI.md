# The operator surface

Item 3 of the design deliverable: the complete terminal information architecture, designed from
zero as brief §6 requires. Every surface is stated three ways — **what the operator sees**, **what
actions exist**, **what each action does**. Read
[`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) first; the boundaries and vocabulary it fixes are
assumed here.

Brief §6 requires at least thirty surfaces and enumerates twenty-seven items. Those twenty-seven
expand into thirty surfaces — four of them name two surfaces each, because a list and its detail
are different things to design — and four more are added because the design needs them. Thirty-four
in total. Source links are pinned to
[dfdac03](https://github.com/maggnus/ymp/commit/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf).

## Shape

One scrolling conversation fills the screen, with an always-ready input line, a thin context header
and a status line. Data lives on full-screen pages opened from the `/` command line and closed with
`Esc`. Decisions arrive as modals and never look like pages. This is the composition already
accepted in
[`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L46-L64)
and implemented in `ymp-tui`; the redesign changes what the surfaces contain, not the shape they
take.

    ┌ ymp · workspace ymp · run 0007 · running · $0.42 of $5.00 ─────────────┐
    │                                                                       │
    │   the conversation: operator turns, ymp replies, run events,          │
    │   participant messages — each attributed and typed                    │
    │                                                                       │
    ├───────────────────────────────────────────────────────────────────────┤
    │ > _                                                                   │
    │ 3 participants · verifying cd-2 · /help                               │
    └───────────────────────────────────────────────────────────────────────┘

Nine rules govern every surface below.

1. Lead with product language: collective, participant, goal, task, activity, result, verification,
   evidence, provider, model, workspace, budget. Internal words appear only inside advanced views,
   and never alone — the plain fact stands beside them.
2. Nothing a participant published is actionable in one keypress (INV-4).
3. No ranking, scoring or grading anywhere; sorting uses raw mechanical fields (INV-1).
4. Control, collaboration and verification planes stay visually distinct where they meet.
5. Every view holds its own cursor and recovers from the journal; lag is a visible state (INV-6).
6. A terminal state is named exactly; `exhausted` is never drawn as success (INV-7).
7. Budget dimensions never trade against one another on screen (INV-2).
8. The assurance profile never appears without its limit.
9. Every action here exists as a command of the same executable (node `W1-APP-02n`).

## Surface map

| Brief §6 item | Surface |
|---|---|
| startup | S01 |
| first run | S02 |
| provider discovery / auth | S03, S04 |
| model catalog | S05 |
| — (policy, added) | S06 |
| workspace selection | S07 |
| task creation and clarification | S08, S09 |
| — (authorization, added) | S10 |
| collective startup | S11 |
| live collective view | S12 |
| `/agents` and participant details | S13, S14 |
| task / obligation view | S15 |
| communication / activity view | S16 |
| candidate view | S17, S18 |
| verification state | S19 |
| failures | S20 |
| pause | S21 |
| resume | S22 |
| cancellation | S23 |
| intervention | S24 |
| result | S25 |
| evidence | S26 |
| export | S27 |
| history | S28 |
| archive | S29 |
| provider / model configuration | S04, S05, S31 |
| budgets | S30 |
| runtime readiness | S31 |
| recovery from invalid configuration | S32 |
| — (help, added) | S33 |
| — (diagnostics, added) | S34 |

---

## S01 · Startup and readiness

**Sees.** Four lines appended to an empty transcript as each fact is established, then the input
line ready. Nothing blocks: probes run in the background and the operator may type immediately.

    ymp 0.1.0 · state under ~/.ymp
    workspace  ymp · /Users/…/Code/ymp · git clean · 1 812 files
    providers  probing… (3 configured)
    assurance  poc_process_isolation · attempts run as separate processes with your own
               permissions; this is not hostile-code containment
    > state what you want done
    ─────────────────────────────────────────────────────────────────
    /help for commands

**Actions.**

| Action | Effect |
|---|---|
| type a goal | Opens S08. The workspace is read locally at once, which costs nothing; deriving what *done* means draws on the derivation allowance and is stated as it happens. |
| `/providers` | Opens S03. |
| `/workspace` | Opens S07. |
| `/history` | Opens S28. |
| `/help` | Opens S33. |

**Notes.** The provider line resolves to `3 ready · 1 needs authentication` when probes return; if
none is ready the surface becomes S02. The assurance line is a single unit of text and the profile
name is never shown without its limit
([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L228-L246)).

## S02 · First run

The state where nothing is configured. It is the only moment the product asks the operator to
set anything up, and it asks for exactly one thing.

**Sees.** Two sentences of what ymp is, the detected providers with their state, and one next step.

    ymp runs a collective of agents against a goal you state, and returns a result an
    independent check accepted. You do not write the check, pick the models, or build a team.

    to start, connect one provider:
      anthropic   not configured   the Claude Code engine reaches it
      openai      not configured   the Codex engine reaches it
      nvidia      not configured   reached through Claude Code, Messages endpoint

    > /providers

**Actions.**

| Action | Effect |
|---|---|
| `/providers` | Opens S03. |
| type a goal anyway | Accepted and held. The workspace is read locally and the facts are shown; the derivation that needs a model does not run, because no provider is connected and nothing may leave the host. Authorization (S10) states that, and offers S03. Nothing is spent, nothing is disclosed, and nothing is lost. |

**After the first provider connects**, this surface asks its one policy question and then never
appears again for this workspace:

    before ymp can turn a goal into checkable requirements it sends bounded excerpts of this
    workspace to one provider. that happens before you authorize a run.
      disclose to        anthropic          [change]
      derivation budget  $0.10 per goal     [change]
    [accept]  these stay visible under /policy and can be changed there.

`accept` records the disclosure class and the derivation allowance for the workspace; `change`
opens the corresponding row of S06. Declining is possible and honest: the product states that
without a derivation allowance it can read the workspace but cannot derive what *done* means, and
the operator would have to state the acceptance condition themselves — which is the one path the
design otherwise removes.

**Notes.** No contract, verifier, model or team is mentioned on this surface. An empty catalog is a
state, not an error. This is the only setup question the product asks, and it exists because the
alternative is disclosing without saying so.

## S03 · Providers

**Sees.** One row per provider: name, state, what reaches it, how many models it serves, and the
disclosure sentence.

    providers ─ 4 · 2 ready ─────────────────────────────────────────────────
    NAME        STATE                 REACHED BY          MODELS   LAST PROBE
    anthropic   ready                 claude-code 2.1     4        12s ago
    openai      ready                 codex 0.147         5        12s ago
    nvidia      needs authentication  claude-code · Messages —      12s ago
      ↳ fix: no credential found; connect to authenticate
    local       unavailable           codex · Responses      —      12s ago
      ↳ endpoint http://localhost:8000 did not answer

    every provider is reached through an installed engine. enabling one permits repository
    content to be sent to it, including during derivation, before a run is authorized.
    Enter describe · c connect · e enable/disable · r re-probe · Esc back

**Actions.**

| Action | Effect |
|---|---|
| `Enter` | Opens S04 for that provider. |
| `c` connect | Where the engine owns the credential, launches that engine's own authentication in a child process and re-probes when it returns. Where the provider is reached by a route override on an engine, asks where the credential is read from and against which endpoint, records both — never the credential itself — and runs the pairing's conformance probe before the entries become admissible. |
| `e` enable / disable | Flips the enabled flag on the provider's catalog records. A disabled provider's entries leave every pool; running participants are unaffected, and the surface says so. |
| `r` re-probe | Re-runs the probe. Measured properties and model lists are replaced by what the probe returned. |
| `Esc` | Returns to the conversation. |

**Notes.** Disclosure is stated here because host allowlisting is not a confidentiality control
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L562-L568)).

## S04 · Provider detail and connection

**Sees.** Grouped facts rather than a table: identity, authentication, models served, disclosure
class, and the last probe with its exact result.

    providers › anthropic ────────────────────────────────────────────────────
    identity        anthropic · reached by engine claude-code
    engine          /usr/local/bin/claude · 2.1.227 · sha256:4c1e…
    credential      read from the engine's own store; ymp holds no copy
    models          claude-opus-5, claude-sonnet-5, claude-haiku-4-5, …
    disclosure      repository content sent to anthropic
    conformance     tool calls ✓ · streaming ✓ · cancellation ✓ · usage ✓
    last probe      12s ago · ready

**Actions.** `c` connect (as S03), `e` enable/disable, `r` re-probe, `Esc` back. Connect on a ready
provider re-authenticates rather than duplicating a credential.

**Notes.** A pairing that has not passed its conformance probes is shown with the failing capability
named and is not admissible to a pool
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L494-L500)).

## S05 · Model catalog

**Sees.** Every catalog entry — a provider · engine · model triple — with its measured properties.
This is the level that answers "what could be used", never "what is running".

    models ─ 11 entries · 9 admissible ───────────────────────────────────────
    PROVIDER   ENGINE       MODEL              STATE        LABELS
    anthropic  claude-code  claude-opus-5      admissible   reasoning
    anthropic  claude-code  claude-haiku-4-5   admissible   fast, cheap
    openai     codex        gpt-5.6-sol        admissible   reasoning
    nvidia     claude-code  nemotron-…         unavailable  reasoning
      ↳ provider needs authentication · route: Messages endpoint, pairing probed separately

    nothing here is an agent. a participant exists only when one is recruited and paid for.
    Enter describe · p add/remove from pool · Esc back

**Actions.**

| Action | Effect |
|---|---|
| `Enter` | Describes the entry: engine version and digest, wire protocol, account scope, disclosure class, conformance results, and which runs used it. |
| `p` | Adds or removes the entry from the workspace's default pool (S06). Removing an entry a running participant occupies does not stop it; the change applies to pools frozen after it. |
| `Esc` | Back. |

**Notes.** The line about agents is permanent, not a hint: it is criterion 17.3 said on the surface
where it would otherwise be misread. Labels are resource groupings a participant may read; they are
never a role and never an input to a kernel decision.

## S06 · Permitted pool and policy

**Sees.** The boundaries a run would start with, all with their defaults visible and their source
named.

    policy · workspace ymp ───────────────────────────────────────────────────
    pool                9 entries · every enabled, ready, conformant entry
    derivation budget   $0.10 per goal · spent before authorization    set at first run
    derivation provider anthropic · claude-code · claude-haiku-4-5     set at first run
    spend ceiling       $5.00                                      default
    wall clock          2h                                          default
    participants        up to 6                                     default
    concurrent attempts up to 3                                     default
    verification        4 queries                                   default
    questions           up to 3                                     default
    disclosure          anthropic, openai                        from providers
    external actions    none                                        default
    assurance           poc_process_isolation · no hostile-code containment

**Actions.**

| Action | Effect |
|---|---|
| `Enter` on a row | Edits that boundary. A narrower value is applied immediately to the workspace default; a wider one states what it permits before it applies. |
| `p` | Opens S05 to change pool membership. |
| `Esc` | Back. |

**Notes.** The two derivation rows are the exception to the sentence below: they are set once at
first run (S02) rather than defaulted silently, because they govern spend and disclosure that
happen *before* an authorization exists. Everything else on this surface has a default.

This surface is otherwise never a prerequisite: every field has a default and a run may be
authorized without opening it. The policy declares *what may be used and how much*; it contains no
field that says which model suits which task, and none may be added — that sentence has no
representation in the kernel and criterion 17.7 depends on it.

## S07 · Workspace selection

**Sees.** The project the goal is about, identified from the launch directory, with the other
projects the product root already knows.

    workspace ─ current: ymp ────────────────────────────────────────────────
    NAME     PATH                          RUNS   LAST RUN
    ymp      /Users/…/Code/ymp             7      12m ago · accepted
    parser   /Users/…/Code/parser          2      3d ago · exhausted

**Actions.** `Enter` switches the workspace for this session and re-reads its runs and policy;
`Esc` back. Switching never moves, copies or writes anything in either project directory.

**Notes.** *Workspace* is the project, per brief §13. The private writable copy of one attempt is
the *attempt sandbox* and appears only in S34.

## S08 · Goal entry

**Sees.** The input line, then two visibly different stages: the local reading, which is free, and
the derivation, which spends the workspace's derivation allowance and names the provider it
discloses to while it does. The run's own budget is untouched throughout.

    > Implement feature X in this repository.

    ymp  reading the workspace… 1 812 files · rust · cargo · 214 tests
         (local · nothing left this host)

         deriving what "done" means… sending bounded excerpts to anthropic
         $0.03 of the $0.10 derivation budget

         the goal, as I read it:
           add feature X so that <observable statement> …
         checkable from the repository: the existing test suite, the build, the public API
         one thing I could not settle — see below

**Actions.**

| Action | Effect |
|---|---|
| type another line | Amends the goal and re-derives, drawing on the same derivation budget; the remaining amount is restated. The previous derivation is discarded. The run's budget is still untouched. |
| `/authorize` or `Enter` on the authorization prompt | Opens S10. |
| `/policy` | Opens S06 to narrow boundaries, or to change the derivation budget and its provider, before authorizing. |
| `Esc` | Discards the derivation. The typed goal stays in the transcript. What derivation already spent is not returned, and the surface says so rather than implying a free retry. |

**Notes.** The two stages are drawn apart because they differ in what they cost and what they
disclose, and an operator who reads only one line should read the true one. A derivation budget
that runs out stops the derivation with what it has and states it here and on S10; it never
produces a thinner plan silently.

The operator is never asked here for a verifier, a negative control, a source path, a
task breakdown, a team or a model. The advanced amendment grammar that exists today
([`draft.rs`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-rust/crates/ymp-tui/src/draft.rs#L67-L88))
survives as an override for an operator who wants one, and is never required, offered or implied.

## S09 · Clarification

The only interruption before authorization, and only for an assumption that materially changes the
result and cannot be settled from the request, the repository, the documentation, the visible
checks, the policy or discovery.

**Sees.** One question, why it matters, and concrete options where the derivation found them.

    ymp  one thing changes what "done" means and I cannot settle it from the repository:

         should the token endpoint use Authorization Code or Client Credentials?
         both appear in the codebase; the tests cover neither path end to end.

         [1] Authorization Code — user-facing flow; the redirect handler is added
         [2] Client Credentials — service-to-service; no redirect handler
         [3] tell me in your own words

         questions used: 1 of 3

**Actions.**

| Action | Effect |
|---|---|
| choose an option or type an answer | Recorded as provenance class E, folded into the requirements, and the derivation reruns. The run is **not** marked intervened. |
| `/skip` | ymp proceeds under the assumption it states, records it as class C, and shows it at authorization and in the result checklist. |
| `Esc` | Leaves the question open. Authorization is blocked while a material question is unanswered, and the surface says which one. |

**Notes.** The question is never "please create an acceptance oracle / verifier / contract"; brief
§1 names that as the inappropriate shape and no code path may produce it. Questions are bounded by
the `ClarificationRequests` dimension, so asking more is not a way to avoid deciding.

**The mid-run form.** The same surface appears during a run when a participant meets a material
ambiguity and spends a clarification unit. It differs in one stated way: the answer reaches the
collective as attributed collaboration data and does not change what the candidate is judged
against, because the run is bound to one contract. A solicited answer does not mark the run
intervened and is recorded as solicited.

**The divergence state.** When the classifier of
[`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) item 16 reads a mid-run answer as contradicting a
recorded requirement — or cannot classify it, which is treated the same way — this surface returns
with the divergence rather than letting the run continue quietly.

    your answer changes something this run is already being judged against

      R2 (from your goal)  the token endpoint uses Authorization Code
      your answer          it must be Client Credentials

    this run is judged against R2. until you choose, no verification query is spent.

    [1] it does not change what "done" means — continue this run, answer stays advisory
    [2] it does change what "done" means — end this run and continue with the amended goal

**Actions in that state.**

| Action | Effect |
|---|---|
| `[1]` | The answer stays inert collaboration data, the run continues, the acceptance path is released, and the operator's judgement is recorded beside the divergence fact. |
| `[2]` | The run stops and records `cancelled`, which is the honest terminal for an operator decision. `Continue` opens a new run against the amended goal, with the candidates, journal and findings of this one still readable and available as a base. |
| leave it | The run continues working, but no verification query may be spent against the diverged requirement set, and the status line states which choice it is waiting for. |

**Notes.** The hold on the acceptance path is the mechanism, not the wording: a run cannot reach
`accepted` against a definition the operator has contradicted, because it cannot spend the query
that would produce the verdict until the contradiction is resolved. An unclassifiable answer is
treated as divergent, so the classifier fails towards asking (item 22, tests 27 and 28).

## S10 · Run authorization

The single confirmation that stands between a goal and the **run's** budget. It is a spend
decision, not a review of an oracle. It is not the first money the product ever spends — deriving
what *done* means came first, from a separate allowance — and the `already` row states that rather
than letting the screen imply otherwise.

**Sees.**

    authorize run ────────────────────────────────────────────────────────────
    goal        add feature X so that <observable statement>
    done means  6 observable requirements
                  4 from your goal, 2 from the repository, 1 assumption
                1 part of the goal is not mechanically checkable and is named as yours
    already     deriving this cost $0.03 and sent bounded excerpts to anthropic
    spend       up to $5.00 · 2h · 6 participants · 4 verification queries
    models      9 entries across anthropic, openai
    disclosure  repository content will be sent to anthropic, openai
    assurance   poc_process_isolation · no hostile-code containment
    run         0007 in workspace ymp

    type the run id to authorize:  ▁▁▁▁
    Enter what "done" means · p boundaries · d diagnostics · Esc discard

**Actions.**

| Action | Effect |
|---|---|
| type the run id, confirm | Stores the internal contract, creates the run bound to it, commits the pool-freeze fact — every permitted entry with its identity, disclosure class, assurance profile and measured readiness, plus the entry the run will ignite on — and hands control to bootstrap (S11). This is the first irreversible act and the first charge against the run's own budget. |
| `Enter` on "done means" | Expands the requirements in plain sentences with their provenance marks. Reading is optional; nothing here is an item to approve. |
| `p` | Opens S06. Returning re-derives against the narrowed boundaries and restates this surface. |
| `d` | Opens S34, where the requirement-to-evidence coverage map, the generated checks and their digests are available in full. |
| `Esc` | Discards. No contract was stored and the run's budget was never touched. What derivation already spent and disclosed stands, and the `already` row is what said so before the decision. |

**Notes.** The typed-identifier ceremony applies to the first authorization in a workspace; an
unchanged re-authorization is one confirmation, as already accepted in node `W1-APP-02v`. Blocking
conditions are ymp's own limits, never requests for authorship: an unanswered material question, a
derived plan that failed its negative control, no ready provider. Each states what the operator can
do — answer, narrow the goal, connect a provider — and none asks for a check.

## S11 · Collective startup

**Sees.** Four lines in the transcript, one per bootstrap step, so the operator sees a collective
being created rather than a spinner.

    run 0007 started · $0.00 of $5.00
    participant A · anthropic · claude-code · claude-opus-5 · started
      the first participant is created by ymp; further participants are recruited by the
      collective itself
    A is reading the workspace

**Actions.** None required. `/agents`, `/activity`, `/budget` and `/tasks` are available; `Esc`
keeps the conversation.

**Notes.** Exactly one participant is created here. The catalog entry is the first ready entry of
the frozen pool in declared order — a mechanical rule that reads no property of the goal
([`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) item 8).

## S12 · Live collective view

The main surface for the duration of a run. Reading a conversation is an activity; reading an event
table is not.

**Sees.** One transcript carrying four kinds of entry, each distinguishable at a glance: operator
turns, ymp replies, run events, and participant messages attributed by author and typed by kind.
The header carries workspace, run, state and spend; the status line carries participant count and
what verification is doing.

    14:02  A  observation   the parser has no fixture for the redirect path
    14:03  ⚙  recruitment   A requested a participant · admitted · B started
                            anthropic · claude-code · claude-sonnet-5
    14:06  B  challenge     the fixture exists under tests/legacy; A's claim is stale
    14:07  A  confirmation  agreed — taking the legacy path
    14:11  ⚙  candidate     cd-1 submitted by A
    14:12  ⚙  verification  cd-1 · query 1 of 4 · running

**Actions.**

| Action | Effect |
|---|---|
| scroll | Pauses following. The run continues; the header states that drawing is paused. |
| `End` | Resumes following. |
| `/agents` `/tasks` `/activity` `/candidates` `/verify` `/budget` | Open S13, S15, S16, S17, S19, S30. |
| type a message | Opens S24, which states the intervention consequence before sending. |
| `/pause` `/cancel` | Open S21, S23. |

**Notes.** A participant's message is inert: it can be read, cited and inspected, and nothing in it
can be executed with one key (INV-4). No entry is ranked, scored or highlighted as more important.

## S13 · Agents

**Sees.** Who is working and on what, active and finished, with mechanical columns only.

    agents ─ 3 active · 1 finished ───────────────────────────────────────────
    ID  STATE      ENTRY                                TASK            SPENT
    A   working    anthropic·claude-code·opus-5         root goal       $0.31
    B   working    anthropic·claude-code·sonnet-5       t-2 parser      $0.09
    C   yielded    openai·codex·gpt-5.6-sol             t-3 review      $0.04
      ↳ waiting for the verification result of cd-1
    D   returned   openai·codex·gpt-5.6-sol             t-1 survey      $0.07

**Actions.** `Enter` opens S14; `Tab` switches active/finished; `Esc` back.

**Notes.** There is no action here that assigns work, changes a model, promotes, ranks or stops one
participant. The operator's authority over the collective is the run, not its members: brief §10
makes the operator the principal, not the team manager. Sorting is by identifier or by a raw
mechanical column only (INV-1).

## S14 · Participant detail

**Sees.** Grouped facts: identity and entry; who recruited it and for what; lifecycle; budget by
dimension with what remains; the work it holds and returned; what it published; what it submitted;
what it requested from verification; whom it recruited; and its failures, yields and cancellations.

    agents › B ──────────────────────────────────────────────────────────────
    entry        anthropic · claude-code · claude-sonnet-5
    recruited    by A at 14:03 · admitted: entry in pool, 4 participant starts left
    state        working · attempt 2 · slice 3 of 8
    budget       $0.41 left · 38 min left · 1 attempt start · 5 slices
    holds        t-2 parser · taken 14:04 · not returned
    published    3 observations · 1 challenge
    submitted    cd-1 at 14:11
    verification requested cd-1 · failed · diagnostic below
    recruited    nobody

**Actions.** `Enter` on a row opens the corresponding surface (task → S15, message → S16,
candidate → S18, verification → S19); `d` opens S34 for the internal record; `Esc` back.

**Notes.** No private chain-of-thought is shown, requested or stored — only published summaries and
externally visible actions
([`CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/CONCEPT.md#L74-L78)).

## S15 · Tasks

**Sees.** The work the collective created, with recorded parentage expressed as indentation.

    tasks ─ 4 · 1 open ──────────────────────────────────────────────────────
    ID    WORK                        SPONSOR  HOLDER  STATE
    root  add feature X               ymp      A       open
      t-1 survey the parser paths     A        D       returned · result
      t-2 implement the redirect      A        B       open
      t-3 review cd-1                 A        C       open

**Actions.** `Enter` describes a task — what it asks for, the base it works from, what funds it,
what it depends on, and its return; `d` shows the internal obligation and escrow rows; `Esc` back.

**Notes.** Indentation is recorded parentage, never priority or importance. The words *obligation*
and *escrow* appear only under `d`.

## S16 · Activity

The collaboration plane, labelled untrusted wherever it is drawn.

**Sees.** Attributed messages with kind, audience, evidence references and time. Kinds are the
accepted set: proposal, question, hypothesis, observation, constraint, dead end, challenge,
confirmation, decision, help request.

    activity ─ 27 messages · untrusted collaboration data ────────────────────
    TIME   FROM  KIND          AUDIENCE   SUMMARY
    14:06  B     challenge     t-2        the fixture exists under tests/legacy
    14:07  A     confirmation  t-2        agreed — taking the legacy path
    14:09  C     question      cd-1 rev   which requirement does the new branch serve?

**Actions.** `Enter` opens the full message with its references and delivery receipts; `f` filters
by author, kind or task; `Esc` back.

**Notes.** A delivery receipt proves that bytes were made available, not that a model read or
believed them, and the surface says so where receipts are shown. No message is a button: acting on
a suggestion is a separate operator command under the operator's own authority.

## S17 · Candidates

**Sees.** Every immutable candidate the run produced, with its ancestry and verification state.

    candidates ─ 2 ──────────────────────────────────────────────────────────
    ID    FROM  BASE   DIGEST         VERIFICATION
    cd-1  A     S0     sha256:1a4f…   failed · 14:12
    cd-2  B     cd-1   sha256:9f2c…   passed · 14:31

**Actions.** `Enter` opens S18; `Esc` back.

**Notes.** No candidate is presented as ready to apply to the working tree, and no candidate is
ranked or recommended. Where several pass, the accepted set is presented and the product invents no
tie-breaker
([`PROJECT-CONTRACT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROJECT-CONTRACT.md#L190-L191)).

## S18 · Candidate detail

**Sees.** What changed, against which base, by whom, and what verification said. The change is a
summary of paths and sizes with a path to the full difference.

    candidates › cd-2 ───────────────────────────────────────────────────────
    digest      sha256:9f2c…
    base        cd-1 · sha256:1a4f…
    submitted   B · 14:28 · attempt 2
    changes     4 files · +212 −38
    verification passed · 14:31 · 6 of 6 requirements observed
    evidence    contract sha256:… · environment sha256:… · oracle sha256:…

**Actions.** `Enter` on `changes` shows the difference; `Enter` on `verification` opens S19;
`e` opens S26; `Esc` back.

## S19 · Verification

**Sees.** Three states, drawn differently.

*Running:* `verifying cd-2 · query 2 of 4 · requested by B · started 14:29`.

*Failed:*

    verification › cd-1 · failed ────────────────────────────────────────────
    class       behavioral check did not hold
    diagnostic  2 of 6 requirements were not observed:
                  R3 the redirect path returns 302 — observed 500
                  R5 existing suite still passes — 3 failures
    disclosure  bounded by the run's policy; protected material is not shown
    the collective has this same diagnostic and is reacting. the run is not over.

*Passed:*

    verification › cd-2 · passed ────────────────────────────────────────────
    R1  A  the endpoint accepts a valid code            observed by check k-1
    R2  A  invalid codes are rejected                   observed by check k-1
    R3  A  the redirect path returns 302                observed by check k-2
    R4  B  the existing suite still passes              observed by check k-3
    R5  B  the build succeeds from a clean base         observed by check k-4
    R6  C  tokens are not written to logs (assumption)  observed by check k-5
    ─   A  "clean and idiomatic" is not mechanically checkable — yours to judge
    digests  contract sha256:… candidate sha256:… environment sha256:… oracle sha256:…

**Actions.** `Enter` on a requirement shows its provenance and the check that observed it (S34 for
the check's own bytes and digest); `Esc` back.

**Notes.** Protected material never appears, and the diagnostic is exactly what the run's
disclosure policy allows — unbounded feedback would let a candidate be fitted to a fixed check
([`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROTOCOL.md#L471-L478)).
A verifier infrastructure failure is drawn as its own state and is never a rejection.

## S20 · Failures

One surface for the things that go wrong short of a terminal: a participant that failed, an
invocation that hit a limit, an engine that stopped answering, a submission that would not
integrate.

**Sees.**

    failures ─ 3 ────────────────────────────────────────────────────────────
    TIME   WHAT                    DETAIL
    14:19  B · invocation ended    model route error · upstream 529 · retried
    14:22  cd-1 · integration      conflict against base cd-0 · became task t-4
    14:26  C · limit               slice wall clock exhausted · yielded

**Actions.** `Enter` describes one failure with the exact recorded reason; `Esc` back.

**Notes.** A model route error is not evidence that the task is unsolvable, and the surface states
that where it appears
([`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROTOCOL.md#L398-L414)).
A failure here is never presented as a terminal state.

## S21 · Pause

**Sees.** A modal stating exactly what pausing does and does not do.

    pause run 0007
      running slices finish; no new slice is admitted
      leases keep running out, so a paused run can still expire
      spend already committed is not returned
    [pause]  Esc cancel

**Actions.** `Enter` pauses — the launcher stops admitting new invocations and the transcript
records it; `Esc` closes.

**Notes.** Pause does not mark the run intervened: it is authority over resources, not content.
The honest line about leases is required, because a pause that silently let leases expire would
look like a stop and behave like a timeout.

## S22 · Resume

**Sees.** What a paused run would resume with: participants waiting, remaining budget by dimension,
and how much wall clock was lost to the pause.

**Actions.** `Enter` resumes — admission restarts and each eligible yielded slice becomes eligible
again, consuming its own `InvocationStarts` unit; `Esc` closes. If a lease expired during the
pause, the surface names the task and states that it must be re-taken rather than resumed.

## S23 · Cancellation

**Sees.** The typed confirmation already accepted in
[`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L248-L260),
with the run identifier required exactly.

    cancel run 0007
      3 participants are interrupted; 2 attempts are lost
      $0.42 of spend is not returned
      the journal and every published candidate stay readable
      the recorded outcome will be cancelled
    type the run id:  ▁▁▁▁

**Actions.** Type the identifier and confirm — the cancel command reaches the kernel, processes are
wound down and the run reaches `cancelled`; `Esc` closes.

**Notes.** `cancelled` and `infrastructure_error` are different facts and are never substituted. A
runtime that dies while being stopped is a consequence of the decision and does not change the
recorded outcome.

## S24 · Intervention

**Sees.** The message composer, with its consequence stated before anything is sent.

    message the collective
      your message enters the collaboration plane as untrusted data
      it grants no authority and cannot form a contract, choose a bid or spend budget
      the run will be marked intervened and excluded from autonomous comparison
    to: t-2 (task audience) · all participants
    > _

**Actions.** `Enter` sends — the message is attributed to the operator, delivered to the chosen
audience and the run is marked intervened; `Esc` discards.

**Notes.** An answer to a question ymp asked (S09) is *not* an intervention and does not pass
through this surface: it is provenance class E on the designed path. The distinction is drawn on
both surfaces so neither is mistaken for the other.

## S25 · Result

**Sees.** What the operator got, before how it was made.

    run 0007 · completed · verified ─────────────────────────────────────────
    <one paragraph, published by the collective, of what changed>

    verification   6 of 6 requirements observed
                   1 part of the goal is not mechanically checkable — yours to judge
    participants   3 · 5 attempts · 2 candidates
    time / spend   41 min · $2.18 · 214k tokens across 2 models
    candidate      cd-2 · sha256:9f2c…
    [Inspect result] [Inspect evidence] [Export] [Continue] [Archive]

**Actions.**

| Action | Effect |
|---|---|
| Inspect result | Opens S18 for the accepted candidate. |
| Inspect evidence | Opens S26. |
| Export | Opens S27. |
| Continue | Opens S08 with the accepted candidate as the base and the previous goal as context. It is a new run with its own authorization, because it spends new money against a new definition of done. |
| Archive | Opens S29. |

**Notes.** A non-success terminal reaches this surface too, with its own sentence and its own
actions, and is never drawn as a result: brief §12 forbids presenting `exhausted` as success, and
the five terminals are listed in [`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) item 14.

## S26 · Evidence

**Sees.** Everything that supports the claim, in one place: the four digests; the
requirement-to-observation map with provenance; the per-model spend; the participants and their
attempts; the candidate ancestry; the journal head digest; and the assurance profile with its
limit.

**Actions.** `Enter` descends into any row; `x` writes the evidence bundle to a path the operator
names, outside the project directory by default; `Esc` back.

**Notes.** The bundle contains no protected oracle material, no capability material and no runtime
session capsule
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L444-L458)).

## S27 · Export

The only surface that writes into the project directory.

**Sees.** Exactly which paths will be written, with the state of the working tree stated first.

    export cd-2 into /Users/…/Code/ymp
      working tree: clean
      writes 4 files:  src/auth/redirect.rs (new) · src/auth/mod.rs · tests/…
      nothing else in the directory is touched
    [export]  Esc cancel

**Actions.** `Enter` applies the candidate and records the export as an operator act; `Esc` closes.
A working tree with uncommitted changes is stated as such and the operator decides.

**Notes.** Export is a delivery, not storage: the product's own state stays under the product root
(node `W1-APP-02w.1`). No candidate is presented as ready to apply before this surface is opened
deliberately.

## S28 · History

**Sees.** Runs of the current workspace, newest first, with their terminals stated exactly.

    history ─ workspace ymp · 7 runs ────────────────────────────────────────
    RUN   GOAL                       OUTCOME              SPEND   WHEN
    0007  add feature X              accepted             $2.18   12m ago
    0006  add feature X              exhausted · money    $5.00   2h ago
    0005  fix the parser panic       accepted · intervened $0.94  1d ago

**Actions.** `Enter` opens that run's result surface (S25) in read-only form; `Esc` back.

**Notes.** `exhausted` names the dimension that ran out. `intervened` is shown wherever an outcome
is reported, because it changes what the run's evidence may be used for.

## S29 · Archive

**Sees.** A confirmation stating that archiving removes a terminal run from default views and
rewrites nothing.

**Actions.** `Enter` archives: the run leaves the default history view and its record is marked
archived. Nothing is deleted, rewritten or moved, and no retention policy is bypassed. `Esc`
closes. Archived runs remain readable from S28 with a filter, and their journals and candidates are
untouched
([`ARCHITECTURE.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/ARCHITECTURE.md#L90-L94)).

## S30 · Budgets

**Sees.** Every dimension on its own line, with what was reserved, what is committed and what
remains. No total, no percentage of "progress", no dimension expressed in terms of another.

    budget · run 0007 ───────────────────────────────────────────────────────
    DIMENSION              LIMIT      SPENT     LEFT    ENFORCED
    money                  $5.00      $2.18     $2.82   yes · route ceiling
    model tokens           —          214k      —       observed only
    wall clock             2h         41m       1h19m   yes
    verification queries   4          2         2       yes
    participant starts     6          3         3       yes
    attempt starts         12         5         7       yes
    invocation starts      40         14        26      yes
    offer creations        20         4         16      yes
    obligation creations   20         4         16      yes
    external actions       0          0         0       yes

**Actions.** `Enter` shows where a dimension was spent, by participant; `p` opens S06 to raise a
ceiling, which restates what raising it permits; `Esc` back.

**Notes.** The `ENFORCED` column is not decoration: a ceiling is strict only where the driver,
broker or provider can enforce it, and an observational dimension must say so rather than imply a
hard limit
([`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/PROTOCOL.md#L450-L453)).

## S31 · Runtime readiness

The engine level of the catalog: what ymp can actually start on this host.

**Sees.**

    engines ─ 2 · 2 ready ───────────────────────────────────────────────────
    ENGINE       STATE  VERSION            EXECUTABLE                 MODELS
    claude-code  ready  2.1.227            /usr/local/bin/claude · sha256:4c1e…  4
    codex        ready  codex-cli 0.147.0  /usr/local/bin/codex · sha256:8b0d…   5

**Actions.** `e` enables or disables an engine — a disabled engine is neither admitted nor offered,
which is the outcome of node `W1-APP-02e.6`; `r` re-probes; `Enter` describes the measured
properties, capability matrices and admission chain; `Esc` back.

**Notes.** The interface reads the catalog; it does not construct drivers. That is what removes the
fake runtime from this list and from the shipped interface (node `W1-APP-02s`, finding G-06).
Admission semantics — tool set, permission mode, no delegation, budget ceilings — stay compiled in
and are shown here as facts, not settings.

## S32 · Recovery from invalid configuration

**Sees.** The state where the product cannot proceed, with the exact obstacle and one thing to do.
One row per obstacle; nothing is hidden behind a generic failure.

    ymp cannot start a run in this workspace
      no provider is ready              → /providers · connect one
      engine claude-code is 2.0.9       → the pinned profile expects 2.1.227; update or
                                          disable the engine
      the product root is not writable  → ~/.ymp is owned by another account
      an earlier store sits beside the  → .ymp in the project directory is read only with
      project                             an explicit --root

**Actions.** Each row's `Enter` opens the surface that fixes it. `r` re-probes everything.

**Notes.** Refusal rather than degradation is already the accepted rule for admission (node
`W1-APP-02q`): a profile that cannot enforce what it claims is ineligible, not silently weaker. The
surface never offers to proceed without the property.

## S33 · Help and keys

**Sees.** The command list with one line each, and the key overlay for the current surface.

**Actions.** `/` opens the command line with completion; `?` toggles the key overlay; `Esc` closes.
Every command shown here has a command-line twin of the same name, which is what node
`W1-APP-02n` requires.

## S34 · Advanced diagnostics

Where the internal vocabulary lives. Nothing here is required to use the product, and nothing here
is hidden from an operator who asks.

**Sees.** Grouped by plane.

- **Contract** — the internal contract as stored: requirements with provenance classes, the
  acceptance plan with each check's kind, digest and the requirement it observes, the frozen pool by
  digest, the environment manifest, and the requirement-to-evidence coverage map that used to be the
  authorization screen (finding G-07).
- **Control** — obligations, leases and fencing generations, escrow movements, capability grants,
  the command log with idempotency keys.
- **Collaboration** — audiences, membership grants and their expiry, projection lifetimes,
  communication charges.
- **Verification** — query reservations, the disclosure policy in force, the negative-control and
  substituted-entry-point results that admitted each check.
- **Runtime** — invocations, attempt sandboxes, admitted programs with roles and digests, the
  environment each managed process was given, per-model usage evidence.
- **Journal** — sequence, head digest, cursor and lag per view.

**Actions.** `Enter` descends; `x` exports the selected record into the evidence bundle; `Esc`
back.

**Notes.** This surface is how the design keeps two promises at once: the operator's path never
mentions an obligation or an oracle bundle, and no fact is unreachable. A trust-critical fact must
not be hidden — but it need not be simultaneous
([`VISUAL_CONCEPT.md`](https://github.com/maggnus/ymp/blob/dfdac03dede6fa6d50298b07d6d1cd8c6d6687bf/ymp-docs/VISUAL_CONCEPT.md#L20-L27)).
Protected oracle bytes, capability material and runtime session capsules are not here and are not
anywhere the operator can reach.

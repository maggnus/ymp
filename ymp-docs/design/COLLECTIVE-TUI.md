# The operator surface

Item 5 of [`PRODUCT-BRIEF-collective-v2.md`](PRODUCT-BRIEF-collective-v2.md) Part A §24: the
terminal information architecture, designed from the lifecycle of §21 rather than from the screens
that exist. Every surface is stated three ways — **what the operator sees**, **what actions exist**,
**what each action does**.

Read [`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) first; the boundaries it fixes and the resources
of [`COLLECTIVE-RESOURCES.md`](COLLECTIVE-RESOURCES.md) are assumed here.

## Method and the rules that follow from it

The design starts from §21's list of lifecycle moments and gives each one a surface. §8 fixes the
style: **tables wherever a table works, property views only where it does not, and `list → select →
properties → action` instead of wizard chains.** §23 fixes the ceiling: this is not a Kubernetes
administration console, and the ordinary path stays `$ ymp` → a sentence → a verified result.

Thirteen rules govern every surface below. The first three are the owner's hard rule of 2026-08-15,
taken after the previous command-line release: *no confirmation dialogs, no identifiers typed by the
operator, no acknowledgement steps, no setup wizard.*

1. **Nothing is confirmed and no identifier is ever typed.** A run identifier, a candidate digest, a
   participant identifier and a workspace hash are never operator input. Every act is taken on a
   **selected row**, and the consequences of that act are stated in the row's properties *before* the
   key is pressed — not in a modal after it. What replaces confirmation is that nothing is destroyed:
   the journal, the candidates and the evidence of a cancelled, exported or archived run all stay
   readable.
2. **The operator's deliberate acts are two:** enable a provider, and type a goal. Everything else —
   pause, cancel, clarify, inspect, export, continue, archive — is optional and is a keypress on a
   row. There is no authorization step, no acceptance step and no setup step.
3. **Tables first.** Providers, models, pools, agents, tasks, activity, candidates, budgets,
   failures, engines and history are tables. Property views exist for one provider, one pool, one
   participant, one candidate and one verification, because a single object's facts do not form
   rows.
4. **List → select → properties → action.** No surface asks a question whose answer is another
   question.
5. **No conversational setup wizard** (§6). The one thing the product ever asks to be configured is
   *enable a provider*, and it asks by naming the command.
6. Lead with product language: collective, participant, goal, task, activity, result, verification,
   evidence, provider, model, pool, workspace, budget. Internal words appear only inside diagnostics,
   and never alone.
7. Nothing a participant published is actionable in one keypress (INV-4). Rule 1 makes the
   operator's own acts single keypresses; a participant's suggestion is still not one of them, and
   the two are never the same key.
8. No ranking, scoring or grading anywhere; sorting uses raw mechanical fields (INV-1).
9. Control, collaboration and verification planes stay visually distinct where they meet.
10. Every view holds its own cursor and recovers from the journal; lag is a visible state (INV-6).
11. A terminal state is named exactly; `exhausted` is never drawn as success (INV-7).
12. Budget dimensions never trade against one another on screen (INV-2).
13. Every action here exists as a command of the same executable (node `W1-APP-02n`) — and, under
    rule 1, that command takes its subject from a selection or from the open run, never from a typed
    identifier used as a confirmation.

Two rules are inherited from accepted decisions and are load-bearing rather than stylistic. **All
product state lives under `~/.ymp`** (node `W1-APP-02w.1`), so no surface writes into the project
directory except export. And **the assurance profile never appears without its limit** — which is
why it is absent from startup and present on the block that starts a run, on `/budget` and in `?`.

## Shape

One scrolling conversation fills the screen, with an always-ready input line, a thin context header
and a status line. Data lives on full-screen pages opened from the `/` command line and closed with
`Esc`. There are no decision modals left to place: the two acts that carry weight — enabling a
provider and stating a goal — happen where the operator already is. This is the accepted composition
([`VISUAL_CONCEPT.md`](../VISUAL_CONCEPT.md)); the redesign changes what the surfaces contain, and
the owner's ruling removes the modals it used to carry.

    ┌ ymp · minesweeper · run 0001 · running · $0.42 of $5.00 ───────────────┐
    │                                                                       │
    │   the conversation: operator turns, ymp replies, run events,          │
    │   participant messages — each attributed and typed                    │
    │                                                                       │
    ├───────────────────────────────────────────────────────────────────────┤
    │ > _                                                                   │
    │ 3 participants · verifying cd-2 · /help                               │
    └───────────────────────────────────────────────────────────────────────┘

## The command set

Brief §13 names the minimum. These are the commands, and every one has a command-line twin.

    /providers  /models  /pools  /agents  /tasks  /activity
    /budget     /verify  /result /history /workspace  /help

`/engines` and `/diagnostics` exist and are not advertised in the short help: the first is the
engine level beneath providers, the second is where the internal vocabulary lives.

## Surface map, in lifecycle order

| §21 lifecycle moment | Surface |
|---|---|
| startup | S01 |
| first run | S02 |
| provider discovery | S03 |
| provider enable / authentication | S04 |
| model discovery | S05 (`r` refresh) |
| model catalog | S05 |
| automatic default pool | S35 list · S36 detail |
| task creation | S08 |
| ambiguity / clarification | S09 |
| the run starts | S10 — a transcript block, not a gate |
| collective startup | S11 |
| live collective | S12 |
| `/agents` | S13 list · S14 detail |
| task / activity | S15 tasks · S16 activity |
| recruitment | S37 |
| candidate | S17 list · S18 detail |
| verification | S19 |
| revision after failure | a state of S12 and S19, drawn as one |
| result | S25 |
| evidence | S26 |
| export | S27 |
| pause | S21 |
| resume where supported | S22 |
| cancellation | S23 |
| intervention | S24 |
| budget exhaustion | S30, and S25 in its `exhausted` form |
| infrastructure failure | S20, and S25 in its `infrastructure error` form |
| history | S28 |
| archive | S29 |
| recovery from invalid configuration | S32 |
| — the workspace's standing ceiling (added) | S06, the first tab of `/budget` |
| — workspace selection (added) | S07 |
| — engine level (added) | S31 |
| — help and keys (added) | S33 |
| — diagnostics (added) | S34 |

Thirty-seven surfaces. Three lifecycle moments are drawn as *states* of a surface rather than as
their own screens — revision after failure, budget exhaustion and infrastructure failure — because
each is a state the operator arrives in rather than a place they navigate to, and each is named
exactly where it appears.

**Four surfaces the previous revision had are gone, and their disappearance is the point.** The
setup question that followed the first provider connection, the run-authorization screen with its
typed identifier, the typed cancel confirmation and the continue confirmation are all removed by the
owner's decisions D3, D4 and D10 and by the hard rule above. What each of them carried is kept: the
disclosure consequence is stated on the provider properties view before `Enable`, what *done* means
is stated in the transcript as the run starts, and the consequences of cancelling are stated on the
run's own row.

---

## S01 · Startup

The accepted start screen (node `W1-APP-02e.5`) and brief §5 agree, so the design conforms to both
rather than inventing a third.

**Sees.** The logo, one line of basics, the invitation. Shown once, and never repeated on later
turns.

                       ymp
             /Users/…/Code/minesweeper · 0.1.0

    > state what you want done
    ─────────────────────────────────────────────────────────────────
    /help for commands

**Actions.**

| Action | Effect |
|---|---|
| type a goal | Opens S08. |
| `/providers` | Opens S03. |
| `/workspace` | Opens S07. |
| `/history` | Opens S28. |
| `/help` | Opens S33. |

**Notes.** No provider count, no probe result, no assurance sentence and no store path: the header
carries the workspace and the assurance glyph, and the full assurance text lives in `?` and on the
surfaces where a run's spend is stated. **Nothing is probed at startup**, because a provider is not
autodetected before it is enabled (§7): with nothing enabled, launching ymp opens no process and no
network connection.

## S02 · First run

The state where no provider is enabled. It is the only moment the product asks for anything to be
set up, and it asks for one thing by naming a command — not by starting a dialogue (§6).

**Sees.**

    ymp runs a collective of agents against a goal you state, and returns a result an
    independent check accepted. you do not write the check, pick the models, or build a team.

    to start, enable one provider:  /providers

**Actions.**

| Action | Effect |
|---|---|
| `/providers` | Opens S03. |
| type a goal anyway | Accepted and held. The workspace is read locally and the facts are shown; deriving what *done* means needs a model, so it does not run and nothing leaves the host. The reply names `/providers` and nothing is lost. |

**Notes.** The technical sentence `no providers configured` never appears (§6). An empty catalog is
a state, not an error, and `No AgentPool configured` is not a sentence this product can produce (§4).

## S03 · Providers

Table first, k9s style (§7). The **full supported list is always shown**, whether or not anything is
configured, so the operator sees the whole space rather than the part they have touched.

**Sees.**

    providers ─ 5 supported · 2 ready ────────────────────────────────────────
    PROVIDER    STATE      REACHED BY            MODELS   OBSERVED
    anthropic   ready      claude-code 2.1.227   4        12s ago
    openai      ready      codex 0.147.0         5        1h ago
    nvidia      ready      claude-code · Messages 2       3m ago
    google      disabled   —                     —        —
    local       disabled   —                     —        —

    enabling a provider permits repository content to be sent to it.
    Enter properties · e enable/disable · r re-probe · Esc back

**Actions.**

| Action | Effect |
|---|---|
| `Enter` | Opens S04 for that provider. |
| `e` | Enables or disables. **Enabling is what triggers autodetect**; a disabled provider is never probed. Disabling removes its entries from later pools and leaves running participants alone, and the row says so. |
| `r` | Re-probes an enabled provider and replaces its measured properties and model list with what the probe returned. |
| `Esc` | Back. |

**Notes.** `STATE` is one of `disabled`, `not configured`, `needs authentication`, `ready`,
`unavailable`. The build accepted as P1 writes three of them and does not write `needs
authentication`, because nothing in it authenticates against a provider; the surface therefore does
not claim a state no measurement produced. `OBSERVED` is the age of the observation, which the
accepted build does not yet record — migration unit P2 owes it, and until then the column reads
`—` rather than `just now`.

## S04 · Provider properties

The one place a property view beats a table: a single account's facts.

**Sees.**

    providers › anthropic ────────────────────────────────────────────────────
    state        ready
    reached by   engine claude-code · /usr/local/bin/claude · 2.1.227 · sha256:4c1e…
    credential   read from the engine's own store; ymp holds no copy
    models       4                                        (Enter opens /models)
    disclosure   repository content is sent to anthropic
    conformance  own vendor account · no separate pairing probe required
    observed     12s ago, from build 2.1.227

    Enter models · c connect · e disable · r re-probe · Esc back

**Actions.** `c` connect launches the engine's own authentication in a child process where the
engine owns the credential, or records where the credential is read from and against which endpoint
where the provider is reached by a route override — never copying a credential into ymp. `e`
disables. `r` re-probes. `Enter` opens S05 filtered to this provider.

**Notes.** A pairing that has not passed its conformance probes is shown with the failing capability
named and is not admissible to a pool. In this build every provider is an engine reaching its own
vendor, so no entry claims a conformance result it does not have.

**Enabling is the disclosure consent, and the consequence is stated above the action.** Decision D4
places the consent here and nowhere else: there is no per-workspace question and no per-run
acknowledgement. A provider that is not yet enabled shows the sentence in the same properties view
that carries the `e` key, so the operator reads it before pressing and not after:

    providers › nvidia ───────────────────────────────────────────────────────
    state        disabled
    reached by   engine claude-code · Messages endpoint · pairing probed separately
    disclosure   enabling sends repository content from any workspace to nvidia,
                 including the bounded excerpts ymp uses to work out what "done"
                 means before a run starts
    e enable · Esc back

Pressing `e` enables the provider, records the disclosure class and starts the autodetect. Nothing
is confirmed and nothing is acknowledged: the sentence above the key **is** the disclosure notice,
which is what D4 decided and what rule 1 requires. A workspace may later exclude a provider as an
explicit policy on S06, but it is never asked to.

## S05 · Model catalog

The level that answers "what could be used", never "what is running".

**Sees.**

    models ─ 11 entries · 6 admissible ───────────────────────────────────────
    PROVIDER   ENGINE       MODEL              STATE        LABELS
    anthropic  claude-code  claude-opus-5      admissible   reasoning
    anthropic  claude-code  claude-sonnet-5    admissible   reasoning
    anthropic  claude-code  claude-haiku-4-5   admissible   fast, cheap
    nvidia     claude-code  nemotron-…         admissible   reasoning
    openai     codex        gpt-5.6-sol        not offered  reasoning
      ↳ engine codex is disabled — usage limit until 2026-09-12

    nothing here is an agent. a participant exists only when one is recruited and paid for.
    Enter properties · p add to a pool · r refresh · Esc back

**Actions.**

| Action | Effect |
|---|---|
| `Enter` | Properties of the entry: engine version and digest, wire protocol, account scope, disclosure class, conformance result, and which runs used it. |
| `p` | Adds the entry to a pool, or removes it. Removing an entry a running participant occupies does not stop it; the change reaches pools frozen after it. |
| `r` | Re-measures the model list of the entry's engine. |

**Notes.** The line about agents is permanent rather than a hint: it is the sentence §1 spends a page
on, said where it would otherwise be misread. Labels are resource groupings a participant may read;
they are never a role, and no code path turns a label into a selection (§16). An entry that is not
offered is present and carries its measured reason — a catalog that hid it would answer "why is this
model not available" with silence.

## S06 · The workspace's standing ceiling

The first tab of `/budget`. Decision D3 makes this the human-approved bound: a goal starts
immediately and spends inside it, and the operator is never asked to authorize a run. Decision D8
sets the placeholder values, so the surface is never a prerequisite — it is where an operator who
wants a different bound goes to set one.

**Sees.**

    budget · workspace minesweeper ── standing ceiling │ run 0001 ────────────
    FIELD                 VALUE                                    SOURCE
    spend ceiling         $5.00                                    default (D8)
    wall clock            2h                                       default
    participants          up to 6                                  pool capacity
    concurrent attempts   up to 3                                  pool capacity
    verification queries  4                                        default
    questions             up to 3                                  default
    derivation allowance  $0.10 per goal                           default
    pool                  default · 6 admissible entries           automatic
    disclosure            anthropic, openai, nvidia                from enabled providers
    external actions      none                                     default
    assurance             poc_process_isolation · not hostile-code containment
    spent so far          $2.18 of $5.00 · headroom for the next goal

**Actions.** `Enter` edits a row in place — a narrowing applies at once, a widening states what it
permits on the row itself; `Tab` switches to the live run's dimensions (S30); `p` opens S35;
`Esc` back.

**Notes.** Editing a row is not a confirmation and asks for nothing: the new value is typed into the
row and takes effect, and the previous value stays in the transcript. This surface contains no field
that says which model suits which task, and none may be added: that sentence has no representation
in the kernel.

The standing ceiling is the reason there is no authorization screen. It bounds *how much*; the goal
sentence itself states *what*. Both are the operator's own acts, and the constitutional requirement
that a human approves the definition of done is met by the pair rather than by a modal
([`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) item 2).

## S07 · Workspace

**Sees.**

    workspace ─ current: minesweeper ─────────────────────────────────────────
    NAME          PATH                              TASKS   LAST RESULT
    minesweeper   /Users/…/Code/minesweeper         1       —
    ymp           /Users/…/Code/ymp                 7       12m ago · verified

**Actions.** `Enter` switches the workspace for this session and re-reads its tasks and boundaries;
`Esc` back. Switching writes nothing in either project directory.

## S08 · Task creation

**Sees.** Two visibly different stages: the local reading, which is free, and the derivation, which
draws on the derivation allowance and names the provider it discloses to. The run's own budget is
untouched throughout.

    > Создай браузерную игру сапер

    ymp  reading the workspace… 3 files · empty project · no tests · git clean
         (local · nothing left this host)

         working out what "done" means… sending bounded excerpts to anthropic
         $0.02 of the $0.10 derivation allowance

         the goal, as I read it:
           a minesweeper game that runs in a browser, playable from a single page
         checkable: the page loads, a grid renders, a first click never loses,
                    flags toggle, a cleared board reports a win
         one part is not mechanically checkable and stays yours to judge: "looks good"

    Task created · Collective starting…

**Actions.**

| Action | Effect |
|---|---|
| nothing | The run starts. There is no authorization step: the standing ceiling (S06) is the approved bound and the goal sentence is the statement of what is wanted (decision D3). S10 states what was created. |
| type another line | Before the run starts, amends the goal and re-derives from the same allowance. After it starts, the line is a message to the collective (S24) and the surface says which of the two it is. |
| `/budget` | Opens S06 to change the standing ceiling. |
| `/cancel` | Ends the run from its row (S23). |

**Notes.** The operator is never asked here for a verifier, a negative control, a task breakdown, a
team, a model or a pool. The advanced amendment grammar survives as an override for an operator who
wants one and is never required, offered or implied. The goal is carried in the operator's own words
and is never translated or rewritten.

## S09 · Clarification

The only thing the product ever asks the operator, and only for an assumption that materially
changes the result and cannot be settled from the request, the repository, the documentation, the
visible checks, the policy or discovery (§10). Decision D6 fixes the shape: assume wherever an
assumption is safe, ask only about genuine intent ambiguity, at most three questions per run.

**Sees.**

    ymp  one thing changes what "done" means and I cannot settle it from the project:

         should a first click be guaranteed safe, as in the classic game?

         [1] yes — the first click never uncovers a mine; the board is generated after it
         [2] no  — the board is fixed before the first click
         [3] tell me in your own words

         questions used: 1 of 3

**Actions.**

| Action | Effect |
|---|---|
| choose a numbered option, or type an answer in prose | Recorded as provenance class E, folded into the requirements, derivation reruns and the run starts. The run is **not** marked intervened. |
| `/skip` | ymp proceeds under the assumption it states, records it as class C, and shows it on S10 and in the result. |
| leave it | The run does not start while a material question is unanswered, and the line says which one it is waiting for. An unattended run that is never answered ends `NEEDS CLARIFICATION` (decision D6) with the question published; answering it later and pressing `Continue` resumes from there. |

**Notes.** The question is never "create an acceptance oracle, verifier or contract"; §10 names that
shape as the one that must not be asked, and no code path may produce it. The numbered options are a
selection, not an identifier: choosing `[1]` is one keypress and nothing is typed back to the
product. Questions are bounded by a budget dimension, so asking more is not a way to avoid deciding.

**The mid-run form.** The same surface appears during a run when a participant meets a material
ambiguity and spends a clarification unit. The answer reaches the collective as attributed
collaboration data and does not change what the candidate is judged against, because a run is bound
to one contract.

**The divergence state.** When a mid-run answer contradicts a recorded requirement — or cannot be
classified, which is treated the same way — this surface returns with the divergence instead of
letting the run continue quietly.

    your answer changes something this run is already being judged against

      R2 (from your goal)  a first click never uncovers a mine
      your answer          the board is fixed before the first click

    until you choose, no verification query is spent.

    [1] it does not change what "done" means — continue, the answer stays advisory
    [2] it does change what "done" means — end this run and continue with the amended goal

Choosing `[2]` records `cancelled`, which is the honest terminal for an operator's decision, and
`Continue` opens a new run against the amended goal with this run's candidates, journal and findings
still readable. Leaving it unanswered holds the acceptance path: the run keeps working and no
verification query may be spent against the diverged requirement set.

## S10 · The run starts

Not a gate. Decision D3 removed the authorization screen: the goal sentence states *what*, the
standing ceiling (S06) bounds *how much*, and the run starts on the sentence. What the screen used
to ask for is now stated — in the transcript, as information the operator reads while the collective
is already working.

**Sees.** One block appended to the conversation, immediately after the derivation lines of S08.

    ymp  run 0001 · started
         done means   5 observable requirements · 3 from your goal, 1 from the project,
                      1 assumption · 1 part is not mechanically checkable and stays yours
         spends from  your standing ceiling · $2.82 of $5.00 left · 2h · 6 participants
                      4 verification queries
         pool         default · 6 entries · frozen sha256:2b91…
         disclosure   repository content goes to anthropic, nvidia
         assurance    poc_process_isolation · not hostile-code containment
         derivation   working out what "done" means cost $0.02

**Actions.**

| Action | Effect |
|---|---|
| `Enter` on *done means* | Expands the requirements as plain sentences with their provenance marks. Reading is optional and changes nothing; there is no item to approve. |
| `d` | Opens S34, where the requirement-to-evidence coverage map, the generated checks and their digests are available in full. |
| `/budget` | Opens S06. A ceiling narrowed now applies to this run from the moment it is narrowed. |
| `/cancel` | Ends the run (S23). This is what replaces "discard": the run exists, and ending it is an ordinary act on its row. |

**Notes.** Three refusals still stop a run from starting, and every one of them is ymp's own limit
rather than a request for authorship: an unanswered material question (S09), a derived plan that
failed its negative control, and an empty pool. Each states what the operator can do — answer, state
a narrower goal, enable a provider — and none asks for a check, a model or a confirmation. **A
refusal never names a runtime profile**, because which engine runs the work is not the operator's
decision.

The one thing this block must never become is a screen that waits. It is appended and the collective
starts; an operator who reads none of it loses nothing except knowledge.

## S11 · Collective startup

**Sees.** Four lines in the transcript, so the operator watches a collective being created rather
than a spinner.

    run 0001 started · $0.00 of $5.00
    pool frozen · 6 entries · sha256:2b91…
    participant A · anthropic · claude-code · claude-opus-5 · started
      the first participant is created by ymp from the pool's declared order;
      every further participant is recruited by the collective itself
    A is reading the workspace

**Actions.** None required. `/agents`, `/activity`, `/tasks` and `/budget` are available.

**Notes.** Exactly one participant is created here. The entry is the first entry of the frozen pool,
in declared order, that admission found live — a mechanical rule that reads no property of the goal
([`COLLECTIVE-DESIGN.md`](COLLECTIVE-DESIGN.md) item 8).

## S12 · Live collective

The main surface for the duration of a run.

**Sees.** One transcript carrying four kinds of entry, each distinguishable at a glance: operator
turns, ymp replies, run events, and participant messages attributed by author and typed by kind.

    14:02  A  observation   an empty project; nothing to build on
    14:03  ⚙  recruitment   A requested a participant · admitted · B started
                            anthropic · claude-code · claude-sonnet-5
    14:06  B  challenge     a first-click guarantee needs the board generated after the click
    14:07  A  confirmation  agreed — generating after the first reveal
    14:11  ⚙  candidate     cd-1 submitted by A
    14:12  ⚙  verification  cd-1 · query 1 of 4 · running

**Actions.** scroll pauses following and the header says so; `End` resumes; `/agents` `/tasks`
`/activity` `/verify` `/budget` open their pages; typing a message opens S24; `/pause` and `/cancel`
open S21 and S23.

**Notes.** A participant's message is inert: it can be read, cited and inspected, and nothing in it
executes with one key. No entry is ranked, scored or highlighted as more important.

**Revision after failure** is a state of this surface and not a separate screen: a failed verdict
appears as an event, the run keeps running, and the next line is the collective reacting.

    14:31  ⚙  verification  cd-1 · failed · 2 of 5 requirements not observed
                            the run is not over — the collective has the same diagnostic
    14:33  B  decision      taking the flag toggle; A revises the first-click generator

## S13 · Agents

**Sees.** Who is working and on what, with mechanical columns only.

    agents ─ 3 active · 1 finished ───────────────────────────────────────────
    ID  STATE      ENTRY                                 TASK          SPENT
    A   working    anthropic·claude-code·opus-5          root goal     $0.31
    B   working    anthropic·claude-code·sonnet-5        t-2 flags     $0.09
    C   yielded    nvidia·claude-code·nemotron-…         t-3 review    $0.04
      ↳ waiting for the verification result of cd-1
    D   returned   anthropic·claude-code·haiku-4-5       t-1 survey    $0.07

**Actions.** `Enter` opens S14; `Tab` switches active and finished; `Esc` back.

**Notes.** There is no action here that assigns work, changes a model, promotes, ranks or stops one
participant. The operator is the principal and not the team manager (§13), and the TUI is not a
per-agent control panel. Sorting is by identifier or by a raw mechanical column (INV-1).

## S14 · Participant properties

**Sees.** Exactly what §14 lists, and nothing more.

    agents › B ───────────────────────────────────────────────────────────────
    identity     B · principal pr-b
    entry        anthropic · claude-code · claude-sonnet-5
    recruited    by A at 14:03 · admitted: entry in pool, 4 participant starts left
    state        working · attempt 2 · slice 3 of 8
    budget       $0.41 left · 38 min left · 1 attempt start · 5 slices
    holds        t-2 flag toggle · taken 14:04 · not returned
    published    3 observations · 1 challenge
    submitted    cd-1 at 14:11
    verification requested cd-1 · failed · diagnostic below
    recruited    nobody
    failures     none · yields 1 · cancellations none

**Actions.** `Enter` on a row opens the corresponding surface; `d` opens S34 for the internal record;
`Esc` back.

**Notes.** No private chain-of-thought is shown, requested or stored — only published summaries and
externally visible actions (§14). Collaboration messages are untrusted collaboration data and the
existing observation policy holds.

## S15 · Tasks

**Sees.** The work the collective created.

    tasks ─ 4 · 2 open ───────────────────────────────────────────────────────
    ID    WORK                        SPONSOR  HOLDER  STATE
    root  browser minesweeper         ymp      A       open
      t-1 survey the browser target   A        D       returned · result
      t-2 flag toggle                 A        B       open
      t-3 review cd-1                 A        C       open

**Actions.** `Enter` describes a task — what it asks for, its base, what funds it, what it depends on
and its return; `d` shows the obligation and escrow rows; `Esc` back.

**Notes.** Indentation is recorded parentage, never priority. The words *obligation* and *escrow*
appear only under `d`.

## S16 · Activity

The collaboration plane, labelled untrusted wherever it is drawn.

**Sees.**

    activity ─ 27 messages · untrusted collaboration data ─────────────────────
    TIME   FROM  KIND          AUDIENCE   SUMMARY
    14:06  B     challenge     t-2        a first-click guarantee needs late generation
    14:07  A     confirmation  t-2        agreed — generating after the first reveal
    14:09  C     question      cd-1 rev   which requirement does the new branch serve?

**Actions.** `Enter` opens the full message with its references and delivery receipts; `f` filters by
author, kind or task; `Esc` back.

**Notes.** A delivery receipt proves that bytes were made available, not that a model read or
believed them, and the surface says so. No message is a button.

## S37 · Recruitment

§21 asks for recruitment to be visible as its own moment, and it is: the point where a semantic
decision by the collective meets a mechanical check by the kernel.

**Sees.**

    recruitment ─ 3 requests · 2 admitted ────────────────────────────────────
    TIME   BY  ENTRY                              DECISION   MECHANICAL REASON
    14:03  A   anthropic·claude-code·sonnet-5     admitted   in pool · 5 starts left
    14:18  B   openai·codex·gpt-5.6-sol           refused    entry not in the frozen pool
    14:22  A   nvidia·claude-code·nemotron-…      admitted   in pool · 4 starts left

**Actions.** `Enter` shows the request in full: the entry, the requester's own funded proposal
allowance, the checks the kernel performed and the fact it committed; `Esc` back.

**Notes.** The `MECHANICAL REASON` column is the guarantee made visible: every entry in it is a
containment, a count or an availability fact. There is no column for why this model and not another,
because the kernel does not have that reason — the collective does, and it publishes it on S16 as an
ordinary message.

## S17 · Candidates

**Sees.**

    candidates ─ 2 ───────────────────────────────────────────────────────────
    ID    FROM  BASE   DIGEST         VERIFICATION
    cd-1  A     S0     sha256:1a4f…   failed · 14:31
    cd-2  B     cd-1   sha256:9f2c…   passed · 14:52

**Actions.** `Enter` opens S18; `Esc` back.

**Notes.** No candidate is presented as ready to apply, and none is ranked or recommended. Where
several pass, the accepted set is presented and the product invents no tie-breaker.

## S18 · Candidate properties

**Sees.**

    candidates › cd-2 ────────────────────────────────────────────────────────
    digest       sha256:9f2c…
    base         cd-1 · sha256:1a4f…
    submitted    B · 14:48 · attempt 2
    changes      4 files · +212 −38
    verification passed · 14:52 · 5 of 5 requirements observed
    evidence     contract sha256:… · environment sha256:… · oracle sha256:…

**Actions.** `Enter` on `changes` shows the difference; `Enter` on `verification` opens S19; `e`
opens S26; `Esc` back.

## S19 · Verification

**Sees.** Three states, drawn differently.

*Running:* `verifying cd-2 · query 2 of 4 · requested by B · started 14:49`.

*Failed:*

    verification › cd-1 · failed ─────────────────────────────────────────────
    class       behavioural check did not hold
    diagnostic  2 of 5 requirements were not observed:
                  R2 a first click never uncovers a mine — uncovered one in 3 of 50 trials
                  R4 a cleared board reports a win — no win state reached
    disclosure  bounded by this run's policy; protected material is not shown
    the collective has this same diagnostic and is reacting. the run is not over.

*Passed:*

    verification › cd-2 · passed ─────────────────────────────────────────────
    R1  A  the page loads and renders a grid              observed by check k-1
    R2  A  a first click never uncovers a mine            observed by check k-2
    R3  A  flags toggle on right click                    observed by check k-3
    R4  B  a cleared board reports a win                  observed by check k-4
    R5  C  the page needs no network at run time (assumption)  observed by check k-5
    ─   A  "looks good" is not mechanically checkable — yours to judge
    digests  contract sha256:… candidate sha256:… environment sha256:… oracle sha256:…

**Actions.** `Enter` on a requirement shows its provenance and the check that observed it (S34 for
the check's own bytes and digest); `Esc` back.

**Notes.** Protected material never appears, and the diagnostic is exactly what the disclosure policy
allows — unbounded feedback would let a candidate be fitted to a fixed check. **A verifier
infrastructure failure is drawn as its own state and is never a rejection.**

## S20 · Failures

Everything that goes wrong short of a terminal, including the infrastructure failures §21 asks to be
visible.

**Sees.**

    failures ─ 3 ─────────────────────────────────────────────────────────────
    TIME   WHAT                    DETAIL
    14:19  B · invocation ended    model route error · upstream 529 · retried
    14:22  cd-1 · integration      conflict against base cd-0 · became task t-4
    14:41  verification · verifier the browser harness did not start · not a rejection

**Actions.** `Enter` describes one failure with the exact recorded reason; `Esc` back.

**Notes.** A model route error is not evidence that the task is unsolvable, and the surface says so
where it appears. A failure here is never presented as a terminal state, and a verifier's own failure
is never counted against a candidate.

## S21 · Pause

**Sees.** The run's own row in `/history` and the live header carry the pause key, and the run's
properties state what pausing does before the key is pressed. There is no modal.

    history › run 0001 · running ─────────────────────────────────────────────
    pause (p)  running slices finish; no new slice is admitted
               leases keep running out, so a paused run can still expire
               spend already committed is not returned

**Actions.** `p` pauses the selected run at once; `p` again resumes it (S22).

**Notes.** Pause does not mark the run intervened: it is authority over resources, not content. The
consequences are stated above the key rather than in a dialogue after it, which is rule 1.

## S22 · Resume

**Sees.** What a paused run would resume with: participants waiting, remaining budget by dimension,
and the wall clock lost to the pause.

**Actions.** `Enter` resumes — admission restarts and each eligible yielded slice becomes eligible
again, consuming its own invocation-start unit; `Esc` closes. Where a lease expired during the pause
the surface names the task and states that it must be re-taken rather than resumed, which is what
"resume where supported" means honestly.

## S23 · Cancellation

The act with the largest consequence, and therefore the one where rule 1 has to be argued rather
than assumed. No identifier is typed and no dialogue appears: the run is a **selected row**, the
consequences are stated on it, and the key ends it.

**Sees.** The properties of the selected run, with the cancel key beneath what it will do.

    history › run 0001 · running ─────────────────────────────────────────────
    cancel (x)  3 participants are interrupted; 2 attempts are lost
                $0.42 of spend is not returned
                the journal and every published candidate stay readable
                the recorded outcome will be cancelled

**Actions.** `x` cancels the selected run: the cancel command reaches the kernel, processes are wound
down and the run reaches `cancelled`.

**Notes.** What makes a single keypress defensible here is that **nothing is destroyed**: the
journal, every candidate, every published finding and the evidence survive a cancellation and stay
readable from `/history`. The irreversible part is the spend already committed, and that is stated
above the key. `cancelled` and `infrastructure error` are different facts and are never substituted;
a runtime that dies while being stopped does not change the recorded outcome.

An accidental `x` costs the work in flight. That is the trade the owner's rule takes deliberately
against the cost of an identifier typed on every deliberate cancellation, and it is recorded here as
a trade rather than as a free lunch. The mitigations that survive are real: the key is on the run's
properties view rather than on the list, `Continue` re-opens the work from the last candidate, and
the accepted rule that a cancelled run is recorded honestly means nothing is silently lost.

## S24 · Intervention

**Sees.**

    message the collective
      your message enters the collaboration plane as untrusted data
      it grants no authority and cannot form a contract, choose a bid or spend budget
      the run will be marked intervened and excluded from autonomous comparison
    to: t-2 (task audience) · all participants
    > _

**Actions.** `Enter` sends and marks the run intervened; `Esc` discards.

**Notes.** An answer to a question ymp asked (S09) is *not* an intervention and does not pass through
this surface. The distinction is drawn on both surfaces so neither is mistaken for the other.

## S25 · Result

What the operator got, before how it was made. §15 fixes the headline: `✓ VERIFIED`, never
"agents stopped".

**Sees.**

    run 0001 · ✓ VERIFIED ────────────────────────────────────────────────────
    a browser minesweeper: one page, a 9×9 board with 10 mines, safe first click,
    flag toggling, and a win state when every safe cell is uncovered.

    verification   5 of 5 requirements observed
                   1 part of the goal is not mechanically checkable — yours to judge
    participants   3 · 5 attempts · 2 candidates
    time / spend   41 min · $2.18 · 214k tokens across 2 models
    candidate      cd-2 · sha256:9f2c…
    Enter result · e evidence · x export · c continue · a archive

**Actions.**

| Key | Effect |
|---|---|
| `Enter` | Opens S18 for the accepted candidate. |
| `e` | Opens S26. |
| `x` | Opens S27, the candidate's properties with the export key and the exact paths. |
| `c` | **Starts the next run at once**, with the accepted candidate as its base and the previous goal as context, while the standing ceiling has headroom (decision D10). The input line opens for the next sentence; nothing is confirmed. |
| `a` | Archives (S29). |

**The other terminals**, each with its own sentence and its own actions, and never drawn as success:

    run 0002 · BUDGET EXHAUSTED
      the money ceiling ran out at $5.00 · 1 candidate exists and was never verified
      Enter inspect · b raise the standing ceiling on /budget · a archive

    run 0003 · INFRASTRUCTURE ERROR
      the verifier's environment could not be established — this is a failure of the
      machinery, not of the work
      Enter journal · e export evidence · a archive

    run 0004 · CANCELLED
      you ended it at 14:22 · 2 attempts were interrupted · spend is not returned
      Enter inspect · a archive

    run 0005 · NEEDS CLARIFICATION
      the collective stopped: a question about the intended scoring rule could not be
      answered from the project and the question budget is spent
      [1] … [2] … answer and it continues · Enter inspect · a archive

`NEEDS CLARIFICATION` is the outcome §15 names and decision D6 requires — never reported as
`abstained` and never as a success. It is the one terminal that carries a question on its face, and
answering it resumes the work rather than starting a new lineage.

`BUDGET EXHAUSTED` is the only place the product asks the operator for a decision about money, and
even there it asks by naming `/budget` rather than by opening a dialogue: raising the standing
ceiling is an edit on a row, and the run continues from the last candidate.

## S26 · Evidence

**Sees.** Everything supporting the claim, in one place: the four digests; the
requirement-to-observation map with provenance; the per-model spend; participants and their attempts;
candidate ancestry; the journal head digest; the frozen pool with its digest; and the assurance
profile with its limit.

**Actions.** `Enter` descends into any row; `x` writes the evidence bundle to a path the operator
names, outside the project directory by default; `Esc` back.

**Notes.** The bundle contains no protected oracle material, no capability material and no runtime
session capsule.

## S27 · Export

The only surface that writes into the project directory. Export is a **delivery**, not storage: the
product's own state stays under `~/.ymp`.

**Sees.** The selected candidate's properties, with the exact paths and the working-tree state above
the export key. No dialogue, and no digest typed.

    candidates › cd-2 ── export (x) ──────────────────────────────────────────
    into          /Users/…/Code/minesweeper
    working tree  clean
    writes        4 files: index.html (new) · game.js (new) · style.css (new) · README.md
                  nothing else in the directory is touched

**Actions.** `x` applies the candidate and records the export as an operator act. A working tree with
uncommitted changes is stated on the same rows, and the operator decides by pressing or not pressing.

**Notes.** No candidate is presented as ready to apply before its properties are opened
deliberately, which is what keeps the export key off a list row. Export is the only write into the
project directory; the paths are exact and stated before the key, and the project's own version
control is what makes the write reversible.

## S28 · History

**Sees.**

    history ─ workspace minesweeper · 5 runs ─────────────────────────────────
    RUN   GOAL                        OUTCOME                SPEND   WHEN
    0001  Создай браузерную игру…     ✓ verified             $2.18   12m ago
    0002  Создай браузерную игру…     budget exhausted        $5.00   2h ago
    0003  add a timer                 cancelled · intervened  $0.94   1d ago

**Actions.** `Enter` opens that run's result surface read-only; `f` filters, including archived runs;
`Esc` back.

**Notes.** `budget exhausted` names the dimension that ran out. `intervened` is shown wherever an
outcome is reported, because it changes what the run's evidence may be used for.

## S29 · Archive

**Sees.** The selected terminal run's row, with one line stating what archiving does: it removes the
run from default views and rewrites nothing.

**Actions.** `a` archives the selected run. Archived runs stay readable from S28 with a filter, and
their journals, candidates and evidence are untouched. `a` again un-archives, which is why this act
needs nothing stated beyond the line above it.

## S30 · The live run's budget

The second tab of `/budget`. Every dimension on its own line. No total, no percentage of "progress",
no dimension expressed in terms of another.

**Sees.**

    budget · workspace minesweeper ── standing ceiling │ run 0001 ────────────
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
    clarification requests 3          1         2       yes
    external actions       0          0         0       yes

**Actions.** `Enter` shows where a dimension was spent, by participant; `Tab` switches back to the
standing ceiling (S06), where a value is edited in place; `Esc` back.

**Notes.** `ENFORCED` is not decoration: a ceiling is strict only where the driver, broker or provider
can enforce it, and an observational dimension says so rather than implying a hard limit. **Budget
exhaustion** is reached here and reported on S25 as its own terminal, never as a result.

## S31 · Engines

The level beneath providers: what ymp can actually start on this host. Not advertised in the short
help, because the operator's mental model is provider and model.

**Sees.**

    engines ─ 2 · 1 ready ────────────────────────────────────────────────────
    ENGINE       STATE     VERSION            EXECUTABLE                    MODELS
    claude-code  ready     2.1.227            /usr/local/bin/claude · 4c1e…  4
    codex        disabled  codex-cli 0.147.0  /usr/local/bin/codex · 8b0d…   5
      ↳ usage limit until 2026-09-12

**Actions.** `e` enables or disables an engine; `r` re-probes; `Enter` describes measured properties,
capability matrices and the admission chain; `Esc` back.

**Notes.** The interface reads the registry; it constructs no driver. A disabled engine is neither
admitted nor offered, and its models are marked not offered in `/models` with this exact reason —
which is how one recorded operator decision reaches every surface without being repeated in any of
them.

## S32 · Recovery from invalid configuration

**Sees.** One row per obstacle, with the exact obstacle and one thing to do. Nothing is hidden behind
a generic failure.

    ymp cannot start a run in this workspace
      no provider is enabled            → /providers · enable one
      pool default has no admissible    → /models · every entry's engine is disabled
      entry
      engine claude-code is 2.0.9       → the pinned profile expects 2.1.227; update or
                                          disable the engine
      the product root is not writable  → ~/.ymp is owned by another account

**Actions.** Each row's `Enter` opens the surface that fixes it; `r` re-probes everything.

**Notes.** Refusal rather than degradation is the accepted rule for admission (node `W1-APP-02q`): a
profile that cannot enforce what it claims is ineligible, not silently weaker. The surface never
offers to proceed without the property.

## S33 · Help and keys

**Sees.** The command list with one line each, and the key overlay for the current surface. The full
assurance sentence lives here, per the accepted start screen.

**Actions.** `/` opens the command line with completion; `?` toggles the key overlay; `Esc` closes.
Every command shown here has a command-line twin of the same name (node `W1-APP-02n`).

## S34 · Diagnostics

Where the internal vocabulary lives. Nothing here is required to use the product, and nothing here is
hidden from an operator who asks.

**Sees.** Grouped by plane.

- **Contract** — the internal contract as stored: requirements with provenance classes, the
  acceptance plan with each check's kind, digest and the requirement it observes, the frozen pool by
  digest, the environment manifest, and the requirement-to-evidence coverage map that used to be the
  authorization screen.
- **Derivation** — the derivation run itself: its participants, its budget, what it read, what it
  disclosed and to whom, and the classifier's verdict on every mid-run answer.
- **Control** — obligations, leases and fencing generations, escrow movements, capability grants, the
  command log with idempotency keys.
- **Collaboration** — audiences, membership grants and their expiry, projection lifetimes,
  communication charges.
- **Verification** — query reservations, the disclosure policy in force, the negative-control and
  substituted-entry-point results that admitted each check.
- **Runtime** — invocations, attempt sandboxes, admitted programs with roles and digests, the
  environment each managed process was given, per-model usage evidence.
- **Journal** — sequence, head digest, cursor and lag per view.

**Actions.** `Enter` descends; `x` exports the selected record into the evidence bundle; `Esc` back.

**Notes.** This is how the design keeps two promises at once: the operator's path never mentions an
obligation or an oracle bundle, and no fact is unreachable. Protected oracle bytes, capability
material and runtime session capsules are not here and are not anywhere the operator can reach.

## S35 · Pools

The advanced surface §16 asks for, and never a prerequisite.

**Sees.**

    pools ─ 2 ────────────────────────────────────────────────────────────────
    NAME       MODELS                                        TRACKING  STATE
    default    all admissible · 6 entries                    yes       ready
    cheap      claude-haiku-4-5, claude-sonnet-5             no        ready

    a pool is what the collective may use, not who works. it assigns no roles.
    Enter properties · n new · d delete · Esc back

**Actions.** `Enter` opens S36; `n` creates a pool from selected catalog entries; `d` deletes a pool
no task names; `Esc` back.

**Notes.** `default` exists because a provider was enabled and its models were discovered — it is
never created by the operator and never absent while a provider is ready (§4). `TRACKING` states
whether the pool still follows the catalog: `default` does until it is edited, and an edited pool
holds the explicit list the operator left. The `cheap` pool shown here is the owner's recorded
experiment boundary — the cheaper Claude routes — and it is a resource boundary, not a team:
`architect-pool`, `coder-pool` and `reviewer-pool` are shapes this product does not offer (§16).

## S36 · Pool properties

**Sees.**

    pools › default ──────────────────────────────────────────────────────────
    models            all admissible entries · tracking the catalog
      anthropic  claude-code  claude-opus-5      admissible
      anthropic  claude-code  claude-sonnet-5    admissible
      anthropic  claude-code  claude-haiku-4-5   admissible
      nvidia     claude-code  nemotron-…         admissible
      openai     codex        gpt-5.6-sol        not offered · engine disabled
    capacity          up to 6 participants · up to 3 concurrent attempts
    limits            per participant: $2.00 · 30 min
    disclosure        anthropic, nvidia
    assurance         poc_process_isolation · not hostile-code containment
    digest            sha256:2b91…  (what a run freezes)

**Actions.** `Enter` on a row edits it — editing the model list replaces tracking with the explicit
list and the surface says so before it applies; `p` opens S05 to add entries; `Esc` back.

**Notes.** The order of the list is the declared order, and it is the order bootstrap uses to pick
the entry a run ignites on ([decision D2](COLLECTIVE-OWNER-DECISIONS.md#d2--the-entry-rule-for-the-origin-participant)).
That is stated here so it is never mistaken for a preference about quality: it names an ignition
point and nothing else, and an operator who wants a different one reorders the list rather than
answering a question. Editing a pool never disturbs a running run, which holds its own frozen
snapshot.

---

## The acceptance scenario of §22, screen by screen

The end-to-end scenario, driven through these surfaces. Operator acts are marked ▶. Nowhere in it
does the operator create a contract, an oracle, a verifier, a team, an agent, a role, a model
assignment or a decomposition — which is the test §22 sets.

| # | What happens | Surface | What the operator does |
|---|---|---|---|
| 1 | ▶ `$ ymp` in an empty project directory | **S01** logo, `/Users/…/Code/minesweeper · 0.1.0`, the prompt. Nothing probed, nothing disclosed | launch |
| 2 | no provider is enabled, so the product states the one thing it needs | **S02** two sentences and `/providers` | reads |
| 3 | ▶ `/providers` | **S03** the full supported list: anthropic, openai, nvidia, google, local — every one `disabled` | opens |
| 4 | ▶ enable anthropic, openai, nvidia | **S03** `e` on each. Enabling is what triggers autodetect; each provider is probed as it is enabled | three keystrokes |
| 5 | catalog fills from the probes | **S05** 11 entries · 6 admissible. The codex entries read `not offered · engine disabled — usage limit until 2026-09-12`, which is a recorded operator decision reaching a surface rather than a failure | may look |
| 6 | the `default` pool appears | **S35** `default · all admissible · 6 entries · tracking`. The operator is never asked to create it and never sees `No AgentPool configured` | nothing |
| 7 | ▶ `Создай браузерную игру сапер` | **S08** the local reading is shown first and costs nothing; then the derivation names anthropic and the allowance it draws on | types one sentence |
| 8 | one assumption cannot be settled | **S09** "should a first click be guaranteed safe?" with concrete options | ▶ presses `1` — class E, not an intervention |
| 9 | the run starts on the sentence | **S10** a transcript block: what *done* means, what it spends from, the frozen pool, the disclosure, the assurance. **No confirmation, no identifier, nothing to acknowledge** (decision D3) | nothing |
| 10 | bootstrap | **S11** run created, pool frozen with its digest, participant A started on the first entry of the declared order, root obligation created | watches |
| 11 | A analyses the empty project | **S12** attributed events | watches |
| 12 | A decides more expertise is useful and requests a participant | **S37** the request, the entry it named, and the mechanical reason it was admitted | watches |
| 13 | the kernel checks containment against the frozen pool, availability, disclosure class, assurance, participant starts and concurrency; B starts | **S13** two participants with their entries | watches |
| 14 | A advertises a scoped offer; B bids; A awards; contract, obligation and lease form atomically | **S15** tasks | watches |
| 15 | they investigate, publish findings, disagree and resolve | **S16** attributed, labelled untrusted | watches |
| 16 | one implements and submits; the integrator builds an immutable candidate | **S17** cd-1 | watches |
| 17 | the other reviews as a separate blinded assessment | **S16**, **S18** | watches |
| 18 | a participant spends a verification query on the exact digest | **S19** `verifying cd-1 · query 1 of 4` | watches |
| 19 | verification fails | **S19** verdict, failure class, bounded diagnostic. The run stays alive and the obligation stays open | watches |
| 20 | the collective receives the same bounded diagnostic and revises | **S12** in its revision state | watches |
| 21 | a new candidate is submitted and verified; it passes | **S19**, then **S25** | watches |
| 22 | ▶ the result | **S25** `✓ VERIFIED` with the summary, verification count, participants, time, spend, candidate digest and five keys | presses `Enter`, `x` or `c` — or nothing |

**Operator acts in total:** launch, enable three providers, type one sentence, press one option key
for the clarification — then, optionally, one key to inspect, export or continue. **Five acts before
the result, no confirmation, and not one identifier typed.** None of them is authorship.

Reduced further, that is the product §23 asks for:

    $ ymp
    > Создай браузерную игру сапер

    [collective works]

    ✓ VERIFIED

**The negative half — three of them, and one is this design's own predecessor.**

- *The current build* fails at step 4, 7, 9 and 10: it asks which agent does the work
  ([`attempt.rs:143-151`](../../ymp-rust/crates/ymp-tui/src/attempt.rs)), refuses a goal that states
  no acceptance condition
  ([`contract.rs:183-194`](../../ymp-rust/crates/ymp-application/src/contract.rs)), makes the
  operator type an identifier to authorize
  ([`surface.rs:78-84`](../../ymp-rust/crates/ymp-cli/src/surface.rs)), and then starts no
  participant at all ([`app.rs:800-820`](../../ymp-rust/crates/ymp-tui/src/app.rs)).
- *The design set before this correction* fails at step 7, because it asks the operator for the
  permitted model set at request time (leak L-08).
- *The design set before this correction* also fails at step 9, because it drew an authorization
  surface with a typed run identifier
  ([`COLLECTIVE-TUI.md:415` at f0376be](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-TUI.md#L415))
  and a setup question after the first provider
  ([`:147`](https://github.com/maggnus/ymp/blob/f0376be/ymp-docs/design/COLLECTIVE-TUI.md#L147)) —
  leak L-16.

# Visual concept

This document states the shape of the ymp terminal interface: what the operator sees, in what
order, and why the composition is arranged this way. It governs composition and operator path.
Exact screens, state variants, deterministic fixtures, and the reusable structure handoff are owned
by [`design/ymp_chat_tui.dc.html`](design/ymp_chat_tui.dc.html). The primary fixed-layout review
version is [`design/ymp_chat_tui.pdf`](design/ymp_chat_tui.pdf). Together they supersede the earlier
dashboard artifact `ymp_k9s_tui.dc.html`; the constraints that no composition may violate remain
owned by [`INVARIANTS.md`](INVARIANTS.md) and [`PROTOCOL.md`](PROTOCOL.md).

## Why the composition changes

The first interface was derived from the architecture rather than from the operator's path. Every
domain distinction — trust planes, budget dimensions, verification digests, freshness, assurance —
became a mandatory screen element, and the rule that a trust-critical fact must not be hidden was
read as a rule that every fact must be shown at once. The reference frame carries a six-line header
with twelve hotkeys above a twelve-row table of areas, at a moment when the operator is asking one
question: is it working, and do I need to step in.

Density was not an implementation accident. Any faithful rendering of that composition is
overloaded, which is why successive implementation attempts produced the same result. The artifact
itself contains the exit: the narrow-terminal frame demonstrates that the substance of each screen
is an identifier, a state, a next action, and one limiting fact, and the detail template explicitly
permits a list to omit values as long as a visible path to full depth exists. The invariants demand
honesty — do not display a falsehood, do not invent semantics, do not hide the path to a fact —
not simultaneity.

## Product shape

The operator's path has three stages:

```
intent  →  contract  →  collective
```

A human states an intent in ordinary prose. A contract package is drafted from it through
repository analysis and structured interview, as required by
[`PROJECT-CONTRACT.md`](PROJECT-CONTRACT.md). The human authorizes the package, which grants
authority and reserves budget. Participants then work under that contract, publish candidates, and
a protected oracle decides acceptance.

The interface follows that path rather than the component diagram. Drafting is a conversation,
authorization is a decision, and execution is observation — three different activities that the
previous composition tried to serve with one dashboard.

## The main screen is a conversation

A single scrolling transcript fills the screen, with an input line at the bottom, a thin context
header, and a status line. Data views — runtimes, providers, provider properties, pools, agents,
candidates, events, budgets, object detail — are full-screen pages opened on demand through a `:`
command line and closed with `Esc`.

The opening screen is a three-line banner: a one-line logo carrying version, project and store, the
invitation to state a goal, and a line naming the two doors out (`:providers` with its state
counters, and the key map). A five-line logo was considered and rejected.

The three stages are one continuous transcript, not three screens. The transcript is interrupted
exactly once, by contract authorization, because that decision is irreversible and begins spending
money.

Transcript entries are heterogeneous and must remain distinguishable at a glance: human turns,
application replies, run events, and — from POC-2 — collaboration board messages, each attributed
to a participant and typed with the message kinds already defined in `PROTOCOL.md`. The entry model
separates origin from body precisely so that board messages differ by author and kind rather than
by structure, and arrive without reshaping the transcript.

This also answers what the operator does during a two-hour run. Reading an event table is not an
activity; reading a conversation and intervening when the collective goes astray is.

### One turn, one reply

One operator turn produces exactly one application reply. The interface may not stack several
replies that narrate its own machinery — pool state, contract assembly, verifier derivation —
around a single question that asked for none of it.

A refusal is bound by the same discipline. It occupies at most two lines, its first line opens with
the verb of the operator's next action, and the reason follows after a dash on that same line. The
mechanics of the decision live behind `:describe refusal`, not in the transcript.

Entry text starts in a fixed column that does not depend on the content — column 20 for a run
event, column 8 for a reply — and a wrapped line resumes exactly under that column. A digest is
shown as six characters without an algorithm prefix; the full form appears only in `describe`.

### Reference points

The closest reference for the main screen is Claude Code: the transcript is the product and the
input line is always ready. Two things differ.

The operator enters structured data, not only prose: scoping answers during the interview, a budget
vector across independent dimensions, runtime profiles and model routes, the starting roster of
participants and the ceiling on further recruitment, and explicit confirmation text for
irreversible commands. These require real input affordances inside a transcript-first layout.

Behind the conversation there are full pages of data. Journals, candidate lists and budget tables
are too large to live as inline replies, so they are separate navigable pages in the k9s tradition:
one dense list per page, filter, selection, drill-down, `Esc` back. Navigable data pages and
decision modals behave differently and must not look alike.

### Inertness

Board messages are inert data. Nothing in the transcript is executable in one keypress: acting on
a suggestion always requires a separate command issued under the operator's own authority. This is
INV-4 expressed as layout, and it is the reason a suggestion from a participant can never become a
button. The same rule reaches into the tables: the `↳` line under an untrusted collaboration-plane
row carries no action key and stays data.

## Action classes

Three classes of action are distinguished, and the affordance follows the class rather than the
screen.

- **Reversible** — a single key, no confirmation, because the same key undoes the result:
  `d disable`, `r measure again`, `F follow`, `/ filter`.
- **Consent** — a modal in which the consequence is stated on the line directly above the key that
  takes the decision.
- **Irreversible** — a modal with a preview of the consequences and a typed confirmation of the
  exact identifier: authorizing a contract, cancelling a run.

A key never takes a decision whose consequence is not stated on screen, and a modal never stands
where one key already undoes the action.

Enabling a provider has exactly one form. It is the consent to disclose repository content, so it
is reached only through the provider's own properties page: list, `Enter`, properties, `e`. No
enable key exists in the list. The consent modal states, once and in one place, that enabling
permits repository content of any workspace on this host to reach that provider — asked once, never
per project and never per run — and it carries the pool selection alongside, since that choice is
reversible.

## State vocabulary

One vocabulary of entity states — `off`, `ready`, `measuring`, `error` — applies identically to a
provider, a pool and a run. `off` means the operator never enabled it and nothing is measured or
spent. `ready` means the measurement succeeded and the age of the last measurement is named.
`measuring` means the measurement is running now, and the cell is never left empty. `error` means
the measurement happened and failed, with the reason and the next action on the `↳` line.

*Enabled* is therefore not a state: enabling is an operator action, and what follows it is `ready`,
`measuring` or `error`. A header counts in that one vocabulary, and the counters sum to the number
of rows.

A run that has finished leaves this vocabulary: its state is `ended`, and which end it was is named
by exactly one of the five terminal outcomes. A run whose state is `error` failed to start; that
word never stands in for `infrastructure_error`, which terminates a run that had already begun.

Surface state — `loading`, `empty`, `stale`, `degraded`, `error` — is a separate axis from entity
state. Both markers may appear on one screen, they mean different things, and neither substitutes
for the other. Every such marker is textual and survives the absence of colour.

## Table discipline

A data page is drawn by fixed rules rather than per-screen judgement. A column is as wide as the
larger of its header and its longest value plus two, up to its own cap; the leftover width goes to
the last text column instead of being spread across all of them. A truncated value is cut with an
ellipsis inside its own cell, and at least one space always remains before the next column.

Numbers are aligned to the right by digit position. An em dash marks a quantity that was not
measured, and never a measured quantity equal to zero.

A reason that is the same for every row stands once in the header, not repeated under each row. A
note under a table is at most one line; everything longer lives behind `?`. Key hints occupy a
single line at the right of the status line. Filtering is `/` across all columns, or `/state:ready`
against one, and the header names the active filter together with the way back.

## Contract authorization

This is the most consequential screen in the product, because contract and oracle quality is the
decision that can sink a run and no later mechanism repairs it.

It shows a requirement-to-evidence coverage map rather than contract prose: each public
requirement, what checks it, and how many deliberately broken versions of the code that check
rejected. Requirements with no mechanical check are shown as such and remain for a human judge.
Blocking items disable authorization. Price, runtime routes and assurance profile are visible.

Human approval establishes authority, not validity. The screen therefore exposes gaps —
unresolved ambiguities, requirements without checks, external effects, oracle coverage holes —
instead of presenting a generated document as finished.

## Naming model

The interface borrows the navigation model of k9s: a `:` command line instead of numbered screens,
`Enter` to descend, `Esc` to return, `/` to filter, and a thin header. The provider and pool screens
follow the product's list, select, properties, action path.

| Kubernetes | ymp | Note |
|---|---|---|
| namespace | project | determined by the launch directory; a header fact, not an entry screen |
| pod | agent | the primary resource of the lists |
| Job | run | creates agents, owns their lifetime, immutable after start |

Two naming constraints are load-bearing. The word *workspace* is already owned by
[`ARCHITECTURE.md`](ARCHITECTURE.md) for the private writable copy of one attempt and cannot also
name the enclosing scope. Isolation is not an operator-facing choice: workspaces share no writable
Git metadata, which excludes `git worktree` as an isolation mode regardless of convenience.

## Roster and recruitment

The operator sets the starting roster of participants and a ceiling on how many more may be
recruited, before authorization. Participants then recruit one another within budget:
`request_participant` spends offer-stage budget, and participant starts are a budget dimension.
The agent list therefore changes during a run, and the remaining funded starts are visible next to
the ceiling.

## Contract drafting is a run

Drafting is not performed by a privileged agent standing outside the collective. A privileged role
would contradict the protocol, where a role is a position inside a contract rather than a property
of an agent, and where authority is granted per task, short-lived and revocable.

Drafting is therefore a run under a built-in contract package that ships with the product and is
approved once, because it describes the procedure rather than the user's project. The authority to
read the whole repository and to write protected cases is granted to a task inside that run. The
conversation with the human is an ordinary board exchange; the intervention marker is normal here,
since this run is never compared against others in an experiment. Its acceptance is mechanical in
the same sense as any other: the package is accepted when its negative controls reject deliberately
broken versions of the code.

Protection of the hidden oracle is enforced by the boundary between two runs — different budgets,
audiences and capabilities — not by a special class of agent.

## Storage boundary

The public part of a contract — specification, visible checks, environment manifest — lives in the
project directory and is committed with the code, where ordinary review applies. Protected oracle
bytes never enter the source snapshot: agents work from a copy of the repository, so anything
committed there stops being hidden. Runs, journals, objects, protected material and transcript
history live in the ymp store.

## Semantic constraints

The composition may not introduce what the kernel refuses to do.

- No ranking, scoring, grading, or recommendation of agents, bids, or candidates anywhere; sorting
  uses raw mechanical fields only (INV-1).
- No lead agent, no hierarchy, no assignment. Indentation expresses recorded parentage, never
  priority.
- The control, untrusted collaboration, and verification planes stay visually distinct wherever
  they appear together (INV-4, PLN-001…PLN-004).
- Every view holds its own event cursor and recovers from the journal; lag is a visible state, not
  a silent divergence (INV-6).
- Quiescence is not acceptance. The five terminal outcomes are always named exactly (INV-7).
- Budget dimensions never trade against one another: spare capacity in one never authorizes an
  action blocked by another (INV-2).
- The assurance profile is stated plainly, including that `poc_process_isolation` provides no
  hostile-code containment (INV-8).
- No candidate is ever presented as ready to apply to the user's working tree.

## Phase honesty

POC-1 has a single participant. Offers, bids, work obligations, the collaboration board and
multiple agents belong to POC-2, and the current domain has no participants, messages, board, or
contract package as objects. The budget carries two dimensions today.

The transcript must be honest at both stages: it may not imply a crowd that is not there, and it
must not require redrawing when the crowd arrives. Concretely, a human turn is marked as local and
not recorded in the journal until a message command exists, and the budget page shows the two real
dimensions rather than a rehearsal of the full vector.

## Relation to the design artifacts

`design/ymp_chat_tui.dc.html` draws this composition, while `design/ymp_chat_tui.pdf` is its primary
fixed-layout review version: seven transcript states, nine full-screen data pages — runtimes,
candidates, events, budgets, describe, agents, providers, provider properties, pools — two decision
modals, one consent picker modal, a key overlay and a size guard, at 80×24, 120×40 and 180×50, with
a state-kind matrix, deterministic fixtures sharing one set of identifiers, and a handoff naming the
reusable structures (`AppFrame`, `TranscriptEntry`, `FieldRow`, `DataPage`, `DescribeGroups`,
`DecisionModal`, `TypedConfirm`, `PickerModal`, `SelectColumn`, `ColumnWidths`, `FooterHints`,
`ActionClass`, `RefusalReply`, `HelpOverlay`, `SizeGuard`, `StateKind`, `EntityState`).
Implementation follows those names and metrics rather than inventing its own.

Revision 1 of the artifact answers five recorded interface failures taken from a running build and
reproduced verbatim in
[`work/backlog/TUI-OWNER-FIXTURES-20260816.md`](work/backlog/TUI-OWNER-FIXTURES-20260816.md):
transcript density, the shape of a refusal, table rules, the state vocabulary, and the separation of
providers from pools. It introduces the provider, provider-properties and pool screens, and for each
theme it names the rejected form beside the adopted one, because the rejected form is what explains
the adopted one. The `:runtimes` page stays separate from `:providers`: an engine is the CLI
installed on this host, a provider is the account whose models are measured.

The earlier artifact `ymp_k9s_tui.dc.html` has been removed from the working tree. Its dashboard
composition and numbered destinations are superseded. Every retained template family, projection,
region rule, and vocabulary constraint needed by the current concept must be present in the three
current sources; implementations must not depend on the removed file. The old artifact remains
readable in history at the revision pinned by work card `W0-UX-01`,
[`35cc659`](https://github.com/maggnus/ymp/commit/35cc659981f73700296c9ed37b378e59eabff4a1).
Because that card is accepted, this supersession is recorded as a new design revision in
`W1-APP-02e.2` rather than as an unreviewed implementation liberty.

## Divergence from the current implementation

The new artifact draws the target system, and the domain lags behind it. These gaps are known and
must not be closed by pretending in the interface.

| Drawn | Present today |
|---|---|
| Assurance profile `poc_disposable` | `poc_process_isolation` in `PROJECT-CONTRACT.md` and code; decided below in favour of the recorded name |
| Event kinds `workspace.fact`, `budget.reserve`, `budget.use`, `candidate.published`, `verification.started/check`, `board.message.*` | eight kinds in `ymp-domain`, none of them these |
| Budget of five dimensions, with enforced / observed / estimated classes | two dimensions: remaining attempts and remaining verification queries |
| Intent, contract package, interview, board messages, participants | absent from the domain |
| Runtime profiles `rp-gemini` and `rp-opencode` | drivers for fake, Codex and Claude Code only (`ymp-runtime-fake`, `ymp-runtime-codex`, `ymp-runtime-claude`) |
| Observed workspace facts — files touched, test runs, diff size | not collected |
| `ymp apply cd-32` as a separate CLI command | no such command |

The last row resolves a real gap rather than creating one: the previous contract forbade applying a
candidate from the TUI without naming any path at all, leaving the cycle open. An explicit CLI
command under the operator's own authority keeps the interface boundary intact while giving the
result somewhere to go.

## Open questions

1. Whether an oracle that misses a deliberate break is a blocking item or a warning. The artifact
   answers it as a warning that stays visible (`R2`, `2/3 killed ▲`), while a test failing on a
   clean base blocks (`R5`).

## Interface decisions

Two questions that the design left open are decided here, because only an implementation can
answer them, and each is shown by a screen rather than only stated.

### The authoritative assurance-profile name is `poc_process_isolation`

The drawn name `poc_disposable` is superseded. `PROJECT-CONTRACT.md` states that the first release
uses `poc_process_isolation`, the code uses that name, and it describes the mechanism the product
actually applies: each attempt runs in a separate process against a private copy of the code.
`poc_disposable` describes the environment the experiment must be run in, which `INV-8` already
owns; using it as the profile name would name the wrong thing and, worse, would suggest a
containment property the profile does not provide.

Wherever the profile appears with room for prose it is followed by its limit — that it provides
no hostile-code containment and that agents run with the operator's own permissions. The header
glyph is the compact form; the full sentence lives in the key map and on `:runtimes`. This follows
the owner decision of 2026-08-15 that the opening transcript stays a three-line banner: the
one-line logo already carries the profile with its glyph, and the remaining two lines are the
invitation and the doors out.

Shown by: the opening transcript, the wide context header, and the `assurance` row of the contract
authorization surface.

### A run the operator ends is recorded as `cancelled`

`cancelled` and `infrastructure_error` are different facts and are never substituted for one
another. `cancelled` records a decision by the operator; `infrastructure_error` records a failure
of the machinery. When the operator confirms a cancellation, the interface issues the domain's
cancel command, the journal records the cancellation with its reason, and the run reaches the
terminal outcome `cancelled`. A runtime that dies while being stopped is a consequence of that
decision and does not change the outcome the operator is shown.

The confirmation is typed: the exact run identifier must be entered before the action becomes
available, and the screen states beforehand which attempts are interrupted, that consumed budget
is not returned, that the journal and every published candidate stay readable, and that the
recorded outcome will be `cancelled`.

Shown by: the cancel confirmation surface and the terminal transcript that follows it.

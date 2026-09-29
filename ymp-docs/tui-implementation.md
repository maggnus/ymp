# Interactive terminal interface

Canonical status remains in `tasks/records/W1-0015.json`. This document records
the delivered interface, its checked boundaries, the intentional adaptations
against [tui-reference.md](tui-reference.md) and what is left unverified. All
evidence here uses the scripted team; it establishes no native run.

## Composition

`ymp` is one executable with two interactive forms and two read-only commands:

```text
ymp --store DIR --workspace DIR --scripted PROGRAM.json
ymp --store DIR --workspace DIR --claude EXECUTABLE --model MODEL [--effort EFFORT]
ymp sessions --store DIR
ymp report --store DIR SESSION
```

`ymp-cli/src/host.rs` composes the SQLite journal, the Direct workspace and the
selected backend behind the `Host` trait of `ymp-tui`. The interface names what
the user asked for; the host calls `Dispatcher::start` or `Dispatcher::recover`
and the kernel decides. `ymp-tui` depends on `ymp-runtime` only and reads domain
and kernel values through its re-exports.

A `Dispatcher` stays on the thread that created it. `LiveSession`
(`ymp-runtime/src/live_session.rs`) owns that thread: it ticks the Dispatcher,
accepts `OwnerCommand::Interrupt` and `OwnerCommand::Answer`, and publishes the
last journal projection with a local `Pulse`. The terminal loop reads what was
published and never waits for a provider. While the session waits, a recorded
change is published within 250 ms. A pulse is a local fact about the thread and
is never shown as a recorded status.

The execution host records the end of a stopped call from its own threads. A
report step the Dispatcher prepared meanwhile is refused because the journal
moved (`stale_revision`, or `report_phase` from the report's preparation). In a
session the owner stopped, the session thread takes such a step again, at most
fifty times in the life of the thread. The steps of a stopped session compose
the report: each reads the record first and starts no call. Every other refused
step fails as it is, because a step of running work can have recorded a part of
itself or hold a prepared call. The first recording of the journey met the
refusal after `Ctrl+C` during the producer's call: the session stayed
`Cancelled` without a report until the repeat was added.

The scripted team (`backends/scripted_team.rs`) reads a program file with the
files to write, optional clarifying questions, the producer's usage coverage
and a pace. It calls no model. `--claude` uses the W1-0020 backend with the
installed executable and its own authentication; the account data of its
initialization response is neither stored nor shown.

The store must be outside the workspace. The location is resolved and compared
before anything is created, `..` is refused, and the check is repeated after the
directories exist. `/expect` and `/preserve` refuse a path that passes through a
symbolic link. The workspace is Direct: a session changes its real files.

## Interaction

The conversation occupies the main column, the sidebar stands to its right at
100 columns or more, and the composer stays at the bottom above a status row
that names the focused region and its keys.

| Input | Effect |
| --- | --- |
| text, `Enter` | States the goal of a draft, or answers the question that waits. It starts and continues nothing. |
| `/expect PATH TEXT`, `/preserve PATH` | Adds a checked expectation to the draft. |
| `/budget AMOUNT` | Sets the budget limit of the next session. |
| `/start` | Starts a session from the draft. |
| `/interrupt`, `Ctrl+C` while work runs | Sends the runtime interrupt. A second `Ctrl+C`, or `Ctrl+C` while nothing runs, leaves. |
| `/sessions`, `/open SESSION` | Lists the recorded sessions; opens one read-only. |
| `/recover continue`, `/recover report` | Asks the runtime to drive the opened session again, or only to deliver its report. |
| `/new` | Leaves the session as recorded and begins a new draft. |
| `/team` … `/policies` | Inspection pages: team, criteria, plan, assignments, commitments, resources, activity, results, checks, acceptance, report, policies. |
| `/quit` | Leaves. Leaving itself records nothing. |
| `Ctrl+P` | Command palette with a filter. |
| `Ctrl+B` | Sidebar; an overlay below 100 columns. |
| `Ctrl+J`, `Alt+Enter` | Newline in the composer. `Tab` completes a command. |
| `PageUp`, `PageDown`, arrows, `Home`, `End` | Read the conversation. Reading back suspends following; `End` resumes it. |

On a page: arrows or `j`/`k` select, `/` filters, `s`/`S` sort, `Tab` moves to
the next page, `Enter` or `d` opens the record's details in a scrollable
overlay, `Esc` closes the topmost layer. `Enter` on the sessions page opens the
selected session read-only.

## What is shown and what it rests on

Every value comes from the journal projection of the session, except the pulse
and the notes of the interface itself, which are labelled as such.

- The header and the sidebar show the recorded status and revision. The
  interface's own state (`working`, `waiting for you`, `report delivered`,
  `read-only`) is shown separately.
- A call with a recorded end shows that end. A call whose provider ended while
  usage or effects are not settled reads `provider ended; not settled`. A call
  with neither reads `working` only while this interface drives the session and
  the call was dispatched by this drive; otherwise it reads `no end recorded`,
  which includes a call left by an earlier drive. Its usage reads `—` with the
  reason. The sidebar counts working calls and calls without an end separately.
- Unknown usage is `present`, `a call has no receipt yet` or `none recorded`.
  Held units stay visible beside spent units.
- The report is the rendered delivered report. Reported agent output and
  verified results keep separate headings; acceptance and its confirmation
  grade come from the runtime's records.
- A denial names its code and message and says what it recorded: nothing, with
  the revision shown, or the move of the recorded revision. A step of the
  session that fails names the recorded revision and claims nothing about what
  the step wrote before it failed. An accepted command shows the revision it
  produced and the one it followed.

Recorded and agent text is data. Control characters and the direction marks
U+061C, U+200E, U+200F, U+202A to U+202E and U+2066 to U+2069 are replaced by
U+FFFD before text reaches the terminal, in the interface, in the output of
`sessions` and `report` and in what the executable prints on leaving, paths
included. No recorded or agent text is interpreted as a command.

## Refusals of the interface

These are decided before anything reaches the runtime and leave the recorded
state unchanged:

| Code | Situation |
| --- | --- |
| `session_open` | `/start`, `/expect` or `/preserve` while a session is open; `/new` begins the next draft. |
| `draft_goal`, `draft_checks`, `draft_budget` | `/start` without a goal, without a checked expectation or with an unusable budget. |
| `already_driving` | `/recover` while this interface drives a working session. |
| `session_working` | `/new` or `/open` while this interface drives a working session. Closing the session thread does not cancel a provider call in flight, so the interface does not abandon one silently. |
| `not_working` | `/interrupt` while nothing runs in the driven session. |
| `not_driving` | `/interrupt` in a session opened read-only. |
| `session_exists` | A start whose session already exists in the store. |
| `session_missing` | `/open` or `/recover` for a session the store does not record. |
| `policy_selection` | `/recover` from a composition whose strategies differ from the recorded ones, for example a scripted session opened with `--claude`. The host compares them before the runtime is asked, because the runtime records the owner's intent first. |
| `usage`, `unknown_command` | A malformed or unknown command. |
| `workspace_path` and the refusals of `/preserve` | A path that is not a normalized relative path, or a file the workspace cannot supply. The refusal comes when the expectation is stated. |

`/new` without a session drops the draft. A start that the runtime refuses
before anything is recorded leaves no session: the goal and the expectations
are the draft again.

Everything else is the runtime's decision and is shown as it answers, for
example `continuation_unavailable` and `recovery_unresolved`.

## Leaving, errors and the terminal

The terminal is restored on normal exit, on a returned error and on a panic of
the interface loop. A panic of the session thread is kept as a fault and shown
in the conversation while the interface keeps running. A panic of any other
thread leaves the terminal as it is; the runtime that owns the thread answers
for it.

On leaving, the interface stops the session thread and reads the record again,
because a command sent just before leaving may still have been recorded. The
executable then prints the session, its recorded revision and status and the
command that reads the report. If work was left and no report is delivered, it
says that nothing drives the session now. If the record cannot be read again,
it says so and names the printed revision as the last one shown.

Recovery resumes nothing that was in flight. A call without a recorded end
keeps its hold and its unknown usage; `/recover report` delivers the
deterministic report without a further call.

## Intentional adaptations against the reference

| Reference | Delivered | Reason |
| --- | --- | --- |
| Text in the composer starts work. | Text states the goal; only `/start` starts a session and only `/recover continue` drives one again. | A session needs a checked expectation and a finite budget first. Text, including pasted agent output, must not act as continuation authority. |
| Compact activity expands in place to its full content. | The conversation shows an excerpt of six lines; the full attributed output is a record of the Activity page and opens as a detail overlay. | One place holds full content, so reading older activity keeps its position while output arrives. |
| A final answer has the greatest visual weight. | The delivered report has it, marked with a bar, together with the user's goal and the question that waits. | The accountable outcome of a session is its report, not an agent's last message. |
| The sidebar carries navigation. | The sidebar lists the pages below the session, team, criteria, work, resources and pending summaries; `Tab` from an empty composer focuses it. | Summaries answer the first questions; pages hold the tables. |
| Sidebar at any width when toggled. | Beside the conversation from 100 columns; an overlay below that. The overlay of a short terminal shows its upper sections only. | The composer and the conversation stay usable in a 70 by 24 terminal. Every section is also a page. |
| A recovery screen. | `/recover continue` and `/recover report` on a session opened read-only, with a note of what recovery does not resume. | The runtime owns recovery; the interface states the intent and shows the answer. |
| Configuration forms. | `/expect`, `/preserve` and `/budget`; policy selections are read on the Policies page. | The task requires no form for every policy parameter. |
| Headless commands do not link a terminal UI. | `sessions` and `report` use `store.rs` only and never enter `ymp-tui`, but the single executable links it. | One executable is the product. A command that must not link the terminal library needs its own binary target. |
| Theme inventory. | One restrained palette; state and selection also differ by text and markers. | Appearance beyond legibility is not part of this task. |

No code of a `legacy-*` tag was opened for this task.

## Evidence

`tui-journey/journey.sh YMP ROOT TRANSCRIPT` drives the built executable in
tmux and writes what the terminal showed. `tui-journey/transcript.txt` is one
such run. Times and session identifiers differ between runs; paths under the
chosen root are written as `<root>`. The parts are:

1. Text alone starts nothing; the session runs to a delivered report; criteria,
   a criterion's details, resources and activity are inspected; a second
   `/start` is denied at an unchanged revision; the terminal is restored; the
   report is read by `ymp report` and by reopening the session.
2. Produced bytes that do not meet the expectation: the unmet criterion in the
   report and on the Criteria page.
3. `Ctrl+C` during work: the recorded stop, the report without a further call,
   the denied second interrupt and the runtime's answer to `/recover continue`.
4. Leaving while a call is in flight: the exit message, the reopened session
   with a call without a recorded end and unknown usage, text that continues
   nothing, and `/recover report`.
5. A terminal of 70 by 24: a clarifying question and its answer, reading older
   activity while work runs, the sidebar overlay and the filtered palette.
6. A store inside the workspace is refused and nothing is created.
7. A path outside the workspace is refused when the expectation is stated,
   `/new` drops a draft, and a start that fails before anything is recorded
   leaves the draft and no session.

A separate terminal run opened a recorded scripted session from a `--claude`
composition (discovery only, no model call): `/recover continue` and
`/recover report` were refused with `policy_selection` and the recorded
revision stayed 103.

Unit tests cover what a terminal run cannot show precisely: text starts and
continues nothing (with a counting host), wrapping with non-breaking and wide
spaces, the cursor position of the composer, replacement of control and
direction characters, stable reading while entries arrive, argument parsing and
store resolution.

A separate terminal run with the producer's coverage `Unknown` showed the
runtime waiting for the call's deadline of 240 s, then
`Blocked(unsettled_usage_or_effects)`, a deterministic report, 4 units spent,
10 held and unknown usage present, without a further call. The journey omits it
because of its duration.

## Limits and unverified behavior

- No native session was driven through the interface. `--claude` is composed
  and compiles; its behavior in the interface is unobserved.
- Terminal restoration after a panic of the interface loop and after a terminal
  error is implemented and was not provoked in a terminal. The same holds for
  the message about a record that cannot be read on leaving.
- `Dispatcher::recover` records the owner's intent before it checks the budget,
  the session definition, the workspace binding and the strategies. The host
  compares the strategies first; the other refusals can still follow a recorded
  intent, and the interface then shows the move of the revision.
- `Dispatcher::run`, used by callers other than the interface, returns the
  first `stale_revision` of a step and does not take the step again.
- Outside a stopped session a step refused with `stale_revision` ends the drive
  with `Failed`. An independent reading of the Dispatcher found steps with more
  than one commit (`blocked`, workspace and final capture, the recovery retry)
  and a launch that keeps a prepared call after a refused dispatch; repeating
  those is not safe, so the interface does not. `/recover` decides afterwards.
- The repeat was observed once, in the recorded journey. The refusal depends on
  timing and is not provoked by a test.
- `SIGTERM` and `SIGHUP` are not handled; the terminal is then left as it was.
- Closing the session thread waits for the tick in progress, so leaving can
  take as long as that tick.
- Closing the session thread does not cancel a provider call in flight. The
  interface refuses `/new` and `/open` while work runs; leaving with `/quit`
  leaves the call to its own deadline.
- The sidebar overlay of a short terminal does not scroll.
- The runtime accepts an interrupt for a session whose report is delivered and
  records it. The interface refuses the command; the runtime is unchanged.
- A Dispatcher tick takes 400 to 650 ms on a journal of more than a hundred
  events, so a scripted session of five calls takes more than a minute.

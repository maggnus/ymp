# Interface

`ymp` opens a chat-first terminal workspace: the conversation fills the main column, and a
right sidebar carries the current session's context and live team activity.

## Layout

```text
ymp  project  /path/to/project      3fa27c81  ✓ completed  154.9k+ tokens  12 / 200 turns
──────────────────────────────────────────────────────────────────────────────────────────
  ▌ you                                                    12:04  │ SESSION
  ▌ Add validation and tests for the import command                │ 3fa27c81 ✓ completed
                                                                   │ turns this session 12 / 200
  · gpt-5.6-sol high proposed a plan · 2 tasks · Validate…   12:04 │ dir  …/project
  · glm-5.2 max review · accepted · Empty rows are rejected  12:07 │
                                                                   │ TOKENS       154.9k+
  ◆ gpt-5.6-sol high · final result                         12:09  │ gpt-5.6-sol   131.8k
    Added `validate_row` and six tests. `cargo test` passes.       │ glm-5.2       23.1k+
                                                                   │
                                                                   │ TEAM
                                                                   │ ○ gpt-5.6-sol   idle
                                                                   │ ○ glm-5.2       idle
                                                                   │
                                                                   │ TASKS
                                                                   │ accepted        2 / 2
                                                                   │
──────────────────────────────────────────────────────────────────────────────────────────
› Describe a task, or type / for commands.
Ready                                     composer  Enter send  Ctrl+P commands  Tab focus
```

The sidebar appears from 80 columns and widens at 100 and 140. Hide it with `Ctrl+B` or
`/sidebar`; the choice is remembered. Below 72 columns the main column takes the full width.
The composer keeps its rows at every size, so a narrow or short terminal loses the header
before it loses the ability to type.

## Reading the conversation

Your prompts and the team's final answers are the content. The coordination that produced
them is summarised: a plan, a bid or a review becomes one line of readable activity instead
of the structured payload the agents exchanged. Internal conversation routing is not shown.
An execution turn is labelled an execution report, because such a turn may conclude that
nothing needed changing. Code keeps its exact spacing and indentation, so a rendered message
can be compared against a file.

A member the runtime reports as waiting is shown as waiting and not as busy: a turn held up
by coordination is not work in flight, and the code it waited under is on the task and on the
decision that recorded it.

Agents are named by model. Neither the caption an installation gives a model, such as
`Default (recommended)`, nor the provider or actor ID is used as a name. A message is headed
by the model and effort of the invocation it is bound to, for example `glm-5.2 max`: the model
the installation reported for that turn, otherwise the concrete model that was sent, with the
internal `default` alias resolved only through the identity captured for that turn. Effort
appears only when the installation reported a graded level. Where it reported none, the
heading is the model alone, as in `claude-opus-5`. A binary thinking switch such as `on` or `off`
is not a level either, so a GLM turn that reported `on` reads `glm-4.7`. A level the installation
really reported as `none` stays, as in `glm-5.2 none`. The assignment details list requested,
sent and reported effort separately and exactly as recorded. Two turns of one actor that ran different models or efforts therefore carry
different headings. A message no recorded invocation is linked to, such as one written
before messages were linked, reads `unknown model`; a later turn, a configuration change or the
provider the actor uses now does not rename it. Your prompts stay `you`, runtime notices stay
`ymp`, and local fixture agents keep the name their turn captured. `Enter` shows the actor ID,
provider and invocation behind a heading. Headless `ymp run` prints the same headings.

A stream, a working member in the sidebar and a tool call in the status row are named by the
invocation that is running. An idle member, a token row and every choice on the team and agent
pages are named by the concrete model alone, as native metadata resolved it. A profile whose
`default` alias nothing resolved reads `unknown model` and is not offered as a new choice.
A decision, a proposal to change the plan and the actors a record names are history: each is
named by the turn that record itself links, and one that links no turn of that actor reads
`unknown model`, even when a later turn of the same actor is known. A turn that captured no
identity is not treated as a local fixture because of how its provider is configured now. The
sessions list names a team by the models its recorded turns ran, which it reads for the loaded
session, and never by the names or aliases of the profiles the session captured.

The sidebar's `TOKENS`, `TEAM` and `TASKS` lists are compact tables: column titles over aligned
rows, with token figures aligned right and the accepted task count beside the `TASKS` title. A
short terminal shortens a section without separating its column titles from its rows, and a
narrow sidebar cuts a long cell rather than wrapping it. In `TEAM` the word for what a member is
doing gives way before its model and effort. The sidebar's team section names the profiles the
loaded session captured when it started, and says so. With no session loaded it describes the
team the next run would use instead; the two are never presented as the same thing. The turn counter works the same way: a loaded
session is measured against the limit it captured, not against a limit edited afterwards.

- `Tab` moves the focus into the transcript, then `Up`/`Down` select an entry.
- `Enter` opens the complete attributed message, with its author, kind and timestamp.
- `Space` expands one collapsed entry in place: a long execution report, or the full message
  behind a one-line plan, bid, review or learning note.
- `/details` or `Ctrl+L` switches the whole transcript to full attributed messages: messages
  that only route the conversation between agents appear, and no agent message is collapsed, so
  `Space` has nothing to expand.

Scrolling up pauses auto-follow. The status row then reads `⏸ paused` and names the key
that returns to the newest message, which is `End`. Text that streams in while you are
reading is appended below without moving your position.

## Keyboard

One region owns the keyboard at a time, and the status row names it. `Tab` moves between the
composer and the transcript or page; the sidebar never takes the keyboard. `Esc` always removes
the topmost thing: an open overlay, then the command completion list, then a page filter, then
the open page, then a focus that is not the composer, and finally the draft in the composer.

| Key | Action |
| --- | --- |
| `Enter` | Send the prompt, or activate the selected row |
| `Ctrl+J` | Insert a newline in the composer |
| `Ctrl+U` | Clear whichever field owns the keyboard |
| `Tab` | Complete a command, or move focus to the next region |
| `Ctrl+P` | Open the command palette |
| `Ctrl+T` | Open the theme chooser |
| `Ctrl+B` | Show or hide the sidebar |
| `Ctrl+L` | Toggle detailed agent messages |
| `PageUp` / `PageDown` | Scroll the transcript or the current list |
| `Home` / `End` | Jump to the oldest or the newest message |
| `/` | Filter the rows of the open page; a leading `!` keeps the rows that do not match |
| `Shift`+letter | Sort a page table by the column whose title has that letter underlined |
| `d` | Inspect the selected page row |
| `Backspace` | On the files page, go to the parent directory |
| `Ctrl+C` | Press twice within two seconds to leave ymp; active work is stopped first |

The first `Ctrl+C` only asks: the status row reads `Press Ctrl-C again to exit`, and nothing is
stopped, closed or cleared, whichever region or overlay owns the keyboard. Another key, a paste
or two seconds without a second press withdraws the question. Repeat and release events are
ignored when a terminal reports them, but ymp does not ask for them, so a terminal that delivers
a held `Ctrl+C` as repeated presses leaves ymp while the key is held. The second press, `/quit`,
or `Ctrl+D` in an empty composer leaves. The status row names the work being stopped, and ymp
waits up to 10 seconds for a run to record its state before it restores the terminal and prints
`Resume this session with:` and a shell-quoted command. The command always names the absolute
metadata directory with `--home`, then the session's saved project directory and its ID, so it
works from a shell with another `HOME` or `YMP_HOME`. It opens the conversation without starting
agents; use `/resume` there to continue the run. Nothing is printed when no session was open.
`/stop` and `/pause` stop a run without leaving.

A command runs on one press of Enter unless it cannot act without an argument. `/theme`,
`/team`, `/memory`, `/limits` and `/resume` all do something useful on their own, so they
run immediately; `/agent` cannot, so Enter writes `/agent ` into the composer and waits.
Tab completes a name without running it.

Text pasted with a bracketed paste goes to the field that owns the keyboard. The instructions
editor preserves multiple lines, indentation, and repeated spaces; Alt+Enter or Ctrl+J inserts
a newline, and Enter saves. While an agent model or instructions editor, or the command palette,
is open, the paste lands there and not in the conversation composer. Cancelling an editor with
Esc discards what was typed or pasted into it.

## Pages

Every page is a read-only projection. A page opens with its command, such as `/tasks`, or from
the command palette; opening one never starts an agent and never writes to your working
directory. Each page states its own keys in the status row.

A page's records are tables in the style of k9s: upper-case column titles over aligned rows,
figures aligned right, and the column titles kept on screen while the rows scroll. A page may
open with a note that says what it is; a note spans its table and is not sorted. On a narrow
terminal the flexible column gives up width first, then columns of lesser importance are
hidden, and a table is never drawn wider than the page. The full record of a row opens in a
popup with `Enter` or `d`; there is no detail pane under the list. The files page is a file
explorer rather than a table, and is described in [its own section](#files).

- `/` starts a filter. Typed text is matched without regard to case against every cell, and a
  filter that starts with `!` keeps the rows that do not match. `Enter` keeps the filter and
  returns the keys to the page, `Esc` clears it, and opening another page clears it too. The
  page title reads `Tasks</text>[3]`: the filter, then the rows shown. When nothing matches,
  the page says so rather than showing its empty state.
- `Shift` and the underlined letter of a column title sort that table by the column: ascending,
  then descending, then back to the page's own order. The sorted column shows its direction.
  Figures sort as numbers, and a value nobody recorded sorts after every known one in either
  direction. The selection stays on the row it was on.

- **Tasks** — the task graph, with state, assignee, reviewer, attempts, checks, results, the
  access each task declared, and any wait the runtime recorded against it. Each task also
  carries what the shared plan records for it: its own version, the member the plan holds
  responsible with the model and effort that responsibility was committed under, the proposal
  that committed it, and any turn the runtime put off rather than started. The changes agents
  asked for are listed under the tasks they are about, one row each, naming what the change
  asks for, the plan and membership version it was made against, why it was asked for, and
  what the runtime decided with the reason it recorded. The decision is the authority: a
  proposal nothing has answered reads as proposed rather than refused, and a proposal whose
  stored status moved without a readable decision says so. A plan that could not be read is
  reported as unavailable rather than as a plan with nothing on it.
- **Token usage** — what the loaded session spent, for the session as a whole and for each
  agent in it. Described in its own section below.
- **Sessions** — saved sessions for this project. `Enter` loads one for reading; `r`
  continues the run, which does start agents. While a run is active the list stays
  browsable, but the loaded conversation cannot be switched: it is the conversation your
  next message is delivered to, so replacing it would send that message to the wrong run.
  Stop the run with `/stop` first.
- **Files** — a read-only file explorer that starts in the working directory. It is described
  in [its own section](#files).
- **Changed files** — the changes recorded for the loaded session. The page opens on how the
  directory is used: agents work in it
  directly, what a turn may do is the access its backend enforces, turns overlap only where
  that access does not conflict, one run holds the directory at a time, nothing is staged or
  copied, and ymp cannot put an earlier version of a file back, because it recorded a path, a
  status and a content hash rather than a copy. It also names the version control found at or above the directory, and
  lists the accepted results of the session with the directory each was recorded in.
- **Recorded checks** — the acceptance commands ymp ran itself for the loaded session, with
  the directory, the recorded outcome and the captured output. A command the plan declared
  that has no recorded run is listed apart from the runs, because it is not a result. See
  [recorded checks and recovery limits](../architecture/recorded-checks-and-recovery.md).
- **Assignments** — the turns the run assigned for the loaded session: the agent, the
  purpose, the task attempt, the directory, the access the execution backend enforced for the
  turn with the reservation it was admitted under and any wait it went through, and the model,
  effort and permission mode that were requested, sent and reported. A turn still open is separated from the turns that
  finished, and the coordination permissions an assignment held are named with it. Where the
  session captured a token ceiling, each turn names the token allowance it was admitted with
  and whether its assignment requested it or inherited the session's per-turn default; an
  allowance is accounting, not a limit the installation enforced.
- **Decisions** — the plans, reviews, acceptances, rejections, competence credit, membership
  changes, per-turn bounds, captured acceptance criteria and directory reservations the session
  recorded, each with its actor, its time and the records it links. A captured contract shows
  what a result will be judged against, its checker, and each input it declares with the
  digest it was captured at; a contract that binds a correction also names the entry it
  corrects and which declared source replaced which. It says that it is a binding rather than
  a result. A record that carries its own outcome, such as a membership change, a change to the
  shared plan, a correction to what was retained, a turn bound or a wait, reads from that
  outcome in its row and in its record alike, and not from a grade it never had. A decision
  that changed the plan names the proposal behind it, who asked, the version the plan took on,
  and the responsibility it created; a correction names the entry it replaced, its
  replacement, the acceptance and contract that authorised it and the policy it was applied
  under. An acceptance
  states whether it rests on passing evidence for every criterion or on an independent
  review alone, and says when the files it was accepted against have changed since.
- **Team**, **Agent profiles**, **Providers** — membership and configuration. The team page
  shows the members of the loaded session, what each is responsible for on the shared plan
  with the settings that responsibility carries, any identity the session captured that its
  roster no longer lists, the agents that worked in it, the roster record itself with the
  final reviewer it keeps free, the bounds the roster was formed under, and the pool that is
  eligible on this machine, with the reason any profile is excluded. Each row names the model the
  agent would actually run as: the one its profile pins, or the default a catalog read from the
  installation reports. Where no catalog has been read, the row says so rather than showing the
  provider's label in place of a model, and the profile's detail says why the name is not known.
  The Providers page shows, per provider, where its catalog came from, what it lists and which
  default it reported. On Agent profiles, `m` edits the model and `i` edits the instructions;
  `Space` enables a profile and `t` toggles membership; `r` re-reads what is installed and the
  catalog already stored, which asks no provider anything. Changes are validated and saved to
  `config.toml`; a rejected change is reverted and reported.
- **Memory** — every entry recorded for this project and as shared procedure, including
  candidates, superseded and retired ones, each with the standing the store gives it now:
  current, superseded, retired, rejected, or the reason it is not offered, which may be that
  it was recorded for other conditions, that its source moved on, or that nothing confirms
  it. An entry is either a projection of a result this project accepted or a candidate a run
  proposed: confirmed means the acceptance it names carried passing checks, unconfirmed means
  no passing evidence is attached, and an entry written before provenance was recorded says
  its confirmation is unknown. The page also says, per entry, whether a run assembling a
  prompt would actually be given it, which is decided by re-reading the source record and not
  by the text of the entry. An accepted correction keeps both sides: the superseded entry
  names its replacement, the replacement names what it replaced, and each carries the
  acceptance, the trusted contract and the policy the correction was authorised by, with its
  own evidence. The page reads under the conditions in `knowledge_scope`, the same ones a run
  asks under, and every entry says what those were. `s` searches the recorded memory, `/`
  filters the rows already listed, and `f` retires an entry after a confirmation.
- **Reputation** — the observations behind competence estimates, with their evidence, their
  evidence status, and what credit toward selection requires.
- **Limits** — the limits the loaded session captured, shown apart from the ones the next run
  would use. Beside the turn, parallel, timeout and attempt bounds, the captured rows give the
  token ceiling with what reported counts leave under it, the per-turn allowance, the review
  tokens still protected and how that figure was reached, the policy the session captured for
  counts an installation left incomplete, and whether a strict token bound holds. Where a count
  is incomplete, a remainder is stated as a reported one and never as a known one. `+` and `-`
  adjust the next run; `Enter` types a value. A captured limit is a record and cannot be edited.
- **Help** — every command and key, generated from the command registry.

## What a run recorded

`/assignments` and `/decisions` read the records a run writes as it works, and they keep apart
the things that are easy to confuse. Requested, sent and reported are three separate columns: a
value that was sent and never reported back is shown as unconfirmed, never as applied. A record
with no end is a turn in flight only while a run is active in this window; in a stored session
the same record means a turn that was left open. An acceptance is either confirmed, meaning the
evidence it bound passed for every criterion it applies to, or unconfirmed, meaning it rests on
an independent review, or unknown, which is never read as a pass. Accepted work stays accepted
even after the files it named change, and the page says that they changed. The vocabulary and
its limits are written out in
[assignment, budget and confirmation views](../architecture/assignment-and-confirmation-views.md).

These pages read a snapshot the controller takes when the window opens, when a session is
loaded, when a run reports while one of them is open, and when the page itself is opened. Each
page states when its records were read, because a run still working writes more of them.

## Token usage

The header carries the session total, the sidebar's `TOKENS` section repeats it and lists
what each agent has spent, and `/usage` opens the full page. Loading a saved session restores
the statistics it recorded, and the page is a read-only projection like every other: opening
it starts nothing.

While a run is active in this window, the header and the sidebar report the session as
running rather than repeating the status stored before it started, and the turn counter
follows the invocations the statistics account for instead of waiting for the run to write
its own counter. Both return to the stored record once the run ends. Browsing a saved session
shows what was stored, unchanged.

Every figure updates as soon as its provider reports, which is not the same moment for every
provider and need not be during the turn. Claude streams its usage, and Codex notifies after
each completed model request, so both appear while a turn is still running. The installed GLM
ACP agent exposes only its last request's usage, and only when its native turn completes, so
a GLM agent shows no figure until then and a partial one afterwards. How each provider is
read is described in the [token accounting](../architecture/token-usage.md) guide.

Every figure belongs to an **agent**, never to a provider. Two agents configured on the same
provider are two rows with two counters, and the provider is shown beside the name only as
metadata. An agent that spent tokens in the session without being in the team it captured is
listed separately rather than dropped.

The page shows input, output, cache reads and writes, and reasoning when a provider reports
it. Cache reads and writes are part of the input, and reasoning is part of the output, so
they are indented under the figure that already contains them and are never added to the
total a second time.

What the interface will not do is fill a gap:

| Reading | Meaning |
| --- | --- |
| `0` | The store knows the amount, and it is zero. |
| `—` | Nobody reported the amount. It is unknown, not zero, and is never estimated. |
| `4200+` | At least this much. Something it counts is not finished or not attributable. |

A total is marked `+` when an invocation is open, when a provider reported only part of the
turn it ran, or when the session holds invocations that cannot be attributed to an agent.
The coverage behind every figure is written next to it in words: how many of the session's
invocations reported, how many were partial, how many are open, and how many were recorded
without an agent. The installed GLM ACP agent reports only the last request of a turn, so its
invocations count as partial and its figures stay lower bounds.

An **open** invocation is one with no final status recorded. During an active run that means
a turn is in flight. In a stored session it means no final status was ever written, which is
what an interrupted run leaves behind; it is not evidence that anything is still running,
and the page says so differently in each case.

An older session can record how many turns it took without recording who took each one.
Those invocations are counted in the session total and reported as unattributed. While there
are any, no agent figure is presented as complete, and an agent with no recorded invocation
of its own reads as `—` rather than zero, because one of the unattributed invocations may be
its own. Nothing is divided between agents to make the figures add up.

## Files

`/files` opens a file explorer in the directory ymp was started in. The explorer is the
`ratatui-explorer` widget; ymp gives it its keys, its colours and the names its rows show. The
header names the directory being listed and, in brackets, how many entries it lists.

```text
 Files[3]  /files                                          /path/to/project/src
───────────────────────────────────────────────────────────────────────────────
   ../
 › nested/
   to-shared -> ../shared/
   main.rs
```

Moving to another directory, including one above the start, changes only what the page shows.
Runs still work in the directory ymp was started in, and the loaded session and every provider
setting stay as they were. Nothing on the page writes, renames or deletes anything, and nothing
it shows is executed.

The first row leads to the parent directory, except at the root of the filesystem. Directories
follow, then every other entry, each group in order of the name as shown. A directory's name ends
in `/`, and a symbolic link shows the target it stores after `->`. Hidden entries are listed;
`.git`, `node_modules`, `target`, `__pycache__` and `.ymp2` are not listed in any directory.

| Key | Action |
| --- | --- |
| `Up` / `k`, `Down` / `j` | Move the selection; it wraps from one end of the list to the other |
| `Home`, `End` | Select the first or the last row |
| `PageUp`, `PageDown` | Move twelve rows |
| `Enter`, `Right`, `l` | List the selected directory, go up from `../`, or show the start of a file |
| `Backspace`, `Left`, `h` | Go to the parent directory, with the directory just left selected |
| `/` | Filter the entries by name without regard to case; a leading `!` keeps the others, and `../` always stays |
| `d` | Describe the selected entry: its path, kind, size, the target a link stores, and the bytes of a name that is not UTF-8 |
| `r` | Read the directory again, keeping the selection on its entry |
| `Esc` | Clear the filter, then close the page |

Entering or leaving a directory clears the filter. A symbolic link to a directory is listed where
it leads, so the header shows the real directory and `Backspace` goes to that directory's parent.
When a directory cannot be listed, because it was removed or its permissions refuse it, the list
stays as it was and a line above it gives the reason; `r` reads the directory on screen again.

A directory is read when the page opens, on every move, filter change and `r`, and when a run
finishes while the page is open. It is not watched in between. The whole directory is read at
once, with a metadata request for every entry, while the interface waits, so a very large
directory or a slow network mount holds the interface until it has been read.

Names are shown so that they can neither drive the terminal nor pass for another name. Control
and bidirectional formatting characters are written as escapes such as `\u{1B}`, a newline, tab
or carriage return as `\n`, `\t` or `\r`, a byte that is not UTF-8 as `\xFF`, and a backslash
as `\\`. Every row still opens the exact file its directory holds, so `same-a\nb.rs`, whose name
contains a newline, and `same-a b.rs` open their own contents.

### Previews

`Enter` on a file opens a read-only preview. At most 256 kB of the file is read, at most 5,000
lines are shown, and a line shows at most 1,000 characters; the preview states each cut. `Up` and
`Down` scroll, `PageUp` and `PageDown` move a page, `Home` and `End` jump, and `Esc`, `Enter`, `q`,
`Backspace`, `Left` or `h` return to the list.

A FIFO, socket or device is never opened, and the preview says what it is. A file with a NUL byte
in what was read is described as binary. A UTF-16 or UTF-32 byte order mark, or bytes that are not
UTF-8, are named rather than decoded. An empty file says so, a broken link names the target it
stores, and a file removed since the list was read says so while the list is read again.

Text keeps its exact characters. Tabs expand to the next multiple of four columns. A control
character appears in caret notation, such as `^[`, and a bidirectional formatting character by
its code point, such as `<U+202E>`, both in the warning colour, so nothing in a file reaches the
terminal as an instruction.

### Syntax highlighting

A preview is highlighted when a grammar matches the file's name, its extension, or its first line,
such as `#!/usr/bin/env python3`. The grammars are the syntax definitions the bat project collects,
bundled by two-face 0.5.2 and run by syntect 5.3.0; tui-syntax-highlight 0.2.0 turns each
highlighted line into terminal styles. The colours are the theme's own rather than a fixed colour
scheme: comments take the muted colour, strings the positive colour, numbers and constants the
attention colour, keywords and tags the accent, types the informational colour, and functions the
text colour. A colour that would not read on the preview's background gives way to the theme's
text colour, in every light and dark theme.

ARM Assembly, JavaScript (Babel), LiveScript, PowerShell, Sass and Salt State SLS are not in this
bundle. Such a file, like any file no grammar matches, is shown as plain text, and the preview says
that no grammar matched. Highlighting is bounded: after a line longer than 4 kB, or after 250 ms,
the rest of the file is plain text, and the preview says where highlighting stopped and why.

Fenced code that names a language, such as a block opened with ```` ```rust ````, is highlighted
the same way in the popup `Enter` opens for a message. The transcript itself does not highlight
code. `/help` links the licences and notices of the bundled syntax definitions.

## Themes

`/theme` or `Ctrl+T` opens the chooser. Moving the selection previews the palette across the
whole interface; `Enter` keeps it and `Esc` restores the stored choice. `/theme ID` sets one
directly. The selection is saved with the other interface preferences under the ymp home
directory and restored at the next start. A saved identifier that is no longer offered, such as
`paper`, `contrast` or `terminal` from earlier versions, starts with Ember instead; the other
preferences are kept.

Ember and Slate are ymp's own palettes. The other sixteen come from the
[`ratatui-themes`](https://crates.io/crates/ratatui-themes) crate: each uses the library's
background, foreground (for body text), accent, muted and state colours, and ymp derives its
heading, panel, code, hairline, label and selected-row colours from them. Headings are the
foreground moved away from the background, and text on a selected row is whichever of the
palette's background and foreground contrasts more with its accent. Solarized keeps its
deliberately soft foreground.

| Theme | Kind | Description |
| --- | --- | --- |
| `ember` | dark | Warm amber on black. The default. |
| `slate` | dark | Cool blue on deep slate, low saturation. |
| `dracula` | dark | Purple and pink accents on a dark violet grey. |
| `one-dark-pro` | dark | Atom's balanced dark palette with a blue accent. |
| `nord` | dark | Arctic, north-bluish colours. |
| `catppuccin-mocha` | dark | Soothing warm pastels on the darkest Catppuccin base. |
| `catppuccin-latte` | light | Catppuccin's warm pastels for bright rooms. |
| `gruvbox-dark` | dark | Retro groove colours on a warm dark background. |
| `gruvbox-light` | light | Retro groove colours on warm cream. |
| `tokyo-night` | dark | Futuristic deep blue with bright accents. |
| `solarized-dark` | dark | Precision-tuned colours on the dark Solarized base. |
| `solarized-light` | light | Precision-tuned colours on the light Solarized base. |
| `monokai-pro` | dark | Classic syntax-highlighting colours on charcoal. |
| `rose-pine` | dark | Muted rose, iris and pine on a deep base. Shown as Rosé Pine. |
| `kanagawa` | dark | Ink and wave tones inspired by Katsushika Hokusai. |
| `everforest` | dark | Comfortable green forest tones. |
| `cyberpunk` | dark | Neon-soaked futuristic colours. |
| `midnight-commander` | dark | The classic file manager: white and cyan on dark blue. |

The chooser lists all eighteen. In a short window it scrolls with the selection, and `Home` and
`End` reach the first and the last palette. In a narrow window the colour chips beside each name
are dropped before a name is cut.

No state is expressed by colour alone. Selection carries a marker and a reversed row, task
and session states are printed as words, the focused region is named in the status row, and
the markers fall back to ASCII when the locale says the terminal cannot render them.

The standard `NO_COLOR` environment variable is respected. If it is set, theme preferences are
still saved, but terminal colours are suppressed. Unset it to display the selected palette.

## Screenshots

Captured from a deterministic demonstration session:

![Ember theme with the right sidebar](images/ember.png)

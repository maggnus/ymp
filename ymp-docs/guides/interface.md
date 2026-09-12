# Interface

`ymp` opens a chat-first terminal workspace: the conversation fills the main column, and a
right sidebar carries navigation, live team activity, and the current session's context.

## Layout

```text
ymp  project  /path/to/project                    3fa27c81  ✓ completed  12 / 200 turns
──────────────────────────────────────────────────────────────────────────────────────────
  ▌ you                                                    12:04  │ NAVIGATE
  ▌ Add validation and tests for the import command                │ ● Conversation
                                                                   │   Tasks            3
  · Codex proposed a plan · 2 tasks · Validate then test     12:04 │   Sessions
  · Claude review · accepted · The parser rejects empty rows 12:07 │   …
                                                                   │ SESSION
  ◆ Codex · final result                                    12:09  │ 3fa27c81 ✓ completed
    Added `validate_row` and six tests. `cargo test` passes.       │ turns      12 / 200
                                                                   │ dir  …/project
                                                                   │ TEAM
                                                                   │ ● Codex     working
                                                                   │ ○ Claude    idle
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

The sidebar's team section names the profiles the loaded session captured when it started,
and says so. The team page describes the configuration for the next run instead, which may
have been edited since; the two are never presented as the same thing.

- `Tab` moves the focus into the transcript, then `Up`/`Down` select an entry.
- `Enter` opens the complete attributed message, with its author, kind and timestamp.
- `Space` expands a long execution report in place.
- `/details` or `Ctrl+L` switches the whole transcript to full attributed messages.

Scrolling up pauses auto-follow. The status row then reads `⏸ paused` and names the key
that returns to the newest message, which is `End`. Text that streams in while you are
reading is appended below without moving your position.

## Keyboard

One region owns the keyboard at a time, and the status row names it. `Tab` cycles composer →
transcript or page → sidebar. `Esc` always removes the topmost thing: an open overlay, then
the command completion list, then the open page, then a focus that is not the composer, and
finally the draft in the composer.

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
| `Ctrl+C` | Stop an active run, or leave ymp when idle |

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

Every destination in the sidebar is a read-only projection. Opening one never starts an
agent and never writes to your working directory. Each page is a list with the detail of the
selected row underneath it, and states its own keys in the status row.

- **Tasks** — the task graph, with state, assignee, reviewer, attempts, checks and results.
- **Sessions** — saved sessions for this project. `Enter` loads one for reading; `r`
  continues the run, which does start agents. While a run is active the list stays
  browsable, but the loaded conversation cannot be switched: it is the conversation your
  next message is delivered to, so replacing it would send that message to the wrong run.
  Stop the run with `/stop` first.
- **Files** and **Changed files** — the working directory, and the changes recorded for the
  loaded session.
- **Team**, **Agent profiles**, **Providers** — membership and configuration. On Agent
  profiles, `m` edits the model and `i` edits the instructions; `Space` enables a profile
  and `t` toggles membership. Changes are validated and saved to `config.toml`; a rejected
  change is reverted and reported.
- **Memory** — verified project and global knowledge. `/` searches, `f` retires an entry
  after a confirmation.
- **Reputation** — the observations behind competence estimates, with their evidence.
- **Limits** — turn budget and parallelism. `+` and `-` adjust; `Enter` types a value.
- **Help** — every command and key, generated from the command registry.

## Themes

`/theme` or `Ctrl+T` opens the chooser. Moving the selection previews the palette across the
whole interface; `Enter` keeps it and `Esc` restores the stored choice. `/theme ID` sets one
directly. The selection is saved with the other interface preferences under the ymp home
directory and restored at the next start.

| Theme | Kind | Description |
| --- | --- | --- |
| `ember` | dark | Warm amber on black. The default. |
| `slate` | dark | Cool blue on deep slate, low saturation. |
| `paper` | light | Dark ink on warm paper. |
| `contrast` | high contrast | Maximum separation on pure black. |
| `terminal` | terminal palette | Inherits the sixteen colours your terminal defines. |

No state is expressed by colour alone. Selection carries a marker and a reversed row, task
and session states are printed as words, the focused region is named in the status row, and
the markers fall back to ASCII when the locale says the terminal cannot render them.

The standard `NO_COLOR` environment variable is respected. If it is set, theme preferences are
still saved, but terminal colours are suppressed. Unset it to display the selected palette.

## Screenshots

Captured from a deterministic demonstration session:

![Ember theme with the right sidebar](images/ember.png)

![Previewing the Paper theme](images/theme-preview.png)

![Conversation and follow-up in Paper](images/paper-conversation.png)

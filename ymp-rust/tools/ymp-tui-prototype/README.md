# ymp — persistent entity-management prototype

**Closed by owner request.** Development of this subproject is stopped. The source and artifacts
are retained as an archived prototype, not an accepted product direction. Do not resume this work
from old goals, schedules or plans without a new explicit owner instruction.

An independent Rust crate with a working terminal UI and deterministic agent-lifecycle simulation.
Configuration, tasks, history, drafts and exported reports are stored locally. No real model,
provider authentication, game process or generated project source is supplied by this prototype.
`SIMULATION` remains visible. The real product may use external development tools; it does not
embed the produced game in its interface.

## Launch

```sh
/Users/maggnus/.local/bin/ymp
```

The normal first start contains no connections, tasks or agents. State lives in
`~/.ymp-prototype/state.json`, separately from real `~/.ymp` data and project source. Another instance
cannot overwrite an open checkpoint. `--state FILE` selects another checkpoint; `--ephemeral`
starts a disposable preview. Corrupt or unsupported checkpoints are reported and preserved.

## Navigation and editing

Use `/` or Ctrl K for commands. Core screens are `/chat`, `/tasks`, `/agents`, `/board` and
`/providers`; related records lead to `/tools`, `/checks`, `/files` and history.

On a table, `n` or `/add` creates an entity, `e` edits a selected proposal/connection, and `.` or
`/actions` opens its contextual menu. The menu identifies its target and explains unavailable
operations. Enter opens details; related records are reachable from details and the menu.
There are no permanent `+ Add` or `Actions` toolbar buttons.

Forms use Tab/Shift Tab to move fields, arrows or Space to choose a value, and Ctrl S or the Save
control to submit. Cancel leaves the existing entity unchanged. Removal/stop confirmations default
to Cancel and state their effect before execution.

`f`/Ctrl F edits a filter, Ctrl U clears it, and Enter hides the filter input again. A query remains
in the title. Exact filters include `id=P-0001`, `agent=A-0002` or `author=A-0002`. Table selection,
sort and filters survive navigation; conversation drafts are scoped to tasks and restored on
restart. `?` contains keyboard help. Routine key instructions and duplicate row-range counters do
not occupy the data view. The established palette is retained; OpenCode was a right-panel reference
only, not a source of a replacement theme or left-hand design.

## Available entity operations

| Entity | Operations and boundary |
| --- | --- |
| Connections | Add, rename/change supported type, enable, disable, refresh simulated profile, remove. Supported types are a catalog in the form, not pre-created accounts. Active use blocks removal. Old agents/runs retain their frozen connection names and profiles. |
| Tasks | Create local draft, edit unused proposal, select conversation, start/stop/retry development, cancel, delete unused draft, archive/restore, export record. Referenced execution or export history blocks deletion; archiving retains it. |
| Agents | Add an explicit assignment to a permitted running task, choose a frozen route, stop one agent, create a replacement attempt, archive/restore terminal records, export. Stopping the origin stops its run; stopping a helper does not stop the origin. Archive never refunds a consumed agent-start allowance. |
| Board | Inspect attributed messages, follow their authors and references, export. Statements are immutable history rather than verification verdicts. |
| Tools | Inspect owned process state/output, stop a running process, follow its agent, export output. Process termination does not erase the record. |
| Checks | Inspect/export evidence tied to its run and candidate. A failed earlier candidate remains in history. |
| Files/reports | Inspect immutable simulated candidate manifests, export records to real local JSON reports, remove unchanged application-owned exports. Changed files or redirected paths are refused. Removing an export does not delete evidence or project files. |

`/limits 6 3` sets future-run ceilings, not a desired team size. A new agent needs an initiator,
assignment, permitted frozen route and capacity. Configuration changes never rewrite existing
launch settings. Context menus keep their original entity even if a live filter changes its row.

## Battleship scenario

1. `/scenario` records the request. Configure a connection through `/providers` → `n`; enable it
   in the form or its menu. Configuration alone creates no agent.
2. `/continue` creates the observable clarification session. Answer in Chat or confirm the proposed
   rules. Review the proposal in Tasks.
3. `/start` authorizes development and freezes its profile and limits. The right panel shows the
   current task, next relevant operation, agents, tool state and candidate checks.
4. The simulation records a scoped help request, admitted recruitment, a failed placement check,
   a revised candidate, external-tool records and separate verification. `/agents` → `n` can add
   an operator assignment; its admission is governed by the same frozen limits.
5. `/checks` and `/files` expose the evidence. `/accept` records simulated delivery. No game source
   is copied into the project. `/stop` and `/retry` retain earlier attempts and outcomes.

Ctrl P pauses the demonstration timeline for inspection/editing. The normal run takes about
21 seconds without pauses or extra assignments. Agent AGE is since recorded creation, board AGE
since publication, and provider CHECKED since a simulated observation. Completed execution
intervals remain separate from increasing record age. Unknown timestamps remain `—`.

On reopening after an interruption, unconfirmed running simulations are marked stopped and need
an explicit retry. Opening the program never silently recreates processes. A fixed Battleship
scenario is used to demonstrate lifecycle behavior; typed requirements are retained, not processed
by a real language model. This is not a cumulative-knowledge or research-POC result.

## Build and previews

```sh
cargo build --manifest-path ymp-rust/Cargo.toml -p ymp-tui-prototype
ymp-prototype --scenario ready --snapshot /tmp/ymp-ready.svg
ymp-prototype --scenario working --view agents --snapshot /tmp/ymp-agents.svg
ymp-prototype --scenario empty --view providers --form provider --snapshot /tmp/provider-form.svg
ymp-prototype --stress 512 --view agents
```

Previews are ephemeral unless a fresh explicit checkpoint path is supplied. Scene names are
`empty`, `provider`, `clarify`, `ready`, `starting`, `working`, `failed`, `checking`, `review`, `done`.
`--detail`, `--actions`, `--form provider|task|agent` and `--expand-tools` expose the corresponding
real UI states. Large populations exist only in explicit stress fixtures. Only the visible table
page is rendered; matching and ordering are cached between relevant changes.

SVG/text snapshots come from the production Ratatui cell buffer. `render_snapshot.py` converts
these cells to PNG. See [management acceptance](MANAGEMENT.md) and [the product scenario](SCENARIO-BATTLESHIP.md).

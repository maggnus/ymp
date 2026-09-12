# Usage

Start `ymp` in the project directory. The first launch creates `~/.ymp2/config.toml` and discovers local executables. Check `/providers` and `/team` before starting a task.

The layout, the keyboard contract and the colour themes are described in the [interface guide](interface.md). `Ctrl+P` opens the command palette and `/help` lists every command and key.

## Profiles and membership

```text
/agent add reviewer codex
/agent instructions reviewer Focus on independently reproducing failures and checking edge cases.
/team add reviewer
/team remove glm
/agent model reviewer default
```

An agent profile's instructions and model determine its experience identity. Changing them starts a new version for future sessions. Existing sessions keep the captured profile definitions.

To configure a provider's command, arguments, or environment references, edit the file printed by `ymp config --path`. `env_refs` maps the variable expected by the provider to a variable already present in the invoking environment. Do not paste secrets into profile instructions.

## Inspecting and continuing work

`/tasks` shows assignments, attempts, and results; `Enter` opens the description, checks and reported result of the selected task, with the acceptance decision recorded for it and whether that acceptance was confirmed by evidence or rests on an independent review alone.

`/assignments` shows every turn the loaded session assigned: the agent, the purpose, the task attempt, the directory, and the model, effort and permission mode that were requested, sent to the installation and reported back by it. A value nobody reported is shown as unconfirmed rather than as applied, and a fixed model or effort is named as the constraint it is. `/decisions` shows the plans, reviews, acceptances and competence credit the session recorded, with the reviewer each acceptance links and the result version it rests on.

`/usage` shows what the loaded session spent, for the session and for each agent in it. A figure updates as soon as its provider reports: Claude streams usage and Codex notifies after each completed model request, both during the turn, while the installed GLM ACP agent exposes only its last request and only at native turn completion. Figures are what the providers reported, never an estimate: an amount nobody reported is shown as a dash rather than as zero, and a total that an open or partly reported invocation can still add to is marked with `+`. The [interface guide](interface.md) describes the page, and [token accounting](../architecture/token-usage.md) describes how each provider is read.

`/sessions` lists sessions for the current project. `Enter` loads one for reading, which does not start agents; `r` on the list, or `/resume SESSION_ID`, continues the run and inspects unfinished work before doing so. Every other page reachable from the sidebar is a read-only projection.

Use `/pause` or Ctrl+C to stop active turns. Native agent sessions and files in the working directory are retained. `/limits` shows the limits the loaded session captured apart from the ones a later run would use, together with what the session admitted, what it reserved for review, what it observed and any stop the budget recorded. A captured limit is a record and cannot be edited there. Increase the turn limit before resuming a run that exhausted its budget:

```text
/limits turns 300
/resume SESSION_ID
```

`/new` clears the view when idle. The next prompt starts an unrelated session. Otherwise, idle messages continue the current conversation; questions do not rerun the task. Messages submitted during an active run are added to its shared chat.

Profiles can also be edited from the pages: on `/agents`, `m` sets the model, `i` writes the instructions, `Space` enables a profile and `t` toggles team membership. On `/providers`, `Space` enables or disables a provider. On `/limits`, `+` and `-` adjust a value and `Enter` types one. Each change is validated before it is written; a rejected change is reverted and the reason is shown.

## Results and experience

Files are written directly in the working directory. The final output names that directory, and `/diff` displays changed-file metadata. A failed or stopped run may leave partial changes for inspection. What `/diff` records is a path, a status and a content hash, never a copy, so ymp cannot restore an earlier version of a file; the page says so and reports what version control it found at or above the working directory. `/checks` lists the acceptance commands ymp ran itself, with the directory, the outcome and the output recorded for each, and a declared command with no recorded run separately. Checks run with no limit on what they may do and nothing is asked before they run, which [recorded checks and recovery limits](../architecture/recorded-checks-and-recovery.md) describes in full.

`/memory QUERY` searches the knowledge recorded for this project and as shared procedure; on the page, `/` searches and `f` retires the selected entry after a confirmation. `/memory forget ID` retires an entry by identifier. An entry names its author and, where one was recorded, its reviewer; an entry with no reviewer is shown as a candidate rather than as a checked fact, and nothing re-checks an entry against the source it came from. `/reputation` shows the observations underlying competence estimates, each with its evidence status: only a confirmed observation that a session credited counts toward who a later run may pick. A high mean from a small number of observations should not be read as certainty.

Runs without a deterministic check still require independent review, but that review is a subjective assessment. Inspect important results directly.

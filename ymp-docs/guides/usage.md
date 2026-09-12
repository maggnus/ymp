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

`/tasks` shows assignments, attempts, and results; `Enter` opens the description, checks and reported result of the selected task.

`/sessions` lists sessions for the current project. `Enter` loads one for reading, which does not start agents; `r` on the list, or `/resume SESSION_ID`, continues the run and inspects unfinished work before doing so. Every other page reachable from the sidebar is a read-only projection.

Use `/pause` or Ctrl+C to stop active turns. Native agent sessions and files in the working directory are retained. Increase the turn limit before resuming a run that exhausted its budget:

```text
/limits turns 300
/resume SESSION_ID
```

`/new` clears the view when idle. The next prompt starts an unrelated session. Otherwise, idle messages continue the current conversation; questions do not rerun the task. Messages submitted during an active run are added to its shared chat.

Profiles can also be edited from the pages: on `/agents`, `m` sets the model, `i` writes the instructions, `Space` enables a profile and `t` toggles team membership. On `/providers`, `Space` enables or disables a provider. On `/limits`, `+` and `-` adjust a value and `Enter` types one. Each change is validated before it is written; a rejected change is reverted and the reason is shown.

## Results and experience

Files are written directly in the working directory. The final output names that directory, and `/diff` displays changed-file metadata. A failed or stopped run may leave partial changes for inspection.

`/memory QUERY` searches verified project knowledge and shared procedures; on the page, `/` searches and `f` retires the selected entry after a confirmation. `/memory forget ID` retires an entry by identifier. `/reputation` shows the observations underlying competence estimates; a high mean from a small number of observations should not be read as certainty.

Runs without a deterministic check still require independent review, but that review is a subjective assessment. Inspect important results directly.

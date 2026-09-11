# Usage

Start `ymp` in the project directory. The first launch creates `~/.ymp2/config.toml` and discovers local executables. Check `/providers` and `/team` before starting a task.

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

`/tasks` shows assignments, attempts, and results. `/sessions` lists sessions for the current project; `/resume SESSION_ID` loads an interrupted session and inspects unfinished work before continuing.

Use `/pause` or Ctrl+C to stop active turns. Native agent sessions and candidate working copies are retained. Increase the turn limit before resuming a run that exhausted its budget:

```text
/limits turns 300
/resume SESSION_ID
```

`/new` clears the view when idle. A new prompt starts a new session. Messages submitted during an active run are added to its shared chat.

## Results and experience

The final output names the integration directory. `/diff` displays its patch after a run stops. No source-directory changes are applied automatically.

`/memory QUERY` searches verified project knowledge and shared procedures. `/memory forget ID` retires an incorrect entry. `/reputation` shows the observations underlying competence estimates; a high mean from a small number of observations should not be read as certainty.

Runs without a deterministic check still require independent review, but that review is a subjective assessment. Inspect important results directly.

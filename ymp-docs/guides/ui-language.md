# UI language

Owner direction, 2026-09-14: use concise professional English throughout the UI.
Do not turn implementation explanations into table values or status labels.

## Main surfaces and details

- A row should identify the object, its relevant state and available action.
  Keep ordinary titles, column labels and values short and consistent.
- Remove `read from the installation` from primary agent rows. Preserve native
  source, method, observed time and related provenance in Inspect/details under
  a concise label such as `Source`; do not turn discovery into a success claim.
- Display known team-membership booleans as `true` and `false`, not `in team`
  versus an empty cell. Preserve an explicit unknown where membership is not
  established. Name the scope: selected session or next-session configuration.
  A Config.team preference must not be labeled as effective current membership.
- Prefer one stable term for each concept: agent, provider, model, session,
  task, assignment, invocation, review and confirmation. Do not collapse distinct
  states merely to shorten text. In particular, accepted and confirmed differ.
- Use short status values, imperative action labels and direct error messages.
  Retain the cause and a useful next action in errors and empty states. Longer
  explanations belong in details or contextual help rather than repeated rows.
- Preserve exact native model IDs, actual effort values and reported controls.
  A provider's on/off thinking control must not be rewritten into a fabricated
  graded effort. Unknown metadata is not false, zero or an inferred default.
- User and agent message contents, raw retained evidence and native diagnostic
  payloads are not copy-editing targets. Improve the application's presentation
  without modifying source text or removing actionable failure information.

## Audit and implementation

Audit primary pages, shared tables, the sidebar, overlays, command palette/help,
notices, errors and empty states. Review repeated patterns and underlying data,
not only the owner's one example. Record a compact old/new glossary with reasons,
surface coverage and explicitly retained technical detail.

Use the shared renderers and terminology helpers. Avoid creating a new abstraction
or dependency merely to centralize a few strings. Do not duplicate /agents and
/team semantics or implement the separate coordination selector in this task.
Their intended responsibilities are in
[team and agent surfaces](../architecture/team-and-agent-surfaces.md).

Preserve keyboard actions, selected-session behavior, native identity, historical
attribution, theme/ASCII support and actual unknown/error states. Update existing
checks for intentional wording changes without weakening their behavioral claims.
Do not create one trivial test for every renamed label. Use existing rendering and
terminal checks to verify readability, clipping and meaning, then run the required
formatting, strict Clippy and workspace tests on the final implementation.

The review begins read-only while YMP-145 is independently accepted. UI edits
follow its accepted component change, by Claude Code claude-opus-5 high. P0 session
recovery remains independent and must not be delayed by this language work.

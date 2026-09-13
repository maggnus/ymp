# YMP-149 terminology audit

Inventory completed read-only by Claude Code claude-opus-5 high, agent
`8d8b7748-c2b4-4468-8585-4be62e5bbec0`, against main `305de15`. Its TUI source
matches `bf894e3`; popup commit `35786ec` remained frozen. No source changes,
builds, tests or application provider calls were made for the inventory.
This is preparation evidence, not an implemented or installed wording change.

## Maintainer decisions

| Current presentation | Required treatment |
| --- | --- |
| `READING` and `read from the installation` | Remove the primary column from agent/member tables; no replacement metadata-status column |
| Native source, freshness, method and observed time | Retain in Inspect under concise source fields; preserve actionable unavailability and unresolved identity |
| Config.team-derived `in team` / empty | Known `true` / `false`, explicitly scoped to the next-session preference until the live UI150 consumer replaces it |
| Membership actions claiming current join/leave | Say which next-session preference changed; do not present the operation as current-session mutation |
| ENABLED on/off | Retain the existing actual switch semantics; native on/off controls remain exact |
| Long provider exclusion sentences | Use concise accurate causes such as `profile disabled`, `provider disabled`, `executable not found` |
| Pool/catalog terminology | Preserve the distinction between eligible pool and native catalog; no global synonym replacement |
| `experience identity` | Explain configuration-specific experience only in details; do not invent a persisted profile-version event or guaranteed experience reset |
| Instructions-saved notice | Prefer the factual `Instructions for {id} saved.`; avoid promising when a captured running session will adopt changes |
| Varied Inspect/open-record hints | Standardize the same action, retaining distinct actions such as opening a file, reading a session or executing a command |
| Primary `(s)` / `(ies)` count forms | Use the existing local singular/plural convention; no new text framework |

Missing provider-reported metadata should say `not reported` where needed. Do not
use `unconfirmed` for absent settings: result confirmation is a distinct domain
concept. An absent default field does not prove that no native default exists;
preserve unknown versus explicitly absent information.

A `left open` record is not automatically running or known to be interrupted.
Keep its uncertainty. Do not replace a precise in-process label with `built in`
unless actual implementation origin supports that statement. No broad branding
or learning-benefit claim is part of this copy edit; retain already concise CLI
descriptions where they are accurate.

## Source findings

- `ymp-tui/src/views.rs`: `AGENT_COLUMNS` at 2862 and `MEMBER_COLUMNS` at 1813
  carry READING. `identity_row_words` at 2148–2162 supplies source narration.
- `/agents` membership at views.rs2892 uses `Config.team`, not selected-session
  membership. `state.rs:1896` and slash/key consumers mutate those preferences.
- Profile/execution/backend version hashes and applicable experience lookup
  explain the old experience-identity wording. Configuration changes do not
  imply a standalone saved version event, and existing migration mappings can
  preserve past experience.
- Some legacy resume paths may use current configuration where historical
  limits were never captured. Avoid mechanically changing every `next run` to
  `next session`; verify the specific consumer and state its actual scope.

These references are to the inspected revisions, before the pending wording
implementation. Source paths are relative to `ymp-rust/crates`.

## Coverage

The inventory covered primary TUI pages and tables, member/candidate rows,
sidebar, overlays, palette/help, key/filter/sort hints, formatted count/status
strings, notices, errors and empty states. A follow-up pass reviewed human-readable
CLI init/catalog-scan/doctor output and left already concise runtime status
messages unchanged. Raw user/agent messages, persisted evidence, JSON outputs and
MCP protocol fields are excluded from copy editing.

Useful CLI corrections include replacing human-readable `actor` with `agent`,
retaining `catalog entries` when entries may include unresolved aliases, and
labeling `health.available` as availability rather than claiming installation.
Keep native-discovery versus inference behavior explicit where that distinction
helps the user choose an action.

## Implementation and remaining verification

Implement the accepted wording through the common helpers after YMP-145 source
acceptance. Extend neither coordination policy nor live-team mutation behavior
in this task. Follow the shared language guide and the scoped UI150 plan.

Verify narrow-terminal readability after column removal, known/unknown and
current/next-session distinctions, preserved diagnostics, actual keyboard
actions and source-message immutability using existing checks. Update expected
wording without weakening behavioral assertions; do not add a trivial test for
each renamed label. Required formatting, Clippy, workspace and terminal checks
have not yet run for the language implementation because it has not started.

# UI redesign verification

The main redesign and first review corrections were implemented by Claude Code (Claude Opus 5). The parent independently reviewed the source, exercised the real terminal interface, and completed multiline instruction editing.

## Automated checks

Version: `0.2.0`. `cargo fmt --all --check`, Clippy with warnings denied, all **64 workspace tests** (including **50 UI tests**), and the Claude bridge TypeScript check passed. The installed release binary also passed a terminal smoke test.

## Coverage

The terminal run used a new temporary working directory, a separate application home, and the deterministic demo provider. No model requests were made by the product during these checks.

Verified in an actual pseudo-terminal:

- The right sidebar is present and keyboard-operable.
- `/theme` opens on one Enter; changing the highlight previews the palette, and Esc restores the saved theme.
- Selecting Paper persists the choice; restarting restores its colours.
- A team task creates `greeting.txt` directly in the selected working directory.
- Asking where the file is located answers in the same session without repeating execution.
- Opening a saved session does not start a provider turn.
- Pasting Unicode and multiline instructions edits the focused profile field and preserves whitespace.
- The file page shows the created file.
- At 80x24, focusing the sidebar and pressing End leaves Help selected and visible.
- Exit restores the terminal's normal screen.

The unit suite additionally checks active-session switching guards, immutable presentation of the captured team, exact code whitespace, bounded stream previews, input focus, theme cancellation, and all screens across representative viewport sizes.

## Screenshots

These are captures of a demonstration session, not claims about model quality:

- [Ember](../../ymp-docs/guides/images/ember.png)
- [Theme preview](../../ymp-docs/guides/images/theme-preview.png)
- [Conversation in Paper](../../ymp-docs/guides/images/paper-conversation.png)

`NO_COLOR` was unset for the colour captures. A separate monochrome run confirmed that labels and selection markers remain understandable without colour.

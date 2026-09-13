# Direct Paseo message audit

Audited on 2026-09-13. Installed baseline: ymp 0.4.2, main `9a5d274`.

The maintainer read user-message history for eight current ymp2 Paseo conversations with
`paseo logs AGENT --filter user_message`. Delegated maintainer prompts were distinguished from
direct owner messages. This audit did not start an agent, run provider inference, or install code.
The CLI transcript does not supply message timestamps; commit timestamps are not used as a
substitute. The requirements below are English translations of the direct messages.

## Current UI requirements

Source: agent `1e1a7a7b-030f-4eca-9d9f-9a02c32b1cd9`,
"YMP-132 model IDs and invocation labels", Claude Code `claude-opus-5`, thinking `max`.

| Owner request | Verified disposition | Tracking |
|---|---|---|
| Display `none` when effort is undefined. | Implemented by `6e9b396` and included in installed 0.4.2. Stored absence remains distinct from a reported effort named `none`. | YMP-132, done |
| Present all list data outside popups as tables, inspired by k9s. | A plan and incomplete saved patch exist. The table conversion is not implemented or installed. | YMP-133, new |
| Do not show navigation in the sidebar. | `6fd4401` removes NAVIGATE and sidebar focus; pages remain accessible through commands and the palette. This commit is outside main and the installed release. | YMP-134, in progress |
| Detailed mode must stop collapsing agent reasoning. | `5c3dda8` expands stored agent messages, including plans, reviews and execution reports. Streaming previews remain bounded. This commit is outside main and the installed release. | YMP-134, in progress |
| Improve the poorly formatted slash panel and popups; consult the earlier ymp. | `56c9d05` adds padding and aligned columns; `5e14f6e` corrects the surrounding margin at 80x24. These commits are outside main and the installed release. | YMP-134, in progress |

The owner also explicitly said to continue and that these changes could be made together or
as additions. They are authorized work; a further design approval is not pending.

"Detailed" applies to agent messages available in the shared conversation. It does not introduce
collection of private provider reasoning. The branch also retains one-line runtime notices;
Inspect continues to show their full text.

## Branch and verification status

The four pending commits are on `feat/ymp133-tables` in `/private/tmp/ymp132-agent-attribution`.
The worktree was clean at `5e14f6e`. Its base `5899589` predates the final 0.4.2 attribution
cache fixes. Integration must preserve those fixes and recheck the combined behavior.

The author's report and final workspace logs are retained under
`evidence/ui-followups-134/`. They report successful formatting, strict workspace Clippy and
462 passing Rust tests, with two ignored tests, on the author's branch. Negative controls and
terminal checks are described in the report. This audit inspected source diffs and existing
evidence; it did not independently rerun those checks or accept the changes for installation.

Outstanding acceptance work includes independent review, integration with current main,
combined checks, and terminal checks for Prompt, Confirm and the narrow theme chooser, which
the author explicitly did not recheck visually. The implementation report retains its original
YMP-133 title as historical evidence; YMP-134 now owns these four commits. Its suggestion to
seek approval for the Memory search key is superseded by the maintained table plan.

## Documentation corrections

- The table plan incorrectly said to retain NAVIGATE. The later owner message requires its removal.
- Sorting, filtering and moving page details into Inspect were described as owner decisions;
  they are implementation choices in the bounded table plan.
- The plan said Memory search could move to `s`, but its final section still asked for approval.
  The duplicate approval requirement has been removed.
- YMP-134 separates already-authored interface fixes from the unfinished table conversion.

## Other conversations checked

The other seven histories were `abc24a9b`, `4618183f`, `ffe18921`, `6864cb3c`, `dd1505f4`,
`b0b4b8d1` and `16df5a57`. Recent delegated UI/review threads contained maintainer instructions;
the two earlier owner-led research threads contained the already-established intent, effort,
MCP, workspace, supported-platform and research discussions. They did not supply additional
recent UI requirements beyond the source conversation above. The approved intent and the
owner's separately modified request document were left unchanged.

# TUI basis: ymp2

Owner decision, 2026-09-16: use the TUI of `ymp2` as the visual and interaction
basis for the new `ymp` interface. W1-0015 owns its adaptation to the working W1
session. This reference makes the starting design concrete while allowing it to
evolve with the approved model.

## Reference and provenance

The reference is repository `ymp2` at commit
`aa6d7225687bdffc71549687f925dabb6a519bc7`. The following material was inspected:

- `ymp-docs/guides/interface.md`: conversation layout, sidebar, composer,
  keyboard focus, command palette, tables and detail overlays;
- `ymp-docs/guides/ui-language.md`: concise English labels, attribution and
  explicit unknown values;
- `ymp-docs/guides/images/ember.png`: historical screenshot of the conversation
  shell, navigation/sidebar, composer and status row.

This document records the relevant observations so implementation does not depend
on a sibling checkout or a machine-specific path. The screenshot is a visual
reference from the earlier product, not evidence that the new interface exists.
No historical executable, provider session or application data was used for this
inspection.

## Layout and interaction basis

| Surface | Basis to carry into ymp |
| --- | --- |
| Main workspace | Conversation occupies the main column. User requests and final answers have greater visual weight than compact coordination activity. |
| Header | A compact project/session summary provides orientation and recorded status. |
| Right sidebar | Navigation and compact session, team, work and resource summaries remain visible beside the conversation when space permits. |
| Composer | Input stays at the bottom, supports multiline text and slash-command completion, and remains usable in short or narrow terminals. |
| Status row | Show the region that owns keyboard focus, relevant shortcuts and actionable status. |
| Command palette | Make supported commands discoverable without memorizing their spelling. |
| Inspection pages | Use aligned tables, right-aligned numbers, filtering and sorting. Open the selected record in a scrollable detail overlay. |
| Conversation details | Expand compact activity to its attributed full content. Preserve source text, code spacing and the distinction between reported output and verified results. |
| Scrolling | Scrolling back suspends automatic following; new output does not move the reader's position. Returning to the end resumes following. |
| Appearance | Retain the restrained separators, consistent typography and palette-based styling of the reference. State and selection remain understandable without color. |

Use the reference's keyboard conventions where the corresponding action exists:

| Key | Interaction |
| --- | --- |
| `Enter` | Submit composer input or activate the selected item. |
| `Ctrl+J` | Insert a newline in the composer. |
| `Tab` | Complete a command or move between interactive regions. |
| `Ctrl+P` | Open the command palette. |
| `Ctrl+B` | Toggle the sidebar. |
| `Esc` | Dismiss the topmost overlay or leave the current subordinate interaction. |
| `PageUp` / `PageDown` | Scroll the current conversation or list. |
| `Home` / `End` | Navigate to the beginning or end of the current view. |

Exact breakpoints, column widths, theme inventory and additional shortcuts remain
implementation choices. Preserve usable input, readable clipping and accessible
details as the terminal shrinks. Opening inspection views must not start work.

## Adaptation to the approved model

The current [domain model](self-organizing-team-domain-model.md), including its
names and authority rules, governs the data and actions behind these surfaces.
W1-0015 uses Application operations and deterministic projections; domain state,
acceptance, resource accounting and recovery remain in their owning components.
Agent identity, provider and model stay distinct even when model and effort are
prominent in a message heading. Historical labels cannot redefine those entities.

The W1 surfaces expose goal/constraints, pool, criteria, plan, assignments and
commitments, resources, results, checks, acceptance and the report. Later waves
extend the same interface with their actual delivered behavior. The reference's
old page inventory, configuration, storage paths, provider APIs and recovery
mechanics do not establish new product requirements or implementation contracts.
The new implementation follows the repository's greenfield rules.

## W1 acceptance evidence

W1-0015 must demonstrate the recognizable conversation/sidebar/composer layout,
keyboard navigation and detail inspection in a real terminal journey over the W1
runtime. Include a narrow-terminal case and show that reading older activity is
stable while output arrives. Record intentional adaptations with their reason.
The same journey must meet the task's existing start, interrupt, recovery,
accounting and report requirements. Documentation or visual similarity alone
does not deliver the task.

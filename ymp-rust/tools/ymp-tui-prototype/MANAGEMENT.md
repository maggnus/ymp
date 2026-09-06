# Entity management completion plan

**Closed by owner request.** Development of this subproject is stopped. The source and artifacts
are retained as an archived prototype, not an accepted product direction. Do not resume this work
from old goals, schedules or plans without a new explicit owner instruction.

Active owner objective: complete management on the existing screens, preserve state, remove
redundant instructions/counters, use K9s table references and OpenCode only as a right-panel reference.
The independent prototype remains a simulation; real provider execution is a separate boundary.

Acceptance, to be checked against executable behavior before completion:

- [ ] Providers: empty configured list; separate supported-type catalog; add/edit/enable/disable/
  remove connections; reject invalid/duplicate names; preserve existing agents' frozen profiles.
- [ ] Agents: explicit assignment and permitted route for creation; individual stop, replacement
  attempt, terminal archive/restore; enforce capacity; every creation has a recorded initiator.
- [ ] Tasks: create/select/edit proposal, run/stop/retry, delete unused drafts, archive/restore
  completed work; preserve all dependent evidence and never reuse identifiers.
- [ ] Board/checks/candidates: attributed immutable history, relation navigation and export;
  no destructive operation that silently invalidates evidence. Tool processes can be stopped;
  their records remain. Explicit cleanup removes only owned exported files.
- [ ] Persistence: isolated prototype state, atomic writes, writer exclusion, schema validation,
  UI draft restoration, clean restart and truthful recovery of interrupted simulations.
- [ ] Interaction: entity action menus and forms; unavailable actions explain their constraint;
  consequential removals/stop operations show the exact effect before execution.
- [ ] Presentation: remove persistent `Esc back`, row-range prose and redundant key instruction
  strips; retain necessary errors, state and explicit action labels. Hidden filter, compact input,
  the established palette and conversation layout, useful task sidebar, expandable tool output. No invented metrics.
- [ ] End-to-end: create provider and task, add/stop/replace agent, complete delivery, archive,
  restart, restore history, delete safe draft/connection/export; check both allowed and rejected
  operations through the actual terminal UI, and inspect 80×24 / 120×40 / wide images.
- [ ] Install the verified normal binary at `/Users/maggnus/.local/bin/ymp` and retain its source
  digest, tests, screenshots and final acceptance audit.

Visual references inspected:

- [K9s pods](https://k9scli.io/assets/screens/pods.png): aligned rows, restrained status color,
  clear selection, one contextual title rather than duplicated row counters.
- [K9s logs](https://k9scli.io/assets/screens/logs.png): dedicated content view and contextual actions.
- [K9s XRay](https://k9scli.io/assets/screens/xray.png): visible resource relationships.
- Owner-supplied OpenCode screenshots: reference for the right-hand context panel only.
  The owner explicitly rejected copying its colors or left-hand presentation.

Slash navigation and contextual menus expose operations; the owner rejected permanent `+ Add`
and `Actions` toolbar controls. Help carries keyboard bindings. Routine instructions do not occupy the main data view.
Kubernetes-specific fields, large logos, LSP statistics and model costs are not copied into ymp.

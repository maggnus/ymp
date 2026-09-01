---
id: W1-COR-03e.5
kind: subtask
wave: W1
card: W1-COR-03
parent: W1-COR-03e
state: ready
risk: significant
maturity: DESIGN
relation: required
depends_on: [W0-UX-01c]
blocks: [W1-COR-03e.2]
created_at: 2026-09-01T22:12:00+08:00
updated_at: 2026-09-01T22:12:00+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 0
escalation_decision:
---

# W1-COR-03e.5 — Chat-first TUI contract defines slash commands and modal windows

## Outcome

The authoritative TUI design sources define a new production-foundation POC surface in which the
chat transcript is always primary, `/` opens a searchable command palette, and contextual or
consequential actions use typed modal windows, while preserving the accepted visual character and
removing the old dashboard-first content hierarchy.

## Scope

### In

- Update `ymp-docs/design/ymp_chat_tui.dc.html` as the exact screen/state/fixture handoff and
  `ymp-docs/VISUAL_CONCEPT.md` as its composition rationale; regenerate
  `ymp-docs/design/ymp_chat_tui.pdf` from the accepted HTML.
- Define four exact state families at 80×24, 120×40 and 180×50: resting chat, filtered slash-command
  palette, informational/error popup, and consent/irreversible confirmation popup.
- Keep a thin context header, attributed heterogeneous transcript, always-ready input and one-line
  status. The default screen contains no dashboard table, permanent hotkey wall or numbered view.
- Derive the minimal POC slash-command catalogue from product surfaces that exist or are already
  contracted. Each entry declares its typed destination, availability and whether it opens a page,
  changes chat context or opens a modal; unavailable future commands are visibly disabled.
- Specify focus, keyboard and return rules: `/` opens and filters, selection never executes while
  browsing, `Enter` chooses the highlighted command, and `Esc` closes only the topmost palette/modal
  before returning to chat input.
- Specify modal classes separately: information/error, consent, and irreversible confirmation.
  Consequence text, permitted keys and typed confirmation belong to the modal; untrusted transcript
  text can never create an action.
- Reproducible PDF path on the accepted macOS design host:
  `Google Chrome 151.0.7922.170` at
  `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`, invoked from the repository root as
  `/Applications/Google\ Chrome.app/Contents/MacOS/Google\ Chrome --headless=new --disable-gpu
  --disable-background-networking --no-pdf-header-footer
  --print-to-pdf="$PWD/ymp-docs/design/ymp_chat_tui.pdf"
  "file://$PWD/ymp-docs/design/ymp_chat_tui.dc.html"`. The HTML may use only inline CSS/graphics and
  the local stack `ui-monospace, Menlo, Monaco, Consolas, monospace`; remote fonts, images, scripts
  and other network resources are forbidden. A different Chrome build is not accepted evidence
  until the same page count and page renders are compared against the accepted fixture.
- Exclusive write zone: the three authoritative visual-design files above and external temporary
  render artefacts only.

### Out

- Ratatui implementation, Application/runtime/protocol changes, new commands or product authority,
  complete observatory tables, web/mobile layouts, animation, theming variants and design-system
  replacement. Existing colour, typography and general visual character remain unless a concrete
  legibility defect requires a bounded correction.

## Acceptance

- [ ] The normal 120×40 resting state is unmistakably a chat product: transcript and input dominate;
      the old dashboard hierarchy, large resource table and persistent hotkey wall are absent.
- [ ] The slash palette has exact empty, filtered, selected, disabled and no-match states; every
      enabled command maps to a named current/contracted typed surface, and filtering or selection
      alone has no side effect.
- [ ] Information/error, consent and irreversible popups have distinct visual and keyboard
      contracts. `Esc` and confirmation behavior are explicit, and consequence text appears adjacent
      to the action that accepts it.
- [ ] Participant messages remain attributed, audience-scoped and visibly untrusted; no message,
      suggestion, link-like text or terminal escape sequence is rendered as an executable control.
- [ ] 80×24 shows an honest size-conscious composition, 120×40 is the primary implementation
      contract, and 180×50 adds useful transcript depth rather than more permanent chrome.
- [ ] HTML carries the exact fixtures and correspondence; PDF is regenerated and visually checked
      page by page with the recorded Chrome command; the HTML has zero network resources and
      `VISUAL_CONCEPT.md` matches it. Link/path checks and `git diff --check` pass.
- [ ] A negative fixture that restores the dashboard-first default, lets palette selection execute,
      or uses the same popup for consent and irreversible actions fails the design-state check.

## Current state

The current concept is already transcript-first, but its exact fixture content still carries the
older observatory/page emphasis. The owner retained the general visual idea and replaced the POC
focus with chat, slash commands and popup windows before new TUI implementation began. The exact
offline Chrome-to-PDF path is now proven on the current design host.

## Next action

Run a Significant design-contract check, then assign one Sol high designer-builder to update and
render the three authoritative sources.

## Guardrails

- Design only surfaces real or explicitly unavailable authority; it never invents a command that
  mutates Application state.
- A popup is not a substitute for a data page, and a data page is not a substitute for consent.
- Exact HTML remains the correspondence authority; the PDF is a review rendering, not a source.

## Findings

- Owner direction on 01/09 replaced the old TUI content before W1-COR-03e.2 wrote code; the retained
  clean implementation workspace was stopped without product changes.
- Contract preflight found no project-owned PDF command and a broken Homebrew Chromium launcher;
  the installed Chrome 151 path produced a 19-page Letter PDF offline, so that exact route and its
  local-only resources are now part of acceptance.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

## Closure

Filled when the subtask is accepted.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.

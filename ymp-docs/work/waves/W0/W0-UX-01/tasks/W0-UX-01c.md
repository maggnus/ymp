---
id: W0-UX-01c
kind: task
wave: W0
card: W0-UX-01
state: accepted
risk: critical
maturity: DESIGN
relation: required
depends_on: [W0-UX-01b]
blocks: [W1-APP-02e, W1-COR-03e]
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-08-12T13:54:50+08:00
started_at: 2026-08-12T13:30:46+08:00
accepted_at: 2026-08-12T13:54:50+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/35cc659981f73700296c9ed37b378e59eabff4a1
closure_commit: https://github.com/maggnus/ymp/commit/4670fa0ad914e73d5dc3a5994d694f82a5ebe449
evidence: ["[35cc659](https://github.com/maggnus/ymp/commit/35cc659981f73700296c9ed37b378e59eabff4a1)", "[4670fa0](https://github.com/maggnus/ymp/commit/4670fa0ad914e73d5dc3a5994d694f82a5ebe449)"]
duration_minutes: 24
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W0-UX-01c — Independent review freezes an implementable screen contract

## Outcome

An independent review accepts a versioned screen contract whose domain semantics match the project,
whose required states are complete and traceable, and whose interactions are implementable with
Rust, Ratatui, and Crossterm at the declared terminal sizes.

## Scope

### In

- Semantic review against the project documents and the sole HTML artifact.
- Coverage review of the POC-1 and POC-2 journeys, material negative states, terminal and
  accessibility constraints, and requirement-to-mockup handoff index.
- Feasibility review against terminal cells, Ratatui rendering and deterministic tests, Crossterm
  input events, high event volume, and one-executable packaging.
- A frozen identifier and revision for every accepted screen and state contract consumed by `W1`.

### Out

- Production implementation, subjective redesign without a falsifiable defect, and acceptance of
  any layout that changes kernel semantics or requires a web or service runtime.

## Acceptance

- [x] A non-author reviewer returns `ACCEPT` only after every POC-1 and POC-2 requirement is traced
  to a design artifact. No open assumption affecting trust, authorization, verification, or a
  terminal outcome may remain; lower-risk assumptions require an owner and implementation effect.
- [x] The reviewer confirms that the screens do not assign work, rank agents or candidates, infer
  leadership, conflate trust planes, overstate causation, hide assurance weakness, or confuse any
  root terminal outcome.
- [x] The 80 × 24, 120 × 40, and wide variants are feasible with terminal cells, keyboard input,
  bounded text, and deterministic Ratatui state tests; no required interaction depends on hover,
  animation, pixel geometry, a browser, or a network service.
- [x] A negative-control design that labels message delivery as causal reasoning or process
  completion as acceptance is returned rather than accepted.
- [x] Negative controls for a lost MCP reply and quiescence with unread inert messages keep
  authorization disabled until the corresponding state is resolved.

## Current state

Accepted. Independent review traced all 172 requirements, every POC-1 and POC-2 scenario, all 71
material states, and every declared template-size combination to the exact artifact at
[35cc659](https://github.com/maggnus/ymp/commit/35cc659981f73700296c9ed37b378e59eabff4a1).
An external terminal-cell and semantic model rejected all required negative interpretations. The
screen implementation tasks in `W1` may now use this immutable contract.

## Next action

Implement the single-participant and multi-participant screens against the frozen projection,
state, command, keyboard, accessibility, and terminal-size contract, and verify their Ratatui
buffers with `TestBackend`.

## Guardrails

- The reviewer attacks correctness, coverage, accessibility, and feasibility rather than replacing
  the designer's layout preferences with another unvalidated preference.
- Any required semantic change returns to the project documents and receives its own decision; it
  is not smuggled into the screen contract.

## Findings

None. The accepted model records `PauseRun` and `ResumeRun` as interface commands whose domain
effects correspond to `PauseAdmission` and `ResumeAdmission`; implementation follows the command
registry rather than inferring new protocol semantics.

## Closure

### Accepted outcome

The exact HTML revision is frozen as an implementable terminal screen contract. It preserves trust
planes and terminal outcomes, forbids central allocation and unsupported causal claims, remains
keyboard-first and color-independent, and fits the declared terminal sizes through bounded wrapping
and scrolling rules.

### Residuals

Rust, Ratatui `TestBackend`, and Chromium execution were outside this design review. The W1 screen
implementation tasks own executable buffer verification.

### Evidence

- [Frozen HTML contract](https://github.com/maggnus/ymp/commit/35cc659981f73700296c9ed37b378e59eabff4a1).
- [Integrated plan and artifact](https://github.com/maggnus/ymp/commit/4670fa0ad914e73d5dc3a5994d694f82a5ebe449).

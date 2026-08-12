---
id: W1-APP-02a.2
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02a
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-12T16:35:41+08:00
updated_at: 2026-08-12T16:35:41+08:00
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
---

# W1-APP-02a.2 — Foreground executable exposes no public headless mode

## Outcome

The installed `ymp` executable exposes the foreground TUI as its only public product mode, while
process-internal child operations remain explicitly nested under `ymp internal`.

## Scope

### In

- Public command parsing and help output in `ymp-cli`.
- Removal or private nesting of `demo`, `inspect`, and `probe` behavior.
- CLI tests that distinguish public product commands from private child operations.

### Out

- Application recovery, candidate isolation, TUI layout, runtime-driver behavior, and new network
  or background interfaces.

## Acceptance

- [ ] `ymp --help` exposes no public `demo`, `inspect`, `probe`, daemon, socket, or headless command.
- [ ] Starting `ymp` without a private child command enters the foreground TUI path.
- [ ] Required process-internal operations remain reachable only through the explicit `internal`
  namespace, and existing child-mode tests continue to pass.

## Current state

The defect is confirmed in public help output. Dispatch is deferred only until the active
W1-APP-02b writer releases the overlapping `ymp-cli` file.

## Next action

Re-baseline after W1-APP-02b integration, then narrow the public parser and add help-output checks.

## Guardrails

- Limit changes to `ymp-cli` and package-local CLI tests.
- Do not add a daemon, listener, public socket, or replacement headless interface.

## Findings

- Public help currently advertises `demo`, `inspect`, and `probe`.

## Closure

Filled when the subtask is accepted.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.

---
id: W1-APP-02a.2
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02a
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-12T16:35:41+08:00
updated_at: 2026-08-12T19:46:07+08:00
started_at: 2026-08-12T19:03:00+08:00
accepted_at: 2026-08-12T19:46:07+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/ecad181e7850df98643fa327dfb0a135f778fe1c
closure_commit: https://github.com/maggnus/ymp/commit/1026143499c201a57e670f4b8b89c099645365fc
evidence: ["[ecad181](https://github.com/maggnus/ymp/commit/ecad181e7850df98643fa327dfb0a135f778fe1c)", "[1026143](https://github.com/maggnus/ymp/commit/1026143499c201a57e670f4b8b89c099645365fc)"]
duration_minutes: 43
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

- [x] `ymp --help` exposes no public `demo`, `inspect`, `probe`, daemon, socket, or headless command.
- [x] Starting `ymp` without a private child command enters the foreground TUI path.
- [x] Required process-internal operations remain reachable only through the explicit `internal`
  namespace, and existing child-mode tests continue to pass.

## Current state

Accepted. The public executable starts the foreground TUI, exposes no public headless operation,
and keeps process-internal operations under `ymp internal`.

## Next action

Continue the parent closure process and the remaining managed-profile tasks.

## Guardrails

- Limit changes to `ymp-cli` and package-local CLI tests.
- Do not add a daemon, listener, public socket, or replacement headless interface.

## Findings

- Public help currently advertises `demo`, `inspect`, and `probe`.

## Closure

### Accepted outcome

The reviewed candidate was integrated byte-equivalently. An external pseudoterminal scenario
observed alternate-screen entry and safe exit, while a non-TUI imitation was rejected.

### Residuals

None.

### Evidence

- [Reviewed candidate ecad181](https://github.com/maggnus/ymp/commit/ecad181e7850df98643fa327dfb0a135f778fe1c)
- [Integrated revision 1026143](https://github.com/maggnus/ymp/commit/1026143499c201a57e670f4b8b89c099645365fc)

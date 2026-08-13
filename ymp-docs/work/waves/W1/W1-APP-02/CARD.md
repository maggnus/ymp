---
id: W1-APP-02
kind: card
wave: W1
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-EXP-01]
blocks: [W1-COR-03]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T16:19:52+08:00
started_at: 2026-08-12T16:14:11+08:00
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
---

# W1-APP-02 — Foreground TUI verifies one exact participant result

## Outcome

One installed Rust executable launches a managed Codex or Claude Code participant from the
foreground TUI, constructs an immutable candidate, verifies its exact digest, and exports the
resulting evidence.

## Invariants

- The TUI and trusted application core share one foreground process and one authoritative writer.
- Commands and committed events are independent of storage, notification, runtime, and tool-call
  bindings.
- Attempt workspaces are private and candidates are immutable.
- The verifier observes exact digests outside the producing attempt.
- Runtime-specific lifecycle and model-route facts remain behind explicit profiles.

## Scope

This card owns `POC-1`: the smallest foreground application, local event and object storage,
attempt workspaces, candidate construction, verifier invocation, Codex and Claude Code drivers,
stdio MCP projection, minimal ratatui views, cancellation, and evidence export.

It excludes local crash continuation, SQLite, strict containment, OpenCode, Nemotron, a public
headless mode, daemon, operator socket, HTTP listener, WebSocket endpoint, and remote clients.

## Aggregate acceptance

All eight required tasks are accepted. From a clean disposable environment, the TUI detects the two
approved runtime profiles, starts either profile itself, completes a single-participant candidate
and verifier path, reproduces the evidence from clean inputs, and reports malformed storage,
runtime, MCP, or verifier state as a typed failure rather than acceptance.

## Tasks

- [W1-APP-02a](tasks/W1-APP-02a/TASK.md) — required
- [W1-APP-02b](tasks/W1-APP-02b.md) — required
- [W1-APP-02c](tasks/W1-APP-02c.md) — required
- [W1-APP-02d](tasks/W1-APP-02d.md) — required
- [W1-APP-02e](tasks/W1-APP-02e/TASK.md) — required
- [W1-APP-02f](tasks/W1-APP-02f.md) — required
- [W1-APP-02g](tasks/W1-APP-02g.md) — required
- [W1-APP-02h](tasks/W1-APP-02h.md) — required
- [W1-APP-02i](tasks/W1-APP-02i.md) — follow_up
- [W1-APP-02j](tasks/W1-APP-02j.md) — follow_up
- [W1-APP-02k](tasks/W1-APP-02k.md) — required
- [W1-APP-02l](tasks/W1-APP-02l.md) — follow_up
- [W1-APP-02m](tasks/W1-APP-02m.md) — required
- [W1-APP-02n](tasks/W1-APP-02n.md) — required
- [W1-APP-02o](tasks/W1-APP-02o.md) — required
- [W1-APP-02p](tasks/W1-APP-02p.md) — required
- [W1-APP-02q](tasks/W1-APP-02q.md) — required
- [W1-APP-02r](tasks/W1-APP-02r.md) — follow_up
- [W1-APP-02s](tasks/W1-APP-02s.md) — follow_up
- [W1-APP-02t](tasks/W1-APP-02t.md) — required

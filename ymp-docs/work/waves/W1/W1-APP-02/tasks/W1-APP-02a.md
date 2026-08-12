---
id: W1-APP-02a
kind: task
wave: W1
card: W1-APP-02
state: review
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-EXP-01c, W1-EXP-01d]
blocks: [W1-APP-02b, W1-APP-02c, W1-APP-02d]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T16:17:00+08:00
started_at: 2026-08-12T16:17:00+08:00
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

# W1-APP-02a — Foreground core commits one recoverable event history

## Outcome

A Rust workspace builds one `ymp` executable whose foreground application owns one authoritative
writer, accepts typed commands, commits ordered events before acknowledgement, and lets lagging
views recover from an event cursor independently of in-memory notifications.

## Scope

### In

- Domain command and event types, application interfaces, one-writer in-memory state, command
  idempotency, event cursors, and projection updates.
- Bounded `events.jsonl`, atomic `run.json`, content-addressed object writes, predecessor digests,
  and a single-writer data-root lock.
- In-process command delivery and lightweight notification channels for the TUI and fake runtime.
- Private child-mode dispatch points compiled into the same executable.

### Out

- Runtime-specific drivers, workspace and candidate semantics, verifier logic, full TUI views,
  SQLite, transparent crash continuation, daemon mode, and any network operator interface.

## Acceptance

- [x] A deterministic integration test sends typed commands, observes monotonic committed events,
  restarts a lagging projection from its cursor, and obtains the same authoritative state.
- [x] Repeating one command identifier returns its recorded result without a second state change or
  resource charge during the live controller interval.
- [x] A gap, duplicate sequence, predecessor-digest mismatch, incomplete journal tail, or object
  reference to unavailable bytes ends the run as `infrastructure_error`.
- [x] A notification receiver that deliberately falls behind rereads committed events; dropped or
  coalesced notifications do not become lost authoritative state.
- [x] A second foreground process using the same data root is rejected without replacing or
  terminating the current writer.

## Current state

Independent review is active. The production workspace builds one executable with typed events, recovered idempotency, atomic
metadata, canonical objects, cursor replay, and one writer. Journal records and total bytes have
hard ceilings with terminal capacity, and exhaustion persists `infrastructure_error`. Notifications
are lossy by design; lagging readers recover from a durable cursor. Version-1 schema and fail-closed
migration rules are frozen in `ymp-rust/SCHEMA.md`; non-author review remains outstanding.

## Next action

Submit the implementation and executable recovery evidence for non-author review.

## Guardrails

- Event notification is an optimization; the committed event log is authoritative.
- A controller crash terminates a POC run and never resumes ambiguous authority.
- No public headless API, socket, HTTP endpoint, database, or dynamic driver ABI is added.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

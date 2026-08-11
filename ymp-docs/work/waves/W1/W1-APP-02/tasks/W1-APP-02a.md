---
id: W1-APP-02a
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-EXP-01c, W1-EXP-01d]
blocks: [W1-APP-02b, W1-APP-02c, W1-APP-02d]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-10T19:34:25+08:00
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

- [ ] A deterministic integration test sends typed commands, observes monotonic committed events,
  restarts a lagging projection from its cursor, and obtains the same authoritative state.
- [ ] Repeating one command identifier returns its recorded result without a second state change or
  resource charge during the live controller interval.
- [ ] A gap, duplicate sequence, predecessor-digest mismatch, incomplete journal tail, or object
  reference to unavailable bytes ends the run as `infrastructure_error`.
- [ ] A notification receiver that deliberately falls behind rereads committed events; dropped or
  coalesced notifications do not become lost authoritative state.
- [ ] A second foreground process using the same data root is rejected without replacing or
  terminating the current writer.

## Current state

No Rust workspace or executable exists. The task becomes dispatchable only after `W1-EXP-01c`,
`W1-EXP-01d`, plan review, and repository gate `G1` are complete.

## Next action

Create the minimal Rust workspace and freeze the domain command and event interfaces first.

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

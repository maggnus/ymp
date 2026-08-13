---
id: W1-APP-02r
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: routine
maturity: BUILD
relation: follow_up
depends_on: [W1-APP-02q]
blocks: []
created_at: 2026-08-13T21:11:27+08:00
updated_at: 2026-08-13T21:11:27+08:00
started_at: 2026-08-13T20:52:00+08:00
accepted_at: 2026-08-13T21:32:25+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/8821e4a3dc2ad14fe8e383cb3c986c1a654b75dc
closure_commit: https://github.com/maggnus/ymp/commit/4ab6a80cb5ffce1d0ad2786fa1ef92fffc72dbd6
evidence: [`8821e4a`](https://github.com/maggnus/ymp/commit/8821e4a3dc2ad14fe8e383cb3c986c1a654b75dc)
duration_minutes: 23
blocker:
pause_reason:
return_trigger: W1-APP-02s removes the fake runtime from the terminal profile list, after which the binary no longer links it
deliberate_partial: true
---

# W1-APP-02r — An unreadable marker is not read as an absent holder

## Outcome

A holder lookup distinguishes "no one holds this" from "this could not be read", so a permission
failure never passes as an empty answer, and the test kit is not a dependency of the shipped binary.

## Scope

### In

- The holder lookup that reads the process utility's exit code and output.
- The dependency entry that keeps the fake runtime in the command crate.

### Out

- The admission gate accepted in W1-APP-02q.

## Acceptance

- [x] With the marker present but unreadable by the account, the lookup reports an error rather than
      an empty holder list; the negative half denies access to the marker and shows today's build
      reporting no holders.
- [ ] The fake runtime is a development dependency of the command crate, and the shipped binary does
      not link it.

## Current state

Accepted by a second look and integrated. An empty holder list is accepted only after the marker has
been opened successfully, and the reviewer confirmed on this machine that the process utility reports
the same exit and empty output whether the marker is unheld or unreadable. The command crate no
longer reaches the test kit.

## Next action

Confirm the marker exists and is readable before accepting an empty answer.

## Guardrails

- An error and an empty result are different answers; a check that merges them reports health it has
  not established.

## Findings

- The lifecycle refusal tests added by W1-APP-02q fail under machine load and pass in isolation,
  measured on the integrated tree: a cancelled run reported a supervision failure instead of naming
  what could not be established. Either their timing assumption is hardened, or they join the named
  group that only the integration check runs on a quiet machine.
- The second look ran the lifecycle suites three times at different machine loads, including a
  twelve-thread load, and all three passed. The load explanation for the earlier red is therefore not
  confirmed; the failure remains unexplained and unreproduced, and the tests stay under watch.
- `minor`, additional work, continued as W1-APP-02s: the fake runtime still reaches the binary
  through the terminal crate, whose probe list constructs it.
- `minor`, additional work, unscheduled: the holder lookup was made publicly reachable, hidden from
  documentation, so that the test could call it, which widens the crate's surface.

## Closure

Filled when the task is accepted.

### Accepted outcome

A holder lookup distinguishes an unheld marker from one it could not read, so a permission failure is
an error rather than an empty answer.

### Residuals

The second acceptance item is not met on this card: the shipped binary still links the fake runtime
through the terminal crate, which lay outside the write zone. It is carried as W1-APP-02s with the
return trigger recorded in the front matter, because removing that profile also changes what the
operator sees and needs its own evidence.

### Evidence

- [`8821e4a`](https://github.com/maggnus/ymp/commit/8821e4a3dc2ad14fe8e383cb3c986c1a654b75dc) — reviewed candidate.
- [`4ab6a80`](https://github.com/maggnus/ymp/commit/4ab6a80cb5ffce1d0ad2786fa1ef92fffc72dbd6) — integration into the release branch.

---
id: W1-APP-02aa
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02k, W1-PRD-05g]
blocks: []
created_at: 2026-08-16T09:25:00+08:00
updated_at: 2026-08-16T11:45:00+08:00
started_at: 2026-08-16T09:30:00+08:00
accepted_at: 2026-08-16T11:45:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/4cd0bee26596152f30b1f4d1c0f50d57d6c29af5
closure_commit: https://github.com/maggnus/ymp/commit/a19c480742f44b13021aab9c47cb149635024d92
evidence: ["[4cd0bee](https://github.com/maggnus/ymp/commit/4cd0bee26596152f30b1f4d1c0f50d57d6c29af5)"]
duration_minutes: 135
blocker:
pause_reason:
return_trigger:
deliberate_partial: true
---

# W1-APP-02aa — Quitting kills a probe that outlives the 5 s bound

## Outcome

Quitting during a provider measurement never leaves the engine process alive, whatever the probe
duration: the worker is joined or the child killed even beyond the existing 5 s bound. The
acceptance test of W1-PRD-05g pinned 2 s because this case was out of its scope (W1-APP-02k
limit); this card closes that limit.

## Scope

### In

- The measurement worker path (`ymp-tui` Session): on shutdown, terminate the probe process tree
  when the join bound expires, using the existing private process-group mechanism; wait for the
  group termination before reporting managed shutdown.
- Acceptance test with a slow fake engine (> 5 s, e.g. 20 s) driven through the real quit path;
  probe by pid file that no process remains.

### Out

- Any change to what is measured or when; worker-pool redesign; runtime drivers.

## Acceptance

- [x] Quitting during a 20 s probe leaves no engine process after the shutdown sequence completes;
      the shutdown stays bounded (state what bound now holds and how it is enforced).
- [x] Negative half: on the pre-change tree the same 20 s scenario leaves the process alive
      (measured: «engine remains, exit took 5.7 s, stderr: did not end within 5000 ms» — the
      reviewer's live measurement from the W1-PRD-05g review).

## Current state

Accepted after one return round. Reviewer blocker (membership recomputed per read; a TERM-rejecting
orphan escaped the kill while Terminated was reported) was accepted and fixed at 4cd0bee:
membership is recorded from measurement start (id+start-time key, 100 ms observe), removed only on
OS disappearance, append-then-signal at shutdown. Lean re-review ACCEPT: the reviewer's own
falsifier (orphan inside the group; observed→setsid→orphaned TERM-ignorer) both terminated;
fault-injected KILL→CONT yields honest LeftRunning; false Terminated unreachable while a process
is in the record. Merged; ymp-tui 172 tests ok; workspace check green.

## Next action

Residuals below stay with their triggers.

## Guardrails

- Descendant cleanup rules of W1-APP-02k apply in full; no new process-spawning surface.

## Findings

None blocking. Review findings recorded as residuals.

## Closure

### Accepted outcome

Quitting during a probe of any duration kills the observed engine tree within the 7 s bounded
sequence (5 s join + 0.5 s TERM + 0.5 s KILL + 1 s thread return), each part a separate deadline;
managed attempts remain excluded by their private process group. Verified by the builder's negative
half (live surviving pid pre-change), a post-fix mutation reproducing the defect, and the
reviewer's independent falsifiers.

### Residuals

1. (minor, additional-work) A process never seen by any read (created and detached inside one
   observe interval: 100 ms during measurement, 20 ms at shutdown) escapes the record. W1-APP-02k
   closes this with a launch marker; the measurement path installs none. Return trigger: any
   report of a surviving process traced to the measurement path, or the marker work item.
2. (minor, independent-defect) The record covers any descendant in the interface's group during
   the measurement, not only engines. Return trigger: a spurious kill of a non-engine descendant
   observed in practice.

### Evidence

- [4cd0bee](https://github.com/maggnus/ymp/commit/4cd0bee26596152f30b1f4d1c0f50d57d6c29af5) — candidate head, range 097a758..4cd0bee
- [a19c480](https://github.com/maggnus/ymp/commit/a19c480742f44b13021aab9c47cb149635024d92) — integration merge — ymp-tui 172 ok / 0 failed; `cargo check --workspace --all-targets` exit 0 (CTO on the merged tree)

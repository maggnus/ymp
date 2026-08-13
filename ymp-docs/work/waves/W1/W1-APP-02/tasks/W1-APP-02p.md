---
id: W1-APP-02p
kind: task
wave: W1
card: W1-APP-02
state: rework
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02k]
blocks: []
created_at: 2026-08-13T14:57:22+08:00
updated_at: 2026-08-13T14:57:22+08:00
started_at: 2026-08-13T15:10:00+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/296b418
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02p — Every program in the launch chain is verified, not only the pinned runtime

## Outcome

The bytes that execute in a managed run are admitted bytes at every step of the chain, not only for
the pinned runtime executable. What the supervisor starts, and whatever that program starts on its
behalf, is verified by digest before it runs.

## Scope

### In

- The launch chain of a managed run, including the shell and environment helpers the supervisor now
  invokes.
- The digest verification already applied to the pinned runtime, extended to cover them.

### Out

- Containment of hostile code, which the POC does not claim.

## Acceptance

- [ ] Every program the supervisor executes is verified by digest before execution; the negative half
      replaces one of them after admission and the run refuses with a captured non-zero exit.
- [ ] The property W1-APP-02c established — executed bytes equal admitted bytes — holds for the whole
      chain, proved by the same substitution test extended to each program.

## Current state

Returned by independent review after one round. Digest verification now covers the shell, the
environment helper, the pinned runtime, the coordination bridge and the version-control program, and
the earlier card's termination properties still pass. The review found one root cause the card did
not close: a program is resolved by name through the environment search path, so a planted program
is admitted and executed normally — reproduced on the built product.

## Next action

Enumerate the programs the supervisor executes, then verify each by digest on the same path.

## Guardrails

- A helper in the chain is part of the trusted path; leaving it unverified reopens the substitution
  the earlier card closed.

## Findings

- `blocker`, defect in the contracted outcome. The command crate holds a second copy of baseline
  construction that invokes the version-control program by name through the search path, with no
  admission and no digest check; the reviewer executed a planted program three times on the built
  product.
- `major`, defect in the contracted outcome. The enumeration omits the platform keychain utility,
  which runs on every Claude launch and extracts credential material handed to the managed process,
  and the process-inspection utility used in place of the Linux process filesystem.
- `major`, defect in the contracted outcome. Admission records the digest of whatever the search
  path yields, so it is bound to nothing: three user-writable directories precede the system
  directory in this machine's path, and a process running as the same user — including an agent from
  an earlier run — can have a planted program admitted.
- `minor`, defect in the contracted outcome. The descriptor validation checks a digest at the path
  the driver named rather than correspondence to the expected program, while its comment claims the
  controller admits the chain independently of the driver.
- Established and not in question: the window between verification and image load is unreachable for
  an unprivileged user on this machine, because the system programs are protected by the platform's
  integrity mechanism and owned by the administrator.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.

---
id: W1-APP-02p
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02k]
blocks: []
created_at: 2026-08-13T14:57:22+08:00
updated_at: 2026-08-13T14:57:22+08:00
started_at: 2026-08-13T15:10:00+08:00
accepted_at: 2026-08-13T16:54:11+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/0461426f1256c0b003fe149c0c11218cf2f0459e
closure_commit: https://github.com/maggnus/ymp/commit/f6bebcf6b2564f6fb133c4545d7209770037f2cd
evidence: [`0461426`](https://github.com/maggnus/ymp/commit/0461426f1256c0b003fe149c0c11218cf2f0459e)
duration_minutes: 118
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

- [x] Every program the supervisor executes is verified by digest before execution; the negative half
      replaces one of them after admission and the run refuses with a captured non-zero exit.
- [x] The property W1-APP-02c established — executed bytes equal admitted bytes — holds for the whole
      chain, proved by the same substitution test extended to each program.

## Current state

Accepted and integrated. A program enters the chain by location rather than by name: its file and
every parent directory must belong to the administrator and be closed to other accounts, and where a
location carries no such guarantee the caller names the expected digest in advance. The reviewer
reproduced its own attack on the corrected revision — the planted program was refused and never
executed, while an unplanted run still succeeded.

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
- All four returned defects are closed and independently re-measured. The enumeration now covers the
  shell, the environment helper, the pinned runtime, the coordination bridge, the version-control
  program, the keychain utility that reads credential material, and the process inspection and
  signalling utilities; each was checked against the ownership rule on this machine.
- `minor`, independent product defect, continued as W1-APP-02q: a failure to admit an observation
  utility is swallowed, so on a differently configured machine a run would start and report a clean
  termination while it could not see its own descendants.
- `minor`, independent product defect, continued as W1-APP-02q: the ownership rule reads the file
  mode only, so a directory that grants write access through an access-control entry would pass.

## Closure

Filled when the task is accepted.

### Accepted outcome

Every program a managed run executes on its own behalf is admitted by an identity that a same-account
process cannot forge, and the substitution proof of the earlier card now covers the whole chain
rather than the pinned runtime alone.

### Residuals

None on this card. Both remaining weaknesses became W1-APP-02q, because a swallowed admission failure
reports health it has not established, which the acceptance rules forbid carrying as a limitation.

### Evidence

- [`0461426`](https://github.com/maggnus/ymp/commit/0461426f1256c0b003fe149c0c11218cf2f0459e) —
  reviewed correction.
- [`f6bebcf`](https://github.com/maggnus/ymp/commit/f6bebcf6b2564f6fb133c4545d7209770037f2cd) —
  integration into the release branch.

---
id: W1-APP-02q
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02p]
blocks: []
created_at: 2026-08-13T16:54:11+08:00
updated_at: 2026-08-13T16:54:11+08:00
started_at: 2026-08-13T18:53:00+08:00
accepted_at: 2026-08-13T21:11:53+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/16d53f7254a309e248c565d32dd8bdb1ee1fa08b
closure_commit: https://github.com/maggnus/ymp/commit/7cf9ed3bdf4642330d4256fc3c0632bd57fd8c92
evidence: [`16d53f7`](https://github.com/maggnus/ymp/commit/16d53f7254a309e248c565d32dd8bdb1ee1fa08b)
duration_minutes: 107
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02q — Admission failures refuse instead of degrading, and ownership is proved beyond file mode

## Outcome

A program that cannot be admitted stops the run instead of degrading it quietly, and the ownership
rule that admits a program cannot be satisfied by a location the running account can still write
through an access-control entry.

## Scope

### In

- The admission path for the process-inspection and signalling utilities, whose failure is currently
  swallowed.
- The ownership rule itself, which today reads the file mode only.
- The exclusion of the fake runtime from launch attestation.

### Out

- The identity rule accepted in W1-APP-02p, which stands; this card removes the ways around it.

## Acceptance

- [x] A run whose observation utility cannot be admitted refuses to start and says which program and
      why; the negative half places that utility in an unsuitable location and today's build reports
      a clean termination instead, with a captured exit.
- [x] The ownership rule rejects an ancestor directory that grants write access through an
      access-control entry while its mode looks safe; the negative half constructs exactly that
      directory outside the repository.
- [x] The fake runtime cannot be selected on a path that produces a candidate in the shipped product,
      proved by a check rather than by convention.

## Current state

Accepted and integrated. One gate now carries attestation and utility admission, invoked by the
controller and by the single place where the command line builds a driver, and the fake runtime no
longer exists on the shipped surface. The reviewer derived the launch sites from the code rather than
from the author's list and found three, each passing the gate or not starting a run at all.

## Next action

Turn each swallowed failure into a refusal, then extend the ownership proof to access-control
entries.

## Guardrails

- A safety check that cannot fail loudly is worse than no check, because its silence reads as
  health.

## Findings

- `blocker`, defect in the contracted outcome. An internal command assembles a candidate without the
  supervisor, and the reviewer produced one with the fake runtime at exit zero. The guarding check
  searches for a seam name and inspects only the attested start.
- `major`, defect in the contracted outcome. The refusal at start guards the attested start alone,
  while another internal command launches a driver directly.
- `minor`, independent product defect. The holder lookup reads the utility's exit code one as "no
  holders" whether or not it failed, and parses output regardless of the exit.
- `minor`, independent product defect. Twenty-eight call sites discard the termination helper's
  result, swallowing the refusal on the cancellation and timeout paths.
- `minor`, defect in the contracted outcome. The structural check excludes both definition files in
  full, which is how the first finding stayed invisible.
- Established and needing no work here: the access-control reader is incomplete rather than
  circular, because an entry high in the tree could replace the reader itself, while ownership and
  mode come from a system call.
- All five findings are closed and independently re-measured: the reviewer's own reproduction now
  exits with an invalid-value refusal and creates no data directory, while a real runtime still
  reaches a candidate, so the gate refuses selectively rather than universally.
- `minor`, independent product defect, continued as W1-APP-02r: the holder lookup still reads a
  permission failure as an empty holder list, because the process utility reports both alike.
- `minor`, additional work, continued as W1-APP-02r: the fake runtime remains an ordinary dependency
  of the command crate although no source file names it.

## Closure

Filled when the task is accepted.

### Accepted outcome

An unadmittable program stops the run naming the program and the reason, the ownership rule reads
access-control entries on every ancestor, and every path that starts a runtime passes one gate.

### Residuals

None. Both remaining weaknesses became W1-APP-02r rather than carried limitations, because a failure
read as an empty answer is the defect this card was opened to remove.

### Evidence

- [`16d53f7`](https://github.com/maggnus/ymp/commit/16d53f7254a309e248c565d32dd8bdb1ee1fa08b) —
  reviewed correction.
- [`7cf9ed3`](https://github.com/maggnus/ymp/commit/7cf9ed3bdf4642330d4256fc3c0632bd57fd8c92) — integration into the release branch.

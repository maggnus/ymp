# YMP-112 R1 scheduling correction

The independent R1 review of `92691607c2652c9fcc913f658e5971509c4dab74` found that Third's valid commitment to agent `one` stopped a parallel wave after Second had already been claimed by `one`. The filtered candidate list represented temporary occupancy, but the commitment lookup reported permanent unavailability. The run stopped before starting Second.

The scheduler now checks a current commitment against the wave's occupied agents before choosing an executor. It records `commitment_busy` and defers that ready task with its commitment intact. Already selected work starts and receives independent review. If later selection discovers a real error, the scheduler drains the claims already made before returning the error; no selected work is abandoned before invocation. An empty wave cannot spin indefinitely. Membership, native settings, reviewer reservation, atomic claim validation and recovery admission remain enforced.

The permanent public regression creates First, then independent Second and Third, with Third committed to `one`. It checks completion at both parallel widths two and one, the retained executor/settings, exactly three execution assignments, accepted task states and the explicit waiting record at width two. A separate injected allocation failure proves that Second is accepted and Third stays ready with its commitment when choosing later work fails.

| Check | Result | Evidence |
| --- | --- | --- |
| New public regression against unchanged R1 runtime | Failed with exit 101 and `commitment_unavailable` | [Before](r1-before.txt) |
| Same regression after correction, widths two and one | Passed | [After](r1-after.txt) |
| Reviewer’s external unchanged width-two repro | Passed; all three tasks accepted, three executions | [External repro](r1-external-repro.txt) |
| Complete board suite | 11 tests passed | [Board tests](r1-board-tests.txt) |
| Required formatting, clippy and workspace checks | Passed; 308 workspace tests; final verification 2026-09-12 19:08 UTC | [Formatting](r1-fmt.txt), [Clippy](r1-clippy.txt), [Workspace tests](r1-workspace-tests.txt) |

The exact external reproduction source remains `/tmp/ymp112-review-mpha21ll/tests/board.rs`, unchanged by this correction. Its command is the R1 review command with `PROBE_PARALLEL=2`. Test workload providers are scripted, with small/low settings. The final aggregate remains accepted/unconfirmed because only First has an objective contract; completion is not upgraded into aggregate confirmation.

The unrelated YMP-126 driver remains in its separate checkout. No UI, intent, task registry, main branch or live provider was changed here. R2 independent acceptance and delegated TUI acceptance remain outstanding.

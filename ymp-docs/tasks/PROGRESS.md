# Development task progress

Generated from the canonical JSON records in [`records/`](records/) by `python3 ymp-docs/tasks/manage.py progress --write`. Task write commands and `make tasks-progress` refresh it; do not edit statuses here.

Tasks: 45 (15 done, 2 in progress, 26 open, 2 paused or blocked, 0 rejected). Times are Hong Kong time (UTC+08:00). Last record change: 2026-09-28 23:40.

| Mark | Meaning |
| :---: | --- |
| [ ] | not started (`new` is unscheduled; `planned` is ready only when dependencies are done) |
| [x] | done |
| [~] | in progress |
| [!] | rejected |
| [=] | paused or blocked (`paused`, `owner_question`, or `planned` with unfinished dependencies) |

## Waves

| Wave | Done | In progress | Open | Paused or blocked | Rejected | Total | Done % |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| [W1](#wave-w1) | 15 | 2 | 0 | 2 | 0 | 19 | 79% |
| [W2](#wave-w2) | 0 | 0 | 4 | 0 | 0 | 4 | 0% |
| [W3](#wave-w3) | 0 | 0 | 9 | 0 | 0 | 9 | 0% |
| [W4](#wave-w4) | 0 | 0 | 3 | 0 | 0 | 3 | 0% |
| [W5](#wave-w5) | 0 | 0 | 5 | 0 | 0 | 5 | 0% |
| [W6](#wave-w6) | 0 | 0 | 5 | 0 | 0 | 5 | 0% |

## Current work

### W1-0013 — [~] in_progress

Finalize the integrated result and deliver an evidence-audited report. Owner: codex; revision 7; updated 2026-09-28 23:40 by codex.

Final make verify exited 0: legacy scan and all four required offline workspace checks passed. Final code evidence: report scenarios 298.60s (including mismatched causal environment), scoped capture/stop report 2.56s, planning 93.56s, progress/recovery 501.63s, result/acceptance 240.62s, restart and remaining workspace tests green. Independent code/documentation review R1(9/10) ACCEPT; doc precision corrections applied; staged diff and task-register checks passed. Ready to commit the implementation; completion will be recorded only after its commit is reachable from main. W1-0018 is independently ready and claimed in its separate owned worktree; no native inference or product experiment has begun.

### W1-0018 — [~] in_progress

Build the native Codex backend and discovery for accountable sessions. Owner: codex; revision 7; updated 2026-09-28 23:28 by codex.

Claimed by codex.

## Wave W1

15 of 19 done; 2 in progress; 2 paused or blocked.

| Status | Task | Title | Area | State | Depends on | Owner | Updated (HKT) | Rev | Evidence |
| :---: | --- | --- | --- | --- | --- | --- | --- | ---: | --- |
| [x] | [W1-0001](records/W1-0001.json) | Create the Rust domain/kernel foundation and replayable in-memory Journal | journal | done | — | codex | 2026-09-16 03:47 | 9 | commit [`adc78d1`](https://github.com/maggnus/ymp/commit/adc78d190c023192d158259756fc86f31ca4fd59) |
| [x] | [W1-0002](records/W1-0002.json) | Commit explicit goals, constraints and versioned acceptance criteria | intake | done | W1-0001 ✓ | codex | 2026-09-16 04:03 | 5 | commit [`43cfb3f`](https://github.com/maggnus/ymp/commit/43cfb3f5fcee7473f2b2b541895ba8b02b8adc85) |
| [x] | [W1-0003](records/W1-0003.json) | Resolve native agents and execution profiles through a trusted Registry | identity | done | W1-0001 ✓, W1-0002 ✓ | codex | 2026-09-16 04:25 | 7 | commit [`33fe98a`](https://github.com/maggnus/ymp/commit/33fe98acdc0d4b1012132443fa21d58b2fa79fda) |
| [x] | [W1-0004](records/W1-0004.json) | Charge invocations while protecting verification and policy-derived reporting capacity | resources | done | W1-0001 ✓, W1-0002 ✓, W1-0003 ✓ | codex | 2026-09-16 06:01 | 7 | commit [`960fc18`](https://github.com/maggnus/ymp/commit/960fc18223a24ebb8d794dcdfa2581ec5cb85282) |
| [x] | [W1-0005](records/W1-0005.json) | Capture direct workspace snapshots and enforce assignment path ownership | workspace | done | W1-0001 ✓, W1-0003 ✓, W1-0016 ✓ | codex | 2026-09-16 09:14 | 15 | commit [`0cff1e9`](https://github.com/maggnus/ymp/commit/0cff1e979a4585df21d866f09136d3a85761f4b1) |
| [x] | [W1-0006](records/W1-0006.json) | Admit contributions atomically with grants, reservations and active commitments | execution | done | W1-0002 ✓, W1-0003 ✓, W1-0004 ✓, W1-0005 ✓, W1-0016 ✓ | codex | 2026-09-28 10:10 | 13 | commit [`5ba6dbf`](https://github.com/maggnus/ymp/commit/5ba6dbf1) |
| [x] | [W1-0007](records/W1-0007.json) | Complete commitment leases, release and delegation without losing accepted work | coordination | done | W1-0006 ✓ | codex | 2026-09-28 10:53 | 6 | commit [`d7ea328`](https://github.com/maggnus/ymp/commit/d7ea328a) |
| [x] | [W1-0008](records/W1-0008.json) | Submit immutable result versions bound to attempts and exact workspace snapshots | results | done | W1-0005 ✓, W1-0006 ✓, W1-0007 ✓, W1-0017 ✓ | codex | 2026-09-28 17:04 | 6 | 9ef44a81; ymp-docs/result-implementation.md; crates/ymp-storage/tests/results.rs |
| [x] | [W1-0009](records/W1-0009.json) | Run versioned checks against immutable snapshots and record attributable check runs | verification | done | W1-0002 ✓, W1-0005 ✓, W1-0006 ✓ | codex | 2026-09-28 14:47 | 10 | commit [`ede5495`](https://github.com/maggnus/ymp/commit/ede54952) |
| [x] | [W1-0010](records/W1-0010.json) | Compute criterion satisfaction and independent acceptance from the recorded evidence | acceptance | done | W1-0009 ✓, W1-0019 ✓ | codex | 2026-09-28 19:13 | 8 | d0b102ef; ymp-docs/acceptance-implementation.md; crates/ymp-storage/tests/results.rs; crates/ymp-runtime/tests/belief.rs |
| [x] | [W1-0011](records/W1-0011.json) | Extract criteria and select minimal criterion-directed work through admitted strategies | planning | done | W1-0002 ✓, W1-0006 ✓, W1-0008 ✓, W1-0010 ✓ | codex | 2026-09-28 20:03 | 8 | df97ee3b; ymp-docs/planning-implementation.md; crates/ymp-storage/tests/planning.rs; crates/ymp-kernel/tests/replay.rs |
| [x] | [W1-0012](records/W1-0012.json) | Diagnose stalls and perform bounded recovery without discarding verified work | progress | done | W1-0007 ✓, W1-0019 ✓, W1-0010 ✓, W1-0011 ✓ | codex | 2026-09-28 21:35 | 8 | ba307435; ymp-docs/progress-implementation.md; crates/ymp-storage/tests/progress.rs; crates/ymp-runtime/tests/progress.rs |
| [~] | [W1-0013](records/W1-0013.json) | Finalize the integrated result and deliver an evidence-audited report | reporting | in_progress | W1-0006 ✓, W1-0008 ✓, W1-0019 ✓, W1-0010 ✓, W1-0011 ✓, W1-0012 ✓ | codex | 2026-09-28 23:40 | 7 | — |
| [=] | [W1-0014](records/W1-0014.json) | Run and recover the minimal accountable producer-reviewer session | session | planned (blocked) | W1-0007 ✓, W1-0011 ✓, W1-0012 ✓, W1-0013, W1-0018 | — | 2026-09-28 21:21 | 4 | — |
| [=] | [W1-0015](records/W1-0015.json) | Expose the accountable session through a simple interactive Ratatui interface | interface | planned (blocked) | W1-0014 | — | 2026-09-16 03:19 | 8 | — |
| [x] | [W1-0016](records/W1-0016.json) | Persist the new Journal and content-addressed payloads across restart | persistence | done | W1-0001 ✓ | codex | 2026-09-16 04:56 | 9 | commit [`07db5fa`](https://github.com/maggnus/ymp/commit/07db5fa45ad10a94e5560c711b7be0274eb5d1c9) |
| [x] | [W1-0017](records/W1-0017.json) | Execute admitted assignments through a new host and Scripted backend | execution | done | W1-0006 ✓, W1-0007 ✓, W1-0016 ✓ | codex | 2026-09-28 16:18 | 7 | 8de8e73a; ymp-docs/execution-implementation.md; crates/ymp-storage/tests/execution.rs |
| [~] | [W1-0018](records/W1-0018.json) | Build the native Codex backend and discovery for accountable sessions | providers | in_progress | W1-0003 ✓, W1-0017 ✓ | codex | 2026-09-28 23:28 | 7 | — |
| [x] | [W1-0019](records/W1-0019.json) | Bind attributed evidence and reviews to immutable results | verification | done | W1-0008 ✓, W1-0009 ✓ | codex | 2026-09-28 18:13 | 7 | 3689ad59; ymp-docs/evidence-implementation.md; crates/ymp-storage/tests/results.rs |

## Wave W2

0 of 4 done; 0 in progress; 0 paused or blocked.

| Status | Task | Title | Area | State | Depends on | Owner | Updated (HKT) | Rev | Evidence |
| :---: | --- | --- | --- | --- | --- | --- | --- | ---: | --- |
| [ ] | [W2-0001](records/W2-0001.json) | Design independent hidden checks before production and enforce their visibility | verification | new | W1-0003 ✓, W1-0006 ✓, W1-0019 ✓, W1-0013, W1-0014 | — | 2026-09-16 01:05 | 3 | — |
| [ ] | [W2-0002](records/W2-0002.json) | Execute property checks in explicit process and container environments | checks | new | W1-0009 ✓, W2-0001 | — | 2026-09-16 00:14 | 2 | — |
| [ ] | [W2-0003](records/W2-0003.json) | Verify browser behavior and external data with attributable check evidence | checks | new | W1-0019 ✓, W1-0013, W2-0001 | — | 2026-09-16 01:05 | 3 | — |
| [ ] | [W2-0004](records/W2-0004.json) | Measure check discrimination with faulty candidates and complete independent verification | verification | new | W1-0010 ✓, W1-0012 ✓, W1-0014, W2-0001, W2-0002, W2-0003 | — | 2026-09-16 00:14 | 2 | — |

## Wave W3

0 of 9 done; 0 in progress; 0 paused or blocked.

| Status | Task | Title | Area | State | Depends on | Owner | Updated (HKT) | Rev | Evidence |
| :---: | --- | --- | --- | --- | --- | --- | --- | ---: | --- |
| [ ] | [W3-0001](records/W3-0001.json) | Let agents use grant-scoped team operations and addressed board projections | communication | new | W1-0006 ✓, W1-0007 ✓, W1-0014, W2-0001 | — | 2026-09-16 01:05 | 3 | — |
| [ ] | [W3-0002](records/W3-0002.json) | Choose team members, profiles and reviewers from explicit demand and independence | team | new | W1-0003 ✓, W1-0006 ✓, W1-0012 ✓, W1-0013, W3-0001 | — | 2026-09-16 00:14 | 2 | — |
| [ ] | [W3-0003](records/W3-0003.json) | Award contributions through voluntary offers, deadlines and bounded reopening | allocation | new | W1-0004 ✓, W1-0011 ✓, W3-0001, W3-0002 | — | 2026-09-16 00:14 | 2 | — |
| [ ] | [W3-0004](records/W3-0004.json) | Respond to goal-status notices and clarification by revising accountable work | planning | new | W1-0011 ✓, W1-0012 ✓, W1-0015, W3-0001, W3-0003 | — | 2026-09-16 00:14 | 2 | — |
| [ ] | [W3-0005](records/W3-0005.json) | Transfer bounded decision and evidence context across commitment changes | context | new | W1-0007 ✓, W1-0013, W3-0001, W3-0003, W3-0004 | — | 2026-09-16 00:14 | 2 | — |
| [ ] | [W3-0006](records/W3-0006.json) | Resolve objections with counterexamples, evidence requests and bounded debate | disputes | new | W1-0010 ✓, W1-0012 ✓, W2-0004, W3-0001, W3-0002, W3-0003 | — | 2026-09-16 00:14 | 2 | — |
| [ ] | [W3-0007](records/W3-0007.json) | Execute diagnosis-driven reassignment, decomposition and profile escalation | recovery | new | W1-0012 ✓, W3-0002, W3-0003, W3-0004, W3-0005, W3-0006 | — | 2026-09-16 00:14 | 2 | — |
| [ ] | [W3-0008](records/W3-0008.json) | Dispatch independent contributions concurrently and react to coordination events | session | new | W1-0014, W3-0003, W3-0004, W3-0005, W3-0006, W3-0007, W3-0009 | — | 2026-09-16 01:05 | 3 | — |
| [ ] | [W3-0009](records/W3-0009.json) | Compare fixed and self-organizing policy sets on matched sessions | evaluation | new | W1-0011 ✓, W1-0014 | — | 2026-09-16 01:05 | 1 | — |

## Wave W4

0 of 3 done; 0 in progress; 0 paused or blocked.

| Status | Task | Title | Area | State | Depends on | Owner | Updated (HKT) | Rev | Evidence |
| :---: | --- | --- | --- | --- | --- | --- | --- | ---: | --- |
| [ ] | [W4-0001](records/W4-0001.json) | Open isolated copies and merge a selected immutable result through WorkspaceGuard | workspace | new | W1-0005 ✓, W1-0008 ✓, W3-0008 | — | 2026-09-16 00:14 | 1 | — |
| [ ] | [W4-0002](records/W4-0002.json) | Run sealed independent attempts and build the complete cross-candidate check matrix | alternatives | new | W2-0004, W3-0002, W3-0003, W3-0005, W4-0001 | — | 2026-09-16 00:14 | 1 | — |
| [ ] | [W4-0003](records/W4-0003.json) | Select an admissible attempt, merge it and recheck the integrated result | selection | new | W1-0013, W3-0007, W3-0008, W4-0002 | — | 2026-09-16 00:14 | 1 | — |

## Wave W5

0 of 5 done; 0 in progress; 0 paused or blocked.

| Status | Task | Title | Area | State | Depends on | Owner | Updated (HKT) | Rev | Evidence |
| :---: | --- | --- | --- | --- | --- | --- | --- | ---: | --- |
| [ ] | [W5-0001](records/W5-0001.json) | Create qualified competence observations and decaying profile reputation | experience | new | W1-0010 ✓, W3-0002, W3-0007 | — | 2026-09-16 01:05 | 2 | — |
| [ ] | [W5-0002](records/W5-0002.json) | Compare success and cost forecasts with attributable outcomes | calibration | new | W1-0004 ✓, W3-0003, W5-0001 | — | 2026-09-16 00:14 | 1 | — |
| [ ] | [W5-0003](records/W5-0003.json) | Curate scoped knowledge after delivery with independent review and provenance | knowledge | new | W1-0013, W3-0001, W3-0003, W5-0001 | — | 2026-09-16 00:14 | 1 | — |
| [ ] | [W5-0004](records/W5-0004.json) | Test knowledge with controlled retrieval and record exact context inclusion | learning | new | W3-0005, W5-0002, W5-0003 | — | 2026-09-16 00:14 | 1 | — |
| [ ] | [W5-0005](records/W5-0005.json) | Regrade accepted results from later signals and correct dependent experience | consequences | new | W1-0014, W3-0008, W5-0001, W5-0003, W5-0004 | — | 2026-09-16 00:14 | 1 | — |

## Wave W6

0 of 5 done; 0 in progress; 0 paused or blocked.

| Status | Task | Title | Area | State | Depends on | Owner | Updated (HKT) | Rev | Evidence |
| :---: | --- | --- | --- | --- | --- | --- | --- | ---: | --- |
| [ ] | [W6-0001](records/W6-0001.json) | Execute Claude assignments with native identity, team operations and accountable receipts | providers | new | W1-0017 ✓, W3-0001 | — | 2026-09-16 01:05 | 2 | — |
| [ ] | [W6-0002](records/W6-0002.json) | Execute Glm assignments with native identity, team operations and accountable receipts | providers | new | W1-0003 ✓, W1-0004 ✓, W1-0006 ✓, W3-0001, W3-0008, W4-0001 | — | 2026-09-16 00:14 | 1 | — |
| [ ] | [W6-0003](records/W6-0003.json) | Choose executable work methods from task structure and qualified history | routing | new | W1-0011 ✓, W3-0008, W4-0003, W5-0002, W5-0004 | — | 2026-09-16 00:14 | 1 | — |
| [ ] | [W6-0004](records/W6-0004.json) | Use calibrated contribution value throughout the complete session loop | allocation | new | W3-0008, W4-0003, W5-0002, W5-0005, W6-0001, W6-0002, W6-0003 | — | 2026-09-16 00:14 | 1 | — |
| [ ] | [W6-0005](records/W6-0005.json) | Evaluate and calibrate the complete policy set on reproducible comparisons | evaluation | new | W6-0004 | — | 2026-09-16 00:14 | 1 | — |

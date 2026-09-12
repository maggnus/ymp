# YMP-115 R1 correction

13/09 00:30 HKT — F1 corrected in the isolated candidate based on
`32624efd0b666e442068601c0828aeb174735542`; independent R2 acceptance remains pending.
The [independent R1 report](ymp-115-independent-review.md) is retained unchanged.

The default allocation heuristic now sizes membership for retained occupied actors
plus a distinct selected executor or reviewer. When B and C fail before A finishes,
their unresolved responsibilities remain occupied and the distinct eligible reviewer
gets the additional slot within the captured ceiling of four. A reaches exact-byte
confirmed acceptance in the same run, while the run remains blocked for the actual
scripted sibling failure. No pin, member ceiling, eligible pool, final-review rule,
authority check, resource exclusion or grant admission path was changed.

The public R1 probe is retained as
`independent_probe_two_failed_siblings_keep_eligible_review_reachable`, with a separate
`TwoFailures` fixture mode so the existing one-failure restart consumer is preserved.
All three independent producers reach execution barriers. The strengthened test
checks A's confirmation, retained failed invocations and unresolved sibling tasks,
14 observed input tokens across two partial calls, occupied actor retention, the
distinct admitted reviewer, unused budget, revoked grants and balanced workspace
reservations. On the same captured review input, a real ceiling of two or fixed size
of two still rejects the proposal; an already occupied reviewer adds no member.

| Verification | Actual result |
| --- | --- |
| Public probe before source correction | Exit 101; A remained `Review` |
| Final retained probe with the original sizing block temporarily restored | Exit 101; A remained `Review` |
| Identical retained probe with corrected sizing restored | Exit 0; A confirmed, B/C failures retained |
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| `cargo test --workspace` | Exit 0; 268 tests, including all 10 concurrency consumers |
| Bridge comparison with the reviewed candidate | Exit 0; no bridge changes |

The final failure/pass control records matching test-file SHA-256 values. Full
commands, outputs and timestamps are in
[correction evidence](../../ymp-docs/research/evidence/ymp115-r1-correction-checks.json).
Three intermediate regression-construction failures are labeled separately from
behavioral controls: a numeric type conversion and two assertions corrected to the
existing terminal-classification and inherited-model contracts. The original-policy
control was rerun after those test edits.

The unchanged bridge reuses the independent R1 type check and 13 passing tests.
Every execution was offline and scripted. No real-provider probe or credential read
was performed. Both implementation notes now identify `ymp.bounded-allocation` as
version 2. Intent, task registry, UI and application workspace policy were unchanged.

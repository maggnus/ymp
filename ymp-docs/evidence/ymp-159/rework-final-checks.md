# YMP-159 rework final checks

All build commands used
`CARGO_TARGET_DIR=/tmp/ymp159-parser-rework-target`. The independent probe used
`/tmp/ymp159-current-probe-target`. No native provider was invoked.

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo test -p ymp-core model::tests:: -- --nocapture` | 0 | All 14 parser and adjacent model tests passed. |
| `cargo test --manifest-path /tmp/ymp159-current-probe/Cargo.toml --offline --test ymp159_probe -- --nocapture` | 0 | All 32 independent semantic probe cases completed; exact results are in `rework-probe.jsonl`. |
| `cargo fmt --all --check` | 0 | No output. |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | Finished the workspace `dev` profile without warnings. |

## Workspace-test blocker

The unfiltered `cargo test --workspace` command was not repeated on the rework
because YMP-161 was still in progress and its correction was absent from this
branch. At the final check:

- `git merge-base --is-ancestor a24d125 HEAD` returned 1;
- the YMP-161 worktree still had an uncommitted change to
  `ymp-rust/crates/ymp-eval-driver/tests/weak_pilot_consumer.rs`;
- `git diff --quiet 1c17f4e` over the product paths guarded by that executable
  returned 1.

Therefore five `weak_pilot_consumer` tests in this branch still stop at
`Product bytes differ from accepted P0 base 1c17f4e` before exercising their
fixtures. Candidate `1ab42b1` already captured an unfiltered workspace run and
the supplemental run that skipped only those five externally blocked cases in
`final-checks.md`. The rework changes only `ymp-core::model::parse_response`,
its tests, and this evidence directory; it does not alter the blocker.

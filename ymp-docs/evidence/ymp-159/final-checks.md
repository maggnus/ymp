# Final checks

All Cargo commands used `CARGO_TARGET_DIR=/tmp/ymp159-parser-target`, except
`cargo fmt`, which does not build artifacts.

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all --check` | 0 | No output. |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | Finished the workspace `dev` profile without warnings. |
| `cargo test --workspace` | 101 | Five `weak_pilot_consumer` cases stopped at their frozen-product guard; all tests reached before that executable passed. |
| `cargo test --workspace -- --skip all_six_conditions_consume_real_task_exports_and_freeze_one_blind_result --skip optional_reported_settings_follow_the_same_rule_in_all_conditions --skip ordinary_negative_results_do_not_gate_calibration_or_later_conditions --skip solo_uses_whole_remaining_time_and_independent_members_do_not_reset_it --skip unknown_usage_and_deadline_stop_the_same_consumer_without_losing_observations` | 0 | Every remaining test passed; two existing tests were ignored. |

## Unfiltered workspace failure

```text
     Running tests/weak_pilot_consumer.rs (/tmp/ymp159-parser-target/debug/deps/weak_pilot_consumer-f0904d6c90685e8b)

running 6 tests
test unknown_usage_and_deadline_stop_the_same_consumer_without_losing_observations ... FAILED
test ordinary_negative_results_do_not_gate_calibration_or_later_conditions ... FAILED
test all_six_conditions_consume_real_task_exports_and_freeze_one_blind_result ... FAILED
test solo_uses_whole_remaining_time_and_independent_members_do_not_reset_it ... FAILED
test optional_reported_settings_follow_the_same_rule_in_all_conditions ... FAILED
test approval_kind_and_source_drift_fail_before_any_provider ... ok

failures:

---- unknown_usage_and_deadline_stop_the_same_consumer_without_losing_observations stdout ----
called `Result::unwrap()` on an `Err` value: Os { code: 2, kind: NotFound, message: "No such file or directory" }

---- ordinary_negative_results_do_not_gate_calibration_or_later_conditions stdout ----
called `Result::unwrap()` on an `Err` value: Os { code: 2, kind: NotFound, message: "No such file or directory" }

---- all_six_conditions_consume_real_task_exports_and_freeze_one_blind_result stdout ----
Error: Product bytes differ from accepted P0 base 1c17f4e

---- solo_uses_whole_remaining_time_and_independent_members_do_not_reset_it stdout ----
Error: Product bytes differ from accepted P0 base 1c17f4e

---- optional_reported_settings_follow_the_same_rule_in_all_conditions stdout ----
Error: Product bytes differ from accepted P0 base 1c17f4e

test result: FAILED. 1 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.25s

error: test failed, to rerun pass `-p ymp-eval-driver --test weak_pilot_consumer`
```

The two missing-file failures occur after the same consumer exits before
creating its report or trace. The guard checks `git diff --quiet 1c17f4e` over
all product paths. Running that exact diff against the clean YMP-159 base
`a8367c4` returns 1 and lists only the already accepted YMP-146 TUI files.

## Supplemental workspace completion

After the five base-incompatible cases were skipped, every remaining unit,
integration, and documentation test completed successfully. The final result
blocks included:

```text
test result: ok. 113 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 275 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

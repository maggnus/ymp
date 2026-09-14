# YMP-161 frozen pilot binding evidence

Branch: `fix/ymp161-pilot-source-binding`

Task-registration base: `a24d125178a90e9e2b38d16ae2b91241839d2b5c`

The change is limited to the unattended weak-pilot consumer tests, this
evidence directory and the generated task-register views. It adds no
dependency and does not change the runtime consumer, the manifest generator,
the accepted product revision, an executable digest, a wrapper digest, a
fixture digest or either inactive native manifest.

## Original failure

The original `weak_pilot_consumer.rs` from `a24d125` was checked out in a
separate temporary local clone before the retained uncommitted change was
altered. The command was equivalent to:

```sh
CARGO_TARGET_DIR=<worktree>/target cargo test \
  --manifest-path <temporary-clone>/Cargo.toml \
  -p ymp-eval-driver --test weak_pilot_consumer -- --nocapture
```

Result: exit 101; 1 passed and 5 failed. Three failures reported
`Product bytes differ from accepted P0 base 1c17f4e`. The other two attempted
to read controller output that had not been created after the same early
rejection and reported `No such file or directory`. This reproduced the task
card's failure without changing the retained worktree source.
[baseline.log](baseline.log) captures the same five failures on the accepted
YMP-159 source before applying the YMP-161 test change.

## Test boundary

`FrozenRepository` creates a temporary local clone, checks out the exact
accepted product object
`1c17f4e447b839e20f8efbdee0900963a5d8fcad`, and restores only the current
weak-pilot consumer, validators and evaluation-driver binary source from the
current committed revision. A sanity check requires every product path used by
the runtime guard to remain byte-identical to the accepted object. The
existing manifest generator then hashes every consumer, wrapper and fixture
file in that constructed repository.

The production guard is unchanged. It still checks the manifest kind, current
runner digest, Python executable, restricted-Python executables and wrapper,
native or fixture executable, source ancestry, product bytes and every frozen
consumer or fixture file before any work. Native execution additionally still
requires a separate exact-manifest approval and one-phase ledger claim.

Three controls cover the safety boundary added or made explicit here:

- `one_manifest_bound_product_file_change_is_rejected_before_fixture_work`
  generates a valid fixture manifest, appends bytes to the frozen clone's
  `ymp-rust/crates/ymp-core/src/lib.rs`, confirms that this is the one changed
  product path and observes `Product bytes differ from accepted P0 base
  1c17f4e` before a controller directory exists.
- `exact_old_accepted_manifests_remain_inactive_for_the_current_runner` checks
  all six archived calibration and pilot manifest files. The `consumer-v2`
  pair retains manifest digests
  `34993fee70abbffb0db35a77277b42b4d22dc3c7ec6bb77ced8c05e33f70c076`
  and `a3eaadab9d484f3d2adabaf486a959f58b49dd2c333490d2bebe6d1053fd282a`
  and runner digest
  `ecb46cb19cb6d22017be05ef52d386307d76b8d0766c2f758c581bd4b2201f46`;
  the byte-identical `proposed-run` and `rework-round1` pairs retain manifest
  digests
  `b21ad8dc537fe9c11244b43293c6162cf5c774d328fc47c0be9762310386b179`
  and `dffb4e86c30afe58da32564e474064793aab0c88208bd757f7cc4c41d295f98b`
  and runner digest
  `4790774e92090df62a62083c471a05e96f855efd25862577ca4266cc918a92bc`.
  Both frozen runner digests differ from the runner built for these tests.
- `unapproved_native_manifest_stops_before_provider_work` uses a temporary
  executable that would create a sentinel if started. The fully bound native
  control manifest is rejected with `owner_approval_required: no measured
  launch is authorized`; no controller, approval ledger or sentinel exists.

No old native manifest was executed. The native control uses artificial model
identifiers and a temporary executable; it does not call a provider.

## Focused results

| Check | Result |
| --- | --- |
| Product-object mutation, inactive-manifest and no-approval native controls | exit 0; each control passed alone; [focused-controls.log](focused-controls.log) |
| `cargo test -p ymp-eval-driver --test weak_pilot_consumer -- --nocapture` | exit 0; 9 passed, 0 failed; 57.49 s; [focused-suite.log](focused-suite.log) |
| Same nine tests applied to accepted YMP-159 revision `1ab42b1f3d5d4f0d1e6df28c93633128b659327f` in a temporary clone | exit 0; 9 passed, 0 failed; 58.16 s; [parser159-combined.log](parser159-combined.log) |

The YMP-159 revision descends from the accepted YMP-146 source. Its product
changes include `ymp-rust/crates/ymp-core/src/model.rs` in addition to the
YMP-146 TUI changes, so this compatibility run confirms that both later
accepted changes can use the self-contained weak-pilot tests without relaxing
the frozen product guard.

## Required final checks

All commands ran in this worktree on the final Rust source. No release build or
real-provider check was run.

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | exit 0; [final-fmt.log](final-fmt.log) |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0; [final-clippy.log](final-clippy.log) |
| `cargo test --workspace` | exit 0; 651 passed, 0 failed, 2 ignored |
| `python3 ymp-docs/tasks/manage.py check` | exit 0; 79 tasks valid before completion update |

Exact source, manifest and Git-tree bindings are recorded in
[hashes.txt](hashes.txt).

## Scope and limitations

- YMP-201 remains paused. No calibration, measured pilot, provider discovery,
  provider invocation, Astra use, native approval or approval-ledger claim
  occurred.
- No application-owned or other user data was opened or changed. All new
  repositories, controller paths, workspaces, executables and sentinels used
  by the controls were temporary.
- The unattended consumer tests already require Python and the Git executable.
  The new fixture also requires the accepted Git object `1c17f4e...` to be
  present in the local repository. This is the exact-object option permitted
  by YMP-161; a source archive or a shallow checkout that omits the object is
  not covered.
- The two existing ignored workspace tests were not enabled: one requires a
  locally built Claude bridge and one retains interactive interface fixtures.
- Native manifests, executables and user data were not exercised end to end;
  their byte bindings were preserved in unchanged runtime code and checked by
  non-invoking controls.

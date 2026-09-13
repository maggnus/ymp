# Host-independent pool refusal in a TUI width test

`no_row_puts_more_in_its_right_column_than_the_narrowest_column_holds` checks every page's right
column at the narrowest supported width. Its second half builds an `App` from the shipped
configuration and requires at least one row to carry the pool's own refusal, `no native model`.

## Cause

The shipped providers name `codex`, `claude` and `npx` on `PATH`. `inspect_pool` records
`ExecutableMissing` before `NativeModelUnresolved`, and a pool row shows the first exclusion. On a
host without those programs, such as the Linux container, every shipped profile therefore read
`its program was not found`, no row read `no native model`, and the test failed at
`tests.rs:7241` (`refusals == 0`). A macOS host with the providers installed passed.

## Correction (test only)

The test still starts from `Config::default()`, but every provider's command is set to the
absolute path of the running test executable (`std::env::current_exe()`). `discovery::executable`
only reads that file's metadata; nothing is launched and `PATH` is not consulted. The profiles stay
the shipped placeholders, which `is_legacy_provider_placeholder` decides from the profile and
provider kind, not from the command, so the pool records `NativeModelUnresolved` on every host.
Every assertion is unchanged, and so is production eligibility and presentation.

## Evidence

Reproduced on macOS by running the test executable with `PATH=/usr/bin:/bin`, where `codex`,
`claude` and `npx` are absent.

| File | What it shows |
| --- | --- |
| [before-restricted-path.txt](before-restricted-path.txt) | Before the correction, restricted `PATH`: fails at `tests.rs:7241`, as on Linux. |
| [after-restricted-path.txt](after-restricted-path.txt) | After, restricted `PATH`: passes. |
| [after-host-path.txt](after-host-path.txt) | After, the host's own `PATH` with `codex` installed: passes. |

Required checks, run once after the correction with `CARGO_BUILD_JOBS=2`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0` and a target directory inside the worktree:

| Check | Result | Output |
| --- | --- | --- |
| `cargo fmt --all --check` | exit 0 | [fmt.txt](fmt.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | [clippy.txt](clippy.txt) |
| `cargo test --workspace` (macOS host) | exit 0; 573 passed, 0 failed, 2 ignored | [workspace-tests.txt](workspace-tests.txt) |

Not run here: the Linux workspace suite itself; the restricted `PATH` run stands in for the
container's missing providers.

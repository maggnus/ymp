# Embedded Git backend verification

The single backend is git2 0.21.0 with vendored libgit2 and statically built zlib. Network
transports are disabled. No Git executable, shell, hook, credential helper or external filter
is invoked by the backend. The new dependency supplies a demonstrated missing capability after
the owner excluded local Git tools. The existing highlighter and diff styles are retained.

Nine real-repository fixtures pass on macOS using only the embedded library: unborn/non-Git,
base-versus-working comparison, staged plus unstaged changes whose net content cancels, exact
native paths/renames, linked-worktree branch occupancy, dirty/stale guards, binary/large files,
ignored collisions and hooks, raw non-UTF-8 path conversion, and cancellation ownership.
The tests create commits and linked worktrees through git2, not the Git CLI. Strict package
Clippy passes. Workspace-wide and installed/UI verification follow the final integration.

Git work is serialized behind one worker permit. Dropping an awaiting UI task does not release
the running worker or its passed StoreLock. Library filesystem calls are not forcibly cancelled;
timing checks are cooperative, and status enumeration itself can exceed the time allowance.
This is a bound on concurrent admitted work, not a hard-duration guarantee on network mounts.

The popup correction is independently authored and verified separately under YMP-141.

## Reference-lock refusal

A later adversarial check found that an existing HEAD.lock was rejected only after files and
index were changed. `a_locked_head_refuses_checkout_before_changing_files_or_index` fails on the
previous implementation (`head-lock-before.txt`). The switch now reserves HEAD, current branch
and target branch references through a libgit2 transaction before checkout. Ten backend tests
and strict package Clippy pass after the correction. This avoids the predictable lock refusal;
it does not promise filesystem rollback if a device fails during checkout or reference commit.

## Independent review follow-up

The lock regression now covers pre-existing HEAD.lock, index.lock and target-branch lock files,
checking unchanged file bytes, index, HEAD and the foreign lock itself. The expanded control
passed before adding any index-lock-specific check, so the review hypothesis that a pre-existing
index.lock corrupts this implementation was not reproduced; its existing dry-run preflight is
retained. The standard `checkout: moving from ... to ...` reflog format is restored and tested
through `@{-1}`. Branch occupancy is rechecked after the target reference lock is acquired.
Eleven backend tests pass after these corrections. No filesystem transaction or protection
against arbitrary external writers is claimed.

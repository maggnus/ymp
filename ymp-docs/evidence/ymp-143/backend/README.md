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

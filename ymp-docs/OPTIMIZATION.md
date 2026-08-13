# Execution optimization

Measured slowdowns in this project's delivery loop and what removes each. Every entry states the
measurement it came from, so a later reader can re-check rather than trust the claim. Recorded
2026-08-13.

## What actually costs time

A card that one agent builds and one agent reviews takes 85 to 145 minutes end to end. Compilation
is not the cost: an incremental workspace build with a warm cache finishes in 6 to 15 seconds. The
cost is the agent's own work inside the card — reading, searching, writing, and proving — plus the
waiting introduced by real processes.

## 1. Every workspace compiles from scratch

**Measured.** The integration tree's build directory holds 3.3 GB; two more exist under temporary
paths at 1.4 GB and 409 MB, and each agent worktree carries its own. The workspace has 141
dependencies, so a new worktree starts with a full cold build.

**Remove it with a shared compilation cache.** A shared build directory is the wrong tool: cargo
locks it, so concurrent agents would serialize. A compiler cache keyed by input hash shares artifacts
without a shared lock. Neither `sccache` nor a project `.cargo/config.toml` exists today.

**Expected effect.** The cold build at the start of each card collapses to a cache read. With two to
three agents dispatched per hour, this is the largest single saving available.

## 2. Timing-sensitive tests run against a loaded machine

**Measured.** Three test binaries that wait on real processes take 15 to 19 seconds each. Run while
two agents were compiling, one of six tests failed twice; the same suite passes cleanly when the
machine is idle — sixty test binaries, no failures.

**Remove it by grouping.** Tests that observe the process table, timeouts and descendant termination
belong to a named group that the integration check runs on a quiet machine, not to the set every
card runs. A flaky red result costs a full diagnosis cycle and teaches the fleet to distrust its own
checks.

## 3. The full suite ran at the end of every card

**Measured.** A full workspace run takes 7 minutes 14 seconds and was repeated at the end of each of
eight cards.

**Already applied.** The suite is an integration check: it runs once before a merge into the release
branch. A worker proves its card with the narrow tests for what it changed and their negative
halves. Recorded in `AGENTS.md` and `DECISIONS.md`.

## 4. Oversized source files

**Measured.** Three files exceed two thousand lines: the Codex driver at 2818, the Claude Code driver
at 2760 and the supervisor at 2447. Agents read them in two-hundred-line windows before the first
edit, and two of the eight cards hit context compaction while doing so.

**Remove it by splitting along seams.** The terminal crate already went from a 2307-line file to
thirteen modules, and the card after it read only what it needed. Splitting also widens
parallelism: two cards cannot run in one file, but they can run in two modules.

## 5. Only two independent lanes exist at a time

**Measured.** The task budget allows three, and the third slot stood empty for most of the session
because every remaining ready node touched a crate a live card already owned.

**Remove it two ways.** Batch homogeneous nodes into one contract — accounting evidence together,
unfalsifiable guards together, ledger defects together — which turns six cards into three without
weakening any acceptance. And split cards along subsystem seams before dispatch rather than
discovering the overlap at admission time.

## What does not help here

A faster linker is a Linux answer. On this machine the platform linker is already the fast one, and
the alternatives either do not support the platform or are not free software.

## Order

The compilation cache first, because it pays on every future card. Then the test grouping, because a
flaky red is more expensive than a slow green. Then the file splits, taken as ordinary cards where
they block the next piece of work.

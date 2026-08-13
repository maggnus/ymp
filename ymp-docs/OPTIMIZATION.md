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

**A compiler cache was tried and refuted.** `sccache` 0.17 was installed and wired in as the compiler
wrapper with incremental compilation disabled. Three consecutive builds — a second target directory,
a different source path, and a wiped target directory — each produced fifty compilations and **zero
cache hits**, while the cache itself filled to 33 MiB and reported twenty-two calls as non-cacheable
by crate type. The wrapper was removed rather than kept for an effect it does not deliver here.

**What did work: less debug information.** Setting the development profile to line tables only cut a
cold build of the command crate from 23.2 to 14.0 seconds and its build directory from 479 to 403
megabytes, with panic backtraces still carrying file and line. Applied.

**Still open.** Sharing artifacts across worktrees needs either path remapping so that identical
sources hash identically, or a shared target directory whose lock contention is measured rather than
assumed. Neither is worth a card until the measurement above is repeated on a machine that is idle.

## 2. Timing-sensitive tests run against a loaded machine

**Measured.** Three test binaries that wait on real processes take 15 to 19 seconds each. Run while
two agents were compiling, one of six tests failed twice; the same suite passes cleanly when the
machine is idle — sixty test binaries, no failures.

**Remove it by grouping, and by sequencing.** Tests that observe the process table, timeouts and
descendant termination belong to a named group that the integration check runs on a quiet machine,
not to the set every card runs. Sequencing matters as much as grouping: the integration check runs
**before** the fleet is refilled, never while freshly dispatched writers are compiling. Measured
three times — each red disappeared when the same suite was rerun in isolation, and each cost a
diagnosis cycle.

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

A compiler cache, as measured above. The assumption was reasonable and the measurement refuted it;
the entry stays here so the next reader does not spend the same hour.

## Order

Reduced debug information and the removal of the per-card full suite are applied. Next is the test
grouping, because a flaky red costs a full diagnosis cycle and teaches the fleet to distrust its own
checks. Then batching homogeneous nodes into one contract, which needs no code at all. Then the file
splits, taken as ordinary cards where they block the next piece of work. Artifact sharing across
worktrees stays open until it can be measured on an idle machine.

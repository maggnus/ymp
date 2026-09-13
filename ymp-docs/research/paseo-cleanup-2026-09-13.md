# Paseo cleanup for ymp2

Task: YMP-138. Date: 2026-09-13.

The owner requested investigation and deletion of orphaned ymp2 resources in Paseo.
Cleanup preserved unfinished theme work, existing owner edits, historical commits
and uncommitted review evidence. Product source and approved intent were unchanged.

## Removed resources

| Resource | Result |
| --- | --- |
| Obsolete temporary Paseo projects | 4 deleted |
| Their workspaces and completed agents | 4 workspaces and 4 agents archived |
| Registered development worktrees | 61 removed; 2 retained |
| Additional Cargo build caches | 14 removed |
| Abandoned YMP-107 mock TUI process | PID 7407 stopped with SIGTERM after verifying its exact command and parent PID 1 |

Deleted projects: `prj_a99d6b0ed921a5ea` (`107`),
`prj_5ff48c181fc2d7b0` (`review107`), `prj_8330649f364b3e40`
(`review118_core`), and `prj_b1dd91f3c79de8d3` (`ymp131-double-ctrl-c`).
Their agents were idle with completed assignments before deletion and verified
archived afterwards. Native conversation history remains available.

Removed directories accounted for 62,947,372 KiB of allocated blocks (about
60.03 GiB). Filesystem free space increased by about 57.24 GiB, from 14.26 to
71.50 GiB during cleanup and validation. These are separate measurements: directory
allocation totals and observed filesystem free space need not match.

## Preservation and remaining work

- All 61 original worktree HEADs remain reachable in Git. Fifteen commits without
  existing references were retained under `refs/archive/ymp2-cleanup-20260913/`.
  Existing branches were retained.
- Ninety-seven changed or untracked files and symbolic links were archived, read
  back and checked by SHA-256 or link target before removal. Tracked changes also
  have binary-capable patches. Ignored build output and installed dependencies were
  discarded; linked dependencies in the main checkout remain intact.
- The main checkout remains at `730a814a368d5a0ed44338f1803b17eee0b077c1`.
  Existing request and theme-design files retained their original checksums.
- The clean worktree `/private/tmp/ymp132-agent-attribution` remains on
  `feat/ymp137-library-themes` at `508d7f575ea73e9b4d65e81949b006883197caad`.
  It contains authored YMP-136/137 changes awaiting independent verification,
  integration and installation. Its Paseo agent `1e1a7a7b-030f-4eca-9d9f-9a02c32b1cd9`
  and `/private/tmp/ymp132-test-target` build cache remain available.
- Other projects, current owner conversations, historical evidence directories and
  source snapshots outside the removed worktrees were retained. This assignment
  did not perform a general purge of temporary directories or provider history.

## Validation

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace` all exited successfully on the unchanged main source.
The test run passed 498 tests, failed none and ignored two. No real-provider
inference was used for verification.

The final Git worktree list contains only main and the pending theme branch. All
deleted paths are absent, all preserved HEADs remain referenced, the old mock
process and archived native agent processes have exited, and unrelated Paseo
projects remain registered. No Paseo schedules or managed terminals were present
in the inventory. No process had an open file in any removed worktree or cache
when its removal started.

## Local recovery evidence

The local archive is
`/Users/maggnus/.paseo/cleanup/ymp2-20260913T082334Z/`.
Its `README.md` describes reconstruction; `preservation.json` maps old paths,
HEADs, Git references and archive members. `verification.json` records the result;
the same directory contains before/after inventories and validation logs.

`uncommitted-files.tar.gz` SHA-256:
`1c223f0ec1b9f600f2ffb316c95c1bead0f20c83f7ecadf2ee5414f00c4c06ff`.

Archive entries were verified individually; full restoration of all historical
worktrees and new theme acceptance were not performed. The next product step is
independent verification and integration of the retained YMP-136/137 branch.

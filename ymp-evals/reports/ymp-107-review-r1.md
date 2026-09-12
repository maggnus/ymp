# YMP-107 independent review: recorded checks and factual recovery limits

**Verdict: RETURN. R1 = 8/10.** The implementation is accepted as correct; two sentences of
delivered text state more than the delivered behaviour supports and must be corrected before
integration. No change to the logic is required.

Reviewed commit `9562f8b44f16291cc21f93d472f5ca3953ad0a48` ("Expose recorded checks and state
the recovery limits") in the detached `review107` worktree, which was at `9562f8b` and clean
before and after the review and is still clean. No file in the worktree was edited except this
report, and nothing was committed. Every mutation experiment ran in throwaway directories
under `/tmp` outside the worktree.

Reviewer: delegated Claude session, model `claude-opus-5`, permission mode `bypassPermissions`,
thinking level requested as `max` by the assignment. The model identifier and the permission
mode are observable inside the session; the thinking level is not, so it is recorded as
requested rather than as measured. No provider request was made, no credential was read, and
the task registry was not modified. The author's report was not used as evidence.

## Commands and results

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace` | exit 0, 118 tests passed, 0 failed |
| `cargo build --bin ymp` | exit 0 |
| `ymp --home … -C … demo` (mock provider, nested project) | exit 0, two checks recorded |
| `ymp --home … -C … run …` with `[mock:broken-output]` | exit 1, session blocked, failing check recorded |

The full diff was read: 14 files, 1160 insertions, 19 deletions, including `CheckOutcome` and
`CheckRun` in `ymp-core/src/model.rs:34` and `:47`, `Store::checks` and `recorded_text` in
`ymp-storage/src/lib.rs:213` and `:399`, the whole of `ymp-workspace/src/repository.rs`, and
every changed line of `views.rs`, `state.rs`, `commands.rs` and the four documents.

## Verification against the acceptance criteria

**Recorded check commands and outcomes are shown for the selected session, with no claim of
sandboxing or approval.** `Store::checks` reads `events` rows of kind `check` for one session
ordered by `seq` and maps each field through `recorded_text`, so an absent, blank or
non-string field stays `None` and `success` missing becomes `CheckOutcome::Unrecorded` rather
than a pass. This matches what the runtime writes at `ymp-runtime/src/engine.rs:1124`. Driven
in a pseudo-terminal at 100x30 the page showed all four categories at once: two `passed` runs,
one `failed`, one `no recorded outcome` for a record written without `success`, and one
`no recorded run` for a plan command with no run in the log, under the subtitle
`4 recorded · 1 declared without a run`. Each of the two runs of the identical command showed
its own timestamp (`…198769` and `…237889`), so two runs of one command are not collapsed.

Every sentence of `HOW_CHECKS_RUN` was checked against the runtime rather than taken as
written. `/bin/sh` with `-c` in the task directory: `engine.rs:1111`. The directory is the
project working directory: `task.workspace` is assigned `ctx.workspace.directory` at
`engine.rs:651`, and the final pass uses `ctx.workspace.directory` at `engine.rs:691`. Nothing
limits the command and nothing is asked first: there is no gate between the status line and
`cmd.output()`. First 20000 characters: `text.chars().take(20000)` at `engine.rs:1123`. A
timeout or a cancellation leaves no record: both paths `bail!` before the `store.event` call.
A non-zero exit keeps the task unaccepted whatever an agent reported: `review.approved &=
check_result.is_ok()` at `engine.rs:1049`. That last one was confirmed in a real run, not only
by reading: with the executor forced to write wrong content, the reviewer agent returned
`approved: true`, the check exited 1, the task ended `blocked` and the page showed `failed`
with the real output and directory. A grep of the new surfaces for `sandbox`, `approval`,
`permitted` and `allowlist` returned nothing.

**The change view explains recovery limits without promising rollback or adding a
confirmation.** The statement leads the populated list as a selected first row and carries the
empty state, so it is on the first frame in both states. It was read on screen at 100x30 and
at 80x24, with and without recorded changes, and with and without a repository. No new
keystroke, confirmation or block was introduced anywhere in the change path. The two claims
the statement makes about the record were checked against `ymp-workspace/src/lib.rs`: the
skipped names are exactly `.git`, `node_modules`, `target`, `__pycache__`, `.DS_Store`,
`.ymp2` and any path under the session metadata directory, and the stored value per file is a
SHA-256 of the content, or of `symlink:` plus the link target, never a copy. The baseline is
taken when the session's workspace metadata is first created and reloaded unchanged
afterwards, which is what "the fingerprint taken when the session started" means. The list is
written once per run at `engine.rs:313`, on the completed, paused and blocked paths alike; a
blocked run in the adverse case did write it.

**Repository absence is not inferred from `cwd/.git`.** Discovery lives in
`ymp-workspace/src/repository.rs`, is called from the controller at three points
(`state.rs:276`, `:497`, `:658`) and never from a page. It reads directory metadata and, for a
`.git` file, its first 4096 bytes; it spawns no process and writes nothing. Nine cases were
run against the real module through an external probe crate outside the worktree:

| Case | Result |
| --- | --- |
| Working directory two levels below a repository root | marker at the root, `inspected=3` |
| `.git` file with a `gitdir:` line | reported as a linked worktree or submodule, target named as written, not followed |
| `.git` file without a `gitdir:` line | link with unknown target, "which repository covers this directory is unknown" |
| No marker up to `/` | bounds of the search stated, absence not concluded |
| `.git` is a fifo | "neither a directory nor a readable file", unknown |
| Ancestors unreadable (mode 000) | both denied entries listed with their reason, absence not concluded |
| Working directory that cannot be canonicalized | path kept as given, walk still performed |
| `GIT_DIR` set, `GIT_WORK_TREE` blank | `GIT_DIR` reported, the blank variable not reported as a setting |
| `.git` is a symlink to a directory | followed, reported as a repository directory |

The three phrases that would conclude absence ("not under version control", "no repository",
"is not a repository") appear nowhere in the output of any case. End to end, a project with no
marker anywhere produced "No .git entry was found in the working directory or in the 4
directories inspected above it, up to /", followed by what that does not establish.

**Project data stays in the working directory and metadata in its home.** The commit adds two
reads and no write. Discovery opens files read-only and `Store::checks` issues a `SELECT`.
Nothing was created under the reviewed worktree during the walks.

## Findings

### Required before integration

1. **`ymp-docs/architecture/recorded-checks-and-recovery.md:28` states something the interface
   does not do.** The sentence reads "the page shows the first 80 display lines of that; the
   rest is reachable in the full record with `Enter`". The rest is reachable nowhere.
   `recorded_check` cuts the output to `OUTPUT_LINES` when the row is built
   (`views.rs:1063`, `:1093`), and `Enter` opens `Overlay::Inspect` with `item.detail.clone()`
   (`state.rs:1308`), that is, the same already-cut lines. Driven against a seeded check whose
   recorded output was the full 20000 characters, the overlay scrolled to its end and finished
   with "208 further recorded lines are not shown here." and "0 more" remaining. Correct the
   sentence to say the remainder is not reachable from the interface. This is the feature's
   own contract document, and the feature is disclosure.

2. **The page hint "Enter — show the whole record" overstates for a truncated output**
   (`views.rs:1019`). What `Enter` shows is the record up to 80 wrapped lines of output, and
   the detail then says so itself, which is the honest part. The hint should not promise the
   whole record; "show the recorded run" or equivalent would match the behaviour. This is the
   only code change required.

### Non-blocking

3. **The `/checks` empty state asserts a result when nothing was read** (`views.rs:1011`).
   With no session loaded, the subtitle correctly says "No session is loaded" while the
   headline says "No checks were recorded", which is a statement about a session that was
   never opened. `Store::checks` is not even called in that state. The pattern is inherited
   from the change page's empty state, so it is consistent, but on a page built to keep an
   absent record distinct from a result it is the wrong default. Wording such as "Nothing was
   read: no session is loaded" would remove the ambiguity.

4. **A declared command is hidden by a recorded run of the same string from a different
   task.** The filter in `checks` matches `run.command` against `task.checks` by string
   equality with no task identity (`views.rs:968`). If two tasks declare the same command and
   the run reaches only one of them, the other task's unrun command is not listed as
   "declared, without a recorded run". The final pass runs the union of all task commands in
   the working directory, so the gap opens only when a run stops before it. Matching on the
   pair of task and command, or naming the task on the recorded row, would close it.

5. **Detail text is clipped, and this is pre-existing, not introduced here.** Two columns are
   lost in the inline pane, because pages wrap for `area.width - 2` while the pane is painted
   into `area.width - 4` (`ui.rs:569`), and up to sixteen columns are lost in the `Enter`
   overlay, because key handling rebuilds the page at `app.viewport.width`, the full terminal
   width (`lib.rs:242`, `ui.rs:31`), and the modal is 86 columns wide. Words disappear mid
   sentence: "the first 20000 characters of its combin[ed] output", "was judged [by]
   inspection alone", "so earl[ier content] exists only where". The same clipping was
   reproduced on the untouched memory page ("Run the check [on the final] integrated
   artifact"), which establishes that it belongs to the shared frame, not to this commit. It
   is recorded here because the disclosures this commit delivers are longer prose than any
   existing page carries, so they are the text most damaged by it, and because it deserves a
   task of its own.

6. **Below the documented minimum size the change page shows no statement until a row is
   opened.** The detail pane is suppressed when the body is shorter than twelve rows
   (`ui.rs:532`). At 100x16 the leading row still reads "What was recorded, and what cannot be
   put back"; at 40x12 that title is truncated to "What was recorded,…". Both are below the
   80-column minimum the README documents, and the statement remains one keystroke away.

7. **`Store::checks` is a full table scan** (no index on `events(session_id)`; `EXPLAIN QUERY
   PLAN` reports `SCAN events`). The architecture document discloses this. The page is rebuilt
   on every state revision, so during a live run the scan repeats per event; at present event
   counts this is not measurable, and it is noted only so the cost is on record.

8. **One delivered test depends on the temporary directory having no Git ancestor.** The
   second half of `repository_discovery_looks_above_the_working_directory_and_never_concludes_absence`
   asserts that discovery from a fixture directory finds no marker. It passes wherever `TMPDIR`
   is outside a repository, which is the normal case on macOS and Linux, but it would fail in
   an environment whose temporary directory is inside one.

## Limits of this review

The walks used the in-process mock provider through `ymp demo --tui` and a hand-written
configuration with `kind = "mock"`, which returns before any process is spawned, so no
statement here rests on a real provider. The pseudo-terminal captures were produced by a
minimal screen emulator written for this review; it reproduces cursor positioning, erasure and
text placement, and its readings were cross-checked against the assertions of the delivered
interface tests, but it is not a full terminal. Everything observed was at 40x12, 60x20,
80x24, 100x16 and 100x30 on the Ember palette with Unicode markers; the other four palettes
were read in `theme.rs` and not rendered. The `faint` token that the "no recorded run" row
uses measures about 3.7:1 against the Ember background, which is below WCAG AA for body text,
but it is the token already used for hints and timestamps and the state is also carried in
words, so it is not raised as a finding. `cargo` was run without `--locked` and used the
worktree's own ignored `target/` directory; `Cargo.lock` was not modified and `git status`
stayed clean. The thinking level used by the delivering session cannot be verified from the
repository, and the model attribution rests on the commit trailer alone. Nothing in the
neighbouring YMP-118 work was reviewed or relied upon.

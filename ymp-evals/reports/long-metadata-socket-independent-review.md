# Long metadata socket independent review

13/09 05:17 HKT, 2026-09-13 (2026-09-12T21:17:45Z) — **ACCEPT, 9/10** for exact candidate `af6a4dc8e73bf6a203a96785742c65a89b895824`. No blocker found in the bounded YMP-128 correction. The inherited workspace-policy and Opus UI commits remain separately accepted inputs; this review does not reassess UI implementation or declare the release complete.

The complete socket correction diff and evidence were read. `Binding` now separates the actual metadata socket path from its connectable address. It canonicalizes the metadata directory and returns an absolute address. A pathname that exceeds the OS Unix-socket representation uses a private short `/tmp` directory whose `run` symlink points to the original metadata directory. The socket inode remains under the selected metadata home; no cwd change, source copy, shortened metadata location or new credential store is introduced.

The alias directory is explicitly mode 0700 and the actual socket mode 0600. A successful bind records its actual device/inode. Failed binding leaves ownership absent, so its destructor cannot delete an existing listener. Normal cleanup removes the filesystem entry only when the recorded device/inode still matches, then the temporary directory drops its alias. The replacement-listener control keeps the earlier listener alive, replaces its pathname, drops the old binding, and proves the new listener remains reachable. This is ordinary owned-entry cleanup, not a claim of protection against arbitrary concurrent same-user filesystem tampering.

TeamServer retains the binding for its lifetime. Its existing Drop still cancels acceptance and revokes active grants before fields are dropped. Admitted/finish methods are byte-identical to the parent; the request handler, catalog and internal stdio bridge body are unchanged. The entire public MCP facade is unchanged. Moving the existing tempfile dependency from dev-only to runtime matches its new alias ownership use; no new package version or protocol was introduced.

## Independent controls

All commands below independently exited **0** in the exact candidate, with `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0` and `CARGO_BUILD_JOBS=4`:

| Command | Result |
| --- | --- |
| `cargo test -p ymp-runtime mcp::socket::tests -- --nocapture` | Four real socket tests pass: short path, two independent long aliases, failed bind and replaced pathname |
| `cargo test -p ymp-runtime mcp::tests -- --nocapture` | 21 matching existing MCP controls pass, including grant lifetime, scope, replay and server shutdown behavior |
| `cargo test -p ymp-cli --test long_metadata_home -- --nocapture` | Real executable demo passes with the deliberately long metadata home and selected work directory |
| `git diff --check` | Passed |

The long-alias controls perform round trips, verify permissions and absolute/canonical mapping, drop one binding while continuing to use the other, and preserve metadata while removing owned socket/alias entries. The executable regression retains the long home, produces the requested greeting only in the selected work directory, retains the metadata sentinel/database, creates no Git repository and leaves no socket behind.

Independent logs and exact command results are `/tmp/ymp128-independent-{sockets,team,executable}.log` and `/tmp/ymp128-independent-checks.json`. Their SHA-256 values, in that order, are `5108bee317406c071b6ac004445dc31edfbb6cd77b77007ead30c78c0138163f`, `550c555b8a8024766fa4f2007ea2497f159d0c1c46f6412f05e78850a64f0b1a`, and `d4952f8986b1708d7380a48da8db7bd545ce8b8cc55370196f774a585d98251b`.

## Discriminating prior evidence and exact source

The original installer/demo log and the new regression's pre-correction log both contain the real `path must be shorter than SUN_LEN` failure. The author's rerun in the same original long-home setup succeeds; that setup was not shortened to pass. This review independently ran the permanent executable regression, not the full installer workflow or the original retained home.

The author also removed the inode ownership comparison and observed exit **101** when the replacement listener became unreachable with NotFound. This is a meaningful cleanup guard control: it catches deletion of another live binding's pathname, rather than merely checking a cleanup counter. The log was inspected; the mutation was not independently reintroduced into the read-only candidate.

All **125 source files** independently match their exact HEAD blobs and `/tmp/ymp128-final/source-files.json`; that manifest's SHA-256 is `87248fdcfae803db2806c39c36d565dac2441103038dc7e142f1a48e7f72f75e`. All four final formatting/Clippy/workspace/build log hashes match the candidate ledger. The workspace log's passing summaries total **382 tests**, matching the author record. Those are exact-source author full checks; a duplicate complete suite was unnecessary after the 26 focused controls passed.

## Limits and handoff

The alias mechanism is Unix-specific, consistent with the existing Unix transport. Normal teardown and setup failures are covered; abrupt termination may leave stale sockets or short aliases, as explicitly documented. No crash sweep, arbitrary filesystem adversary isolation or SIGKILL cleanup guarantee is added.

No source, main, UI, intent, registry or real application home was edited. No credential access or native inference occurred; temporary test homes and deterministic mock execution provide the evidence. Only this report was added and no commit was made. The parent still needs to repeat the complete installer/package workflow on the accepted final composition; YMP-126 reservation work remains outside this verdict.

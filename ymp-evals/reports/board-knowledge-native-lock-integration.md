# Board, corrected knowledge and native lock composition

13/09 04:05 HKT, 2026-09-13 (2026-09-12T20:05:58Z). This isolated composition is ready for independent review. **It is integration preparation, not an independent acceptance.** The integrator previously reviewed `afede54`; the parent explicitly assigned a new integration role for this work. Another reviewer or the parent must assess this new composition.

The worktree starts from main `d1ca3c94f42e04a2366efc3bd1f7e48a620c57b9`. Only the requested commits were cherry-picked:

| Supplied source | Composition mapping |
| --- | --- |
| Native explicit lock release `54153929584d849866e95a7695d432d227a03914` | `d07d044` |
| Board initial `23298acdbd3580c2e247eeab41ddb9e626f94284` | `070063c` |
| Board R2 `52345d53fb926ede998d077f325fa5952de54e84` | `2b66c1d` |
| Bound knowledge correction `187b67c7ae33ff93ae9d05e4c739aa063c8a6a28` | `82228e4` |
| Accepted board/knowledge composition `afede54a1523d5d300f0b4bfd7103a40ed94b404` | `da6c975` |

The parent supplied native-lock R4 ACCEPT 9/10 for the exact `541539` source during preparation. Newer main review/status/audit documents are intentionally left for the parent's merge; existing documents in this source were not rewritten to imitate that later main state. The copied [board/knowledge independent report](board-and-knowledge-independent-review.md) is byte-identical to the report accepting only `afede54`.

## Mechanical resolution and preservation

The only textual conflict was adjacent `Config` declarations: both the runtime-owned native scan snapshot and trusted knowledge applicability map are retained with their original serde behavior. The later compiler check identified one CLI native-catalog fixture requiring the newly optional `knowledge_correction: None` field. That fixture default is the only code edit after the requested picks. No semantic conflict or additional production correction was introduced.

The native catalog implementation and configuration-lock ownership section match supplied `541539` byte for byte. Board preparation/storage, knowledge correction/storage, internal tool catalog and authority implementation match the accepted board/knowledge source. The public MCP transport and StoreLock sections match the selected main base. Thus native names and settings continue to come from captured native scans, board proposals remain runtime-validated, corrected knowledge keeps exact captured authority, and existing execution/grant/workspace checks remain in place.

No UI source, AGENTS.md, main file, task registry, owner-request file or intent was edited. The parent owns the concurrent real configuration refresh; this assignment did not read credentials, initiate native inference or alter the real application home.

## Final validation

All final commands exited **0** on one stable source fingerprint:

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | **327 tests passed**, including both explicit configuration/scan lock-release regressions |
| `cargo test -p ymp-runtime --test board_coordination --test knowledge_correction -- --nocapture` | **17 tests passed**, including the joint correction/board/failure/retrieval consumer |
| `cargo build -p ymp-cli --bin ymp` | Passed |
| `git diff --check` | Passed |

The final run used `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`, and `CARGO_BUILD_JOBS=2` in this worktree's own target directory. These controls reduce disposable build artifacts and preserve normal test assertions. No sibling build cache was cleaned or modified.

The initial Clippy compilation failed solely because `ymp-cli/tests/native_catalog.rs` omitted the added optional contract field. Its complete output and pre-fix fingerprint are preserved under `board-knowledge-native-lock-checks/initial` beside the worktree. The fixture correction preceded all final checks. This was a compiler compatibility finding, not a claimed behavioral failing-before control.

The final source fingerprint is `8f9014cb1cf012ba1b46ef4bb882f84d1a524178e0f9c28389fd2e59c821b29c`. The [evidence ledger](../../ymp-docs/research/evidence/board-knowledge-native-lock-integration.json) contains every source-file hash, full pick identities, environment, command timestamps/statuses, raw log paths/hashes, preserved source comparisons and executable digest. No source bytes changed during or after final validation.

## Retained earlier exact-source evidence

The earlier independent report noted missing raw-log paths. The parent subsequently supplied them, and this integration assignment verified every log against the accepted `afede54` ledger. That earlier complete suite totals **316 tests** and its SDK walk reports **331 calls**. These are prior-source results, not a new full SDK run on this composition.

All paths below are under `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/`:

- `board-knowledge-fmt.log`
- `board-knowledge-clippy.log`
- `board-knowledge-workspace-tests.log`
- `board-knowledge-executable-build.log`
- `board-knowledge-actual-mcp-sdk.log`
- `board-knowledge-checks.json`
- `board-knowledge-source-fingerprints.json`

Their full absolute paths and hashes are retained in the new ledger. Existing source, reports, negative controls and external logs remain intact.

Independent composition acceptance and delegated UI acceptance remain outstanding. Direct selected-directory execution remains the approved 0.4.0 MVP limitation, without a new isolation, rollback or publication claim.

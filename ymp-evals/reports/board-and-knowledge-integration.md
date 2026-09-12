# Board and knowledge backend integration

2026-09-12T19:46:43Z. Combined source starts at main `a18f37a60990e0ef318482e15778415752fdcc4e`, preserving accepted executable contracts, MCP output lifecycle and StoreLock behavior. Only the three accepted backend commits below were cherry-picked. No earlier branch ancestry or YMP-127 discovery changes were imported.

| Accepted source | Combined mapping |
| --- | --- |
| YMP-112 initial `92691607c2652c9fcc913f658e5971509c4dab74` | `23298acdbd3580c2e247eeab41ddb9e626f94284` |
| YMP-112 R2 `154ed3f7a3d16b3c07c81182cfb91aecc6e84cda` | `52345d53fb926ede998d077f325fa5952de54e84` |
| YMP-114 `6ed96c808090ff748764e619ad8118b411c5159f` | `187b67c7ae33ff93ae9d05e4c739aa063c8a6a28` |

The copied [YMP-112 independent R1/R2 review](ymp-112-independent-review.md) and [YMP-114 independent acceptance](ymp-114-independent-review.md) match their supplied files byte for byte. They accept the individual backends; independent acceptance of this combined candidate belongs to the parent review.

## Integration changes

Two textual conflicts overlapped the internal tool catalog and argument allowlist. Both changes are retained: `board_read` and versioned `task_propose`, together with bounded scoped `memory_search`. The accepted board fixture now supplies `knowledge_correction: None` for the newly optional contract field. Formatting and the joint consumer are the remaining source changes after the picks. No additional production behavior correction was necessary, so no failing-before integration result is claimed. Existing backend controls and their genuine before/after evidence remain retained.

Nine board/correction subsystem files match the accepted author commits exactly. The public MCP transport body from `serve` onward and the storage body containing StoreLock match main exactly. Provider adapters, bridges, workspace behavior, native validation, UI, AGENTS.md, task registry and intent have no integration diff. Source hashes, complete pick mappings, binary digest and validation-log hashes are recorded in [board-knowledge-integration.json](../../ymp-docs/research/evidence/board-knowledge-integration.json).

## Joint public consumer

The [new consumer](../../ymp-rust/crates/ymp-runtime/tests/knowledge_correction/board_integration.rs) runs actual Engine sessions, live internal team calls, trusted file checks and Store reopening. It starts from the existing real CSV-backed confirmed O04/Hill/2026-W36 value of 95. The next session captures its correction obligation and then:

1. Assigns the correction to agent `a` through the live shared board.
2. Adds an approach, dependency and check while preserving that responsibility and the exact captured contract. An attempted hidden correction-authority override is rejected without adding a proposal.
3. Explicitly reassigns the correction and a sibling to agent `b`. The width-two wave executes the sibling and records `commitment_busy` for the correction, keeping its owner and work intact.
4. Produces and independently confirms 60 against the actual corrected CSV. The reviewer sees the revised task, exact correction relation and captured predecessor. The runtime supersedes 95 and activates 60 atomically.
5. Retrieves the supported correction through scoped team search, rejects another site's scope, then fails after a separate admitted side effect.

After reopening, the original acceptance/source records remain identical, old knowledge is superseded, and the corrected entry is available with producer `b`. Completed assignments have revoked grants. Exactly two qualified observations remain across the source and correction sessions; correction and observation replay add none. A subsequent real Engine prompt receives 60 and excludes the superseded entry. The fixture writes no acceptance journals directly and uses only mock/scripted agents pinned to low effort.

## Validation

Targeted checks first passed all 11 board cases and five existing correction cases. The new joint case then passed on the combined production source. Final validation passed:

- `cargo fmt --all --check`.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test --workspace`: **316 tests**, plus doc tests. This includes the actual scoped CLI/public-stdio correction consumer.
- `cargo build -p ymp-cli --bin ymp`.
- Full official Python MCP SDK walk against that executable: **331 tool calls**, largest reply **73,149 bytes**. Configured contracts, immutable resume, supported retrieval, authority rejection, bounds, reconnect/recovery, signal cancellation, undrained output, final EOF frames and broken-pipe cleanup all pass.

Builds used the existing sibling `build114-final` cache exclusively after its prior review completed. Reuse exposed a missing generated SQLite build artifact; the cache was fully cleaned once and final checks rebuilt from this source. No production patch was made for that cache failure.

This is a backend integration candidate. The parent owns final combined review and integration; delegated TUI visibility and the separate native-scanned agent pool remain outside this assignment. No native inference or credential access was used.

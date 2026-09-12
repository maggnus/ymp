# Board and knowledge integration independent review

13/09 03:55 HKT, 2026-09-13 (2026-09-12T19:55:07Z) — **ACCEPT R1(9/10)** for exact combined HEAD `afede54a1523d5d300f0b4bfd7103a40ed94b404`. Code **9/10**; evidence **9/10**; public consumer **9/10**. No backend integration blocker was found. Individual and combined backend acceptance does not complete the separately delegated UI work or native-pool correction.

The review read the updated `AGENTS.md`, integration report/evidence ledger, accepted YMP-112 R1/R2 and YMP-114 reviews, correction contract, merged authority/catalog changes, runtime/storage transition paths and full joint consumer. Source remained read-only.

## Source composition and protected boundaries

The base is `a18f37a60990e0ef318482e15778415752fdcc4e`. Accepted YMP-112 `92691607c2652c9fcc913f658e5971509c4dab74` and `154ed3f7a3d16b3c07c81182cfb91aecc6e84cda` map to `23298acdbd3580c2e247eeab41ddb9e626f94284` and `52345d53fb926ede998d077f325fa5952de54e84`. Accepted YMP-114 `6ed96c808090ff748764e619ad8118b411c5159f` maps to `187b67c7ae33ff93ae9d05e4c739aa063c8a6a28`.

All **100 source-file hashes** independently match the ledger. The independently recomputed aggregate source hash is `6a640db866950e5b891ca7f8c5f035c60766f00da9b01367a58bafdaa34f8faf`. All nine identified board/correction modules match their accepted source commits byte for byte. The corrected board selection/committed-executor region matches YMP-112 R2; the complete incremental retention/correction/observation region matches accepted YMP-114.

The two conflict resolutions preserve both changes: actual board operations and versioned task proposals remain in the internal catalog/allowlist, while scoped memory search retains bounded scope validation and the existing response projection. The only storage-authority diff after the picks is formatting. The other post-pick changes are the absent optional correction fixture field, joint test and evidence/review files. No new integration behavior correction was required.

The public MCP body from `serve` onward and StoreLock section independently match main and their recorded hashes. Runtime workspace coordination, its exclusive verification wrapper, storage budget implementation and provider access declaration also match main. No UI, native discovery, bridge or provider implementation was imported or changed by this integration.

## Joint public outcome independently reproduced

The new Engine/socket consumer passes within the independently rerun correction suite. Its starting value 95 and correction value 60 are checked against actual CSV rows by the captured Python verifier; they are not accepted from model agreement alone. Every scripted backend rejects non-Mock providers and requires the low-effort fixture setting.

The correction session captures its trusted predecessor/version, applicability, source replacement and correction criteria before planning. Real admitted board participants assign the future correction to `a`, append an approach/dependency/check while preserving that responsibility, and explicitly reassign it to `b`. The board rejects a hidden `knowledge_correction` override without persisting a proposal. Both the correction and its ready sibling are committed to `b`; the scheduler records `commitment_busy`, executes the sibling, then performs the correction without losing its owner or demanding reassignment.

The final task definition includes the board approach, while the captured acceptance contract and digest remain unchanged. The actual reviewer prompt includes the revised task, predecessor ID and captured correction relation. Independent review plus current criterion evidence confirms 60, and the correction transaction supersedes 95 while activating the new projection. The stored producer is `b`.

A later admitted assignment retrieves 60 through scoped internal `memory_search`, obtains no result for the different Harbor scope, then writes a separate uncertain-effect file and fails. The session remains honestly blocked. The correction survives that failure: reopening returns the old claim as Superseded and the new claim as Available, preserves the original acceptance record, and shows ended assignments with revoked grants. Exactly two supported observations exist across the original and corrected source outcomes. Observation replay and correction replay add no credit; the latter returns AlreadyApplied.

A subsequent actual Engine invocation receives the corrected entry and `"value":60` in its provider prompt while excluding the superseded ID. This proves delivery through the combined runtime, beyond persisted inventory. The fixture creates no acceptance or correction journals directly.

## Shared authority, versions and lease lifetime

Board proposals remain bound by the live team-call transaction to their session, agent, assignment, invocation and grant. The merged scope argument cannot select another project or install correction authority. Unknown board fields are rejected; the typed ordering policy continues to select proposal IDs without bypassing runtime preparation or transactional version checks.

Board revisions are additive and restricted to Ready tasks. The current responsibility generation participates in task versioning; plan and membership changes retain their separate digests. The unchanged claim transaction checks the exact task version/definition, responsible agent, membership, reviewer reservation and busy state before selecting execution. Pins, budget admission and a fresh invocation grant still apply after commitment. The passing board suite retains conflicting claims, stale plan/task/team denial, reverse-policy selection and uncertainty inspection, including the R2 busy-owner and late-selection-error regressions.

A board proposal cannot rewrite the captured correction target, original applicability, criterion coverage or source snapshots. Corrections resolve the exact predecessor and accepted source; replacement activation requires the completed independent review to carry the same result and correction-context digest. Actual passing evidence is rechecked inside the correction transaction, which writes predecessor lifecycle, replacement link/state and receipt together. Stale, conflicting, cyclic, unrelated or self-reviewed replacements remain denied. The focused correction tests exercise those existing boundaries on this combined source.

The unchanged production lease lasts through candidate snapshotting. The exclusive verification lease lasts through the revised result's checks, review, acceptance, knowledge retention, correction transaction and ordinary supported observation. Board metadata changes introduce no alternative acceptance path or filesystem permission. Scoped/default/ID retrieval continues to resolve source freshness and excludes superseded or explicitly replaced sources; historical predecessor context remains explicitly labelled and separately captured for review.

## Independent commands and validation evidence

All independently run commands below exit **0**:

```sh
CARGO_TARGET_DIR=../build114-final cargo test -p ymp-runtime --test board_coordination --test knowledge_correction -- --nocapture
CARGO_TARGET_DIR=../build114-final cargo test -p ymp-cli --test acceptance_contracts executable_correction_contract_scope_and_public_mcp_history_are_reachable -- --nocapture
git diff --check
```

The first command passes **17 tests**: eleven board cases and six correction cases including the joint run. Log `/tmp/ymp-board-knowledge-independent-focused.log` has SHA-256 `8ec329bba98e3e7ba8a866362f78aa87529e9bacdb60c3532c283620f2a96a69`. The second passes the real executable/scoped CLI/public-stdio history consumer; log `/tmp/ymp-board-knowledge-independent-public.log` has SHA-256 `db80b607a94c7471c6226b426171413e777746c7e2c380dce3be109975348536`.

The integration ledger records formatting, denied-warning workspace Clippy, **316 workspace tests**, executable build and the full official SDK walk as passing on its source fingerprint. The embedded SDK output independently hashes to its recorded value `8375743df195b4755f27ce895169fdcab1caabdd1d91dab7d58bc4b96d9a6629` and reports **331 calls**, largest reply **73,149 bytes**, with the stated lifecycle, authority, contract, scope and recovery controls. The actual executable independently matches the recorded SHA-256 `a671a23da36350275c08b7fa4e51ef335ff3439758533ed811f7c1d31cb67604`, both before and after focused verification.

Those complete checks are author-run evidence, not a newly claimed reviewer whole-suite or full SDK execution. The ledger supplies source/binary/log hashes and SDK output but does not name raw paths for the other four logs; their recorded results are distinguished from the focused commands independently executed here. No redundant complete suite was needed after verifying source identity and the relevant actual consumers.

There is no additional production correction in this composition, so this review does not invent an integration failing-before result. Existing accepted backend controls and R1/R2 failure evidence remain applicable to the byte-identical implementations; no source mutation was performed in this review.

## Acceptance boundary

No inference, credentials, main edits, task-registry changes, intent edits, UI work or commit occurred. The only review addition is this report. The separate YMP-127 native-scanned naming and configuration-lock work remains outside this source and verdict. The inherited direct-directory policy is the approved MVP limitation, not a native narrow-path sandbox, external-effect recovery or rollback promise. These checks establish deterministic backend behavior and evidence preservation, not model quality, strategy optimality or the complete release.

Ledger: `R1(9/10) ACCEPT 13/09 03:55 HKT — board revision/reassignment and busy-owner scheduling preserve exact captured correction authority; confirmed supersession survives later failure and reaches scoped lookup and the next actual prompt; focused public consumers pass and source/executable evidence matches; no backend integration blocker found`.

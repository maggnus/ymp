# YMP-125 independent implementation review

Verdict: **ACCEPT — 9/10**. No blocking finding within YMP-125. The original executable confirmation gap is closed on the reviewed candidate: a user-authored TOML contract can produce confirmed output, one evidence-backed producer observation, and supported retrieval in a later executable session. The interface retains the existing confirmation authority rather than interpreting model-authored checks as trusted evidence.

Reviewed revision: `ef9a1254696baef5b1bdbf72a9c1745da3eacc37`, based on `3145f5f8bdfb57366c4065d077b761de7e9604e0`. Review started at `2026-09-12 17:15:15 UTC`; validation completed after the `17:19:25 UTC` clock check. The checkout was clean before this report. No source, UI, task-register or intent changes were made by this review.

## Acceptance assessment

| YMP-125 requirement | Assessment and evidence |
| --- | --- |
| Explicit typed executable input with clear activation | Met. `Config.acceptance_contracts: Option<Vec<AcceptanceContract>>` is loaded through the normal configuration path (`ymp-core/src/config.rs:89`). CLI and TUI pass this same configuration to the runtime. `requested_contracts` resolves configured or compiled-client authority and rejects conflicting sets (`ymp-runtime/src/engine/confirmation.rs:4`). Documentation explicitly states that the set applies to every new team session using that application home, including a new task started through conversation. Demo forwards it (`ymp-cli/src/main.rs:322`). A dedicated `--home` is the documented way to select workflow-specific activation. |
| Communicate requirements and enforce unique targets, including revisions | Met. The typed `AcceptanceRequirements` projection contains only title, criteria and artifact/input paths (`ymp-core/src/confirmation.rs:43`). It contains no assertions or expected/verifier bytes. The projection comes from captured records and reaches the actual provider prompt (`ymp-runtime/src/engine.rs:970`). Every initial/revised plan is checked before recording or production (`engine.rs:1398`); saved bindings are checked before resumed inspection and at scheduling boundaries (`engine.rs:1489`, `1505`). Missing or ambiguous targets block explicitly. Duplicate contract titles fail before session creation. |
| Capture before prompts; preserve authority on resume | Met. All selected contracts are prepared before session creation and committed with the session/policy in one transaction (`engine.rs:567`, `617`; `ymp-storage/src/provenance.rs:284`). Storage independently validates each capture. Explicit replacement, removal, or addition on resume is rejected before another invocation (`engine/confirmation.rs:48`). Omission uses the captured set; it does not recapture initial inputs/verifier contents or remove obligations. Target order may change, while complete contract definitions remain equal. |
| Actual executable confirmation and later supported retrieval; honest qualitative fallback | Met. Both the five CLI integration tests and the independent documented-config consumer passed. The independent run returned confirmed task/final acceptances, exact greeting bytes and one observation. A distinct later session retrieved a confirmed entry naming the source session before its deliberately failed planning response. Absent and partially covered contracts remain unconfirmed without credit; a failing applicable contract blocks. |
| Failure, partial coverage, drift, binding and resume controls | Met. Seven focused runtime ingress tests passed, including initial/revised missing or duplicate targets, changed saved bindings, sanitized provider prompts, changed inputs/verifiers on resume, and input/verifier/artifact changes during production or review. The atomic-storage test passed both invalid-second-contract and duplicate-second-contract rollback cases. The independent executable consumer additionally changed an accepted artifact: resume blocked with a fresh-inspection requirement, an invalidation record was preserved, historical credit did not duplicate, and subsequent supported retrieval excluded the stale source. |
| Shared CLI/TUI records; no new UI or confirmation semantics | Met. No TUI source changes are present. Existing TUI construction uses `Engine::new` with the same loaded configuration, and configuration serialization preserves the optional field. Existing acceptance, evidence, observation and knowledge records remain the consumers. No permanent role, plugin framework, service or semantic-trust mechanism was introduced. |

## Independent executable consumer

The consumer extracts the TOML contract directly from `ymp-docs/architecture/executable-acceptance-contracts.md`, adds two built-in mock agents with explicit `low` effort, and uses the actual `ymp` executable. All task artifacts are in a fresh selected directory, with configuration/state outside it. No credentials or native model providers were read or invoked.

- Reproduction script: `/tmp/ymp125-review-consumer.py`.
- Complete command ledger, exit codes, stdout/stderr and raw traces: `/tmp/y125-r-_9u05e0w/` (`result.json` records every exact executable argument array).
- Source session: `7b15fb83-671e-4f06-87c8-569c4284e6f3`.
- Later retrieval session: `dcdcd0fc-0617-4b76-8d23-2f47c6c5e976`.
- Executable: `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp125-target/debug/ymp`.
- Executable SHA-256: `1517ab6e7cd4a4d87ab09b6e9781e89c9d9fd522752cb983a5c40f8fbb9491dd`.

The consumer checks confirmed output and requested low effort, later source-linked retrieval, byte-for-byte unchanged source trace after rejected configuration replacement, capture-only resume, artifact invalidation, no duplicate observation, and exclusion of the stale source from a later supported retrieval.

The first consumer script expected the literal word `unconfirmed` in the stale-artifact CLI message. The executable correctly returned exit 1 and `Accepted artifact version changed; result requires fresh inspection and review`. This was a harness wording assumption, not a failed runtime guard. The corrected assertion checks that actual stale-result diagnostic and journal/retrieval consequences. The first script/log remain at `/tmp/ymp125-review-consumer-initial.py` and `/tmp/ymp125-review-consumer-initial.log`; the corrected consumer passed without source changes.

## Commands and validation evidence

Commands ran from the reviewed checkout. These are the exact build environment values used for Cargo checks:

```sh
export CARGO_TARGET_DIR=/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp125-target
export CARGO_BUILD_JOBS=3
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0

cargo test -p ymp-cli --test acceptance_contracts -- --nocapture > /tmp/ymp125-review-cli.log 2>&1
cargo test -p ymp-runtime contract_ingress_tests -- --nocapture > /tmp/ymp125-review-runtime.log 2>&1
cargo test -p ymp-storage session_contract_capture_rolls_back_the_whole_set_on_second_contract_failure -- --nocapture > /tmp/ymp125-review-storage.log 2>&1
python3 /tmp/ymp125-review-consumer.py > /tmp/ymp125-review-consumer.log 2>&1
cargo fmt --all --check > /tmp/ymp125-review-fmt.log 2>&1
cargo clippy --workspace --all-targets -- -D warnings > /tmp/ymp125-review-clippy.log 2>&1
```

All corrected commands exited 0. The independently rerun focused tests passed **5 CLI + 7 runtime + 1 storage** tests. Formatting and workspace/all-target clippy passed. The CLI Cargo invocation reused the current candidate's executable artifacts; the runtime/storage invocations rebuilt their required feature combinations from this checkout.

| Retained independent evidence | SHA-256 |
| --- | --- |
| `/tmp/ymp125-review-cli.log` | `e8747a75debd47901434a78e0db71d287f047edbf6272d960756d24c00e4716c` |
| `/tmp/ymp125-review-runtime.log` | `4b561ae6bee9ad33f306e9001daf42238aa0e8606175855974be662c2ab2eb25` |
| `/tmp/ymp125-review-storage.log` | `32b11f1dc9b83ec5313a9f2b806b75c09f193ea82b5d54807b3e9a507c88a9a4` |
| `/tmp/ymp125-review-consumer.log` | `0f70a7ebaf7ac5918c219e218a4cd83c8a6ee454ce7d41a6ea46aeef0b42a33e` |
| `/tmp/ymp125-review-consumer.py` | `c4929b0017e266ad41e6d0ad927ba5657f8510ba3b53fc31d70f77353163d542` |

The author's full workspace test log at `/tmp/ymp125-workspace-tests.log` was read, counted as **280 passed / 0 failed**, and its SHA-256 independently matched `e7f24ef2e2a9de257eb2d910578e467698e479db446e4491010b5c525dbf619d` recorded in `ymp-docs/research/evidence/executable-contract-ingress.json`. Both retained author CLI trace digests also matched that evidence record. A second full workspace suite was unnecessary after the exact-source focused controls passed.

The recorded old-source positive control fails with unconfirmed instead of confirmed acceptance. The retained author mutation evidence `/tmp/ymp125-mutation-controls.json` contains concrete failing assertions for removed binding validation, removed resume replacement validation, and removed requirements context (each exit 101). Those destructive source controls were inspected, not repeated in this read-only implementation review. Their unchanged corresponding tests passed independently on the candidate.

## Boundaries and integration disposition

This acceptance establishes executable ingress and authority preservation, not model quality, resource savings, OS filesystem isolation, or whole-release acceptance. Program meaning and declared verifier dependencies remain a trusted user/client responsibility; model-authored commands do not become trusted contracts. Configuration-wide activation is deliberate and documented, rather than an implicit project-file discovery rule.

Captured output locations are now available for declared artifacts. Results with no declared artifact contract continue to retain their original directory and report without guessed artifact identities, consistent with the existing outcome API.

The candidate predates the separately accepted YMP-115/MCP/StoreLock changes identified in the review assignment. They are outside this review; integration must retain their fixes and rerun required checks on the combined source. The original executable gap and all six YMP-125 acceptance items are satisfied on this exact candidate. Final release acceptance remains YMP-121 work.

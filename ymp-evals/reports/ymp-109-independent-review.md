# YMP-109 independent review: provider-backed agent pool

**Verdict: ACCEPT. R1 = 9/10.**

Reviewed commit `254db24229a23dc3318b5b82b1082b716811ac71` ("Expose provider-backed agent pool and native capability metadata") on branch `ymp/task-109-release` in the `109` delivery worktree. The review was read-only: no file in that worktree was edited, its build directory was not used, and it remained clean at `254db24` before and after the checks. Mutation experiments ran in a throwaway copy outside both worktrees.

Reviewer: delegated Claude session, model `claude-opus-5`, permission mode `bypassPermissions`, thinking level requested as `max` by the assignment. The model identifier and permission mode are observable inside the session; the thinking level is not independently observable from within it, so it is recorded as requested rather than as measured.

## Commands and results

All commands were run from the `109` worktree with an external build directory (`CARGO_TARGET_DIR=/tmp/ymp-review-109-target`) and `--locked`, so nothing was written into the reviewed worktree and `Cargo.lock` was not updated.

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace --locked` | exit 0, 106 tests passed, 0 failed |

Diff reviewed in full: 9 files, 888 insertions, 7 deletions. The two new test files, the two new core modules, and every changed line of `discovery.rs`, `config.rs` and `engine.rs` were read.

## Verification of the four required properties

Each property was checked against the code and then re-checked by breaking the implementation in a scratch copy and confirming that a specific test fails. A test that cannot fail proves nothing, so the failure direction was established for every claim below.

**Configured catalogs remain labeled configured.** `inspect_pool` clones `Config.capabilities` and overwrites every `source` with `CapabilitySource::Configured` before returning, so a hand-written `native_metadata` claim in `config.toml` cannot reach a consumer as observed evidence. Removing that overwrite makes `configured_metadata_cannot_claim_to_be_an_observed_native_response` fail at `discovery.rs:299`.

**Unknowns remain unknown.** A configured model that is absent from a catalog yields `PoolModelStatus::Unknown` and no exclusion unless the catalog is explicitly `models_complete`. Only then does it become `Unlisted` with a `ModelUnlisted` exclusion. A profile without a model stays `InheritedDefault` and no default is resolved on its behalf. Treating any catalog as authoritative makes `eligibility_distinguishes_disabled_missing_unknown_and_explicitly_unlisted` fail at `discovery.rs:251`.

**Discovery invokes no model and mutates no roster.** `inspect_pool` reads configuration and executable metadata only; `discover_glm_in` reads `package.json` files and never executes the resolved entry point. The delivered test first runs a synthetic sentinel provider to prove that a launch would be detected, then asserts the sentinel was not touched during save, load and pool inspection. Adding a `Command::status()` call to `inspect_pool` makes that test fail with "discovery or config inspection executed the provider". The previous `config.team.push("glm")` in GLM discovery was removed, and `cached_installation_discovery_preserves_roster_ids_and_names` asserts the roster, IDs and display names are byte-identical afterwards; restoring the push makes it fail at `discovery.rs:337`.

**Captured identity and activity views are accurate.** `SessionAgentView::from_captured` takes its participants from the stored session, not from today's configuration, so a renamed or deleted profile cannot rewrite history; `captured_name` resolves chat and task authorship against that capture. `None` for `current_members` and `active_invocations` means unknown rather than empty, and the runtime integration test asserts that saved native continuation keys and durable usage rows do not become live activity after a restart. Foreign, duplicate, non-participant and non-member invocation references are rejected; loosening that check makes `live_activity_requires_bound_distinct_invocations_for_current_members` fail at `pool_metadata.rs:176`.

The runtime test also confirms the separate-context requirement against real engine behavior: two profiles sharing one provider and model receive distinct continuation keys of the form built at `engine.rs:345`, independent per-agent usage rows, distinct task assignee and reviewer, and their own board authorship. It uses the in-process mock provider, which returns before any process is spawned (`ymp-providers/src/lib.rs:55`), so no real provider request is made.

## Findings

None of the following blocks acceptance.

1. **First-run roster composition changed.** With a cached GLM installation present, the bootstrap in `ymp-cli/src/main.rs:115` previously added GLM to the starting roster; it now enables the provider and profile but leaves the roster at Codex and Claude. This is the intended pool-versus-roster separation, it is documented in `ymp-docs/architecture/agent-pool.md`, and the user can still add GLM from the team view. It is recorded here because it changes observable first-run behavior and belongs in the release notes.
2. **An unverified native claim can still be stored.** `Config::validate` accepts `source = native_metadata` in `config.toml` as long as the method is nonempty and the timestamp is RFC3339. Only the pool projection sanitizes it, so any future consumer that reads `Config.capabilities` directly would see the claim verbatim. Routing all presentation through the pool projection, or rejecting the variant until a real adapter exists, would close that gap.
3. **Dynamic membership will need a wider capture.** `from_captured` requires every current member to be a captured participant. Once YMP-110 admits an agent mid-session, the session record must be extended at the same time or the view will fail rather than report the new member.
4. **Mock providers now report as installed.** `inspect` treats `ProviderKind::Mock` as available without an executable. This is accurate because mock turns never spawn a process, and the default configuration contains no mock provider, so only hand-edited configurations see the change.

## Limits of this review

No real provider request, authentication check or quota check was made, so nothing here establishes actual model availability for any installation. The backend pool API has no consumer yet; presentation is tracked in YMP-118, and this review does not assess it. The review covers the single commit named above and not the branch's earlier history.

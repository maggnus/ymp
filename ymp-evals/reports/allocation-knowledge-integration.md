# Allocation and knowledge integration

13/09 00:08 HKT, 2026-09-13 — Combined YMP-110/YMP-113 backend prepared for independent integration review. The source base is clean MAIN `2695d69`; MAIN was not edited. Acceptance of the separate outcomes is preserved, while this combined result still requires the parent's independent review.

## Exact commit mapping

Only the four requested outcome commits were cherry-picked, in order:

| Accepted source | Combined mapping |
| --- | --- |
| `ec552bbec842928c33b257b6d385f047bb0f25c1` | `22f47aa` |
| `535b6193913057eb7d808bfadf3f5aeb0b944e05` | `388f099` |
| `0bc18b0ba8389da4f56dee695b65f389174ca90e` | `61b6e14` |
| `80017df` | `e1a1519` |

Parent execution-identity corrections were already present in the base and were not duplicated. The only cherry-pick conflicts were adjacent module exports, optional learning and adjacent test insertion. Both allocation and knowledge modules/defaults remain present. Optional learning uses YMP-113's candidate-only persistence and YMP-110's eligible-agent selection. Both original tests were retained. Current-membership admission, protected reviewer eligibility, resource limits, grants, confirmation and shared execution-version validation were preserved.

## Integration defect and correction

The joint public consumer found an actual location-route defect in the combined code: after `Store.relocate_project` changed only the association, YMP-110's existing literal location-question handler opened prior workspace metadata against the new directory. `Engine.follow_up` failed with `Stored workspace belongs to a different working directory` before it could answer. Its old path assembly also joined captured relative paths onto the selected directory instead of using YMP-113's immutable outcome locations.

The handler now reads `Store.outcomes` before opening an execution workspace. Recorded absolute artifact paths therefore survive association relocation and require no inference or production. When no artifact paths were captured, the existing file-inspection fallback reads stored workspace metadata and labels files as current contents of that recorded directory. It does not initialize a new workspace for this question. Other conversation/execution behavior retains the original workspace checks. No new phrase router, filesystem isolation policy or TUI behavior is introduced.

`cargo test -p ymp-runtime --test state_integration -- --nocapture` exited **101** with the final joint test against combined baseline `e1a1519`, then **0** after restoring the correction. The retained failure and passing outputs are `/tmp/ymp-state-integration-failure-before.log` and `/tmp/ymp-state-integration-after.log`. The failing assertion was the public `follow_up` call, after the membership, grant, reputation and later-session knowledge controls had passed. The temporary baseline restoration was reverted immediately; it is not part of the final source.

## Joint public consumer

`tests/state_integration.rs` uses public `Engine`, `Store`, `TeamServer`, policy injection and actual captured backend requests. Its backend explicitly requires `ProviderKind::Mock`, forwards ordinary work to the existing mock adapter, fails only the later sibling, and later fails a separate session's planner after capturing its actual prompt.

The consumer demonstrates all of the following in one stored history:

- A first task receives exact-byte confirmation and produces retrievable knowledge before a later sibling fails. No final learning assignment runs and exactly one supported competence observation remains.
- An injected allocation policy replaces the idle producing agent under fixed size two. The current membership changes, the initial policy and spend stay intact, and all three captured names remain available through `SessionAgentView`, including the retired producer.
- Both explicit and reserved-ordinal public admission reject the retired producer and the reserved final reviewer. Each rejection preserves the complete trace and issues no grant. The ordinary current executor is admitted through both wrappers; two synthetic failed controls add exactly two admitted calls and no competence credit.
- The original producer's evidence-linked knowledge stays available after replacement and storage reopening. A later session receives its exact source ID/session/version, and the actual provider prompt contains the retained greeting. Proposed lessons remain absent from supported retrieval by entry identity; lexical matches in independently supported check descriptions are not misclassified as candidate activation.
- Metadata-only relocation preserves the complete original `StoredOutcome`. The public location-question handler returns its original existing path with zero new invocations/backend requests; it creates no artifact in the new association. The original file bytes and sole observation ID remain unchanged.

This is protocol and integration evidence. It does not establish model quality, team superiority, memory efficiency or production isolation.

## Accepted probes and final checks

All final commands below exited **0**:

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | Formatting passed. |
| `cargo clippy --workspace --all-targets -- -D warnings` | All workspace targets passed with warnings denied. |
| `cargo test --workspace` | **267 tests passed**, including all 13 permanent public allocation-admission controls and the joint consumer; output `/tmp/ymp-state-integration-workspace.log`. |
| `cargo test -p ymp-runtime --test state_integration -- --nocapture` | Corrected joint consumer passed; source restoration control failed 101. |
| `git diff --check` | No whitespace errors. |

The accepted independent probes were copied into `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-state-probes-vo1po52l` and their path dependencies redirected to the combined checkout. Allocation probe source is unchanged. The knowledge probe only gains `team_constraints: Default::default()` for the combined `Config` shape; its assertions and workload remain unchanged.

Exact external commands:

```sh
cargo test --offline --manifest-path /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-state-probes-vo1po52l/allocation/Cargo.toml --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/integrate_state/target -- --nocapture
PROBE_DIR=/tmp/ys-state-vi4t6hw0 cargo run --offline --manifest-path /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-state-probes-vo1po52l/knowledge/Cargo.toml --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/integrate_state/target -- normal
PROBE_DIR=/tmp/ys-state-vi4t6hw0 cargo run --offline --manifest-path /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-state-probes-vo1po52l/knowledge/Cargo.toml --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/integrate_state/target -- relocate
```

The allocation command passed all **seven** accepted probes; output is `/tmp/ymp-state-allocation-probes.log`. Its inherited harness-only unused-code warnings do not affect the clean repository clippy result. Both knowledge commands exited **0**, retaining captures under `/tmp/ys-state-vi4t6hw0/{normal,relocate}`. The relocation JSON SHA-256 is `acfd2a1a653a1fd70eb2d1c879eece338e63d5d5d6401eb73000737ab14617d5`; its before/after outcome values preserve the original directory, artifact digest, source/result identities and confirmed grade. An initial longer harness data root exceeded the existing Unix-socket path limit; the shorter data root corrected that fixture setup without changing transport or probe semantics.

## DTO and UI integration notes

No DTO was introduced by the integration correction. Both accepted policy interfaces remain injectable. YMP-118 should use `SessionTrace.team_state.current_members` for membership, `Session.team`/`SessionAgentView` for captured history, and the existing invocation records for live work. The reserved reviewer is future review eligibility, not a role or permission grant. Allocation/resource decisions retain their implementation identity and rationale.

Knowledge inventory remains a historical lifecycle snapshot. Its provenance names source session/result/version, evidence and policy; `resolve_memory(..., Supported)` establishes current availability. Unknown/unconfirmed candidates remain inspectable without being labelled verified. Historical authors need not be current team members. Outcome paths are already absolute captured locations; consumers must not rejoin them to current project registration. `outcome_location_unknown` remains the explicit legacy-absence error. The location-question handler consumes these existing contracts without inference.

No UI source or fixture, intent, task registry, AGENTS.md, version/confirmation/budget/grant implementation or MCP transport was changed by the additional integration correction. Workspace behavior remains direct only for the approved 0.4.0 MVP; no isolation or publication guarantee was added. No real model inference or credentials were used. UI acceptance, independent combined review, YMP-115 concurrency, YMP-123 public MCP and YMP-124 isolation remain separate work.

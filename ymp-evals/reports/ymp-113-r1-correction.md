# YMP-113 R1 location correction

12/09 23:25 HKT, 2026-09-12 — R1 F1 correction completed for independent R2 review. The [independent R1 report](ymp-113-independent-review.md) remains unchanged. This is an author correction record, not an acceptance verdict.

The returned defect was reproduced before editing: the exact external relocation probe exited **1** with `Outcome original captured path replaced by mutable project relocation`. The new public-engine regression also exited **101**, showing that registration relocation changed the reported directory/artifact path and downgraded the grade although the original file remained intact.

`Store.outcomes` now resolves the immutable directory captured by the producing assignment, then the accepted task definition's workspace, then the captured session policy. It never derives a historical location from `Project.path`. Source freshness and confirmation use that same captured result directory; contract-level inspection uses the captured session directory. Criterion coverage, evidence identities, result versions, acceptance and reputation rules are unchanged.

The DTO shape is unchanged. A legacy outcome without any captured absolute directory returns `outcome_location_unknown`, including result/version/session identity. An existing matching file in today's project association does not supply missing historical provenance. A temporary project-path fallback was reintroduced for this control; `cargo test -p ymp-storage legacy_outcome_without_captured_directory -- --nocapture` then exited **101** because the API returned an invented location. The fallback was restored to the corrected implementation, and the test passed in final workspace validation.

## Consumer checks and exact commands

The unchanged external probe was run before and after the correction:

```sh
PROBE_DIR=/tmp/ymp113-independent-04rj1w98 cargo run --offline --manifest-path /tmp/ymp113-independent-04rj1w98/Cargo.toml --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/113/target -- relocate
PROBE_DIR=/tmp/ymp113-independent-04rj1w98 cargo run --offline --manifest-path /tmp/ymp113-independent-04rj1w98/Cargo.toml --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/113/target -- normal
```

The corrected relocation and normal runs both exited **0**. Their new captures are:

- `/tmp/ymp113-independent-04rj1w98/relocate-b4bebbeb-ac7a-49ad-9d70-ce6831394154/relocation.json`, SHA-256 `6a8cd3d3d06a200812595e7918bc53a6b872720905d2e38de607440c4cc0c962`.
- `/tmp/ymp113-independent-04rj1w98/normal-983bb6a0-95d0-4b66-b3d6-edd455aaf1bb/`, including the normal source-drift and public-memory captures.

The relocation capture has byte-equivalent outcome values before/after the association edit: original directory and artifact path, result ID/version, acceptance/source session IDs, digest, `current: true`, and `confirmation: confirmed`. Both `original_exists` and `reported_exists` are true. Source inspection starts no new invocation.

`cargo test -p ymp-runtime knowledge_outcomes_retain_captured_location -- --nocapture` changed from **101** to **0**. The regression uses public `Engine.run`, a verified first task, a deliberately failed later sibling, `relocate_project`, storage reopening and public outcome/memory consumers. It compares the complete original outcome with the post-relocation value. It then installs matching files in the new association and independently changes the original artifact and original supplied input. Each original-source change makes the same historical outcome unconfirmed/current=false and excludes its knowledge, while preserving the original paths, IDs and digests. Restoring the original bytes restores the original outcome; invocation count remains unchanged.

`cargo test -p ymp-storage legacy_outcome_without_captured_directory -- --nocapture` exited **0** with the corrected implementation. It reads legacy accepted-result storage with no directory capture, including a matching file under current project registration, and requires the explicit location-unknown error without starting an invocation.

Final required checks all exited **0**:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` — **238 passed**, no failing suite; command output retained at `/tmp/ymp113-r1-workspace.log`.
- `git diff --check`

No UI, intent, task-registry, MCP transport or native-provider code was changed. No real-provider inference or credential reads were performed. The independently observed MCP response-size issue remains assigned to YMP-123; this correction introduces no transport limit or pagination redesign.

Ledger: `R1(6/10) RETURN 12/09 23:09 → original-path/grade regression reproduced with external exit 1 and public test exit 101 → immutable source-directory resolution, explicit legacy absence, original-source drift controls and 238 passing workspace tests → independent R2 pending`.

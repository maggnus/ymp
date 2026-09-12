# Backend documentation preflight

13/09 04:22 HKT, 2026-09-13 (2026-09-12T20:22:57Z). Reviewed active English backend/user documentation against exact main `541cf56e543216d087a33a7457f91aa9be46c3cb`. The approved intent bytes match the registry digest `4479e919c5b47934e69e81ac493d16e90a0dc420a9a370382e79bca3f3c754d9`.

Five backend documentation files have narrow corrections in this isolated candidate. Source line references below describe the reviewed base. No code, historical research, approved intent, request 02, task registry, main, README or UI-owned file was edited.

| Priority | Stale active statement | Concrete correction and supporting implementation |
| --- | --- | --- |
| P1 | `architecture/agent-pool.md:5`, `:18`, `:54` preserves configured display names and calls observed native metadata future work; `architecture/providers.md:19` mentions configured offerings only | Describe separate metadata refresh and cached pool inspection, actual native names/aliases/controls, explicit unknown identities and historical captures. `ymp-providers/src/discovery.rs:62` overlays native snapshots; `ymp-core/src/config/catalog.rs:116` resolves presentation; `ymp-providers/src/discovery/catalog.rs:40` scans and publishes offerings. Fixed in candidate. |
| P1 | `architecture/storage.md:20`, `:22` describes resolved outcomes and peer review as sufficient credit/activation; `protocols/team.md:37` grants plan/reviewer credit | Require current confirmed independent acceptance and completed producing execution. Free text stays a candidate; supported global procedures come from observed passing checks. `ymp-storage/src/confirmation.rs:816`, `ymp-runtime/src/engine.rs:1883`, `ymp-storage/src/knowledge.rs:38`. Fixed in candidate. |
| P1 | `protocols/team.md:8`, `:35` describes bidding and Beta-sampled assignment | Describe current metadata selection, mean qualified experience, runtime-validated board changes and exact responsibilities. `ymp-runtime/src/allocation.rs:47`, `ymp-runtime/src/engine.rs:1646`, `:2173`; internal `board_read`/typed proposals are in `ymp-runtime/src/mcp.rs:253`. Fixed in candidate. |
| P1 | `architecture/providers.md:17` and `README.md:34` call `doctor --probe` a small request without its fan-out | State that every matching enabled profile is probed; several may share a provider. `ymp-cli/src/main.rs:220` is the actual loop. Provider guide fixed; README deliberately left for parent integration. |
| P2 | `architecture/system.md:26`, `:28`, `:32` presents unlimited-duration direct mode, sequential work/bids and current-directory follow-ups | Qualify direct mode as MVP-only; describe bounded independent waves, conflicting access, exclusive verification and original captured result paths. `ymp-runtime/src/engine.rs:1674`, `ymp-runtime/src/engine/workspace_access.rs:154`, `ymp-storage/src/confirmation.rs:29`, `ymp-storage/src/knowledge.rs:375`. Fixed in candidate. |

Documentation paths in the table are relative to `ymp-docs/`; Rust paths are relative to `ymp-rust/crates/`.

## README handoff — overlap intentionally excluded

The parent should coordinate these backend-only wording changes with the ongoing README/UI work:

- Line 5: replace “bid for work” with “propose and accept temporary responsibilities; the runtime validates assignments and revisions.” Keep confirmation-qualified experience distinct from qualitative acceptance.
- Lines 34–36: label the probe's per-profile fan-out and point to the bounded native-check procedure in `guides/release-verification.md`; a metadata-only `catalog --refresh` example is distinct from authentication inference.
- Line 67: replace blanket one-writer wording with “For the 0.4.0 MVP, work stays in the selected directory. Independent assignments may overlap when enforced access permits; conflicting or unbounded writes serialize. No rollback guarantee is provided.” Leave the UI descriptions to Opus.

No TUI code/layout or current UI behavior was reviewed. `interface.md`, `usage.md` and the UI contract remain untouched.

## Verified without correction

`architecture/public-mcp.md` correctly describes public stdio, the fixed project scope, read-only default and explicit `--allow-execution`, separately from assignment-capability internal team tools. The CLI flag is a default-false boolean at `ymp-cli/src/main.rs:123`; its public-server dispatch is at line 189.

`guides/release-verification.md` is explicitly a pending procedure, keeps the current 0.3.0 package distinct from target 0.4.0, separates offline checks/native metadata/authorized inference, already warns about doctor fan-out, and requires revalidation of its dated native model/effort observations before a concrete quota proposal. Its install controls match `ymp-scripts/install.sh`. No release completion or new quota was inferred from that guide.

Validation: `git diff --check` passes; every relative Markdown target in the five edited guides resolves. Only English Markdown changed, so no duplicate Cargo or provider checks were run. Historical research was preserved as historical.

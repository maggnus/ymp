# ymp

The executable is named `ymp`. Application-owned data lives in `~/.ymp2`.
All project documentation, code comments, UI strings, and examples are in English.

The owner-approved intent.md is the authoritative product definition. Preserve it unless the owner explicitly requests a revision; it remains in Russian and derived documentation remains in English. Keep terminology, policy and task priorities aligned with its recorded digest in ymp-docs/tasks/tasks.json.

On 2026-09-12 the owner approved the final intent and requested tasks and a work plan. ymp-docs/tasks/plan.md now replaces the earlier product-definition pause; follow task dependencies, scope and acceptance criteria. YMP-010 records product approval, while YMP-116 specifies executable contracts. Agents have no permanent hierarchy: roles and permissions last for one assignment. The trusted runtime commits assignments, grants and final acceptance. An independently accepted result without confirmation stays unconfirmed and does not increase reputation. Terms are defined in ymp-docs/product/entities.md and the detailed policy in ymp-docs/architecture/team-and-effort-policy.md. Historical reviews and unproven optimization proposals do not override approved intent.

UI work must be delegated to Claude Code using claude-opus-5 with thinking level max; use Paseo CLI when available. The parent owns backend contracts, integration, and independent verification.

Agents are the working units and form each session's captured team. Providers and models are execution backends. Attribute usage to agent IDs within the session, never group team statistics by provider.

The owner clarified on 2026-09-13 that selectable agent names must come from native provider scans. Preserve the identifiers, display names and supported effort/control values returned by Claude Code, Codex and other enabled systems; never substitute provider labels or a hand-maintained model-name table. Keep provider identity separate, unknown metadata explicit, and historical names/settings bound to their captured records. Scanning populates the actual agent pool and stored snapshots; UI painting does not initiate native discovery. YMP-127 owns this correction.

Track project work in ymp-docs/tasks/tasks.json. manage.py generates both plan.md (progress) and README.md (task details); do not maintain their statuses by hand. Use planned, in_progress, owner_question, done, rejected, paused or new with a timestamped progress note. Readiness and dependency blocking are derived automatically; owner_question requires an actual unresolved question. Keep delivery counts separate from research and planning. Update evidence as work finishes and report progress during sustained work; a written design is not implemented functionality.

The root Cargo workspace contains packages in `ymp-rust/crates`. Documentation lives in `ymp-docs`, SDK bridges in `ymp-bridges`, and evaluation scenarios in `ymp-evals`.

Use the installed agents' native authentication. Never log credentials or copy tokens into application storage. For the 0.4.0 MVP only, team runs work directly in the user's selected directory. Serialize conflicting or unbounded writes; keep only metadata under ~/.ymp2. Do not create hidden source copies or Git repositories as MVP application behavior. This is an explicit MVP limitation, not a production isolation or rollback guarantee. The owner approved this policy on 2026-09-12; see ymp-docs/architecture/workspace-policy.md. Post-MVP isolation and recoverable publication are tracked in YMP-124. Isolated development worktrees for parallel implementation are separately authorized.

Favor narrow typed subsystem interfaces and injectable implementations. Runtime validation remains responsible for constraints, permissions, budgets, acceptance and evidence. Public MCP uses stdio for the first release; see subsystem-interfaces.md and public-mcp.md under ymp-docs/architecture. Unattended tests use mock/scripted providers. Real-provider probes require explicit minimal supported effort and the separate quota authorization; max/xhigh fixture values are data, not inference calls.

After changes, run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`. Real provider checks are separate from unattended tests.

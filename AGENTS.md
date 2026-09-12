# ymp

The executable is named `ymp`. Application-owned data lives in `~/.ymp2`.
All project documentation, code comments, UI strings, and examples are in English.

The owner-approved intent.md is the authoritative product definition. Preserve it unless the owner explicitly requests a revision; it remains in Russian and derived documentation remains in English. Keep terminology, policy and task priorities aligned with its recorded digest in ymp-docs/tasks/tasks.json.

On 2026-09-12 the owner approved the final intent and requested tasks and a work plan. ymp-docs/tasks/plan.md now replaces the earlier product-definition pause; follow task dependencies, scope and acceptance criteria. YMP-010 records product approval, while YMP-116 specifies executable contracts. Agents have no permanent hierarchy: roles and permissions last for one assignment. The trusted runtime commits assignments, grants and final acceptance. An independently accepted result without confirmation stays unconfirmed and does not increase reputation. Terms are defined in ymp-docs/product/entities.md and the detailed policy in ymp-docs/architecture/team-and-effort-policy.md. Historical reviews and unproven optimization proposals do not override approved intent.

UI work must be delegated to Claude Code using claude-opus-5 with thinking level max; use Paseo CLI when available. The parent owns backend contracts, integration, and independent verification.

Agents are the working units and form each session's captured team. Providers and models are execution backends. Attribute usage to agent IDs within the session, never group team statistics by provider.

Track project work in ymp-docs/tasks/tasks.json. manage.py generates both plan.md (progress) and README.md (task details); do not maintain their statuses by hand. Use planned, in_progress, owner_question, done, rejected, paused or new with a timestamped progress note. Readiness and dependency blocking are derived automatically; owner_question requires an actual unresolved question. Keep delivery counts separate from research and planning. Update evidence as work finishes and report progress during sustained work; a written design is not implemented functionality.

The root Cargo workspace contains packages in `ymp-rust/crates`. Documentation lives in `ymp-docs`, SDK bridges in `ymp-bridges`, and evaluation scenarios in `ymp-evals`.

Use the installed agents' native authentication. Never log credentials or copy tokens into application storage. Team runs work directly in the user's working directory. Serialize writes; keep only metadata under ~/.ymp2. Do not create hidden source copies or Git repositories.

After changes, run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`. Real provider checks are separate from unattended tests.

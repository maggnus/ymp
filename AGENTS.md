# ymp

The executable is named `ymp`. Application-owned data lives in `~/.ymp2`.
All project documentation, code comments, UI strings, and examples are in English.

The owner-authored intent.md is the concise product definition; edit its wording only as authorized by the owner and keep it in Russian. Derived documentation remains in English. Keep terminology, detailed policies and task priorities aligned with it. The documented direction includes bounded dynamic teams, fixed user constraints, joint executor/model/effort selection, independent verification, a shared budget and incremental reusable experience.

On 2026-09-12 the owner reopened the architecture and paused implementation. Product goals and the team/effort direction have since been documented at the owner's request. Coordination topology, numerical defaults and performance claims remain open in YMP-010; do not resume feature delivery merely because a documentation task is complete. Terms are defined in ymp-docs/product/entities.md and the detailed policy in ymp-docs/architecture/team-and-effort-policy.md. Historical UI reviews and research proposals do not override current intent.

UI work must be delegated to Claude Code using claude-opus-5 with thinking level max; use Paseo CLI when available. The parent owns backend contracts, integration, and independent verification.

Agents are the working units and form each session's captured team. Providers and models are execution backends. Attribute usage to agent IDs within the session, never group team statistics by provider.

Track accepted project work in ymp-docs/tasks/tasks.json and keep the generated ymp-docs/tasks/README.md current using manage.py. Update status and evidence as milestones finish, and report progress during sustained work. Distinguish research recommendations, authorized implementation, and completed delivery; do not mark a proposed feature as implemented.

The root Cargo workspace contains packages in `ymp-rust/crates`. Documentation lives in `ymp-docs`, SDK bridges in `ymp-bridges`, and evaluation scenarios in `ymp-evals`.

Use the installed agents' native authentication. Never log credentials or copy tokens into application storage. Team runs work directly in the user's working directory. Serialize writes; keep only metadata under ~/.ymp2. Do not create hidden source copies or Git repositories.

After changes, run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`. Real provider checks are separate from unattended tests.

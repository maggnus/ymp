# ymp project entry points

This file is the short operational guide for new Codex and Claude Code sessions.

## Start here

1. Inspect the worktree without discarding existing changes:

   ```sh
   git status --short --branch
   git remote -v
   ```

2. Read the project documents in this order:

   - `ymp-docs/README.md` — product boundary and document index;
   - `ymp-docs/PROJECT-CONTRACT.md` — public requirements and acceptance rules;
   - `ymp-docs/ROADMAP.md` — POC, MVP, and Alpha outcomes;
   - `ymp-docs/ARCHITECTURE.md` and `ymp-docs/PROTOCOL.md` — system and protocol design;
   - `ymp-docs/INVARIANTS.md` and `ymp-docs/SECURITY.md` — non-negotiable constraints;
   - `ymp-docs/DECISIONS.md` and `ymp-docs/REPUTATION.md` — decisions and rationale;
   - `ymp-docs/CALIBRATION.md` — pinned runtime profiles, calibration ladder, evidence, and blockers;
   - `ymp-docs/work/WORKFLOW.md` — execution-record rules;
   - `ymp-docs/work/STATUS.md` and `ymp-docs/work/WAVES.md` — generated status indexes.
   - `ymp-rust/SCHEMA.md` — durable event, object, compatibility, and migration rules.

3. For a specific work item, read its `WAVE.md`, `CARD.md`, and task file under
   `ymp-docs/work/waves/`.

## Production workspace layout

The POC is the first production version with a bounded feature set, not a disposable
implementation. The repository currently has two root subprojects:

- `ymp-docs/` — documents, work records, and the design artifact;
- `ymp-rust/` — the complete Rust workspace, including production packages, protocol models,
  fixtures, and evaluation tools. It produces one release executable named `ymp`.

Stable Rust boundaries live under `ymp-rust/crates/`; executable models and other development
tools live under `ymp-rust/tools/`. Do not create root-level `ymp-tui`, `ymp-runtime`,
`ymp-verifier`, or similar Rust projects. Do not create nested Git repositories. Every future root
repository or subproject must use the `ymp-<name>` prefix.

## Only design source

The only existing and authoritative design artifact is:

`ymp-docs/design/ymp_k9s_tui.dc.html`

Do not search for, recreate, or request `CLAUDE_REQUESTS.md`, `CLAUDE_DESIGN_REQUEST_V2.md`, or
other Claude Design material. References to those files are stale and must be removed when the
surrounding documentation is updated. Do not send Claude requests to recover or clarify design.

## Repository facts

- Worktree: `/Users/maggnus/Code/ymp`.
- Canonical repository: `https://github.com/maggnus/ymp.git`.
- Integration branch: `main`.
- Preserve user changes and never use destructive Git commands without explicit authorization.
- `ymp-docs/work/STATUS.md` and `ymp-docs/work/WAVES.md` are generated; never edit them manually.

## Required plugins

Install `paseo-cto` and `russian-speech` only from the GitHub repository
`maggnus/claude-plugins`, pinned to the declared tag. Do not use a local directory as the
installation source.

Claude Code:

```sh
PASEO_CTO_TAG=v9.13.0
claude plugin marketplace add "maggnus/claude-plugins@${PASEO_CTO_TAG}"
claude plugin install paseo-cto@maggnus
claude plugin install russian-speech@maggnus
```

Codex:

```sh
PASEO_CTO_TAG=v9.13.0
codex plugin marketplace add maggnus/claude-plugins --ref "$PASEO_CTO_TAG"
codex plugin add paseo-cto@maggnus
codex plugin add russian-speech@maggnus
```

Verify both the source and installed state:

```sh
claude plugin marketplace list
claude plugin list
codex plugin marketplace list
codex plugin list
```

The required source is GitHub `maggnus/claude-plugins`; the required `paseo-cto` version is
`9.13.0`.

## work.py

`work.py` ships with the `paseo-cto` plugin. Its absence from this repository is not a defect; do
not add a separate repository copy unless the owner explicitly changes this rule. The template is
inside the installed plugin:

- Codex: `~/.codex/plugins/cache/maggnus/paseo-cto/*/skills/paseo-cto/templates/work.py`;
- Claude Code: `~/.claude/plugins/cache/maggnus/paseo-cto/*/skills/paseo-cto/templates/work.py`.

If several versions are present, use the copy from `paseo-cto` version `9.13.0` installed from the
GitHub source above.

## Change and validation rules

- Build the POC as the production foundation. Documentation records verified behavior and must
  not replace implementation evidence.
- Check claims against Git, current documents, executable tests, and the sole design artifact.
- Do not make Claude requests for design work.
- Change primary work files first, then regenerate indexes with the plugin-provided `work.py`.
- Before handing off, run the narrow tests, affected project tests, format and lint checks,
  `git diff --check`, link/path checks, and `git status --short`.
- Report completed checks, anything not verified, and the next concrete step.

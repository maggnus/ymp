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
   - `ymp-docs/VISUAL_CONCEPT.md` — chat-first composition, operator path, and known gaps;
   - `ymp-docs/ARCHITECTURE.md` and `ymp-docs/PROTOCOL.md` — system and protocol design;
   - `ymp-docs/INVARIANTS.md` and `ymp-docs/SECURITY.md` — non-negotiable constraints;
   - `ymp-docs/DECISIONS.md` and `ymp-docs/REPUTATION.md` — decisions and rationale;
   - `ymp-docs/research/README.md` — research index, experiment protocols, analyses, and results;
   - `ymp-docs/research/001-calibration.md` — pinned runtime profiles, calibration ladder, evidence, and blockers;
   - `ymp-docs/work/WORKFLOW.md` — execution-record rules;
   - `ymp-docs/work/STATUS.md` and `ymp-docs/work/WAVES.md` — generated status indexes.
   - `ymp-rust/SCHEMA.md` — durable event, object, compatibility, and migration rules.

3. For a specific work item, read its `WAVE.md`, `CARD.md`, and task file under
   `ymp-docs/work/waves/`.

## Production workspace layout

The POC is the first production version with a bounded feature set, not a disposable
implementation. The repository currently has two root subprojects:

- `ymp-docs/` — documents, work records, and the visual-design sources;
- `ymp-rust/` — the complete Rust workspace, including production packages, protocol models,
  fixtures, and evaluation tools. It produces one release executable named `ymp`.

Stable Rust boundaries live under `ymp-rust/crates/`; executable models and other development
tools live under `ymp-rust/tools/`. Do not create root-level `ymp-tui`, `ymp-runtime`,
`ymp-verifier`, or similar Rust projects. Do not create nested Git repositories. Every future root
repository or subproject must use the `ymp-<name>` prefix.

## Current visual-design sources

The current chat-first visual concept has three authoritative sources with distinct roles:

- `ymp-docs/design/ymp_chat_tui.dc.html` — exact screen, state, fixture, and reusable-structure
  handoff;
- `ymp-docs/design/ymp_chat_tui.pdf` — the primary fixed-layout review and reading version;
- `ymp-docs/VISUAL_CONCEPT.md` — composition rationale, operator path, semantic constraints, and
  known gaps between the target interface and the current domain.

These sources are reference implementations of the future interface, not a frozen contract. What
suits the product is taken; what does not is changed, and the change is recorded in the work tree.
The HTML source is the only carrier of correspondence: the PDF is a reading convenience and must
never be cited as the source of a screen or a fixture.

The removed `ymp-docs/design/ymp_k9s_tui.dc.html` is a superseded historical artifact. Accepted
work records may retain immutable references to the revision they actually reviewed; those
references are evidence of history, not current design authority.

Do not search for, recreate, or request `CLAUDE_REQUESTS.md`, `CLAUDE_DESIGN_REQUEST_V2.md`, or
other Claude Design material. References to those files are stale and must be removed when the
surrounding documentation is updated. Do not send Claude requests to recover or clarify design.

## Research-document boundary

Human-readable hypotheses, experimental protocols, intervention plans, scientific analyses,
results, and negative findings live only under `ymp-docs/research/`. Product contracts,
architecture, security, invariants, and roadmap documents remain at the `ymp-docs/` root.
Executable manifests, corpora, or compliance tools remain beside their owning Rust tool and are
linked from the research document; do not duplicate executable truth into prose.

## Repository facts

- Worktree: `/Users/maggnus/Code/ymp`.
- Canonical repository: `https://github.com/maggnus/ymp.git`.
- Integration branch: `main`.
- Preserve user changes and never use destructive Git commands without explicit authorization.
- `ymp-docs/work/STATUS.md` and `ymp-docs/work/WAVES.md` are generated; never edit them manually.

## Required plugins

Install `paseo-cto` and `russian-speech` only from the GitHub repository
`maggnus/agentic-plugins`, pinned to the declared tag. Do not use a local directory as the
installation source.

Claude Code:

```sh
PASEO_CTO_TAG=v10.8.2
claude plugin marketplace add "maggnus/agentic-plugins@${PASEO_CTO_TAG}"
claude plugin install paseo-cto@maggnus
claude plugin install russian-speech@maggnus
```

Codex:

```sh
PASEO_CTO_TAG=v10.8.2
codex plugin marketplace add maggnus/agentic-plugins --ref "$PASEO_CTO_TAG"
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

The required source is GitHub `maggnus/agentic-plugins`; the required `paseo-cto` version is
`10.8.2`.

## work.py

`work.py` ships with the `paseo-cto` plugin. Its absence from this repository is not a defect; do
not add a separate repository copy unless the owner explicitly changes this rule. The template is
inside the installed plugin:

- Codex: `~/.codex/plugins/cache/maggnus/paseo-cto/*/skills/paseo-cto/templates/work.py`;
- Claude Code: `~/.claude/plugins/cache/maggnus/paseo-cto/*/skills/paseo-cto/templates/work.py`.

If several versions are present, use the copy from `paseo-cto` version `10.8.2` installed from the
GitHub source above.

## Change and validation rules

- Repository code is written only by `codex/gpt-5.6-sol`: use `high` for local, mechanically
  bounded changes and `xhigh` for architecture, state, security, concurrency, and cross-component
  boundaries. Research, architecture analysis, and experiment design use `codex/gpt-5.6-sol` at
  `max`. Weaker GPT, GLM, and Claude profiles may run tests, search for counterexamples, review
  results, or participate in controlled POC experiments, but they do not author repository code,
  test code, research conclusions, or plans; any resulting fix returns to a Sol author.
- Keep one read-only `codex/gpt-5.6-sol` `max` researcher attached to the scientific component
  across experiment-design and interpretation work. Consult it before changing a hypothesis,
  oracle regime, arm definition, budget comparison, metric, causal claim, or POC conclusion. Its
  report informs the CTO contract but never substitutes for executable or controlled evidence.
- GPT, GLM, and Claude are the only model families executed for development, testing, review, or
  POC participation. Other model routes may remain documented as possible integrations, but they
  are not run unless the owner explicitly changes this allowlist.
- Every behavioral test or evaluation that launches the product executable or an external agent
  runs in a newly created disposable directory with isolated project, `HOME`, `YMP_HOME`, `TMPDIR`,
  build, and export paths. It must not use the repository worktree as the launch directory or touch
  the operator's real `~/.ymp`. Library unit tests may run in their isolated Git worktree, but the
  moment a check crosses the executable/runtime boundary it uses the disposable evaluation root.
- Build the POC as the production foundation. Documentation records verified behavior and must
  not replace implementation evidence.
- Check claims against Git, current documents, executable tests, and all three current
  visual-design sources.
- Do not make Claude requests for design work.
- Change primary work files first, then regenerate indexes with the plugin-provided `work.py`.
- Before handing off, run the narrow tests for what changed, their negative halves, format and
  lint checks on the packages touched, `git diff --check`, link/path checks, and
  `git status --short`.
- The full workspace suite is an integration check, not a card check: it runs once before a
  merge into the release branch. A worker does not run it at the end of its card, and a
  reviewer runs it only to settle a stated hypothesis the combined tree alone can answer.
- Report completed checks, anything not verified, and the next concrete step.

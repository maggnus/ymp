# Development contract

The application and executable are named `ymp`. Product documents, code comments,
examples and user-facing strings are written in English.

Read `intent.md` and `ymp-docs/self-organizing-team-domain-model.md` before changing
product behavior. The latter is the owner-approved, single authoritative source
of architecture, domain names and product scope. `ymp-docs/domain.md` and
`ymp-docs/architecture.md` are subordinate vocabulary and implementation notes;
`ymp-docs/foundation.md` retains historical implementation material. This iteration
starts from scratch; do not count previous code, APIs or tests as delivered
functionality or assume they are a baseline to extend. That code is reachable only
at git tags named `legacy-*` (currently `legacy-foundation`) and never returns to the
working tree. Read `ymp-docs/legacy-lessons.md` instead of the code: it carries the
protocol facts, mechanics, pitfalls and test cases worth keeping, each with its
source. Open a tag directly only when the task record's context names the tag path
and the facts to extract. Copy nothing: no files, functions, type or module names,
API shapes or event vocabularies; derive every construct from the approved model
and its names. `make legacy-scan`, part of `make verify`, rejects denylisted legacy
identifiers and blocks copied from any `legacy-*` tag. Every commit message carries
a `Legacy-Consulted:` trailer, either `none` or `<tag>:<path> — <fact>`, and
reviewers check it against the diff. These documents,
historical amendments and previous iterations cannot override the approved model.
Use the model's names in code. Add a domain term only with its definition and
rationale in the model; architectural changes require an explicit owner decision.

Keep this repository self-contained. Dependencies between workspace crates use
relative paths inside the repository. Do not introduce machine-specific source
paths, ambient configuration requirements or undisclosed external services.

Keep domain values and validation separate from I/O. Strategies propose; the kernel
validates and commits. Adapters implement explicit ports and cannot grant themselves
authority through a returned verdict. Never label planned behavior as implemented.

Preserve native agent identities and authentication. No inference, installation,
publication or real user-data mutation is implied by a local build or test.
The explicit standing authorization below covers native development experiments.

Use existing dependencies for shared responsibilities. Add dependencies only for
an actual capability need. Avoid placeholder crates, generic service containers,
empty plugin systems and speculative configuration fields. One exception exists by
owner decision (2026-09-16): the module skeleton described in
`ymp-docs/project-worktree.md` is checked in as empty placeholder files. A
placeholder has no behavior and is never counted as delivered; only its owning
task fills it, and "create" in a task record means "give the placeholder its
behavior". Do not add further placeholders.

Verification should answer the changed behavior's actual questions. There is no
required test count. Test maintenance is a means to useful evidence, not a product
objective. Keep unsupported claims and unexecuted checks explicit.

Before committing code, run:

```sh
cargo build --workspace --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
```

Read the diff and check affected consumers. Use independent review for changes to
public contracts, resource ownership, persistence or execution authority. Keep
parallel work read-only unless writers have disjoint, explicitly owned worktrees.

Record delivered outcomes in Git. Development task records under
`ymp-docs/tasks/records/` are the single source of task status; follow
`ymp-docs/development-tasks.md`. Start with `manage.py next`, then `show ID` for the
selected task, instead of reading every record. Use the task tool for coordinated
updates and retain its expected-revision checks. The roadmap describes product
outcomes and links to records; do not maintain duplicate status labels there.
Task ownership is cooperative development coordination, not runtime authority.
Do not record speculative progress as completion or create another task database.

Use these exact development-task commands from the repository root:

```sh
python3 ymp-docs/tasks/manage.py next
python3 ymp-docs/tasks/manage.py show W1-0004
python3 ymp-docs/tasks/manage.py check
```

Before committing changes to the task workflow itself, also run:

```sh
python3 -m unittest discover -s ymp-docs/tasks/tests -v
python3 ymp-docs/tasks/manage.py render --limit 20 >/dev/null
```

## Standing authorization for native experiments

Owner decision, 2026-09-16: contributors may use Codex, Claude and GLM (the model's
`Glm` provider kind) for bounded ymp development and evaluation experiments through
available native providers and existing authentication. This permission persists
across tasks and sessions until the owner changes or revokes it. Do not request
per-run confirmation for experiments within this scope. It satisfies task-record
requirements for a "separately authorized" or "explicitly authorized" native run.

Prefer available models and execution profiles that minimize expected token use
for the experiment. Use the native `low` reasoning setting when supported;
otherwise choose the lowest supported reasoning setting or an available model
with evidence of low token use. Discover actual model offerings and supported
settings at run time; do not invent model IDs, translate effort scales between
providers, or assume that `low` guarantees the fewest tokens. When comparable
usage evidence is unavailable, record that uncertainty and start with a small
bounded run. Do not automatically escalate to higher reasoning effort or a more
token-intensive profile under this permission.

Record the experiment purpose, selected provider/model, requested/sent/reported
settings, finite resource limits, actual usage and unknown coverage with its
evidence. Preserve the model's accounting, workspace and authority rules, and
keep native results distinct from Scripted runs and protocol fixtures. This
permission covers inference for experiments; installation, publication and
mutation of real user data retain their separate authorization requirements.
A later user stop or narrower instruction takes precedence.

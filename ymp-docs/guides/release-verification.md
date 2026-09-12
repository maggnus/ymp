# Local release verification

This is the procedure for YMP-121, not a release-completion record. The
[delivery plan](../tasks/plan.md) is the source of progress. The target release is
0.4.0; the current package version remains 0.3.0 until the accepted changes are
integrated. Preserve the approved intent and the owner's unrelated working changes.

## Integrated source and offline checks

Record the exact source commit and executable/bridge hashes. Complete the required
checks on that source:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm ci --prefix ymp-bridges/claude --no-audit --no-fund
npm run check --prefix ymp-bridges/claude
npm test --prefix ymp-bridges/claude
npm run build --prefix ymp-bridges/claude
cargo test -p ymp-cli --test native_catalog -- --include-ignored
```

Run the completed YMP-126 driver against all four workflows and all thirteen
protocol cases specified by [the evaluation contract](../../ymp-evals/README.md).
Retain actual artifacts, raw records, ordered observations, source maps and
validator output. A skipped adapter, a fixture-only result or an unsupported
projection does not pass. Scripted results establish runtime behavior, not model
quality or a cost advantage.

The independent final reviewer uses max reasoning and checks each release
requirement against the integrated evidence. Development reviewer effort is
separate from provider effort inside an application test.

## Package and install

After the release checks and review pass, update the workspace package version
and lockfile together, build with `cargo build --release --locked`, and verify
`--version`, `--help` and the deterministic `demo` on the resulting executable.
Exercise `ymp-scripts/install.sh` with a temporary `YMP_BIN_DIR` and application
home before updating the owner's installation. Record the tested source and
binary hashes after any version change.

For this checkout the expected paths are:

| Component | Path |
| --- | --- |
| Release executable | `/Users/maggnus/Code/ymp2/target/release/ymp` |
| Development executable | `/Users/maggnus/Code/ymp2/target/debug/ymp` |
| Installed command | `/Users/maggnus/.local/bin/ymp` |
| Claude SDK bridge | `/Users/maggnus/Code/ymp2/ymp-bridges/claude/dist/index.js` |

The installed command links to the checkout. Its target must be verified; a
successful development build does not update an existing release binary. The
bridge can be overridden with `YMP_CLAUDE_BRIDGE`.

Use `ymp -C /path/to/work` for the selected output directory. Application data
belongs to `~/.ymp2`. Describe the MVP's direct-write and recovery limitations
using [the workspace policy](../architecture/workspace-policy.md).

## Native metadata and optional inference

`ymp catalog --refresh` queries installed native metadata without a model prompt.
Preserve native names, alias resolution, controls and observation times. On
2026-09-13 the supported owner-home refresh found 19 offerings across three
providers and preserved the existing configuration and history; see the
[refresh audit](../research/evidence/native-catalog-owner-refresh.json). That is
metadata compatibility evidence, not a completed native task test.

Real model checks are outside the unattended suite. No inference quota is granted
by this procedure. If a native smoke check is authorized, use a temporary app
home and an explicitly selected empty working directory with one enabled test
actor for the selected provider. Do not use the owner's full discovered pool:
`doctor --probe` currently probes every matching enabled profile.

The bounded initial proposal is one invocation for each of Codex and Claude,
sequentially, with no application retry, no file writes and a 120-second deadline
per invocation. The exact prompt is `Reply with exactly YMP_OK. Do not modify
files or run commands.` Pin the model and minimum supported effort explicitly:

| Native installation observed on 2026-09-13 | Model selection | Effort |
| --- | --- | --- |
| Codex CLI 0.154.0 | `gpt-5.6-sol` | `low` |
| Claude Code 2.1.269, Agent SDK 0.3.246 | `claude-opus-5[1m]`, revalidate its advertised alias | `low` |

The metadata snapshot advertises these settings. Revalidate installation versions
and alias/control support without inference immediately before presenting this
exact two-invocation proposal for quota approval. Changed versions require an
updated proposal. GLM is a separate optional check: its current native
`thought_level` choices are `none`, `high` and `max`; `none` is the explicit
minimum, while the observed default is `max`. Capture its exact installed adapter
version before including it in an authorized proposal.

Set an observed-token stop threshold of 20,000 for the two-invocation proposal,
with no second invocation after exhaustion or unknown usage. This is an admission
and cancellation threshold, not a hard native token ceiling: a usage report can
arrive after expenditure. If strict expenditure is required, do not run a provider
that cannot enforce it. Charge failures and cancellation, retain requested/sent/
reported settings and coverage, and never copy credentials or native authentication
errors into the report. Verify shutdown and absence of active child work after a
timeout or cancellation.

Release notes distinguish offline behavior, metadata compatibility and any
separately authorized native results. Unrun native checks remain explicitly unrun.

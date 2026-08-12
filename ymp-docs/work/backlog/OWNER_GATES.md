# Owner gates

Decisions that may not be inferred by the execution plan. A resolution remains in this table with
its date so that later evidence can be interpreted against the decision actually made.

| Gate | Decision required | Blocks | Opened | Resolution |
|---|---|---|---|---|
| `G1` | Establish the canonical Git repository and HTTPS source URL, integration branch, commit convention, initial validation commands, and work-tree validation tooling. | First dispatch; all durable source evidence | 10/08 | Resolved 12/08: repository `https://github.com/maggnus/ymp`, branch `main`, English `Conventional Commits`, the `ymp-rust` validation ladder, and plugin-supplied `work.py` from GitHub tag `v9.13.0` are established. Accepted changes may be pushed directly to `origin/main`. |
| `G2` | Approve the first POC task corpus, public requirements, protected-oracle regime, decomposability labels, and minimum useful experimental effect. | `W1-EXP-01a`, `W1-EXP-01b` | 10/08 | Partially resolved 12/08: the owner delegated construction of the development calibration program. The L1-L3 engineering ladder and its protected oracles are established, but they are explicitly not the primary POC corpus; decomposability strata and the minimum useful experimental effect remain open. |
| `G3` | Approve pinned Codex and Claude Code profiles, provider disclosure, experiment-specific accounts or quotas, total comparison budget, and permitted in-flight overshoot. | `W1-EXP-01d`, `W1-APP-02c`, `W1-APP-02d`, `W1-EVL-04a` | 10/08 | Partially resolved 12/08: real low-effort Claude and Codex development runs are authorized. Codex `codex-cli 0.147.0` / `gpt-5.6-sol` / `ymp-codex-low-v1` and Claude Code `2.1.227` / `claude-opus-5` / `ymp-claude-low-v1` are pinned; Claude has a USD 1.00 per-invocation stop. Claude authentication, experiment-specific quotas, the total primary-comparison budget, and permitted in-flight overshoot remain open. |

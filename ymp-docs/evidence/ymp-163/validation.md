# YMP-163 validation record

## Before-fix control

The public `Engine` regression was first added with the legacy additive `revise`
proposal, an ordinary command that always failed in the temporary workspace, and an
equivalent proposed command whose retained file existed. The exact command was:

```text
cargo test -p ymp-runtime --test board_coordination active_executor_can_replace_a_broken_ordinary_check_before_attempts_exhaust -- --exact --nocapture
```

It failed as required. The session ended `blocked`; the final task result retained
`test -f unavailable-check` with exit status 1. This reproduced both causes: the
running-task proposal was not applied before final-attempt review, and additive
`revise` could not remove the failed command.

## After-fix focused controls

The same public-engine test now submits typed `replace_checks` from
`missing-check-binary -f proof.txt` to `/bin/test -f proof.txt`. This preserves the
same file-existence requirement while changing only the unavailable command
implementation. It completed successfully. Assertions establish that:

- the board exposes and enforces `definition_version`;
- the retained command fails and the proposed command passes before review;
- one independent `review_check_revision` assignment receives both command sources,
  outputs and rationale;
- the final task contains only `/bin/test -f proof.txt`; the result binding has a
  distinct ID while the saved attempt counter remains one before and after;
- the accepted sibling has exactly one execution assignment;
- the revised target also has exactly one production assignment, while both the
  replacement review and fresh ordinary candidate review are charged invocations;
- the revised ordinary task remains accepted but unconfirmed, and the captured
  trusted contract count is unchanged.

The one new negative control proposes `test -d .`, which succeeds trivially in the
working directory while the retained requirement still fails. It reaches the same
independently admitted `review_check_revision` boundary with both outputs present.
The reviewer rejects the weakened meaning, the runtime records one rejected review
without selecting another reviewer, and the original command remains authoritative.
Existing tests retain the unchanged stale-version, authority and trusted-contract
protections; no additional mutation matrix was added.

`cargo test -p ymp-runtime --test board_coordination -- --nocapture` passed all 13
tests. `cargo test -p ymp-core
board::tests::definition_binding_ignores_review_bookkeeping_but_detects_check_changes
-- --exact --nocapture` passed and proves that running-to-review bookkeeping preserves
the definition digest while an actual check change does not.

## Final checks and byte binding

The first full-workspace run exposed a stack overflow while deserializing a large
evaluation trace. The added provenance fields had increased `RecordLinks` stack size.
The final implementation combines review/result provenance behind one boxed tagged
type and boxes the existing board-release value without changing its JSON shape. The
previously failing evaluation test then passed with the ordinary test-thread stack.

No product source changed after the following final checks:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p ymp-cli --bin ymp
```

All commands exited 0. The workspace test passed every required test. Two existing
tests remained explicitly ignored: the Claude SDK executable scan that requires a
separate bridge build, and the retained interactive TUI fixture walk. No native
provider was called.

The resulting debug executable is bound as:

```text
target/debug/ymp sha256 4848b1bbc26129a8383b17270dca79cf36508e125df98378b0733654ba4b6f48
```

The SHA-256 of the final staged implementation diff from `c18fe31`, excluding
`ymp-docs/evidence/ymp-163/**`, is:

```text
b6cfb46f3c19e762f4f94b9ba4be300636b335e4fc38dd990581a55464b297bd
```

The final independent read-only review verdict is `ACCEPT`; its findings and
corrections are recorded in `independent-review.md`.

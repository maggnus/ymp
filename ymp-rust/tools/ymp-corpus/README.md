# ymp repair-corpus candidate: first reproducible edition

ymp-corpus prepares and reproduces a compact first edition proposed for the primary POC corpus.
It is a development tool, not a production package and not part of the ymp executable. This
edition is neither statistically sufficient nor frozen for primary execution.

`corpus/OWNER-DECISION.json` is a non-authoritative decision packet, not evidence of owner
authorization. ymp-corpus cannot authenticate the project CTO channel, does not accept approval
files or self-declared authority, and never reports an owner-authorization result.

The committed corpus contains public contracts, exact upstream revisions, lockfiles, protected
oracles, requirement matrices, and small reverse-applied repair patches. Upstream source archives
and vendored dependencies are never committed. prepare obtains them from public sources and stores
them outside Git under content-derived paths.

    cd ymp-rust
    CORPUS_CACHE=/absolute/cache/path
    cargo run -p ymp-corpus -- --corpus tools/ymp-corpus/corpus \
      --cache "$CORPUS_CACHE" prepare
    cargo run -p ymp-corpus -- --corpus tools/ymp-corpus/corpus \
      --cache "$CORPUS_CACHE" check \
      --technical-evidence-only
    cargo run -p ymp-corpus -- --corpus tools/ymp-corpus/corpus \
      --cache "$CORPUS_CACHE" verify \
      --technical-evidence-only \
      --report tools/ymp-corpus/corpus/results/2026-08-12-reproduction.json

`check` and `verify` require the explicit `--technical-evidence-only` option because their successful
result establishes technical integrity only. Reports identify their authorization scope as
`technical_verification_only`. Admission and owner authorization remain external decisions.

Only prepare may use the network. `verify` points Cargo at the digest-checked vendor tree, sets
Cargo's offline mode, uses an empty CARGO_HOME, and builds each candidate in a fresh temporary
directory. The command verifies the upstream baseline, the known fixed revision, and one
reverse-applied invalid variant for every major requirement. An invalid variant that passes, a
failure in the wrong protected test, a timeout, or a cache digest mismatch makes the package
unusable and causes a nonzero exit.

The approved-set root is SHA-256 over the exact `registry.json` bytes. The registry binds every
policy and task manifest; each manifest binds the exact public contract, technical review,
requirement matrix, protected oracle, lockfiles, and negative-control patches. Each public
`PROJECT.md` repeats the SHA-256 values of every other substantive task and corpus artifact,
including protected artifacts by digest only. It excludes itself, the enclosing manifest, the
registry, and any external owner decision to avoid a hash cycle. A changed byte is therefore rejected
while loading the corpus, before any candidate command is started. Updating all dependent digests
changes the registry root; ymp-corpus does not determine whether an external decision remains valid.

The registry is withheld from producing attempts because it identifies the fixed commits and
protected artifacts. A producer receives only its task's PROJECT.md, source archive, pinned source
lockfile, and declared visible upstream checks. L1-L3 are development calibration and are
explicitly excluded.

## Frozen W1-EXP-01b study design

`corpus/study/PROTOCOL.md` and `corpus/study/manifest-v1.json` preregister the matched-budget
comparison. The manifest is bound to the approved initial root while explicitly refusing to call
the four-package edition sufficient. The distribution-free fixed-sample calculation requires
1,992 distinct tasks in each structural stratum, so 3,980 additional packages are required before
primary collection.
This task does not add them.

The commands below require every study input explicitly. They do not consult an environment
variable, user configuration, or result-dependent default:

    cd ymp-rust
    cargo run -q -p ymp-corpus -- \
      --corpus tools/ymp-corpus/corpus \
      --cache /tmp/unused-by-study-command \
      study-check \
      --manifest tools/ymp-corpus/corpus/study/manifest-v1.json \
      --digest tools/ymp-corpus/corpus/study/manifest-v1.sha256
    cargo run -q -p ymp-corpus -- \
      --corpus tools/ymp-corpus/corpus \
      --cache /tmp/unused-by-study-command \
      study-power \
      --manifest tools/ymp-corpus/corpus/study/manifest-v1.json \
      --digest tools/ymp-corpus/corpus/study/manifest-v1.sha256
    cargo run -q -p ymp-corpus -- \
      --corpus tools/ymp-corpus/corpus \
      --cache /tmp/unused-by-study-command \
      study-dry-run \
      --manifest tools/ymp-corpus/corpus/study/manifest-v1.json \
      --digest tools/ymp-corpus/corpus/study/manifest-v1.sha256 \
      --records tools/ymp-corpus/corpus/study/synthetic-records-v1.json

    cargo run -q -p ymp-corpus -- \
      --corpus tools/ymp-corpus/corpus \
      --cache /tmp/unused-by-study-command \
      study-negative-controls \
      --manifest tools/ymp-corpus/corpus/study/manifest-v1.json \
      --digest tools/ymp-corpus/corpus/study/manifest-v1.sha256 \
      --records tools/ymp-corpus/corpus/study/synthetic-records-v1.json

Each aggregate case is also reachable as a real failing public command by replacing
`study-negative-controls` with `study-negative-control --case CASE`. The process must return a
nonzero code for an invalid case. The RETURN-specific controls and their boundaries are:

| Cases | Distinguishes | A conforming record passes when | Does not establish |
|---|---|---|---|
| `arm_runtime_profile_mismatch`, `arm_model_route_mismatch` | an arm differs from its block profile or exact route | every arm repeats the profile and its sole accounted route equals the block route | external profile admission or truthful provider telemetry |
| `early_protected_result_disclosure` | protected output is revealed at or before selection commit | query reveal is later, or both query and reveal are absent | authenticity of future event sequences |
| `arbitrary_block_seed`, `reversed_assignment_order`, `arm_assignment_mismatch` | seed, order, or arm correspondence differs from the digest-derived schedule | all three values match the frozen derivation | that a future launcher truthfully reports what it started |
| missing or substituted `--corpus` | the argument is absent or the loaded registry digest differs from the manifest root | an explicit path loads the exact approved digest | owner authorization; a byte-identical copy is intentionally accepted |

These fields are the W1-EVL record boundary rather than a test-only branch. Future W1-EVL export
records supply the same block, arm, route-accounting, selection, and protected-result sequences
that the synthetic dry run supplies here.

The cache argument remains a global compatibility option for the corpus preparation commands and
is not read by a study command. A study command reports `technical_verification_only`; it cannot
assert owner approval, admit a runtime profile, or authorize primary collection.

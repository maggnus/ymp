# ymp repair-corpus candidate: first reproducible edition

ymp-corpus prepares and reproduces a compact first edition proposed for the primary POC corpus.
It is a development tool, not a production package and not part of the ymp executable. This
edition is neither statistically sufficient nor frozen for primary execution.

The committed corpus contains public contracts, exact upstream revisions, lockfiles, protected
oracles, requirement matrices, and small reverse-applied repair patches. Upstream source archives
and vendored dependencies are never committed. prepare obtains them from public sources and stores
them outside Git under content-derived paths.

    cd ymp-rust
    CORPUS_CACHE=/absolute/cache/path
    cargo run -p ymp-corpus -- --cache "$CORPUS_CACHE" prepare
    cargo run -p ymp-corpus -- --cache "$CORPUS_CACHE" check
    cargo run -p ymp-corpus -- --cache "$CORPUS_CACHE" verify \
      --report tools/ymp-corpus/corpus/results/2026-08-12-reproduction.json

Only prepare may use the network. verify points Cargo at the digest-checked vendor tree, sets
Cargo's offline mode, uses an empty CARGO_HOME, and builds each candidate in a fresh temporary
directory. The command verifies the upstream baseline, the known fixed revision, and one
reverse-applied invalid variant for every major requirement. An invalid variant that passes, a
failure in the wrong protected test, a timeout, or a cache digest mismatch makes the package
unusable and causes a nonzero exit.

The registry is withheld from producing attempts because it identifies the fixed commits and
protected artifacts. A producer receives only its task's PROJECT.md, source archive, pinned source
lockfile, and declared visible upstream checks. L1-L3 are development calibration and are
explicitly excluded.

## Input for W1-EXP-01b

corpus/W1-EXP-01b.md records the verified fields and the remaining statistical decisions.
`corpus/policies/expansion.json` defines the mechanical expansion and freeze rule. W1-EXP-01b must
first preregister its power rationale and the required number of distinct tasks in each structural
stratum. It then extends this edition until those thresholds are met; repeated runs of one task do
not increase the number of distinct tasks. Task structure cannot be relabelled after ymp outcomes
are visible, and corpus preparation checks are not primary-arm observations.

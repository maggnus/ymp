# Repair partial less-than SemVer requirement evaluation

Source revision: 35d918d334cdecc9725e217f460b90f478b5dabd.

## Goal

Make partial less-than comparators follow Cargo-compatible prerelease boundaries inside compound
requirements that otherwise admit a prerelease, without changing parsing or unrelated comparison
operators.

## Requirements

1. SEMVER-LT: <I.J must match stable versions below minor J and must exclude prereleases at the
   I.J boundary even when another comparator in the same requirement admits that prerelease.
2. SEMVER-LTE: <=I.J must include stable releases in minor J but must exclude prereleases at its
   lower boundary even when another comparator in the same requirement admits that prerelease.

## Visible check

Run cargo test --locked --offline -p semver --lib --all-features. The protected oracle uses
held-out compound requirements and versions around the declared boundaries.

## Scope

Changes may affect only requirement evaluation and its tests. Parser, display, dependency, and
manifest changes are out of scope.

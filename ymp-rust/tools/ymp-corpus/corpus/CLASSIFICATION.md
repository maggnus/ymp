# First-edition structural classification

This classification is fixed within the reproducible first edition before any ymp outcome is
observed. The edition is a candidate for the primary corpus, not a statistically sufficient or
study-frozen corpus. Classification uses only the public contract and the source-level dependency
structure.

- `decomposable`: the contract has at least two independently testable requirements whose
  implementation paths do not require one another and whose results can be combined without an
  ordering dependency.
- `strongly_sequential`: the defect is one state transition, parser path, or evaluation rule for
  which later work depends on the correctness of the same earlier logic. Dividing examples among
  participants does not divide the implementation dependency.

The label is an experimental stratum, not a claim that every possible repair strategy has the
same structure. Labels must not be changed after ymp outcomes are available; a correction creates
a new corpus version and excludes the old classification from the primary comparison.

| Task | Label | Pre-outcome basis |
|---|---|---|
| bstr-eof-and-debug | decomposable | EOF handling and byte-debug formatting occupy independent functions and have separate tests and patches. |
| toml-datetime-validation | decomposable | Leap-second bounds and Gregorian calendar-day bounds are independently testable validations. |
| semver-less-than-prerelease | strongly_sequential | Both comparator requirements depend on the same ordered matches_less evaluation rule. |
| serde-json-surrogate-escapes | strongly_sequential | Correctness depends on one escape-parser cursor transition after a lone surrogate. |

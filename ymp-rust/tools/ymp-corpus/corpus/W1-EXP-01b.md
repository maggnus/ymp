# First-edition handoff to W1-EXP-01b

The machine-readable source of truth is registry.json. This first reproducible edition is a
candidate for the primary corpus, not a statistically sufficient or study-frozen corpus.
W1-EXP-01b may rely on these verified pre-outcome fields:

- exact source and fixed Git commits and archive digests;
- public requirement identifiers and protected-test mappings;
- decomposable or strongly_sequential classification;
- pinned toolchain, lockfile, command, timeout, and vendor-tree digests;
- known fixed candidates and reverse-applied invalid control variants; and
- the rule that any unexpected control pass or infrastructure error makes a package unusable.

Corpus preparation and reproduction use no model calls and are not observations from a study arm.
L1-L3 remain excluded calibration cases. The monetary budget is unset; W1-EXP-01b must still
preregister equal resource opportunity, actual call/token/time/cost accounting, repetitions,
exclusions, stopping rules, and the minimum useful effect before any primary ymp outcome is
observed.

The first edition contains two tasks in each structural stratum. This count records successful
reproduction only; it is not a statistically justified study size. Before any primary ymp outcome,
W1-EXP-01b must preregister the power analysis and the resulting required number of distinct tasks
per stratum. `policies/expansion.json` then applies mechanically: add independently verified tasks
while any stratum is below its preregistered threshold, using the declared deterministic selection
order. Repetitions of one task do not increase the distinct-task count. Freeze is permitted only
when every threshold and admission condition holds; after primary outcomes, tasks cannot be added,
substituted, or relabelled.

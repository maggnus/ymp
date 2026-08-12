# W1-EXP-01b frozen study handoff

The corpus source of truth remains `registry.json`, whose approved root is
`e4e886bf1dfd433342f3f10f34c086415462908d60f5a53d1448824c9fe3bfc7`.
The separate study source of truth is `study/manifest-v1.json`, frozen at the SHA-256 recorded in
`study/manifest-v1.sha256`. The first reproducible edition remains a primary-corpus candidate, not
a statistically sufficient execution corpus.

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

The frozen distribution-free power calculation requires 1,889 distinct tasks per stratum for
instrumental reliability and 1,992 for the communication intervention; the corpus threshold is
therefore 1,992 per stratum. Five stochastic repetitions receive no distinct-task credit. The
initial two tasks per stratum leave 1,990 additional tasks in each stratum, 3,980 total.

`policies/expansion.json` applies mechanically: add independently verified tasks while a stratum is
below 1,992, using ascending `(repository_url, source_commit, task_id)` from a preregistered candidate
list. Before primary collection an unusable task is replaced by the next same-stratum candidate.
After the first primary route call, tasks cannot be added, substituted, removed, or relabelled.

Technical readiness requires the study check, expanded corpus root, admitted exact runtime
profiles, and a compliant dry run. Execution authority remains external: the owner must approve
the study digest, expanded root, profiles, and start. A self-authored JSON value cannot satisfy that
gate.

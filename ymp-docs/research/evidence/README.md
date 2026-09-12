# Research evidence

The baseline is ymp 0.3.0 at 56e21dd. These records distinguish product observations, controlled offline fixtures, and hypotheses. Product code and provider configuration were not changed. No experimental provider inference was performed; the explicitly delegated Claude UI review used development-agent quota.

| Record | Coverage | What it does not establish |
| --- | --- | --- |
| [Journal audit](journal-audit.json) | Consistent content-free snapshot; phase counts; 32 choices and their reconstructed propensities | Comparative quality, counterfactual cost, or a population reliability rate |
| [History audit](history-audit.json) | Four commits, source sizes, and touched files | Causation from file size or a review-latency trend |
| [Protocol experiments](protocol-experiments.json) | 32 cases; ceremony counts, phase failures, retry prototypes, cancellation, duplicate-effect control | Real model quality, real retry cost, or predicted latency savings |
| [Context probe](context-probe.json) | Release-profile calls into actual workspace/storage crates; byte sizes, timings, path equality, FTS5 relevance control | Native tokenizer counts or model answer-quality equivalence |
| [Existing scenario smoke](scenario-smoke.json) | Mock's canned result fails the independent greeting.py scenario | A score for a real agent or evidence of learning |
| [Installed effort audit](effort-capabilities.json) | Installed versions, source hashes, native fields and scope | A guarantee that an advertised option is honored by a live resolved model |
| [GLM capability functions](glm-effort-capabilities.json) | Pure local checks for the cached 1.3.0 package | Current upstream-server model behavior |
| [Effort-policy probe](effort-policy-probe.json) | 81 declared-capability cases and effort-sensitive identity control | Token savings or quality from changing effort |
| [Check-gate controls](check-gate-probe.json) | Four harmless stub cases exposing prefix-gate limits | Containment of real native agents |
| [Outcome reuse controls](outcome-reuse-probe.json) | Twelve conditional guards; a false-intent negative control and an external symlink dependency | Semantic matching accuracy or safe automatic substitution |
| [Outcome economics](outcome-economics.json) | Per-row canonical counts, native shapes, project identities, input weighting and twelve sensitivity scenarios | Complete historic cost, actual billing, or a universal economic denominator |
| [UI architecture review](ui-architecture-review.md) | Claude Opus 5 max source/history review, corrected independently; H9 presentation semantics | New product behavior or a measured refactoring payoff |
| [Verification record](verification.json) | Source scope, request digest, check results, and quota classification | Authorization to implement proposed changes or run paid experiments |

The scripts live in [research tools](../tools/). Journal exports deliberately omit private prompts, message bodies, file contents, credentials, and native session handles. Their timestamps and maximum event sequence define the observations; later runs of the audit scripts produce a new dataset.

Weighted-input ranges bound missing-cache composition only within the recorded input subtotal. Partial invocations may omit additional requests, so these are not upper bounds on whole-run cost. Fixture token counters are deliberately synthetic. Working-tree file counts can change as research files are added; the generated directory fixtures are controlled.

The main deliverables are [research findings](../research-program-findings.md), [the experimental protocol](../experiment-protocol.md), and [the maintained task register](../../tasks/README.md).

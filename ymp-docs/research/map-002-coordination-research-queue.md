# Coordination research queue

## Status and boundary

`MAP-002` orders unresolved scientific questions by the earliest evidence needed to interpret later
work. Priorities are readiness classes, not product scheduling or kernel policy. Every item is
specified only: none is an experiment, harness, accepted causal result, or authorization to spend
model budget. It is curated under `SCI-QUEUE-01` against baseline
[59288d1](https://github.com/maggnus/ymp/commit/59288d11b400a00ee22c8cc36f9a80e4077aea1c).

This record changes no current arm, task set, seed, budget, metric, frozen protocol, or kernel
semantics. In particular, `weak-diagnostic-v1` and the frozen primary comparison remain unchanged.

## P0 — pull-transport reachability and profile tool use

**Claim.** The accepted `publish`/`read_board` seam can make one authorized message from participant
A available to participant B during bounded active invocation opportunities, early enough for a
predeclared later B action. This is transport readiness only.

**Zero-model conformance.** A deterministic fake runtime may force ordering because it tests the
runner, not cooperation. It must exercise S1 (publish, then read), S2 (empty read, publish, remaining
read), and S3 (all reads spent or yield before publish, with no board-message wake). S1 and S2 must
preserve, in order:

```text
A MessagePublished -> DeliveryRecorded to B -> predeclared later B action -> honest terminal
```

An unauthorized participant receives nothing, S3 terminates honestly, and the fixed read cap
precludes polling pressure. `DeliveryRecorded` proves availability and cursor advance, never
reading, belief, influence, or task value.

**Real participant boundary.** The runner may freeze content-independent active windows, their
order, read cap, accounting, and termination. It may not force `publish`, `read_board`, continued
activity, or a semantic response. An unused opportunity, early yield, or reads spent before
publication is valid negative participant behaviour. Missing promised mechanics or accounting, or
a successful authorized post-publication read without `DeliveryRecorded`, is infrastructure-invalid.
The first model experiment therefore needs bounded active invocation opportunities, not a typed
board-message wake. Communication after a participant has yielded remains a separate hypothesis.

**Two-stage profile admission.** Stage 1 is a zero-model launch-path compatibility check of the
exact executable version, flags, Git trust, isolated configuration, generated tool binding, and
fake tool host. It cannot be called exact-route proof because no model route or model tool choice is
exercised. Stage 2 is one separately frozen and tightly capped no-task-output model call in a
disposable project. A random nonce absent from the prompt is placed in an allowed input file; the
model must read it, write the exact bytes to the sole allowed output, read them back through the
bound workspace tools, terminate honestly, and produce complete call, token, time, and cost
accounting. This is profile-admission evidence, never an arm or communication observation, and it
does not replace lifecycle, isolation, cancellation, or descendant-cleanup admission. The
read-back trace proves tool use and returned bytes, not comprehension.

**Falsifier / stop.** Stage 1 fails on any compatibility or fake-host defect. Stage 2 fails on a
wrong or absent nonce, missing attributable read/write/read-back trace, an unauthorized effect,
dishonest terminal, budget excess, or absent usage or cost. Either failure leaves the exact profile
unadmitted and stops model experiments. S1/S2 transport failure, unauthorized delivery, S3 waking
from publication, an unbounded poll, or dishonest termination stops the runner. A participant's
valid early yield is retained as negative behaviour and does not become an infrastructure repair.
Any repair starts under a new manifest and budget; it cannot continue a failed run.

**Dependencies.** Accepted collaboration and recruitment tools; an authorized two-participant
managed path; both admission stages; a predeclared B action and finite invocation/read schedule.
The board emits
[`MessagePublished` and `DeliveryRecorded`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-rust/crates/ymp-board/src/protocol.rs#L253-L274),
and `read_board` records delivery only after B pulls it
([`ymp-application`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-rust/crates/ymp-application/src/lib.rs#L2061-L2118)).
The accepted kernel's enumerated
[`WakeCondition`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-rust/crates/ymp-domain/src/commitment/invocations.rs#L61-L85)
has no board-publication condition; none is assumed for this bounded-active-window experiment.

**Priority and evidence status.** **P0.** The publish/read seam and audience filtering
([`W1-COR-03z`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/work/waves/W1/W1-COR-03/tasks/W1-COR-03z.md#L79-L105))
and participant recruitment
([`W1-PRD-05j.1`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/work/waves/W1/W1-PRD-05/tasks/W1-PRD-05j/subtasks/W1-PRD-05j.1.md#L64-L90))
are accepted.
[RUN-001](https://github.com/maggnus/ymp/blob/bad509110f7fa1e0f8d7438552878a3f21fff679/ymp-docs/research/run-001-manual-file-communication-pilot.md#L3-L113)
is infrastructure-invalid and
rejects its probe-only readiness as sufficient admission: its declared-ready profile could not use
workspace tools. It did not execute the complete fake-host stage specified here. No admitted S1-S3
trace or communication observation exists.

## P1 — independent-first communication

**Claim.** Freezing an independent first assessment before social information preserves an
identifiable baseline for evidence-driven revision and reduces correlated social convergence.

**Observable prediction.** Each receiver commits a bounded initial assessment digest while board
history, producer rationale, and prior votes are unavailable; later authorized communication
produces an attributable revision, maintained dissent, or abstention whose accuracy and task value
can be compared with the frozen first assessment.

**Falsifier / stop.** Early disclosure, a missing initial digest, reconstruction of the first
assessment after discussion, or a revision equally induced by unsupported social pressure makes the
contrast inadmissible. No beneficial revision records a negative result rather than proof that
independence is unnecessary.

**Dependencies.** P0 reachability; blinded reveal enforcement; a valid reviewer oracle; fresh
sessions and matched budgets. The product protocol already specifies the independent commitment
boundary
([`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/PROTOCOL.md#L236-L242)).

**Priority and evidence status.** **P1.** [Lorenz et al. (PNAS 2011)](https://doi.org/10.1073/pnas.1008636108)
found that social influence can narrow human estimate diversity without improving accuracy. Human
estimation groups do not establish the effect for language-model coding reviewers. ymp has a
protocol specification, but no communication harness or result.

## P1 — synthesis versus selection

**Claim.** On an eligible decomposable stratum, coordinated synthesis can add value beyond choosing
the best of two independent candidates under the same total opportunity.

**Observable prediction.** An independently accepted candidate requires at least two attributable,
non-redundant contributions: removing either contribution breaks the declared evidence, while a
blinded best-of-2 selector and one blinded synthesizer given the same artifacts and total budget do
not match the coordinated result.

**Falsifier / stop.** The best individual branch already passes, an ablation changes nothing, the
single synthesizer matches the result, selection is unblinded, or any resource class is unequal.
Sequential/null strata remain expected negative controls.

**Dependencies.** Frozen L4+ strata and oracles; candidate ancestry and contribution ablations;
blinded selection; exact usage and protected-query accounting. The existing mechanism map states
the same rejection boundary
([`map-001`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/research/map-001-mechanism-map.md#L14-L20)).

**Blocking split conflict.** The accepted manifest assigns the only decomposable task to
`development` and the only sequential expected-null task to `transfer`
([`manifest.json`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-v1/manifest.json#L10-L43)).
`W1-EVL-04e` requires the weak cohort across both strata, while `W1-EVL-04f` requires a qualifying
effect to reproduce on the frozen transfer split
([`W1-EVL-04e`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04e.md#L40-L75),
[`W1-EVL-04f`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04f.md#L30-L58)).
Running development only omits the null stratum; running both leaves no same-stratum held-out
decomposable transfer, and a sequential expected-null task cannot reproduce a decomposable positive
effect. The model gate is therefore **STOP**. The accepted corpus is not amended: resumption needs a
prospective, separately frozen scientific decision that makes diagnosis and same-stratum transfer
jointly identifiable without changing observed tasks, arms, seeds, budgets, outcomes, or the frozen
primary comparison.

**Priority and evidence status.** **P1.** [Kim et al. (Nature Machine Intelligence 2026)](https://www.nature.com/articles/s42256-026-01268-y)
provide matched-compute evidence that collaboration varies sharply with task structure and model
capability, including negative sequential-task effects. Their fixed canonical architectures do not
establish synthesis value in ymp. The task packages and oracles exist, but their current split does
not support the planned inference; no selector, ablation harness, admitted observation, or result
exists.

## P2 — bounded adaptive recruitment

**Claim.** Only after fixed-two coordination shows a frozen signal and that signal transfers to one
stronger admitted profile, participant-local recruitment may improve accepted task value or reduce
duplicated work relative to fixed-two and fixed/random recruitment under the same budget.

**Observable prediction.** A participant recruits from the frozen pool in response to a
predeclared unresolved-work signal; the resulting participant, topology change, remaining budget,
duplicate-work cost, and accepted outcome are attributable and outperform both controls on fresh
tasks.

**Falsifier / stop.** No qualifying fixed-two signal, failed or inconclusive stronger-profile
transfer, profile shopping, hidden extra budget, prescribed semantic roles, or no gain over the
controls stops the item. Adaptive recruitment is explicitly outside `weak-diagnostic-v1`; it may
not alter that diagnostic's fixed two-participant coordinated arm.

**Dependencies.** Accepted P1 synthesis signal; the strong-profile gate in
[`W1-EVL-04f`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04f.md#L28-L77);
an accepted model-callable recruitment surface. Recruitment is accepted, but no recruitment
experiment exists
([`W1-PRD-05j.1`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/work/waves/W1/W1-PRD-05/tasks/W1-PRD-05j/subtasks/W1-PRD-05j.1.md#L64-L90)).

**Priority and evidence status.** **P2.** [MANTA](https://arxiv.org/abs/2607.28527) reports bounded
inference-time topology changes informed by collaboration traces and prior structural experience.
Its planner, auditor, role changes, designated aggregator, benchmarks, and recent preprint status do
not establish safe participant-local recruitment or transfer to ymp. No ymp recruitment experiment
or result exists.

## P2 — model-dependent message representation

**Claim.** Communication value depends directionally on sender profile, frozen representation, and
receiver profile rather than on a universally optimal format. The exact hypothesis and typed
envelope are fixed in
[HYP-001](https://github.com/maggnus/ymp/blob/bad509110f7fa1e0f8d7438552878a3f21fff679/ymp-docs/research/hyp-001-model-dependent-communication-representation.md#L14-L64).

**Observable prediction.** Natural, structured, hybrid, and reusable-symbolic renderings of the same
evidence produce preregistered differences in receiver action and independently assessed task value
after full token accounting, with fresh-session replication and separately reported profile
transfer.

**Falsifier / stop.** No receiver action change, no task value, equivalence to direct evidence,
failure in fresh sessions, pair-specific coadaptation, unequal content, or a gain erased by full
accounting rejects or narrows the corresponding claim.

**Dependencies.** P0 reachability; eligible post-transfer intervention episodes; exact profile and
evidence digests; semantic-equivalence fixtures; a frozen representation renderer and complete usage
records.

**Priority and evidence status.** **P2.** [Lowe et al. (AAMAS 2019)](https://www.ifaamas.org/Proceedings/aamas2019/pdfs/p693.pdf)
motivate the signaling/listening separation, while
[AutoForm (Findings of EMNLP 2024)](https://aclanthology.org/2024.findings-emnlp.623/)
reports efficient alternative formats and cross-model transfer. Neither source tests this
directional factorial claim for coding agents. No ymp experiment, harness, or result exists.

## P3 — participant-local management-policy abstraction

**Claim.** A reusable management or workflow policy can improve held-out task value while remaining
participant-local; the trusted kernel need not rank work, assign semantic roles, choose a topology,
or synthesize an answer.

**Observable prediction.** A versioned policy learned only on development tasks is frozen, selected
and executed outside the kernel, then outperforms a no-policy or fixed-policy control on fresh tasks
and profiles at matched total cost. Kernel event fields contain only existing mechanical effects.

**Falsifier / stop.** The gain requires kernel semantic scoring or dispatch, a hidden permanent
manager, post-hoc workflow edits, benchmark-specific role choreography, unequal search cost, or
fails held-out profile/task transfer.

**Dependencies.** Positive lower-priority mechanism evidence; a versioned participant-local policy
artifact; held-out development and transfer splits; external acceptance and full search-cost
accounting. The protocol deliberately excludes a global semantic scheduler
([`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/PROTOCOL.md#L595-L614)).

**Priority and evidence status.** **P3.** [ADAS](https://arxiv.org/abs/2408.08435),
[AFlow](https://arxiv.org/abs/2410.10762), and the
[Darwin Gödel Machine](https://arxiv.org/abs/2505.22954) show that code-represented agent or
workflow search can discover useful designs, sometimes with model/task transfer. They use
benchmark-guided meta-search, archives, fixed operators, or substantial extra search; none proves
the ymp boundary or a matched-cost participant-local policy. No ymp abstraction or result exists.

## P3 — cross-run learning without primary reuse or oracle leakage

**Claim.** Development-verified abstract policies or structural summaries may improve future fresh
runs without reusing primary candidates, messages, tasks, seeds, protected diagnostics, or
participant-private reasoning.

**Observable prediction.** Every retained item has development provenance and exogenous acceptance;
the learning rule and archive freeze before a disjoint transfer or primary run. At matched budget,
the frozen archive improves accepted outcome or calibrated abstention over no-memory, and the effect
survives fresh tasks and profile substitution.

**Falsifier / stop.** Any primary artifact, seed, message, task instance, protected oracle content,
diagnostic leakage, or self-authored acceptance enters the archive; the archive changes after
outcome reveal; the gain vanishes under disjoint tasks/profiles; or the external oracle contradicts
the retained self-score. The affected run is invalid, not a failed candidate.

**Dependencies.** An accepted P3 management abstraction; cryptographically separated development,
transfer, and primary lineages; a frozen retention/redaction policy; limited oracle disclosure; and
an acceptance signal outside the learner's control. The current contract already treats adaptive
holdout reuse as a threat
([`PROJECT-CONTRACT.md`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/PROJECT-CONTRACT.md#L124-L137)).

**Priority and evidence status.** **P3.** DGM's archive and transfer results and MANTA's prior
structural experience make cross-run abstraction plausible, but do not establish leakage-free ymp
learning. [SEAL](https://arxiv.org/abs/2607.24300) directly cautions that self-authored tests can
stay strong while deployment performance regresses and finds benefit from a sealed exogenous audit;
its heuristic-game setting does not validate ymp's oracle or archive. No archive design, harness,
authorized experiment, or result exists.

## Queue-wide stop rule

An earlier negative result narrows or stops its dependants; it is not repaired by changing a frozen
arm, task, seed, budget, metric, protocol, or kernel semantic. Literature supplies hypotheses and
counterexamples only. A later record must preserve source-specific transfer limits and distinguish
specified design, executable evidence, observation, and causal conclusion.

# Coordination research queue

## Status and boundary

`MAP-002` orders unresolved scientific questions by the earliest evidence needed to interpret later
work. Priorities are readiness classes, not product scheduling or kernel policy. Every item is
specified only: none is an experiment, harness, accepted causal result, or authorization to spend
model budget. It is curated under `SCI-QUEUE-01` against baseline
[fbf52f7](https://github.com/maggnus/ymp/commit/fbf52f763f0dc2a3badff3e21fae33dc6469cd87).

This record changes no current arm, task set, seed, budget, metric, frozen protocol, or kernel
semantics. In particular, `weak-diagnostic-v1` and the frozen primary comparison remain unchanged.

## P0 — pull-transport causal reachability

**Claim.** The accepted `publish`/`read_board` seam can place one authorized message from participant
A in participant B's available context early enough for a predeclared later B action. This is only
transport readiness, not listening.

**Observable prediction.** One isolated, bounded trace must contain, in order:

```text
A MessagePublished -> DeliveryRecorded to B -> predeclared later B action -> honest terminal
```

The same fixture must hide the message from an unauthorized audience member. B performs only the
predeclared finite read opportunity; repeated polling, an unbounded retry loop, or message traffic
that keeps the run alive fails the reachability gate. `DeliveryRecorded` proves availability and
cursor advance, never reading, belief, influence, or task value.

Before the first participant call, the exact installed runtime, profile, route, and effective tool
binding must pass a zero-model tool-host fixture that proves bounded inspection and mutation of its
isolated workspace. Authentication, model availability, and filesystem creation alone are not this
proof.

**Falsifier / stop.** If B can yield before observing the message and no already named control wake
can resume B, the trace stops and returns for explicit core/protocol review. A board publication is
not assumed to wake a yielded participant. Absence of the ordered trace, unauthorized delivery,
polling pressure, or a dishonest terminal also stops all downstream communication intervention.
Failure of the exact-route tool-host fixture stops before participant output; a later repair requires
a newly frozen budget and a fresh run rather than continuation of the failed pilot.

**Dependencies.** Accepted collaboration tools; an authorized two-participant managed path; a
zero-model exact-route tool-host fixture; a predeclared B action and finite invocation/read schedule.
The board emits
[`MessagePublished` and `DeliveryRecorded`](https://github.com/maggnus/ymp/blob/fbf52f763f0dc2a3badff3e21fae33dc6469cd87/ymp-rust/crates/ymp-board/src/protocol.rs#L253-L274),
and `read_board` records delivery only after B pulls it
([`ymp-application`](https://github.com/maggnus/ymp/blob/fbf52f763f0dc2a3badff3e21fae33dc6469cd87/ymp-rust/crates/ymp-application/src/lib.rs#L2036-L2094)).
The accepted kernel's enumerated
[`WakeCondition`](https://github.com/maggnus/ymp/blob/fbf52f763f0dc2a3badff3e21fae33dc6469cd87/ymp-rust/crates/ymp-domain/src/commitment/invocations.rs#L61-L85)
has neither board-publication nor direct-message delivery, so a control wake must be named rather
than inferred.

**Priority and evidence status.** **P0.** The publish/read seam and audience filtering are accepted
repository evidence
([`W1-COR-03z`](https://github.com/maggnus/ymp/blob/fbf52f763f0dc2a3badff3e21fae33dc6469cd87/ymp-docs/work/waves/W1/W1-COR-03/tasks/W1-COR-03z.md#L79-L105));
the isolated causal-readiness trace, action link, and terminal observation do not exist.

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
([`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/fbf52f763f0dc2a3badff3e21fae33dc6469cd87/ymp-docs/PROTOCOL.md#L236-L242)).

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
([`map-001`](https://github.com/maggnus/ymp/blob/fbf52f763f0dc2a3badff3e21fae33dc6469cd87/ymp-docs/research/map-001-mechanism-map.md#L14-L20)).

**Priority and evidence status.** **P1.** [Kim et al. (Nature Machine Intelligence 2026)](https://www.nature.com/articles/s42256-026-01268-y)
provide matched-compute evidence that collaboration varies sharply with task structure and model
capability, including negative sequential-task effects. Their fixed canonical architectures do not
establish synthesis value in ymp. No eligible task set, selector, ablation harness, or result exists.

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
[`W1-EVL-04f`](https://github.com/maggnus/ymp/blob/fbf52f763f0dc2a3badff3e21fae33dc6469cd87/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04f.md#L28-L77);
an accepted model-callable recruitment surface. At this baseline, recruitment wiring is active but
not accepted
([`W1-PRD-05j.1`](https://github.com/maggnus/ymp/blob/fbf52f763f0dc2a3badff3e21fae33dc6469cd87/ymp-docs/work/waves/W1/W1-PRD-05/tasks/W1-PRD-05j/subtasks/W1-PRD-05j.1.md#L1-L18),
[`current state`](https://github.com/maggnus/ymp/blob/fbf52f763f0dc2a3badff3e21fae33dc6469cd87/ymp-docs/work/waves/W1/W1-PRD-05/tasks/W1-PRD-05j/subtasks/W1-PRD-05j.1.md#L81-L89)).

**Priority and evidence status.** **P2.** [MANTA](https://arxiv.org/abs/2607.28527) reports bounded
inference-time topology changes informed by collaboration traces and prior structural experience.
Its planner, auditor, role changes, designated aggregator, benchmarks, and recent preprint status do
not establish safe participant-local recruitment or transfer to ymp. No ymp recruitment experiment
or result exists.

## P2 — model-dependent message representation

**Claim.** Communication value depends directionally on sender profile, frozen representation, and
receiver profile rather than on a universally optimal format. The exact hypothesis and typed
envelope are fixed in [HYP-001](hyp-001-model-dependent-communication-representation.md).

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
([`PROTOCOL.md`](https://github.com/maggnus/ymp/blob/fbf52f763f0dc2a3badff3e21fae33dc6469cd87/ymp-docs/PROTOCOL.md#L595-L614)).

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
([`PROJECT-CONTRACT.md`](https://github.com/maggnus/ymp/blob/fbf52f763f0dc2a3badff3e21fae33dc6469cd87/ymp-docs/PROJECT-CONTRACT.md#L124-L137)).

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

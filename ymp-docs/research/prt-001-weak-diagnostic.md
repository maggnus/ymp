# Weak-participant diagnostic protocol

`weak-diagnostic-v1` is a development experiment for making a coordination effect, or its absence,
visible before stronger executors approach an oracle ceiling. It never replaces the frozen primary
comparison, its strong-single-agent baseline, or its inference thresholds.

## Design

L1-L3 are prerequisite profile and oracle calibration only; they never enter this protocol as arm
observations. `W1-EXP-01e` separately freezes a held-out L4+ task set containing decomposable and
sequential/null tasks. Its exact task identifiers, count, contract and oracle digests freeze before
any model call under `weak-diagnostic-v1`; this protocol does not invent them.

One exact weak runtime/profile cohort is completed before another family begins. Every frozen L4+
task receives three fresh repetitions and three matched-budget conditions per repetition:

1. one weak participant;
2. two independent weak participants plus a blinded best-of-2 selector; and
3. two locally coordinated weak participants.

The complete schedule contains three conditions times three repetitions times the task count that
`W1-EXP-01e` froze. Conditions are ordered by SHA-256 over cohort, frozen task identifier,
repetition, and arm rather than by an operator choice. Their seeds retain the separate namespace
`ymp-weak-diagnostic-v1`; no frozen primary seed is consumed. The profile candidates and evidence
boundary are defined in
[cal-001-calibration.md](cal-001-calibration.md).

## Matched budget

Each condition receives three diagnostic quanta of five model requests: at most 15 calls,
1,966,080 input tokens, 1,966,080 cached-input tokens, 245,760 output tokens, 245,760 reasoning
tokens, 9,000,000 milliseconds, parallelism at most two, and exactly one protected verification
query. Profile and route, every token class, cost and currency, wall time, infrastructure failures,
retries, and in-flight excess remain separate recorded fields.

These are reduced diagnostic budgets derived from the frozen quantum. The primary `single_strong`,
best-of-11, twelve-quantum, five-repetition comparison remains unchanged in the
[pinned study protocol](https://github.com/maggnus/ymp/blob/066d3b8b616788952a0592e0c353da0e3f45ba68/ymp-rust/tools/ymp-corpus/corpus/study/PROTOCOL.md#L19-L88).

## Disposable execution boundary

Every condition receives a new root:

```text
<cohort>/<task-id>/<repetition>/<arm>/
  project/
  home/
  ymp-home/
  tmp/
  build/
  export/
```

Participant workspaces are separate children. Protected verification runs in a sibling directory
unavailable to participants. The source worktree and the operator's real `~/.ymp` are never used.

## Selection and observations

The single and coordinated conditions commit one exact candidate digest. In the independent arm, a
selector sees only blinded candidate artifacts and public evidence, commits its choice before arm
identity or producer rationale is revealed, and then spends the arm's one protected query. Every
condition emits its manifest identity, assignment, usage vector, lifecycle and collaboration
events, candidate digests, selector commitment where applicable, verifier evidence, and terminal
outcome.

The diagnostic stops without selective reruns when profile admission, a negative control, oracle
separation, seed assignment, order, budget conservation, or usage evidence fails. Infrastructure
failure is never recoded as candidate failure.

## Diagnostic decision

After the task-set freeze exists, the separate diagnostic-runner owner must freeze an executable
aggregation and minimum diagnostic signal before any model call; this record does not invent a
numeric rule before the task count exists. Proceed to a stronger admitted profile only if every
scheduled condition is compliant, coordination repeatedly exceeds both controls under that frozen
rule, and accepted candidates contain at least two attributable non-redundant contributions.
Otherwise record an explicit negative stop. A weak-only result cannot authorize message
interventions. Three repetitions characterize development variation and never become a statistical
product claim.

## Current implementation boundary

Implemented now: preparation and protected verification of L1-L3, their seeded negative controls,
the managed single-participant path, persistent obligations, candidates, collaboration board, and
agent-facing collaboration tools.

Missing before execution: the frozen held-out L4+ task set, a model-callable recruitment path,
blinded best-of-2 selection, the diagnostic manifest/scheduler/compliance record, weak-profile
admission and complete enforced budget accounting. The immediate product path is therefore:

```text
accepted collaboration tools → agent recruitment → two-participant run
→ held-out task-set freeze → diagnostic scheduler and compliance dry run
```

The protocol was derived on 2026-09-01 under standing Sol max scientific review. Its next executable
decision is a Sol-authored diagnostic scheduler only after the two-participant path and held-out
task-set freeze exist; before any model call, that scheduler must pass a deterministic fake-runtime
compliance run. TUI rendering is a product and observatory obligation, not a prerequisite for this
headless comparison. The complete stage boundary is recorded in
[rdr-001-evaluation-stage-boundaries.md](rdr-001-evaluation-stage-boundaries.md).

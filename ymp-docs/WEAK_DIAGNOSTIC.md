# Weak-participant diagnostic protocol

`weak-diagnostic-v1` is a development experiment for making a coordination effect, or its absence,
visible before stronger executors approach an oracle ceiling. It never replaces the frozen primary
comparison, its strong-single-agent baseline, or its inference thresholds.

## Design

One exact weak runtime/profile cohort is completed before another family begins. A cohort runs the
existing calibration cases in order `L1-line-endings` → `L2-size-parser` → `L3-command-ledger`, with
three fresh repetitions per case and three matched-budget conditions per repetition:

1. one weak participant;
2. two independent weak participants plus a blinded best-of-2 selector; and
3. two locally coordinated weak participants.

The 27 conditions are ordered by SHA-256 over cohort, case, repetition, and arm rather than by an
operator choice. Their seeds use the separate namespace `ymp-weak-diagnostic-v1`; no frozen primary
seed is consumed. The profile candidates and evidence boundary are defined in
[CALIBRATION.md](CALIBRATION.md).

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
<cohort>/<case>/<repetition>/<arm>/
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

Proceed to stronger profiles only if all 27 conditions are compliant and coordination beats both
controls in at least two of the three case blocks, produces at least two attributable
non-redundant contributions, and survives preregistered message removal or neutral replacement at
the same total budget. Otherwise record that this cohort supplies no diagnostic support for the
coordination mechanism. Three repetitions are a development signal, not a statistical product
claim.

## Current implementation boundary

Implemented now: preparation and protected verification of L1–L3, their seeded negative controls,
the managed single-participant path, persistent obligations, candidates, and collaboration board.

Missing before execution: a multi-participant product path, agent-facing collaboration tools,
blinded best-of-2 selection, the diagnostic manifest/scheduler/compliance record, weak-profile
admission and complete enforced budget accounting. The immediate product path is therefore:

```text
board payload seam → agent collaboration tools → TUI messages → two-participant run
→ diagnostic scheduler and compliance dry run
```

The protocol was derived read-only on 2026-09-01 by the standing Sol max scientific researcher. Its
next executable decision is a Sol-authored diagnostic scheduler only after the two-participant path
exists; before any model call, that scheduler must pass a deterministic fake-runtime compliance run.

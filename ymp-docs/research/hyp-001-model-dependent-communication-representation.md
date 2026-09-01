# Model-dependent communication representation

## Status and boundary

`HYP-001` is a specified-only, directional hypothesis. It does not assert that one message
representation is universally best, or that a delivered message was read. No experiment manifest,
harness, treatment assignment, eligible episode set, model observation, or result exists.
It is curated under `SCI-QUEUE-01` against baseline
[59288d1](https://github.com/maggnus/ymp/commit/59288d11b400a00ee22c8cc36f9a80e4077aea1c).

This record changes no current arm, task set, seed, budget, metric, frozen protocol, or kernel
semantics. Any later test needs its own preregistration and executable compliance check.

## Typed design envelope

The envelope below fixes the factors and observations that a future design must name. It is a
research-manifest shape, not a new product record or agent API.

| Field | Fixed meaning |
|---|---|
| `task` | Frozen task identifier, stratum, contract, environment, and protected-oracle digests. |
| `sender_profile` | Exact sender runtime, harness and prompt policy, model route, model snapshot, and coordination-tool binding. |
| `receiver_profile` | The same exact identity tuple for the receiver; a profile label alone is insufficient. |
| `direction` | Ordered `sender_profile -> receiver_profile`; reversing the two profiles is a different cell. |
| `representation` | Exactly one of `natural`, `structured`, `hybrid`, or `reusable_symbolic`, under a versioned and frozen rendering rule. |
| `evidence_payload` | Digest of the same underlying public evidence and semantic proposition used across representation contrasts. |
| `treatment` | Original represented message, absence, size-matched neutral payload, eligible shuffle, or direct delivery of the same evidence. |
| `exposure` | Fresh sender and receiver session identifiers, prior representation exposure, pair history, and transfer stratum. |
| `receiver_action` | One externally observable action or action class and its measurement window, declared before treatment reveal. |
| `task_value` | Independently accepted outcome, calibrated abstention, recovery, or cost effect; influence alone is not value. |
| `accounting` | Sender generation and conversion, publication and delivery, receiver context and inference, all input, cached-input, output and reasoning tokens, calls, retries, wall time, monetary cost, communication bytes, and verification use. |
| `terminal` | Honest `accepted`, `exhausted`, `abstained`, `cancelled`, or `infrastructure_error`, kept distinct from candidate failure. |

The representation families are:

- **natural** — bounded natural-language prose without a required machine-readable field layout;
- **structured** — a frozen schema or grammar whose required fields carry the message content;
- **hybrid** — the frozen structured fields plus bounded natural-language explanation;
- **reusable symbolic** — a symbol table, grammar, or codebook learned or selected only on
  development tasks, versioned by digest, then frozen before transfer. A task-specific shorthand
  invented after seeing the evaluation task is not reusable symbolic representation.

Representation contrasts are admissible only when the underlying evidence and proposition are
held fixed. Otherwise content selection is confounded with representation.

## Directional claim

For at least one predeclared task stratum, the effect of representation on a receiver's later action
and task value depends on the ordered interaction

```text
sender profile × representation × receiver profile.
```

The claim requires both a preregistered change in the receiver action distribution and positive
task value after full accounting. A representation that is effective from profile A to profile B
need not be effective from B to A, between two other profiles, or for a stronger receiver. Message
length, fluency, agreement, and a delivery receipt are not outcomes.

The first replication tier uses fresh sessions of the same ordered profile pair with no shared
conversation history. A separate transfer tier freezes the representation before substituting an
unseen admitted sender profile, receiver profile, or both. Failure in the first tier rejects a
repeatable pair effect. Success only in the first tier records a pair-specific result; it does not
support profile transfer or a reusable representation claim.

## Counterhypotheses and falsifiers

1. **Direct-evidence equivalence.** Any benefit comes from exposing the receiver to the underlying
   evidence, not from another participant or its representation. If direct delivery matches the
   participant message under the same total budget, separate-participant value is rejected.
2. **Pair-specific coadaptation.** The apparent code works only because a repeated sender-receiver
   pair accumulated shared history. Failure in fresh sessions, reversed direction, or the frozen
   profile-transfer tier bounds the finding to that pair and history.
3. **Compression-only advantage.** Structured or symbolic messages save visible communication
   tokens but shift cost into format induction, conversion, retries, or receiver inference. If the
   advantage disappears under full token and cost accounting, the task-value claim is rejected.
4. **Content-selection confounding.** Different formats carry different evidence or propositions.
   A contrast without a common evidence digest is invalid rather than negative.

No receiver action change means no positive listening. An action change without independently
accepted outcome, calibrated abstention, recovery, or cost improvement means influence without task
value. Post-hoc representation choice, profile shopping, treatment leakage, a weak oracle, or
unverifiable accounting invalidates the affected contrast.

## Dependencies

- The P0 reachability gate in
  [map-002-coordination-research-queue.md](https://github.com/maggnus/ymp/blob/bad509110f7fa1e0f8d7438552878a3f21fff679/ymp-docs/research/map-002-coordination-research-queue.md#L14-L77)
  must first show bounded pull reachability while keeping participant publication, read, yield, and
  action choices observable rather than forced.
- Each exact sender and receiver profile must pass the two-stage tool-use admission boundary in
  [RUN-001](https://github.com/maggnus/ymp/blob/bad509110f7fa1e0f8d7438552878a3f21fff679/ymp-docs/research/run-001-manual-file-communication-pilot.md#L89-L113).
  A zero-model compatibility check cannot
  substitute for the separately capped model nonce round trip or the existing lifecycle,
  isolation, cancellation, usage, and cost requirements
  ([`cal-001`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/research/cal-001-calibration.md#L61-L77)).
- Eligible intervention episodes and predeclared receiver actions belong to
  [`W1-EVL-04b`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04b.md#L28-L67).
- Message intervention remains downstream of stronger-profile transfer under the accepted
  [stage boundary](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/research/rdr-001-evaluation-stage-boundaries.md#L42-L51),
  and the split conflict recorded in
  [MAP-002](https://github.com/maggnus/ymp/blob/bad509110f7fa1e0f8d7438552878a3f21fff679/ymp-docs/research/map-002-coordination-research-queue.md#L123-L135)
  must be resolved prospectively before that transfer can identify a same-stratum effect.
- Exact profile admission, frozen task and oracle identities, semantic-equivalence fixtures, and
  complete accounting must exist before a design can freeze.

## Evidence and transfer limits

[Lowe et al. (AAMAS 2019)](https://www.ifaamas.org/Proceedings/aamas2019/pdfs/p693.pdf)
separate sender-correlated signaling from receiver-sensitive listening and motivate message
interventions. Their trained reinforcement-learning agents and simple games do not establish the
effect for pretrained coding agents or long-horizon actions.

[Chen et al. (Findings of EMNLP 2024, AutoForm)](https://aclanthology.org/2024.findings-emnlp.623/)
report efficiency, communication-token, and cross-model transfer results for model-selected
non-natural formats. That study does not identify a causal directional
sender-profile-by-representation-by-receiver-profile effect under ymp's full resource accounting
or protected acceptance oracle.

[Kim et al. (Nature Machine Intelligence 2026)](https://www.nature.com/articles/s42256-026-01268-y)
show large task-, architecture-, and capability-dependent variation under matched compute. Their
architectures and benchmarks motivate interaction and null strata, but do not test the four
representation families defined here.

[RUN-001](https://github.com/maggnus/ymp/blob/bad509110f7fa1e0f8d7438552878a3f21fff679/ymp-docs/research/run-001-manual-file-communication-pilot.md#L3-L56)
exercised no representation cell: both A
calls ended before publication, B and C were never launched, and the board remained unchanged. It
falsifies readiness of that exploratory runtime/tool-host binding but supplies no signaling,
delivery, receiver-action, listening, task-value, or representation evidence. The current ymp
record therefore supports only the specification above. Evidence status: **no HYP-001 experiment,
harness, or result**.

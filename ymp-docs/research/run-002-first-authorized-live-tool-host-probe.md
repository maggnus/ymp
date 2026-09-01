# First authorized live tool-host probe

## Status and evidentiary boundary

`RUN-002` records the first authorized live no-task-output tool-host probe on 2026-09-01. The
execution used candidate
[2ab84f4449d44ea103e0bca820929daaab9ab057](https://github.com/maggnus/ymp/commit/2ab84f4449d44ea103e0bca820929daaab9ab057)
and ended after 47.65 seconds with
`ToolHostProbeError::RuntimeFailed ProcessExit`. Its scientific classification is
**`infrastructure-invalid / failure phase indeterminate`**. The separate probe reservation is
spent. The execution was stopped after this terminal outcome and was not retried, continued, or
reclassified as a model failure.

The later accepted deterministic implementation and the unchanged live STOP are recorded by
[W1-EVL-04n](https://github.com/maggnus/ymp/blob/c414f19fd292d17f1ce4acc78b66a0952be94d7f/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04n.md#L110-L190).
This run is an execution record only. It changes no task, arm, seed, budget, outcome, protocol, or
admission decision.

## Authorized command and isolation

Exactly one live probe command was authorized. The ignored
`one_live_controller_attested_codex_tool_host_probe` harness spawned the product command:

```text
ymp --data-root <isolated-application-store> internal tool-host-probe \
  --admission-manifest-digest d354f20c8482cd5df7e33fab70dcd267befb621ef09647d430dc40f3924b8ea2
```

The exact command construction is pinned in the
[candidate harness](https://github.com/maggnus/ymp/blob/2ab84f4449d44ea103e0bca820929daaab9ab057/ymp-rust/crates/ymp-cli/tests/live_tool_host_probe.rs#L26-L108).
It used `gpt-5.6-terra`, low reasoning, route `openai_responses_chatgpt`, prompt policy
`ymp-codex-low-v2`, and `codex-cli 0.151.0`. The harness supplied separate project, `HOME`,
`CODEX_HOME`, `YMP_HOME`, `TMPDIR`, build, export, and Application-store locations; the
controller then created a fresh short child root below the isolated temporary directory. The real
repository, current directory, and real `~/.ymp` were not execution roots.

The absolute disposable-root name was not retained before deletion. Processes and the disposable
root were cleaned after the failure. Those facts establish isolation and cleanup, but the deleted
root cannot be reproduced or independently re-inspected from this record.

## Frozen identity and reservation

| Evidence | Exact value |
|---|---|
| Candidate commit | `2ab84f4449d44ea103e0bca820929daaab9ab057` |
| `ymp` executable SHA-256 | `f3707ba48d784047c992c27f49c921f3c208a89f23225a67bd7dbf70d915a9d7` |
| Codex executable SHA-256 | `98491713ffb196061003ee148636e743997cc31d76144ba7c53462269896891d` |
| Admission-v2 manifest SHA-256 | `d354f20c8482cd5df7e33fab70dcd267befb621ef09647d430dc40f3924b8ea2` |
| Codex compatibility-contract SHA-256 | `65894b25843beae807ef3337867ba4886e8c8e392fcf637678f16be309034e77` |

The fingerprints are preserved in the accepted
[W1-EVL-04n evidence](https://github.com/maggnus/ymp/blob/c414f19fd292d17f1ce4acc78b66a0952be94d7f/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04n.md#L175-L190);
the compatibility and route bindings and the separately frozen stage-two vector are in the
[v2 manifest](https://github.com/maggnus/ymp/blob/c414f19fd292d17f1ce4acc78b66a0952be94d7f/ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-admission-v2/manifest.json#L12-L35).
The reservation allowed at most one model request, 32,768 input tokens, 32,768 cached-input tokens,
1,024 output tokens, 1,024 reasoning tokens, and 120,000 milliseconds
([stage two](https://github.com/maggnus/ymp/blob/c414f19fd292d17f1ce4acc78b66a0952be94d7f/ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-admission-v2/manifest.json#L110-L121)).
The reservation remains spent even though actual provider expenditure could not be recovered.

## Observed outcome

The sole live command returned `ProcessExit` after 47.65 seconds. It produced none of the evidence
required for a successful infrastructure admission:

- no `Completed` terminal and no terminal `Usage`;
- no tool event or complete untrusted `ToolHostProbeTrace`;
- no controller filesystem read-back;
- no handle or `AttestedToolHostProbe`;
- no `model_ready=true` transition.

Whether a provider request was sent or accepted is unknown. Input, cached-input, output and reasoning
usage, in-flight request state, monetary cost and cost availability are also unknown; they are not
zero. The pre-repair supervisor discarded the runtime failure's intermediate `Usage` and
`DiagnosticSummary`, so elapsed time and `ProcessExit` cannot localize the failure to
configuration, provider, model, MCP transport/server, or controller
([source finding](https://github.com/maggnus/ymp/blob/c414f19fd292d17f1ce4acc78b66a0952be94d7f/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04n.md#L144-L180)).
Deterministic code and mutation checks passed, but they establish only the fail-closed mechanics and
do not repair or reinterpret the live observation.

## Scientific classification

This run supplies evidence only that the exact authorized live path failed without enough retained
observability to locate the infrastructure layer. It is not evidence that the model can or cannot
use workspace tools. With no successful tool event or read-back, it establishes no tool-host
reachability. With no sender, receiver, board delivery, receiver action, controlled intervention, or
task outcome, it establishes no communication, listening, task value, or self-organization.

No artifact from this run may satisfy `W1-EVL-04m`, an experimental arm, or a later retry. The
absence of usage and provider evidence is a validity defect, not a zero-cost or zero-request result.

## Minimum diagnostic gate and STOP

Before any separately authorized second run, [W1-EVL-04r](https://github.com/maggnus/ymp/blob/c414f19fd292d17f1ce4acc78b66a0952be94d7f/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04r.md#L28-L111)
must be accepted without a live call. Every failed probe must durably preserve:

- manifest, runtime, route, compatibility, executable and probe-transport identities;
- probe, invocation, reservation and replay identities;
- the last accepted event type, identifier and sequence;
- exit status or signal, duration, complete token classes, in-flight request count, cost
  availability, and bounded Codex/MCP diagnostics as only `{sha256, bytes, truncated}`;
- an observed failure phase and provider state chosen from `not_started`,
  `turn_started_unconfirmed`, `provider_responded`, or explicit `unknown`;
- the absence of attestation and the process/root cleanup result.

Missing observations remain `unknown`; they are never inferred from duration or process exit. A
failure record grants no refund, retry, attestation, or model readiness.

Only [W1-EVL-04s](https://github.com/maggnus/ymp/blob/c414f19fd292d17f1ce4acc78b66a0952be94d7f/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04s.md#L28-L90)
may later authorize one new attempt, after all deterministic diagnostic negatives pass and a fresh
probe id, nonce namespace, disposable root and stage-two budget are frozen. That attempt requires a
new explicit budget and authorization. Any failed preflight prevents the provider call; any
unlocalized live failure remains an infrastructure STOP and cannot be selectively retried.

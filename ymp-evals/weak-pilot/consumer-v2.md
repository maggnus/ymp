# Frozen-manifest consumer, version 2

This supersedes the permanently refusing native command in preparation commit
15529e6. The original task variants, reference answers, negative controls and
version-1 protocol specifications remain unchanged. No model trial or calibration
is authorized by this document, a generated manifest, or a passing fixture.

The additional `ymp-weak-pilot` bin remains a research component of the existing
evaluation crate. No product API or dependency is added. Native execution uses
the installed provider's authentication; no credentials or native home are copied.

## One executable path

`consumer_manifest.py` freezes schema-2 manifests with exact models, native catalog,
fixed low policies, Config limits, source revision, file/executable digests,
condition order, separate private/solving directories and blinded submission IDs.
`ymp-weak-pilot scripted` and `native` consume the same structure and execute the
same staging, provider, selection, sealing, result and external-scoring code.

Raw solo and independent conditions use `Group::invoke`, production Store
admission and `NativeExecutionBackend` through the existing provider boundary.
All independent contexts start fresh, without MCP or inherited native conversation.
Cooperation uses the actual Engine with fixed two/three-member rosters, one fixed
weak model, ordinary independent acceptance, memory off and reputation influence
off. The allocation algorithm is unchanged. The hidden scorer is never an in-run
acceptance check. Actual participation, rejection, confirmation and external
correctness remain separate observations.

Private controller directories and solving directories are disjoint. Each
request's permission profile denies the repository/evaluator, controller, native
home and every other solving directory. Frozen user-home and temporary-data roots
also deny prior scripted/calibration outputs and live application data, with the
native, more-specific current-workspace grant verified by a parent-deny canary.
Only its own current working directory
receives the original read/write access. Native state/authentication is still
managed by the provider host; model commands cannot read it. Public tests, final Python scoring and direct repair-probe calls use the same
macOS sandbox-exec boundary. Candidate processes see only copied public tests or
function names/arguments and their own copied files, not expected answers, other
candidates, protected files, parent-directory contents or inherited credentials.
The public-only selector, staging and sealing share the condition's absolute deadline. External
scoring occurs only after freezing and has its own short deterministic timeout.

All emitted ProviderEvents are retained with stream, actor and native context
bindings. Store usage snapshots replace prior snapshots; cache/reasoning are not
added twice. Errors, cancellation and incomplete usage retain their journals.
Measurement validity and task outcome are separate. Wrong answers, missing
artifacts, negative Engine review, fully accounted budget refusal, underused
rosters and a post-invocation deadline remain measured outcomes; later conditions
continue. Unknown/partial usage, contradictory controls, a broken execution
boundary, unexpected actors or unverified termination stop the phase. Only those
invalid measurements leave later cells `not_started`. Calibration continuation
uses measurement validity, never universal answer correctness. Hidden scores do
not choose an independent candidate or authorize repair.

## Verified native controls and the narrow adapter

The installed Codex 0.154.0 interprets the documented subagent/memory switches.
Real command canaries confirm named-profile read/write/deny. Its legacy sandbox
parameter overrides named permissions, so the trial includes a small stdio
envelope: it replaces only `thread/start/resume.sandbox` with the corresponding
named `permissions` profile and verified controls. Model, effort, prompt, IDs,
responses and usage are preserved. Read-only never becomes writable.

The envelope reads only inherited MCP **names** from a metadata-only config query
and disables those servers. The exact current Engine MCP endpoint is preserved
and enabled; no foreign endpoint is added. Full config responses, native histories
and credentials are neither recorded nor copied. The metadata query has no model
turn and remains inside the condition's process group/deadline.

See the [native controls report](../../ymp-docs/evidence/ymp-201/consumer-v2/native-controls.md)
for official sources, exact settings, canaries, the rejected legacy/outer-sandbox
alternatives and preserved failing controls. Transport retries and multiple API
requests within one native agent loop do not create extra participants by
themselves. Unbounded connection retries are disabled; observed accounting and
the shared deadline remain mandatory. No hard billed-token ceiling is claimed.

## Commands and authority

Build the existing evaluation package after the accepted P0 integration:

```sh
cargo build -p ymp-eval-driver --bin ymp-weak-pilot
python3 ymp-evals/weak-pilot/consumer_manifest.py --runner target/debug/ymp-weak-pilot --phase protocol-e2e --output /tmp/ymp201-scripted-controller --workspaces /tmp/ymp201-scripted-work > /tmp/ymp201-scripted.json
target/debug/ymp-weak-pilot scripted --manifest /tmp/ymp201-scripted.json
```

Both destination parents must exist; the destinations themselves must be fresh.
Use the corresponding executable path when using an external Cargo target.
Scripted manifests can execute only the checked-in local protocol fixture and
only `fixture-*` models. Their outputs are marked `protocol-fixture`, never native
measurements. They contain no fabricated owner approval. The fixture intentionally
supplies synthetic answers; it proves the complete consumer, not model quality.

Prepare two reviewable native proposals after the final combined source is fixed:

```sh
python3 ymp-evals/weak-pilot/consumer_manifest.py --runner target/debug/ymp-weak-pilot --phase preparation-calibration --output /tmp/ymp201-calibration-controller --workspaces /tmp/ymp201-calibration-work > /tmp/ymp201-calibration.json
python3 ymp-evals/weak-pilot/consumer_manifest.py --runner target/debug/ymp-weak-pilot --phase measured-pilot --prerequisite-manifest /tmp/ymp201-calibration.json --output /tmp/ymp201-pilot-controller --workspaces /tmp/ymp201-pilot-work > /tmp/ymp201-pilot.json
```

These commands spend no model quota and do not approve anything. Actual commands
require an out-of-band owner authorization record supplied by the parent:

```sh
target/debug/ymp-weak-pilot native --manifest /tmp/ymp201-calibration.json --approval /absolute/calibration-approval.json
target/debug/ymp-weak-pilot native --manifest /tmp/ymp201-pilot.json --approval /absolute/pilot-approval.json --calibration-report /tmp/ymp201-calibration-controller/run.json
```

Approval schema: `schema_version: 2`, `authority: owner`, a nonempty actual
`reference` and `approved_at`, exact `phase`, `scope: one_phase_once`, and one
`manifest_sha256` string. Each record covers one bounded phase once. It is an
operator input outside solving workspaces, never agent metadata. After validation
and approval, an atomic durable marker is created in the manifest's private
`approval_ledger` before outputs or providers start. Output cleanup never removes
that marker or renews authority; failed and interrupted phases keep it too.
The default ledger is `~/.local/state/ymp201/phase-approvals`, outside disposable
outputs. It is not created by proposal generation or scripted runs. No completed
approval is included. Absence, mismatch or an existing marker refuses native work. The conditional pilot additionally
requires the completed native calibration report bound to its frozen prerequisite.
A scripted report cannot satisfy that condition.

Native manifests must descend from accepted P0 base **1c17f4e**; a stale trial
against the earlier main is rejected. The pinned source commit must be an ancestor
of the checkout; documentation/evidence-only descendants are allowed, while all
frozen consumer/fixture bytes, the executable digest, and accepted product bytes
must still match. This allows a manifest to be committed without invalidating its
own source reference. Checksum or scope failures are concrete refusals.

## Proposed calibration and conditional pilot

These are administrative spending/stop proposals for owner review, not scientific
sample-size or performance estimates. Synthetic token counts do not justify them.
The larger headroom avoids intentionally constraining an eight-phase Engine
workflow to the former 12,000-token/120-second preparation example.

| Phase | Outcomes | Per-outcome observed tokens | Outer invocations | Whole condition / one turn |
| --- | --- | --- | --- | --- |
| Preparation calibration | 6 | 80,000 | 12 | 480 s / actual remaining condition time |
| Conditional measured pilot | 12 | 160,000 | 16 | 900 s / actual remaining condition time |

Calibration runs both solo models on both **preparation** tasks, then cooperation-2
and cooperation-3 on preparation repair. It verifies real accounting, context/
participant binding, reliable outcome capture and available budget headroom before the
measured variant is exposed. Independent orchestration uses already-checked raw
turns and deterministic selection; it does not add a separate calibration catalog.

Every condition shares one allowance, one application attempt, scheduling ceiling
two, and fixed low. Solo receives one complete native tool loop with the actual
remaining condition time. Independent attempts run in frozen ordinal order under
one absolute deadline; later attempts receive only its remainder. The config's
per-turn timeout equals the whole-condition ceiling, with the precise outer
deadline authoritative for raw and Engine calls alike. Conflicting writes still serialize. Native internal turn and
retry counts are not equated to outer invocation counts. The raw Store retains
one unused protected review slot and one protected token; the external call and
observed-token ceilings remain shared. Cooperation's final reviewer is included
in its roster and expenditure. Native usage per invocation is the accounting
source, not the per-invocation reservation.

Calibration proposes aggregate observed thresholds of 480,000 tokens and at most
72 outer invocations; pilot proposes 1,920,000 and 192 respectively. These sums are
not hard billed-token ceilings or predicted spend. Unreported work may overshoot
the threshold before cancellation; the owner must approve this form of limit.
Currency cost remains unknown until supported native/pricing observations exist.

No automatic allowance increase or model-effort change is permitted. Unknown or
partial usage, contradictory identity/permission settings, broken measurement or
unverified process termination stops the phase. Missing reported model/effort
remains null with incomplete metadata identically in every condition. Requested
and sent values must match the frozen configuration; contradicting reported
values invalidate it. Underused rosters remain requested/actual observations,
not a claim that all configured agents worked. Ordinary unsuccessful calibration
artifacts do not prevent the pilot or authorize tuning against measured answers. The parent reviews the actual calibration observations and preserves all
failed/interrupted cells. Any revised settings require a new frozen proposal.


## Other resource units

The 128,000/32,000 context-character bounds apply to assignment prompt/instructions;
the visible task bundles are only 1,752–4,061 characters. The common 64,000-character
per-call limit covers visible output, not total input/output usage or reasoning.
Its aggregate visible-text allowance is not claimed equal between one and several
calls. Per-invocation token reservations are admission allowances, not a cap on a
started native loop: a fully accounted call may exceed its reservation while the
shared observed-token ceiling remains active. The protocol tests exercise that
case. `native_max_turns=16` is retained configuration data, not a proved Codex
internal-request cap. None of these values are inferred from synthetic token costs.

The current correction and preserved review controls are documented in
[rework evidence](../../ymp-docs/evidence/ymp-201/rework-round1/README.md).

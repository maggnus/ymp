# Exact YMP-201 run proposal awaiting acceptance and owner approval

Candidate d12d40a, executable consumer 75f60a6. The owner requested the small
comparison; no concrete experimental allocation has been approved. These are
byte-identical copies of the candidate's proposals, not authorization records.
Independent re-review of C1/C2/C3 and final controls passed; parent acceptance
is recorded in [acceptance.md](../acceptance.md). Exact proposal bytes are
unchanged and are ready for the owner decision.

## Models and conditions

Use discovered gpt-5.6-luna as the proposed weak model and gpt-6-astra as the
proposed strong model, both with explicit low effort. Solo retains a full native
tool loop and the available shared condition deadline. The measured phase uses
strong solo, weak solo, isolated weak attempts of size 2/3 and actual ymp teams of
size 2/3 on each of the two frozen measured tasks. Checking participants are counted
inside the teams; there is no additional strong model judge.

Calibration has six attempts on separate preparation variants, covering both
models and the real cooperation paths. It establishes measurement validity;
wrong answers or fully accounted task failures do not by themselves exclude the
pilot. Unknown spend or broken controls still stop progression. The conditional
measured phase is allowed only by both its own owner approval and a valid native
calibration bound to the exact prerequisite manifest. Scripted success is not a
valid native prerequisite.

## Proposed maximum allowance and stop rules

| Phase | Outcome attempts | Shared deadline per outcome | Observed input+output stop threshold per outcome | Outer calls per outcome |
| --- | --- | --- | --- | --- |
| Preparation calibration | 6 | 480 seconds | 80,000 | 12 |
| Conditional measured pilot | 12 | 900 seconds | 160,000 | 16 |

Summed administrative thresholds are 480,000 and 1,920,000 observed tokens, totaling
2,400,000 across both phases. Outer-call ceilings are 72 and 192, totaling 264; one
outer invocation can contain multiple native requests and tool calls. Summed
group deadlines are 48 and 180 minutes, totaling 228 minutes (3 hours 48 minutes),
excluding preparation and external evaluation overhead. These are limits, not
predictions of consumption or an allocated budget.

The token values are observation/admission stop thresholds, not hard billed-token
ceilings. An unfinished native request may overshoot a threshold before its
usage is known. Actual currency cost is unknown; cross-model token volume is not
an equal-money claim. Complete input/output usage includes cached input and
reasoning as subsets rather than adding them twice. Keep failed work and all
planning, review, communication and selection in the condition accounting.

Use one application attempt, at most two scheduled invocations within a condition,
and no reset of the shared deadline for another participant. A native call can
use the actual remaining whole-condition time. One approved phase can execute
once: its durable private ledger survives removal of output directories. Neither
a failure nor unused quota authorizes another phase or a repeated paid run.

## Exact manifest identities

- Calibration: b21ad8dc537fe9c11244b43293c6162cf5c774d328fc47c0be9762310386b179.
- Conditional pilot: dffb4e86c30afe58da32564e474064793aab0c88208bd757f7cc4c41d295f98b.
- Frozen runner: 4790774e92090df62a62083c471a05e96f855efd25862577ca4266cc918a92bc.

The JSON files contain the exact native binary/wrapper/interpreter/source bindings,
protected paths, input hashes and condition order. Approval is a separate trusted
record naming these hashes and the owner's actual decision; none is supplied here.
Run from the accepted frozen candidate with the pinned executable. A main-branch
integration does not authorize replacing a frozen executable or model silently.

The owner may approve calibration alone or both phases with the stated conditional
transition. The concrete question now asks whether to approve calibration and its conditional
measured pilot with these exact models, thresholds, deadlines and one-use scope. A failed scientific hypothesis is an
allowed result. This small pilot can supply examples and diagnose a protocol,
but cannot establish universal equivalence or impossibility.

## Owner correction and restoration

On 2026-09-14 the owner briefly requested Sonnet 5 instead of Astra, then withdrew
that change and requested the original configuration. These unchanged accepted
Astra/Luna manifests are therefore the active proposal again. The parent verified
both hashes and the frozen runner after cancelling the Sonnet adaptation. The
owner's model correction/restoration did not approve expenditure; the original
concrete spending question remains pending.

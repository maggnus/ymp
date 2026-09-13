# YMP-201 minimal pilot package

Status: executable preparation and deterministic controls; no native experiment,
calibration, model-quality result or approved quota. Base:
`399ecc2c40ba2473f2676c482648e664cba9a52e`. The parent owns the hypothesis,
scientific sources, spending approval, final study freeze and interpretation.
See the [parent study](../../ymp-docs/research/weak-agent-pilot.md).

This package uses Python 3.9+ and its standard library, the existing
[universal validator](../validators/universal.py) for strict parsing, confined
paths and digests, and the existing Rust evaluation crate for protocol checks.
There are no additional dependencies, browser requirements or model calls in
the commands labelled offline below. Nothing reads the user's application home.

## Tasks and visibility

| Task | Preparation variant | Measured variant | External decision |
| --- | --- | --- | --- |
| `reconcile` | 12 ledger rows | 28 different ledger rows, including integers above 2^53 | Independently recompute active groups and exception reasons, cross-check handwritten reference, compare exact CSV bytes |
| `repair` | Repair interval union in `normalize_windows` | Repair free intervals in `available_windows` | 11 preparation / 21 measured behavioral cases, including unchanged inputs and the preserved `total_duration` function |

Each task has `fixtures/{preparation,measured}/{task}/visible/` and `private/`.
Only `visible/` is exported. Requirements specify all scored behavior; there are
no hidden requirements. The private side contains exact answers, behavioral
checks and materialized incorrect controls. It never enters solver prompts,
working directories, native instructions, memory or peer messages. The complete
repository and study manifest are observer-side materials, not solving inputs.
The reference author and preparation agents must not solve, select, advise or
repair measured outcomes. Fresh measured participants do not inherit this fork,
preparation examples, test logs or reference-author context.

Stage only one variant into a fresh absolute directory outside Git:

```sh
python3 ymp-evals/weak-pilot/observer.py stage --task reconcile --variant preparation --destination /tmp/ymp201-preparation-csv
python3 ymp-evals/weak-pilot/observer.py stage --task repair --variant measured --destination /tmp/ymp201-measured-python
```

The fixed task prompt for every condition is:

> Read REQUIREMENTS.md in the selected directory and deliver the requested files.
> Use only the permitted tools and the visible files in this directory.

All six conditions receive the same requirements, source bytes, public checks
and permitted tools for a given task/variant. Solo retains its full native tool
loop and may self-correct within the allowance. Runtime orchestration differs
between conditions; that is part of the treatment and must remain in the journals.

Staging outside Git and keeping the evaluator in another directory are **data
separation**, not an OS security boundary. Neither Python subprocesses nor a
provider `cwd` prevent arbitrary reads of other directories. Future native
execution requires verified tool/access confinement that excludes private
fixtures, other attempts and user state; until then native execution is refused.
The Python checker runs submitted code in a separate, timed process; use a
disposable, credential-free evaluation environment for untrusted submissions.
Current offline tests execute only reviewed, checked-in synthetic controls.

## Proposed conditions and output selection

| Condition | Participants | Producers | Final review / selection |
| --- | --- | --- | --- |
| strong solo | 1 proposed strong | 1 | Freeze the only artifact; external scoring is not runtime acceptance |
| weak solo | 1 proposed weak | 1 | Same as strong solo |
| independent-2 | 2 weak | 2 | Isolated initial attempts; deterministic public-only selection |
| independent-3 | 3 weak | 3 | Isolated initial attempts; deterministic public-only selection |
| cooperation-2 | 2 weak | At most 1 | The other real participant is reserved for independent Engine review |
| cooperation-3 | 3 weak | At most 2 | One real nonproducing participant is reserved for independent Engine review |

Proposed native IDs `gpt-5.6-luna` / `gpt-6-astra`, both `low`, come from the
discovery report supplied by the parent. They are proposals, not an approved
capability ordering, quota or a substitute for a captured native catalog.
All cooperating participants, including checking and integration, use the weak
model. There is no hidden strong coordinator or extra model judge. The proposed
independent selector uses **no model at all**; if the parent instead chooses a
model-based selector, it must be an already-counted weak participant, and its
invocations must share the allowance. That would be a new protocol freeze.

Independent candidates start in fresh native contexts and separate directories;
they see neither peer answers nor peer tool results. Stop and freeze every
candidate before selecting. `public_select.py` receives candidate directories in
preassigned participant ordinal order and the frozen visible check. It chooses
the lowest ordinal passing that public check; if none pass, it chooses ordinal 1
and preserves the failure. Missing candidates remain in the record. This rule
does not promise selection of the correct answer.

```sh
python3 ymp-evals/weak-pilot/public_select.py --task repair --public-check ymp-evals/weak-pilot/fixtures/measured/repair/visible/public_test.py /tmp/candidate-1 /tmp/candidate-2
```

The selector imports no private checker. Its subprocess time and failures are
part of condition overhead. In particular, the checked-in `strict-minimum.py`
control passes public tests, fails hidden tests, and is still selected over a
correct second candidate. The test suite verifies that no hidden oracle changes
this outcome. Selection is external and deterministic, not a native participant.

Cooperation must use actual `Engine` records and its ordinary independent
acceptance. Fixed profiles must correspond to real distinct contexts and
assignments, not renamed authors. `adaptive = false` removes reputation influence;
it does not disable the existing allocation algorithm. Memory is disabled.
A captured three-profile roster is not proof that three agents performed work.
Record actual participating agent IDs; underuse is a retained protocol deviation,
not evidence from three working agents. No private scorer is installed as an in-run acceptance check: a hidden failure
must not cause another attempt, output choice or repair. Runtime acceptance and
external objective correctness are recorded separately.

## Blind freezing and scoring

An operator records the selected candidate/hash and runtime outcome before
external scoring. Assign an opaque random 32-hex blind ID independently of task
order; retain its condition mapping outside the scorer's input. Directory names
and evaluator labels must not encode models or conditions. The scorer receives
only task, variant, opaque ID and the frozen submission. Deterministic scoring
does not read the treatment manifest or select among multiple candidates.

```sh
python3 ymp-evals/weak-pilot/observer.py seal --task repair --variant measured --workdir /tmp/selected-candidate --destination /tmp/opaque-submission --blind-id 84a91f0d56bf4d16a3e9cdbd7185c542
python3 ymp-evals/weak-pilot/observer.py score --submission /tmp/opaque-submission
```

The example ID is illustrative, not an assigned experiment ID. `seal` checks
immutable visible inputs and hashes the deliverable without private evaluation.
`score` rejects changed source files, stale artifacts, extra receipt fields and
escaping symlinks; it exits 1 for incorrect artifacts. Do not send its output back
to participants or alter accepted artifacts. A missing deliverable is a failed
outcome in the result record, not a skipped condition. Invalid snapshots and
timeouts are retained as operational failures, not quietly replaced with a run.

## Manifest and result contracts

`manifest.template.json` proposes six conditions for each of two measured tasks:
**12 outcome attempts**, not 12 model requests. It contains no measured data.
Generate a reproducible proposed order and source/checker hashes offline:

```sh
python3 ymp-evals/weak-pilot/observer.py matrix --seed 2010914 > /tmp/ymp201-proposal.json
```

The seed is an explicit illustrative choice, not a completed random allocation.
Before execution, the parent records approval, freezes this generated JSON and
its SHA-256, captures exact source/runner/native-catalog revisions and assigns
opaque submission IDs. Conditions are shuffled separately within each task;
retain the seed and resulting order. Never reorder based on observed outcomes.
Measured artifacts must bind the exact frozen manifest; the current command does
not authorize or start execution.

`result.template.json` defines one outcome record. `invocation.template.json`
defines each row in `invocations`; populate them from the trusted native/runtime
journals, by originating `agent_id`, `assignment_id` and `invocation_id`. Keep
requested, sent and reported model/effort separate. Unknown values remain `null`.
Native context IDs and originating invocation records carry historical identity.
Use `coverage: complete/partial/unknown` once observed. Usage events are cumulative
snapshots for an invocation, not additive charges. Cache tokens are already in
input and reasoning tokens in output; do not sum them twice. An error or cancelled
turn retains its last observed usage, without asserting complete coverage.

Outcome `status` becomes `completed`, `failed`, `interrupted`, or
`blocked_before_native` only from real observations. `execution_kind` distinguishes
`offline-protocol-proof` from any later `native-measurement`. Preserve every failed
invocation, selection/review/coordination cost, interruption and human intervention.
Do not aggregate by provider. `runtime_acceptance`, `runtime_confirmation` and
`external_score` remain independent. A successful artifact score supplies neither
a runtime acceptance receipt nor model-quality conclusions.

Preparation, including development agents, reference construction, protocol
inspection, deterministic tests and selection/scoring infrastructure, has a
separate ledger. Unknown preparation effort/time/cost must not become zero. Only
the number of experimental native invocations is known to be zero in this package.

## Offline verification and remaining execution boundary

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s ymp-evals/tests -v
cargo build -p ymp-eval-driver --bin ymp-weak-pilot
target/debug/ymp-weak-pilot offline-proof --output /tmp/ymp201-offline-proof
```

The narrow evaluation binary is a research tool inside the existing evaluation
crate, not a second distributed application. It tests production provider,
storage and Engine boundaries using controlled backends. It does not write task
answers or convert scripted success into native evidence. Its `native` command
refuses before inference while required controls remain unverified. See
[quota and boundary proposal](quota-and-boundary.md) and
[preparation evidence](../../ymp-docs/evidence/ymp-201/preparation.md).

No calibration, provider probe or measured attempt is authorized here. The
parent's next step is to review this package and resolve the stated native
control/access boundaries before approving concrete commands and expenditure.

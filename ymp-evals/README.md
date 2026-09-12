# Acceptance fixtures

YMP-119 supplies deterministic inputs, independent validators and scripted expectations for the [runtime contract](../ymp-docs/architecture/runtime-contract.md). It does **not** run the integrated application or establish native model quality. YMP-126 now provides the production-boundary driver below; YMP-121 independently validates the accepted composition and release package. The existing [greeting scenario](scenarios/greeting.json) remains a separate software smoke check.

The standalone validators use Python 3.9+ and its standard library. The trusted driver is a Rust workspace package. No provider, network, credentials or paid inference is used by the validators or tests.

## Model use during verification

Unattended Rust, bridge and evaluation tests use deterministic backends or local protocol fixtures. Effort values such as `max` and `xhigh` are test data for transmission and validation; they must not launch real model inference. The Claude bridge tests use the real SDK with its executable explicitly replaced by `tests/fixtures/claude.py`.

Real-provider compatibility checks are separate, explicitly authorized work. Use an explicit minimal supported effort in the temporary probe configuration, record requested/sent/reported settings, and do not inherit native or agent defaults that may select `max` or `xhigh`. If the provider cannot offer a suitable low-cost setting, record the limitation rather than silently using an expensive default. The production `doctor --probe` command currently follows configured/native settings and is not an unattended test command.

Development and independent code-review agents have their own reasoning settings and consume resources separately from the test workload. The final YMP-121 reviewer remains at the owner's requested `max`; that setting does not authorize running tested providers at `max`.

## Run the fixture checks

From the repository root:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s ymp-evals/tests -v
```

Tests create temporary non-Git task directories with explicit inputs and synthetic reference artifacts. They accept the reference artifacts, then reject missing files, altered facts, incorrect totals, invented citations, changed input bytes, unrelated passing checks, stale evidence, self-review, unjustified reputation and protocol regressions. Passing these tests proves validator discrimination only. Reference outputs are never runtime completion evidence.

The CLI emits one JSON object and exits 0 when its specified check succeeds or 1 when validation fails:

```sh
python3 ymp-evals/validators/universal.py artifact --case document --workdir /absolute/task-directory
python3 ymp-evals/validators/universal.py workflow --case document --workdir /absolute/task-directory --observed /absolute/workflow.json
python3 ymp-evals/validators/universal.py protocol --case restart-inspection --workdir /absolute/task-directory --observed /absolute/observed.json
```

`artifact` checks actual files and source identity independently of review claims. `workflow` additionally checks the runtime's normalized acceptance receipt. `protocol` compares a normalized observed trace with the scripted contract, including event order and unexpected/missing entries. An exit status alone does not confirm any broader result.

## Universal workflows

[universal-workflows.json](scenarios/universal-workflows.json) contains exact prompts, input and output paths, criterion/check identifiers, and comparison conditions. Copy only its listed `inputs/` files from [fixtures/universal](fixtures/universal) into a fresh task directory before running a workflow. Keep `reference/`, validators, expected traces and test code outside agent context and accumulated knowledge. Runtime metadata belongs in its configured application home; task outputs belong in the selected directory.

| Case | Required output | Independent check | Acceptance grade |
| --- | --- | --- | --- |
| `document` | `outputs/workshop.md` | Agenda title, date, place and ordered table match the supplied JSON; only newline/trailing-space differences are ignored | Confirmed after independent review and relevant runtime-observed check |
| `transform` | `outputs/totals.csv` | Recompute posted-row integer totals and counts, including refund, zero quantity, quoted account, ordering and excluded void rows | Confirmed after independent review and relevant runtime-observed check |
| `grounded` | `outputs/claims.json` | Recompute three rates/differences from the supplied fictional external CSV and check each source/row reference | Confirmed only for those three claims |
| `qualitative` | `outputs/ideas.md` | Required file structure only; independent reviewer judges usability | Accepted/unconfirmed, no reputation increase |

The document workflow must run outside Git, then answer “Where did you save it?” from its saved outcome with zero new invocations. The validator checks actual directory ancestry, delivered bytes and the follow-up path. Qualitative validation returns `objective_success: null` and `check_scope: structure_only`; valid formatting is not confirmation of quality. The example qualitative reference is one possible candidate, not an oracle. An unrelated but well-formatted qualitative response still needs an independent relevance/usability review and may be rejected there.

All objective input files must retain the checked-in bytes. JSON duplicate keys, non-finite metrics, wrong types and extra unsupported claims fail. Paths must resolve inside the selected directory; a symlink to an external reference artifact fails.

## Scripted protocol cases

[universal-protocol.json](scenarios/universal-protocol.json) describes each setup, sequence of harness actions, expected event projection and final state. The scripts are consumed by the trusted runtime adapters below; expected traces remain exclusively validator-side comparison data.

| Cases | Required behavior |
| --- | --- |
| `fixed-size`, `fixed-roster`, `adaptive-team` | Distinguish pinned size from pinned identities; bounded membership changes preserve history; impossible independent review stays pending |
| `effort-support` | Reject unsupported or pinned-out settings before invocation; preserve requested/sent/reported effort, including absent native acknowledgment |
| `budget-reservations`, `partial-usage` | Charge execution, planning, communication and review to the same allowance; deny reservation races and unaffordable retry/consultation; preserve unknown cost and partial usage |
| `concurrency-conflicts` | Hold barriers to prove independent work overlaps; a conflicting writer or reader waits until the writer releases its resource |
| `assignment-authority` | Bind caller identity, reject cross-session and runtime-only operations, expire grants, and keep old continuation context from restoring authority |
| `restart-inspection` | Preserve a confirmed sibling, record partial usage, revoke old permission and inspect actual files before any production replay |
| `location-retrieval` | Reuse the exact non-Git artifact in both a same-session follow-up and a later session without new inference |
| `knowledge-correction` | Preserve applicability/provenance, filter changed or unrelated sources, reject agreement-only promotion, supersede with confirmed corrected data and ignore duplicate observations |
| `evidence-boundaries`, `artifact-version` | Reject self-review, unrelated checks, failing applicable evidence, aggregate over-confirmation and stale version checks |

Budget units are controlled synthetic token counts, not currency or claims about native bounds. The fictional corrected source changes Hill's W36 completion rate from 95% to 60%; the old entry remains historical and cannot answer a request against the corrected source. Symbolic names such as `a1` and `grant-a1` are test aliases, never secrets.

## Trusted runtime driver (YMP-126)

Build both the driver and the real public MCP stdio bridge, then select a fresh absolute evidence directory outside any Git checkout:

```sh
cargo build -p ymp-eval-driver -p ymp-cli
target/debug/ymp-eval-driver --output /tmp/ymp-universal-unique-run
```

Use `--case document` (or any named case below) for a bounded diagnostic run. A selected-case pass exits zero, while `run.json.complete` remains false until all 17 named cases have run and passed. Any failed or missing adapter exits nonzero and leaves a concrete case error. Existing evidence directories are rejected. Rebuild after source or fixture changes: the executable compares live source/checker hashes with its embedded build snapshot before running. No live provider discovery, native inference, network credentials or account data are needed.

Each case creates its selected `work/` directory and separate `metadata/` application home. The providers are explicit scripted identities. Workflow artifacts are independently computed by `driver/artifact_writer.py` from listed input files; trusted runtime checks run the external validators. Expected traces, reference outputs and validator implementation bytes never enter provider prompts or retained knowledge. Scripted qualitative review remains unconfirmed and makes no claim about real-model quality.

The driver uses real `Engine` execution for workflows, location, knowledge and crash recovery. Protocol setup helpers use actual Store version/contract validation, live TeamServer admission and the same Engine workspace coordinator; they do not synthesize accepted records. The fixed-roster adverse setup starts without a standing team reservation, then asks actual Engine review admission to resolve the impossible independent-review roster. The authority adapter runs the actual `ymp mcp` stdio executable; its fixture ends with a live second grant and exports subsequent cleanup separately. Restart uses separate OS processes, actual SIGKILL after durable partial usage, and private inherited pipes for the expiring capability. Native tokens never enter evidence files.

The restart session explicitly captures `unknown_usage: bounded_native` before starting. It preserves the 100 reported-token ceiling, all native/admission caps and partial coverage. The default `stop` policy still denies continuation after incomplete accounting. See the [policy correction evidence](reports/ymp-126-unknown-usage-policy.md). The [reservation correction evidence](reports/ymp-126-runtime-corrections.md) distinguishes new runtime corrections from earlier acceptance.

Every case retains raw runtime/native journals, actual artifact bytes, aliases with projection sources, nullable resource/cost metrics and independent validator results. `run.json` embeds source and fixture hashes plus the actual executable digest; each case’s metrics bind the actual backend IDs/versions and invocation IDs. Resources remain attributed to agents. Evidence outside a projected scenario endpoint is retained as raw history or an explicitly named cleanup journal. Scripted outcomes establish boundary behavior, not cooperation or model-efficiency improvements.

## Release integration contract

Implement a trusted harness against actual runtime admission, provider, storage and tool boundaries. Its provider should consume each script, return controlled native responses/usage and hold the stated barriers. Setup and `seed_*` actions must establish valid records through production transitions or reviewed test setup; they must not create fake completion events. Restore persisted state in a fresh runtime process for restart cases. Work directly in each explicitly selected task directory.

The expected traces define a projection of the named boundary behaviors, not Rust enum names or a provider wire format. The exporter must derive that projection from actual runtime/native records and measured counters, map real IDs consistently to the declared aliases, preserve event order, and retain raw record IDs in a sidecar map. Extra occurrences of the projected behaviors must be exported, including denied-request invocations, duplicate credits, extra members or stale grants; do not filter by expected values. The complete raw journal and native observations remain required review evidence for transitions outside the projection. Script barriers make the asserted order deterministic without requiring wall-clock timing thresholds.

Expand fixture-owned `$WORKDIR`, `$PATH:relative/path` and `$SHA256:relative/path` placeholders using the actual directory/artifact. Observed exports contain resolved values, never placeholders. Cases with `artifact_cases` also run independent artifact validation; listed protocol `inputs` are checked against immutable fixture bytes. `file_assertions` compare exact final bytes: interrupted totals must remain the specified partial output until inspected. Recovery inspection usage is charged as well as the interrupted attempt. `request_admission` actions are one-shot boundary probes; a deferred probe creates no new assignment or implicit queued work.

For every workflow, export these required fields in `workflow.json` (the test helper `UniversalTests.receipt` shows a fully populated **synthetic** example):

| Record | Fields |
| --- | --- |
| Envelope | `schema_version: 1`, `case_id`, `session_id`, absolute `working_directory` |
| `artifact` | Relative `path`, `sha256`, stable `result_id`, positive integer `result_version`, single-element `producer_agent_ids` |
| `review` | `review_id`, independent `reviewer_agent_id`, matching `result_id`/`result_version`/`artifact_sha256`, `decision: accepted`, nonempty `rationale` |
| `acceptance` | Matching `result_id`/`result_version`/`review_id`, `state: accepted`, scenario `confirmation` grade and matching `confirmation_ids` |
| `confirmations` | One objective record with `confirmation_id`, `observer: runtime`, `outcome: passed`, scenario `check_id`, matching result/version/artifact hash, exact `criterion_ids`, and `inputs: [{path, sha256}]`; empty for qualitative quality |
| `reputation_observations` | Zero or one supported producing-agent observation: unique `observation_id`, `agent_id`, matching `result_id`/`result_version`/`confirmation_id`; empty for qualitative acceptance. Credit magnitude is outside this fixture |
| `usage` | All invocation rows with `agent_id`, `assignment_id`, unique `invocation_id`, `phase`, nonnegative integer/null `tokens`, and `coverage: complete/partial/unknown`; execution and independent review must both appear |
| `follow_up` (document) | Matching `session_id`/`result_id`, absolute `answer_path`, matching `artifact_sha256`, and `invocations_started: 0` |

Run the declared artifact check through the trusted runtime and capture its input/output bytes, criterion and result identity before acceptance. A model-authored receipt claiming `observer: runtime` is not trusted evidence. These standalone validators cannot establish export authenticity; the independent release reviewer must verify the exporter and retained raw records. Applicable failing checks, unreviewed work and rejection are covered by the negative protocol cases, not fabricated positive workflow receipts.

Persist an integration evidence bundle containing:

- `run.json`: application/exporter/scripted-provider revisions, fixture digests, commands, all case IDs and explicit `execution_kind: scripted-runtime` (or `synthetic-validator-test`).
- Per workflow: selected task directory with inputs and outputs, `workflow.json`, raw runtime/native events, alias map and validator JSON result.
- Per protocol case: selected directory, `observed.json`, raw runtime/native events, alias map and validator JSON result.
- Per case resource metrics: measured elapsed time, attempts, peak active invocations, all phase usage by agent/assignment/invocation, coverage and nullable reported cost. Keep quality/confirmation outcomes in separate fields. `reported_tokens` from the validator is the sum of available measurements, or null if all are unknown; partial coverage is not a complete total.

Run all four workflows and all thirteen protocol cases against the integrated application; missing adapters, missing evidence or skipped cases remain unverified release conditions. Also rerun the validator negative controls, required Cargo checks and release packaging/native checks specified by YMP-121. Do not mark runtime acceptance complete based on this fixture suite's unit tests.

The comparison specification gives solo, isolated independent-attempt and cooperating-team treatments identical prompts, data, total budget and acceptance checks. It separates quality from resources, charges selection and review, and requires unknown cost to remain null. Synthetic scripts cannot demonstrate better models, cooperation benefits or memory improvements. Such comparisons require separately authorized real inference and repeated measurements.

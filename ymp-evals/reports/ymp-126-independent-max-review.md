# YMP-126 independent max-reasoning review

**ACCEPT, 9/10, for YMP-126 scripted runtime boundary evidence.** No blocking finding remains in the reviewed driver candidate. This verdict does not accept the final YMP-121 release, UI composition, native model behavior, installation or packaging.

Reviewed candidate: `a04aef41e003609c42c81ac5b698c0ca2a208274`, in the independent detached worktree `/tmp/ymp126-max-review`. Its compiled source is byte-identical to author checkpoint `a8493ac2a912b8f44fe5854a666607e4b7f6739b`; the later candidate commit adds evidence and documentation. The compiled source-tree SHA-256 is `535a26d0c3e1d885072a134f93677056ec3548a6115df907970ba9f151f759db`.

Review read the full driver adapters and exporters, source-binding implementation, workload scripts, validator boundaries, task acceptance criteria and approved intent. The independent review used max reasoning as requested. Workloads used mock/scripted implementations only; no installed-provider inference, account discovery or credential access occurred. No main, sibling checkout, tracker, approved intent or owner request file was changed. Mutations were confined to the independent worktree and restored before handoff.

## Authenticity and unchanged acceptance criteria

The author archive SHA-256 `ea6d989bb55b3d2e483bcc69d65a14ac80c9d1c33bcfbc554fb861dec5d8d2d2` matches its manifest. All **232 archived files** match their individual hashes. Every one of the **142 compiled-source files and 12 fixture/checker files** matches both the candidate and Git `a8493ac`. Recomputing the sorted source inventory produces the declared source-tree hash. The original retained driver executable exists and independently matches `4c2e619d441b13954aff4adbe8af58d83b745d658d6c177b186771ca64b94601`.

The authoritative workflow/protocol scenarios and validator have no modifications after their original `456311b` fixture commit. They also have no diff from accepted main `bfba34b890672bdd03772030f7968f0a1457a6c8`. The approved intent still hashes to `4479e919c5b47934e69e81ac493d16e90a0dc420a9a370382e79bca3f3c754d9`.

Independent SQLite inspection passes integrity checking for all **18 databases**. The exported task, assignment, invocation and decision collections match the durable tables: **682 records** checked. Full histories match durable event rows, including the explicit authority cleanup and later location session. All **79 protocol observations** have resolvable source mappings in the retained journals, with their per-source sequence preserved. Objective artifact/input digests match their captured receipts. Details are in [independent-record-audit.json](ymp-126-review-evidence/independent-record-audit.json).

The reviewer also audited the separate fresh run against its SQLite state and inventoried all 232 files; see [fresh-run-audit.json](ymp-126-review-evidence/fresh-run-audit.json) and [fresh-run-file-hashes.json](ymp-126-review-evidence/fresh-run-file-hashes.json). The second authority grant is intentionally active at the named endpoint, and its subsequent closure is retained separately rather than omitted.

## Actual runtime behavior

| Required cases | Independently established behavior |
| --- | --- |
| `document`, `transform`, `grounded`, `qualitative` | Public Engine execution produces actual files through the scripted subprocess, observes usage, captures contracts/checks, obtains a separate agent review and records runtime acceptance. Objective validators recompute from immutable inputs. Each workflow records six invocations and 42 synthetic units. Qualitative acceptance remains unconfirmed, with no objective confirmation or reputation observation. |
| `fixed-size`, `fixed-roster`, `adaptive-team` | Actual allocation decisions enforce size and identity constraints. The adverse fixed roster has admitted native contributions and validated storage transitions; impossible independent final review remains pending. Adaptive membership exercises actual `board_read`, `task_propose` and Engine proposal commit boundaries, including an active-assignment barrier. |
| `effort-support`, `budget-reservations`, `partial-usage` | Real admission rejects unsupported/pinned settings and unaffordable requests before invocation. Requested/sent/reported effort stay distinct, including absent acknowledgment. Shared spend is 80 units in the budget case, with zero remaining live reservations and zero denied invocations started. Partial usage remains partial and cannot establish a strict token bound. |
| `concurrency-conflicts`, `assignment-authority` | Real workspace owner/reservation objects and held native futures establish overlap of independent work and deferral of conflicts. The authority adapter runs the real `ymp mcp` stdio executable; its path and hash are retained. Actual TeamServer responses bind the actor, deny foreign-session/runtime-only operations and reject the expired first grant during a fresh native continuation. |
| `artifact-version`, `evidence-boundaries` | Admitted native production/review, actual artifact snapshots, trusted checker execution and public storage invariants reject self-review, stale review/check bindings, failing applicable evidence and aggregate over-confirmation. The unrelated command is actually executed and supplies no confirmation. |
| `location-retrieval`, `knowledge-correction` | Actual outcomes support zero-inference same-session and later-session path retrieval. Knowledge uses real Engine-produced and independently checked 95/60 source computations, applicability filtering, rejected agreement-only promotion, atomic supersession and duplicate-observation handling. The preallocated query context is explicitly distinguished from later persisted session capture. |
| `restart-inspection` | Two actual OS processes, SIGKILL after persisted partial usage and file output, expired-capability denial, one new scoped read-only inspection, preserved confirmed document and unchanged incomplete totals. Reported spend is 47 then 50 of the captured 100; partial coverage remains and no production replay is accepted. |

The scripted implementations control workload, native responses and barriers; they do not supply trusted completion receipts. Setup adapters use the real public Engine reservation/execution objects, TeamServer admission and validated Store transitions. Checks execute through the owning confirmation subsystem. The source contains no alternate accepted-event generator that bypasses those boundaries.

The writer reads only listed input files, and validators/reference artifacts are outside selected task directories. All 50 retained native prompt records were inspected for reference/expected-trace leakage, covering 183,984 characters; prompt digests also match where recorded. Provider prompts carry requirements and runtime check outcomes, not validator implementation or reference outputs. Native backend IDs and versions derive from invocation records; absent metadata stays explicit. Usage is attributed to session agent IDs. Selected working directories have no Git ancestry, and separate application homes contain metadata rather than hidden working copies. See [prompt-audit.json](ymp-126-review-evidence/prompt-audit.json).

## Independent execution and negative controls

The reviewer built driver and CLI with a separate target, disabled debug information/incremental compilation and two build jobs. The fresh full command was:

```sh
CARGO_TARGET_DIR=/tmp/ymp126-max-review-target \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 \
cargo build --locked -p ymp-eval-driver -p ymp-cli
/tmp/ymp126-max-review-target/debug/ymp-eval-driver \
  --output /tmp/ymp126-max-review-run
```

It exited **0**, with **17/17 cases** and `run.json.complete: true`, from **2026-09-12 22:34:44 to 22:35:15 UTC**. Its recorded driver digest is `86c1b61159f56c2c78ccf9210a2578994daa9065a4ea30c9baf7385c55094307`; its real stdio CLI digest is `a0d98cc6e0e829a348a0c279914cb56e3d2c67500bf1fd79292d09264e110b3f`. The driver executable was subsequently rebuilt for mutation controls; the full run retains its original observed hash and complete embedded source inventory. [Review provenance](ymp-126-review-evidence/review-provenance.json) binds the commands' logs and source identity; [fresh runtime archive](ymp-126-review-evidence/fresh-runtime-evidence.tar.gz) retains the full returned evidence.

Independent checks also passed:

- `cargo fmt --all --check`.
- `cargo test --locked -p ymp-eval-driver --bin ymp-eval-driver`: **7 passed**, including actual-adapter negative controls for missing acceptance, extra usage/credit, duplicate/reordered knowledge transitions, missing native/durable sources, additional/early/unattributed effort events, location answers and partial-usage revocations.
- `cargo test --locked -p ymp-eval-driver --test restart`: **1 passed**, with actual new-process recovery and mutated duplicate closures/revocations and premature recovery closure.
- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s ymp-evals/tests -v`: **26 passed**.

Additional reviewer-owned controls went beyond the provided tests:

1. Changing the workload writer without rebuilding makes the actual compiled CLI reject source drift before any case starts. Changing the validator independently makes it reject checker drift at the same boundary.
2. Rebuilding a writer that deliberately produces an incorrect document title creates actual artifacts and actual positive scripted reviews. The trusted checks fail twice, the runtime rejects both attempts, and **zero task acceptances** are recorded. The driver exits nonzero and stays incomplete. Thus even a cooperating scripted reviewer cannot fabricate confirmed success. [Control results](ymp-126-review-evidence/additional-negative-controls.json), [source mutation](ymp-126-review-evidence/incorrect-writer.patch) and the [complete retained control directory](ymp-126-review-evidence/incorrect-artifact-control.tar.gz) show the result.

The author's retained workspace log independently sums to **402 passed, zero failed, one ignored**, and clippy completed with warnings denied. The ignored native catalog SDK fixture is outside the 17 required cases. These complete prior suite logs were inspected rather than rerun blindly; independent driver checks and all actual cases were rerun. Final integrated-source Cargo checks remain YMP-121 work.

## Policy delta and limits of acceptance

The separate reviewer accepted `d31d49f1e28e498fa85bb3d0e6a4287cbad8c0d2` and the restart adapter at `32e7c5fe0fb8304d17e981132ce89c9e8ec74752`; report: `/tmp/ymp126-unknown-review-mehhjlfz/unknown-usage-policy-independent-review.md`. This review independently compared those files with the candidate: the production policy and actual process/admission code are unchanged. The later change extracts the restart projection and adds terminal anomaly controls; the reviewer ran those controls and the full current restart case.

Default/missing-field `Stop` still blocks incomplete accounting. Explicit captured `BoundedNative` permits admission against reported spend and reservations while preserving the other ceilings. It does not make unknown spend known or claim a strict true-spend bound. The preserved before case remains denied at reported 47/100; the successful after case captures bounded mode before the interruption.

One nonblocking diagnostic limitation was observed in the intentionally failed document control: `workflows::run` reports that location follow-up has no stored artifact, and its exported `runtime.json` precedes the two final follow-up message events. The complete SQLite database retains those events and all six invocations; no positive receipt or lost accounting results. Successful required bundles have exact full-history parity. Capturing the final snapshot on this error path would make future failure investigation easier.

All seven YMP-126 acceptance criteria are satisfied for this candidate as scripted boundary evidence. There is no native model-quality, efficiency, cooperation, paid-cost or production-isolation claim. Final YMP-121 still requires the integrated versioned release source, required checks on that composition, packaging/bridge build, temporary install verification, and the independent requirement-by-requirement release verdict. Paused studies and post-MVP work remain outside this acceptance.

Evidence files and logs are inventoried in [review-evidence/manifest.json](ymp-126-review-evidence/manifest.json). No source edit or commit remains in the reviewed candidate; only this independent report and its evidence are untracked in the isolated worktree.

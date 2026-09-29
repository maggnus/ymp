# First homogeneous Claude pilot

This is experimental evidence, not a development-task status register. It was
prepared from the retained files of fourteen runs. The Dispatcher supplies the
fixed workflow; agent initiative and dynamic cooperation are not claimed by
this pilot. No run reached independent final acceptance: every delivered record
has `acceptance: null`. Runs 07, 09, 11, 12 and 13 each recorded an accepted
result and the expected artifact. Runs 11, 12 and 13 also reached the
final-review stage and made a final-review call; none of them records a review
or an acceptance of the aggregate. Every session ended blocked or, in run 12,
cancelled, so there is no completed native team result. The
[summary](#summary-of-the-pilot) at the end counts the runs by how they ended.

## Procedure

The explicit example is `crates/ymp-storage/examples/homogeneous_claude.rs`
(`discover CLAUDE | run CLAUDE NEW_DIRECTORY MODEL [EFFORT]`). Each run retains a
SQLite journal, exact inputs and settings, policy parameters, a discovery record,
a final projection and a summary.

Purpose recorded in every `experiment.json`: "Homogeneous light Anthropic team
elementary fixed-workflow pilot".

Two agent identities, `claude-a` and `claude-b`, are pinned to the same discovered
model `claude-haiku-4-5-20251001`. Discovery recorded provider kind `Claude`,
native version `2.1.284`, capabilities `ReadFiles` and `WriteFiles`, eleven
offerings and default model `claude-opus-5-5`. The pinned model lists no effort
levels, so no effort was requested. Every recorded invocation has requested, sent
and reported model equal to the pinned model, and requested, sent and reported
effort `null`. The single call of run 08 has no invocation record; its dispatch
record carries the pinned model and effort `null`. Comparative token efficiency
is recorded as unknown.

The task transforms `[3,2,1,2]` into exactly `[1,2,2,3]` followed by a newline.
Two owner checks cover exact output bytes and unchanged input. Attempt capacity is
two, one concurrent invocation and two members are allowed, and the method is
`SoloWithVerifier` with a `StopPreserving`-only ladder. The overall deadline is
480 seconds with 30 seconds of bounded cleanup. Base invocation timeout is
60 seconds in runs 01 to 09 and 240 seconds in runs 10 to 14. The output bound
is 8,000 characters; each invocation permits one native turn. The backend
parameters record a 30-second connect timeout, 1,048,576-byte frames and 8 round
trips. The report is deterministic, with no paid narration call.

Accounting uses relative token weights, **not currency or provider billing**:
input 1, cache read 0.1, cache write 1.25, output 4. Overall limit is 250,000 units,
with a 60,000 verification reserve and 30,000 reporting reserve. The per-call
ceiling is 30,000 and the report-call cost 15,000. UnknownUsage::Stop preserves
incomplete accounting. Complete receipt coverage means complete provider-reported
usage under the adapter's documented boundary, not independent billing
verification.

Runs were made one after another, as recorded by the operator: one dimension
was changed per run in runs 02 to 10, 12 and 14, run 11 repeated run 10 with
nothing changed, and run 13 carried two changes. The run files confirm the
changes of runs 03, 04, 06, 10 and 12 and the first change of run 13. The
changes of runs 02, 05, 07, 08, 09 and 14 and the second change of run 13 are
source or build changes that the run files do not record (see the table below).

Source checkpoint for runs 01 to 13, as recorded by the operator: commit
`197aaa7b` plus the uncommitted W1-0020 working tree. The description of run 14
names one change against run 13 and no other checkpoint. The working-tree state
and build profile of each run are not recorded in run files.

The compact run files were produced by [`derive.py`](derive.py) in this
directory from `view.json`, `summary.json`, `experiment.json`,
`discovery.json`, the workspace files, the log and a copy of the journal. It
uses the Python standard library only and is run from the repository root:

```sh
python3 ymp-docs/experiments/homogeneous-claude-elementary/derive.py \
  [--acceptance-path] RUN_DIR JOURNAL_COPY LOG_PATH NOTE_FILE OUTPUT
```

`RUN_DIR` is a retained run directory, `JOURNAL_COPY` a copy of its
`journal.sqlite` (the original is never opened), `LOG_PATH` the shell log of the
run or `-` when none exists, and `NOTE_FILE` a text file holding the text that
becomes the `note` value. Runs 01 to 05 were derived without the flag and runs
06 to 14 with it. The run directories, journals and logs are the operator's
local inputs, for this pilot `/tmp/ymp-claude-pilot/runNN` and
`/tmp/ymp-claude-pilot/runNN.log`; they are not part of the repository. A run
file is reproduced byte for byte only from those inputs under the same absolute
paths, because `journal.path` and `log.path` record them, and with the `note`
text of that file. [`NOTES.md`](NOTES.md) lists the commands that were used.

Files for runs 06 to 14 carry one additional section, `acceptance_path`,
holding the review, acceptance, ledger, commitment, session-change and
file-access records. Every other section has the same structure in all
fourteen files. In `run-08.json` the `invocation`,
`receipt`, `cost` and `backend_terminal` values of the single call are `null`,
as in the source. In the files for runs 11 to 13 the `acceptance_path` section
has one more key, `final_review`. It holds the recorded purpose of the
final-review call, the finalization values of the projection, the `failure`
text of `summary.json`, every commitment end record with its stored basis, and
the journal records from the `Finalizing` phase change to the end. Its `derived`
values were computed by `derive.py` with the Python JSON parser; they are not
the kernel's decoding result.

## Observations

| Run | Native calls | Spent | Held | Elapsed ms | Session phase | Observed outcome |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| [01](run-01.json) | 0 | 0.0 | 0.0 | 10,052 | `Blocked(no_offers)` | The intake solicitation received no offer inside its 1,000 ms window. |
| [02](run-02.json) | 2 | 25,663.25 | 0.0 | 53,429 | `Blocked(work_failed)` | Intake completed. Planning exceeded its admitted allowance; diagnostic `cost_limit`, terminal `Failed(Content)`. |
| [03](run-03.json) | 3 | 32,717.0 | 0.0 | 78,792 | `Blocked(no_offers)` | Intake, planning and production completed; `sorted.json` has the expected bytes. The next solicitation received no offer inside 1,000 ms. |
| [04](run-04.json) | 1 | 8,700.0 | 0.0 | 24,446 | `Blocked(decoding)` | Intake answered with the expected JSON inside a Markdown code fence. |
| [05](run-05.json) | 5 | 69,253.0 | 0.0 | 188,320 | `Blocked(decoding)` | All five calls completed with Complete receipts; the reviewer answered in key=value text, not JSON. |
| [06](run-06.json) | 5 | 59,361.0 | 0.0 | 186,233 | `Blocked(candidate_rejected)` | All five calls completed with Complete receipts. The producer reported `stale_revision` errors and `sorted.json` kept its initial bytes. Reviewer verdict `Reject`; acceptance decision `Rejected`, grade `Refuted`. |
| [07](run-07.json) | 5 | 59,098.0 | 0.0 | 221,560 | `Blocked(commitment_expired)` | All five calls completed with Complete receipts; `sorted.json` has the expected bytes. Reviewer verdict `Approve`; acceptance decision `Accepted`, grade `Confirmed(TrustedCheck)`. The producer commitment had been recorded `Expired`. |
| [08](run-08.json) | 1 | 0.0 | 28,000.0 | 9,647 | `Blocked(claude_environment)` | The intake call has no start record, no usage observation and no receipt. Terminal `Failed(Protocol)`, diagnostic `claude_environment`. The intake reservation is `Held` with no settlement; usage is unknown. |
| [09](run-09.json) | 5 | 51,212.0 | 0.0 | 173,615 | `Blocked(commitment_expired)` | All five calls completed with Complete receipts; `sorted.json` has the expected bytes. Reviewer verdict `Approve`; acceptance decision `Accepted`, grade `Confirmed(TrustedCheck)`. The producer commitment had been recorded `Expired`. |
| [10](run-10.json) | 1 | 14,028.0 | 0.0 | 35,568 | `Blocked(decoding)` | Intake completed with a Complete receipt and answered with prose followed by the expected JSON. No later call was made. |
| [11](run-11.json) | 6 | 79,245.75 | 0.0 | 266,019 | `Blocked(decoding)` | All six calls completed with Complete receipts; `sorted.json` has the expected bytes. Reviewer verdict `Approve`; acceptance decision `Accepted`, grade `Confirmed(TrustedCheck)`. The producer commitment was recorded `Discharged`. The session reached `Finalizing`; the final reviewer answered with one text line beginning `FinalVerdict:`, not JSON. |
| [12](run-12.json) | 6 | 78,274.0 | 0.0 | 285,149 | `Cancelled` | All six calls completed with Complete receipts; `sorted.json` has the expected bytes. Reviewer verdict `Approve`; acceptance decision `Accepted`, grade `Confirmed(TrustedCheck)`. The producer commitment was recorded `Discharged`. The final reviewer answered with JSON whose `basis` names the two check identifiers. `summary.json` records failure `final_review_basis`; the journal records a session stop. |
| [13](run-13.json) | 6 | 70,628.75 | 0.0 | 253,456 | `Blocked(decoding)` | All six calls completed with Complete receipts; `sorted.json` has the expected bytes. Reviewer verdict `Approve`; acceptance decision `Accepted`, grade `Confirmed(TrustedCheck)`. The producer commitment was recorded `Discharged`. The final reviewer answered with prose followed by JSON whose `basis` names the two supplied evidence identifiers. |
| [14](run-14.json) | 5 | 70,835.5 | 0.0 | 197,683 | `Blocked(decoding)` | All five calls completed with Complete receipts; `sorted.json` has the expected bytes. The reviewer answered with prose followed by JSON inside a Markdown code fence. No review and no acceptance was recorded. The producer commitment has no end record; its state is `Active`. |

Every run except run 12 delivered outcome
`Blocked(final_acceptance_unavailable)`; run 12 delivered outcome `Cancelled`.
Every delivered record has report grade `Unconfirmed`, both criteria listed as
unmet and final acceptance `null`. `input.json` was unchanged in every run.

Recorded parameter changes between runs:

| Run | Change recorded in run files | Described change not recorded in run files |
| --- | --- | --- |
| 02 | None; `experiment.json` equals run 01 apart from timestamps. | Optimized example build. |
| 03 | Forecast `expected_input 10000`, `expected_output 1000`, `p90_factor 2.0` (previously 20000 / 1000 / 1.25); new driver digest. | |
| 04 | `offer_window_ms 5000` (previously 1000); new driver digest. | |
| 05 | None; `experiment.json`, driver digest and backend policy digest equal run 04. | Adapter strips one whole-response Markdown fence; guidance asks for raw text. |
| 06 | The recorded review response contract asks for `CandidateVerdict JSON` and states its shape. `experiment.json`, driver digest and backend policy digest equal run 05 apart from timestamps. | The source state that produced the new contract text. |
| 07 | None; `experiment.json`, both digests and every recorded response contract equal run 06 apart from timestamps. | Adapter waits until the host has recorded every emitted observation before each mediated file operation. |
| 08 | None; `experiment.json`, both digests and the recorded intake response contract equal run 07 apart from timestamps. | Adapter revised after independent review: shared child-process channel module for both process backends; the refusal code that ended observation is journaled as the diagnostic; reported counters are kept as Partial when the response text is undeliverable; the account must be first-party; more environment variables are removed from the child; the isolation check requires the native `plugins` list to be empty. |
| 09 | None; `experiment.json`, both digests and every recorded response contract equal run 08 and run 07 apart from timestamps. | The isolation check accepts plugins whose `path` is `builtin`. |
| 10 | Resource policy `timeout 240000` (previously 60000); new digest of that policy's parameters; new driver digest. Nothing else differs from run 09 apart from timestamps. | |
| 11 | None; `experiment.json`, `discovery.json` and both digests equal run 10 apart from timestamps, and the response contracts of the first five calls equal those of run 09. This matches the description of an unchanged repeat. | |
| 12 | The recorded final-review purpose carries the aggregate reference under `subject`, and its response contract states the JSON shape of `FinalVerdict`. `experiment.json`, both digests and the five other response contracts equal run 11 apart from timestamps. | The source state that produced the new purpose and contract text. |
| 13 | The recorded final-review purpose also carries the two final evidence identifiers under `evidence`, and its response contract reads `basis:[ids copied exactly from evidence]` (previously `basis:[supplied Evidence ids]`). `experiment.json`, both digests and the five other response contracts equal run 12 apart from timestamps. | Adapter revised after independent review: the mediated file operation moved to a module shared with the other process backend; a file operation is refused with `claude_unsettled` when the wait for host records ends by a stop or by the invocation limit; `CLAUDE_CONFIG_DIR` is no longer removed from the child. |
| 14 | None; `experiment.json`, `discovery.json`, both digests and the five recorded response contracts equal run 13 apart from timestamps. The recorded prompts of the five calls differ from run 13 only in versions, in the planning allowance and in text produced by earlier calls of the run. | The adapter's fixed system prompt additionally says that a JSON response starts with `{` and ends with `}` and that analysis stays out of the final message. |

Run 01 made no native call. Its opening steps (session change, contribution
proposal, solicitation) took 1,627 ms and 1,416 ms, against 168 to 195 ms for the
same steps in runs 02 to 05. No offer was recorded, and the phase became
`Blocked(no_offers)` 1,873 ms after the solicitation event.

Run 02 intake reported input 5,275 / cache read 0 / cache write 0 / output 1,156 /
reasoning 1,061 and settled at 9,899.0 units. Planning reported input 7,401 /
cache write 4,477 / output 1,811 / reasoning 1,560 and was priced at 15,764.25
units against an admitted allowance of 12,373.75. Both receipts were Complete.
The plan output was not consumed.

Run 03 intake reported input 5,306 / output 905 / reasoning 811 (8,926.0 units).
Planning reported input 6,387 / output 1,056 / reasoning 793 (10,611.0 units,
allowance 17,852.0). Production by `claude-a` reported input 9,952 / output 807 /
reasoning 600 (13,180.0 units). Cache read and cache write were zero and all
receipts were Complete. A result was retained with `sorted.json` digest
`9378b8dc…c357`. No kernel check run, verification call or review was recorded.

Run 04 intake reported input 4,928 / output 943 / reasoning 816 (8,700.0 units),
Complete. Its output was `{"criteria":[],"questions":[]}` wrapped in one
` ```json ` fence.

Run 05 usage, all with cache read and cache write zero and Complete coverage:

| Call | Agent | Role | Input | Output | Reasoning | Units |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| `call-intake` | claude-a | Planner | 5,808 | 2,254 | 2,132 | 14,824.0 |
| `call-plan` | claude-a | Planner | 6,246 | 1,058 | 758 | 10,478.0 |
| `call-next-108-0` | claude-a | Producer | 17,953 | 1,168 | 716 | 22,625.0 |
| `call-next-174-0` | claude-b | Verifier | 6,770 | 1,143 | 751 | 11,342.0 |
| `call-review-result-next-108-0` | claude-b | Reviewer | 3,384 | 1,650 | 1,323 | 9,984.0 |

Run 05 totals by purpose: Production 22,625.0, Verification 21,326.0,
Coordination 25,302.0, Reporting 0.0. Four kernel check runs were recorded: both
candidate checks Pass, baseline `sorted-json` Fail, baseline `input-preserved`
Pass. Two evidence records of class `Executed` exist. No acceptance and no review
record exist in run 05.

Run 06 usage, all with cache read and cache write zero and Complete coverage:

| Call | Agent | Role | Input | Output | Reasoning | Units | Duration ms |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| `call-intake` | claude-a | Planner | 7,741 | 1,074 | 899 | 12,037.0 | 17,298 |
| `call-plan` | claude-a | Planner | 6,282 | 772 | 519 | 9,370.0 | 12,362 |
| `call-next-111-0` | claude-a | Producer | 13,326 | 969 | 595 | 17,202.0 | 18,394 |
| `call-next-170-0` | claude-b | Verifier | 6,532 | 940 | 519 | 10,292.0 | 21,443 |
| `call-review-result-next-111-0` | claude-b | Reviewer | 3,504 | 1,739 | 1,099 | 10,460.0 | 23,028 |

Run 06 totals by purpose: Production 17,202.0, Verification 20,752.0,
Coordination 21,407.0, Reporting 0.0. The producer's output is prose that reports
`stale_revision` errors on reading `input.json` and writing `sorted.json`. The
journal holds no file-access record for the production assignment, and
`sorted.json` has the same digest (`37517e5f…b570`, the bytes `[]` and a newline)
in the session-base, before and after snapshots. Four kernel check runs were
recorded: `input-preserved` Pass for baseline and candidate, `sorted-json` Fail
for baseline and candidate. The evidence record for `sorted-json` has polarity
`Contradicts`, and the ledger status for that criterion is `Contradicted`. The
reviewer returned JSON in the stated shape with verdict `Reject`, one `Blocking`
and one `Advisory` finding, recorded as review `review-result-next-111-0`.
Acceptance `accepted-result-next-111-0` records decision
`Rejected("failing applicable check")`, criterion grades
`Confirmed(TrustedCheck)` for `input-preserved` and `Refuted` for `sorted-json`,
and overall grade `Refuted`. The final revision is 274.

Run 07 usage, all with cache read and cache write zero and Complete coverage:

| Call | Agent | Role | Input | Output | Reasoning | Units | Duration ms |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| `call-intake` | claude-a | Planner | 4,978 | 1,660 | 1,540 | 11,618.0 | 34,322 |
| `call-plan` | claude-a | Planner | 6,286 | 837 | 567 | 9,634.0 | 24,572 |
| `call-next-108-0` | claude-a | Producer | 9,672 | 743 | 528 | 12,644.0 | 21,525 |
| `call-next-164-0` | claude-b | Verifier | 7,009 | 1,062 | 856 | 11,257.0 | 23,361 |
| `call-review-result-next-108-0` | claude-b | Reviewer | 7,489 | 1,614 | 1,190 | 13,945.0 | 26,681 |

Run 07 totals by purpose: Production 12,644.0, Verification 25,202.0,
Coordination 21,252.0, Reporting 0.0. The journal holds eight file-access
records: a read of `input.json` for intake and for planning, a read of
`input.json` and a write of `sorted.json` for production, and reads of both files
for verification and for review. `sorted.json` holds `[1,2,2,3]` and a newline
(digest `9378b8dc…c357`). Four kernel check runs were recorded: both candidate
checks Pass, baseline `sorted-json` Fail, baseline `input-preserved` Pass. Both
evidence records have class `Executed` and polarity `Supports`, and both ledger
entries have status `Satisfied`. The reviewer returned JSON in the stated shape
with verdict `Approve` and no findings, recorded as review `verdict-next-108-0`.
Acceptance `accepted-result-next-108-0` records decision `Accepted` with grade
`Confirmed(TrustedCheck)` for both criteria. The final revision is 275.

Run 07 sequence. Times are journal record times in milliseconds after the
recorded start of the experiment:

| Time | Journal record |
| ---: | --- |
| 86,874 | Producer commitment `next-108-0` activated, lease 60,000 ms. |
| 93,415 | Production invocation start. |
| 99,950 | File access prepared: production read of `input.json`. |
| 104,666 | File access prepared: production write of `sorted.json`. |
| 111,154 | Production backend terminal `Completed`. |
| 113,870 | Production invocation end (its recorded `ended` value is 111,154). |
| 123,641 | Verifier commitment `next-164-0` activated. |
| 129,981 | Verification invocation start. |
| 147,195 | Commitment `next-108-0` ended with outcome `Expired` ("Commitment lease expired"), 60,321 ms after activation. |
| 153,235 | Verification invocation end (its recorded `ended` value is 149,385). |
| 153,578 | Verifier commitment ended with outcome `Discharged`. |
| 209,494 | Review `verdict-next-108-0` recorded. |
| 212,726 | Acceptance `accepted-result-next-108-0` recorded. |
| 214,313 | Session phase `Blocked(commitment_expired)`. |
| 216,946 | Deterministic reporting started. |
| 221,049 | Delivered record written. |

The delivered record of run 07 lists `accepted-result-next-108-0` under
`accepted_sources`, while its `acceptance` and `aggregate` are `null`, its grade
is `Unconfirmed`, both criteria are listed as unmet and its single claim, of kind
`Status`, reads "Uncertain: independent final acceptance is unavailable; retained
work is not presented as a completed artifact."

Run 08 usage. No usage was recorded and no receipt exists; coverage is unknown:

| Call | Agent | Role | Input | Output | Reasoning | Units | Duration ms |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| `call-intake` | claude-a | Planner | not recorded | not recorded | not recorded | none settled; 28,000.0 held | no start record |

Run 08 totals by purpose: Coordination spent 0.0 and held 28,000.0; Production,
Verification and Reporting spent 0.0 and held 0.0. The usage fields of the
projection are input 0, cache read 0, cache write 0, output 0 and reasoning
`null`, with zero turns and no observation; they are not a provider report. The
journal holds the Dispatch and Ready records of the call at 6,305 ms after the
recorded start of the experiment. At 8,369 ms it holds the reservation change
`Revoked` and the assignment revocation, both with reason "Invocation authority
ended", and the invocation end record with terminal `Failed(Protocol)` and
`confirmed: false`. The diagnostic follows at 8,505 ms: class `Protocol`, code
`claude_environment`, message "Backend observation could not be validated". The
session phase became `Blocked(claude_environment)` at 8,751 ms, deterministic
reporting started at 8,828 ms and the delivered record was written at 9,881 ms.
The intake reservation has amount 28,000.0, state `Held`, no receipt and no
settlement; its account is marked `revoked`. These units are held: the files
record neither a release nor a charge. The delivered accounting records spent
0.0, held 28,000.0 and `unknown: true`; the treasury records `unbounded: true`
and `unsettled_usage: true`. The intake commitment, activated at 5,866 ms with a
60,000 ms lease, has no end record and its state is `Active`. No file-access,
check-run, review or acceptance record exists, and `sorted.json` holds its
initial bytes (`[]` and a newline). The final revision is 36.

Run 09 usage, all with cache read and cache write zero and Complete coverage:

| Call | Agent | Role | Input | Output | Reasoning | Units | Duration ms |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| `call-intake` | claude-a | Planner | 4,921 | 669 | 574 | 7,597.0 | 11,233 |
| `call-plan` | claude-a | Planner | 6,108 | 804 | 474 | 9,324.0 | 12,413 |
| `call-next-108-0` | claude-a | Producer | 9,646 | 673 | 419 | 12,338.0 | 17,300 |
| `call-next-164-0` | claude-b | Verifier | 6,792 | 972 | 620 | 10,680.0 | 17,987 |
| `call-review-result-next-108-0` | claude-b | Reviewer | 3,421 | 1,963 | 1,465 | 11,273.0 | 24,058 |

Run 09 totals by purpose: Production 12,338.0, Verification 21,953.0,
Coordination 16,921.0, Reporting 0.0. The journal holds six file-access records:
a read of `input.json` for intake and for planning, a read of `input.json` and a
write of `sorted.json` for production, and reads of both files for verification.
It holds none for the review assignment. `sorted.json` holds `[1,2,2,3]` and a
newline (digest `9378b8dc…c357`). Four kernel check runs were recorded: both
candidate checks Pass, baseline `sorted-json` Fail, baseline `input-preserved`
Pass. Both evidence records have class `Executed` and polarity `Supports`, and
both ledger entries have status `Satisfied`. The reviewer returned JSON in the
stated shape with verdict `Approve` and one `Advisory` finding, recorded as
review `review-result-next-108-0`. Acceptance `accepted-result-next-108-0`
records decision `Accepted` with grade `Confirmed(TrustedCheck)` for both
criteria. The final revision is 268.

Run 09 sequence, on the same time basis as the run 07 table:

| Time | Journal record |
| ---: | --- |
| 51,397 | Producer commitment `next-108-0` activated, lease 60,000 ms. |
| 56,527 | Production invocation start. |
| 62,590 | File access prepared: production read of `input.json`. |
| 66,611 | File access prepared: production write of `sorted.json`. |
| 71,413 | Production backend terminal `Completed`. |
| 74,128 | Production invocation end (its recorded `ended` value is 71,413). |
| 83,969 | Verifier commitment `next-164-0` activated, lease 60,000 ms. |
| 88,494 | Verification invocation start. |
| 108,012 | Verification invocation end (its recorded `ended` value is 104,374). |
| 108,339 | Verifier commitment ended with outcome `Discharged`. |
| 111,683 | Commitment `next-108-0` ended with outcome `Expired` ("Commitment lease expired"), 60,286 ms after activation. |
| 126,746 | Reviewer commitment `review-result-next-108-0` activated, lease 60,000 ms. |
| 132,784 | Review invocation start. |
| 158,892 | Review invocation end (its recorded `ended` value is 154,047). |
| 159,313 | Reviewer commitment ended with outcome `Discharged`. |
| 161,627 | Review `review-result-next-108-0` recorded. |
| 164,783 | Acceptance `accepted-result-next-108-0` recorded. |
| 166,336 | Session phase `Blocked(commitment_expired)`. |
| 168,950 | Deterministic reporting started. |
| 173,205 | Delivered record written. |

The delivered record of run 09 has the same shape as that of run 07: it lists
`accepted-result-next-108-0` under `accepted_sources`, while its `acceptance`
and `aggregate` are `null`, its grade is `Unconfirmed`, both criteria are listed
as unmet and its single claim has the same text.

Run 10 usage, with cache read and cache write zero and Complete coverage:

| Call | Agent | Role | Input | Output | Reasoning | Units | Duration ms |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| `call-intake` | claude-a | Planner | 5,680 | 2,087 | 1,842 | 14,028.0 | 26,461 |

Run 10 totals by purpose: Coordination 14,028.0; Production, Verification and
Reporting 0.0. The intake output is four blocks of prose followed by
`{"criteria": [], "questions": []}`; it contains no Markdown fence. The journal
holds one file-access record, a read of `input.json` for intake. No check-run,
review, acceptance or result record exists, and `sorted.json` holds its initial
bytes (`[]` and a newline). The final revision is 59.

Run 10 sequence, on the same time basis. The run has one commitment, for intake;
it has no producer commitment:

| Time | Journal record |
| ---: | --- |
| 5,945 | Intake commitment `intake` activated, lease 240,000 ms. |
| 6,395 | Intake dispatch; the recorded allowance timeout is 239,550 ms. |
| 8,525 | Intake invocation start. |
| 19,387 | File access prepared: intake read of `input.json`. |
| 33,036 | Intake backend terminal `Completed`. |
| 34,214 | Intake invocation end (its recorded `ended` value is 33,036). |
| 34,319 | Intake commitment ended with outcome `Discharged`, 28,374 ms after activation. |
| 34,816 | Session phase `Blocked(decoding)`. |
| 34,917 | Deterministic reporting started. |
| 35,790 | Delivered record written. |

Run 11 usage, all with cache read zero and Complete coverage. `Cache write`
is the reported number of cache-write tokens; the `Input` value includes them:

| Call | Agent | Role | Input | Cache write | Output | Reasoning | Units | Duration ms |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `call-intake` | claude-a | Planner | 6,254 | 0 | 2,045 | 1,927 | 14,434.0 | 24,549 |
| `call-plan` | claude-a | Planner | 6,547 | 0 | 1,156 | 861 | 11,171.0 | 15,590 |
| `call-next-108-0` | claude-a | Producer | 9,625 | 0 | 690 | 471 | 12,385.0 | 18,499 |
| `call-next-164-0` | claude-b | Verifier | 7,557 | 4,484 | 1,817 | 1,487 | 15,946.0 | 27,630 |
| `call-review-result-next-108-0` | claude-b | Reviewer | 8,001 | 4,549 | 1,489 | 1,055 | 15,094.25 | 27,493 |
| `call-final-review` | claude-b | FinalReviewer | 4,280 | 4,270 | 1,217 | 941 | 10,215.5 | 20,040 |

Run 11 totals by purpose: Production 12,385.0, Verification 41,255.75,
Coordination 25,605.0, Reporting 0.0. The journal holds eight file-access
records: a read of `input.json` for intake and for planning, a read of
`input.json` and a write of `sorted.json` for production, and reads of both files
for verification and for review. It holds none for the final-review assignment.
`sorted.json` holds `[1,2,2,3]` and a newline (digest `9378b8dc…c357`). Four
kernel check runs were recorded for the candidate result: both candidate checks
Pass, baseline `sorted-json` Fail, baseline `input-preserved` Pass. Both
evidence records for the result have class `Executed` and polarity `Supports`,
and both ledger entries have status `Satisfied`. The reviewer returned JSON in
the stated shape with verdict `Approve` and no findings, recorded as review
`review-result-next-108-0`. Acceptance `accepted-result-next-108-0` records
decision `Accepted` with grade `Confirmed(TrustedCheck)` for both criteria. The
final revision is 326.

Run 11 sequence, on the same time basis as the run 07 table:

| Time | Journal record |
| ---: | --- |
| 68,036 | Producer commitment `next-108-0` activated, lease 240,000 ms. |
| 73,457 | Production invocation start. |
| 79,439 | File access prepared: production read of `input.json`. |
| 83,933 | File access prepared: production write of `sorted.json`. |
| 89,288 | Production backend terminal `Completed`. |
| 92,035 | Production invocation end (its recorded `ended` value is 89,288). |
| 101,861 | Verifier commitment `next-164-0` activated, lease 240,000 ms. |
| 107,540 | Verification invocation start. |
| 135,749 | Verification invocation end (its recorded `ended` value is 131,908). |
| 136,091 | Verifier commitment ended with outcome `Discharged`. |
| 153,777 | Reviewer commitment `review-result-next-108-0` activated, lease 240,000 ms. |
| 160,324 | Review invocation start. |
| 189,335 | Review invocation end (its recorded `ended` value is 184,495). |
| 189,762 | Reviewer commitment ended with outcome `Discharged`. |
| 192,066 | Review `review-result-next-108-0` recorded. |
| 195,252 | Acceptance `accepted-result-next-108-0` recorded. |
| 195,903 | Commitment `next-108-0` ended with outcome `Discharged` ("Recorded kernel completion basis"), 127,867 ms after activation and 651 ms after the acceptance record. |
| 197,831 | Session phase `Finalizing`. |
| 200,983 | Finalization started and final snapshot `integrated` captured. |
| 204,646 | Four final check runs and two final evidence records. |
| 209,148 | Final reviewer selected: `claude-b`. |
| 213,310 | Final-review solicitation opened; its single offer followed at 215,440. |
| 221,432 | Final-review commitment `final-review` activated, lease 240,000 ms. |
| 223,569 | Final-review context recorded. |
| 229,022 | Final-review invocation start. |
| 252,558 | Final-review invocation end (its recorded `ended` value is 245,808). |
| 253,153 | Final-review commitment ended with outcome `Discharged`. |
| 256,946 | Session phase `Blocked(decoding)`. |
| 257,375 | Progress records `Monitor`, `Diagnosis` (outcome `Unknown`) and `Escalation`, the last at 259,485. |
| 260,316 | Deterministic reporting started. |
| 265,439 | Delivered record written. |

Run 12 usage, all with cache read zero and Complete coverage:

| Call | Agent | Role | Input | Cache write | Output | Reasoning | Units | Duration ms |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `call-intake` | claude-a | Planner | 6,565 | 4,239 | 2,316 | 2,201 | 16,888.75 | 45,736 |
| `call-plan` | claude-a | Planner | 6,204 | 0 | 869 | 526 | 9,680.0 | 16,997 |
| `call-next-108-0` | claude-a | Producer | 9,807 | 0 | 763 | 536 | 12,859.0 | 19,338 |
| `call-next-164-0` | claude-b | Verifier | 6,904 | 0 | 1,189 | 860 | 11,660.0 | 20,941 |
| `call-review-result-next-108-0` | claude-b | Reviewer | 3,433 | 0 | 2,310 | 1,726 | 12,673.0 | 21,351 |
| `call-final-review` | claude-b | FinalReviewer | 4,419 | 4,409 | 2,248 | 1,922 | 14,513.25 | 30,034 |

Run 12 totals by purpose: Production 12,859.0, Verification 38,846.25,
Coordination 26,568.75, Reporting 0.0. The journal holds six file-access
records: a read of `input.json` for intake and for planning, a read of
`input.json` and a write of `sorted.json` for production, and reads of both files
for verification. It holds none for the review and final-review assignments.
`sorted.json` holds `[1,2,2,3]` and a newline (digest `9378b8dc…c357`). The
check runs, evidence records and ledger entries for the candidate result have
the same outcomes as in run 11. The reviewer returned JSON in the stated shape
with verdict `Approve` and two `Advisory` findings, recorded as review
`review-result-next-108-0`. Acceptance `accepted-result-next-108-0` records
decision `Accepted` with grade `Confirmed(TrustedCheck)` for both criteria. The
final revision is 316.

Run 13 usage, all with cache read zero and Complete coverage:

| Call | Agent | Role | Input | Cache write | Output | Reasoning | Units | Duration ms |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `call-intake` | claude-a | Planner | 4,988 | 0 | 676 | 581 | 7,692.0 | 11,896 |
| `call-plan` | claude-a | Planner | 6,307 | 0 | 956 | 700 | 10,131.0 | 14,402 |
| `call-next-108-0` | claude-a | Producer | 9,673 | 0 | 697 | 480 | 12,461.0 | 17,714 |
| `call-next-164-0` | claude-b | Verifier | 6,864 | 0 | 1,055 | 816 | 11,084.0 | 19,494 |
| `call-review-result-next-108-0` | claude-b | Reviewer | 3,453 | 0 | 2,198 | 1,846 | 12,245.0 | 24,198 |
| `call-final-review` | claude-b | FinalReviewer | 4,501 | 4,491 | 2,848 | 2,366 | 17,015.75 | 32,797 |

Run 13 totals by purpose: Production 12,461.0, Verification 40,344.75,
Coordination 17,823.0, Reporting 0.0. The journal holds six file-access
records, for the same assignments and files as in run 12. `sorted.json` holds
`[1,2,2,3]` and a newline (digest `9378b8dc…c357`). The check runs, evidence
records and ledger entries for the candidate result have the same outcomes as in
run 11. The reviewer returned JSON in the stated shape with verdict `Approve`
and no findings, recorded as review `review-next-108-0-approved`. Acceptance
`accepted-result-next-108-0` records decision `Accepted` with grade
`Confirmed(TrustedCheck)` for both criteria. The final revision is 319.

Run 14 usage, all with cache read zero and Complete coverage:

| Call | Agent | Role | Input | Cache write | Output | Reasoning | Units | Duration ms |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `call-intake` | claude-a | Planner | 5,032 | 0 | 2,621 | 2,526 | 15,516.0 | 29,288 |
| `call-plan` | claude-a | Planner | 6,282 | 0 | 713 | 452 | 9,134.0 | 12,216 |
| `call-next-108-0` | claude-a | Producer | 9,852 | 0 | 798 | 542 | 13,044.0 | 17,883 |
| `call-next-164-0` | claude-b | Verifier | 7,364 | 4,257 | 1,382 | 1,107 | 13,956.25 | 22,526 |
| `call-review-result-next-108-0` | claude-b | Reviewer | 8,174 | 4,701 | 2,459 | 1,873 | 19,185.25 | 29,850 |

Run 14 totals by purpose: Production 13,044.0, Verification 33,141.5,
Coordination 24,650.0, Reporting 0.0. The journal holds eight file-access
records: a read of `input.json` for intake and for planning, a read of
`input.json` and a write of `sorted.json` for production, and reads of both files
for verification and for review. `sorted.json` holds `[1,2,2,3]` and a newline
(digest `9378b8dc…c357`). Four kernel check runs were recorded: both candidate
checks Pass, baseline `sorted-json` Fail, baseline `input-preserved` Pass. Both
evidence records have class `Executed` and polarity `Supports`. The ledger of
the projection has no entry. The whole intake output is
`{"criteria":[],"questions":[]}`, and the planning and verification outputs
parse as JSON. The reviewer output has 1,052 characters: three blocks of prose
(218 characters with their separators) followed by one JSON object inside a
Markdown code fence labelled `json`, with which the output ends. The fenced
text parses as JSON with the Python parser: verdict `Approve`, no findings, a
`result` equal to the retained result reference, `criteria` equal to the two
criterion references and `basis` equal to the two evidence identifiers of the
run. No review, acceptance or final-review record exists, and the session did
not reach `Finalizing`. The delivered record has empty `accepted_sources`,
`aggregate` and `acceptance` `null`, grade `Unconfirmed`, both criteria listed
as unmet, the same single claim text as in run 07, five receipts, held 0.0,
`unknown: false` and one unresolved item. The final revision is 271.

Run 14 sequence, on the same time basis as the run 07 table:

| Time | Journal record |
| ---: | --- |
| 69,486 | Producer commitment `next-108-0` activated, lease 240,000 ms; the recorded lease expiry is at 309,486. |
| 74,307 | Production invocation start. |
| 80,502 | File access prepared: production read of `input.json`. |
| 84,682 | File access prepared: production write of `sorted.json`. |
| 90,083 | Production backend terminal `Completed`. |
| 93,091 | Production invocation end (its recorded `ended` value is 90,083). |
| 102,784 | Verifier commitment `next-164-0` activated, lease 240,000 ms. |
| 107,646 | Verification invocation start. |
| 131,509 | Verification invocation end (its recorded `ended` value is 127,677). |
| 131,852 | Verifier commitment ended with outcome `Discharged`. |
| 133,603 | Four check runs and two evidence records, the last at 139,223. |
| 149,547 | Reviewer commitment `review-result-next-108-0` activated, lease 240,000 ms. |
| 155,023 | Review invocation start. |
| 167,051 | File access prepared: review read of `input.json`; the read of `sorted.json` followed at 167,735. |
| 187,669 | Review invocation end (its recorded `ended` value is 182,623). |
| 188,117 | Reviewer commitment ended with outcome `Discharged`. |
| 190,827 | Session phase `Blocked(decoding)`. |
| 191,150 | Progress records `Monitor`, `Diagnosis` (outcome `Unknown`) and `Escalation`, the last at 192,720. |
| 193,365 | Deterministic reporting started. |
| 194,981 | Narrative record; audit record, outcome `Valid`, at 196,198. |
| 197,349 | Delivered record written, 127,863 ms after the activation of the producer commitment. |

The journal of run 14 holds no end record for the producer commitment
`next-108-0`. Its state in the projection is `Active`. The last journal record,
the delivered record, was written 112,137 ms before the recorded lease expiry.
The four other commitments of the run were recorded `Discharged` 109 to 448 ms
after their invocation end record, with an event reference as basis and the
reason "Recorded kernel completion basis".

Producer commitment `next-108-0` in runs 11 to 13, on the same time basis. The
lease is 240,000 ms in each run:

| Run | Activated | Production invocation end | Acceptance recorded | Commitment ended | Outcome | After activation | After acceptance record |
| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| 11 | 68,036 | 92,035 | 195,252 | 195,903 | `Discharged` | 127,867 ms | 651 ms |
| 12 | 91,218 | 116,062 | 207,852 | 208,479 | `Discharged` | 117,261 ms | 627 ms |
| 13 | 55,138 | 78,438 | 169,960 | 170,610 | `Discharged` | 115,472 ms | 650 ms |

In each of these runs the end record of the producer commitment is the journal
record that directly follows the acceptance record, and the `Finalizing` phase
change directly follows it. That end record stores the acceptance reference
`accepted-result-next-108-0` as the basis of its `Discharged` outcome. The other
five commitments of each run were recorded `Discharged` directly after their
invocation end record, 102 to 612 ms later, and their end records store an
event reference as the basis. All eighteen end records carry the reason
"Recorded kernel completion basis".

### Final-review stage of runs 11 to 13

After the acceptance record and the discharge of the producer commitment, each
of the three runs records session phase `Finalizing`, finalization control
`Continue`, a final snapshot `integrated`, four final check runs (candidate
checks Pass on `integrated`; baseline `sorted-json` Fail and baseline
`input-preserved` Pass on `before-next-108-0`) and two final evidence records of
class `Executed` and polarity `Supports`. The final reviewer selection record
has policy `AnyNonProducer`, producers `[claude-a]`, one candidate with one
prior review, and outcome `claude-b`. A solicitation for the contribution
`final-review` received one offer, from `claude-b`, 2,130 ms, 2,102 ms and
2,183 ms after opening. The assignment has role `FinalReviewer` and access
`ReadFiles`. The aggregate reference has the same identifier, `final-27f8fcd7…8274`,
in all three runs, with a different version in each.

Times of the closing records, on the same time basis:

| Journal record | Run 11 | Run 12 | Run 13 |
| --- | ---: | ---: | ---: |
| Session phase `Finalizing` | 197,831 | 210,319 | 172,539 |
| Final snapshot captured | 200,983 | 213,380 | 175,723 |
| Final check runs and final evidence | 204,646 | 216,892 | 179,306 |
| Final reviewer selected | 209,148 | 221,285 | 183,763 |
| Final-review commitment activated | 221,432 | 233,296 | 196,179 |
| Final-review context recorded | 223,569 | 235,484 | 198,375 |
| Final-review invocation start | 229,022 | 240,297 | 203,835 |
| Final-review invocation end, `Completed`, confirmed | 252,558 | 274,724 | 240,032 |
| Final-review commitment ended, `Discharged` | 253,153 | 275,336 | 240,620 |
| Session change | 256,946, phase `Blocked(decoding)` | 279,114, `Stop` | 244,374, phase `Blocked(decoding)` |
| Progress records `Monitor`, `Diagnosis`, `Escalation` | 257,375 to 259,485 | none | 244,795 to 246,907 |
| Deterministic reporting started | 260,316 | 279,975 | 247,758 |
| Narrative record | 262,390 | 281,971 | 249,860 |
| Audit record, outcome `Valid` | 263,965 | 283,477 | 251,492 |
| Delivered record | 265,439 | 284,846 | 253,019 |

What the final reviewer was given and what it returned:

| | Run 11 | Run 12 | Run 13 |
| --- | --- | --- | --- |
| Keys of the recorded purpose | `aggregate`, `operation`, `response`, `runs` | the same and `subject` | the same and `subject`, `evidence` |
| Response contract | `FinalVerdict: id, aggregate Ref, verdict Approve/Reject/NeedsEvidence, basis Evidence IDs, rationale` | `Return only FinalVerdict JSON: {id:new review id,aggregate:{id,version} copied from subject,verdict:"Approve" or "Reject" or "NeedsEvidence",basis:[supplied Evidence ids],rationale:text}.` | The run 12 text with `basis:[ids copied exactly from evidence]` |
| Aggregate reference in the prompt | absent | under `subject` | under `subject` |
| Final evidence identifiers in the prompt | once each, in an earlier frame | once each, in an earlier frame | in an earlier frame and under `evidence` |
| Output | one line of 567 characters beginning `FinalVerdict:` | a JSON object, 698 characters | four blocks of prose followed by a JSON object, 1,236 characters |
| Whole output parses as JSON | no | yes | no |
| Verdict in the output | `Approve` | `Approve` | `Approve` |
| Aggregate in the output | the version of the `integrated` snapshot | equal to `subject` | equal to `subject` |
| Basis in the output | the identifiers of the two candidate final check runs | `check-sorted-json`, `check-input-preserved` | the two identifiers supplied under `evidence` |
| `failure` in `summary.json` | `null` | `final_review_basis: Final review basis is not applicable` | `null` |
| Session phase in the projection | `Blocked(decoding)` | `Cancelled`, with `stopped: true` | `Blocked(decoding)` |
| Delivered outcome | `Blocked(final_acceptance_unavailable)` | `Cancelled` | `Blocked(final_acceptance_unavailable)` |

The rows "Whole output parses as JSON", "Aggregate in the output" and "Basis in
the output" were computed from the recorded output with the Python JSON parser
and by string comparison. They are not the kernel's decoding result, which the
run files do not record.

In run 12 the journal holds no record of the refusal. The failure code
`final_review_basis` and its text occur only in `summary.json` and in the summary
printed in the log; they occur in no journal record and in no projection value.
The journal order after the final-review call is: invocation end (`Completed`,
confirmed), final-review commitment `Discharged`, `SessionChanged` with change
`Stop`, reporting started, report prepared, narrative, audit, delivered. No
`Phase` record and no progress record follows the stop. The value `Cancelled`
occurs in two journal records, the narrative input and the delivered record. In
the projection the session phase is `Cancelled` and `stopped` is `true`, while
the finalization `outcome` value is `Blocked(final_acceptance_unavailable)` and
the finalization `stopped` value is `false`.

The delivered records of runs 11 to 13 list `accepted-result-next-108-0` under
`accepted_sources` and carry the aggregate reference under `aggregate`, which is
`null` in runs 07 and 09. Their `acceptance` is `null`, their grade is
`Unconfirmed`, both criteria are listed as unmet, and the single claim has the
same text as in run 07, in run 12 as well. Delivered accounting records held 0.0,
`unknown: false`, six receipts and none unresolved. In all three runs the
projection holds no review and no acceptance of the aggregate, and the report
record has `narrated: false`.

## Findings

- The mediated file boundary produced the expected artifact eight times (runs
  03, 05, 07, 09, 11, 12, 13 and 14) with production assigned to `claude-a`. In
  run 06 it produced no file change.
- Run 05 is the first run in which a second identity, `claude-b`, performed paid
  verification and review work. Its review was not decoded.
- With the response shape stated in the review contract (runs 06, 07, 09 and
  11 to 14), the reviewer returned JSON that was recorded as a review in six of
  seven runs. The recorded verdicts agree with the kernel check outcomes of
  their runs: `Reject` with a failing candidate check, `Approve` with passing
  candidate checks. In run 14, with the same recorded contract, the reviewer
  returned prose followed by fenced JSON, and no review was recorded.
- The recorded goal of every run ends with the sentence "Follow each role's
  structured response schema without Markdown fences." The outputs of run 04
  (intake) and run 14 (review) contain a Markdown fence.
- Runs 07, 09, 11, 12 and 13 are the runs with a recorded acceptance decision
  `Accepted`. Each is an acceptance of one result version. None became final
  acceptance. The session phase after the acceptance is
  `Blocked(commitment_expired)` in runs 07 and 09 and `Finalizing` in runs 11,
  12 and 13.
- Runs 11, 12 and 13 are the runs that reached the final-review stage. Each
  made one final-review call that ended `Completed` with a Complete receipt, and
  each final reviewer output states verdict `Approve`. No run records a review
  or an acceptance of the aggregate. The three outputs differ from what was asked
  in three different ways: text that is not JSON (run 11), JSON whose basis
  names check identifiers (run 12), and the requested JSON preceded by prose
  (run 13).
- Each change to the final-review purpose was followed by the corresponding
  change in the output. With the aggregate reference supplied under `subject`
  (runs 12 and 13) the output's aggregate equals it; in run 11, whose prompt does
  not contain that reference, the output names a snapshot version. With the
  evidence identifiers supplied under `evidence` (run 13) the output's basis
  equals them. These are single runs.
- Run 12 is the only run whose delivered outcome is `Cancelled` and the only run
  whose `summary.json` records a failure text. Its journal records the stop and
  not its reason.
- The producer commitment was recorded `Expired` in runs 05, 06, 07 and 09,
  61,770 ms, 60,282 ms, 60,321 ms and 60,286 ms after activation, in each case
  after the production invocation end had been recorded. In those runs the
  intake, planning, verification and review commitments were each recorded
  `Discharged` within 500 ms after their invocation end record; no such record
  follows the production invocation end. In run 03 the producer commitment was
  still `Active` at the end of the session. Only in runs 07 and 09 is the expiry
  the recorded blocking reason. In run 07 the expiry was recorded while the
  verification call was running; in run 09 it was recorded after the verifier
  commitment had been discharged and before the review solicitation opened.
- Every activated commitment lease equals the resource policy timeout of its
  run: 60,000 ms in runs 02 to 09 and 240,000 ms in runs 10 to 14. Run 10
  activated only the intake commitment.
- Under the 240,000 ms lease the producer commitment was recorded `Discharged`
  in runs 11, 12 and 13, 127,867 ms, 117,261 ms and 115,472 ms after activation.
  Each of these intervals is longer than 60,000 ms and shorter than the lease.
  The end record follows the acceptance record by 651 ms, 627 ms and 650 ms; it
  does not follow the production invocation end, which was recorded 23,999 ms,
  24,844 ms and 23,300 ms after activation. Its stored basis is the acceptance
  reference. No commitment expired in runs 10 to 14. No run with a 60,000 ms
  lease records a producer commitment with outcome `Discharged`.
- In run 14, which records no acceptance, the producer commitment has no end
  record. It was `Active` when the delivered record was written, 127,863 ms
  after its activation and 112,137 ms before its recorded lease expiry. Run 03
  is the other run that ended with an `Active` producer commitment.
- Run 08 made one call that ended `Failed(Protocol)` 2,064 ms after dispatch,
  with no start record and no backend observation. The journal records the
  diagnostic code `claude_environment`; it does not record which environment
  property was refused. The cause given by the operator, that the native client
  always reports two built-in plugins while the isolation check required an
  empty list, is not recorded in run files.
- Run 10 stopped at intake with phase `Blocked(decoding)`. Its intake output
  ends with the JSON value that runs 02, 03, 05, 06, 07 and 09 returned as the
  whole intake output, preceded by prose. Run 04 stopped with the same phase on
  a fenced intake output. In runs 11 to 14 the whole intake output is
  `{"criteria":[],"questions":[]}`. The final-review output of run 13 has the
  same form as the run 10 intake output, prose followed by the requested JSON,
  and run 13 ended with the same phase. The review output of run 14 combines
  both forms, prose followed by the requested JSON inside a fence, and run 14
  ended with the same phase.
- The run 06 producer reported `stale_revision` refusals. The journal contains
  that code only in the producer's output and in records that quote it; it
  holds no refusal record. The cause given by the operator, that the host was
  journaling usage and progress observations at the same revision, is not
  recorded in run files.
- In run 07 each production file-access record follows the host's records for
  the observations emitted before it. In run 05 the corresponding records are
  interleaved with them. This ordering is consistent with the change described
  for run 07 and does not prove it.
- Offer arrival is slower later in a session. In runs 05, 06, 07 and 09 the
  single offers for the verification and review solicitations arrived 1,128 to
  1,668 ms after opening, and the second production offer after 1,630 to
  1,655 ms. A 1,000 ms window would have excluded all of them; this matches the
  run 03 stop. In runs 11, 12 and 13 the verification offers arrived after
  1,106 to 1,130 ms, the review offers after 1,367 to 1,403 ms, the second
  production offers after 1,631 to 1,650 ms and the final-review offers after
  2,102 to 2,183 ms. In run 14 the verification offer arrived after 1,113 ms,
  the review offer after 1,381 ms and the second production offer after
  1,627 ms.
- The recorded response contracts for intake and planning name JSON explicitly.
  The run 05 review contract does not contain the word JSON, and that reviewer's
  answer mirrors the contract's field list as key=value text. The run 11
  final-review contract does not contain the word JSON either, and that
  reviewer's answer mirrors the contract's field list as one comma-separated
  line.
- The verifier's prose in run 05 attributes the work to `claude-b`, although the
  recorded producer is `claude-a`.
- Recorded charges are consistent with `input` counted inclusive of cache-write
  tokens and with reasoning tokens not priced separately from output. Six calls
  of runs 11 to 13 reported cache-write tokens: the verification, review and
  final-review calls of run 11 (4,484, 4,549 and 4,270), the intake and
  final-review calls of run 12 (4,239 and 4,409) and the final-review call of
  run 13 (4,491). Every recorded charge of these runs equals input plus 0.25
  times cache write plus 4 times output. In the three final-review calls all but
  ten of the input tokens were reported as cache-write tokens. Two calls of run
  14 reported cache-write tokens, verification (4,257) and review (4,701), and
  its five charges follow the same formula.
- Among the runs with an accepted result, the review assignment has file-access
  records in runs 07 and 11 and none in runs 09, 12 and 13; the reviewer's
  input is 7,489 and 8,001 tokens in the former and 3,421 to 3,453 tokens in
  the latter. No final-review assignment has a file-access record. In run 14
  the review assignment has two file-access records and the reviewer's input is
  8,174 tokens.
- Every invocation that has a start record carries a `lease_renewal` diagnostic
  ("Recorded progress did not renew the commitment"); progress signals were
  heartbeats only. The run 08 call carries only the `claude_environment`
  diagnostic.

These are single bounded runs. They do not establish a rate, a causal benefit or
superiority over the GPT pilot.

## Unknown coverage and unverified items

- All 50 final receipts (runs 02 to 07 and 09 to 14) are Complete. No receipt
  has unknown or Partial final coverage. Intermediate Partial receipts appear in
  the journal before each terminal receipt.
- The run 08 call has no receipt. Its usage is unknown, and 28,000.0 units
  remain held in its reservation. They are not counted as spent and were not
  released. No other run has held units. Whether the native process consumed
  provider tokens in run 08 is unknown from the run files.
- The described retention of reported counters as Partial when the response text
  is undeliverable was not exercised: no run has a final Partial receipt.
- Complete coverage is provider-reported, not independent billing verification.
- The build profile, the adapter source state and the adapter guidance text sent
  to the native process are not recorded in run files.
- The recorded dispatch prompt is identical between runs 04 and 05 apart from the
  contract version. Between runs 05 and 06 the only recorded prompt change is the
  review response contract.
- Why the run 06 production assignment has no file-access record is unknown from
  the run files. Run 06 also has none for intake, planning and review, where
  runs 05 and 07 have them for intake and planning.
- Why the producer commitment is not discharged when production ends is not
  recorded.
- The adapter revision described for run 08 and the change described for run 09
  are not recorded in run files. The backend policy digest is the same in all
  fourteen runs.
- The adapter revisions described for run 13 are not recorded in run files. The
  strings `claude_unsettled` and `CLAUDE_CONFIG_DIR` occur in no journal record,
  projection or log of runs 11 to 13, so the described refusal was not exercised
  in these runs. Whether the run 13 binary contained those revisions is not
  recorded.
- The system prompt change described for run 14 is not recorded in run files.
  No run file records the adapter's system prompt, and the phrases of the
  described addition occur in no journal record, projection value or log of
  run 14. Whether the run 14 binary contained the change is not recorded.
- Which part of the run 10 intake output the decoding step refused is not
  recorded. The journal records the phase change after the intake commitment was
  discharged, with no separate decoding diagnostic. The same holds for the
  final-review outputs of runs 11 and 13: the phase change follows the discharge
  of the final-review commitment, and the word "decoding" occurs in no other
  journal record of those runs. It also holds for the review output of run 14,
  where the phase change follows the discharge of the reviewer commitment.
- Whether the adapter changed the run 14 review output before the kernel
  received it is not recorded. The recorded output contains the prose and the
  fence. Whether the JSON inside the fence would have been recorded as a review
  without them is not shown by the run files.
- Whether the run 13 final-review output would have been accepted without its
  leading prose is not shown by the run files.
- The reason for the run 12 stop is recorded in `summary.json` and the log only.
  Which component refused the basis, and at which journal revision, is not
  recorded in the journal.
- In runs 11 to 13 the end record of the producer commitment names the
  acceptance as its basis. The rule that selects this basis is not recorded in
  run files.
- In runs 08 and 10 the journal time of the delivered record (9,881 ms and
  35,790 ms) is later than `elapsed_ms` in `summary.json` (9,647 and 35,568).
  In runs 11, 12 and 13 it is earlier (265,439, 284,846 and 253,019 ms against
  266,019, 285,149 and 253,456), and in run 14 as well (197,349 ms against
  197,683). How `elapsed_ms` is measured is not recorded in run files.
- `sorted_bytes_match` in run 03 comes from the example's summary comparison; the
  kernel recorded no check run in that run.
- Run 01 has no log file. The logs of runs 06 to 14 have no shell timing line,
  so no shell wall time is recorded for them; `elapsed_ms` comes from
  `summary.json`. The logs of runs 08 to 14 end with the line `exit=0`, in run
  12 as well, whose summary records a failure.
- Run 14 was executing while runs 11 to 13 were added to this evidence and was
  not read then. It was read after it had ended and is covered here.
- See [`NOTES.md`](NOTES.md) for disagreements with the described context and
  for the extraction commands.

## Evidence class

This is **native** evidence: real native provider calls through the delivered
adapter, with `source: Native` in discovery and in the backend parameters. It is
distinct from protocol-fixture evidence and from Scripted runs, and must not be
merged with either.

The compact evidence is the content of this directory: this file,
[`NOTES.md`](NOTES.md), [`derive.py`](derive.py) and `run-01.json` to
`run-14.json`. It preserves the observed inputs, effective parameters, usage and
outcomes. The full journals and logs stayed in the operator's local run
directories, which were the inputs of `derive.py`. Never restart a native run
solely because its process or conversation disappeared; inspect the journal.

## Summary of the pilot

Counted from the fourteen run files:

| Session ended with | Runs | Count |
| --- | --- | ---: |
| Phase `Blocked(decoding)` | 04, 05, 10, 11, 13, 14 | 6 |
| Phase `Blocked(no_offers)` | 01, 03 | 2 |
| Phase `Blocked(commitment_expired)` | 07, 09 | 2 |
| Phase `Blocked(work_failed)` | 02 | 1 |
| Phase `Blocked(candidate_rejected)` | 06 | 1 |
| Phase `Blocked(claude_environment)` | 08 | 1 |
| Phase `Cancelled` | 12 | 1 |

Six of the fourteen runs ended with phase `Blocked(decoding)`. In each of them
the phase record follows a call that ended `Completed` with a Complete receipt
and whose recorded output is not, as a whole, the requested value:

| Run | Call | Role | Agent | Recorded output |
| --- | --- | --- | --- | --- |
| 04 | `call-intake` | Planner | claude-a | The expected JSON inside a Markdown code fence. |
| 05 | `call-review-result-next-108-0` | Reviewer | claude-b | key=value text, not JSON. |
| 10 | `call-intake` | Planner | claude-a | Prose followed by the expected JSON. |
| 11 | `call-final-review` | FinalReviewer | claude-b | One text line beginning `FinalVerdict:`, not JSON. |
| 13 | `call-final-review` | FinalReviewer | claude-b | Prose followed by the requested JSON. |
| 14 | `call-review-result-next-108-0` | Reviewer | claude-b | Prose followed by the requested JSON inside a Markdown code fence. |

By role these are two intake responses, two candidate-review responses and two
final-review responses. The journal records the phase and no separate record of
the decoding step. In run 12 the final-review output parses as JSON; that run
ended `Cancelled` with the failure `final_review_basis` in `summary.json`.

No run reached final acceptance. Every delivered record has `acceptance: null`
and report grade `Unconfirmed`, and no projection holds a review or an
acceptance of an aggregate. Five runs (07, 09, 11, 12 and 13) recorded the
acceptance of one result version; three of them (11, 12 and 13) reached the
final-review stage.

Across the fourteen runs 51 native calls were made. The recorded spent units
sum to 619,016.25. The recorded held units sum to 28,000.0, all of them in the
intake reservation of run 08, which has no receipt and no settlement; they
remain held. These are relative token weights, not currency or provider
billing.

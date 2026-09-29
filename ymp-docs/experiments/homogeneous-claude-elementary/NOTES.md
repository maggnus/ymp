# Notes for the Claude pilot evidence

Scope: runs 01 to 14. Runs 01 to 05 were covered by the first version of these
files; runs 06 and 07 were added afterwards by the same derivation, runs 08, 09
and 10 in a second extension, runs 11, 12 and 13 in a third extension and run
14 in a fourth. Statements marked "as recorded by the operator" come from the
description supplied with the runs and were not confirmed from run files. Items
1 to 15 below were written for runs 01 to 07; items 16 to 31 belong to the
second extension, items 32 to 47 to the third and items 48 to 59 to the fourth.
Earlier items were left as written, apart from the counts of runs in items 2, 4
and 21 and a pointer in item 28.

Run 14 was executing while the third extension was being prepared. Its
directory and log were then not read, not listed beyond their names and not
touched. They were read for the fourth extension, after the log had received
its `exit=0` line.

Run 15 was added in a fifth extension, described in its own section at the end
of this file. It has no run file, and nothing in items 1 to 59 or in the
sections before that one covers it.

These files were prepared in the operator's local pilot directory,
`/tmp/ymp-claude-pilot`, where this directory had the name `evidence-draft`.
The commands and scratch paths quoted below are given as they were used there.

## Disagreements between the described context and the run files

1. **Run 02 stop reason.** Described as `cost_limit`. The files record
   `cost_limit` as a diagnostic code on `call-plan`, the invocation terminal as
   `Failed(Content)`, and the session phase as `Blocked(work_failed)`. The backend
   terminal for that call is `Completed`, with `confirmed_terminal: false`.
2. **Final outcome.** `final_acceptance_unavailable` is the delivered outcome and
   the `summary.json` status of runs 01 to 11, 13 and 14. Run 12 has
   `Cancelled` for
   both (item 39). The per-run stop codes
   (`no_offers`, `work_failed`, `decoding`, `candidate_rejected`,
   `commitment_expired`, `claude_environment`) are the session phase, recorded
   in `view.json`.
3. **Run 02 "optimized example build".** Not recorded in run files. The logs for
   runs 02 to 05 show the executed binary under `target/debug/examples/`. The
   repository manifest has no `[profile]` section and no `.cargo` directory, so
   the optimization setting cannot be confirmed from files. Event cadence is about
   eight times faster in runs 02 to 05 than in run 01, which is consistent with
   the description but does not prove it. The logs of runs 06 and 07 do not show
   the executed binary.
4. **Run 05 adapter change.** Not recorded in run files. `experiment.json`, the
   driver digest and the backend policy digest (`976f746a…5642`) are identical in
   runs 04 and 05, and the backend digest is identical in all fourteen runs. The
   recorded dispatch prompt differs between runs 04 and 05 only in the contract
   version. The current working tree does contain a fence-stripping function and
   a guidance constant asking for raw text in
   `crates/ymp-runtime/src/backends/claude.rs`, but nothing ties that source state
   to run 05.
5. **Run 01 cause.** "Dispatcher tick longer than the offer window" is not
   recorded. The files show only that no offer was recorded and that event cadence
   was slow.
6. **Native tool name.** Discovery records provider kind `Claude` and version
   `2.1.284`. The product name "Claude Code" does not appear in run files.
7. **Run 06 session phase.** The description names the acceptance decision and
   grade. The recorded session phase is `Blocked(candidate_rejected)` (journal
   record 266); the delivered outcome is `Blocked(final_acceptance_unavailable)`.
8. **Run 06 refusals.** Described as every producer mediated file operation being
   refused with `stale_revision`. The journal holds no refusal record. The code
   appears in six records: the producer's output observation (158), the submitted
   result summary (170) and four later records that quote that text (235, 236,
   251, 263). `run06.log` does not contain it. What the files do show: no
   file-access record exists for the production assignment, and `sorted.json` has
   the same digest in the session-base, before and after snapshots. The described
   cause, the host journaling usage and progress observations at the same
   revision, is as recorded by the operator.
9. **Run 06 file access outside production.** Not part of the description. Run
   06 has file-access records only for the verifier (two reads). It has none for
   intake, planning and review. Runs 05 and 07 have a read for intake and for
   planning. Whether those run 06 calls attempted file operations is unknown.
10. **Run 06 change.** Recorded in run files as the review response contract text.
    The current working tree contains that text in
    `crates/ymp-kernel/src/acceptance/paid_review.rs`; nothing else ties that
    source state to run 06.
11. **Run 07 adapter change.** Not recorded in run files. `experiment.json`, both
    digests and every recorded response contract are identical in runs 06 and 07
    apart from timestamps. Journal
    ordering is consistent with the description: in run 07 the production
    file-access records 136 and 142 follow the host's records for the preceding
    observations (130 to 135 and 137 to 141); in run 05 the production
    file-access record 134 lies between a cost record and a progress record, and
    148 lies between a usage observation (147) and the host's records for it
    (149 to 151). This does not prove the change.
12. **Run 07 production end time.** Described as about 113.8 s. The host's
    invocation end record is at 113,870 ms; the `ended` value stored in that
    record and the backend terminal observation are at 111,154 ms.
13. **Commitment expiry is not specific to run 07.** The producer commitment was
    also recorded `Expired` in run 05 (record 221, 61,770 ms after activation)
    and run 06 (record 210, 60,282 ms). The compact file for run 05 does not
    carry this, because it has no `acceptance_path` section. In run 03 the
    producer commitment was left `Active`; in run 02 the planning commitment was
    left `Active`.
14. **Run 07 delivered record against the ledger.** Both ledger entries are
    `Satisfied` and the result acceptance is `Accepted`, while the delivered
    report lists both criteria as unmet with grade `Unconfirmed`, `acceptance`
    `null` and `aggregate` `null`. This is what the files record; it is not
    interpreted here.
15. **Wall time of runs 06 and 07.** `run06.log` and `run07.log` end with the
    summary and have no shell timing line. The values 186,233 ms and 221,560 ms
    are `elapsed_ms` in `summary.json`.
16. **Run 08 adapter revision.** Not recorded in run files. `experiment.json`,
    the driver digest, the backend policy digest and the recorded intake
    response contract are identical in runs 07 and 08 apart from timestamps; the
    recorded intake prompt differs only in the contract version. None of the six
    described changes (shared child-process channel module, refusal code
    journaled as the diagnostic, Partial retention of reported counters,
    first-party account requirement, removal of environment variables, empty
    `plugins` list) appears in a run file. The current working tree contains
    `crates/ymp-runtime/src/backends/process.rs` and, in
    `crates/ymp-runtime/src/backends/claude.rs`, a refusal with the code
    `claude_environment`; nothing ties that source state to run 08.
17. **Run 08 diagnostic.** Described as the refusal code that ended observation,
    journaled as the diagnostic. The journal holds one diagnostic record (29)
    with class `Protocol`, code `claude_environment` and message "Backend
    observation could not be validated". It follows the invocation end record
    (28). The journal holds no record that names the code as a refusal and no
    backend observation record for this call, so the files confirm the code and
    not the mechanism.
18. **Run 08 cause.** The cause given by the operator, that the native client
    always reports two built-in plugins and the requirement of an empty list was
    wrong, is not recorded. The strings `plugin` and `builtin` occur in no
    journal value, projection, log, `experiment.json` or `discovery.json` of
    runs 08 to 10. Discovery of run 08 equals that of run 07 apart from
    `observed_at`.
19. **Run 08 "no usage reported".** The files hold no usage observation and no
    receipt for the call. The projection nevertheless carries usage fields:
    input 0, cache read 0, cache write 0, output 0, reasoning `null`, turns 0.
    These are the absence of a report, not a reported zero. The delivered
    accounting has `unknown: true`, and the treasury has `unbounded: true` and
    `unsettled_usage: true`. Whether the native process consumed provider tokens
    before the call ended is unknown.
20. **Run 08 held units.** Confirmed: spent 0.0, held 28,000.0, all under
    Coordination. The reservation record has state `Held`, no receipt and no
    settlement. The same account is marked `revoked: true`, and journal record
    25 is a reservation change `Revoked` with reason "Invocation authority
    ended". In these files the revocation concerns the authority of the
    reservation; the amount stays held. No record releases or charges it.
21. **Run 08 receipt count.** `report.accounting.receipts` is 1 in
    `run-08.json` although no receipt exists. The field is a count of the source
    reference list, and its single reference equals the `last` reference of the
    intake account. `report.unresolved` is 1 in run 08, as in runs 02, 03 and
    14, and 0 in the other ten runs. `report.accounting.unknown` is `true` in
    run 08 only.
22. **Run 08 session phase and commitment.** Not part of the description. The
    session phase is `Blocked(claude_environment)` (journal record 30). The
    intake commitment has no end record and its state is `Active`, with a lease
    that expires 60,000 ms after activation, later than the end of the run.
23. **Run 08 invocation terminal.** Confirmed as `Failed(Protocol)`, with
    `confirmed_terminal: false` and `backend_terminal: null`. The call has no
    start record, so it has no `native_session` value, no reported settings and
    no duration. The end record is 2,064 ms after the dispatch record.
24. **Run 09 change.** Not recorded in run files. `experiment.json`, the driver
    digest, the backend policy digest and every recorded response contract are
    identical in runs 07, 08 and 09 apart from timestamps. The current working
    tree contains an isolation check that accepts plugins whose `path` is
    `builtin` in `crates/ymp-runtime/src/backends/claude.rs`; nothing ties that
    source state to run 09. What the files show is that the run 09 intake call,
    unlike the run 08 intake call, has a start record and backend observations.
25. **Run 09 summary.** Confirmed: five calls, `sorted.json` equal to the
    expected bytes, session phase `Blocked(commitment_expired)`, delivered
    outcome `Blocked(final_acceptance_unavailable)`, `elapsed_ms` 173,615.
26. **Run 09 differences from run 07 that the description does not mention.**
    The review has one `Advisory` finding (run 07: none) and the identifier
    `review-result-next-108-0` (run 07: `verdict-next-108-0`). The journal holds
    no file-access record for the review assignment (run 07: two reads). The
    producer commitment was recorded `Expired` after the verifier commitment had
    been discharged (run 07: while the verification call was running).
27. **Run 10 change.** Visible in `experiment.json`: the parameters of the
    `ResourcePolicy` entry (`PurposeBounded`) have `timeout 240000`, where runs
    01 to 09 have `60000`. Two further values differ from run 09: the `params`
    digest of that policy (`b0d423e7…acab`, previously `90d03ca5…1e8b`) and
    `driver_digest` (`7648e234…7687`, previously `2becdbb3…e58c`). Apart from
    these and the timestamps (`started_at_ms` and the deadline derived from it)
    nothing differs. The `EscalationPolicy` parameter `timeout_ms` is 60000 in
    both runs. Discovery of run 10 equals that of run 09 apart from
    `observed_at`. The change also appears in the journal and the projection:
    assignment allowance `timeout 240000`, dispatch allowance `timeout 239550`
    and a lease of 240,000 ms on the intake commitment.
28. **Run 10 reason for the change.** Described as: a producer's commitment
    lease is bounded by the base invocation timeout and is discharged only by
    acceptance. The files show that every activated lease equals the resource
    policy timeout of its run. They do not show a producer commitment that was
    discharged, by acceptance or otherwise: it is `Active` in run 03 and
    `Expired` in runs 05, 06, 07 and 09, and in runs 07 and 09 the acceptance
    record follows the expiry record. This was written before runs 11 to 13
    existed; item 34 records what those runs show.
29. **Run 10 outcome.** Not known when the description was written. The files
    record one call, 14,028.0 units spent, zero held, `elapsed_ms` 35,568,
    session phase `Blocked(decoding)` and delivered outcome
    `Blocked(final_acceptance_unavailable)`. The run did not reach planning or
    production, so it holds no evidence about a producer commitment under the
    240,000 ms timeout.
30. **Run 10 state when this extension started.** Described as still executing.
    `run10.log` already ended with the line `exit=0` when it was first read
    (16:08:55 local time); its modification time and that of the run 10 journal
    files is 16:08:40. No waiting was needed.
31. **Elapsed time against journal time.** In runs 08 and 10 the journal time
    of the delivered record, counted from `started_at_ms`, is 9,881 ms and
    35,790 ms, later than `elapsed_ms` in `summary.json` (9,647 and 35,568). In
    runs 07 and 09 it is earlier (221,049 against 221,560; 173,205 against
    173,615). How `elapsed_ms` is measured is not recorded in run files.
32. **Run 11 parameters.** Described as a repeat of run 10 with nothing
    changed. Confirmed as far as run files go: `experiment.json` equals that of
    run 10 apart from `started_at_ms` and the deadline derived from it,
    `discovery.json` equals it apart from `observed_at`, and the driver digest
    (`7648e234…7687`) and the backend policy digest are the same. Run 10 made
    one call, so only its intake contract can be compared; the response
    contracts of the first five calls of run 11 equal those of run 09. The
    source and build state of the run are not recorded.
33. **Run 11 figures.** Confirmed: six calls, every call `Completed` with a
    Complete receipt, 79,245.75 units spent, zero held, `elapsed_ms` 266,019,
    `sorted.json` equal to the expected bytes, session phase
    `Blocked(decoding)`, delivered outcome
    `Blocked(final_acceptance_unavailable)`, a sixth call in the `Finalizing`
    phase.
34. **Producer commitment in runs 11 to 13.** Described for run 11 as "did not
    expire". The files record more than that: in each of the three runs the
    commitment `next-108-0` has a 240,000 ms lease and an end record with
    outcome `Discharged`, at 195,903 ms, 208,479 ms and 170,610 ms after the
    recorded start, which is 127,867 ms, 117,261 ms and 115,472 ms after
    activation and 651 ms, 627 ms and 650 ms after the acceptance record. The
    end record is the journal record directly after the acceptance record
    (266 after 265 in run 11; 259 after 258 in runs 12 and 13), and its outcome
    stores the acceptance reference `accepted-result-next-108-0` as basis. The
    end records of the other commitments store an event reference. Each of the
    three intervals is longer than the 60,000 ms lease of runs 02 to 09. The
    files therefore agree with the reason given for the run 10 change (item
    28); they do not show what would have happened under the shorter lease.
35. **Run 11 final-review output.** Described as one text line beginning
    `FinalVerdict:` instead of JSON. Confirmed: the output is one line of 567
    characters and does not parse as JSON. Not part of the description: the
    recorded response contract of that call does not contain the word JSON; the
    recorded purpose has the keys `aggregate`, `operation`, `response` and
    `runs` and no `subject`; the identifier and the version of the aggregate
    reference occur nowhere in the recorded prompt; the second field of the
    output line is the version of the `integrated` snapshot; and the bracketed
    list in the line names the identifiers of the two candidate final check
    runs, not evidence identifiers. The two final evidence identifiers occur
    once each in the prompt, in an earlier frame.
36. **Run 12 change.** Described as: the purpose names the aggregate reference
    under `subject` and the response contract states the JSON shape of
    `FinalVerdict`. Confirmed from the recorded prompt of `call-final-review`.
    `experiment.json`, both digests and the five other response contracts equal
    run 11 apart from timestamps. In the current working tree the phrase
    `basis:[supplied Evidence ids]` of the run 12 contract occurs in the
    candidate-review contract
    (`crates/ymp-kernel/src/acceptance/paid_review.rs`) and not in the
    final-review source, which holds the run 13 text. The source state of run
    12 is not recorded.
37. **Run 12 figures.** Confirmed: six calls, every call `Completed` with a
    Complete receipt, zero held, `elapsed_ms` 285,149, `sorted.json` equal to
    the expected bytes. "Spent 78,274" is recorded as 78274.0.
38. **Run 12 verdict.** Confirmed: the output parses as JSON, its verdict is
    `Approve`, its aggregate equals `subject`, and its basis is
    `["check-sorted-json", "check-input-preserved"]`. Neither value is the
    identifier of a final evidence record of that run. Not part of the
    description: the `id` in the output is `final-review-` followed by 63
    hexadecimal digits, which are the digits of the first final evidence
    identifier without its leading zero.
39. **Run 12 refusal and cancellation.** Described through `summary.json`:
    failure `final_review_basis: Final review basis is not applicable`, outcome
    `Cancelled`. Both are confirmed in `summary.json`, whose `status` is also
    `Cancelled`. What the journal and the projection record:
    - No journal record and no projection value contains the failure code or
      its text. They occur in `summary.json` and in the summary printed in
      `run12.log`. No journal record contains a refusal or rejection of the
      verdict.
    - The order of the closing journal records is: 307 receipt settled; 308
      lock released; 309 invocation end, `Completed`, confirmed; 310
      final-review commitment `Discharged`; 311 `SessionChanged` with change
      `Stop`; 312 reporting started, mode `Deterministic`; 313 report prepared;
      314 narrative, whose input outcome is `Cancelled`; 315 audit, outcome
      `Valid`; 316 delivered, outcome `Cancelled`.
    - No `Phase` record follows the stop, and no progress record. Runs 11 and
      13 have three progress records after their phase change.
    - The value `Cancelled` occurs in records 314 and 316 only.
    - In the projection `session_state.phase` is `Cancelled` and
      `session_state.stopped` is `true`; `finalization.stopped` is `false`,
      `finalization.control` is `Continue`, and `finalization.outcome` is
      `Blocked(final_acceptance_unavailable)`, while
      `finalization.delivered.outcome` is `Cancelled`.
    - The delivered claim text is the same as in the other runs and speaks of
      unavailable final acceptance, not of a cancellation.
    - `finalization.reviews` is empty and `finalization.acceptance` is `null`.
    - `run12.log` ends with the line `exit=0`.
    The description does not give a session phase for run 12. The current
    working tree contains the failure code and text in
    `crates/ymp-kernel/src/finalization/review.rs`; nothing ties that source
    state to the run.
40. **Run 13 change (a).** Confirmed from the recorded prompt: the purpose has
    the key `evidence` with the two final evidence identifiers of the run, and
    the contract reads `basis:[ids copied exactly from evidence]`. The current
    working tree contains that purpose and text in
    `crates/ymp-kernel/src/finalization/context.rs`.
41. **Run 13 change (b), adapter revisions.** Not recorded in run files.
    `experiment.json`, the driver digest and the backend policy digest equal
    those of runs 10 to 12. The strings `claude_unsettled` and
    `CLAUDE_CONFIG_DIR` occur in no journal record, projection value or log of
    runs 11 to 13, so no file operation was refused with that code in these
    runs. The current working tree contains
    `crates/ymp-runtime/src/backends/files.rs`, calls into it from the other
    process backend, a refusal with the code `claude_unsettled` in
    `crates/ymp-runtime/src/backends/claude.rs`, and a reference to
    `CLAUDE_CONFIG_DIR` in `crates/ymp-runtime/src/backends/claude/stream.rs`.
    Nothing ties that source state to run 13. The modification time of
    `claude.rs` (16:26:58 local time) is later than that of the run 13 journal
    files (16:26:26), so the file was changed or rewritten after the run ended.
42. **Run 13 figures.** Confirmed: six calls, every call `Completed` with a
    Complete receipt, 70,628.75 units spent, zero held, `elapsed_ms` 253,456,
    `sorted.json` equal to the expected bytes, session phase
    `Blocked(decoding)`, delivered outcome
    `Blocked(final_acceptance_unavailable)`.
43. **Run 13 final-review output.** Described as several blocks of prose
    followed by `FinalVerdict` JSON. The output has 1,236 characters: four
    blocks of prose (587 characters with their separators) and one JSON object
    on one line. It has no Markdown fence. The text from the first brace to the
    end parses as JSON with the Python parser; its verdict is `Approve`, its
    aggregate equals `subject` and its basis equals the two identifiers
    supplied under `evidence`. The `id` in the output is
    `final-review-56f8c3a7b2e1d9f4c6a0`. Which part of the output the decoding
    step refused is not recorded.
44. **Source checkpoint.** Commit `197aaa7b` plus the uncommitted W1-0020
    working tree is as recorded by the operator. No run file records it. The
    third extension ran no Git command, so the repository HEAD was not read
    again.
45. **Elapsed time against journal time, runs 11 to 13.** The journal time of
    the delivered record is 265,439 ms, 284,846 ms and 253,019 ms, earlier than
    `elapsed_ms` (266,019, 285,149 and 253,456), as in runs 07 and 09.
46. **Differences between runs 11, 12 and 13 that the description does not
    mention.** The review assignment has two file-access records in run 11 and
    none in runs 12 and 13; the reviewer's reported input is 8,001 tokens in run
    11 and 3,433 and 3,453 tokens in runs 12 and 13. The review has no findings
    in runs 11 and 13 and two `Advisory` findings in run 12. The review
    identifier is `review-result-next-108-0` in runs 11 and 12 and
    `review-next-108-0-approved` in run 13. Six calls reported cache-write
    tokens: verification, review and final review in run 11, intake and final
    review in run 12, final review in run 13.
47. **Records after the phase change in runs 11 and 13.** Not part of the
    description. Three progress records follow the `Blocked(decoding)` phase
    record: `Monitor`, `Diagnosis` with outcome `Unknown`, and `Escalation`
    with step `AddVerifier` and the limitation "The selected method does not
    authorize this escalation step". None of them describes the decoding
    failure.
48. **Run 14 parameters.** Described as one change against run 13, in the
    adapter. Confirmed as far as run files go that nothing recorded changed:
    `experiment.json` equals that of run 13 apart from `started_at_ms` and the
    deadline derived from it, `discovery.json` equals it apart from
    `observed_at`, the driver digest (`7648e234…7687`) and the backend policy
    digest are the same, and the five recorded response contracts equal those
    of the first five calls of run 13. The recorded prompts of the five calls
    differ from those of run 13 in versions, in the cost value of the planning
    allowance, in the identifier, plan and title of the work item, which come
    from the planning output, and in the version and source summary of the
    result.
49. **Run 14 figures.** Confirmed: five calls, every call `Completed` with a
    Complete receipt, 70,835.5 units spent, zero held, `elapsed_ms` 197,683,
    final revision 271 in `summary.json`, in the projection and as the last
    journal sequence number, `sorted.json` equal to the expected bytes,
    session phase `Blocked(decoding)`, delivered outcome
    `Blocked(final_acceptance_unavailable)`. The described usage of the five
    calls (input / cache write / output / reasoning) equals the recorded
    usage: intake 5,032 / 0 / 2,621 / 2,526; planning 6,282 / 0 / 713 / 452;
    production 9,852 / 0 / 798 / 542; verification 7,364 / 4,257 / 1,382 /
    1,107; review 8,174 / 4,701 / 2,459 / 1,873. The description has no
    disagreement with the run files.
50. **Run 14 change.** Not recorded in run files, as the description itself
    says. No run file records the adapter's system prompt. The strings
    `starts with`, `system prompt`, `system_prompt`, `final message` and
    `analysis` occur in no journal record, in no value of `view.json`,
    `summary.json`, `experiment.json` or `discovery.json` and not in
    `run14.log`. The current working tree contains, in
    `crates/ymp-runtime/src/backends/claude.rs`, a guidance constant that ends
    "When the requested response is JSON, the first character of the final
    message is { and the last is }; keep analysis out of the final message."
    The modification time of that file (16:26:58 local time) is earlier than
    the recorded start of run 14 (16:27:20). Nothing ties that source state to
    the run, and when the executed binary was built is not recorded.
51. **Run 14 review output.** Described as several lines of prose followed by
    a `CandidateVerdict` JSON object inside a Markdown code fence. The output
    has 1,052 characters: three blocks of prose (218 characters with their
    separators), then one fence labelled `json` that encloses a JSON object
    written on several lines. The output ends with the closing fence and
    contains two fence markers. The fenced text parses as JSON with the
    Python parser: `id` `review-verdict-result-next-108-0`, verdict
    `Approve`, no findings, a `result` reference equal to the retained result
    (`result-next-108-0`, version `4f9c8b6d…eaf1`), `criteria` equal to the
    two criterion references of the run, and `basis` equal to the two
    evidence identifiers of the run. This is a reading of the recorded text,
    not the kernel's decoding result.
52. **Run 14, what happened to the review output.** The operator's note, which
    is the `note` value of `run-14.json`, says that the adapter removes a
    fence only when it encloses the whole response, that the text therefore
    reached the kernel unchanged, and that it was refused by the decoder. The
    run files do not show this. They record the output text, the review
    invocation end with terminal `Completed`, the discharge of the reviewer
    commitment and then the phase record `Blocked(decoding)` (journal record
    263), which is the only journal record of the run that contains the word.
    No record names a decoder, a refusal or the text that was examined. The
    current working tree contains a function in `claude.rs` that removes a
    fence only when the whole trimmed text is one fence; nothing ties that
    source state to the run. The files do not contradict the note, so its
    text was left as the operator wrote it.
53. **Run 14 producer commitment.** Not part of the description. The
    commitment `next-108-0` was proposed at 68,628 ms (journal record 115)
    and activated at 69,486 ms (record 119) with a lease of 240,000 ms, whose
    recorded expiry is 309,486 ms after the start. The journal holds no end
    record for it, and its state in the projection is `Active`. The last
    journal record, the delivered record (271), was written at 197,349 ms,
    which is 127,863 ms after the activation and 112,137 ms before the
    recorded expiry. The four other commitments were recorded `Discharged`
    with an event reference as basis: intake at 37,165 ms, planning at 60,501
    ms, verification at 131,852 ms and review at 188,117 ms, which is 109,
    165, 343 and 448 ms after the invocation end record. The files of runs 11
    to 13 record the producer commitment discharge directly after the
    acceptance record (item 34); run 14 has no acceptance record.
54. **Run 14 review and acceptance records.** Described as: no review,
    acceptance or final-review record exists. Confirmed: `reviews` and
    `acceptances` of the projection are empty, the ledger has no entry, no
    call has the operation `final_review`, the journal has no `Finalizing`
    phase record, and the delivered record has empty `accepted_sources` and
    `null` for `aggregate` and `acceptance`. Four check runs and two evidence
    records exist; they were recorded before the review call.
55. **Run 14 unresolved count.** Not part of the description.
    `report.unresolved` is 1 in run 14. It is 0 in runs 04, 05, 10, 11 and
    13, which ended with the same session phase, and 1 in runs 02, 03 and 08.
    What the reference stands for was not examined.
56. **Elapsed time against journal time, run 14.** The journal time of the
    delivered record is 197,349 ms, earlier than `elapsed_ms` (197,683), as in
    runs 07, 09 and 11 to 13.
57. **Records after the phase change in run 14.** Three progress records
    follow the `Blocked(decoding)` phase record, as in runs 11 and 13:
    `Monitor`, `Diagnosis` with outcome `Unknown`, and `Escalation` with step
    `AddVerifier` and the same limitation text. None of them describes the
    decoding failure.
58. **Source checkpoint of run 14.** The description of run 14 names one
    change and no checkpoint. No run file records one. `derive.py` writes the
    same `runtime_source_commit` value into every run file; for the fourth
    extension the repository HEAD was read once with `git rev-parse HEAD` and
    equals it. The working-tree state of the run is not recorded.
59. **Goal sentence about fences.** Not part of any description. The goal
    text recorded in `experiment.json` is the same in all fourteen runs and
    ends "Follow each role's structured response schema without Markdown
    fences." The intake output of run 04 and the review output of run 14
    contain a Markdown fence.

## Correction to the first version of these notes

The first version stated that the journal records heartbeat progress only and
that file effects are evidenced by snapshots alone. That was wrong. The journal
holds `LockChanged` records of kind `FileAccessPrepared` with assignment, mode
and path:

| Run | Records | Assignments |
| --- | ---: | --- |
| 01 | 0 | |
| 02 | 0 | |
| 03 | 3 | planning read; production read and write |
| 04 | 1 | intake read |
| 05 | 6 | intake read; planning read; production read and write; verification two reads |
| 06 | 2 | verification two reads |
| 07 | 8 | intake read; planning read; production read and write; verification two reads; review two reads |
| 08 | 0 | |
| 09 | 6 | intake read; planning read; production read and write; verification two reads |
| 10 | 1 | intake read |
| 11 | 8 | intake read; planning read; production read and write; verification two reads; review two reads |
| 12 | 6 | intake read; planning read; production read and write; verification two reads |
| 13 | 6 | intake read; planning read; production read and write; verification two reads |
| 14 | 8 | intake read; planning read; production read and write; verification two reads; review two reads |

A `FileAccessPrepared` record shows that the host prepared the access. Whether the
operation then completed is evidenced by snapshots, result artifacts and
workspace bytes. The compact files for runs 03, 04 and 05 do not carry these
records. No run has such a record for a final-review assignment.

## Items confirmed against the files

- Model `claude-haiku-4-5-20251001`, empty effort list in discovery, requested,
  sent and reported settings equal, effort `null`, for all 50 invocations that
  have an invocation record. The run 08 call has none; its dispatch settings
  are the same model and effort `null`.
- Call counts 0 / 2 / 3 / 1 / 5 / 5 / 5 / 1 / 5 / 1 / 6 / 6 / 6 / 5, 51 in
  all.
- Spent units 0.0 / 25,663.25 / 32,717.0 / 8,700.0 / 69,253.0 / 59,361.0 /
  59,098.0 / 0.0 / 51,212.0 / 14,028.0 / 79,245.75 / 78,274.0 / 70,628.75 /
  70,835.5, 619,016.25 in all. Held units are 28,000.0 in run 08 and 0.0 in
  the other thirteen runs. Both sums were computed from `budget` of the
  fourteen run files.
- Session phases as listed in the summary table of `README.md`, read from
  `session_phase` of the fourteen run files. `report.acceptance` is `null` and
  `report.report.grade` is `Unconfirmed` in all fourteen.
- Run 03 forecast values and run 04 `offer_window_ms 5000`.
- Run 03, run 05 and run 07 `sorted.json` equal the expected bytes; `input.json`
  unchanged in all runs.
- Run 04 intake output wrapped in one Markdown fence.
- Run 05: five calls Completed, Complete coverage, reviewer output in key=value
  text, phase `decoding`.
- Run 06: five calls, 59,361.0 units spent, zero held, `elapsed_ms` 186,233,
  reviewer verdict `Reject`, acceptance decision
  `Rejected("failing applicable check")`, grade `Refuted`, `sorted.json` not
  matching.
- Run 07: five calls, 59,098.0 units spent, zero held, `elapsed_ms` 221,560,
  final revision 275, `sorted.json` equal to `[1,2,2,3]` and a newline,
  acceptance `accepted-result-next-108-0` with decision `Accepted` and grade
  `Confirmed(TrustedCheck)`, delivered outcome
  `Blocked(final_acceptance_unavailable)`, phase `Blocked(commitment_expired)`.
- Run 07 per-call usage (input / output / reasoning): intake 4,978 / 1,660 /
  1,540; planning 6,286 / 837 / 567; production 9,672 / 743 / 528; verification
  7,009 / 1,062 / 856; review 7,489 / 1,614 / 1,190.
- Run 07 times: commitment `next-108-0` activated at 86,874 ms with a 60,000 ms
  lease, recorded `Expired` at 147,195 ms, acceptance recorded at 212,726 ms,
  followed by the phase change and deterministic reporting.
- Run 08: one call, diagnostic `claude_environment`, invocation terminal
  `Failed(Protocol)`, no usage observation and no receipt, 0.0 units spent,
  28,000.0 units held, `elapsed_ms` 9,647, final revision 36.
- Run 09: five calls, 51,212.0 units spent, zero held, `elapsed_ms` 173,615,
  final revision 268, `sorted.json` equal to `[1,2,2,3]` and a newline,
  acceptance `accepted-result-next-108-0` with decision `Accepted` and grade
  `Confirmed(TrustedCheck)`, delivered outcome
  `Blocked(final_acceptance_unavailable)`, phase `Blocked(commitment_expired)`.
- Run 09 per-call usage (input / output / reasoning): intake 4,921 / 669 / 574;
  planning 6,108 / 804 / 474; production 9,646 / 673 / 419; verification
  6,792 / 972 / 620; review 3,421 / 1,963 / 1,465.
- Run 09 times: commitment `next-108-0` activated at 51,397 ms with a 60,000 ms
  lease, recorded `Expired` at 111,683 ms, acceptance recorded at 164,783 ms,
  followed by the phase change and deterministic reporting.
- Run 10: one call, 14,028.0 units spent, zero held, `elapsed_ms` 35,568, final
  revision 59, intake usage 5,680 / 2,087 / 1,842, resource policy timeout
  240000, intake commitment lease 240,000 ms, `Discharged` 28,374 ms after
  activation.
- Every recorded charge of runs 09 and 10 equals input plus four times output.
- Cache read and cache write are zero in runs 03 to 07, 09 and 10. Run 02
  planning reported a cache write of 4,477.
- Runs 11 to 13: cache read is zero in all eighteen calls. Cache write is
  4,484, 4,549 and 4,270 for the verification, review and final-review calls
  of run 11, 4,239 and 4,409 for the intake and final-review calls of run 12,
  and 4,491 for the final-review call of run 13; it is zero in the other twelve
  calls. Every recorded charge equals input plus 0.25 times cache write plus
  four times output. Every receipt usage equals the usage of its call.
- Runs 11 to 13: acceptance `accepted-result-next-108-0` with decision
  `Accepted` and grade `Confirmed(TrustedCheck)` for both criteria and overall;
  both ledger entries `Satisfied`; eight check runs and four evidence records
  per run, half of them recorded in the `Finalizing` phase.
- Runs 11 to 13: every one of the eighteen commitments has a 240,000 ms lease
  and outcome `Discharged`. Final revisions 326, 316 and 319.
- Runs 11 to 13 per-call usage and durations as listed in the `README.md`
  tables; totals by purpose as listed there.
- Runs 11 to 13: final reviewer selection with policy `AnyNonProducer` and
  outcome `claude-b`; the final-review assignment has role `FinalReviewer` and
  access `ReadFiles`.
- When runs 11 to 13 were added, the sha256 of the example source in the
  working tree equalled the driver digest of runs 10 to 13 (`7648e234…7687`).
  Run 14 carries the same driver digest, and the sha256 was the same when run
  14 was added.
- Run 14: cache read is zero in all five calls; cache write is 4,257 and
  4,701 for the verification and review calls and zero in the other three.
  The charges 15,516.0, 9,134.0, 13,044.0, 13,956.25 and 19,185.25 equal input
  plus 0.25 times cache write plus four times output. Every receipt usage
  equals the usage of its call. Durations as listed in the `README.md` table.
- Run 14: totals by purpose Production 13,044.0, Verification 33,141.5,
  Coordination 24,650.0, Reporting 0.0; every activated commitment has a
  240,000 ms lease; four check runs and two evidence records of class
  `Executed`; offer delays 1,113 ms (verification), 1,381 ms (review) and 833
  and 1,627 ms (production).
- Run 14 times as listed in the sequence table of `README.md`.
- Weights input 1, cache read 0.1, cache write 1.25, output 4 and the pricebook
  name stating that units are not currency.
- Repository HEAD is `197aaa7bd8d1b8d85c996adec3bd29f1cefd0b0b`.
- When runs 06 and 07 were added, the sha256 of the example source in the
  working tree equalled the driver digest of runs 04 to 07 (`2becdbb3…e58c`).
  Runs 08 and 09 carry the same driver digest. When runs 08 to 10 were added,
  the sha256 of the example source equalled the driver digest of run 10
  (`7648e234…7687`), and that source holds a resource policy timeout of 240000.
  Runs 01 and 02 share `8bd61d37…fe78`; run 03 has `bbcf527a…839f`. The driver
  digest covers the example source only, not the adapter.

## Not verified

- Per-run working-tree state of the uncommitted W1-0020 changes.
- Build profile of any run.
- The text actually sent to the native process beyond the recorded dispatch
  prompt.
- Which decoding step refused the run 05 review output. The journal records the
  phase change after the review invocation ended, with no separate decoding
  diagnostic.
- The cause of the run 06 refusals and where they were issued.
- Why the producer commitment is not discharged after production ends, and why
  its expiry became the blocking phase only in run 07.
- Provider billing. Complete coverage is provider-reported.
- Wall time of runs 01 and 06 to 14 as measured by the shell.
- The adapter source state of runs 08 and 09, and each of the changes described
  for them.
- Which property of the native environment was refused in run 08, and whether
  the native process was started.
- Whether provider tokens were consumed in run 08.
- Which decoding step refused the run 10 intake output.
- The behaviour of a producer commitment under the 240,000 ms timeout was not
  verified when this list was written for run 10. Runs 11 to 13 record it
  (item 34).
- Which decoding step refused the final-review outputs of runs 11 and 13, and
  whether the run 13 output would have been accepted without its prose.
- Which component refused the run 12 basis and stopped the session. The journal
  records the stop without a reason.
- The adapter source state of runs 11 to 13, and each of the adapter revisions
  described for run 13.
- Why the run 11 reviewer has file-access records and the reviewers of runs 12
  and 13 have none.
- Whether the three final-review verdicts would have led to final acceptance.
  No run records a review or an acceptance of the aggregate.
- The adapter source state of run 14, the system prompt sent in it and the
  change described for it.
- Whether the adapter changed the run 14 review output, which decoding step
  refused it, and whether the fenced JSON would have been recorded as a review
  without the prose and the fence.
- Why the producer commitment of run 14 has no end record.

## Derivation

`derive.py` in this directory produces a compact run file from a run directory, a
copy of its journal, the log path and a text file holding the `note`:

```sh
python3 derive.py [--acceptance-path] RUN_DIR JOURNAL_COPY LOG_PATH NOTE_FILE OUTPUT
```

Use `-` as `LOG_PATH` when no log exists. Paths are written into the output as
given, so absolute paths are needed to reproduce the existing files.

Reproduction result: without the flag, and with each file's own `note` text, the
script regenerates `run-01.json` to `run-05.json` byte for byte (compared with
`cmp`). Those five files were left as they were. The `note` text is the only
hand-written input; every other value is copied or computed from run files.

`run-06.json` and `run-07.json` were generated with `--acceptance-path`. They
differ from the structure of `run-05.json` in these points only:

- One added top-level section, `acceptance_path`, placed after `workspace`. It
  holds `reviews`, `acceptances`, `ledger`, `commitments`, `session_changes` and
  `file_access`. It was added because runs 06 and 07 are the first with review and
  acceptance records, and the earlier structure kept only their counts.
- `log.shell_wall_time` is `null`, because the logs have no shell timing line.
- `report.accepted_sources` is not empty in run 07.

Regenerating run 05 with the flag gives the same file plus that section; this
was checked in a scratch copy and not written to this directory.

`run-08.json`, `run-09.json` and `run-10.json` were generated with
`--acceptance-path`. `derive.py` was changed for run 08, on which it stopped
with an error: the projection of the failed call has `cost: null` and
`invocation: null`, and the script indexed both. The change is limited to two
guards in the function `invocation` and one paragraph added to the docstring: a
`null` cost is written as `null`, and `derived.duration_ms` is `null` when the
invocation record is `null`. No key was added, removed or renamed. The earlier
version is kept as `scratch/work/derive.before-run08.py`.

After the change the script was run again for runs 01 to 07, with each file's
own `note` text, without the flag for runs 01 to 05 and with it for runs 06 and
07, writing to `scratch/work/run-NN.repro.json`. All seven outputs are identical
to the files in this directory byte for byte (compared with `cmp`). The seven
existing files were not rewritten.

`run-08.json` differs from the other files with the section in these points:
the call's `invocation`, `receipt`, `cost` and `backend_terminal` are `null`,
`derived.duration_ms` is `null`, the account's `receipt` and `settlement` are
`null`, and `unbounded` and `unsettled_usage` are `true`.

`run-11.json`, `run-12.json` and `run-13.json` were generated with
`--acceptance-path`. The script as it stood ran on these runs without error. It
already kept the final-review call with its full output and response contract,
the `failure` text of `summary.json`, the session phase, the delivered outcome,
the session change `Stop`, and the final check runs and final evidence. It did
not keep the other fields of the final-review purpose, the finalization records
of the journal, the finalization values of the projection, or the order of the
closing journal records. `derive.py` was therefore extended:

- One key, `final_review`, is added at the end of `acceptance_path`, and only
  when a call of the run has `operation: "final_review"` in the last frame of
  its recorded prompt. No call of runs 02 to 10 has it.
- The key holds `invocation`, `purpose` (keys, operation, `subject`,
  `evidence`, response contract, a compact form of the aggregate, the listed
  check runs), `projection` (finalization and session-state values, final
  evidence identifiers), `failure`, `commitment_ends`, `records` (journal
  records from the `Finalizing` phase record to the end, in journal order, with
  sequence number, time, kind, variant name and compact content) and `derived`.
- The function `acceptance_path` received `summary` as a third argument. Four
  functions were added: `first_key`, `finalization_detail`, `journal_record`
  and `final_review`. One paragraph was added to the docstring. No existing key
  was removed or renamed, and no existing function body was changed otherwise.

The earlier version is kept as `scratch/work/derive.before-run11.py`.

After the change the script was run again for runs 01 to 10, with each file's
own `note` text, without the flag for runs 01 to 05 and with it for runs 06 to
10, writing to `scratch/work/run-NN.repro3.json`. All ten outputs are identical
to the files in this directory byte for byte (compared with `cmp`). The ten
existing files were not rewritten. After the last edit of the third extension
the derivation was repeated for all thirteen runs into
`scratch/work/run-NN.repro4.json`, with the same result for every file.

`run-14.json` was generated with `--acceptance-path`, the mode of every run
from 06 on. The section is empty under `reviews`, `acceptances` and `ledger`
for run 14 and carries its commitment, session-change and file-access
records. `derive.py` was not changed for run 14. No call of the run has the
operation `final_review`, so the file has no `final_review` key and the
structure of `run-06.json` to `run-10.json`. After the last edit of the fourth
extension the derivation was repeated for all fourteen runs into
`scratch/work/run-NN.repro5.json`, each with the `note` text taken from the
existing file (`scratch/work/note-repro5-NN.txt`). All fourteen outputs are
identical to the
files in this directory byte for byte (compared with `cmp`), and `derive.py`
is identical to its copy made before the extension.

## Values that are null or derived

- `effort` is `null` throughout because none was requested and none was reported.
- `receipt.cost`, `complete_cost` and `upper_bound` are `null` in the source.
- `log.path` and `log.shell_wall_time` are `null` for run 01;
  `log.shell_wall_time` is `null` for runs 06 to 14.
- In `final_review.purpose`, `subject` and `evidence` are `null` where the
  recorded purpose has no such key (both in run 11, `evidence` in run 12).
- Values under `final_review.derived` are computed by `derive.py`:
  `whole_output_is_json` and `json_after_prose` by the Python JSON parser, the
  latter on the text from the first brace to the end; `prose_chars_before_json`
  is the position of that brace; `aggregate_equals_subject`,
  `basis_in_purpose_evidence` and `basis_in_final_evidence` are comparisons;
  `final_evidence_in_earlier_frames` counts occurrences in the prompt frames
  before the purpose; `failure_code_journal_records` lists the journal records
  whose payload contains the failure code, and is `null` when `summary.json`
  has no failure. They are `null` where no JSON object could be parsed or the
  compared value is absent. None of them is a kernel decision.
- In `final_review.records` the key `detail` is the variant name of the record
  and is left out where the record has none.
- `final_review.projection.final_evidence` lists the evidence identifiers whose
  `result` is `null` in the projection.
- For the run 08 call `invocation`, `receipt`, `cost`, `backend_terminal` and
  `derived.duration_ms` are `null`. The first four are `null` in the source.
- `port` is `null` where the recorded prompt names no port (production,
  verification, review).
- Fields under `derived` are computed from run-file values, not copied:
  `duration_ms` = `ended - started`; `window_ms` = solicitation `deadline` minus
  the journal time of its `SolicitationOpened` event; `offer_delay_ms` = offer
  `at` minus that same time; workspace `bytes` and `sha256` from the retained
  files; commitment `lease_ms` = lease `expires` minus the journal time of the
  activation record; `ended_after_activation_ms` = journal time of the end record
  minus that of the activation record. `opened_at` is the journal time of the
  opening event.
- Journal record times are later than the values stored inside the records. For
  the run 07 production call the stored `started` and `ended` values give a
  duration of 21,525 ms, while the start and end records are 20,455 ms apart.
- `report.accounting.receipts` and `report.unresolved` are counts of the source
  reference lists, not the references themselves.
- Held units and coverage values are copied unchanged.

## Compaction relative to the GPT run files

Dropped for size: full prompt text (the `response` contract is kept as
`response_contract`), per-observation lists, event references, admission and
nonce values, the `last` fields, and the registry decision list. Added: `totals`,
`session_phase`, `coordination` timing, `workspace`, `journal`, `log`,
`evidence_class`, for runs 06 to 14 `acceptance_path`, and for runs 11 to 13
`final_review` inside it.

## Sensitive data

Source files contain data that was not copied:

- `view.json` and the journal: the absolute path of the native executable under a
  home directory (backend parameter `executable`). Replaced by
  `[redacted local path]`. `derive.py` replaces every string value that contains
  a home-directory prefix.
- `run02.log` to `run05.log`: the same kind of path in the shell timing line.
- The journal: `token_digest` values on grant events. Not copied.
- `workspace/.ymp-workspace-owner`: not opened beyond listing its keys.

`native_session` identifiers are kept, as in the GPT run files. They are session
identifiers, not credentials. Remove them if they are considered sensitive.

Final scan of every file in this directory (`run-*.json`, `README.md`,
`NOTES.md`, `derive.py`), repeated after run 14 was added; seventeen files:

| Pattern | Result |
| --- | --- |
| at sign | No match. |
| home-directory path prefix | No match. |
| API key field name | No match. |
| secret-key prefix (s, k, hyphen) | No match. |
| `token`, case-insensitive | Matches in the run files are `comparative_token_efficiency` and the pricebook version name in each of the fourteen files, and the word "tokens" in one usage sentence of `run-02.json`. Matches in `README.md` and in this file are usage wording ("tokens", "token weights"), the `token_digest` mention above and this table. No match in `derive.py`. |

No match in any file is account data, an address, a credential, a key or a user
name.

## Side effects to disclose

During the first version of these files, reading the WAL-mode journals read-only
updated the modification time of each `journal.sqlite-shm` sidecar of runs 01 to
05 (size unchanged at 32,768 bytes). `journal.sqlite` and `journal.sqlite-wal`
kept their original times and sizes.

For the extension to runs 06 and 07 no original journal was opened. The three
journal files of every run were copied with `cp -n` into
`/tmp/ymp-claude-pilot/scratch/runNN/` and only the copies were read. The
original journal files of runs 06 and 07 still carry their original modification
times (all three files 15:36:17 and 15:43:08 local time).

For the extension to runs 08, 09 and 10 no original journal was opened either.
The copies of runs 08 and 09 existed before this work. The three journal files
of run 10 were copied with `cp` after `run10.log` contained its `exit=` line.
Each `journal.sqlite` copy was compared with its original by `cmp` and is
identical. The original journal files of runs 08, 09 and 10 carry the
modification times 16:01:30, 16:06:18 and 16:08:40 local time, the same for all
three files of a run. Reading the copies updated the modification time of their
`journal.sqlite-shm` sidecars.

For the extension to runs 11, 12 and 13 no original journal was opened as a
database. The copies of all three runs existed before this work. The files
`journal.sqlite` and `journal.sqlite-wal` of each copy were compared with their
originals by `cmp`, which reads bytes only, and are identical. The original
journal files carry the modification times 16:13:42, 16:20:26 and 16:26:26
local time; the `journal.sqlite-shm` files of runs 11 and 12 are one second
later. The copies are dated 16:15:36, 16:22:09 and 16:26:58. Reading the copies
updated the modification time of their `journal.sqlite-shm` sidecars. The three
logs ended with the line `exit=0` when they were first read.

In the third extension `README.md`, `NOTES.md` and `derive.py` were changed in
place. Their state before it is kept in `scratch/work/` as
`README.before-run11.md`, `NOTES.before-run11.md` and
`derive.before-run11.py`. The files `run-11.json`, `run-12.json` and
`run-13.json` were created; `run-01.json` to `run-10.json` were not written.
The note texts are kept as `scratch/work/note-11.txt`, `note-12.txt` and
`note-13.txt`. A file `scratch/work/note-14.txt` existed during this work. It
was not created, read or changed by it.

In the repository, the third extension ran `shasum` on the example source,
`grep`, `sed -n`, `ls` and `stat` on source files under `crates/`. It ran no
Git command, no build and no native model invocation, and it wrote nothing
there.

For the extension to run 14 no original journal was opened as a database. The
copy existed before this work, and its `journal.sqlite-shm` sidecar was
already dated later (16:30:55) than the two other files of the copy
(16:30:44). The files `journal.sqlite` and `journal.sqlite-wal` of the copy
were compared with their originals by `cmp` and are identical. The three
original journal files and `run14.log` carry the modification time 16:30:38
local time, before and after this work. Reading the copy updated the
modification time of its `journal.sqlite-shm` sidecar. The log ended with the
line `exit=0` when it was first read.

In the fourth extension `README.md` and `NOTES.md` were changed in place and
`run-14.json` was created; `derive.py` and `run-01.json` to `run-13.json` were
not written. The state before it is kept in `scratch/work/` as
`README.before-run14.md`, `NOTES.before-run14.md` and
`derive.before-run14.py`. The note text `scratch/work/note-14.txt` was written
by the operator and was not changed; a copy is kept as
`note-14.operator-original.txt`.

In the repository, the fourth extension ran `git rev-parse HEAD` once and
`shasum` on the example source, and it read files under `crates/` and
`ymp-docs/` with `grep`, `sed -n`, `cat`, `ls`, `wc` and `stat`. It ran no
other Git command, no build and no native model invocation, and it wrote
nothing there.

`README.md`, `NOTES.md` and `derive.py` were changed in place. Their state
before this extension is kept in `scratch/work/` as `README.before-run08.md`,
`NOTES.before-run08.md` and `derive.before-run08.py`.

In the repository, the second extension ran `shasum` on the example source and
`grep` on source files under `crates/`. It ran no Git command and no build.

`/tmp/ymp-claude-pilot/scratch/journal.sqlite` existed before this work. It was
not created, read or changed by it.

`git status --porcelain` was run once in the repository during this work. It
changes no tracked file, but Git may refresh its index file when it runs.

## Commands used

Reference shape and rules (read only):

```sh
ls -la ymp-docs/experiments/homogeneous-gpt-elementary/
python3 -c "import json; d=json.load(open('run-05.json')); ..."   # key structure
git rev-parse HEAD
git log -1 --format='%H %s'
grep -n -A6 '^\[profile' Cargo.toml
shasum -a 256 crates/ymp-storage/examples/homogeneous_claude.rs
grep -rn --include='*.rs' "Return only CandidateVerdict JSON" crates
```

Run files:

```sh
cat runNN/summary.json runNN/experiment.json runNN/discovery.json
diff <(python3 -m json.tool run01/experiment.json) <(python3 -m json.tool runNN/experiment.json)
diff <(python3 -m json.tool run01/discovery.json) <(python3 -m json.tool runNN/discovery.json)
cat runNN.log
xxd runNN/workspace/input.json; xxd runNN/workspace/sorted.json
shasum -a 256 runNN/workspace/input.json runNN/workspace/sorted.json
```

Journal copies and derivation:

```sh
mkdir -p scratch/runNN
cp -n runNN/journal.sqlite runNN/journal.sqlite-wal runNN/journal.sqlite-shm scratch/runNN/
python3 evidence-draft/derive.py /tmp/ymp-claude-pilot/runNN \
  /tmp/ymp-claude-pilot/scratch/runNN/journal.sqlite \
  /tmp/ymp-claude-pilot/runNN.log NOTE_FILE OUTPUT
cmp OUTPUT evidence-draft/run-NN.json
stat -f '%N %z %Sm' runNN/journal.sqlite*
```

Journal reads used Python's `sqlite3` module on `file:<copy>?mode=ro` with this
query:

```sql
select e.seq, c.bytes from journal_events e
join content_values c on c.digest = e.payload order by e.seq;
select session, last_seq, chain from journal_heads;
select identity from store_identity;
```

Projection values came from `view.json` through Python `json`, at these paths:

- `execution.invocations.<id>`: `dispatch`, `invocation`, `output`, `usage`,
  `turns`, `receipt`, `cost[1]`, `terminal`, `confirmed_terminal`,
  `backend_terminal`, `ended_at`, `diagnostics`
- `treasury`: `budget`, `totals`, `accounts`, `unbounded`, `unsettled_usage`,
  `reporting_mode`
- `session_state.phase`, `finalization.delivered`
- `coordination.solicitations`, `coordination.awards`, `coordination.commitments`
- `results.results`, `check_runs`, `evidence`, `acceptances`, `reviews`,
  `ledger.entries`

Second extension (runs 08 to 10):

```sh
grep -n '^exit=' run10.log
mkdir -p scratch/run10 && cp run10/journal.sqlite* scratch/run10/
cmp runNN/journal.sqlite scratch/runNN/journal.sqlite
python3 evidence-draft/derive.py --acceptance-path /tmp/ymp-claude-pilot/runNN \
  /tmp/ymp-claude-pilot/scratch/runNN/journal.sqlite \
  /tmp/ymp-claude-pilot/runNN.log scratch/work/note-NN.txt evidence-draft/run-NN.json
diff <(python3 -m json.tool run09/experiment.json) <(python3 -m json.tool run10/experiment.json)
grep -c -i plugin runNN/view.json runNN.log runNN/experiment.json runNN/discovery.json
grep -rn -E 'plugins|builtin|claude_environment' crates --include='*.rs'
```

Journal record times in the sequence tables are the `at` value of each journal
record minus `started_at_ms` of `experiment.json`.

Third extension (runs 11 to 13):

```sh
cmp runNN/journal.sqlite scratch/runNN/journal.sqlite
cmp runNN/journal.sqlite-wal scratch/runNN/journal.sqlite-wal
stat -f '%N %z %Sm' runNN/journal.sqlite* scratch/runNN/journal.sqlite* runNN.log
grep -n '^exit=' runNN.log
nice -n 10 python3 evidence-draft/derive.py --acceptance-path /tmp/ymp-claude-pilot/runNN \
  /tmp/ymp-claude-pilot/scratch/runNN/journal.sqlite \
  /tmp/ymp-claude-pilot/runNN.log scratch/work/note-NN.txt evidence-draft/run-NN.json
nice -n 10 python3 evidence-draft/derive.py [--acceptance-path] /tmp/ymp-claude-pilot/runNN \
  /tmp/ymp-claude-pilot/scratch/runNN/journal.sqlite \
  /tmp/ymp-claude-pilot/runNN.log scratch/work/note-NN.txt scratch/work/run-NN.repro3.json
cmp scratch/work/run-NN.repro3.json evidence-draft/run-NN.json
grep -c -E 'claude_unsettled|CLAUDE_CONFIG_DIR|final_review_basis' runNN/view.json runNN.log runNN/summary.json
shasum -a 256 crates/ymp-storage/examples/homogeneous_claude.rs
grep -rn --include='*.rs' -E 'claude_unsettled|CLAUDE_CONFIG_DIR|final_review_basis' crates
grep -rn --include='*.rs' -E 'supplied Evidence ids|copied exactly from evidence' crates
stat -f '%N %Sm' crates/ymp-runtime/src/backends/*.rs crates/ymp-kernel/src/finalization/*.rs
```

`experiment.json` and `discovery.json` of runs 10 to 13 were compared through
Python `json` after flattening, not with `diff`. The final-review prompt was
read from `execution.invocations.call-final-review.dispatch.prompt.text` of
`view.json`, which holds a JSON list of frames; the purpose is its last frame.
Additional projection paths: `finalization` (`control`, `stopped`, `fence`,
`outcome`, `reviews`, `acceptance`, `pending_work`, `delivered`) and
`session_state` (`phase`, `stopped`, `control`).

Fourth extension (run 14):

```sh
grep -n '^exit=' run14.log
cmp run14/journal.sqlite scratch/run14/journal.sqlite
cmp run14/journal.sqlite-wal scratch/run14/journal.sqlite-wal
stat -f '%N %z %Sm' run14/journal.sqlite* scratch/run14/journal.sqlite* run14.log
nice -n 10 python3 evidence-draft/derive.py --acceptance-path /tmp/ymp-claude-pilot/run14 \
  /tmp/ymp-claude-pilot/scratch/run14/journal.sqlite \
  /tmp/ymp-claude-pilot/run14.log scratch/work/note-14.txt evidence-draft/run-14.json
nice -n 10 python3 evidence-draft/derive.py [--acceptance-path] /tmp/ymp-claude-pilot/runNN \
  /tmp/ymp-claude-pilot/scratch/runNN/journal.sqlite \
  /tmp/ymp-claude-pilot/runNN.log scratch/work/note-repro5-NN.txt \
  scratch/work/run-NN.repro5.json
cmp scratch/work/run-NN.repro5.json evidence-draft/run-NN.json
cmp evidence-draft/derive.py scratch/work/derive.before-run14.py
grep -c -E 'starts with|system prompt|system_prompt|final message|analysis' \
  run14/view.json run14.log run14/summary.json run14/experiment.json run14/discovery.json
git rev-parse HEAD
shasum -a 256 crates/ymp-storage/examples/homogeneous_claude.rs
grep -n -A3 'const GUIDANCE' crates/ymp-runtime/src/backends/claude.rs
grep -n -B6 -A12 'fn unfenced' crates/ymp-runtime/src/backends/claude.rs
stat -f '%N %Sm' crates/ymp-runtime/src/backends/claude.rs crates/ymp-runtime/src/backends/claude/*.rs
```

`experiment.json`, `discovery.json` and the recorded prompts of runs 13 and 14
were compared through Python `json` after flattening. The review output was
read from `invocations.call-review-result-next-108-0.output` of `run-14.json`
and from the same call of `view.json`. Commitment states of all fourteen runs
were read from `coordination.commitments` of `view.json`. The sums of the
`README.md` summary were computed from the fourteen run files.

Final scan:

```sh
grep -c -- "<pattern>" run-*.json README.md NOTES.md derive.py
grep -o -i -h -E -- "[A-Za-z_-]*<pattern>[A-Za-z_-]*" run-*.json README.md NOTES.md derive.py | sort | uniq -c
```

## Fifth extension (run 15)

Run 15 was made through the interactive interface of the `ymp` executable. The
example, `derive.py` and the fourteen run files were not used or changed for it.
The run directory was `/tmp/ymp-claude-pilot/run15` (`work`, `store`), new for
this run, and the log `/tmp/ymp-claude-pilot/run15.log`.

How the evidence was produced:

- A script started the executable inside a detached `tmux` session of 130 by 44
  cells, typed the goal, `/expect`, `/preserve` and `/start`, and appended the
  visible frame to the log with `tmux capture-pane -p -J`. While the session
  worked it opened the resources page every 30 seconds and recorded how long the
  request took. After the report it saved the criteria, resources, activity and
  results pages, typed `/quit`, and appended the output of `stty -a`,
  `ymp sessions`, `ymp report` and `od -c` of the two workspace files.
- After the run the `store` directory was copied to
  `/tmp/ymp-claude-pilot/scratch/run15/store`. The copy was opened by the same
  executable with `--scripted` and an empty workspace directory, the session was
  opened with `/open`, and the description of each call, the commitments page
  and the acceptance page were saved in the same way. The original store was
  opened only by the run itself and by the `sessions` and `report` commands of
  the script.
- `run-15-terminal.txt` and `run-15-calls.txt` are those two logs with trailing
  blanks removed, the run directory replaced by `RUN15` and the copy by
  `RUN15-COPY`. The script replaced the home directory by a marker while it
  wrote the log; the marker does not occur in either file.

Confirmed against the two records:

- Five calls, each `Completed` with Complete coverage and cache read zero;
  requested, sent and reported model `claude-haiku-4-5-20251001` and no effort
  in each description.
- Usage as in the run 15 table of `README.md`. The units per call are computed
  there, not read; their sum equals the displayed spent value 65,406.25.
- Five accounts `Settled`, held 0, no unknown usage; limit 250,000 and reserves
  60,000 and 30,000.
- Final revision 272 in the interface, in `ymp sessions`, in `ymp report` and in
  the later read of the copy.
- `sorted.json` and `input.json` have 10 bytes each, with the expected and the
  original content.
- Each call description carries the `lease_renewal` diagnostic, as in runs 02
  to 14.

Not verified for run 15:

- Anything that needs the journal as a database: record order, times of
  records, file-access records, offers, digests, prompts and response contracts.
- Call durations and the elapsed time of the session. The frames carry the wall
  time of the capture and the start time of each call only.
- The native client version during the run; `2.1.284` was read afterwards.
- Whether the policy values of the executable's native composition equal those
  of `experiment.json` of run 14 in every field.
- Interrupt, recovery and a question to the user on a native session.

Sensitive data: both records were scanned for an at sign, a home-directory path
prefix, a temporary-directory path prefix, an API key field name and the
secret-key prefix, with no match. `token`, `account` and `email` match only the
column heading `ACCOUNT` of the resources page and the word `accounting` of the
report. The description of a call shows no executable path and no native session
identifier.

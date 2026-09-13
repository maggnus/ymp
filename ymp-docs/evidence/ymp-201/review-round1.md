# YMP-201 connected-consumer review, round 1

Candidate: 8a2acaf, executable consumer 0ac1650.
Static reviewer 332e0b99 returned two required findings. The executable reviewer 52111c72 has finished its bounded scenarios and returned
three P1 findings. Both reviews are complete; the same author may now correct
the candidate in its existing isolated branch. No native model run or quota is authorized.

## Required correction: task outcome versus measurement validity

consumer.rs currently requires a completed Engine and no protocol deviations for
cooperation to be interpretable, whereas raw conditions depend on accounting.
An ordinary final-review rejection, a known budget limit or an underused roster
can stop the remaining phase. Calibration additionally requires every objective
answer to be correct before the measured pilot can run. This is asymmetric and
conditions measurement readiness on the result being measured.

Introduce explicit measurement validity and task outcome. A fully accounted,
properly configured attempt may yield an incorrect answer, an absent artifact,
a normal review rejection or a known limit/deadline failure. Retain these negative
outcomes and run the remaining approved conditions while sufficient resources and
verified termination permit. Do not silently repair outputs, skip failures or
replace missing artifacts with successful placeholders. Calibration continuation
must depend on trustworthy controls/accounting, not universal answer correctness.

Retain requested and actual participant counts separately. Underuse is an
allocation outcome/deviation and must not be claimed as N active participants;
it is not an unexplained extra model or automatically a reason to abort the whole
phase. Do not add purposeless model calls merely to satisfy a roster count.
Unknown spend, unexpected participants, contradictory controls, leaked private
answers and broken measurement remain distinct stop conditions. A cancellation
without complete accounting still obeys unknown_usage=stop.

## Required correction: effective time allowance

Raw solo/independent currently get one outer invocation capped at 120/180 seconds,
while cooperation can spend 480/900 seconds through multiple calls. The parent
chooses to equalize the accessible whole-condition deadline. Do not preserve
this asymmetry under a nominal equal-resource label.

A solo native tool loop should have its complete remaining condition allowance;
it need not be forced to make redundant calls. Independent participants still
share a single absolute deadline and total budget, with an explicit scheduling
rule. A later candidate does not receive a fresh group deadline. Check other
per-call limits and reservations so a declared common allowance is usable, and
retain selection overhead inside the same condition deadline. Production Engine
acceptance and access rules must remain unchanged.

Use identical measurement rules for native-setting acknowledgment in every arm.
Record requested, sent and reported controls separately. A missing acknowledgment
must remain unknown, and a contradictory acknowledgment must not pass. Do not
silently allow missing effort in solo while rejecting the same evidence in teams.

## Accepted static areas and remaining audit points

The reviewer accepted actual participant counting, no hidden strong judge,
public-only selection, blind external scoring, native subagent/memory controls,
normal authentication custody, private/peer path restrictions, exact manifest
approval and the stated observed-token limit rather than a false hard billing cap.

Refresh native-control evidence on the final wrapper and bind its executable and
wrapper hashes. Retain the Python executable identity and a clear one-run scope
for future owner authorization. Do not create a broad security framework or read
and copy credential files to satisfy these checks. The noted output-directory
replay and inherited-config limits must be resolved or explicitly constrained
before presenting the concrete spending request. These points do not grant
permission to run native calibration during rework.

The complete write assignment is now the three corrections below together with
the time/outcome requirements above. Original external controls are retained
unchanged. All proposed quotas remain unapproved.

## Completed executable review and C3

The [full external record](independent-round1/review-result.json) binds the source,
commands and hashes. Four existing Rust tests (including all 12 protocol cells)
and 16 envelope tests passed. Nine independent protocol reproductions establish
that valid negative task outcomes and usable time still violate the intended
comparison. The provider-error control retained numbers but had partial usage;
it is not evidence of a fully accounted ordinary failure. All fixture processes
ended and the reviewer removed its own pycache. No native inference occurred.

C3 is an additional required correction: public_select.py launches public_test.py,
which imports candidate code, outside the restricted native environment. A
synthetic candidate read an artificial private marker outside its directory and
exposed it in selector stderr. See the retained selection-canary report. The
public selection rule itself does not use an oracle; its execution boundary is
the defect.

Apply a shared restricted execution boundary to every import/execution of candidate
code, including public checks and the final repair_probe subprocess. The trusted
observer may read private expected answers, but the candidate subprocess must
receive only function names/arguments or public checks, never the expected answers,
private fixture files, other candidates, native state or credentials. The current
repair_checks code already omits expected values from its stdin; preserve that.
Do not disable the useful public checker or replace it with hidden-oracle selection.
Use existing native/OS facilities and no new product dependency or authority API.

Prove the artificial canary read is refused in selection and scoring while a
legitimate solution still passes. Preserve the original successful canary artifact
as the before-control. Record the final executable/wrapper/interpreter binding;
rerun the inference-free native control probe against the final wrapper when its
previous evidence predates the relevant implementation. No real confidential
file should appear in a test or log.

## Unified metadata and spending scope

The study identifies configured conditions. Requested and sent model/effort must
match the frozen condition, using the verified native envelope. Any contradictory
native acknowledgment invalidates that condition's configuration. Missing reported
fields remain null with explicit incomplete metadata in every arm; they must not
be filled from requested values or uniquely disqualify cooperation. This does not
claim that an absent native acknowledgment has been confirmed. Existing production
provider checks must remain intact.

A native approval covers the exact bounded phase once. Preserve a durable usage/
execution marker outside disposable model outputs, or explicitly constrain the
trusted executor so output cleanup cannot cause an automatic fresh paid run under
the same approval. Do not build a general permission service. Newly frozen manifests
must state this scope and the final time/allocation rules before approval.

## Rework sequence and acceptance

Use the same eval/ymp201-weak-pilot fork from 8a2acaf. Keep original private task
requirements/variants and reviewed controls, fix only the evaluation consumer,
selectors/probes and their evidence. Accepted product crates and existing runtime
admission remain byte-identical. New test cases must cover actual behavior, not
just result-label changes: later matched conditions run after a fully accounted
negative outcome; unknown spend still stops; solo receives the available common
deadline; metadata follows one rule; candidate code cannot read the private canary.
Refresh manifests and execute the final required checks after changes. Deliver
one reviewable range with exact evidence and no native calibration, model trial,
quota consumption, installation or task-register changes.

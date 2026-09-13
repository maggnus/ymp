# YMP-201 connected-consumer review, round 1

Candidate: 8a2acaf, executable consumer 0ac1650.
Static reviewer 332e0b99 returned two required findings. The executable reviewer
52111c72 is still completing its bounded scenarios; keep the candidate unchanged
until those finish. No native model run or quota is authorized.

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

The author has not yet been sent a write assignment for these findings; the
parent will combine them with the ongoing executable result to avoid changing
source underneath that review. All proposed quotas remain unapproved.

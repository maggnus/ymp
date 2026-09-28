# Criterion assessment and independent acceptance

Canonical status remains in `tasks/records/W1-0010.json`. AcceptanceAuthority now
records A8 criterion assessments and A7 decisions for actual immutable candidates.
An accepted candidate can discharge exactly its producer's artifact commitment.

## Recorded assessment

The ledger consumer obtains canonical evidence and reviews using the shared
ApplicabilityContext before considering polarity, correlation or numerical belief.
Each input identifies the current criterion and result versions, previous entry,
assessment rules and journal digest. Supporting records with the same
`(independence, class, discrimination)` count once. The retained Decision includes
the implementation/version, effective parameters, input digest, proposal, prior
basis and validated outcome. SessionControl is required to commit or change policy.

LikelihoodRatioTable starts at 0.5 and applies the model's initial ratios
50/30/20/5/1.5/1.5/1.1 using log odds. Default behavior, quality and support
thresholds are 0.9, 0.6 and 0.6. These remain initial, uncalibrated assumptions.
StrongestSupport uses the maximum of its explicit prior and individual support
strengths rather than multiplying groups. Both run through the same consumer.

A non-neutral experimental prior must lie strictly between zero and one, name the
current criterion/result and cite applicable recorded Evidence or Review with a
matching polarity. Its rationale describes a present-result assumption with
unproven calibration. A Contribution or Offer forecast cannot supply that basis.
StrongestSupport's returned prior must match its recorded parameters; using max
avoids counting the same review twice as independent probabilistic support.

The kernel independently enforces executable/browser/external contradiction as
Contradicted with belief zero. Satisfied requires applicable Supports, positive
needs_class coverage and the selected threshold. High belief alone is insufficient.
Ledger entries retain criterion and result versions; abandoning a candidate or
starting another attempt invalidates its current assessment without deleting
history. Old candidates cannot overwrite the current ledger. The accepted pointer
or recorded attempt order identifies the current subject, never ID/time ordering.

## Acceptance, grades and credit

A7 requires an applicable approving Reviewer distinct from the actual producer.
Applicable Executed/Browser/ExternalData contradiction defeats approval. Satisfied
is not an additional acceptance condition: Accepted with Unconfirmed is possible,
and unmet criteria remain visible in the ledger.

Grades are derived from applicable evidence per criterion; the overall grade is
the minimum over required criteria. An empty required set conservatively yields
Unconfirmed. AssessmentRules records the mutation threshold explicitly. Actual
sources currently support Executed and Inspection; owner-registered passing
checks can establish Confirmed(TrustedCheck). Hidden discrimination, external
sources and delayed consequences remain with their owning tasks. A review's own
claim cannot manufacture these grades.

ConfirmedOnly and IncludeDiscriminated are selectable CreditPolicy implementations.
AcceptanceRecorded retains the exact policy decision and eligibility. Switching
selection does not rewrite earlier decisions or grades. Eligibility alone creates
no competence observation; W5-0001 owns those observations.

Accepted records the exact Attempt and immutable WorkItem.accepted pointer.
Rejected records the reason and sets WorkItem to Failed while retaining the
candidate. Explicit abandonment can reopen rejected work; rejection does not
automatically retry or discharge its producer.

The Arbiter resolves the exact committed Acceptance reference, verifies Accepted
and its current assessment contract, then follows ResultVersion, Attempt,
Assignment and Contribution to the original producer and production contract.
Its completion subject is the original WorkItem reference. Forged, stale, rejected
or unrelated acceptance cannot discharge that commitment. A later assessment
contract does not substitute for the original production contract.

Assessment context is stored with each decision. Exact check references cannot
be JSON object keys, so their environment sets serialize as sorted pairs;
duplicate check keys are refused when reading.

## Evidence and remaining coverage

The existing `crates/ymp-storage/tests/results.rs` scenario now runs real Scripted
production, checks and independent reviews through both belief implementations,
rejection, a new candidate, acceptance, exact P2 discharge and SQLite reopening.
It checks repeated correlated evidence, excluded old support/contradiction,
high priors without supporting evidence and mismatched prior parameters,
forged Satisfied, current-result invalidation,
policy history and rejected/foreign/forged/stale discharge sources. An independent
approval with no supporting evidence previews Accepted(Unconfirmed) while the
ledger remains unsatisfied.

`crates/ymp-runtime/tests/belief.rs` uses explicitly synthetic values for the
approximately 0.712 producer/inspection/static-read example, correlation,
contradiction and positive-coverage rules. It also distinguishes Discriminated
eligibility between the two credit policies; this is not a real hidden-check
acceptance. Removing the positive-support guard made this test fail with exit 101;
restoring it passed. The real integration scenario passed in 233.81 seconds.

Native inference, hidden/mutation checks, Browser/ExternalData/StaticRead source
adapters, consequence regrading, final aggregate acceptance and competence
observations are not delivered or verified by this task. Independent review
returned R1(9/10) ACCEPT. Final `make verify` exited 0: legacy scan, offline
workspace build, formatting, Clippy with denied warnings and all workspace tests.
The task register and `git diff --check` also passed.

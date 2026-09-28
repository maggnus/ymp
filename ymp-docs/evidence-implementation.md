# Attributed evidence and reviews

Canonical status remains in `tasks/records/W1-0019.json`. AcceptanceAuthority now
records evidence and independent review statements for an exact retained result.
The [W1-0010 consumer](acceptance-implementation.md) adds belief, grades,
acceptance and criteria-ledger transitions using this same applicability contract.

## Evidence sources

`AcceptanceAuthority.evidence` uses the original owner's SessionControl. The
request supplies references to existing runs or one recorded review; it cannot
choose class, independence, polarity, author or discrimination. The Journal
derives those fields again when applying EvidenceRecorded.

Check evidence uses exactly one Candidate run and optionally one Baseline run of
the same current check version and environment. Candidate must cover result.after;
Baseline must cover result.before. Pass becomes Supports, Fail becomes Contradicts,
and Error supplies neither polarity. Control runs and mixed candidate conclusions
are refused. Baseline failure is unknown when no valid baseline outcome exists;
mutation_score remains unknown because mutant execution is not implemented.

Current Command and ExactBytes CheckRuns produce Executed evidence for the
specific check performed. ExactBytes proves execution of a byte comparison;
candidate-program startup remains unobserved. Claim audit must preserve that
scope. A Review produces Inspection with no invented passing CheckRun. Approve
supports, Reject contradicts, and NeedsEvidence alone supplies no polarity.
StaticRead, Browser and ExternalData remain domain classes without new source
producers in this slice.

Trusted is derived only from an already owner-registered User check. An agent
check retains its origin, and its evidence becomes ProducerAuthored when that
check's author is the current result's producer. A former author's expired grant
does not erase valid historical attribution. Review-based Inspection is
IndependentVisible and cannot become Trusted.

## One applicability contract

`EvidenceScope` stores provenance: exact result and criterion references, plus
check/environment where applicable. It is not a selection of the current context.
An `ApplicabilityContext` supplied by the consumer names the expected result,
criterion references and allowed environment digests for each exact check.

`evidence_applicable`, `review_applicable` and `applicable_evidence` share one
iterative traversal of canonical Journal records. It follows Review bases and
derived Inspection back to their check evidence under the same expected context.
Changing an expected environment excludes every dependent statement. Unrecorded or
modified adapter objects are not canonical records and cannot pass this contract.
Visited sets bound repeated traversal; no recursive chain can overflow the stack.

For a Review, context.criteria includes all criteria of its result. A7 and claim
audit use the common selection; A8 groups the selected evidence by criterion and
its correlation rules. Consumers choose their actual assessment environment;
latest IDs or the last observed environment never choose it implicitly. Internal
validation while recording a statement only checks its declared existing sources
and does not define a future assessment context.

New results, changed criterion/check versions or a different expected environment
exclude inapplicable records without deleting them. Inspection follows the same
dependency checks as direct run evidence.

## Independent reviewer statements

Only a real, live Reviewer assignment with a Review contribution naming the exact
ResultVersion can submit a review. Gatekeeper requires a different Agent ID from
the producer and the capabilities A6 requires for the criterion. Existing
Scripted admission remains file-only; an Executed/Browser requirement does not
silently add RunProcess/Browser rights. FinalReviewer and Advocate admission remain
with their owning tasks.

The kernel derives reviewer/profile from the assignment. It retains Approve,
Reject and NeedsEvidence, Blocking/Advisory findings, proposed CheckSpecs and
version-bound evidence references as statements. Proposed checks are not
automatically registered. Completing the Reviewer invocation may discharge its
non-artifact commitment; its target result remains unaccepted.

## Evidence and limits

The existing real-candidate scenario in `crates/ymp-storage/tests/results.rs` now
also checks supporting and contradicting byte evidence, before/after discrimination,
foreign snapshots/environments, independent review, producer self-review refusal,
forged Trusted promotion, retained old evidence after a retry and environment
changes through the whole Review/Inspection chain. Both existing CheckRunner
implementations supply actual retained-byte runs. Disabling review-basis traversal
causes the E1-to-E2 review assertion to fail.

Native inference, Browser/ExternalData/StaticRead source adapters, mutations,
numeric belief, grades and acceptance are not exercised or implemented by this
slice. The check-author downgrade and inconclusive-run branches receive code
review; no separate end-to-end scenario is claimed for them. The test's wall-clock
watchdog permits debug SQLite replay of a larger history; product time limits
continue to use the unchanged clock and allowance contracts.

Final `make verify` exited 0: legacy scan, workspace build, formatting, Clippy
with denied warnings and all workspace tests. Independent review returned
R2(9/10) ACCEPT after the transitive-context correction. The first broad run also
caught an unsupported-subject error-code regression; the Review-only check now
precedes result lookup, preserving the prior denial for unsupported contribution
kinds, and the affected arbiter tests passed.

# Resource accounting implementation notes

Canonical task status is in `tasks/records/W1-0004.json`. This checkpoint supplies
pure resource values and policy implementations. Treasury's journal consumer,
reservations, evidence validation and settlement are not implemented yet; this
is not completion of W1-0004 or an admission/accounting guarantee.

## Current code

`ymp-domain/src/resources.rs` represents Budget, PriceBook/Rates, Reservation,
Receipt/Usage, Allowance, CostEstimate and ReportingPlan. Purpose, coverage and
unknown-usage values match the model. CostUnits remains the validated finite Real
used by intake. Cost sums and products round conservatively upward where needed;
available remainders round downward. Negative values, overflow and invalid cache
or reasoning subsets are refused. Usage.since subtracts an established cumulative
baseline and refuses resets instead of charging an entire native history again.

`ymp-kernel/src/ports/resources.rs` defines immutable policy input views and
input-digest-bound ResourceResponse values. These views are inputs to strategies,
not proofs of funding authority. Treasury must construct/validate them against its
recorded state before accepting a response.

`PriceWeighted` implements the model's uncached/cache-read/cache-write/output
formula with the 1/0.1/1.25/4 fallback rates. Reasoning is already part of output.
For estimates it uses matching non-estimated history bound to the exact profile,
provider, contribution kind, difficulty, policy and PriceBook digest; otherwise it
uses explicit expected usage and the configured p90 factor. These are initial
forecast assumptions, not measured calibration.

`PurposeBounded` caps allowances by the estimate, available resources, policy
ceiling and deadline. Contribution kind and difficulty influence time/output
limits. Reporting plans include a bounded narration and one correction when the
pool and budget allow them; otherwise they select deterministic reporting.

Typed parameter defaults are fully materialized by construction and stored in
PolicySelection. Restoring an existing selection retains its original parameter
bytes/PolicyRef identity, including integer versus floating JSON representation.
No new runtime dependency was introduced for pricing or arithmetic.

## Consumer work still required

Treasury must enforce the model's per-purpose reserves, preserve observations
before pricing, account for every authorized invocation, and make receipt
settlement idempotent. It must retain uncertainty after estimates and block paid
work when prior cost has no defensible upper bound. Any bound above the nominal
reservation must reduce available capacity by the full exposure.
Invocation funding must be recorded before external work can start; revocation
alone cannot settle or release it. A known actual charge above the reservation or
budget must be retained as an overrun that blocks further spending, not rejected
in a way that hides the incurred expense.

A private AccountingEvidence object will bind verified bounds or complete cost to
reservation/assignment/invocation and recorded bases. Deserializing a stored fact
must not create this capability. Treasury can establish NeverStarted before it
has authorized an invocation. Concrete executor-derived upper-bound/complete-cost
proof construction belongs to W1-0017; positive financial-rule tests can use a
closed test-only constructor and must label those facts as synthetic seam inputs.
A raw Receipt.cost or a policy's numeric claim is not such evidence.

The journal consumer must also enforce reporting-only admission and deterministic
reporting after a user stop, without implying that the actual session stop or
native execution lifecycle exists here. W1-0013/0014/0017 supply those integrations.
All financial transitions need typed events, deterministic replay, recoverable
policy parameters and Event.contents support for their immutable bytes. Tests of
substantially different policies through that real consumer remain outstanding.

## Checkpoint evidence

`ymp-runtime/tests/resource_policies.rs` checks cache-heavy pricing, independent
unknown-cost handling, cumulative baselines, bounded allowances, deterministic
report fallback, conservative arithmetic and exact policy identity restoration.
These are value/policy tests; they do not establish Treasury behavior or actual
native invocation accounting.

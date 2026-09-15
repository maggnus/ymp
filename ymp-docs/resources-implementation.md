# Resource accounting implementation notes

Canonical task status is in `tasks/records/W1-0004.json`. Treasury is a journal-backed
financial consumer. Its authorization records funding once; it does not start an
executor, issue an assignment Grant, stop external effects or deliver a report.

## Values and policies

`ymp-domain/src/resources.rs` represents Budget, PriceBook/Rates, Reservation,
Receipt/Usage, Allowance, CostEstimate and ReportingPlan. CostUnits is the validated
finite Real used by intake. Cost sums, products and retained exposure round upward
where needed; available remainders round downward. Negative values, overflow and
invalid cache or reasoning subsets are refused. Usage.since subtracts an established
cumulative baseline and refuses resets instead of charging an entire history again.
The host remains responsible for establishing that baseline and coverage.

`ymp-kernel/src/ports/resources.rs` defines read-only policy inputs and responses
bound to their input digest. Treasury reconstructs those inputs from the journal,
checks the selected implementation/version, parameters and basis references, and
commits the decision with its effective parameters. Deserializing a projection or
returning a proposal does not confer authority.

`PriceWeighted` uses A13's uncached/cache-read/cache-write/output formula and the
1/0.1/1.25/4 fallback rates. Reasoning is already part of output. Estimates use
matching known history bound to the exact profile, provider, contribution kind,
difficulty, policy and PriceBook digest; otherwise they use the explicit usage
forecast and p90 factor. These defaults are assumptions, not measured calibration.

`PurposeBounded` caps allowances by the estimate, available capacity, policy ceiling
and deadline. Contribution kind and difficulty influence time/output limits.
Narration uses the per-call reporting ceiling. ReportingPlan protects narration
and one correction or selects deterministic reporting, without a new user constraint.
Policy defaults are materialized; restoring a selection preserves its original
parameter bytes and identity, including integer versus floating JSON representation.

## Treasury and replay

`ymp-kernel/src/treasury.rs` opens one immutable budget/PriceBook, reserves resources,
authorizes each invocation identity once, observes receipts before pricing and
settles them through the selected CostModel. SessionView reconstructs all balances
from events. Intake cannot silently replace the opened budget or verification
reserve. Registry eligibility and deadlines are checked at reservation and again
at authorization; a narrowed deadline also narrows the invocation's timeout.

Both reservation and authorization enforce overall and per-purpose A13 limits.
Authorization includes existing holds without counting them twice, including when
another invocation has overrun since reservation. Reporting also fits its own
reserve. Revoking work retains the hold. A confirmed actual cost above the nominal
reservation or total budget is recorded, and further spending is constrained by
that overrun. Matching observations and committed settlements are idempotent;
conflicting or nonmonotone receipts are denied. Unsettled incomplete observations
may be replaced by later attributable usage; committed settlements are immutable.

Under UnknownUsage::Stop, unresolved consumption retains its hold and blocks
Production/Coordination. Only defensibly bounded protected work can continue within
its own reserve. Without a verified bound or complete cost, no further paid work is
funded. UnknownUsage::Estimate settles the labeled allowance estimate and retains
uncertainty; a larger verified bound leaves the difference held. Confirmed complete
cost is accounted as exposure before settlement and dominates a contradictory
smaller bound. Later incomplete observations retain that confirmed expense as a
minimum; a disproved bound is removed and cannot restore available funds. An
estimate with no defensible bound cannot create available funds.

AccountingEvidence is an opaque, non-deserializable capability bound to its issuing
Treasury, session, reservation, assignment, invocation, current reservation event
and basis. Complete-cost evidence also binds the exact receipt digest. Treasury
can establish NeverStarted only before it authorizes execution. A fresh instance
may issue new evidence for still-unstarted work, but cannot reuse an old instance's
capability. Raw Receipt.cost and policy numeric claims do not certify partial usage.

BudgetControl restricts entry into financial reporting mode. Narrated reporting
permits the planned narration and one correction after active work is stopped;
deterministic mode authorizes no reporting call. Reporting cannot reopen production.
The consumer retains a post-delivery Curate condition for unprotected learning,
but delivery and cancellation transitions are not implemented by Treasury.

## Evidence and integration limits

- `ymp-runtime/tests/resource_policies.rs` checks cache-heavy pricing, unknown
  coverage, cumulative baselines, bounded allowances, reporting fallback,
  conservative arithmetic and exact policy restoration.
- `ymp-storage/tests/treasury.rs` exercises the real Treasury with both initial
  policies and materially different flat-price/fixed-allocation implementations.
  It covers purpose boundaries, authorization after overruns, duplicate/late/conflicting
  receipts, release versus revocation, raw-cost rejection and SQLite reopening with
  known charges, unstarted holds and unbounded estimates. Cached cumulative usage is
  normalized and charged once through the consumer.
- `ymp-kernel/src/treasury_tests.rs` uses a private test-only evidence constructor
  and the real Treasury. It checks retained bound exposure before/after estimates,
  bounded Stop behavior, contradictory cost/bound observations, complete-cost
  settlement and rejection of evidence from the wrong issuer, session, assignment,
  invocation, state or receipt. These are synthetic accounting seams, not completed
  native invocations. A compile-fail example checks that stored data cannot
  deserialize into AccountingEvidence.

W1-0006 integrates funding with assignment admission. W1-0017 must derive executor
bounds/complete-cost evidence from validated concrete facts and normalize actual
receipts; no public numeric proof factory is provided here. W1-0013/W1-0014 own
session stop, narrated/deterministic report delivery and post-delivery integration.
Their end-to-end claims are not established by these financial tests. No native
model calls, hardware power-loss claims or new runtime dependencies are involved.

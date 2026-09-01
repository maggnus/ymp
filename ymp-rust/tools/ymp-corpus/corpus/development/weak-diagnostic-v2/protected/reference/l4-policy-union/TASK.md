# L4 policy union

Implement the library in `src/lib.rs` without changing this contract, the package manifest, lock
file, or visible tests.

`compile_plan` independently normalizes quota and route contributions, then joins them into one
deterministic plan. Repeating an identical quota or route is idempotent; conflicting definitions
return `PolicyError::QuotaConflict` or `PolicyError::RouteConflict`. Every route must survive the
join and reference a service with a quota, otherwise compilation returns
`PolicyError::MissingQuota`. The returned maps use their canonical key order.

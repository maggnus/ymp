# Preserve ordered replay through a dependent event pipeline

Implement the event ledger without changing its public API. Each input passes through the fixed
dependency order `decode → predecessor validation → transition validation → commit identity`.

## Requirements

1. Lines use `id|predecessor|delta`; empty identifiers, malformed fields, zero deltas, and numeric
   overflow are rejected.
2. The first predecessor is `GENESIS`; every later predecessor is the current committed head.
3. A rejected event changes neither total nor head. Its result is nevertheless bound to the event
   identifier, so reuse replays the first result even after intervening accepted events.
4. A successful receipt reports the committed total and deterministic head.

Only files under `src/` may change. The dependency order is part of the public contract. This is an
expected-null control: parallel branch synthesis is not expected to help because every transition
depends on the preceding committed result.

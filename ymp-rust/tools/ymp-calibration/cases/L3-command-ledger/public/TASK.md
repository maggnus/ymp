# Task

Repair the in-memory command ledger while preserving all public types and method signatures. A
non-empty command ID is an idempotency key for the complete command and its result. Repeating the
same ID with the same command must return the original result without changing state; a repeated
successful result sets `Outcome::replayed` to `true`. Reusing an ID with a different command must
return `IdempotencyConflict` without changing state.

All results of a well-formed command ID, including `InvalidAmount`, `Insufficient`, and `Overflow`,
are final for that ID. They must be replayed as the same error even if intervening commands change
the available amount. An empty ID returns `InvalidCommandId` and is not recorded. Reserve and
credit amounts must be non-zero, subtraction and addition must be safe, and an error must never
partially change state.

Add regression tests for temporal replay and conflicting reuse, run all tests and strict Clippy,
do not add dependencies, and do not commit.

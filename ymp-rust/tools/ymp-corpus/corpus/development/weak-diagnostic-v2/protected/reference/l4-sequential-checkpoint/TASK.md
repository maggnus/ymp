# L4 sequential checkpoint

Implement the library in `src/lib.rs` without changing this contract, the package manifest, lock
file, or visible tests.

`Ledger::apply` processes one `Command` at a time. A command identifier is immutable: repeating the
same identifier and payload returns the exact recorded result, including a rejection, even after
later commands change the ledger. Reusing an identifier with a different payload returns
`ApplyError::CommandConflict`.

For a new identifier, processing order is fixed: validate the expected generation, calculate the
signed value transition with overflow checking, increment the generation, then record the result.
A rejected command changes neither value nor generation. `Ledger::value` and `Ledger::generation`
expose the committed state.

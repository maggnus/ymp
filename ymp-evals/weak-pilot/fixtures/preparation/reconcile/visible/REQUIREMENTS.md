# Reconcile integer CSV entries

Read `ledger.csv` in file order and create `totals.csv` and `exceptions.csv`.
Do not change the inputs or public checks. Use exact integer arithmetic.

The header is `entry_id,account,currency,quantity,unit_cents,status,ref_id`.
The input is valid UTF-8 CSV. IDs and group names are case-sensitive, nonempty
strings. Quantities are signed integers; unit prices are nonnegative integers.
All statuses are `posted`, `void` or `cancel`. Posted/void references are empty;
cancel quantities and prices are zero. Quoted account names may contain commas.

Process each row with this precedence:

1. The first occurrence of an entry_id wins, including rejected rows. Every later
   occurrence is excluded with reason `duplicate_id`.
2. A first `void` row is excluded with reason `void`.
3. A first `posted` row becomes an active entry, contributing quantity * unit_cents
   and one active entry to its (account, currency) group. Zero and negative totals
   count normally.
4. A first `cancel` row must reference an earlier accepted posted entry. Otherwise
   exclude it with `unknown_target` (this includes references to future, void or
   cancel rows). If that posted entry was already cancelled, use `inactive_target`.
   If it is active but the cancel's account or currency differs, use
   `group_mismatch`. Otherwise remove the referenced entry completely. The valid
   cancel itself contributes neither money nor count. Excluded cancels do not
   change the target. A posted entry removed by a valid cancel is not an exception.

`totals.csv` has header `account,currency,total_cents,active_entries`. Include
only groups with at least one remaining active entry, even when their sum is zero.
Sort by (account, currency) using Python string order. `exceptions.csv` has header
`row_number,entry_id,reason`; list every excluded row in input order. Row 1 is the
first data row, not the header. Emit one reason per excluded row.

Both outputs must use UTF-8 without BOM, LF line endings, comma delimiters, double
quotes with minimal quoting (quote only fields containing comma, double quote,
CR or LF), canonical decimal integers, no spaces added, and a final newline.
No additional columns or explanatory rows are allowed.

You may inspect files, run Python 3.9+ and standard-library tools, write a script
or calculate directly, and create your own tests in this directory. No network,
packages, browser, other workspaces, native subagents or model calls are permitted.
`python3 public_test.py` checks only output shape; its success is not correctness.

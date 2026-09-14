# YMP-159 structured response parser evidence

Branch: `fix/ymp159-structured-response-parser`

Base: `a8367c4169bf17a33566d45b5bd537ce30d59869`

The change is limited to `ymp-core::model::parse_response`, its unit tests, and
this evidence directory. No provider, runtime, storage, TUI, CLI, dependency,
release, or task-register code changed. The real owner session and application
data were not opened, resumed, modified, or copied. No native provider was run.

## Parser contract

A brace block is JSON-looking when it starts with a valid JSON object token,
contains a nested object candidate, starts with a single-quoted member, or has
multiple unquoted object-style members. JSON whitespace after `{` is ignored
for this classification. Ordinary balanced CSS declarations such as
`body{overflow-x:hidden}` and quoted CSS brace values remain non-JSON blocks;
the definition does not depend on provider-specific tokens. The exact schema
member `approved` is treated as a decision signal even in a one-member
unquoted block, so a malformed draft cannot be skipped before a later result.

The parser skips a complete outer brace block that clearly cannot begin a JSON
object. It treats a JSON-looking outer block as one indivisible candidate,
validates its complete syntax, and never searches inside it after a syntax
error. A second valid object is ambiguous. Nonempty text after the selected
object remains an error. Final deserialization into generic `T` remains the
authoritative schema check.

Tests cover the sanitized `message233` shape (`body{overflow-x:hidden}` before
one final review), non-JSON braces in quoted commentary and CSS, JSON whitespace
after a skipped brace block, two valid decisions, malformed JavaScript-,
Python-, and YAML-style drafts, malformed and incomplete outer wrappers with
nested candidates, malformed JSON before a valid decision, trailing prose,
direct JSON, fenced JSON, and the existing streamed-commentary case.

## Before and after

The regression was added while the base parser implementation was unchanged.
It failed with the malformed-JSON error from the first CSS block; the captured
output is in `regression-before.txt`. The same test passes after the parser
change in `regression-after.txt`. The complete parser test group passes 12 of
12 tests in `targeted-tests.txt`.

Source bindings before the test-first change and after implementation are in
`source-hashes.txt`.

## Final checks

Candidate `1ab42b1` was reworked after independent review. The five mandatory
probe cases now reject the complete malformed or incomplete outer block instead
of accepting a later or nested decision. The whitespace test fails under the
reviewer's whitespace mutation, and focused mutations also prove that the
unquoted-draft and nested-candidate protections are observed. See
`rework-controls.md` and `rework-probe.jsonl`.

Formatting and strict workspace Clippy pass. The required unfiltered workspace
test command was run and reached five failures in `weak_pilot_consumer`; that
test executable intentionally rejects any product change relative to
`1c17f4e`. The YMP-159 base `a8367c4` already differs from that frozen revision
in the accepted YMP-146 TUI files, so the same guard rejects the base before it
can exercise its fixtures. No guard was bypassed or changed.

A supplemental workspace run skipped only those five base-incompatible test
cases. Every remaining test passed, with the two existing ignored tests left
ignored. Those results describe candidate `1ab42b1`; see `final-checks.md`.

For the rework, YMP-161 remained in progress and its fix was not present in this
branch, so the known frozen-product guard still blocked an honest unfiltered
workspace run. Formatting, strict workspace Clippy, focused parser tests, and
the independent 32-case probe passed on the reworked source. See
`rework-final-checks.md`.

## Second bounded rework

The residual independent finding on candidate `90eda29` is closed. When a
balanced outer block is classified as non-JSON, the parser now continues
searching inside it instead of skipping the complete range. Cases `c34` and
`c44` therefore reject the nested draft plus opposite final decision as
ambiguous, while `c35` rejects its nested malformed object. A single unquoted
`approved` member is a malformed Review decision; an unrelated single-member
`verdict` block remains commentary. The sanitized `message233` case and
`a{color:red}` remain accepted. The six requested focused tests, formatting,
and strict workspace Clippy passed. The parent will run workspace tests after
integration with the already accepted YMP-161 correction.

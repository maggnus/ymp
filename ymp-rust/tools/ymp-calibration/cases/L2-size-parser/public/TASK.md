# Task

Repair `parse_size` while preserving its public API and error enum. The accepted grammar is an
unsigned non-empty ASCII decimal integer immediately followed by either no unit or exactly one of
`B`, `KiB`, `MiB`, and `GiB`. Whitespace, signs, separators, Unicode digits, and differently cased
units are invalid. Arithmetic must be checked: a syntactically valid value that does not fit in
`u64` returns `ParseSizeError::Overflow` and must never panic. An empty input returns `Empty`; a
missing or malformed number returns `InvalidNumber`; an otherwise valid number with an unsupported
suffix returns `InvalidUnit`.

Add focused regression tests, run all tests and strict Clippy, and do not add dependencies or
commit changes.

# Repair two independent bstr byte-string behaviours

Source revision: cc13102d64de7b43efaf804046065b68f6c9a0c0.

## Goal

Repair both observable behaviours without changing the public API or unrelated formatting and I/O
semantics.

## Requirements

1. BSTR-EOF: record iteration over BufRead must treat an empty fill_buf result as terminal EOF. It
   must return without requesting another buffer and without emitting a record.
2. BSTR-DEBUG: the Debug representation of invalid UTF-8 bytes in BStr must use exactly two
   lowercase hexadecimal digits after \x.

## Visible check

Run cargo test --locked --offline -p bstr --lib. The protected oracle uses held-out byte and reader
instances for the same two requirements.

## Scope

Changes may affect only the implementation and tests needed for these requirements. Dependency,
manifest, public API, and unrelated behaviour changes are out of scope.

# Preserve escape-parser state after lone JSON surrogates

Source revision: 4c28c5737b26f15bc7457d53eff8517b33cb4a43.

## Goal

Correct the byte-buffer JSON string path after a lone leading surrogate without weakening normal
string validation or changing unrelated deserialization.

## Requirement

JSON-SURROGATE-STATE: when validation is disabled for a byte buffer, a lone surrogate must remain
available in its WTF-8 byte form and parsing must resume at the immediately following escape.
Valid escapes must be decoded, while invalid, incomplete, or repeated leading-surrogate sequences
must still be rejected.

## Visible check

Run cargo test --locked --offline -p serde_json@1.0.71 --lib --tests --all-features. The protected
oracle supplies held-out surrogate and following-escape combinations covered by the public
requirement.

## Scope

Changes may affect only JSON string escape parsing and its tests. Public API, dependency,
manifest, numeric, map, and serialization changes are out of scope.

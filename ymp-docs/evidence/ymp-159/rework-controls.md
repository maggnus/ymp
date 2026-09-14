# YMP-159 rework controls

Candidate: `1ab42b1f3d5d4f0d1e6df28c93633128b659327f`

Final pre-commit `model.rs` SHA-256:
`8d3aa8c52ae1293d7c900b806cd0b1b30afc1671bf0d52784789b329592cf479`

Final pre-commit Git blob:
`f4ce5584e0c5f97184686aaadc24c42f18cd3eaf`

## Mandatory cases

The independent probe was compiled against the current worktree through a
temporary Cargo manifest. The complete output is in `rework-probe.jsonl`.

| Case | Final result |
| --- | --- |
| `c16_js_style_draft_opposite_before_final` | Rejected as malformed JSON. |
| `c17_python_dict_draft_before_final` | Rejected as malformed JSON. |
| `c18_unclosed_unquoted_outer_with_nested` | Rejected as an incomplete JSON object. |
| `c19_malformed_inner_in_non_json_block` | Rejected as malformed JSON. |
| `c32_yaml_like_draft_multiline` | Rejected as malformed JSON. |

The same probe still accepts the sanitized `message233` shape, direct JSON,
plain and JSON fences, JSON whitespace, UTF-8 source text, nested valid data,
and CSS containing a single-quoted brace. It still rejects multiple decisions,
malformed or incomplete JSON-looking objects, nested candidates, and trailing
text.

## Focused negative controls

Each mutation was applied to the current source, its named test was run, and
the source was restored before the next control.

| Mutation | Command | Expected observation |
| --- | --- | --- |
| Stop skipping JSON whitespace after `{`. | `cargo test -p ymp-core model::tests::parses_json_whitespace_after_a_non_json_brace_block -- --exact` | Exit 101; the test failed because no complete JSON object was found. |
| Stop classifying multiple unquoted members as one candidate. | `cargo test -p ymp-core model::tests::rejects_non_json_object_drafts_and_nested_fragments -- --exact` | Exit 101; a later valid decision was incorrectly accepted. |
| Stop classifying a nested `{` as part of its outer candidate. | `cargo test -p ymp-core model::tests::rejects_non_json_object_drafts_and_nested_fragments -- --exact` | Exit 101; a nested or later decision was incorrectly accepted. |

The final `model.rs` SHA-256 after all restorations was again
`8d3aa8c52ae1293d7c900b806cd0b1b30afc1671bf0d52784789b329592cf479`.

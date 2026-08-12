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

## Approval-set artifact binding

The canonical binding below lists every other artifact subject to the same decision by role and
SHA-256. Protected artifacts are identified only by digest. Entries are sorted by role. To avoid a hash cycle, the
binding omits this file, its task-manifest envelope, the top-level registry, and the separate owner
decision record. The task manifest binds the exact bytes of this file and the registry binds the
exact task manifest.

<!-- YMP-CORPUS-BINDING-V1-BEGIN -->
{
  "schema_version": 1,
  "task_id": "bstr-eof-and-debug",
  "artifacts": [
    {"role": "corpus.approval_policy", "sha256": "e5e81e2d4b7a03292f3f75a6d64a0e7908cb847cd02e54366b2c6320951b200a"},
    {"role": "corpus.budget_policy", "sha256": "78b410fc8e2dc55c85ffb3a1e1404d954309b26ff0709b47338fb3edf3bdae3d"},
    {"role": "corpus.evidence_policy", "sha256": "ad0755e0d57d5f4caada72e031d2de098939ce37d78c79219002486a5b1beaea"},
    {"role": "corpus.expansion_policy", "sha256": "3718bd97288ab8304dec0acc5e9825aad45434629786327594579382ff1ce5c9"},
    {"role": "corpus.observation_policy", "sha256": "a33e23b316b06de8cc6d13e967436f8cb89b58234929e42cc3a5cfa3b79241a2"},
    {"role": "protected.control.eof-stop", "sha256": "8bd8532960f95eb4598ca4f02a8503cb65067700d78caf82e9d3f04d00501fb6"},
    {"role": "protected.control.lowercase-debug", "sha256": "c672cfa1661de4b2bed4caf216069ba84eddbfada2d3a997843e29f7b4fcc871"},
    {"role": "protected.oracle", "sha256": "9d4925ce08644fc3ac9fee03e86bd92ac051f8286973f24bd31337ea1c554232"},
    {"role": "task.fixed_archive", "sha256": "ebbc17f687e279fbe305c6735ad7f3b69d26577dc52130ba1d5bac5d9411a069"},
    {"role": "task.fixed_lock", "sha256": "ccad548f61de67f04c959247e6503f920a1bbfb3c3512ad7945ab0e5dad0c3bf"},
    {"role": "task.requirement_matrix", "sha256": "dc20cac4ad040d14bf56ae52a368770f170db76210f846828996364e128f579d"},
    {"role": "task.source_archive", "sha256": "bdb29953c4cc14b5d9b51d0350ef2b290a2ddee5354b599a11725c6c0cad606b"},
    {"role": "task.source_lock", "sha256": "fad92cf2402fd544e78c0bbadeec211ce5a45d53c1b79fa56b457c7dad559bbf"},
    {"role": "task.technical_review", "sha256": "47f5dad0a665325d132e521010334d72a18b852c7fcec8dc1672c0858fe71f30"},
    {"role": "task.vendor_tree", "sha256": "fd535557cf00093f948db6ccfe17d23a0be03e1f268530a156f35d0424b147d5"}
  ]
}
<!-- YMP-CORPUS-BINDING-V1-END -->

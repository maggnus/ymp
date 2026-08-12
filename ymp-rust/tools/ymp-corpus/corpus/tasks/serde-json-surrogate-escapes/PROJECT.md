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

## Approval-set artifact binding

The canonical binding below lists every other artifact subject to the same decision by role and
SHA-256. Protected artifacts are identified only by digest. Entries are sorted by role. To avoid a hash cycle, the
binding omits this file, its task-manifest envelope, the top-level registry, and the separate owner
decision record. The task manifest binds the exact bytes of this file and the registry binds the
exact task manifest.

<!-- YMP-CORPUS-BINDING-V1-BEGIN -->
{
  "schema_version": 1,
  "task_id": "serde-json-surrogate-escapes",
  "artifacts": [
    {"role": "corpus.approval_policy", "sha256": "e5e81e2d4b7a03292f3f75a6d64a0e7908cb847cd02e54366b2c6320951b200a"},
    {"role": "corpus.budget_policy", "sha256": "78b410fc8e2dc55c85ffb3a1e1404d954309b26ff0709b47338fb3edf3bdae3d"},
    {"role": "corpus.evidence_policy", "sha256": "ad0755e0d57d5f4caada72e031d2de098939ce37d78c79219002486a5b1beaea"},
    {"role": "corpus.expansion_policy", "sha256": "3718bd97288ab8304dec0acc5e9825aad45434629786327594579382ff1ce5c9"},
    {"role": "corpus.observation_policy", "sha256": "a33e23b316b06de8cc6d13e967436f8cb89b58234929e42cc3a5cfa3b79241a2"},
    {"role": "protected.control.surrogate-state", "sha256": "ceea2a29f2f443f04ab4922b9a64bcc4628f86acf119b965301b211e66ee63de"},
    {"role": "protected.oracle", "sha256": "d593c1e3c36c95c6416df51dee4822ed1da873b8c36e84c0dcd66242c5f893e9"},
    {"role": "task.fixed_archive", "sha256": "6aac4ff7bf42bca4f13a2a12aec0a9109a64f2ba30bb1504be67b7e58a8651f7"},
    {"role": "task.fixed_lock", "sha256": "483b18972121f69207cf94c881ebdfd0184e31ec2ad087cb2570e0a53e719cb9"},
    {"role": "task.requirement_matrix", "sha256": "e3c09550e85bccb3649dba6702c1d91c95a768f35e699df4b29a5c48fbcdc0dc"},
    {"role": "task.source_archive", "sha256": "de4feabf1e003eabac1d9ff9e2ed458d155a8014859ef5a8465d0fa1c696812e"},
    {"role": "task.source_lock", "sha256": "483b18972121f69207cf94c881ebdfd0184e31ec2ad087cb2570e0a53e719cb9"},
    {"role": "task.technical_review", "sha256": "c28bdc6356b562162880d64697fa8fd5435588cac2abf94fd4bf25341508dba0"},
    {"role": "task.vendor_tree", "sha256": "72575df23df458ce96319f8940d4aca9af5f015d8f9f5f90000abd7e85ea9005"}
  ]
}
<!-- YMP-CORPUS-BINDING-V1-END -->

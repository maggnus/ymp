# Repair partial less-than SemVer requirement evaluation

Source revision: 35d918d334cdecc9725e217f460b90f478b5dabd.

## Goal

Make partial less-than comparators follow Cargo-compatible prerelease boundaries inside compound
requirements that otherwise admit a prerelease, without changing parsing or unrelated comparison
operators.

## Requirements

1. SEMVER-LT: <I.J must match stable versions below minor J and must exclude prereleases at the
   I.J boundary even when another comparator in the same requirement admits that prerelease.
2. SEMVER-LTE: <=I.J must include stable releases in minor J but must exclude prereleases at its
   lower boundary even when another comparator in the same requirement admits that prerelease.

## Visible check

Run cargo test --locked --offline -p semver --lib --all-features. The protected oracle uses
held-out compound requirements and versions around the declared boundaries.

## Scope

Changes may affect only requirement evaluation and its tests. Parser, display, dependency, and
manifest changes are out of scope.

## Approval-set artifact binding

The canonical binding below lists every other artifact subject to the same decision by role and
SHA-256. Protected artifacts are identified only by digest. Entries are sorted by role. To avoid a hash cycle, the
binding omits this file, its task-manifest envelope, the top-level registry, and the separate owner
decision record. The task manifest binds the exact bytes of this file and the registry binds the
exact task manifest.

<!-- YMP-CORPUS-BINDING-V1-BEGIN -->
{
  "schema_version": 1,
  "task_id": "semver-less-than-prerelease",
  "artifacts": [
    {"role": "corpus.approval_policy", "sha256": "e5e81e2d4b7a03292f3f75a6d64a0e7908cb847cd02e54366b2c6320951b200a"},
    {"role": "corpus.budget_policy", "sha256": "78b410fc8e2dc55c85ffb3a1e1404d954309b26ff0709b47338fb3edf3bdae3d"},
    {"role": "corpus.evidence_policy", "sha256": "ad0755e0d57d5f4caada72e031d2de098939ce37d78c79219002486a5b1beaea"},
    {"role": "corpus.expansion_policy", "sha256": "3718bd97288ab8304dec0acc5e9825aad45434629786327594579382ff1ce5c9"},
    {"role": "corpus.observation_policy", "sha256": "a33e23b316b06de8cc6d13e967436f8cb89b58234929e42cc3a5cfa3b79241a2"},
    {"role": "protected.control.partial-less-than", "sha256": "bb538f455291e891ce887039c0d836474fb33a43ab30485d5d2e05a15b4b2fc5"},
    {"role": "protected.oracle", "sha256": "8f458c1a8c95a0db8701a2b1ff5de67a11fbc8cf3e7f88bce332c2066888cd1f"},
    {"role": "task.fixed_archive", "sha256": "8c64149ff923d33a79514d7b0884c01c36956dc49aa9c91a43b5561f07cd18f1"},
    {"role": "task.fixed_lock", "sha256": "f09abe7b1d9830320701b2a2a06a0d106bbc47379c952a5453e05c0b3f835a47"},
    {"role": "task.requirement_matrix", "sha256": "f88aa85c245c9340783c994b8b9df33706b47ec56e803649163c7a0a24e11977"},
    {"role": "task.source_archive", "sha256": "8e909f8bedaf41b2f65849327f11c740f92a0dd42ccd7cf2b99090d65888f1d0"},
    {"role": "task.source_lock", "sha256": "f09abe7b1d9830320701b2a2a06a0d106bbc47379c952a5453e05c0b3f835a47"},
    {"role": "task.technical_review", "sha256": "7dfc9e515992e3a8c58f9a53d2d676260f4220e6265c634887192586ff624d65"},
    {"role": "task.vendor_tree", "sha256": "73d134855ddeea37d4626987c97a84200e0350ffd848909d13e1af413bed8a25"}
  ]
}
<!-- YMP-CORPUS-BINDING-V1-END -->

# Repair independent TOML datetime bounds

Source revision: 99d50bb686adcdb569c0a1d37db1ac175c57e5ea.

## Goal

Repair two independent datetime validity bounds without changing parsing syntax, public API, or
other TOML crates.

## Requirements

1. TOML-LEAP-SECOND: seconds value 60 must be accepted as the leap-second boundary and values
   greater than 60 must be rejected.
2. TOML-CALENDAR-DAY: day validity must follow Gregorian month lengths and leap-year rules,
   including century and 400-year exceptions.

## Visible check

Run cargo test --locked --offline -p toml_datetime --lib. The protected oracle supplies held-out
boundary dates and times from the same public rules.

## Scope

Changes may affect only toml_datetime validation and its tests. Dependencies, manifests, public
API, other workspace crates, and formatting or serialization semantics are out of scope.

## Approval-set artifact binding

The canonical binding below lists every other artifact subject to the same decision by role and
SHA-256. Protected artifacts are identified only by digest. Entries are sorted by role. To avoid a hash cycle, the
binding omits this file, its task-manifest envelope, the top-level registry, and the separate owner
decision record. The task manifest binds the exact bytes of this file and the registry binds the
exact task manifest.

<!-- YMP-CORPUS-BINDING-V1-BEGIN -->
{
  "schema_version": 1,
  "task_id": "toml-datetime-validation",
  "artifacts": [
    {"role": "corpus.approval_policy", "sha256": "e5e81e2d4b7a03292f3f75a6d64a0e7908cb847cd02e54366b2c6320951b200a"},
    {"role": "corpus.budget_policy", "sha256": "78b410fc8e2dc55c85ffb3a1e1404d954309b26ff0709b47338fb3edf3bdae3d"},
    {"role": "corpus.evidence_policy", "sha256": "ad0755e0d57d5f4caada72e031d2de098939ce37d78c79219002486a5b1beaea"},
    {"role": "corpus.expansion_policy", "sha256": "3718bd97288ab8304dec0acc5e9825aad45434629786327594579382ff1ce5c9"},
    {"role": "corpus.observation_policy", "sha256": "a33e23b316b06de8cc6d13e967436f8cb89b58234929e42cc3a5cfa3b79241a2"},
    {"role": "protected.control.calendar-days", "sha256": "6bff35d4f98867e8da68cd3b0cd50794ef90163ad47c6c7940294839b16014b7"},
    {"role": "protected.control.leap-second", "sha256": "6e0e84429a9b9f369cde677cadde7c36ee121a7b2a38138af1d39de094eb024d"},
    {"role": "protected.oracle", "sha256": "eb782986c9d9b341ebfa5ec8e8381963d38ee6e044c3f43072f178eb716dd0f8"},
    {"role": "task.fixed_archive", "sha256": "9dfed63f98283dd73326ff103bee20d18a651612fbc647d206cd83553cfed0ae"},
    {"role": "task.fixed_lock", "sha256": "4fe086693ba6a075fb6b9a5d5196ebb28092f03da9935b47fab94962d419afe7"},
    {"role": "task.requirement_matrix", "sha256": "86db19aa588a52619682953dee77dca8141597cba917efcca1632279fb757dcb"},
    {"role": "task.source_archive", "sha256": "6068b1af5c4eaa76dc2d6653a1a8f681b371fb2a2151777637d6531c99a960c7"},
    {"role": "task.source_lock", "sha256": "4fe086693ba6a075fb6b9a5d5196ebb28092f03da9935b47fab94962d419afe7"},
    {"role": "task.technical_review", "sha256": "408bd52c2b5c61b0caea5efdbc3b1ddd64c52b95fa5baf1a1bcb468f6080be90"},
    {"role": "task.vendor_tree", "sha256": "0c55f7ec82e580bd0d728792586eeceef38c8e4cc3c17f83c9b3b8743fca0ad5"}
  ]
}
<!-- YMP-CORPUS-BINDING-V1-END -->

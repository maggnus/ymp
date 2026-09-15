# Historical self-organizing team domain model amendments

Status: superseded as architectural guidance, 2026-09-15.

The owner approved the [self-organizing team domain model](self-organizing-team-domain-model.md)
as the single architectural authority. The earlier proposals below are retained
for historical context only; they must not override or supplement that model.
In particular, the advisory-only interpretation of A8 was not adopted: approved
A8 uses belief together with applicable evidence coverage for `Satisfied`.
Implementation must follow the model's current definitions and consistency rules,
not the historical mappings below. This file grants no implementation authority.

## Earlier proposals (not normative)

## Acceptance and belief

Criterion satisfaction and acceptance are governed by deterministic evidence and
authority rules. The likelihood-ratio belief values in algorithm A8 are advisory;
by themselves they never produce `Satisfied` or an acceptance decision.

## Declared and actual independence

Independence levels and model-family diversity are declared assumptions. Their
weights require calibration against observed outcomes and do not guarantee factual
independence between attempts, checks, reviews, or evidence sources.

## Criterion revision

The proposed `AcceptanceContract` is versioned, but the model does not define a
criterion-revision lifecycle. A revision protocol, including explicit user
confirmation for a material criterion change, remains an open design item.

## Canonical terminology mapping

The following mapping was proposed before approval. It is retained as historical
text and does not redefine names or structures in the approved model:

| Transferred specification term | Canonical interpretation and required adaptation |
| --- | --- |
| `Task`, `Plan`, `WorkItem` | `Task` is the user's complete goal, acceptance contract, and constraints. `Plan` arranges work, and each actionable plan unit is a `WorkItem`, never another product `Task`. Repository development task records such as `W1-0001` remain separate development tooling. |
| `Goal`, `Assumption`, `Clarification` | `Goal.request` corresponds to the canonical `Goal` and must preserve the user's request as stated. Assumptions and clarifications may accompany it as accountable records but must not silently rewrite it. |
| `Constraints`, `Pins` | Canonical `Constraints` are user-stated conditions. Typed budgets, permissions, roster choices, and limits may be faithful projections with provenance; defaults or inferred values must not be represented as user constraints or authority. |
| `Runtime` | At admission, state transition, evidence validation, and acceptance sites, the generic name means the named kernel service that owns the decision. It does not transfer that authority to the `ymp-runtime` assembly crate or an adapter. |
| `ExecutionProfile`, `ModelOffering` | These names retain actual native settings and environment information. Model family, effort, capability, or other unknown metadata must remain unknown rather than being inferred to complete the proposed structures. |
| `Grant` | A `Grant` is assignment-scoped authority issued after kernel validation. A token or `token_digest` may represent a grant but cannot create or extend authority by itself. |
| `Board` | The proposal's notices, solicitations, and commitments are not an exhaustive canonical `Board`. Canonical `Board` also exposes proposals, objections, and kernel decisions; decision authority and durable truth remain in the `Journal`, not in the board projection. |
| `ResultVersion`, `Snapshot`, `Artifact` | `ResultVersion` identity must bind to actual candidate contents. Snapshot and artifact digests can provide that binding; a work-item association or summary alone cannot. |
| `Check`, `CheckSpec` | Proposed `Check`/`CheckSpec` is an executable specialization of canonical `Check` and does not narrow the canonical term. Executability, independence, and visibility are additional requirements only where the proposal explicitly invokes them. |
| `Evidence` | Canonical `Evidence` used for criterion satisfaction, review, or acceptance covers the exact result, criterion, and checker versions and identifies the check source and environment. Evidence without a result binding is preliminary only: it cannot support acceptance and cannot be carried to a changed result without a new applicable check. This restates the binding already required by [`domain.md`](domain.md), [`intent.md`](../intent.md) and [`architecture.md`](architecture.md); it adds no new versioning entity or subsystem. |
| `Acceptance`, `ConfirmationGrade` | Each canonical `Acceptance` decides one immutable `ResultVersion`; a final aggregate must first be captured as a `ResultVersion`. `ConfirmationGrade` is proposed grading vocabulary for the recorded basis, not a representation of the separate canonical `Confirmation`. A grade constitutes canonical `Confirmation` only when its actual basis provides independent deterministic or external support; the grade label alone does not combine criterion satisfaction, review, acceptance, and confirmation. |
| `Journal`, `Store`, `SessionView`, `Revision` | `Journal` owns atomic ordered session facts and coherent histories. `SessionView` is the immutable projection at a stream `Revision`. `Store` is a proposed implementation or projection mechanism, not a second canonical authority or trusted service. |

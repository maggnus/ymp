# Domain language

This is a companion vocabulary and an explanation of some foundation API names.
The approved [self-organizing team model](self-organizing-team-domain-model.md)
owns normative names, structures and semantics. This summary cannot override it;
in particular, foundation API shapes need not yet implement the full target model.
Names identify responsibilities; a Rust crate is an implementation boundary and
may contain several closely related responsibilities.

## Task and work

| Name | Meaning |
| --- | --- |
| `Goal` | The user's requested outcome, preserved as stated. |
| `Criterion` | An individually identifiable condition used to judge the result. |
| `AcceptanceContract` | The criteria and evidence requirements governing a task. |
| `Constraints` | User-stated conditions on scope, resources, participants or permitted actions. |
| `Task` | A goal with its acceptance contract and constraints. |
| `Session` | The accountable history and state of solving one task. |
| `Contribution` | Proposed work aimed at a requirement, evidence gap or diagnosed problem. |
| `Plan` | A versioned arrangement of work and dependencies. |
| `WorkItem` | An actionable part of the plan. |

## Participants and authority

| Name | Meaning |
| --- | --- |
| `Agent` | A participant with an identity, instructions and context. |
| `Provider` | The native environment supplying models, tools and execution. |
| `ModelOffering` | A discovered model and its supported native settings. |
| `ExecutionProfile` | The captured settings and environment of an agent invocation. |
| `Pool` | Agents eligible to be considered, with explicit exclusion reasons. |
| `Team` | The agents participating in a particular session. |
| `Assignment` | Bounded work assigned to one agent with a role, permissions and limits. |
| `Grant` | Authority to perform specified operations within an assignment. |
| `Invocation` | One admitted execution of an agent. |
| `Commitment` | An accepted responsibility with an explicit lifecycle and expiry conditions. |
| `Board` | Shared or addressed notices, proposals, objections and kernel decisions. |

An agent is not a provider label or model name. Team membership, availability and
currently running work are separate facts. A role has no authority beyond its
assignment. Requested, sent and reported native settings retain their own meanings.

## Results and experience

| Name | Meaning |
| --- | --- |
| `ResultVersion` | An immutable candidate identified with its actual artifact contents. |
| `Check` | A defined method for assessing a criterion. |
| `Evidence` | Attributed, version-bound support or contradiction for a criterion. |
| `CriterionEvaluation` | A per-criterion projection of observed check evidence as satisfied, failed or not evaluated; it is not an `Acceptance` decision. |
| `Review` | An independent assessment of a result with its recorded basis. |
| `Acceptance` | The kernel's decision about a result under its contract. |
| `ConfirmationGrade` | The model's `Refuted`, `Unconfirmed`, `Discriminated` or `Confirmed(basis)` classification of an acceptance's evidence. |
| `Confirmation` | Explanatory foundation terminology for confirmation; the approved target represents its strength and basis with `ConfirmationGrade`. |
| `Knowledge` | A retained finding with scope, provenance and supporting evidence. |
| `Reputation` | An estimate based on qualified outcomes attributable to an agent's execution profile. |

Acceptance does not imply a `Confirmed` grade. Missing confirmation is not a failed
check, and an unknown state is not false. A8 uses belief and applicable evidence
coverage together for criterion satisfaction; probability alone is insufficient.
A7 governs acceptance and grades separately, and A12 governs competence credit.

`CriterionEvaluation` is named separately because the executable foundation can
report what a check observed before it implements immutable `ResultVersion`
capture and final `Acceptance`. Each recorded evaluation must retain its invocation
and environment attribution, must not be reused for a changed result, and cannot
by itself establish final acceptance or confirmation.

## Trusted services

| Canonical name | Responsibility |
| --- | --- |
| `Journal` | Atomically record ordered session facts and provide coherent histories. |
| `Dispatcher` | Coordinate the session lifecycle and route admitted actions and outcomes. |
| `Registry` | Resolve native identities, offerings, eligibility and effective capabilities. |
| `Treasury` | Reserve, account for and settle session resources. |
| `WorkspaceGuard` | Coordinate effective access, result capture and publication ownership. |
| `Gatekeeper` | Validate and admit assignments, constraints and authority. |
| `Arbiter` | Commit valid board decisions and responsibility transitions. |
| `AcceptanceAuthority` | Validate result-bound evidence, independence and final acceptance. |
| `ExperienceVault` | Retain qualified observations, knowledge, corrections and reputation. |

`SessionView` is an immutable projection of one session history at a stated
`Revision`. A revision is a stream position, not an event time or a claim of quality.

## Replaceable behavior

Strategies such as `Planner`, `ContributionPolicy`, `ReviewerPolicy`,
`FailureDiagnoser`, `ResourcePolicy` and `RetrievalPolicy` return proposals or
observations. Their identifiers, versions and actual inputs must accompany the
decisions they influence. Model-assisted strategies use ordinary admitted work.

`ExecutionBackend` supplies execution through the provider boundary. Its results
are observations, not authority to admit another assignment or accept a result.

The [foundation contract](foundation.md) states which names have executable
representations now. A name in this glossary is not evidence of implementation.

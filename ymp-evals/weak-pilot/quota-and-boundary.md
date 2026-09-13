# Proposed bounded execution envelope

Status: proposal for parent review; no spending authorization or native calls.

| Scope | Proposed value | Actual guarantee / limitation |
| --- | --- | --- |
| Outcomes | 12 | Six conditions times two measured tasks; retain failures |
| Aggregate observed tokens per outcome | 12,000 input + output | Admission/stop threshold; not an in-flight token ceiling |
| Outer invocations per outcome | At most 8 | Shared across all participants and phases; one `run_turn` may itself contain several native requests |
| Outer group duration | 120 seconds | One external deadline, not 120 seconds per member; terminate/cancel in-flight work when it expires |
| Per-turn timeout | 45 seconds | Also subject to the remaining group deadline |
| Application attempts | 1 | No application retries; native internal retries still require verified controls |
| Scheduler parallelism | 2 | Common ceiling; independent candidates may be scheduled separately, while conflicting/unbounded Engine writes remain serialized |
| Visible native output | 16,000 characters per invocation | Stream/result limit, not a model-token bound |
| Native turns | 2 where enforced | Codex enforcement is not established; this cannot bound Codex requests |
| Effort | Explicit fixed `low` for both proposed models | Must bind requested and sent controls, preserving unknown native acknowledgment |
| Raw Store admission slots | 9 | One protected unused final-review slot plus the external maximum of 8 actual calls; no raw solo self-acceptance |
| Cooperation Store admission slots | 8 | Final independent weak review is included in both roster and allowance |
| Money | Unknown | Parent must approve separately from cross-model token counts |

The 12-outcome proposal permits at most 96 **outer invocations**, with summed
observed-token thresholds of 144,000; neither figure is an upper bound on native
API requests or billed tokens. The 24-minute sum of group deadlines excludes
preparation, post-freeze external scoring and cleanup overhead. Public-only
selection must fit inside the same condition deadline. At most two outer
invocations may be active in a condition (solo has one participant). Their
unreported and unfinished native work may exceed the observed threshold; local
cancellation is not proof that remote
billing stopped. A missing/partial final usage snapshot stops further admission.
No continuation is allowed under unknown accounting.

The scheduler ceiling of two lets the current Engine assign two ready tasks to
distinct producers before its ordinary workspace coordinator serializes
conflicting writes. A three-profile roster alone does not prove three active
participants: record both captured roster and actually invoked agents. If only
two agents were invoked, retain that outcome with an explicit protocol deviation;
do not label it evidence from three working agents or silently rerun it. The
autonomous cooperation-3 proof uses two visible protocol tasks and requires actual
invocations from all three actors. This does not force future model-generated
plans to use all three participants.

These values replace product defaults for this proposal. They are not inherited
from 200 calls, 900 seconds per turn or three attempts. The autonomous proof uses
smaller synthetic values to force failures quickly; synthetic usage and elapsed
time are not extrapolated into native costs.

## Existing APIs and the missing guarantees

- Raw solo and isolated groups must call the existing `ymp_providers::run_turn`
  boundary, collect all `ProviderEvent`s, and share production Store admission
  across the group. `ymp ask` discards usage and does not apply session admission;
  it is not a measurement command. Distinct participant contexts must start with
  `resume: None` and no inherited usage baseline or peer answers.
- Cooperation must use `Engine`, fixed size/roster of two or three real agent
  profiles, ordinary assignments, and an independent nonproducing weak reviewer.
  `Engine.executable` must point to the existing `ymp` for native MCP, or delegate
  directly to the existing stdio bridge. No replacement authority protocol is
  allowed. A standalone group chat is a different experiment.
- Pin `Config.execution[agent_id].fixed` to the captured model and `low`.
  Merely setting defaults is insufficient. Policy validation must reject an
  incompatible allocation; it must never silently replace low with high/xhigh.
- `TurnRequest` / `NativeResourceControls` currently do not expose demonstrated
  switches for disabling Codex automatic subagents, native memory or native
  retries. A prompt prohibition does not enforce the participant count. Codex
  `max_turns` is not established as enforced. Do not guess configuration keys or
  assert that a native context equals exactly one model request.
- Provider cwd alone does not prove that hidden fixtures, peer directories,
  user application data or persistent native context cannot be read. This package
  deliberately supplies no private data to the request; native access confinement
  still needs evidence.

Until these guarantees are verified against the actual native API and supported
configuration, `native --manifest ... --output ...` writes a concrete
`blocked_before_native` record and refuses before invoking the provider. It has
no flag that turns unsupported controls into an assertion of safety. No
production authority is relaxed. The missing boundary is recorded, not worked
around through fabricated identities, final-review purpose or the scripted
artifact writer. YMP-148 is not a prerequisite.

The standalone public selector has a five-second timeout per candidate; it does
not enforce a deadline shared with prior native work. A future enabled native
runner must supervise selection under the same absolute condition deadline.
The current offline proof establishes the group deadline for provider invocations
and the Engine loop, not a complete native experiment including that integration.

## Commands requiring a later parent decision

1. Freeze the generated 12-attempt manifest, exact tool/access controls, models,
   limits, selection rule and protocol/source hashes.
2. After resolving the missing guarantees, approve and validate the concrete
   native invocation command in fresh explicitly selected task and metadata
   directories. Native authentication stays with the installed provider; tokens
   are never copied into the study or application metadata.
3. Run conditions in the recorded order, persist raw observations on every exit,
   freeze selected artifacts, score blind once, and join scores to conditions
   only after scoring. No retry/repair is authorized by a failed hidden check.

There is currently no approved runnable native experiment command. Offline
`stage`, `matrix`, `public_select`, `seal`, `score` and `offline-proof` commands
are concrete and reviewable now. The explicit native refusal is itself tested
without launching Codex, Claude, npx or any model.

# Manual file-communication pilot

## Status and evidentiary boundary

`RUN-001` records an **infrastructure-invalid exploratory run** performed on 2026-09-01. It is not
admitted calibration, a communication experiment, an arm observation, or evidence for
`weak-diagnostic-v1`. The disposable root was
`/tmp/ymp-file-comm-pilot.qKu2Ay`; its repository copy used base
[fbf52f7](https://github.com/maggnus/ymp/commit/fbf52f763f0dc2a3badff3e21fae33dc6469cd87).
The exploratory override used `codex-cli 0.151.0`, `gpt-5.6-luna`, low reasoning, isolated
`HOME`, `CODEX_HOME`, `YMP_HOME`, and `TMPDIR`, workspace-write sandboxing, and disabled network,
plugins, and subagents. That exact profile was unadmitted.

The run is permanently stopped. Participants B and C remain unlaunched; no continuation, selective
retry, post-hoc call increase, or reuse of this manifest is admissible. A future pilot must start
from a new manifest and budget after profile admission and cannot change the frozen diagnostic's
arms, tasks, seeds, budgets, outcomes, or the primary comparison.

## Planned file protocol

The planned three-call synthetic task used an append-only `project/shared/board.md` and no primary
task or seed. A held `{R4Q9, T8V1, N5X0, C9D4}` and B held
`{T8V1, P3L6, C9D4, F6J3}`; each was to append exactly one self-authored bounded message while
preserving existing bytes. C held `{B2H7, C9D4, N5X0, F6J3}`, was to read the board, infer the unique
three-set intersection, and write only `answer-C.json`. The protected expected answer was `C9D4`.
This uncontrolled pilot could at most have exposed early feasibility; even a correct answer would
not by itself have established listening or value beyond an independent control.

## Execution chronology

Three launch checks ended with zero model usage:

1. the copied runtime required `codex-cli 0.147.0`, while the installed executable was `0.151.0`;
2. incompatible feature flags were removed before a request was sent;
3. the shared directory was initially rejected by Git trust before a request was sent.

The exploratory manifest originally planned three participant calls. After the first model-bearing
compatibility attempt, the total ceiling was frozen at four calls: one compatibility call plus A,
B, and C. That first call (`A0`) emitted a completed event but refused because the workspace tool
host was unavailable. A compatibility modification intended to expose workspace tools preceded the
second call (`A1`), but its launch trace still disabled `code_mode_host`; A1 again emitted a
completed refusal and did not append. A further A retry plus B and C would have required three calls
with only two left, so the run stopped rather than extend its budget. The evolving preflight and
manifest are an additional reason the run is exploratory, not preregistered evidence.

The two exact outputs were:

> Unable to complete: the workspace tool host is unavailable, so I could not safely inspect or append to `board.md`.

> Unable to access the workspace tools, so I could not safely append to `board.md`.

The board remained byte-for-byte unchanged at SHA-256
`7b1cc1735bde309b557725c045a79582352aab37161921f62e29c3515c0731bb`. No message was
published, no bytes were available or delivered to a receiver, no later receiver action occurred,
and neither listening nor task value was observable. The refusals avoided fabricated file effects;
that is useful refusal behaviour, not self-organization evidence.

## Usage and artifact identities

| Call | Input | Cached input | Output | Reasoning | Wall time | Cost |
|---|---:|---:|---:|---:|---:|---|
| A0 compatibility | 39,276 | 28,672 | 544 | 321 | 34,326 ms sidecar; 34,325 ms terminal event | unavailable (`null`) |
| A1 participant | 39,271 | 28,672 | 439 | 218 | 27,382 ms | unavailable (`null`) |
| Total | 78,547 | 57,344 | 983 | 539 | 61,708 ms by sidecars | unavailable |

Both calls recorded zero protected queries, empty per-model cost details, and no in-flight excess.
The one-millisecond A0 sidecar/event discrepancy is retained rather than normalized. Because
monetary cost was unavailable, the exact profile also fails the existing complete-accounting
admission boundary
([`cal-001`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/research/cal-001-calibration.md#L69-L77)).

| Ephemeral artifact | SHA-256 |
|---|---|
| `export/manifest.json` | `9928045379a4fa629ea456fcd6d48f5aa5859a2108eddeb0e50f6d940dad6419` |
| `export/probe.json` | `e368ffd1b7f93c900bc37f7aa6eef07e8d08f61f6016a183b0be2fc310b0ca15` |
| A0 events | `06893d64b2b7ae2cc48ea95d17207c71ba7dad309af93ad417f6da457f0a4496` |
| A1 events | `5973406e4e8111bb38f48ce716937155b4168ef9044bcd0c90fe0e16f1a481ee` |
| A0 output | `b88913d8c1dde8670a672a856d09e576f3df56bd366db405b7e3d005074fff29` |
| A1 output | `e41a197407e9d7b9fd26fa55aa54917a82f8929de10c44395dccb2783fd4363d` |
| A0 usage | `896c20617812926154dd5d708751724d3b0770e0acf64e3684d4b0af45b12f72` |
| A1 usage | `4b2dab147230f312eb492f1a7d74643ee7e00b161359e7890dedd203009e7481` |
| exploratory harness source | `307576d58691e0ef433496a81eb0616efc1ea9b274f52d9ee9a1b99c21ccb461` |

These raw artifacts were not imported into the repository and remain under an ephemeral `/tmp`
root at the time of this record. Their hashes identify the observed bytes; after that root is
deleted, this record cannot reproduce or reverify those bytes and must not be described as a
reproducible evidence package.

## Profile-admission disposition

The proposed two-stage distinction is accepted with one refinement:

1. **Zero-model launch-path compatibility** may establish executable version, flags, Git trust,
   isolated configuration, generated tool binding, and fake-host conformance. It is not
   “exact-route” evidence because no model route or model tool choice is exercised.
2. **Model tool-use admission** is one separately frozen, tightly capped, no-task-output call in a
   disposable project. A random nonce absent from the prompt is stored in the sole allowed input;
   the model must read it, write the exact bytes to the sole allowed output, read them back through
   the bound tools, terminate honestly, and expose complete usage and monetary-cost accounting.

Stage 2 is admission expenditure, never an arm observation, and does not replace lifecycle,
isolation, cancellation, or descendant-cleanup evidence. Its exact prompt, nonce-generation rule,
input and output paths, allowed effects, environment identity, call/token/time/cost ceiling, and
terminal rule freeze before launch. The read-back trace establishes tool use and returned bytes,
not comprehension or listening.

The sufficiency of stage 1 is falsified by one exact profile that passes a complete stage-1 fixture
but fails stage 2 for a model/tool-use reason under the identical effective binding. Conversely,
RUN-001 does not itself establish that discordance: its `ready` probe was weaker than the required
fake-host stage, and the intended binding change was absent from the second launch trace. It does
falsify probe-only readiness as admission. A missing or wrong nonce, absent attributable
read/write/read-back trace, unauthorized effect, dishonest terminal, budget excess, or missing
usage/cost leaves the profile unadmitted and keeps the first model experiment at **STOP**.

## Scientific conclusion and blind spots

The only supported result is infrastructure invalidity of this exact exploratory path. There is no
evidence about signaling, delivery, receiver action, listening, task value, language or message
representation, fixed-two coordination, or comparative performance. The current real-model gate
also remains downstream of deterministic S1-S3 transport conformance and the content-independent
schedule required by
[`W1-EVL-04e`](https://github.com/maggnus/ymp/blob/59288d11b400a00ee22c8cc36f9a80e4077aea1c/ymp-docs/work/waves/W1/W1-EVL-04/tasks/W1-EVL-04e.md#L53-L100).

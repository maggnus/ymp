# Cumulative-knowledge minimum — advisory consultation

Status: advisory research input, not a frozen experiment, implementation evidence or POC conclusion.
Author: `claude/claude-fable-5-1`, `xhigh`, read-only consultation on 6 September 2026.
Source tree: [e1fd980](https://github.com/maggnus/ymp/commit/e1fd98032becce5d88180275f6255cf44a2a3d88).
Owner: W2-DEF-01. No model experiment or product execution occurred.

## Proposed minimum

The consultation proposes a first two-task transfer demonstration. An inert text note at subtask granularity records a procedure or localization lesson extracted from a previously accepted attempt A, with provenance and a digest frozen before a new task B. Existing payload/export or source-snapshot facilities may carry the note; `ContractDocument` has no arbitrary attachment field. A deterministic delivery path is enough for this first experiment; semantic search is a later independent mechanism.

Observe delivery and any subsequent tool use, then judge B with an independent verifier. Compare the same receiver profile in fresh sessions with the experience-derived note, no note, and a similarly sized generic note. Record costs and every outcome. A later collective-benefit claim also needs one agent with access to the exact same note bytes.

The researcher suggested bstr→toml and serde-json→semver as possible pairs from the existing corpus. These are suggestions only: no task selection, compatibility, independence or permission to reuse a frozen corpus is established by this consultation. The future protocol owns fresh task selection and its pre-collection freeze.

## Evidence and transfer limits

- [Break It Down, Pass It On](https://arxiv.org/abs/2608.20274), a preprint submitted 20 August 2026: the inspected abstract reports negative transfer from coarse task-level skills and better average transfer from text subtask skills. Repository repair and independent external-oracle transfer remain untested here.
- [ExpeL](https://arxiv.org/abs/2308.10144): experience-derived insights support a single-agent memory control; reported QA, ALFWorld and WebShop results do not establish coding-agent collective benefit.
- [Voyager](https://voyager.minedojo.org/): executable skill reuse in Minecraft motivates procedural units. Its self-verification does not replace ymp's independent oracle.
- [G-Memory](https://arxiv.org/abs/2506.07398): fixed multi-agent schemes outside repository development; [Darwin Gödel Machine](https://arxiv.org/abs/2505.22954): costly agent-code archive search. Neither architecture is required for the first note-transfer demonstration.

## Falsifiers and unresolved interpretation

Digest changes after B, leaked protected inputs or lost execution/evidence integrity invalidate an affected observation. A note that does not improve outcomes compared with the controls supplies no demonstrated useful transfer. The exact decision rule and trial limits are still to be written before live collection.

The consultation proposes read traces or a nonce read-back as observability candidates. The future protocol must distinguish proved delivery, a participant choosing not to read/use an available note, and missing instrumentation; these are different outcomes. Echoing bytes is not evidence of understanding. This clarification is required before any causal conclusion.

A small series can establish an executable chain and report exact observations and variation. It does not establish a population effect or general collective intelligence. Infrastructure failures must be retained and separately labelled; they cannot be recoded as failed solutions or silently removed from accounting.

## Unverified and next boundary

Runtime read-trace fidelity, `capture_exclusions`, the full newest-paper experiments, eligible task pairs, effect size and live profile admission remain unverified. RUN-003 left the live model gate closed. A fake-runtime chain can prepare delivery, digest binding, no-leak checks and honest terminals without model expenditure.

This minimum is one possible first measurement, not the whole cumulative-knowledge POC. W2-DEF-01 must state the additional observations needed to claim accumulation across a task sequence, transfer, and any later collective advantage. The representation, task set and total budget remain open until the executable preparation answers those questions.
